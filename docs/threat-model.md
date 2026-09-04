# Threat model

## Scope

Vinny captures a Mac display and serves it over VNC. ScreenCaptureKit provides screen access, and macOS Accessibility APIs provide keyboard and pointer control. macOS grants those permissions to Vinny's signed bundle identity.

A new server listens without authentication on `127.0.0.1`. Users can choose another IPv4 or IPv6 address, enable VeNCrypt/X509Plain with a password, or disable remote control. Passwords are stored in the macOS Keychain.

## VNC clients

A connected client can receive screen and clipboard contents. Unless remote control is disabled, it can also send clipboard text, keyboard events, and pointer events.

Vinny applies these limits:

- a loopback default and a warning for plaintext non-loopback listeners
- optional TLS and password authentication
- view-only and sharing controls
- strict security negotiation and size limits for protocol fields
- a 10-second handshake timeout
- a one-second delay after a wrong password
- a limit of eight clients per server

## TLS certificates

Vinny creates a self-signed certificate when a server starts. The certificate changes when the server is recreated, so its fingerprint is not a stable server identity. Check certificate prompts or use a trusted tunnel on hostile networks.

## Releases

Pull-request jobs cannot access release credentials. A maintainer starts a release from protected `main` and approves the `release` environment after the unprivileged build passes. Releases use immutable tags and assets. Homebrew updates go through a separate pull request.

## Remaining risks

| Risk | Exposure | Mitigation |
|---|---|---|
| Unauthenticated viewing or input | An untrusted device can reach a plaintext listener | Keep it on loopback or a trusted network. Otherwise use encrypted mode or a secure tunnel. |
| Server impersonation | A viewer accepts a changed self-signed certificate on a hostile network | Check the certificate prompt or use a trusted tunnel. |
| Password guessing or resource exhaustion | An encrypted listener is reachable from an untrusted network | Use a strong password. Vinny delays failures and limits clients. |
| Release compromise | The maintainer account or approval session is compromised | Keep branch protection, account security, manual approval, and cask review enabled. |

## Relevant code

- `vendor/rustvncserver/src/client.rs`: RFB parsing, VeNCrypt, and TLS authentication
- `vendor/rustvncserver/src/server.rs`: client limits, sharing policy, and client cleanup
- `src/main.rs`: server configuration and event routing
- `src/VinnyUI.swift`: listener settings, Keychain access, and security controls
- `.github/workflows/release.yml`: signing, notarization, publishing, and Homebrew updates

Report vulnerabilities through the process in [SECURITY.md](SECURITY.md).
