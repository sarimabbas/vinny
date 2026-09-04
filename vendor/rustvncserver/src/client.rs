// Copyright 2025 Dustin McAfee
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! VNC client connection handling and protocol implementation.
//!
//! This module manages individual VNC client sessions, handling:
//! - RFB protocol handshake and negotiation
//! - Client message processing (input events, encoding requests, etc.)
//! - Framebuffer update transmission with batching and rate limiting
//! - Client-specific state management (pixel format, encodings, dirty regions)
//!
//! # Protocol Flow
//!
//! 1. **Handshake**: Protocol version exchange and security negotiation
//! 2. **Initialization**: Send framebuffer dimensions and pixel format
//! 3. **Message Loop**: Handle incoming client messages and send framebuffer updates
//!
//! # Performance Features
//!
//! - **Update Deferral**: Batches small changes to reduce message overhead
//! - **Region Merging**: Combines overlapping dirty regions for efficiency
//! - **Encoding Selection**: Chooses optimal encoding based on client capabilities
//! - **Rate Limiting**: Prevents overwhelming clients with excessive update frequency

use bytes::{Buf, BufMut, BytesMut};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compress;
use flate2::Compression;
use flate2::FlushCompress;
use log::error;
#[cfg(feature = "debug-logging")]
use log::info;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_rustls::rustls;
use tokio_rustls::TlsAcceptor;

use crate::encoding;
use crate::encoding::tight::TightStreamCompressor;
use crate::framebuffer::{DirtyRegion, Framebuffer};
use crate::protocol::{
    PixelFormat, Rectangle, ServerInit, CLIENT_MSG_CLIENT_CUT_TEXT,
    CLIENT_MSG_ENABLE_CONTINUOUS_UPDATES, CLIENT_MSG_FENCE, CLIENT_MSG_FRAMEBUFFER_UPDATE_REQUEST,
    CLIENT_MSG_KEY_EVENT, CLIENT_MSG_POINTER_EVENT, CLIENT_MSG_QEMU, CLIENT_MSG_SET_DESKTOP_SIZE,
    CLIENT_MSG_SET_ENCODINGS, CLIENT_MSG_SET_PIXEL_FORMAT, ENCODING_COMPRESS_LEVEL_0,
    ENCODING_COMPRESS_LEVEL_9, ENCODING_CONTINUOUS_UPDATES, ENCODING_COPYRECT, ENCODING_CORRE,
    ENCODING_CURSOR, ENCODING_DESKTOP_NAME, ENCODING_DESKTOP_SIZE, ENCODING_EXTENDED_CLIPBOARD,
    ENCODING_EXTENDED_DESKTOP_SIZE, ENCODING_FENCE, ENCODING_HEXTILE, ENCODING_LAST_RECT,
    ENCODING_QEMU_EXTENDED_KEY_EVENT, ENCODING_QUALITY_LEVEL_0, ENCODING_QUALITY_LEVEL_9,
    ENCODING_RAW, ENCODING_RRE, ENCODING_TIGHT, ENCODING_TIGHTPNG, ENCODING_ZLIB, ENCODING_ZLIBHEX,
    ENCODING_ZRLE, ENCODING_ZYWRLE, PROTOCOL_VERSION, PROTOCOL_VERSION_3_3, PROTOCOL_VERSION_3_7,
    PROTOCOL_VERSION_3_8, SECURITY_RESULT_FAILED, SECURITY_RESULT_OK, SECURITY_TYPE_NONE,
    SECURITY_TYPE_VENCRYPT, SERVER_MSG_END_OF_CONTINUOUS_UPDATES, SERVER_MSG_FENCE,
    SERVER_MSG_FRAMEBUFFER_UPDATE, SERVER_MSG_SERVER_CUT_TEXT,
};
use rfb_encodings::translate;

#[cfg(not(test))]
const AUTH_FAILURE_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const AUTH_FAILURE_DELAY: Duration = Duration::from_millis(20);

/// Represents various events that a VNC client can send to the server.
/// These events typically correspond to user interactions like keyboard input,
/// pointer movements, or clipboard updates.
/// An asynchronous bidirectional transport used after RFB security negotiation.
pub trait RfbStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Sync> RfbStream for T {}

/// The independently lockable write half of an RFB transport.
pub type ClientWriteStream = tokio::io::WriteHalf<Box<dyn RfbStream>>;

type ClientReadStream = tokio::io::ReadHalf<Box<dyn RfbStream>>;

#[derive(Clone)]
pub(crate) enum SecurityConfig {
    None,
    VeNCrypt {
        tls: Arc<rustls::ServerConfig>,
        password: Arc<str>,
    },
}

impl SecurityConfig {
    pub(crate) fn from_password(password: Option<String>) -> Result<Self, std::io::Error> {
        let Some(password) = password else {
            return Ok(Self::None);
        };
        let certificate = rcgen::generate_simple_self_signed(vec!["vinny.local".into()])
            .map_err(std::io::Error::other)?;
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
        let tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                rustls::pki_types::PrivateKeyDer::Pkcs8(key),
            )
            .map_err(std::io::Error::other)?;
        Ok(Self::VeNCrypt {
            tls: Arc::new(tls),
            password: password.into(),
        })
    }
}

pub(crate) enum ClientCommand {
    CutText(String),
    DesktopName(String),
    DesktopSize {
        width: u16,
        height: u16,
        sent: oneshot::Sender<()>,
    },
}

pub enum ClientEvent {
    /// A key press or release event.
    /// - `down`: `true` if the key is pressed, `false` if released.
    /// - `key`: The X Window System keysym of the key.
    KeyPress { down: bool, key: u32 },
    /// A layout-independent XT keycode supplied through the QEMU key event extension.
    ExtendedKeyPress {
        down: bool,
        keysym: u32,
        keycode: u32,
    },
    /// A pointer (mouse) movement or button event.
    /// - `x`: The X-coordinate of the pointer.
    /// - `y`: The Y-coordinate of the pointer.
    /// - `button_mask`: A bitmask indicating which mouse buttons are pressed.
    PointerMove { x: u16, y: u16, button_mask: u8 },
    /// A client-side clipboard (cut text) update.
    /// - `text`: The textual content from the client's clipboard.
    CutText { text: String },
    /// Notification that the client has disconnected.
    Disconnected,
}

/// Manages persistent zlib compression streams for Tight encoding.
///
/// Per RFC 6143 Tight encoding specification, uses 4 separate zlib streams
/// to maintain compression dictionaries:
/// - Stream 0: Full-color (truecolor) data
/// - Stream 1: Mono rect (2-color bitmap) data
/// - Stream 2: Indexed palette (3-16 colors) data
/// - Stream 3: Unused (reserved)
///
/// Each stream maintains its own dictionary and compression level, allowing
/// dynamic compression parameter changes without reinitializing the stream.
pub struct TightZlibStreams {
    /// Array of 4 zlib compression streams
    streams: [Option<Compress>; 4],
    /// Active flag for each stream
    active: [bool; 4],
    /// Compression level for each stream
    levels: [u8; 4],
}

impl TightZlibStreams {
    /// Creates a new `TightZlibStreams` with all streams uninitialized.
    pub fn new() -> Self {
        Self {
            streams: [None, None, None, None],
            active: [false; 4],
            levels: [0; 4],
        }
    }

    /// Gets or initializes a stream for the given stream ID and compression level.
    ///
    /// Implements lazy initialization and dynamic level changes:
    /// - On first use: Initialize stream with zlib
    /// - On level change: Update compression level dynamically
    /// - Otherwise: Use existing stream with preserved dictionary
    ///
    /// # Arguments
    /// * `stream_id` - The stream ID (0-3)
    /// * `level` - Desired compression level (0-9)
    ///
    /// # Returns
    /// Mutable reference to the initialized Compress stream
    fn get_or_init_stream(&mut self, stream_id: usize, level: u8) -> &mut Compress {
        assert!(stream_id < 4, "stream_id must be 0-3");

        if !self.active[stream_id] {
            // Initialize stream on first use
            self.streams[stream_id] = Some(Compress::new(Compression::new(u32::from(level)), true));
            self.active[stream_id] = true;
            self.levels[stream_id] = level;
        } else if self.levels[stream_id] != level {
            // Compression level changed - Don't recreate the stream!
            // Changing compression level mid-session with persistent streams is problematic:
            // - Recreating the stream resets the dictionary, causing client decompression errors
            // - Using set_level() can corrupt the stream state
            //
            // The safest approach: Keep using the ORIGINAL compression level for this stream.
            // The client's compression level preference mainly affects NEW streams.
            // This matches behavior of other VNC servers (e.g., TigerVNC).
            //
            // Do nothing - keep using self.levels[stream_id]
        }

        self.streams[stream_id].as_mut().unwrap()
    }

    /// Compresses data using the specified stream with `Z_SYNC_FLUSH`.
    ///
    /// Uses `Z_SYNC_FLUSH` to maintain the dictionary state for subsequent compressions
    /// per RFC 6143 Tight encoding specification.
    ///
    /// CRITICAL: This function does NOT reset the stream between calls! The stream maintains
    /// its dictionary state across multiple compressions, which allows the client to decompress
    /// the data using the same persistent stream state. This is essential for TIGHT encoding.
    ///
    /// # Arguments
    /// * `stream_id` - The stream ID (0-3)
    /// * `level` - Desired compression level (0-9)
    /// * `input` - Data to compress
    ///
    /// # Returns
    /// Compressed data, or error if compression fails
    #[allow(clippy::cast_possible_truncation)] // Zlib total_out limited to buffer size, safe to truncate
    fn compress(&mut self, stream_id: usize, level: u8, input: &[u8]) -> Result<Vec<u8>, String> {
        let stream = self.get_or_init_stream(stream_id, level);

        // Prepare output buffer (worst case: input size + overhead)
        let mut output = vec![0u8; input.len() + 64];

        // Compress with Z_SYNC_FLUSH to preserve dictionary for next compression
        // IMPORTANT: Do NOT reset() the stream! We need to maintain the dictionary state.
        let before_out = stream.total_out();

        match stream.compress(input, &mut output, FlushCompress::Sync) {
            Ok(flate2::Status::Ok | flate2::Status::StreamEnd) => {
                let total_out = (stream.total_out() - before_out) as usize;
                output.truncate(total_out);
                Ok(output)
            }
            Ok(flate2::Status::BufError) => Err("Compression buffer error".to_string()),
            Err(e) => Err(format!("Compression failed: {e}")),
        }
    }
}

/// Implement `TightStreamCompressor` trait for `TightZlibStreams`.
/// This allows the tight encoding module to use our stream manager.
impl TightStreamCompressor for TightZlibStreams {
    fn compress_tight_stream(
        &mut self,
        stream_id: u8,
        level: u8,
        input: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.compress(stream_id as usize, level, input)
    }
}

/// Manages a single VNC client connection, handling communication, framebuffer updates,
/// and client input events.
///
/// This struct encapsulates the state and logic for interacting with a connected VNC viewer.
/// It is responsible for sending framebuffer updates to the client based on dirty regions,
/// processing incoming client messages (e.g., key events, pointer events, pixel format requests),
/// and managing client-specific settings like preferred encodings and JPEG quality.
async fn throttle_failed_authentication(authenticated: bool) {
    if !authenticated {
        // A fixed delay plus the client ceiling throttles guesses without lockout state.
        tokio::time::sleep(AUTH_FAILURE_DELAY).await;
    }
}

