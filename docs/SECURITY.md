# Security Policy

## Supported versions

Security fixes are provided for the latest release only. See [threat-model.md](threat-model.md) for the trust boundaries and known risks.

## Reporting a vulnerability

Report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/sarimabbas/vinny/security/advisories/new). Do not open a public issue before a fix is available.

Include the affected version, reproduction steps, impact, and suggested fix if you have one. Expect an initial response within seven days.

## Known security boundary

The default listener is unauthenticated and bound to `127.0.0.1`. Encrypted servers use VeNCrypt/X509Plain, a Keychain-stored password, and a self-signed certificate. The certificate changes when the server is recreated, so the viewer may ask for approval again.

The optional legacy VNC authentication mode supports macOS Screen Sharing. It limits the server password to eight bytes and does not encrypt screen contents or input.

Anyone who can reach an unsecured or legacy-compatible listener can see the screen after satisfying that listener's authentication requirements and, unless view-only mode is on, control the Mac. Use encrypted mode, a trusted network, or a secure tunnel such as Tailscale. Do not expose Vinny directly to the public internet.
