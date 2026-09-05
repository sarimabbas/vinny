# Connection guide

Vinny runs on the Mac you want to share. On the other device, use a VNC client to view and control it.

## Choose a client

[TigerVNC](https://tigervnc.org/) is one option. [Remmina](https://remmina.org/remmina-vnc/) provides a VNC client on Linux, and macOS includes [Screen Sharing](https://support.apple.com/guide/mac-help/mh11848/mac). Other VNC clients can work when they support the authentication mode you choose below.

The address format varies by client. Some have separate host and port fields; TigerVNC uses two colons before an explicit port.

## Prepare Vinny

[Install Vinny](../README.md#install), open it, and grant Screen Recording and Accessibility. Choose a display, keep **Listen on** at `127.0.0.1:5900`, enable the server, and choose **Apply & restart**. Confirm its status is listening.

For access from another device, choose a route:

- **Tailscale:** connect your devices to a private network and forward a tailnet port to Vinny with Serve. Follow the [Tailscale guide](tailscale.md). No SSH session is needed.
- **SSH:** forward a local port from your viewing computer to the Mac. The steps are below.
- **Direct connection:** bind Vinny to an IP address the viewer can reach and allow that port through the Mac's firewall. Use a client that supports Vinny's VeNCrypt/X509Plain mode with **Encrypted + password**. See the [settings reference](share-selected-display.md) for authentication and certificate details.

## Choose authentication

For SSH or Tailscale, a client that accepts VNC security type **None** can use Vinny with **Encrypted + password** off. The tunnel protects traffic between devices. Local processes can still reach the unauthenticated listener; with Tailscale Serve, anyone permitted to reach the served port can connect.

For Apple's Screen Sharing or another client that requires a VNC password, turn on **Encrypted + password**, then **Legacy authentication (unencrypted)**, and set a password of 1–8 bytes. Use this mode through SSH or Tailscale: legacy VNC does not encrypt the desktop or input. Choose **Apply & restart** after changing authentication.

## Connect over SSH

On the shared Mac, enable **Remote Login** in System Settings → General → Sharing and allow your account. Use the account and host shown there. [Apple's Remote Login instructions](https://support.apple.com/guide/mac-help/mchlp1066/mac) cover older macOS versions too.

On the viewing computer, replace `YOUR_USER` and `YOUR_MAC` and run:

```bash
ssh -N -o ExitOnForwardFailure=yes -L 127.0.0.1:15900:127.0.0.1:5900 YOUR_USER@YOUR_MAC
```

Check the SSH host identity on the first connection and sign in with your Mac account's SSH credentials. Leave the terminal open. The Mac must already be reachable from this computer.

In your VNC client, connect to host `127.0.0.1`, port `15900`:

- **TigerVNC:** enter `127.0.0.1::15900`. Its [viewer manual](https://tigervnc.org/doc/vncviewer.html) describes the address syntax and security options.
- **macOS Screen Sharing:** in Finder, choose Go → Connect to Server and enter `vnc://127.0.0.1:15900`. Use the legacy password configured in Vinny, not your Mac login password.
- **Other clients:** select VNC, then enter the host and port in the client's connection fields. Match its security mode to Vinny's settings.

You should see the selected display. Check pointer movement and typing. For view-only access, turn off **Allow keyboard and mouse**, apply, and reconnect.

To finish, close the viewer and press Control-C in the SSH terminal. Quit Vinny or disable its server to stop sharing locally too.

## Troubleshooting

- **Server will not start:** check both macOS permissions and the error on the server card.
- **Port already in use:** if another app uses `5900` on the Mac, change Vinny's port and the final `:5900` in the SSH command. If the viewing computer's `15900` is occupied, change it in both the command and viewer.
- **Connection refused or timed out:** check that the Mac is awake, Vinny is listening, and your SSH tunnel or Tailscale Serve is running. The default loopback listener cannot be reached directly using the Mac's network IP.
- **No matching security types:** use None for an unauthenticated tunnel connection, legacy VNC password authentication for Screen Sharing, or VeNCrypt/X509Plain for Vinny's TLS mode.
- **Picture works but input does not:** check Accessibility and **Allow keyboard and mouse**.

Changes to a running server restart the listener. Reconnect after applying them.