fn constant_time_eq(expected: &[u8], supplied: &[u8]) -> bool {
    if expected.len() != supplied.len() {
        return false;
    }
    expected
        .iter()
        .zip(supplied)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

fn default_wire_pixel_format() -> PixelFormat {
    // Conventional RFB 32-bit true-colour format: pixel value 0x00RRGGBB,
    // transmitted as B, G, R, padding on little-endian clients.
    PixelFormat {
        bits_per_pixel: 32,
        depth: 24,
        big_endian_flag: 0,
        true_colour_flag: 1,
        red_max: 255,
        green_max: 255,
        blue_max: 255,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProtocolVersion {
    V3_3,
    V3_7,
    V3_8,
}

impl ProtocolVersion {
    fn parse(bytes: &[u8; 12]) -> Result<Self, std::io::Error> {
        match bytes {
            PROTOCOL_VERSION_3_3 | b"RFB 003.005\n" => Ok(Self::V3_3),
            PROTOCOL_VERSION_3_7 => Ok(Self::V3_7),
            PROTOCOL_VERSION_3_8 => Ok(Self::V3_8),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported RFB protocol version",
            )),
        }
    }
}

pub struct VncClient {
    /// The read half of the TCP stream for receiving client messages.
    read_stream: ClientReadStream,
    /// The write half of the transport for sending updates to the client.
    write_stream: Arc<tokio::sync::Mutex<ClientWriteStream>>,
    /// A reference to the framebuffer, used to retrieve pixel data for updates.
    framebuffer: Framebuffer,
    shared: bool,
    /// The pixel format requested by the client, protected by a `RwLock` for concurrent access.
    /// It is written by the message handler and read by the encoder.
    pixel_format: RwLock<PixelFormat>, // Protected - written by message handler, read by encoder
    /// The list of preferred encodings supported by the client, protected by a `RwLock`.
    /// It is written by the message handler and read by the encoder.
    encodings: RwLock<Vec<i32>>, // Protected - written by message handler, read by encoder
    /// Sender for client events (e.g., key presses, pointer movements) to be processed by other parts of the server.
    event_tx: mpsc::UnboundedSender<ClientEvent>,
    /// The `Instant` when the last framebuffer update was sent to this client, protected by a `RwLock`.
    /// Used for rate limiting and deferral logic.
    last_update_sent: RwLock<Instant>, // Protected - written by update sender, read by rate limiter
    /// The JPEG quality level for encodings, stored as an `AtomicU8` for atomic access from multiple contexts.
    jpeg_quality: AtomicU8, // Atomic - simple u8 value accessed from multiple contexts
    /// The compression level for encodings (e.g., Zlib), stored as an `AtomicU8` for atomic access.
    compression_level: AtomicU8, // Atomic - simple u8 value accessed from multiple contexts
    /// The VNC quality level (0-9, or 255 for unset = use JPEG).
    /// Stored as an `AtomicU8` for atomic access from multiple contexts.
    quality_level: AtomicU8, // Atomic - VNC quality level (0-9, 255=unset)
    /// Whether the client supports the `ContinuousUpdates` extension (advertised via -313 pseudo-encoding).
    /// When true, server has sent `EndOfContinuousUpdates` and client can send `EnableContinuousUpdates`.
    supports_continuous_updates: AtomicBool, // Atomic - set when client advertises -313
    supports_cursor: AtomicBool,
    supports_fence: AtomicBool,
    supports_last_rect: AtomicBool,
    supports_desktop_size: AtomicBool,
    supports_extended_desktop_size: AtomicBool,
    supports_desktop_name: AtomicBool,
    supports_extended_key_event: AtomicBool,
    supports_extended_clipboard: AtomicBool,
    /// Whether continuous updates are currently enabled via the `ContinuousUpdates` extension.
    /// When true, server pushes updates without waiting for `FramebufferUpdateRequest`.
    continuous_updates_enabled: AtomicBool, // Atomic - set by EnableContinuousUpdates message
    /// The region for which continuous updates are enabled (when using `ContinuousUpdates` extension).
    continuous_updates_region: RwLock<Option<DirtyRegion>>, // Protected - set by EnableContinuousUpdates
    /// Legacy flag: whether server is actively sending updates after `FramebufferUpdateRequest`.
    /// Used when client does NOT support `ContinuousUpdates` extension (traditional VNC behavior).
    update_requested: AtomicBool, // Atomic - set by FramebufferUpdateRequest, cleared after update sent
    /// A shared, locked vector of `DirtyRegion`s specific to this client.
    /// These regions represent areas of the framebuffer that have been modified and need to be sent to the client.
    modified_regions: Arc<RwLock<Vec<DirtyRegion>>>, // Per-client dirty regions (standard VNC protocol style - receives pushes from framebuffer)
    /// The region specifically requested by the client for an update, protected by a `RwLock`.
    /// It is written by the message handler and read by the encoder.
    requested_region: RwLock<Option<DirtyRegion>>, // Protected - written by message handler, read by encoder
    /// `CopyRect` tracking (standard VNC protocol style): destination regions to be copied
    copy_region: Arc<RwLock<Vec<DirtyRegion>>>, // Destination regions for CopyRect
    /// Translation vector for `CopyRect`: (dx, dy) where src = dest + (dx, dy)
    copy_offset: RwLock<Option<(i16, i16)>>, // (dx, dy) translation for copy operations
    /// The duration to defer sending updates, matching `standard VNC protocol`'s default.
    defer_update_time: Duration, // Constant - set once at init
    /// The timestamp (in nanoseconds since creation) when deferring of updates began (0 if not deferring).
    /// Stored as an `AtomicU64` for atomic access.
    start_deferring_nanos: AtomicU64, // Atomic - nanos since creation (0 = not deferring)
    /// The `Instant` when this `VncClient` instance was created, used for calculating elapsed time.
    creation_time: Instant, // Constant - for calculating elapsed time
    /// The maximum number of rectangles to send in a single framebuffer update message, matching `standard VNC protocol`'s default.
    max_rects_per_update: usize, // Constant - set once at init
    /// A mutex used to ensure exclusive access to the client's `TcpStream` for sending data,
    /// preventing interleaved writes from concurrent tasks.
    send_mutex: Arc<tokio::sync::Mutex<()>>,
    /// Persistent zlib compressor for Zlib encoding (RFC 6143: one stream per connection).
    /// Protected by `RwLock` since encoding happens during `send_batched_update`.
    zlib_compressor: RwLock<Option<Compress>>,
    /// Persistent zlib compressor for `ZlibHex` encoding (RFC 6143: one stream per connection).
    /// Protected by `RwLock` since encoding happens during `send_batched_update`.
    zlibhex_compressor: RwLock<Option<Compress>>,
    /// Persistent zlib compressor for ZRLE encoding (RFC 6143: one stream per connection).
    /// Protected by `RwLock` since encoding happens during `send_batched_update`.
    #[allow(dead_code)]
    zrle_compressor: RwLock<Option<Compress>>,
    /// ZYWRLE quality level (0 = disabled, 1-3 = quality levels, higher = better quality).
    /// Stored as `AtomicU8` for atomic access. Updated based on client's quality setting.
    zywrle_level: AtomicU8, // Atomic - updated when ZYWRLE encoding is detected
    /// Persistent zlib compression streams for Tight encoding (4 streams with dictionaries).
    /// Protected by `RwLock` since encoding happens during `send_batched_update`.
    tight_zlib_streams: RwLock<TightZlibStreams>,
    /// Remote host address (IP:port) of the connected client
    remote_host: String,
    /// Destination port for repeater connections (None for direct connections)
    destination_port: Option<u16>,
    /// Repeater ID for repeater connections (None for direct connections)
    repeater_id: Option<String>,
    /// Request ID for tracking connection requests (optional, set by caller)
    request_id: Option<String>,
    /// Unique client ID assigned by the server
    client_id: usize,
    command_tx: mpsc::Sender<ClientCommand>,
    command_rx: Option<mpsc::Receiver<ClientCommand>>,
    pending_clipboard: Option<String>,
}

impl VncClient {
    /// Creates a new `VncClient` instance, performing the VNC handshake with the connected client.
    ///
    /// This function handles the initial protocol version exchange, security type negotiation,
    /// and sends the `ServerInit` message to the client, providing framebuffer information.
    ///
    /// # Arguments
    ///
    /// * `client_id` - The unique client ID assigned by the server.
    /// * `stream` - The `TcpStream` representing the established connection to the VNC client.
    /// * `framebuffer` - The `Framebuffer` instance that this client will receive updates from.
    /// * `desktop_name` - The name of the desktop to be sent to the client during `ServerInit`.
    /// * `password` - An optional password for VNC authentication. If `Some`, VNC authentication
    ///   will be offered. (Note: Current implementation uses a placeholder for authentication).
    /// * `event_tx` - An `mpsc::UnboundedSender` for sending `ClientEvent`s generated by the client
    ///   (e.g., key presses, pointer movements) to other parts of the server.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(VncClient)` on successful handshake and initialization, or
    /// `Err(std::io::Error)` if an I/O error occurs during communication or handshake.
    pub async fn new(
        client_id: usize,
        stream: TcpStream,
        framebuffer: Framebuffer,
        desktop_name: String,
        password: Option<String>,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
    ) -> Result<Self, std::io::Error> {
        let security = SecurityConfig::from_password(password)?;
        Self::new_with_security(
            client_id,
            stream,
            framebuffer,
            desktop_name,
            security,
            event_tx,
        )
        .await
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) async fn new_with_security(
        client_id: usize,
        mut stream: TcpStream,
        framebuffer: Framebuffer,
        desktop_name: String,
        security: SecurityConfig,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
    ) -> Result<Self, std::io::Error> {
        const X509_PLAIN: u32 = 262;
        const MAX_CREDENTIAL_LENGTH: usize = 1024;

        let remote_host = stream
            .peer_addr()
            .map_or_else(|_| "unknown".to_string(), |addr| addr.to_string());
        let _ = stream.set_nodelay(true);
        stream.write_all(PROTOCOL_VERSION.as_bytes()).await?;

        let mut version_buf = [0u8; 12];
        stream.read_exact(&mut version_buf).await?;
        let protocol_version = ProtocolVersion::parse(&version_buf)?;
        #[cfg(feature = "debug-logging")]
        info!("Client version: {}", String::from_utf8_lossy(&version_buf));

        let offered_security_type = match security {
            SecurityConfig::None => SECURITY_TYPE_NONE,
            SecurityConfig::VeNCrypt { .. } => SECURITY_TYPE_VENCRYPT,
        };
        if protocol_version == ProtocolVersion::V3_3
            && offered_security_type == SECURITY_TYPE_VENCRYPT
        {
            stream.write_all(&0u32.to_be_bytes()).await?;
            let reason = b"Encrypted servers require RFB 3.7 or newer";
            let reason_length = u32::try_from(reason.len()).map_err(std::io::Error::other)?;
            stream.write_all(&reason_length.to_be_bytes()).await?;
            stream.write_all(reason).await?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "VeNCrypt requires RFB 3.7 or newer",
            ));
        }

        if protocol_version == ProtocolVersion::V3_3 {
            stream
                .write_all(&u32::from(offered_security_type).to_be_bytes())
                .await?;
        } else {
            stream.write_all(&[1, offered_security_type]).await?;
            let mut selected = [0u8; 1];
            stream.read_exact(&mut selected).await?;
            if selected[0] != offered_security_type {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "client selected a security type that was not offered",
                ));
            }
        }

        let mut stream: Box<dyn RfbStream> = match security {
            SecurityConfig::None => {
                if protocol_version == ProtocolVersion::V3_8 {
                    stream.write_all(&SECURITY_RESULT_OK.to_be_bytes()).await?;
                }
                Box::new(stream)
            }
            SecurityConfig::VeNCrypt { tls, password } => {
                stream.write_all(&[0, 2]).await?;
                let mut version = [0u8; 2];
                stream.read_exact(&mut version).await?;
                if version != [0, 2] {
                    stream.write_all(&[1]).await?;
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "unsupported VeNCrypt version",
                    ));
                }
                stream.write_all(&[0, 1]).await?;
                stream.write_all(&X509_PLAIN.to_be_bytes()).await?;
                let mut subtype = [0u8; 4];
                stream.read_exact(&mut subtype).await?;
                if u32::from_be_bytes(subtype) != X509_PLAIN {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "client selected an unoffered VeNCrypt subtype",
                    ));
                }
                stream.write_all(&[1]).await?;
                let mut tls_stream = TlsAcceptor::from(tls).accept(stream).await?;
                let mut lengths = [0u8; 8];
                tls_stream.read_exact(&mut lengths).await?;
                let username_length = u32::from_be_bytes(lengths[..4].try_into().unwrap()) as usize;
                let password_length = u32::from_be_bytes(lengths[4..].try_into().unwrap()) as usize;
                if username_length > MAX_CREDENTIAL_LENGTH
                    || password_length > MAX_CREDENTIAL_LENGTH
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "VeNCrypt credentials are too large",
                    ));
                }
                let mut username = vec![0u8; username_length];
                let mut supplied_password = vec![0u8; password_length];
                tls_stream.read_exact(&mut username).await?;
                tls_stream.read_exact(&mut supplied_password).await?;
                let authenticated = constant_time_eq(password.as_bytes(), &supplied_password);
                throttle_failed_authentication(authenticated).await;
                tls_stream
                    .write_all(
                        &(if authenticated {
                            SECURITY_RESULT_OK
                        } else {
                            SECURITY_RESULT_FAILED
                        })
                        .to_be_bytes(),
                    )
                    .await?;
                if !authenticated {
                    if protocol_version == ProtocolVersion::V3_8 {
                        let reason = b"Authentication failed";
                        let reason_length =
                            u32::try_from(reason.len()).map_err(std::io::Error::other)?;
                        tls_stream.write_all(&reason_length.to_be_bytes()).await?;
                        tls_stream.write_all(reason).await?;
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "VeNCrypt authentication failed",
                    ));
                }
                Box::new(tls_stream)
            }
        };

        let mut shared = [0u8; 1];
        stream.read_exact(&mut shared).await?;
        let shared = shared[0] != 0;

        let server_init = ServerInit {
            framebuffer_width: framebuffer.width(),
            framebuffer_height: framebuffer.height(),
            pixel_format: default_wire_pixel_format(),
            name: desktop_name,
        };
        let mut init_buf = BytesMut::new();
        server_init.write_to(&mut init_buf);
        stream.write_all(&init_buf).await?;
        log::info!("VNC client handshake completed");

        let (read_stream, write_stream) = tokio::io::split(stream);

        let creation_time = Instant::now();
        let (command_tx, command_rx) = mpsc::channel(32);

        Ok(Self {
            read_stream,
            write_stream: Arc::new(tokio::sync::Mutex::new(write_stream)),
            framebuffer,
            shared,
            pixel_format: RwLock::new(default_wire_pixel_format()),
            encodings: RwLock::new(vec![ENCODING_RAW]),
            event_tx,
            last_update_sent: RwLock::new(creation_time),
            jpeg_quality: AtomicU8::new(80),     // Default quality
            compression_level: AtomicU8::new(6), // Default zlib compression (balanced)
            quality_level: AtomicU8::new(255),   // 255 = unset (use JPEG by default)
            supports_continuous_updates: AtomicBool::new(false), // Set when client advertises -313
            supports_cursor: AtomicBool::new(false),
            supports_fence: AtomicBool::new(false),
            supports_last_rect: AtomicBool::new(false),
            supports_desktop_size: AtomicBool::new(false),
            supports_extended_desktop_size: AtomicBool::new(false),
            supports_desktop_name: AtomicBool::new(false),
            supports_extended_key_event: AtomicBool::new(false),
            supports_extended_clipboard: AtomicBool::new(false),
            continuous_updates_enabled: AtomicBool::new(false), // Set by EnableContinuousUpdates
            continuous_updates_region: RwLock::new(None),       // Region for continuous updates
            update_requested: AtomicBool::new(false), // Legacy: set by FramebufferUpdateRequest
            modified_regions: Arc::new(RwLock::new(Vec::new())),
            requested_region: RwLock::new(None),
            copy_region: Arc::new(RwLock::new(Vec::new())), // Initialize empty copy region
            copy_offset: RwLock::new(None),                 // No copy offset initially
            defer_update_time: Duration::from_millis(5),    // Match standard VNC protocol default
            start_deferring_nanos: AtomicU64::new(0),       // 0 = not deferring
            creation_time,
            max_rects_per_update: 50, // Match standard VNC protocol default
            send_mutex: Arc::new(tokio::sync::Mutex::new(())),
            zlib_compressor: RwLock::new(None), // Initialized lazily when first used
            zlibhex_compressor: RwLock::new(None), // Initialized lazily when first used
            zrle_compressor: RwLock::new(None), // Initialized lazily when first used
            zywrle_level: AtomicU8::new(0), // Disabled by default, updated when ZYWRLE is requested
            tight_zlib_streams: RwLock::new(TightZlibStreams::new()), // 4 persistent streams for Tight encoding
            remote_host,
            destination_port: None, // None for direct inbound connections
            repeater_id: None,      // None for direct inbound connections
            request_id: None,       // None for direct inbound connections
            client_id,
            command_tx,
            command_rx: Some(command_rx),
            pending_clipboard: None,
        })
    }

    /// Returns a clone of the `Arc` containing the client's `modified_regions`.
    ///
    /// This handle is used to register the client with the `Framebuffer` to receive
    /// dirty region notifications.
    ///
    /// # Returns
    ///
    /// An `Arc<RwLock<Vec<DirtyRegion>>>` that can be used as a handle for the client's dirty regions.
    pub fn get_receiver_handle(&self) -> Arc<RwLock<Vec<DirtyRegion>>> {
        self.modified_regions.clone()
    }

    /// Returns a clone of the `Arc` containing the client's `copy_region`.
    ///
    /// This handle can be used to schedule copy operations for this client.
    ///
    /// # Returns
    ///
    /// An `Arc<RwLock<Vec<DirtyRegion>>>` that can be used as a handle for the client's copy regions.
    #[allow(dead_code)]
    pub fn get_copy_region_handle(&self) -> Arc<RwLock<Vec<DirtyRegion>>> {
        self.copy_region.clone()
    }

    /// Schedules a copy operation for this client (standard VNC protocol style).
    ///
    /// This method adds a region to be sent using `CopyRect` encoding with the specified offset.
    /// According to standard VNC protocol's algorithm, if a copy operation with a different offset
    /// already exists, the old copy region is treated as modified.
    ///
    /// # Arguments
    ///
    /// * `region` - The destination region to be copied.
    /// * `dx` - The X offset from destination to source (`src_x` = `dest_x` + dx).
    /// * `dy` - The Y offset from destination to source (`src_y` = `dest_y` + dy).
    pub async fn schedule_copy_region(&self, region: DirtyRegion, dx: i16, dy: i16) {
        let mut copy_regions = self.copy_region.write().await;
        let mut copy_offset = self.copy_offset.write().await;
        let mut modified_regions = self.modified_regions.write().await;

        // Check if we have an existing copy with a different offset
        if let Some((existing_dx, existing_dy)) = *copy_offset {
            if existing_dx != dx || existing_dy != dy {
                // Different offset - treat existing copy region as modified
                // This matches standard VNC protocol's behavior in rfbScheduleCopyRegion
                modified_regions.extend(copy_regions.drain(..));
                copy_regions.clear();
            }
        }

        // Add the new region to copy_region
        copy_regions.push(region);
        *copy_offset = Some((dx, dy));
    }

    /// Enters the main message loop for the `VncClient`, handling incoming data from the client
    /// and periodically sending framebuffer updates.
    ///
    /// This function continuously reads from the client's `TcpStream` and processes VNC messages
    /// such as `SetPixelFormat`, `SetEncodings`, `FramebufferUpdateRequest`, `KeyEvent`,
    /// `PointerEvent`, and `ClientCutText`. It also uses a `tokio::time::interval` to
    /// periodically check if batched framebuffer updates should be sent to the client,
    /// based on dirty regions and deferral logic.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the client disconnects gracefully.
    /// Returns `Err(std::io::Error)` if an I/O error occurs or an invalid message is received.
    #[allow(clippy::too_many_lines)] // VNC protocol message handler requires complete state machine
    #[allow(clippy::cast_possible_truncation)] // VNC protocol message fields use u8/u16/u32 as specified in RFC 6143
    #[allow(clippy::cast_sign_loss)] // VNC pseudo-encoding values are negative i32, converted to positive u8/u16 offsets
    pub async fn handle_messages(&mut self) -> Result<(), std::io::Error> {
        // Use standard VNC quality mapping (TigerVNC compatible)
        const TIGHT2TURBO_QUAL: [u8; 10] = [15, 29, 41, 42, 62, 77, 79, 86, 92, 100];
        // Limit clipboard size to prevent memory exhaustion attacks
        const MAX_CUT_TEXT: usize = 10 * 1024 * 1024; // 10MB limit

        let mut buf = BytesMut::with_capacity(4096);
        let mut check_interval = tokio::time::interval(tokio::time::Duration::from_millis(8));
        let mut command_rx = self
            .command_rx
            .take()
            .ok_or_else(|| std::io::Error::other("client message loop has already been started"))?;

        loop {
            tokio::select! {
                command = command_rx.recv() => match command {
                    Some(ClientCommand::CutText(text)) => {
                        if self.supports_extended_clipboard.load(Ordering::Relaxed) {
                            self.pending_clipboard = Some(text);
                            self.send_extended_clipboard_action((1 << 27) | 1, &[]).await?;
                        } else {
                            self.send_cut_text(text).await?;
                        }
                    }
                    Some(ClientCommand::DesktopName(name)) => self.send_desktop_name(&name).await?,
                    Some(ClientCommand::DesktopSize { width, height, sent }) => {
                        self.send_desktop_size(width, height).await?;
                        let _ = sent.send(());
                    }
                    None => {}
                },
                // Handle incoming client messages
                result = self.read_stream.read_buf(&mut buf) => {
                    if result? == 0 {
                        let _ = self.event_tx.send(ClientEvent::Disconnected);
                        return Ok(());
                    }

                    // Process all available messages in the buffer
                    while !buf.is_empty() {

                        let msg_type = buf[0];

                        match msg_type {
                            CLIENT_MSG_SET_PIXEL_FORMAT => {
                                if buf.len() < 20 { // 1 + 3 padding + 16 pixel format
                                    break; // Need more data
                                }
                                buf.advance(1); // message type
                                buf.advance(3); // padding
                                let requested_format = PixelFormat::from_bytes(&mut buf)?;

                                // Validate that the requested format is valid and supported
                                if !requested_format.is_valid() {
                                    error!(
                                        "Client requested invalid pixel format (bpp={}, depth={}, truecolor={}, shifts=R{},G{},B{}). Disconnecting.",
                                        requested_format.bits_per_pixel,
                                        requested_format.depth,
                                        requested_format.true_colour_flag,
                                        requested_format.red_shift,
                                        requested_format.green_shift,
                                        requested_format.blue_shift
                                    );
                                    let _ = self.event_tx.send(ClientEvent::Disconnected);
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Invalid pixel format requested"
                                    ));
                                }

                                // Accept the format and store it for translation during encoding
                                *self.pixel_format.write().await = requested_format.clone();
                                if self.supports_cursor.load(Ordering::Relaxed) {
                                    self.hide_client_cursor().await?;
                                }

                                #[cfg(feature = "debug-logging")]
                                {
                                    info!(
                                        "Client set pixel format: {}bpp, depth={}, bigEndian={}, R_shift={} R_max={}, G_shift={} G_max={}, B_shift={} B_max={} - compatible_with_rgba32={}",
                                        requested_format.bits_per_pixel,
                                        requested_format.depth,
                                        requested_format.big_endian_flag,
                                        requested_format.red_shift, requested_format.red_max,
                                        requested_format.green_shift, requested_format.green_max,
                                        requested_format.blue_shift, requested_format.blue_max,
                                        requested_format.is_compatible_with_rgba32()
                                    );
                                }
                            }
                            CLIENT_MSG_SET_ENCODINGS => {
                                if buf.len() < 4 {
                                    break;
                                }
                                let count = u16::from_be_bytes([buf[2], buf[3]]) as usize;
                                if buf.len() < 4 + count * 4 {
                                    break;
                                }
                                buf.advance(4);
                                let mut encodings_list = Vec::with_capacity(count);
                                for _ in 0..count {
                                    let encoding = buf.get_i32();
                                    encodings_list.push(encoding);

                                    // Check for quality level pseudo-encodings (-32 to -23)
                                    if (ENCODING_QUALITY_LEVEL_0..=ENCODING_QUALITY_LEVEL_9).contains(&encoding) {
                                        // -32 = level 0 (lowest), -23 = level 9 (highest)
                                        let quality_level = (encoding - ENCODING_QUALITY_LEVEL_0) as u8;
                                        let quality = TIGHT2TURBO_QUAL[quality_level as usize];
                                        self.jpeg_quality.store(quality, Ordering::Relaxed);
                                        self.quality_level.store(quality_level, Ordering::Relaxed); // Store VNC quality level
                                        #[cfg(feature = "debug-logging")]
                                        info!("Client requested quality level {quality_level}, using JPEG quality {quality}");
                                    }

                                    // Check for compression level pseudo-encodings (-256 to -247)
                                    if (ENCODING_COMPRESS_LEVEL_0..=ENCODING_COMPRESS_LEVEL_9).contains(&encoding) {
                                        // -256 = level 0 (lowest/fastest), -247 = level 9 (highest/slowest)
                                        let compression_level = (encoding - ENCODING_COMPRESS_LEVEL_0) as u8;
                                        // Use compression level directly (0=fastest, 9=best compression)
                                        self.compression_level.store(compression_level, Ordering::Relaxed);
                                        #[cfg(feature = "debug-logging")]
                                        info!("Client requested compression level {compression_level}, using zlib level {compression_level}");
                                    }

                                    if encoding == ENCODING_CURSOR
                                        && !self.supports_cursor.swap(true, Ordering::Relaxed)
                                    {
                                        self.hide_client_cursor().await?;
                                    }
                                    if encoding == ENCODING_LAST_RECT {
                                        self.supports_last_rect.store(true, Ordering::Relaxed);
                                    }
                                    if encoding == ENCODING_DESKTOP_SIZE {
                                        self.supports_desktop_size.store(true, Ordering::Relaxed);
                                    }
                                    if encoding == ENCODING_EXTENDED_DESKTOP_SIZE {
                                        self.supports_extended_desktop_size.store(true, Ordering::Relaxed);
                                    }
                                    if encoding == ENCODING_DESKTOP_NAME {
                                        self.supports_desktop_name.store(true, Ordering::Relaxed);
                                    }
                                    if encoding == ENCODING_QEMU_EXTENDED_KEY_EVENT
                                        && !self.supports_extended_key_event.swap(true, Ordering::Relaxed)
                                    {
                                        self.send_pseudo_rect(ENCODING_QEMU_EXTENDED_KEY_EVENT, 0, 0, 0, 0, &[]).await?;
                                    }
                                    if encoding == ENCODING_FENCE
                                        && !self.supports_fence.swap(true, Ordering::Relaxed)
                                    {
                                        self.send_fence(0, &[]).await?;
                                    }
                                    if encoding == ENCODING_EXTENDED_CLIPBOARD {
                                        self.supports_extended_clipboard.store(true, Ordering::Relaxed);
                                        self.send_extended_clipboard_caps().await?;
                                    }

                                    // Check for ContinuousUpdates pseudo-encoding (-313)
                                    if encoding == ENCODING_CONTINUOUS_UPDATES {
                                        // Client supports ContinuousUpdates extension
                                        // Send EndOfContinuousUpdates message to confirm support
                                        if !self.supports_continuous_updates.load(Ordering::Relaxed) {
                                            self.supports_continuous_updates.store(true, Ordering::Relaxed);
                                            #[cfg(feature = "debug-logging")]
                                            info!("Client supports ContinuousUpdates extension, sending EndOfContinuousUpdates");

                                            // Send EndOfContinuousUpdates message (1 byte: type 150)
                                            let _guard = self.send_mutex.lock().await;
                                            if let Err(e) = self.write_stream.lock().await.write_all(&[SERVER_MSG_END_OF_CONTINUOUS_UPDATES]).await {
                                                error!("Failed to send EndOfContinuousUpdates: {e}");
                                            }
                                        }
                                    }
                                }
                                self.encodings.write().await.clone_from(&encodings_list);
                                #[cfg(feature = "debug-logging")]
                                info!("Client set {count} encodings: {encodings_list:?}");
                            }
                            CLIENT_MSG_FRAMEBUFFER_UPDATE_REQUEST => {
                                if buf.len() < 10 { // 1 + 1 incremental + 8 (x, y, w, h)
                                    break;
                                }
                                buf.advance(1); // message type
                                let incremental = buf.get_u8() != 0;
                                let x = buf.get_u16();
                                let y = buf.get_u16();
                                let width = buf.get_u16();
                                let height = buf.get_u16();

                                #[cfg(feature = "debug-logging")]
                                info!("FramebufferUpdateRequest: incremental={incremental}, region=({x},{y} {width}x{height})");

                                if self.continuous_updates_enabled.load(Ordering::Relaxed) && incremental {
                                    continue;
                                }

                                // Track requested region (standard VNC protocol cl->requestedRegion)
                                *self.requested_region.write().await = Some(DirtyRegion::new(x, y, width, height));

                                // Mark that an update was requested (traditional VNC behavior)
                                // If ContinuousUpdates extension is enabled, this is ignored
                                self.update_requested.store(true, Ordering::Relaxed);

                                // Handle non-incremental updates (full refresh)
                                if !incremental {
                                    // Clear existing regions and mark full requested region as dirty
                                    let full_region = DirtyRegion::new(x, y, width, height);
                                    let mut regions = self.modified_regions.write().await;
                                    regions.clear();
                                    regions.push(full_region);
                                    #[cfg(feature = "debug-logging")]
                                    info!("Non-incremental update: added full region to dirty list");
                                }

                                // Start deferring if we have regions to send
                                // Note: There's a small window where regions could be drained between
                                // the check and the store, but this is acceptable - at worst we defer
                                // when the queue is already empty (harmless). Using a write lock here
                                // would hurt performance on this hot path.
                                {
                                    let regions = self.modified_regions.read().await;
                                    if !regions.is_empty() && self.start_deferring_nanos.load(Ordering::Relaxed) == 0 {
                                        // Not currently deferring, start now
                                        let nanos = Instant::now().duration_since(self.creation_time).as_nanos() as u64;
                                        self.start_deferring_nanos.store(nanos, Ordering::Relaxed);
                                    }
                                }
                            }
                            CLIENT_MSG_KEY_EVENT => {
                                if buf.len() < 8 { // 1 + 1 down + 2 padding + 4 key
                                    break;
                                }
                                buf.advance(1); // message type
                                let down = buf.get_u8() != 0;
                                buf.advance(2); // padding
                                let key = buf.get_u32();

                                let _ = self.event_tx.send(ClientEvent::KeyPress { down, key });
                            }
                            CLIENT_MSG_POINTER_EVENT => {
                                if buf.len() < 6 { // 1 + 1 button + 2 x + 2 y
                                    break;
                                }
                                buf.advance(1); // message type
                                let button_mask = buf.get_u8();
                                let x = buf.get_u16();
                                let y = buf.get_u16();

                                let _ = self.event_tx.send(ClientEvent::PointerMove {
                                    x,
                                    y,
                                    button_mask,
                                });
                            }
                            CLIENT_MSG_CLIENT_CUT_TEXT => {
                                if buf.len() < 8 {
                                    break;
                                }
                                let signed_length = i32::from_be_bytes(buf[4..8].try_into().unwrap());
                                let length = if signed_length < 0 {
                                    signed_length.checked_abs().map(|value| value as usize)
                                } else {
                                    Some(signed_length as usize)
                                }
                                .ok_or_else(|| std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "invalid cut text length",
                                ))?;
                                if length > MAX_CUT_TEXT {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Cut text too large",
                                    ));
                                }
                                if buf.len() < 8 + length {
                                    break;
                                }
                                buf.advance(8);
                                let payload = buf.split_to(length);
                                if signed_length >= 0 {
                                    let text: String = payload.iter().copied().map(char::from).collect();
                                    let _ = self.event_tx.send(ClientEvent::CutText { text });
                                } else if self.supports_extended_clipboard.load(Ordering::Relaxed) {
                                    self.handle_extended_clipboard(&payload).await?;
                                }
                            }
                            CLIENT_MSG_FENCE => {
                                if !self.supports_fence.load(Ordering::Relaxed) {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Fence used before capability negotiation",
                                    ));
                                }
                                if buf.len() < 9 {
                                    break;
                                }
                                let length = buf[8] as usize;
                                if length > 64 {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Fence payload exceeds 64 bytes",
                                    ));
                                }
                                if buf.len() < 9 + length {
                                    break;
                                }
                                buf.advance(4);
                                let flags = buf.get_u32();
                                let length = buf.get_u8() as usize;
                                let payload = buf.split_to(length);
                                if flags & 0x8000_0000 != 0 {
                                    self.send_fence(flags & 0x7, &payload).await?;
                                }
                            }
                            CLIENT_MSG_SET_DESKTOP_SIZE => {
                                if buf.len() < 8 {
                                    break;
                                }
                                let screens = buf[6] as usize;
                                let message_len = 8usize.saturating_add(screens.saturating_mul(16));
                                if buf.len() < message_len {
                                    break;
                                }
                                buf.advance(message_len);
                                if self.supports_extended_desktop_size.load(Ordering::Relaxed) {
                                    self.send_extended_desktop_size(
                                        1,
                                        1,
                                        self.framebuffer.width(),
                                        self.framebuffer.height(),
                                    ).await?;
                                }
                            }
                            CLIENT_MSG_QEMU => {
                                if buf.len() < 2 {
                                    break;
                                }
                                match buf[1] {
                                    0 => {
                                        if buf.len() < 12 {
                                            break;
                                        }
                                        buf.advance(2);
                                        let down = buf.get_u16() != 0;
                                        let keysym = buf.get_u32();
                                        let keycode = buf.get_u32();
                                        if self.supports_extended_key_event.load(Ordering::Relaxed) {
                                            let _ = self.event_tx.send(ClientEvent::ExtendedKeyPress {
                                                down,
                                                keysym,
                                                keycode,
                                            });
                                        }
                                    }
                                    _ => {
                                        return Err(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            "unsupported QEMU submessage",
                                        ));
                                    }
                                }
                            }
                            CLIENT_MSG_ENABLE_CONTINUOUS_UPDATES => {
                                if !self.supports_continuous_updates.load(Ordering::Relaxed) {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "ContinuousUpdates used before capability negotiation",
                                    ));
                                }
                                // EnableContinuousUpdates: enable(u8) + x(u16) + y(u16) + w(u16) + h(u16) = 10 bytes total
                                if buf.len() < 10 {
                                    break;
                                }
                                buf.advance(1); // message type
                                let enable = buf.get_u8() != 0;
                                let x = buf.get_u16();
                                let y = buf.get_u16();
                                let width = buf.get_u16();
                                let height = buf.get_u16();

                                if enable {
                                    // Enable continuous updates for the specified region
                                    let region = DirtyRegion::new(x, y, width, height);
                                    *self.continuous_updates_region.write().await = Some(region);
                                    self.continuous_updates_enabled.store(true, Ordering::Relaxed);
                                    #[cfg(feature = "debug-logging")]
                                    info!("ContinuousUpdates enabled for region ({x},{y} {width}x{height})");
                                } else {
                                    // Disable continuous updates
                                    *self.continuous_updates_region.write().await = None;
                                    self.continuous_updates_enabled.store(false, Ordering::Relaxed);
                                    #[cfg(feature = "debug-logging")]
                                    info!("ContinuousUpdates disabled");

                                    // Send EndOfContinuousUpdates to confirm disable
                                    let _guard = self.send_mutex.lock().await;
                                    if let Err(e) = self.write_stream.lock().await.write_all(&[SERVER_MSG_END_OF_CONTINUOUS_UPDATES]).await {
                                        error!("Failed to send EndOfContinuousUpdates: {e}");
                                    }
                                }
                            }
                            _ => {
                                error!("Unknown message type: {msg_type}, disconnecting client");
                                let _ = self.event_tx.send(ClientEvent::Disconnected);
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("Unknown message type: {msg_type}")
                                ));
                            }
                        }
                    }
                }

                // Periodically check if we should send updates
                _ = check_interval.tick() => {
                    // Determine if updates should be sent:
                    // - ContinuousUpdates extension enabled (client sent EnableContinuousUpdates with enable=true)
                    // - OR traditional mode: FramebufferUpdateRequest received (update_requested=true)
                    let cu_enabled = self.continuous_updates_enabled.load(Ordering::Relaxed);
                    let update_requested = self.update_requested.load(Ordering::Relaxed);

                    if cu_enabled || update_requested {
                        // Check if we have regions and deferral time has elapsed
                        // Regions are already pushed to us by framebuffer (no merge needed!)
                        let should_send = {
                            let regions = self.modified_regions.read().await;
                            if regions.is_empty() {
                                false
                            } else {
                                let defer_nanos = self.start_deferring_nanos.load(Ordering::Relaxed);
                                if defer_nanos == 0 {
                                    // Not currently deferring, start now
                                    let nanos = Instant::now().duration_since(self.creation_time).as_nanos() as u64;
                                    self.start_deferring_nanos.store(nanos, Ordering::Relaxed);
                                    false // Don't send yet, just started deferring
                                } else {
                                    // Check if defer time elapsed
                                    let defer_start = self.creation_time + Duration::from_nanos(defer_nanos);
                                    let now = Instant::now();
                                    let elapsed = now.duration_since(defer_start);
                                    let last_sent = *self.last_update_sent.read().await;
                                    let time_since_last = now.duration_since(last_sent);
                                    let min_interval = Duration::from_millis(8); // Headroom for 60 FPS capture

                                    elapsed >= self.defer_update_time && time_since_last >= min_interval
                                }
                            }
                        };

                        if should_send {
                            self.send_batched_update().await?;

                            if cu_enabled {
                                *self.requested_region.write().await =
                                    *self.continuous_updates_region.read().await;
                            } else {
                                self.update_requested.store(false, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        }
    }

    async fn send_pseudo_rect(
        &self,
        encoding: i32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        data: &[u8],
    ) -> Result<(), std::io::Error> {
        let mut response = BytesMut::with_capacity(16 + data.len());
        response.put_u8(SERVER_MSG_FRAMEBUFFER_UPDATE);
        response.put_u8(0);
        response.put_u16(1);
        Rectangle {
            x,
            y,
            width,
            height,
            encoding,
        }
        .write_header(&mut response);
        response.put_slice(data);
        let _guard = self.send_mutex.lock().await;
        self.write_stream.lock().await.write_all(&response).await
    }

    async fn send_fence(&self, flags: u32, payload: &[u8]) -> Result<(), std::io::Error> {
        let mut response = BytesMut::with_capacity(9 + payload.len());
        response.put_u8(SERVER_MSG_FENCE);
        response.put_bytes(0, 3);
        response.put_u32(flags);
        response.put_u8(u8::try_from(payload.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Fence payload exceeds 255 bytes",
            )
        })?);
        response.put_slice(payload);
        let _guard = self.send_mutex.lock().await;
        self.write_stream.lock().await.write_all(&response).await
    }

    async fn send_extended_desktop_size(
        &self,
        reason: u16,
        status: u16,
        width: u16,
        height: u16,
    ) -> Result<(), std::io::Error> {
        let mut data = BytesMut::with_capacity(20);
        data.put_u8(1);
        data.put_bytes(0, 3);
        data.put_u32(0);
        data.put_u16(0);
        data.put_u16(0);
        data.put_u16(width);
        data.put_u16(height);
        data.put_u32(0);
        self.send_pseudo_rect(
            ENCODING_EXTENDED_DESKTOP_SIZE,
            reason,
            status,
            width,
            height,
            &data,
        )
        .await
    }

    async fn hide_client_cursor(&self) -> Result<(), std::io::Error> {
        let format = self.pixel_format.read().await;
        let mut data = translate::translate_pixels(&[0, 0, 0, 0], &PixelFormat::rgba32(), &format);
        data.put_u8(0); // A zero mask makes this 1×1 cursor fully transparent.
        self.send_pseudo_rect(ENCODING_CURSOR, 0, 0, 1, 1, &data)
            .await
    }

    async fn send_desktop_size(&self, width: u16, height: u16) -> Result<(), std::io::Error> {
        if self.supports_extended_desktop_size.load(Ordering::Relaxed) {
            self.send_extended_desktop_size(0, 0, width, height).await
        } else if self.supports_desktop_size.load(Ordering::Relaxed) {
            self.send_pseudo_rect(ENCODING_DESKTOP_SIZE, 0, 0, width, height, &[])
                .await
        } else {
            Ok(())
        }
    }

    async fn send_desktop_name(&self, name: &str) -> Result<(), std::io::Error> {
        if !self.supports_desktop_name.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut data = BytesMut::with_capacity(4 + name.len());
        data.put_u32(u32::try_from(name.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "desktop name is too long")
        })?);
        data.put_slice(name.as_bytes());
        self.send_pseudo_rect(ENCODING_DESKTOP_NAME, 0, 0, 0, 0, &data)
            .await
    }

    async fn send_extended_clipboard_action(
        &self,
        flags: u32,
        body: &[u8],
    ) -> Result<(), std::io::Error> {
        let length = 4usize.saturating_add(body.len());
        let capacity = 8usize.saturating_add(length);
        let length = i32::try_from(length).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "clipboard payload too large",
            )
        })?;
        let mut response = BytesMut::with_capacity(capacity);
        response.put_u8(SERVER_MSG_SERVER_CUT_TEXT);
        response.put_bytes(0, 3);
        response.put_i32(-length);
        response.put_u32(flags);
        response.put_slice(body);
        let _guard = self.send_mutex.lock().await;
        self.write_stream.lock().await.write_all(&response).await
    }

    async fn send_extended_clipboard_provide(&self, text: &str) -> Result<(), std::io::Error> {
        let text = text.replace("\r\n", "\n").replace('\n', "\r\n");
        let mut uncompressed = BytesMut::with_capacity(5 + text.len());
        uncompressed.put_u32(u32::try_from(text.len() + 1).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "clipboard text is too long",
            )
        })?);
        uncompressed.put_slice(text.as_bytes());
        uncompressed.put_u8(0);
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        Write::write_all(&mut encoder, &uncompressed)?;
        let compressed = encoder.finish()?;
        self.send_extended_clipboard_action((1 << 28) | 1, &compressed)
            .await
    }

    async fn handle_extended_clipboard(&mut self, payload: &[u8]) -> Result<(), std::io::Error> {
        if payload.len() < 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "extended clipboard message is truncated",
            ));
        }
        let flags = u32::from_be_bytes(payload[..4].try_into().unwrap());
        let text = flags & 1 != 0;
        if flags & (1 << 25) != 0 && text {
            if let Some(pending) = self.pending_clipboard.as_deref() {
                self.send_extended_clipboard_provide(pending).await?;
            }
        } else if flags & (1 << 27) != 0 && text {
            self.send_extended_clipboard_action((1 << 25) | 1, &[])
                .await?;
        } else if flags & (1 << 28) != 0 && text {
            let mut decoder = ZlibDecoder::new(&payload[4..]);
            let mut uncompressed = Vec::new();
            Read::read_to_end(&mut decoder, &mut uncompressed)?;
            if uncompressed.len() < 4 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "extended clipboard text is truncated",
                ));
            }
            let length = u32::from_be_bytes(uncompressed[..4].try_into().unwrap()) as usize;
            if length == 0 || length > uncompressed.len() - 4 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "extended clipboard text length is invalid",
                ));
            }
            let bytes = &uncompressed[4..4 + length - 1];
            let text = String::from_utf8(bytes.to_vec())
                .map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "clipboard text is not UTF-8",
                    )
                })?
                .replace("\r\n", "\n");
            let _ = self.event_tx.send(ClientEvent::CutText { text });
        }
        Ok(())
    }

    async fn send_extended_clipboard_caps(&self) -> Result<(), std::io::Error> {
        const CLIPBOARD_TEXT: u32 = 1;
        const CLIPBOARD_CAPS: u32 = 1 << 24;
        const CLIPBOARD_REQUEST: u32 = 1 << 25;
        const CLIPBOARD_NOTIFY: u32 = 1 << 27;
        const CLIPBOARD_PROVIDE: u32 = 1 << 28;

        let mut payload = BytesMut::with_capacity(8);
        payload.put_u32(
            CLIPBOARD_TEXT
                | CLIPBOARD_CAPS
                | CLIPBOARD_REQUEST
                | CLIPBOARD_NOTIFY
                | CLIPBOARD_PROVIDE,
        );
        payload.put_u32(0);

        self.send_extended_clipboard_action(
            CLIPBOARD_TEXT
                | CLIPBOARD_CAPS
                | CLIPBOARD_REQUEST
                | CLIPBOARD_NOTIFY
                | CLIPBOARD_PROVIDE,
            &payload[4..],
        )
        .await
    }

    /// Sends a batched framebuffer update message to the client.
    ///
    /// This function implements standard VNC protocol's update sending algorithm:
    /// 1. Send `CopyRect` regions first (from `copy_region` with stored offset)
    /// 2. Then send modified regions (from `modified_regions`)
    ///
    /// The update includes multiple rectangles in a single message to improve efficiency.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(())` on successful transmission of the update, or
    /// `Err(std::io::Error)` if an I/O error occurs during encoding or sending.
    #[allow(clippy::too_many_lines)] // VNC framebuffer update encoding requires handling all encoding types
    #[allow(clippy::cast_possible_truncation)] // VNC protocol rectangle headers use u16 dimensions
    async fn send_batched_update(&mut self) -> Result<(), std::io::Error> {
        // Get requested region (standard VNC protocol: requestedRegion)
        let requested = *self.requested_region.read().await;

        #[cfg(feature = "debug-logging")]
        info!("send_batched_update called, requested region: {requested:?}");

        // STEP 1: Get copy regions to send (standard VNC protocol: copyRegion sent FIRST)
        let (copy_regions_to_send, copy_src_offset): (Vec<DirtyRegion>, Option<(i16, i16)>) = {
            let mut copy_regions = self.copy_region.write().await;
            let mut copy_offset = self.copy_offset.write().await;

            if copy_regions.is_empty() {
                (Vec::new(), None)
            } else {
                let offset = *copy_offset;
                let regions: Vec<DirtyRegion> = if let Some(req) = requested {
                    // Filter and drain: only take regions that intersect with requested region
                    // This preserves non-intersecting regions for later updates
                    let mut result = Vec::new();
                    copy_regions.retain(|region| {
                        if let Some(intersection) = region.intersect(&req) {
                            result.push(intersection);
                            false // Remove from copy_regions (drained)
                        } else {
                            true // Keep in copy_regions for later
                        }
                    });
                    result
                } else {
                    copy_regions.drain(..).collect()
                };

                // If we drained all regions, clear the offset
                if copy_regions.is_empty() {
                    *copy_offset = None;
                }

                (regions, offset)
            }
        };

        // STEP 2: Get modified regions to send (standard VNC protocol: modifiedRegion sent AFTER copyRegion)
        let modified_regions_to_send: Vec<DirtyRegion> = {
            let mut regions = self.modified_regions.write().await;

            if regions.is_empty() {
                Vec::new()
            } else {
                // Calculate how many regions we can send
                let remaining_slots = self
                    .max_rects_per_update
                    .saturating_sub(copy_regions_to_send.len());
                let num_rects = regions.len().min(remaining_slots);

                if let Some(req) = requested {
                    // Filter and drain: only take regions that intersect with requested region
                    // This preserves non-intersecting regions for later updates
                    let mut result = Vec::new();
                    let mut drained_count = 0;

                    regions.retain(|region| {
                        if drained_count >= num_rects {
                            true // Keep remaining regions (hit limit)
                        } else if let Some(intersection) = region.intersect(&req) {
                            result.push(intersection);
                            drained_count += 1;
                            false // Remove from regions (drained)
                        } else {
                            true // Keep in regions for later (doesn't intersect)
                        }
                    });
                    result
                } else {
                    // No requested region set, drain up to num_rects
                    regions.drain(..num_rects).collect()
                }
            }
        };

        // If no regions to send at all, nothing to do
        if copy_regions_to_send.is_empty() && modified_regions_to_send.is_empty() {
            #[cfg(feature = "debug-logging")]
            info!(
                "No regions to send (copy={}, modified={})",
                copy_regions_to_send.len(),
                modified_regions_to_send.len()
            );
            return Ok(());
        }

        #[cfg_attr(not(feature = "debug-logging"), allow(unused_variables))]
        let start = Instant::now();

        // Calculate total rectangles including CoRRE tiles
        // For CoRRE encoding, large rectangles are split into 255x255 tiles
        let mut total_rects = copy_regions_to_send.len();

        // Determine preferred encoding from client's list
        // Select the first encoding that the server supports, skipping COPYRECT
        let encodings = self.encodings.read().await;
        let preferred_encoding = encodings
            .iter()
            .find(|&&enc| {
                // Skip COPYRECT - it's only for copy operations, not general encoding
                if enc == ENCODING_COPYRECT {
                    return false;
                }
                // Check if this encoding is supported
                // Either it has explicit handling in client.rs or get_encoder returns Some
                matches!(
                    enc,
                    ENCODING_ZLIB
                        | ENCODING_ZLIBHEX
                        | ENCODING_ZRLE
                        | ENCODING_ZYWRLE
                        | ENCODING_TIGHT
                ) || encoding::get_encoder(enc).is_some()
            })
            .copied()
            .unwrap_or(ENCODING_RAW);
        drop(encodings);

        #[cfg(feature = "debug-logging")]
        info!("DEBUG: preferred_encoding = {preferred_encoding}");

        #[cfg(feature = "debug-logging")]
        info!(
            "DEBUG: modified_regions_to_send.len() = {}",
            modified_regions_to_send.len()
        );

        #[cfg(feature = "debug-logging")]
        info!(
            "DEBUG: copy_regions_to_send.len() = {}",
            copy_regions_to_send.len()
        );

        // For TIGHT encoding, pre-encode regions to determine rectangle count
        let mut tight_encoded_regions = Vec::new();
        if preferred_encoding == ENCODING_TIGHT {
            #[cfg(feature = "debug-logging")]
            info!(
                "DEBUG: Entering TIGHT pre-encoding block, {} regions",
                modified_regions_to_send.len()
            );

            // Get client's pixel format to pass to encoder
            let pixel_format = self.pixel_format.read().await;
            let client_format_clone = pixel_format.clone();
            drop(pixel_format);

            #[cfg(feature = "debug-logging")]
            info!(
                "DEBUG: Client pixel format: {}bpp",
                client_format_clone.bits_per_pixel
            );

            let mut tight_streams = self.tight_zlib_streams.write().await;

            #[cfg(feature = "debug-logging")]
            info!("DEBUG: Acquired tight_zlib_streams lock");

            for region in &modified_regions_to_send {
                #[cfg(feature = "debug-logging")]
                info!(
                    "DEBUG: Processing region {}x{} at ({}, {})",
                    region.width, region.height, region.x, region.y
                );

                let pixel_data = match self
                    .framebuffer
                    .get_rect(region.x, region.y, region.width, region.height)
                    .await
                {
                    Ok(data) => {
                        #[cfg(feature = "debug-logging")]
                        info!("DEBUG: Got pixel data, {} bytes", data.len());
                        data
                    }
                    Err(e) => {
                        error!(
                            "Failed to get rectangle ({}, {}, {}, {}): {}",
                            region.x, region.y, region.width, region.height, e
                        );
                        continue;
                    }
                };

                #[cfg(feature = "debug-logging")]
                info!(
                    "DEBUG: Calling encode_tight_rects for {}x{} with {}bpp",
                    region.width, region.height, client_format_clone.bits_per_pixel
                );

                let sub_rects = encoding::tight::encode_tight_rects(
                    &pixel_data,
                    region.width,
                    region.height,
                    self.quality_level.load(Ordering::Relaxed),
                    self.compression_level.load(Ordering::Relaxed),
                    &client_format_clone,
                    &mut *tight_streams,
                );

                #[cfg(feature = "debug-logging")]
                info!(
                    "DEBUG: encode_tight_rects returned {} sub-rectangles",
                    sub_rects.len()
                );

                #[cfg(feature = "debug-logging")]
                info!(
                    "TIGHT: region {}x{} split into {} sub-rectangles",
                    region.width,
                    region.height,
                    sub_rects.len()
                );

                total_rects += sub_rects.len();
                tight_encoded_regions.push((region, sub_rects));
            }
            drop(tight_streams);

            #[cfg(feature = "debug-logging")]
            info!("DEBUG: TIGHT pre-encoding complete, total_rects={total_rects}");
        } else {
            // Count rectangles for modified regions (accounting for CoRRE tiling)
            for region in &modified_regions_to_send {
                if preferred_encoding == ENCODING_CORRE
                    && (region.width > 255 || region.height > 255)
                {
                    // Count how many tiles this region will be split into
                    let num_tiles_x = region.width.div_ceil(255) as usize;
                    let num_tiles_y = region.height.div_ceil(255) as usize;
                    total_rects += num_tiles_x * num_tiles_y;
                } else {
                    total_rects += 1;
                }
            }
        }

        let mut response = BytesMut::new();

        // Message type
        response.put_u8(SERVER_MSG_FRAMEBUFFER_UPDATE);
        response.put_u8(0); // padding
        let use_last_rect = self.supports_last_rect.load(Ordering::Relaxed);
        response.put_u16(if use_last_rect {
            u16::MAX
        } else {
            total_rects as u16
        });

        #[cfg(feature = "debug-logging")]
        info!("Writing framebuffer update header: total_rects={total_rects}");

        #[cfg_attr(
            not(feature = "debug-logging"),
            allow(unused_variables, unused_assignments, unused_mut)
        )]
        let mut encoding_name = match preferred_encoding {
            ENCODING_TIGHT => "TIGHT",
            ENCODING_TIGHTPNG => "TIGHTPNG",
            ENCODING_ZYWRLE => "ZYWRLE",
            ENCODING_ZRLE => "ZRLE",
            ENCODING_ZLIBHEX => "ZLIBHEX",
            ENCODING_ZLIB => "ZLIB",
            ENCODING_HEXTILE => "HEXTILE",
            ENCODING_RRE => "RRE",
            ENCODING_CORRE => "CORRE",
            _ => "RAW",
        };

        #[cfg_attr(
            not(feature = "debug-logging"),
            allow(unused_variables, unused_assignments)
        )]
        let mut total_pixels = 0u64;
        #[cfg_attr(
            not(feature = "debug-logging"),
            allow(unused_variables, unused_assignments)
        )]
        let mut copy_rect_count = 0;

        // Load quality/compression settings atomically
        let jpeg_quality = self.jpeg_quality.load(Ordering::Relaxed);
        let compression_level = self.compression_level.load(Ordering::Relaxed);
        let _quality_level = self.quality_level.load(Ordering::Relaxed);

        // STEP 1: Send copy regions FIRST (standard VNC protocol style)
        if let Some((dx, dy)) = copy_src_offset {
            for region in &copy_regions_to_send {
                // Calculate source position from destination + offset
                // In standard VNC protocol: src = dest + (dx, dy)
                #[allow(clippy::cast_sign_loss)]
                // CopyRect offset calculation: dx/dy are i16, sum guaranteed positive
                let src_x = (i32::from(region.x) + i32::from(dx)) as u16;
                #[allow(clippy::cast_sign_loss)]
                // CopyRect offset calculation: dx/dy are i16, sum guaranteed positive
                let src_y = (i32::from(region.y) + i32::from(dy)) as u16;

                // Use CopyRect encoding
                let rect = Rectangle {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: region.height,
                    encoding: ENCODING_COPYRECT,
                };
                rect.write_header(&mut response);

                // CopyRect data is just src_x and src_y
                response.put_u16(src_x);
                response.put_u16(src_y);

                total_pixels += u64::from(region.width) * u64::from(region.height);
                copy_rect_count += 1;
            }
        }

        // STEP 2: Send modified regions (standard VNC protocol: sent AFTER copy regions)

        #[cfg(feature = "debug-logging")]
        info!("DEBUG: Starting STEP 2 - Send modified regions");

        // Handle TIGHT encoding separately (already pre-encoded)
        if preferred_encoding == ENCODING_TIGHT {
            use crate::protocol::UPDATE_BUF_SIZE;

            #[cfg(feature = "debug-logging")]
            info!(
                "DEBUG: In TIGHT output section, tight_encoded_regions.len()={}",
                tight_encoded_regions.len()
            );

            #[cfg(feature = "debug-logging")]
            let mut rect_count = 0;

            for (region, sub_rects) in &tight_encoded_regions {
                #[cfg(feature = "debug-logging")]
                info!(
                    "DEBUG: Processing output region {}x{} with {} sub-rects",
                    region.width,
                    region.height,
                    sub_rects.len()
                );

                for (rel_x, rel_y, w, h, encoded) in sub_rects {
                    // Calculate size of this rectangle (header + data)
                    let rect_size = 12 + encoded.len(); // 12 bytes header + encoded data

                    // Check if adding this rectangle would exceed buffer limit
                    if response.len() + rect_size > UPDATE_BUF_SIZE {
                        #[cfg(feature = "debug-logging")]
                        info!("DEBUG: Buffer limit reached ({} bytes), flushing to continue streaming", response.len());

                        // Send current buffer chunk
                        let mut send_mutex = self.write_stream.lock().await;
                        send_mutex.write_all(&response).await?;
                        drop(send_mutex);

                        // Clear buffer and continue streaming rectangles
                        // Header was already sent in first flush, subsequent flushes are just raw rectangle data
                        response.clear();
                    }

                    // Sub-rectangle coordinates are relative to region origin
                    // Convert to absolute screen coordinates
                    let rect = Rectangle {
                        x: region.x + rel_x,
                        y: region.y + rel_y,
                        width: *w,
                        height: *h,
                        encoding: ENCODING_TIGHT,
                    };

                    #[cfg(feature = "debug-logging")]
                    info!("RECT #{}: {}x{} at ({},{}), TIGHT data={} bytes, response_size_before={}, response_size_after={}",
                        rect_count, w, h, region.x + rel_x, region.y + rel_y, encoded.len(), response.len(), response.len() + rect_size);

                    rect.write_header(&mut response);
                    response.extend_from_slice(encoded);

                    total_pixels += u64::from(*w) * u64::from(*h);

                    #[cfg(feature = "debug-logging")]
                    {
                        rect_count += 1;
                    }
                }
            }

            #[cfg(feature = "debug-logging")]
            info!(
                "DEBUG: TIGHT output complete, wrote {} rectangle headers, response.len()={}",
                rect_count,
                response.len()
            );
        } else {
            // Handle other encodings
            for region in &modified_regions_to_send {
                // For CoRRE encoding: split large rectangles into 255x255 tiles
                // (CoRRE uses u8 coordinates, so dimensions must be ≤255)
                if preferred_encoding == ENCODING_CORRE
                    && (region.width > 255 || region.height > 255)
                {
                    #[cfg(feature = "debug-logging")]
                    info!(
                        "CoRRE: Splitting {}x{} region into 255x255 tiles",
                        region.width, region.height
                    );
                    // Split rectangle into tiles ≤255x255 per RFC 6143 CoRRE specification
                    let mut y = 0;
                    while y < region.height {
                        let tile_height = std::cmp::min(255, region.height - y);
                        let mut x = 0;
                        while x < region.width {
                            let tile_width = std::cmp::min(255, region.width - x);
                            #[cfg(feature = "debug-logging")]
                            info!(
                                "CoRRE: Encoding tile at ({},{}) size {}x{}",
                                region.x + x,
                                region.y + y,
                                tile_width,
                                tile_height
                            );

                            // Get pixel data for this tile
                            let tile_pixel_data = match self
                                .framebuffer
                                .get_rect(region.x + x, region.y + y, tile_width, tile_height)
                                .await
                            {
                                Ok(data) => data,
                                Err(e) => {
                                    error!(
                                        "Failed to get rectangle ({}, {}, {}, {}): {}",
                                        region.x + x,
                                        region.y + y,
                                        tile_width,
                                        tile_height,
                                        e
                                    );
                                    x += tile_width;
                                    continue;
                                }
                            };

                            // Encode this tile with CoRRE
                            if let Some(encoder) = encoding::get_encoder(ENCODING_CORRE) {
                                let encoded = encoder.encode(
                                    &tile_pixel_data,
                                    tile_width,
                                    tile_height,
                                    jpeg_quality,
                                    compression_level,
                                );

                                // Calculate nSubrects from encoded buffer size
                                // Encoder returns: bgColor(4) + subrects, each subrect is 8 bytes
                                let n_subrects = if encoded.len() >= 4 {
                                    (encoded.len() - 4) / 8
                                } else {
                                    0
                                };

                                // Write rectangle header for this tile
                                let rect = Rectangle {
                                    x: region.x + x,
                                    y: region.y + y,
                                    width: tile_width,
                                    height: tile_height,
                                    encoding: ENCODING_CORRE,
                                };
                                rect.write_header(&mut response);

                                // Write RRE header (nSubrects in big-endian) - protocol layer responsibility
                                // CoRRE uses same header structure as RRE per RFC 6143
                                response.put_u32(n_subrects as u32);

                                // Write encoder output (background color + subrectangle data)
                                response.extend_from_slice(&encoded);

                                total_pixels += u64::from(tile_width) * u64::from(tile_height);
                            }

                            x += tile_width;
                        }
                        y += tile_height;
                    }
                    continue; // Skip normal encoding path for this region
                }

                // Get pixel data
                let pixel_data = match self
                    .framebuffer
                    .get_rect(region.x, region.y, region.width, region.height)
                    .await
                {
                    Ok(data) => data,
                    Err(e) => {
                        error!(
                            "Failed to get rectangle ({}, {}, {}, {}): {}",
                            region.x, region.y, region.width, region.height, e
                        );
                        continue; // Skip this invalid rectangle
                    }
                };

                // Apply pixel format translation and encode
                // Translation happens before encoding per RFC 6143
                let client_pixel_format = self.pixel_format.read().await;
                let server_format = PixelFormat::rgba32();

                let (actual_encoding, encoded) = if preferred_encoding == ENCODING_RAW {
                    // For Raw encoding: translation IS the encoding (like standard VNC protocol)
                    // Just translate and send directly, no additional processing
                    let translated = if client_pixel_format.is_compatible_with_rgba32() {
                        // Fast path: no translation, but still need to strip alpha
                        let mut buf = BytesMut::with_capacity(
                            (region.width as usize * region.height as usize) * 4,
                        );
                        for chunk in pixel_data.chunks_exact(4) {
                            buf.put_u8(chunk[0]); // R
                            buf.put_u8(chunk[1]); // G
                            buf.put_u8(chunk[2]); // B
                            buf.put_u8(0); // Padding (not alpha)
                        }
                        buf
                    } else {
                        // Translate from server format (RGBA32) to client's requested format
                        translate::translate_pixels(
                            &pixel_data,
                            &server_format,
                            &client_pixel_format,
                        )
                    };
                    (ENCODING_RAW, translated)
                } else if preferred_encoding == ENCODING_ZLIB {
                    // Translate pixels to client format first
                    let translated = if client_pixel_format.is_compatible_with_rgba32() {
                        // Fast path: no translation, but still need to strip alpha
                        let mut buf = BytesMut::with_capacity(
                            (region.width as usize * region.height as usize) * 4,
                        );
                        for chunk in pixel_data.chunks_exact(4) {
                            buf.put_u8(chunk[0]); // R
                            buf.put_u8(chunk[1]); // G
                            buf.put_u8(chunk[2]); // B
                            buf.put_u8(0); // Padding (not alpha)
                        }
                        buf
                    } else {
                        // Translate from server format (RGBA32) to client's requested format
                        translate::translate_pixels(
                            &pixel_data,
                            &server_format,
                            &client_pixel_format,
                        )
                    };

                    // Initialize ZLIB compressor lazily on first use
                    let mut zlib_lock = self.zlib_compressor.write().await;
                    if zlib_lock.is_none() {
                        *zlib_lock = Some(Compress::new(
                            Compression::new(u32::from(compression_level)),
                            true,
                        ));
                        #[cfg(feature = "debug-logging")]
                        info!("Initialized ZLIB compressor with level {compression_level}");
                    }
                    let zlib_comp = zlib_lock.as_mut().unwrap();

                    match encoding::encode_zlib_persistent(&translated, zlib_comp) {
                        Ok(data) => (ENCODING_ZLIB, BytesMut::from(&data[..])),
                        Err(e) => {
                            error!("ZLIB encoding failed: {e}, falling back to RAW");
                            #[cfg(feature = "debug-logging")]
                            {
                                encoding_name = "RAW";
                            }
                            // translated already contains the correctly formatted data
                            (ENCODING_RAW, translated)
                        }
                    }
                } else if preferred_encoding == ENCODING_ZLIBHEX {
                    // Translate pixels to client format first
                    let translated = if client_pixel_format.is_compatible_with_rgba32() {
                        // Fast path: no translation, but still need to strip alpha
                        let mut buf = BytesMut::with_capacity(
                            (region.width as usize * region.height as usize) * 4,
                        );
                        for chunk in pixel_data.chunks_exact(4) {
                            buf.put_u8(chunk[0]); // R
                            buf.put_u8(chunk[1]); // G
                            buf.put_u8(chunk[2]); // B
                            buf.put_u8(0); // Padding (not alpha)
                        }
                        buf
                    } else {
                        // Translate from server format (RGBA32) to client's requested format
                        translate::translate_pixels(
                            &pixel_data,
                            &server_format,
                            &client_pixel_format,
                        )
                    };

                    // Initialize ZLIBHEX compressor lazily on first use
                    let mut zlibhex_lock = self.zlibhex_compressor.write().await;
                    if zlibhex_lock.is_none() {
                        *zlibhex_lock = Some(Compress::new(
                            Compression::new(u32::from(compression_level)),
                            true,
                        ));
                        #[cfg(feature = "debug-logging")]
                        info!("Initialized ZLIBHEX compressor with level {compression_level}");
                    }
                    let zlibhex_comp = zlibhex_lock.as_mut().unwrap();

                    match encoding::encode_zlibhex_persistent(
                        &translated,
                        region.width,
                        region.height,
                        zlibhex_comp,
                    ) {
                        Ok(data) => (ENCODING_ZLIBHEX, BytesMut::from(&data[..])),
                        Err(e) => {
                            error!("ZLIBHEX encoding failed: {e}, falling back to RAW");
                            #[cfg(feature = "debug-logging")]
                            {
                                encoding_name = "RAW";
                            }
                            // translated already contains the correctly formatted data
                            (ENCODING_RAW, translated)
                        }
                    }
                } else if preferred_encoding == ENCODING_ZRLE {
                    // Translate pixels to client format first
                    let translated = if client_pixel_format.is_compatible_with_rgba32() {
                        // Fast path: no translation, but still need to strip alpha
                        let mut buf = BytesMut::with_capacity(
                            (region.width as usize * region.height as usize) * 4,
                        );
                        for chunk in pixel_data.chunks_exact(4) {
                            buf.put_u8(chunk[0]); // R
                            buf.put_u8(chunk[1]); // G
                            buf.put_u8(chunk[2]); // B
                            buf.put_u8(0); // Padding (not alpha)
                        }
                        buf
                    } else {
                        // Translate from server format (RGBA32) to client's requested format
                        translate::translate_pixels(
                            &pixel_data,
                            &server_format,
                            &client_pixel_format,
                        )
                    };

                    // Initialize ZRLE compressor lazily on first use
                    let mut zrle_lock = self.zrle_compressor.write().await;
                    if zrle_lock.is_none() {
                        *zrle_lock = Some(Compress::new(
                            Compression::new(u32::from(compression_level)),
                            true,
                        ));
                        #[cfg(feature = "debug-logging")]
                        info!("Initialized ZRLE compressor with level {compression_level}");
                    }
                    let zrle_comp = zrle_lock.as_mut().unwrap();

                    // Use client's pixel format for encoding
                    match encoding::encode_zrle_persistent(
                        &translated,
                        region.width,
                        region.height,
                        &client_pixel_format,
                        zrle_comp,
                    ) {
                        Ok(data) => (ENCODING_ZRLE, BytesMut::from(&data[..])),
                        Err(e) => {
                            error!("ZRLE encoding failed: {e}, falling back to RAW");
                            #[cfg(feature = "debug-logging")]
                            {
                                encoding_name = "RAW";
                            }
                            // translated already contains the correctly formatted data
                            (ENCODING_RAW, translated)
                        }
                    }
                } else if preferred_encoding == ENCODING_ZYWRLE {
                    // ZYWRLE: Apply wavelet preprocessing then use ZRLE encoder
                    let level = self.zywrle_level.load(Ordering::Relaxed) as usize;

                    // Allocate coefficient buffer for wavelet transform
                    let buf_size = (region.width as usize) * (region.height as usize);
                    let mut coeff_buf = vec![0i32; buf_size];

                    // Apply ZYWRLE wavelet preprocessing
                    let result = if let Some(transformed_data) = encoding::zywrle_analyze(
                        &pixel_data,
                        region.width as usize,
                        region.height as usize,
                        level,
                        &mut coeff_buf,
                    ) {
                        // Translate the wavelet-transformed data to client format
                        let translated = if client_pixel_format.is_compatible_with_rgba32() {
                            // Fast path: no translation, but still need to strip alpha
                            let mut buf = BytesMut::with_capacity(
                                (region.width as usize * region.height as usize) * 4,
                            );
                            for chunk in transformed_data.chunks_exact(4) {
                                buf.put_u8(chunk[0]); // R
                                buf.put_u8(chunk[1]); // G
                                buf.put_u8(chunk[2]); // B
                                buf.put_u8(0); // Padding (not alpha)
                            }
                            buf
                        } else {
                            // Translate from server format (RGBA32) to client's requested format
                            translate::translate_pixels(
                                &transformed_data,
                                &server_format,
                                &client_pixel_format,
                            )
                        };

                        // Now encode the translated data with ZRLE (shares the ZRLE compressor)
                        let mut zrle_lock = self.zrle_compressor.write().await;
                        if zrle_lock.is_none() {
                            *zrle_lock = Some(Compress::new(
                                Compression::new(u32::from(compression_level)),
                                true,
                            ));
                            #[cfg(feature = "debug-logging")]
                            info!(
                            "Initialized ZRLE compressor for ZYWRLE with level {compression_level}"
                        );
                        }
                        let zrle_comp = zrle_lock.as_mut().unwrap();

                        // Use client's pixel format for encoding
                        match encoding::encode_zrle_persistent(
                            &translated,
                            region.width,
                            region.height,
                            &client_pixel_format,
                            zrle_comp,
                        ) {
                            Ok(data) => (ENCODING_ZYWRLE, BytesMut::from(&data[..])),
                            Err(e) => {
                                error!("ZYWRLE encoding failed: {e}, falling back to RAW");
                                #[cfg(feature = "debug-logging")]
                                {
                                    encoding_name = "RAW";
                                }
                                // translated already contains the correctly formatted data
                                (ENCODING_RAW, translated)
                            }
                        }
                    } else {
                        // Analysis failed (dimensions too small), fall back to RAW with translation
                        error!(
                            "ZYWRLE analysis failed (dimensions too small), falling back to RAW"
                        );
                        #[cfg(feature = "debug-logging")]
                        {
                            encoding_name = "RAW";
                        }
                        // Translate original pixel_data for RAW fallback
                        let translated = if client_pixel_format.is_compatible_with_rgba32() {
                            let mut buf = BytesMut::with_capacity(
                                (region.width as usize * region.height as usize) * 4,
                            );
                            for chunk in pixel_data.chunks_exact(4) {
                                buf.put_u8(chunk[0]); // R
                                buf.put_u8(chunk[1]); // G
                                buf.put_u8(chunk[2]); // B
                                buf.put_u8(0); // Padding
                            }
                            buf
                        } else {
                            translate::translate_pixels(
                                &pixel_data,
                                &server_format,
                                &client_pixel_format,
                            )
                        };
                        (ENCODING_RAW, translated)
                    };
                    result
                } else if let Some(encoder) = encoding::get_encoder(preferred_encoding) {
                    // For other encodings (TightPng, Hextile): translate first then encode
                    let translated = if client_pixel_format.is_compatible_with_rgba32() {
                        // Fast path: no translation, but still need to strip alpha
                        let mut buf = BytesMut::with_capacity(
                            (region.width as usize * region.height as usize) * 4,
                        );
                        for chunk in pixel_data.chunks_exact(4) {
                            buf.put_u8(chunk[0]); // R
                            buf.put_u8(chunk[1]); // G
                            buf.put_u8(chunk[2]); // B
                            buf.put_u8(0); // Padding (not alpha)
                        }
                        buf
                    } else {
                        // Translate from server format (RGBA32) to client's requested format
                        translate::translate_pixels(
                            &pixel_data,
                            &server_format,
                            &client_pixel_format,
                        )
                    };
                    (
                        preferred_encoding,
                        encoder.encode(
                            &translated,
                            region.width,
                            region.height,
                            jpeg_quality,
                            compression_level,
                        ),
                    )
                } else {
                    // Fallback to RAW encoding if preferred encoding is not available
                    error!("Encoding {preferred_encoding} not available, falling back to RAW");
                    #[cfg(feature = "debug-logging")]
                    {
                        encoding_name = "RAW"; // Update encoding name to reflect fallback
                    }
                    // Translate for RAW fallback
                    let translated = if client_pixel_format.is_compatible_with_rgba32() {
                        let mut buf = BytesMut::with_capacity(
                            (region.width as usize * region.height as usize) * 4,
                        );
                        for chunk in pixel_data.chunks_exact(4) {
                            buf.put_u8(chunk[0]); // R
                            buf.put_u8(chunk[1]); // G
                            buf.put_u8(chunk[2]); // B
                            buf.put_u8(0); // Padding
                        }
                        buf
                    } else {
                        translate::translate_pixels(
                            &pixel_data,
                            &server_format,
                            &client_pixel_format,
                        )
                    };
                    (ENCODING_RAW, translated)
                };

                // Write rectangle header with actual encoding used
                let rect = Rectangle {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: region.height,
                    encoding: actual_encoding,
                };
                rect.write_header(&mut response);
                response.extend_from_slice(&encoded);

                total_pixels += u64::from(region.width) * u64::from(region.height);
            }
        }

        if use_last_rect {
            Rectangle {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                encoding: ENCODING_LAST_RECT,
            }
            .write_header(&mut response);
        }

        // Acquire send mutex to prevent interleaved writes
        #[cfg(feature = "debug-logging")]
        info!("DEBUG: About to send response, total_rects={}, response.len()={}, copy_rect_count={}, modified_regions={}",
            total_rects, response.len(), copy_rect_count, modified_regions_to_send.len());

        let lock = self.send_mutex.lock().await;

        #[cfg(feature = "debug-logging")]
        info!(
            "DEBUG: Acquired send_mutex, calling write_all with {} bytes",
            response.len()
        );

        self.write_stream.lock().await.write_all(&response).await?;

        #[cfg(feature = "debug-logging")]
        info!("DEBUG: write_all completed successfully");

        drop(lock);

        // Reset deferral timer and update last sent time
        self.start_deferring_nanos.store(0, Ordering::Relaxed); // Reset deferral
        *self.last_update_sent.write().await = Instant::now();

        #[cfg(feature = "debug-logging")]
        {
            let elapsed = start.elapsed();
            info!(
                "Sent {} rects ({} CopyRect + {} encoded, {} pixels total) using {} ({} bytes, {}ms encode+send)",
                total_rects, copy_rect_count, modified_regions_to_send.len(), total_pixels, encoding_name, response.len(), elapsed.as_millis()
            );
        }

        Ok(())
    }

    /// Sends a `ServerCutText` message to the client, updating its clipboard.
    ///
    /// # Arguments
    ///
    /// * `text` - The string to be sent as the clipboard content.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful transmission, or `Err(std::io::Error)` if an I/O error occurs.
    #[allow(clippy::cast_possible_truncation)] // Clipboard text length limited to u32 per VNC protocol
    pub async fn send_cut_text(&mut self, text: String) -> Result<(), std::io::Error> {
        let text = text.replace("\r\n", "\n");
        let text: Vec<u8> = text
            .chars()
            .map(|character| u8::try_from(u32::from(character)).unwrap_or(b'?'))
            .collect();
        let mut msg = BytesMut::new();
        msg.put_u8(SERVER_MSG_SERVER_CUT_TEXT);
        msg.put_bytes(0, 3);
        msg.put_u32(text.len() as u32);
        msg.put_slice(&text);

        // Acquire send mutex to prevent interleaved writes
        let _lock = self.send_mutex.lock().await;
        self.write_stream.lock().await.write_all(&msg).await?;
        Ok(())
    }

    pub fn is_shared(&self) -> bool {
        self.shared
    }

    pub(crate) fn command_sender(&self) -> mpsc::Sender<ClientCommand> {
        self.command_tx.clone()
    }

    /// Returns the unique client ID assigned by the server.
    pub fn get_client_id(&self) -> usize {
        self.client_id
    }

    /// Returns a clone of the Arc containing the write half of the TCP stream.
    ///
    /// This allows external code to close the write half directly for shutdown,
    /// which will cause reads on the read half to fail naturally.
    pub fn get_write_stream_handle(&self) -> Arc<tokio::sync::Mutex<ClientWriteStream>> {
        self.write_stream.clone()
    }

    /// Returns the remote host address of the connected client.
    pub fn get_remote_host(&self) -> &str {
        &self.remote_host
    }

    /// Returns the destination port for repeater connections.
    /// Returns -1 for direct connections (not using a repeater).
    pub fn get_destination_port(&self) -> i32 {
        self.destination_port.map_or(-1, i32::from)
    }

    /// Returns the repeater ID if this client is connected via a repeater.
    /// Returns None for direct connections.
    pub fn get_repeater_id(&self) -> Option<&str> {
        self.repeater_id.as_deref()
    }

    /// Sets the connection metadata for reverse connections.
    pub fn set_connection_metadata(&mut self, destination_port: Option<u16>) {
        self.destination_port = destination_port;
    }

    /// Sets the repeater metadata for repeater connections.
    pub fn set_repeater_metadata(&mut self, repeater_id: String, destination_port: Option<u16>) {
        self.repeater_id = Some(repeater_id);
        self.destination_port = destination_port;
    }

    /// Sets the request ID for tracking connection requests.
    pub fn set_request_id(&mut self, request_id: String) {
        self.request_id = Some(request_id);
    }

    /// Returns the request ID if set, or None.
    pub fn get_request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

