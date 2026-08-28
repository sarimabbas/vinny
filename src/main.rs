mod capture;
mod input;
mod permissions;

use permissions::Permissions;
use rustvncserver::VncServer;
use std::future::pending;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::ExitCode;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

const HELP: &str = "macos-vnc-server — VNC for the active macOS desktop

USAGE
  macos-vnc-server serve [OPTIONS]
  macos-vnc-server doctor [--request]
  macos-vnc-server help

SERVE OPTIONS
  --listen <IP>         Loopback or Tailscale IP (default: 127.0.0.1)
  --port <PORT>         VNC port, 1–65535 (default: 5900)
  --display <INDEX>     Display index (default: 0)
  --max-width <PIXELS>  Downscale wide displays (default: 1920)
  --fps <FPS>           Capture rate, 1–60 (default: 20)
  --no-request          Check permissions without prompting
  --parent-stdio        Stop when stdin closes
  -h, --help            Show help
  -v, --version         Show version

Only loopback and Tailscale addresses are accepted. Wildcard and LAN binds are refused.
";

#[derive(Debug)]
struct ServeOptions {
    listen: IpAddr,
    port: u16,
    display: usize,
    max_width: u32,
    fps: u32,
    request_permissions: bool,
    parent_stdio: bool,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            listen: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 5900,
            display: 0,
            max_width: 1920,
            fps: 20,
            request_permissions: true,
            parent_stdio: false,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => serve(parse_serve(args)?).await,
        Some("doctor") => {
            let request = match args.next().as_deref() {
                None => false,
                Some("--request") => true,
                Some(other) => return Err(format!("unknown doctor option {other}")),
            };
            if args.next().is_some() {
                return Err("doctor accepts only --request".into());
            }
            let status = permission_status(request);
            print_permissions(status);
            if status.granted() {
                Ok(())
            } else {
                Err("permissions are incomplete".into())
            }
        }
        Some("help" | "--help" | "-h") | None => {
            print!("{HELP}");
            Ok(())
        }
        Some("version" | "--version" | "-v") => {
            println!("macos-vnc-server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(other) => Err(format!("unknown command {other}\n\n{HELP}")),
    }
}

fn parse_serve(mut args: impl Iterator<Item = String>) -> Result<ServeOptions, String> {
    let mut options = ServeOptions::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => options.listen = value(&mut args, "--listen")?,
            "--port" => options.port = value(&mut args, "--port")?,
            "--display" => options.display = value(&mut args, "--display")?,
            "--max-width" => options.max_width = value(&mut args, "--max-width")?,
            "--fps" => options.fps = value(&mut args, "--fps")?,
            "--no-request" => options.request_permissions = false,
            "--parent-stdio" => options.parent_stdio = true,
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "-v" | "--version" => {
                println!("macos-vnc-server {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => return Err(format!("unknown serve option {arg}")),
        }
    }
    validate_listen_address(options.listen)?;
    if options.port == 0 {
        return Err("--port must be between 1 and 65535".into());
    }
    if !(1..=60).contains(&options.fps) {
        return Err("--fps must be between 1 and 60".into());
    }
    if !(320..=7680).contains(&options.max_width) {
        return Err("--max-width must be between 320 and 7680".into());
    }
    Ok(options)
}

fn validate_listen_address(address: IpAddr) -> Result<(), String> {
    if address.is_loopback() || is_tailscale_address(address) {
        Ok(())
    } else {
        Err("--listen must be loopback or a Tailscale address (100.64.0.0/10 or fd7a:115c:a1e0::/48)".into())
    }
}

fn is_tailscale_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let octets = address.octets();
            octets[0] == 100 && (64..=127).contains(&octets[1])
        }
        IpAddr::V6(address) => address.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

fn value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    args.next()
        .ok_or_else(|| format!("{name} needs a value"))?
        .parse()
        .map_err(|_| format!("invalid value for {name}"))
}

fn permission_status(request: bool) -> Permissions {
    if request {
        permissions::request()
    } else {
        permissions::check()
    }
}

fn print_permissions(status: Permissions) {
    println!(
        "Screen Recording: {}",
        if status.screen_recording {
            "granted"
        } else {
            "missing"
        }
    );
    println!(
        "Accessibility: {}",
        if status.accessibility {
            "granted"
        } else {
            "missing"
        }
    );
    if !status.screen_recording {
        println!(
            "Grant Screen Recording in System Settings → Privacy & Security, then restart this program."
        );
    }
    if !status.accessibility {
        println!("Grant Accessibility in System Settings → Privacy & Security.");
    }
}

async fn serve(options: ServeOptions) -> Result<(), String> {
    let status = permission_status(options.request_permissions);
    if !status.granted() {
        print_permissions(status);
        if options.request_permissions {
            if !status.screen_recording {
                permissions::open_settings("screen");
            } else if !status.accessibility {
                permissions::open_settings("accessibility");
            }
        }
        return Err("grant the missing permissions, then run serve again".into());
    }

    let (frame_tx, mut frame_rx) = mpsc::channel::<Vec<u8>>(1);
    let mut capture = capture::start(options.display, options.max_width, options.fps, frame_tx)
        .map_err(|error| format!("could not start capture: {error}"))?;
    let geometry = capture.geometry;
    let (server, events) = VncServer::new(
        geometry.capture_width,
        geometry.capture_height,
        "macOS Desktop".into(),
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

    let address = SocketAddr::new(options.listen, options.port);
    println!(
        "Serving display {} at {} ({}×{}, {} FPS)",
        options.display, address, geometry.capture_width, geometry.capture_height, options.fps
    );

    let listen = server.listen_on(address);
    tokio::pin!(listen);
    let parent = wait_for_parent(options.parent_stdio);
    tokio::pin!(parent);
    let result = tokio::select! {
        result = &mut listen => result.map_err(|error| format!("VNC listener failed: {error}")),
        result = tokio::signal::ctrl_c() => result.map_err(|error| format!("signal handler failed: {error}")),
        () = &mut parent => Ok(()),
    };

    capture.stop();
    server.disconnect_all_clients().await;
    frame_task.abort();
    input_task.abort();
    result
}

async fn wait_for_parent(enabled: bool) {
    if !enabled {
        pending::<()>().await;
    }
    let mut stdin = tokio::io::stdin();
    let mut buffer = [0_u8; 64];
    loop {
        match stdin.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_loopback_and_tailscale_listeners() {
        for address in [
            "127.0.0.1",
            "::1",
            "100.64.0.1",
            "100.127.255.254",
            "fd7a:115c:a1e0::1",
        ] {
            assert!(validate_listen_address(address.parse().unwrap()).is_ok());
        }
        for address in ["0.0.0.0", "::", "100.128.0.1", "192.168.1.10", "8.8.8.8"] {
            assert!(validate_listen_address(address.parse().unwrap()).is_err());
        }
    }

    #[test]
    fn parses_tailscale_listener() {
        let options = parse_serve(
            ["--listen", "100.100.100.100", "--port", "5902"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(options.listen, "100.100.100.100".parse::<IpAddr>().unwrap());
        assert_eq!(options.port, 5902);
    }

    #[test]
    fn defaults_to_standard_vnc_port_and_rejects_zero() {
        let options = parse_serve(std::iter::empty()).unwrap();
        assert_eq!(options.port, 5900);
        assert!(parse_serve(["--port", "0"].into_iter().map(str::to_owned)).is_err());
    }
}
