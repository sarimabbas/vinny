# Tailscale guide

Tailscale connects your devices over an encrypted private network. [Serve](https://tailscale.com/docs/features/tailscale-serve) can forward a port on that network to Vinny's localhost listener.

## Connect your devices

Install [Tailscale](https://tailscale.com/download) on the shared Mac and the viewing device, and connect both to your tailnet. Check that your tailnet access rules allow the viewing device to reach the Mac on TCP port `5900`.

Open Vinny, grant its permissions, and enable a server on `127.0.0.1:5900`. Choose a VNC authentication mode from the [connection guide](first-connection.md). For Apple's Screen Sharing, enable the legacy password option.

## Forward the port with Serve

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

Use the Mac's Tailscale IP or DNS name shown in Serve's output, with port `5900`. Connect from the other device, not from the Mac serving Vinny.

- **TigerVNC:** `YOUR_MAC::5900`
- **macOS Screen Sharing:** in Finder → Go → Connect to Server, enter `vnc://YOUR_MAC:5900` and use Vinny's legacy VNC password.
- **Other VNC clients:** host `YOUR_MAC`, port `5900`, with authentication matching Vinny.

Replace `YOUR_MAC` with the actual Tailscale address. If another Serve mapping already uses port 5900, use `--tcp=15900` instead and connect your viewer to port 15900. The destination can remain `127.0.0.1:5900`.

## Why this helps with firewalls

Serve receives the tailnet connection and makes a local connection to Vinny. Vinny can stay on `127.0.0.1`, avoiding the need for a direct inbound connection to its LAN listener. This can help when macOS blocks inbound connections to Vinny.

Tailscale itself still needs to connect, and Serve does not bypass tailnet access rules or every host security product. Router port forwarding is usually unnecessary; Tailscale can use relays when a direct connection is unavailable. See [Tailscale's firewall guidance](https://tailscale.com/docs/reference/faq/firewall-ports).

If it fails, check that both devices are online in Tailscale, inspect `tailscale serve status`, and confirm Vinny is listening on the local destination port. An authentication error means you reached a VNC server; check the client and Vinny security settings next.

## Stop sharing

Remove this mapping with:

```bash
tailscale serve --bg --tcp=5900 off
```

Use the external port you chose if it differs. This leaves other Serve mappings intact. Disable the Vinny server as well if you want to stop local access.