/// Ensures proper cleanup when `VncClient` is dropped.
///
/// When `VncClient` is dropped, the read half of the TCP stream (`read_stream: OwnedReadHalf`)
/// is automatically closed because it's an owned field. This completes the client disconnect
/// sequence after the write half has been closed separately during shutdown.
///
/// The log message helps diagnose the shutdown sequence by confirming when `VncClient`
/// objects are actually being dropped and their TCP read streams are closing.
impl Drop for VncClient {
    fn drop(&mut self) {
        #[cfg(feature = "debug-logging")]
        log::info!(
            "VncClient {} is being dropped (read half will close now)",
            self.client_id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_wire_format_encodes_red_as_bgrx() {
        let wire_format = default_wire_pixel_format();
        let encoded = translate::translate_pixels(
            &[0xff, 0x00, 0x00, 0xff],
            &PixelFormat::rgba32(),
            &wire_format,
        );

        assert_eq!(&encoded[..], &[0x00, 0x00, 0xff, 0x00]);
        assert_eq!(wire_format.red_shift, 16);
        assert_eq!(wire_format.green_shift, 8);
        assert_eq!(wire_format.blue_shift, 0);
    }

    #[tokio::test]
    async fn failed_authentication_is_throttled() {
        let started = Instant::now();
        throttle_failed_authentication(false).await;
        assert!(started.elapsed() >= AUTH_FAILURE_DELAY);

        let started = Instant::now();
        throttle_failed_authentication(true).await;
        assert!(started.elapsed() < AUTH_FAILURE_DELAY);
    }

    async fn connect_with_version(
        version: &[u8; 12],
    ) -> (VncClient, TcpStream, mpsc::UnboundedReceiver<ClientEvent>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let version = *version;
        let peer = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.unwrap();
            let mut server_version = [0; 12];
            stream.read_exact(&mut server_version).await.unwrap();
            stream.write_all(&version).await.unwrap();
            if &version == PROTOCOL_VERSION_3_3 {
                let mut security = [0; 4];
                stream.read_exact(&mut security).await.unwrap();
                assert_eq!(u32::from_be_bytes(security), u32::from(SECURITY_TYPE_NONE));
            } else {
                let mut offered = [0; 2];
                stream.read_exact(&mut offered).await.unwrap();
                assert_eq!(offered, [1, SECURITY_TYPE_NONE]);
                stream.write_all(&[SECURITY_TYPE_NONE]).await.unwrap();
                if &version == PROTOCOL_VERSION_3_8 {
                    let mut result = [0; 4];
                    stream.read_exact(&mut result).await.unwrap();
                    assert_eq!(u32::from_be_bytes(result), SECURITY_RESULT_OK);
                }
            }
            stream.write_all(&[1]).await.unwrap();
            let mut dimensions = [0; 4];
            stream.read_exact(&mut dimensions).await.unwrap();
            assert_eq!(dimensions, [0, 1, 0, 1]);
            let mut pixel_format = [0; 16];
            stream.read_exact(&mut pixel_format).await.unwrap();
            let mut name_length = [0; 4];
            stream.read_exact(&mut name_length).await.unwrap();
            let mut name = vec![0; u32::from_be_bytes(name_length) as usize];
            stream.read_exact(&mut name).await.unwrap();
            stream
        });
        let (stream, _) = listener.accept().await.unwrap();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let client = VncClient::new(
            1,
            stream,
            Framebuffer::new(1, 1),
            "test".into(),
            None,
            event_tx,
        )
        .await
        .unwrap();
        (client, peer.await.unwrap(), event_rx)
    }

