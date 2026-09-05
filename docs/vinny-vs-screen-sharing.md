# Vinny vs built-in macOS Screen Sharing

macOS already includes Screen Sharing. If you need ordinary access to another Mac and its built-in workflow suits you, start there. Apple documents setup under [Screen Sharing settings](https://support.apple.com/guide/mac-help/mh11848/mac).

Vinny is an open-source option for people who want to configure VNC listeners directly in a menu-bar app.

## Choose based on the job

| Your goal | Where to start |
| --- | --- |
| Access another Mac with Apple's built-in tools | macOS Screen Sharing. No additional server app is needed. |
| Select a display and give it a dedicated VNC port | Vinny exposes display and listener settings together. |
| Set a capture-width limit and frame-rate target per listener | Vinny provides both controls in each server card. |
| Configure multiple listeners with different remote-control settings | Vinny lets each listener enable or block remote input. |
| Read and modify the capture, VNC, or input implementation | Vinny is Apache-2.0 licensed, with its Rust and Swift source in this repository. |

This is a guide to Vinny's workflow, not an exhaustive feature comparison across macOS versions. No comparative performance benchmark is claimed.

## What you take on with Vinny

You install an additional app and grant Screen Recording and Accessibility permission. You also choose how the viewer reaches the server securely. Vinny starts with an unauthenticated listener on `127.0.0.1`, so a second computer cannot reach that default listener directly.

The [first connection guide](first-connection.md) uses TigerVNC through an authenticated SSH tunnel. Direct TLS connections use VeNCrypt/X509Plain and self-signed certificates. Optional legacy authentication supports clients such as Apple's Screen Sharing viewer but leaves VNC traffic unencrypted. See the [security model](threat-model.md).

Vinny shares connected displays from the logged-in desktop. It does not provide a hosted relay, automatic internet routing, or separate virtual desktops. The published release archive is for Apple Silicon.

## Try one specific workflow

[Install Vinny](../README.md#install), complete the [first connection](first-connection.md), then [share a selected display](share-selected-display.md). You can judge whether those controls help your setup without replacing an existing working arrangement.

If built-in Screen Sharing already occupies port `5900`, choose another port in Vinny and update the SSH tunnel destination to match.
