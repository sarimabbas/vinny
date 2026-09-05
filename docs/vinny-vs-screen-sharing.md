# About macOS Screen Sharing

macOS includes [Screen Sharing](https://support.apple.com/guide/mac-help/mh11848/mac). You do not need Vinny for ordinary Mac-to-Mac screen sharing.

Vinny provides per-listener settings for the display, capture width, frame rate, port, and remote input. It shares the logged-in desktop; it does not create separate user sessions or virtual displays.

Apple's Screen Sharing viewer can connect to Vinny using **Legacy authentication (unencrypted)**. This does not encrypt screen contents or input. Use a secure tunnel. The [connection guide](first-connection.md) uses TigerVNC over SSH.

If built-in Screen Sharing already uses port `5900`, choose another port in Vinny and update the tunnel destination to match.