    #[tokio::test]
    async fn negotiates_rfb_3_3_3_7_and_3_8() {
        for version in [
            PROTOCOL_VERSION_3_3,
            PROTOCOL_VERSION_3_7,
            PROTOCOL_VERSION_3_8,
        ] {
            let (_client, _peer, _events) = connect_with_version(version).await;
        }
    }

    fn set_encoding(encoding: i32) -> [u8; 8] {
        let mut message = [0u8; 8];
        message[0] = CLIENT_MSG_SET_ENCODINGS;
        message[2..4].copy_from_slice(&1u16.to_be_bytes());
        message[4..].copy_from_slice(&encoding.to_be_bytes());
        message
    }

    #[tokio::test]
    async fn negotiates_fence_and_extended_key_events() {
        let (mut client, mut peer, mut events) = connect_with_version(PROTOCOL_VERSION_3_8).await;
        let task = tokio::spawn(async move { client.handle_messages().await });

        peer.write_all(&set_encoding(ENCODING_FENCE)).await.unwrap();
        let mut fence = [0u8; 9];
        peer.read_exact(&mut fence).await.unwrap();
        assert_eq!(fence[0], SERVER_MSG_FENCE);

        let payload = b"sync";
        let mut request = BytesMut::new();
        request.put_u8(CLIENT_MSG_FENCE);
        request.put_bytes(0, 3);
        request.put_u32(0x8000_0007);
        request.put_u8(4);
        request.put_slice(payload);
        peer.write_all(&request).await.unwrap();
        let mut response = [0u8; 13];
        peer.read_exact(&mut response).await.unwrap();
        assert_eq!(&response[4..8], &7u32.to_be_bytes());
        assert_eq!(&response[9..], payload);

        peer.write_all(&set_encoding(ENCODING_QEMU_EXTENDED_KEY_EVENT))
            .await
            .unwrap();
        let mut qemu_ack = [0u8; 16];
        peer.read_exact(&mut qemu_ack).await.unwrap();
        assert_eq!(
            i32::from_be_bytes(qemu_ack[12..16].try_into().unwrap()),
            -258
        );
        let mut key = BytesMut::new();
        key.put_u8(CLIENT_MSG_QEMU);
        key.put_u8(0);
        key.put_u16(1);
        key.put_u32(0x61);
        key.put_u32(0x1e);
        peer.write_all(&key).await.unwrap();
        match events.recv().await.unwrap() {
            ClientEvent::ExtendedKeyPress {
                down,
                keysym,
                keycode,
            } => assert!(down && keysym == 0x61 && keycode == 0x1e),
            _ => panic!("expected extended key event"),
        }
        task.abort();
    }

