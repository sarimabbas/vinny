# Settings reference

Each server has its own display, port, and settings. Changes take effect when you choose **Apply & restart**, which disconnects existing viewers.

## Enabled

Starts or stops the listener. New servers start disabled. Closing Vinny's window leaves enabled servers running; quitting the app stops them.

## Display

Select the connected display to share. Viewers see everything on that display, including notifications. Check the selection again after unplugging or rearranging monitors.

## Maximum width and frame rate

**Maximum width** limits the captured image width, preserving its aspect ratio. It accepts 320–7680 pixels and does not upscale a smaller display. It does not change the Mac's display resolution.

**Frame rate** sets the capture target from 1–60 FPS. Defaults are 1920 pixels and 20 FPS. Lower values reduce the amount of image data captured and sent.

## Viewers

- **Viewer decides:** clients can share the session or request exclusive access.
- **Allow multiple viewers:** new viewers join without disconnecting existing ones.
- **One viewer at a time:** new connections are rejected while a viewer is connected.

Each listener accepts at most eight clients. All listeners share the logged-in Mac session.

## Remote control

Turn off **Allow keyboard and mouse** for view-only access. This blocks remote keyboard, pointer, and incoming clipboard changes. Viewers still receive the screen and outgoing clipboard contents.

## Security and password

With **Encrypted + password** off, the listener has no VNC authentication or encryption. Keep it on loopback and use the [connection guide](first-connection.md) for SSH or Tailscale access.

With **Encrypted + password** on, Vinny uses VeNCrypt/X509Plain. The viewer must support that mode; TigerVNC does. Vinny creates a self-signed certificate when the server starts, so its fingerprint changes when the listener is recreated. Verify certificates through a trusted channel, or use an authenticated tunnel. Passwords are stored in the macOS Keychain.

**Legacy authentication (unencrypted)** also allows clients such as macOS Screen Sharing to use classic VNC password authentication. Enable **Encrypted + password** first to reveal this option. Legacy passwords are limited to 1–8 bytes; ASCII characters each use one byte. Legacy sessions remain unencrypted even though the parent option is enabled, so use SSH or Tailscale.

See the [security model](threat-model.md) for details.

## Listen on and port

**Listen on** takes an IPv4 or IPv6 address. `127.0.0.1` accepts connections only on this Mac. Binding to a network address makes the listener reachable on that interface, subject to firewall rules. `0.0.0.0` listens on all IPv4 interfaces.

Ports range from 1–65535. The default is `5900`. New configurations choose the next unused configured port, but another application may already occupy it.

## Add or remove a server

Use **Add server** to share another display on a separate port. Select its display, enable it, and apply. With SSH or Tailscale Serve, forward that listener's port separately. **Remove** deletes the server configuration and stops its listener.
