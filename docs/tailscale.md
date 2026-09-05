# Tailscale guide

Tailscale connects your devices over an encrypted private network. You can connect directly to Vinny over Tailscale, or use [Serve](https://tailscale.com/docs/features/tailscale-serve) to forward a tailnet port to Vinny on localhost.

## Connect your devices

Install [Tailscale](https://tailscale.com/download) on the shared Mac and the viewing device, and connect both to your tailnet. Check that your tailnet access rules allow the viewing device to reach the Mac on TCP port `5900`.

Open Vinny, grant its permissions, and choose a display. Set the listener address using one of the options below, then enable the server. Choose a VNC authentication mode from the [connection guide](first-connection.md). For Apple's Screen Sharing, enable the legacy password option.

## Option 1: Connect directly

In Vinny, set **Listen on** to the Mac's Tailscale IP and keep port `5900`. Choose **Apply & restart**. Allow inbound connections to Vinny in the Mac's firewall if needed.

Connect your VNC client to the Mac over Tailscale on port `5900`, using the authentication settings above. If Vinny cannot bind the Tailscale IP, use Serve below.

## Option 2: Keep Vinny on localhost with Serve

Set Vinny back to `127.0.0.1:5900` and choose **Apply & restart** before configuring Serve. Use this option when you want Vinny to keep its local-only listener.

Run these commands on the Mac running Vinny. If `tailscale` is not found, use the [macOS CLI setup](https://tailscale.com/docs/reference/tailscale-cli?tab=macos). For the app installed at `/Applications/Tailscale.app`, you can use this alias in your terminal:

```bash
alias tailscale="/Applications/Tailscale.app/Contents/MacOS/Tailscale"
```

Check existing Serve mappings, then add the VNC forwarder if port 5900 is free:

```bash
tailscale serve status
tailscale serve --bg --tcp=5900 tcp://127.0.0.1:5900
tailscale serve status
```

This is a raw TCP forwarder, as described in the [Serve command reference](https://tailscale.com/docs/reference/tailscale-cli/serve). Tailscale encrypts transport between devices. `--bg` keeps the mapping running after you close the terminal and across Tailscale restarts. Vinny must still be running.

If Serve prints a setup URL, follow it to enable the required tailnet features. Tailscale's [Serve setup documentation](https://tailscale.com/docs/features/tailscale-serve) lists HTTPS certificates as a prerequisite. The VNC client still uses TCP, not an HTTPS address.

Use Serve here, not Funnel, which publishes services to the internet. Tailnet access rules apply to Serve. With Vinny authentication off, anyone those rules allow to reach this port can view the desktop.

## Connect your viewer

Both options use the Mac's Tailscale IP or MagicDNS name, with port `5900`. For Serve, you can also find the address in its output. Connect from the other device, not from the Mac serving Vinny.

- **TigerVNC:** `YOUR_MAC::5900`
- **macOS Screen Sharing:** in Finder → Go → Connect to Server, enter `vnc://YOUR_MAC:5900` and use Vinny's legacy VNC password.
- **Other VNC clients:** host `YOUR_MAC`, port `5900`, with authentication matching Vinny.

Replace `YOUR_MAC` with the actual Tailscale address. If another Serve mapping already uses port 5900, use `--tcp=15900` instead and connect your viewer to port 15900. The destination can remain `127.0.0.1:5900`.

## Stop sharing

For a direct connection, disable the server in Vinny. If you used Serve, remove its mapping with:

```bash
tailscale serve --bg --tcp=5900 off
```

Use the external port you chose if it differs. This leaves other Serve mappings intact. Disable the Vinny server as well if you want to stop local access.