    #[tokio::test]
    async fn cursor_capable_viewers_receive_a_hidden_local_cursor() {
        let (mut client, mut peer, _events) = connect_with_version(PROTOCOL_VERSION_3_8).await;
        let task = tokio::spawn(async move { client.handle_messages().await });

        peer.write_all(&set_encoding(ENCODING_CURSOR))
            .await
            .unwrap();
        let mut update = [0u8; 21];
        peer.read_exact(&mut update).await.unwrap();
        assert_eq!(&update[8..12], &[0, 1, 0, 1]);
        assert_eq!(
            i32::from_be_bytes(update[12..16].try_into().unwrap()),
            ENCODING_CURSOR
        );
        assert!(update[16..].iter().all(|byte| *byte == 0));
        task.abort();
    }

    #[tokio::test]
    async fn reports_extended_desktop_size_and_decodes_latin1_clipboard() {
        let (mut client, mut peer, mut events) = connect_with_version(PROTOCOL_VERSION_3_8).await;
        let commands = client.command_sender();
        let task = tokio::spawn(async move { client.handle_messages().await });
        peer.write_all(&set_encoding(ENCODING_EXTENDED_DESKTOP_SIZE))
            .await
            .unwrap();
        // A second capability with a server acknowledgment makes the first SetEncodings
        // observably complete before the asynchronous desktop-size command is queued.
        peer.write_all(&set_encoding(ENCODING_QEMU_EXTENDED_KEY_EVENT))
            .await
            .unwrap();
        let mut qemu_ack = [0u8; 16];
        peer.read_exact(&mut qemu_ack).await.unwrap();
        let (sent, received) = oneshot::channel();
        commands
            .send(ClientCommand::DesktopSize {
                width: 1,
                height: 1,
                sent,
            })
            .await
            .unwrap();
        received.await.unwrap();
        let mut desktop_size = [0u8; 36];
        peer.read_exact(&mut desktop_size).await.unwrap();
        assert_eq!(
            i32::from_be_bytes(desktop_size[12..16].try_into().unwrap()),
            ENCODING_EXTENDED_DESKTOP_SIZE
        );
        assert_eq!(&desktop_size[8..12], &[0, 1, 0, 1]);

        peer.write_all(&[6, 0, 0, 0, 0, 0, 0, 1, 0xe9])
            .await
            .unwrap();
        match events.recv().await.unwrap() {
            ClientEvent::CutText { text } => assert_eq!(text, "é"),
            _ => panic!("expected clipboard event"),
        }
        task.abort();
    }

