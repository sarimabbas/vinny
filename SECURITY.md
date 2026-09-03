# Security Policy

## Supported versions

Vinny is an early-stage project. Security fixes are provided for the latest published version only.

## Reporting a vulnerability

Please report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/sarimabbas/vinny/security/advisories/new). Do not open a public issue for an undisclosed vulnerability.

Include the affected version, reproduction steps, impact, and any suggested mitigation. You should receive an initial response within seven days.

## Known security boundary

Vinny currently provides neither VNC authentication nor transport encryption. It therefore listens on loopback by default. Binding it to another interface is appropriate only on a trusted network or behind a secured tunnel such as Tailscale. Never expose Vinny directly to the public internet.
