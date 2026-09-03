# Security Policy

## Supported versions

Vinny is an early-stage project. Security fixes are provided for the latest published version only.

The repository's security boundaries and prioritized risks are documented in [threat-model.md](threat-model.md).

## Reporting a vulnerability

Please report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/sarimabbas/vinny/security/advisories/new). Do not open a public issue for an undisclosed vulnerability.

Include the affected version, reproduction steps, impact, and any suggested mitigation. You should receive an initial response within seven days.

## Known security boundary

Vinny currently provides neither VNC authentication nor transport encryption. It therefore listens on loopback by default. Binding it to another interface is a user-controlled risk decision: anyone who can reach that listener can view the screen and control keyboard and pointer input, while network observers can read the traffic. Use a trusted network or a secured tunnel such as Tailscale; direct public-internet exposure is unsupported.