    #[tokio::test]
    async fn continuous_updates_use_last_rect_and_confirm_disable() {
        let (mut client, mut peer, _events) = connect_with_version(PROTOCOL_VERSION_3_8).await;
        let task = tokio::spawn(async move { client.handle_messages().await });
        let mut encodings = BytesMut::new();
        encodings.put_u8(CLIENT_MSG_SET_ENCODINGS);
        encodings.put_u8(0);
        encodings.put_u16(2);
        encodings.put_i32(ENCODING_CONTINUOUS_UPDATES);
        encodings.put_i32(ENCODING_LAST_RECT);
        peer.write_all(&encodings).await.unwrap();
        let mut confirmation = [0u8; 1];
        peer.read_exact(&mut confirmation).await.unwrap();
        assert_eq!(confirmation, [SERVER_MSG_END_OF_CONTINUOUS_UPDATES]);

        peer.write_all(&[150, 1, 0, 0, 0, 0, 0, 1, 0, 1])
            .await
            .unwrap();
        peer.write_all(&[3, 0, 0, 0, 0, 0, 0, 1, 0, 1])
            .await
            .unwrap();
        let mut update = [0u8; 32];
        peer.read_exact(&mut update).await.unwrap();
        assert_eq!(&update[..4], &[0, 0, 0xff, 0xff]);
        assert_eq!(
            i32::from_be_bytes(update[28..32].try_into().unwrap()),
            ENCODING_LAST_RECT
        );

        peer.write_all(&[150, 0, 0, 0, 0, 0, 0, 1, 0, 1])
            .await
            .unwrap();
        peer.read_exact(&mut confirmation).await.unwrap();
        assert_eq!(confirmation, [SERVER_MSG_END_OF_CONTINUOUS_UPDATES]);
        task.abort();
    }

