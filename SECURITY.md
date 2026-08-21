# Security Policy

StickyMD has no published stable version yet. Security fixes currently target the latest `main` branch and the exact source commit named by any local release-candidate artifact.

Please report a vulnerability through GitHub's private security-advisory interface when it is available. If private reporting is unavailable, open a minimal issue that contains no exploit, private note content, personal path, secret, or other sensitive material, and ask the maintainer for a private channel.

StickyMD does not collect telemetry and has no runtime network client or updater. Do not attach real `note.md`, clipboard content, usernames, full local paths, crash dumps, or recovery files to a public report. A safe report should include the affected commit/artifact SHA-256, Windows build, reproducible steps using synthetic content, observed impact, and whether data was overwritten, disclosed, or made unavailable.

Unsigned development and local-RC builds can trigger Windows reputation warnings. Such a warning alone is not evidence of malware; always verify the published SHA-256 and, when available, the GitHub artifact attestation.
