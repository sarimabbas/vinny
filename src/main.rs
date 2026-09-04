mod capture;
mod input;
mod permissions;

use rustvncserver::VncServer;
use rustvncserver::server::SharingPolicy;
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::CStr;
use std::net::{IpAddr, SocketAddr};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot};

static SERVERS: OnceLock<Mutex<HashMap<u64, ServerHandle>>> = OnceLock::new();

unsafe extern "C" {
    fn vinny_run_gui();
}

#[derive(Debug, Clone)]
struct ServerConfig {
    listen: IpAddr,
    port: u16,
    display: usize,
    max_width: u32,
    fps: u32,
    sharing_policy: SharingPolicy,
    view_only: bool,
    password: Option<String>,
    legacy_auth: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerRequest {
    address: String,
    port: u16,
    display: usize,
    max_width: u32,
    fps: u32,
    #[serde(default)]
    sharing_policy: SharingPolicyRequest,
    #[serde(default)]
    view_only: bool,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    legacy_auth: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SharingPolicyRequest {
    #[default]
    FollowClient,
    AlwaysShared,
    SingleClient,
}

impl From<SharingPolicyRequest> for SharingPolicy {
    fn from(value: SharingPolicyRequest) -> Self {
        match value {
            SharingPolicyRequest::FollowClient => Self::FollowClient,
            SharingPolicyRequest::AlwaysShared => Self::AlwaysShared,
            SharingPolicyRequest::SingleClient => Self::SingleClient,
        }
    }
}

enum ServerControl {
    Clipboard(String),
}

struct ServerHandle {
    stop: Option<oneshot::Sender<()>>,
    control: mpsc::UnboundedSender<ServerControl>,
    thread: Option<JoinHandle<()>>,
    status: Arc<AtomicU8>,
}

fn servers() -> &'static Mutex<HashMap<u64, ServerHandle>> {
    SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn vinny_permission_bits() -> i32 {
    let status = permissions::check();
    i32::from(status.screen_recording) | (i32::from(status.accessibility) << 1)
}

#[unsafe(no_mangle)]
pub extern "C" fn vinny_request_permission(permission: i32) -> i32 {
    match permission {
        1 => permissions::request_screen_recording(),
        2 => permissions::request_accessibility(),
        _ => {}
    }
    vinny_permission_bits()
}

/// Writes the ScreenCaptureKit display IDs in capture-index order.
///
/// Pass a null buffer or zero capacity to query the number of displays.
///
/// # Safety
/// When non-null, `buffer` must be valid for writes of `capacity` `u32` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vinny_display_ids(buffer: *mut u32, capacity: usize) -> usize {
    let Ok(content) = screencapturekit::prelude::SCShareableContent::get() else {
        return 0;
    };
    let displays = content.displays();
    if !buffer.is_null() {
        for (index, display) in displays.iter().take(capacity).enumerate() {
            unsafe { buffer.add(index).write(display.display_id()) };
        }
    }
    displays.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn vinny_server_status(id: u64) -> i32 {
    servers()
        .lock()
        .expect("server lock")
        .get(&id)
        .map_or(0, |server| i32::from(server.status.load(Ordering::Relaxed)))
}

/// Starts or restarts a configured VNC server.
///
/// # Safety
/// `configuration` must point to a valid, NUL-terminated UTF-8 JSON string for
/// the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vinny_start_server(id: u64, configuration: *const c_char) -> bool {
    if configuration.is_null() || !permissions::check().granted() {
        return false;
    }
    let Ok(configuration) = unsafe { CStr::from_ptr(configuration) }.to_str() else {
        return false;
    };
    let Ok(request) = serde_json::from_str::<ServerRequest>(configuration) else {
        return false;
    };
    let Ok(config) = server_config(request) else {
        return false;
    };

    stop_server(id);
    let (stop_tx, stop_rx) = oneshot::channel();
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let status = Arc::new(AtomicU8::new(1));
    let thread_status = Arc::clone(&status);
    let thread = std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("could not start runtime: {error}"))
            .and_then(|runtime| runtime.block_on(serve(config, stop_rx, control_rx)));
        match result {
            Ok(()) => thread_status.store(0, Ordering::Relaxed),
            Err(error) => {
                eprintln!("error: {error}");
                thread_status.store(2, Ordering::Relaxed);
            }
        }
    });
    servers().lock().expect("server lock").insert(
        id,
        ServerHandle {
            stop: Some(stop_tx),
            control: control_tx,
            thread: Some(thread),
            status,
        },
    );
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn vinny_stop_server(id: u64) {
    stop_server(id);
}

