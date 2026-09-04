# Vinny threat model

## Security posture

Vinny is a native macOS VNC server. Its intended safe default is an unauthenticated listener on `127.0.0.1`; users may deliberately bind a server to another IPv4 or IPv6 address. Each server can instead use password-authenticated VeNCrypt/X509Plain over TLS, and passwords are stored in the macOS Keychain.

Vinny receives sensitive screen content through ScreenCaptureKit and can inject input through macOS accessibility APIs. macOS permission prompts and the stable signed bundle identity control those capabilities.

## Trust boundaries

### VNC connection

A VNC viewer sends protocol messages, clipboard contents, and—unless remote control is disabled—keyboard and pointer input. Vinny returns captured screen content and clipboard updates.

Existing controls:

- loopback-only default and a warning for unsecured non-loopback listeners;
- optional VeNCrypt/X.509 TLS with password authentication;
- view-only and viewer-sharing policies;
- strict security-type negotiation and bounded protocol fields;
- a 10-second handshake timeout and at most eight clients per server.

### TLS identity

Encrypted servers use a generated self-signed certificate. It encrypts the session, but it is regenerated when the server is recreated. A viewer therefore cannot rely on a stable certificate fingerprint to identify Vinny across restarts.

### Release credentials

Pull-request CI has no signing credentials. Releases are manually started from protected `main`; compilation happens before the protected `release` environment receives Apple and Homebrew credentials. Published assets and tags are immutable, and Homebrew updates require a separate protected pull request.

## Residual risks

| Risk | When it matters | Current guidance |
|---|---|---|
| Unauthenticated screen or input access | A plaintext listener is reachable by an untrusted device | Keep plaintext listeners on loopback or a trusted network; otherwise enable encrypted mode or use a secure tunnel. |
| Server impersonation | A viewer accepts a changed self-signed certificate on a hostile network | Verify certificate prompts or use a trusted tunnel. A persistent certificate can be added if direct hostile-network use becomes important. |
| Password or resource abuse | An encrypted listener is publicly reachable | Direct public-Internet exposure is unsupported. Authentication backoff or additional rate limits can be added if real-world exposure warrants them. |
| Release compromise | The maintainer account and release approval session are compromised | Preserve branch protection, hardware-backed account security, manual release approval, and cask review. |

The first three risks are low under the default loopback deployment and increase when a listener is exposed beyond a trusted network.

## Security-sensitive code

- `vendor/rustvncserver/src/client.rs` — untrusted RFB parsing, VeNCrypt, and TLS authentication
- `vendor/rustvncserver/src/server.rs` — connection limits, sharing policy, and client lifecycle
- `src/main.rs` — server configuration and capture/input event routing
- `src/VinnyUI.swift` — network exposure, Keychain access, and security controls
- `.github/workflows/release.yml` — signing, notarization, publishing, and tap updates

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
