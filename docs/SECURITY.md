# Security Policy

## Supported versions

Vinny is an early-stage project. Security fixes are provided for the latest published version only.

The repository's security boundaries and prioritized risks are documented in [threat-model.md](threat-model.md).

## Reporting a vulnerability

Please report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/sarimabbas/vinny/security/advisories/new). Do not open a public issue for an undisclosed vulnerability.

Include the affected version, reproduction steps, impact, and any suggested mitigation. You should receive an initial response within seven days.

## Known security boundary

Vinny defaults to an unauthenticated, plaintext loopback listener for compatibility. Each server can instead enable password-authenticated VeNCrypt/X509Plain over TLS. The certificate is self-signed and regenerated when the server is recreated, so viewers may require explicit certificate approval after a restart. Passwords are stored in the macOS Keychain rather than `UserDefaults`.

Binding an unsecured server to another interface remains a user-controlled risk decision: anyone who can reach it can view the screen and, unless view-only mode is enabled, control input. Use Vinny's encrypted mode, a trusted network, or a secured tunnel such as Tailscale. Direct public-internet exposure remains unsupported.