    #[tokio::test]
    async fn exchanges_extended_utf8_clipboard_text() {
        let (mut client, mut peer, mut events) = connect_with_version(PROTOCOL_VERSION_3_8).await;
        let commands = client.command_sender();
        let task = tokio::spawn(async move { client.handle_messages().await });
        peer.write_all(&set_encoding(ENCODING_EXTENDED_CLIPBOARD))
            .await
            .unwrap();
        let mut caps = [0u8; 16];
        peer.read_exact(&mut caps).await.unwrap();
        assert_eq!(i32::from_be_bytes(caps[4..8].try_into().unwrap()), -8);

        let text = "héllo\r\nworld\0";
        let mut clipboard = BytesMut::new();
        clipboard.put_u32(u32::try_from(text.len()).unwrap());
        clipboard.put_slice(text.as_bytes());
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        Write::write_all(&mut encoder, &clipboard).unwrap();
        let compressed = encoder.finish().unwrap();
        let payload_length = 4 + compressed.len();
        let mut provide = BytesMut::new();
        provide.put_u8(CLIENT_MSG_CLIENT_CUT_TEXT);
        provide.put_bytes(0, 3);
        provide.put_i32(-i32::try_from(payload_length).unwrap());
        provide.put_u32((1 << 28) | 1);
        provide.put_slice(&compressed);
        peer.write_all(&provide).await.unwrap();
        match events.recv().await.unwrap() {
            ClientEvent::CutText { text } => assert_eq!(text, "héllo\nworld"),
            _ => panic!("expected extended clipboard event"),
        }

        commands
            .send(ClientCommand::CutText("remote ✓".into()))
            .await
            .unwrap();
        let mut notify = [0u8; 12];
        peer.read_exact(&mut notify).await.unwrap();
        assert_eq!(
            u32::from_be_bytes(notify[8..12].try_into().unwrap()),
            (1 << 27) | 1
        );
        task.abort();
    }