/// Broadcasts a local pasteboard change to connected VNC clients.
///
/// # Safety
/// `bytes` must be valid for reads of `length` bytes for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vinny_broadcast_clipboard(bytes: *const u8, length: usize) {
    if bytes.is_null() && length != 0 {
        return;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, length) }
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    for server in servers().lock().expect("server lock").values() {
        let _ = server
            .control
            .send(ServerControl::Clipboard(text.to_owned()));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vinny_stop_all_servers() {
    let ids = servers()
        .lock()
        .expect("server lock")
        .keys()
        .copied()
        .collect::<Vec<_>>();
    for id in ids {
        stop_server(id);
    }
}

fn stop_server(id: u64) {
    let Some(mut server) = servers().lock().expect("server lock").remove(&id) else {
        return;
    };
    if let Some(stop) = server.stop.take() {
        let _ = stop.send(());
    }
    if let Some(thread) = server.thread.take() {
        let _ = thread.join();
    }
}

fn server_config(request: ServerRequest) -> Result<ServerConfig, String> {
    let listen = request
        .address
        .parse::<IpAddr>()
        .map_err(|_| "listen address is invalid")?;
    if request.port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    if !(1..=60).contains(&request.fps) {
        return Err("FPS must be between 1 and 60".into());
    }
    if !(320..=7680).contains(&request.max_width) {
        return Err("maximum width must be between 320 and 7680".into());
    }
    let password = request.password.filter(|password| !password.is_empty());
    if request.legacy_auth && password.is_none() {
        return Err("legacy VNC authentication requires a password".into());
    }
    if request.legacy_auth && password.as_ref().is_some_and(|password| password.len() > 8) {
        return Err("legacy VNC passwords must contain 1 to 8 bytes".into());
    }
    Ok(ServerConfig {
        listen,
        port: request.port,
        display: request.display,
        max_width: request.max_width,
        fps: request.fps,
        sharing_policy: request.sharing_policy.into(),
        view_only: request.view_only,
        password,
        legacy_auth: request.legacy_auth,
    })
}

fn main() {
    unsafe { vinny_run_gui() };
}

async fn serve(
    config: ServerConfig,
    stop: oneshot::Receiver<()>,
    mut control: mpsc::UnboundedReceiver<ServerControl>,
) -> Result<(), String> {
    if !permissions::check().granted() {
        return Err("Screen Recording and Accessibility permissions are required".into());
    }

    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(1);
    let mut capture = capture::start(
        config.display,
        config.max_width,
        config.fps,
        frame_tx.clone(),
    )
    .map_err(|error| format!("could not start capture: {error}"))?;
    let mut geometry = capture.geometry;
    let (server, events) = VncServer::new_with_policy_and_legacy_auth(
        geometry.capture_width,
        geometry.capture_height,
        format!("Vinny Display {}", config.display + 1),
        config.password,
        config.legacy_auth,
        config.sharing_policy,
        config.view_only,
    );
    let server = Arc::new(server);

    let framebuffer = Arc::clone(&server);
    let frame_task = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            if let Err(error) = framebuffer.framebuffer().update_from_slice(&frame).await {
                eprintln!("framebuffer error: {error}");
            }
        }
    });
    let input_geometry = Arc::new(std::sync::RwLock::new(geometry));
    let input_task = tokio::spawn(input::handle_events(events, Arc::clone(&input_geometry)));

    let address = SocketAddr::new(config.listen, config.port);
    println!(
        "Serving display {} at {} ({}×{}, {} FPS)",
        config.display + 1,
        address,
        geometry.capture_width,
        geometry.capture_height,
        config.fps
    );

    let listen = server.listen_on(address);
    tokio::pin!(listen);
    tokio::pin!(stop);
    let mut display_check = tokio::time::interval(std::time::Duration::from_secs(1));
    let result = loop {
        tokio::select! {
            result = &mut listen => break result.map_err(|error| format!("VNC listener failed: {error}")),
            _ = &mut stop => break Ok(()),
            message = control.recv() => match message {
                Some(ServerControl::Clipboard(text)) => {
                    if let Err(error) = server.send_cut_text_to_all(text).await {
                        eprintln!("clipboard error: {error}");
                    }
                }
                None => break Ok(()),
            },
            _ = display_check.tick() => {
                let Ok(next_geometry) = capture::geometry(config.display, config.max_width) else {
                    continue;
                };
                if next_geometry != geometry {
                    capture.stop();
                    if next_geometry.capture_width != geometry.capture_width
                        || next_geometry.capture_height != geometry.capture_height
                    {
                        server
                            .notify_desktop_size(
                                next_geometry.capture_width,
                                next_geometry.capture_height,
                            )
                            .await
                            .map_err(|error| format!("could not notify framebuffer size: {error}"))?;
                        server
                            .framebuffer()
                            .resize(next_geometry.capture_width, next_geometry.capture_height)
                            .await
                            .map_err(|error| format!("could not resize framebuffer: {error}"))?;
                    }
                    capture = capture::start(
                        config.display,
                        config.max_width,
                        config.fps,
                        frame_tx.clone(),
                    )
                    .map_err(|error| format!("could not restart capture: {error}"))?;
                    geometry = capture.geometry;
                    *input_geometry.write().expect("geometry lock") = geometry;
                }
            }
        }
    };

    capture.stop();
    server.disconnect_all_clients().await;
    frame_task.abort();
    input_task.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(address: &str, port: u16, fps: u32) -> ServerRequest {
        ServerRequest {
            address: address.into(),
            port,
            display: 0,
            max_width: 1920,
            fps,
            sharing_policy: SharingPolicyRequest::FollowClient,
            view_only: false,
            password: None,
            legacy_auth: false,
        }
    }

    #[test]
    fn validates_server_configuration() {
        assert!(server_config(request("127.0.0.1", 5900, 20)).is_ok());
        assert!(server_config(request("100.100.100.100", 5901, 30)).is_ok());
        assert!(server_config(request("0.0.0.0", 5900, 20)).is_ok());
        assert!(server_config(request("192.168.1.10", 5900, 20)).is_ok());
        assert!(server_config(request("not-an-address", 5900, 20)).is_err());
        assert!(server_config(request("127.0.0.1", 0, 20)).is_err());
        assert!(server_config(request("127.0.0.1", 5900, 0)).is_err());

        let mut legacy = request("127.0.0.1", 5900, 20);
        legacy.password = Some("12345678".into());
        legacy.legacy_auth = true;
        assert!(server_config(legacy).is_ok());

        let mut too_long = request("127.0.0.1", 5900, 20);
        too_long.password = Some("123456789".into());
        too_long.legacy_auth = true;
        assert!(server_config(too_long).is_err());
    }
}
