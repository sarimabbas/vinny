mod capture;
mod input;
mod permissions;

use rustvncserver::VncServer;
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
}

struct ServerHandle {
    stop: Option<oneshot::Sender<()>>,
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
/// `listen` must point to a valid, NUL-terminated UTF-8 string for the duration
/// of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vinny_start_server(
    id: u64,
    listen: *const c_char,
    port: u16,
    display: usize,
    max_width: u32,
    fps: u32,
) -> bool {
    if listen.is_null() || !permissions::check().granted() {
        return false;
    }
    let Ok(listen) = unsafe { CStr::from_ptr(listen) }.to_str() else {
        return false;
    };
    let Ok(config) = server_config(listen, port, display, max_width, fps) else {
        return false;
    };

    stop_server(id);
    let (stop_tx, stop_rx) = oneshot::channel();
    let status = Arc::new(AtomicU8::new(1));
    let thread_status = Arc::clone(&status);
    let thread = std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("could not start runtime: {error}"))
            .and_then(|runtime| runtime.block_on(serve(config, stop_rx)));
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

fn server_config(
    listen: &str,
    port: u16,
    display: usize,
    max_width: u32,
    fps: u32,
) -> Result<ServerConfig, String> {
    let listen = listen
        .parse::<IpAddr>()
        .map_err(|_| "listen address is invalid")?;
    if port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    if !(1..=60).contains(&fps) {
        return Err("FPS must be between 1 and 60".into());
    }
    if !(320..=7680).contains(&max_width) {
        return Err("maximum width must be between 320 and 7680".into());
    }
    Ok(ServerConfig {
        listen,
        port,
        display,
        max_width,
        fps,
    })
}

fn main() {
    unsafe { vinny_run_gui() };
}

async fn serve(config: ServerConfig, stop: oneshot::Receiver<()>) -> Result<(), String> {
    if !permissions::check().granted() {
        return Err("Screen Recording and Accessibility permissions are required".into());
    }

    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(1);
    let mut capture = capture::start(config.display, config.max_width, config.fps, frame_tx)
        .map_err(|error| format!("could not start capture: {error}"))?;
    let geometry = capture.geometry;
    let (server, events) = VncServer::new(
        geometry.capture_width,
        geometry.capture_height,
        format!("Vinny Display {}", config.display + 1),
        None,
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
    let input_task = tokio::spawn(input::handle_events(events, geometry));

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
    let result = tokio::select! {
        result = &mut listen => result.map_err(|error| format!("VNC listener failed: {error}")),
        _ = stop => Ok(()),
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

    #[test]
    fn validates_server_configuration() {
        assert!(server_config("127.0.0.1", 5900, 0, 1920, 20).is_ok());
        assert!(server_config("100.100.100.100", 5901, 1, 2560, 30).is_ok());
        assert!(server_config("0.0.0.0", 5900, 0, 1920, 20).is_ok());
        assert!(server_config("192.168.1.10", 5900, 0, 1920, 20).is_ok());
        assert!(server_config("not-an-address", 5900, 0, 1920, 20).is_err());
        assert!(server_config("127.0.0.1", 0, 0, 1920, 20).is_err());
        assert!(server_config("127.0.0.1", 5900, 0, 1920, 0).is_err());
    }
}