    #[tokio::test]
    async fn authenticates_x509_plain_inside_tls() {
        let certificate = rcgen::generate_simple_self_signed(vec!["vinny.local".into()]).unwrap();
        let certificate_der = certificate.cert.der().clone();
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
        let tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate_der.clone()],
                rustls::pki_types::PrivateKeyDer::Pkcs8(key),
            )
            .unwrap();
        let security = SecurityConfig::VeNCrypt {
            tls: Arc::new(tls),
            password: "secret".into(),
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.unwrap();
            let mut version = [0; 12];
            stream.read_exact(&mut version).await.unwrap();
            stream.write_all(PROTOCOL_VERSION_3_8).await.unwrap();
            let mut offered = [0; 2];
            stream.read_exact(&mut offered).await.unwrap();
            assert_eq!(offered, [1, SECURITY_TYPE_VENCRYPT]);
            stream.write_all(&[SECURITY_TYPE_VENCRYPT]).await.unwrap();
            let mut vencrypt_version = [0; 2];
            stream.read_exact(&mut vencrypt_version).await.unwrap();
            assert_eq!(vencrypt_version, [0, 2]);
            stream.write_all(&[0, 2]).await.unwrap();
            let mut ack_and_count = [0; 2];
            stream.read_exact(&mut ack_and_count).await.unwrap();
            assert_eq!(ack_and_count, [0, 1]);
            let mut subtype = [0; 4];
            stream.read_exact(&mut subtype).await.unwrap();
            assert_eq!(u32::from_be_bytes(subtype), 262);
            stream.write_all(&subtype).await.unwrap();
            let mut tls_ack = [0; 1];
            stream.read_exact(&mut tls_ack).await.unwrap();
            assert_eq!(tls_ack, [1]);

            let mut roots = rustls::RootCertStore::empty();
            roots.add(certificate_der).unwrap();
            let client_config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
            let server_name = rustls::pki_types::ServerName::try_from("vinny.local").unwrap();
            let mut stream = connector.connect(server_name, stream).await.unwrap();
            stream.write_all(&0u32.to_be_bytes()).await.unwrap();
            stream.write_all(&6u32.to_be_bytes()).await.unwrap();
            stream.write_all(b"secret").await.unwrap();
            let mut security_result = [0; 4];
            stream.read_exact(&mut security_result).await.unwrap();
            assert_eq!(security_result, [0; 4]);
            stream.write_all(&[1]).await.unwrap();
            let mut dimensions = [0; 4];
            stream.read_exact(&mut dimensions).await.unwrap();
            assert_eq!(dimensions, [0, 1, 0, 1]);
        });
        let (stream, _) = listener.accept().await.unwrap();
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        let result = VncClient::new_with_security(
            1,
            stream,
            Framebuffer::new(1, 1),
            "test".into(),
            security,
            event_tx,
        )
        .await;
        peer.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn rejects_a_security_type_that_was_not_offered() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let client = tokio::spawn(async move {
            let mut stream = TcpStream::connect(address).await.unwrap();
            let mut version = [0; 12];
            stream.read_exact(&mut version).await.unwrap();
            stream.write_all(&version).await.unwrap();
            let mut offered = [0; 2];
            stream.read_exact(&mut offered).await.unwrap();
            assert_eq!(offered, [1, SECURITY_TYPE_NONE]);
            stream
                .write_all(&[2]) // Legacy VNCAuth was not offered.
                .await
                .unwrap();
        });
        let (stream, _) = listener.accept().await.unwrap();
        let framebuffer = Framebuffer::new(1, 1);
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        let error = VncClient::new(1, stream, framebuffer, "test".into(), None, event_tx)
            .await
            .err()
            .expect("unoffered security type should fail");
        client.await.unwrap();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
