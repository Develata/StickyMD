# Security Policy

## Supported versions

| Version | Status |
| --- | --- |
| `0.1.0` | Supported |
| Current `main` | Security fixes are developed here, but unreleased commits are not release artifacts |

Security fixes target the latest published version when applicable and the current `main` branch.
Support for an older version may end after a replacement release is published.

## Reporting a vulnerability

Please use GitHub's
[private security-advisory interface](https://github.com/Develata/StickyMD/security/advisories/new).

If private reporting is unavailable, open a minimal public Issue that contains no exploit details,
private note content, personal path, secret, or other sensitive material, and ask the maintainer for a
private channel.

A useful report includes:

- The affected StickyMD version or full source commit.
- The Release ZIP or executable SHA-256 when reporting a published artifact.
- The Windows 11 build and relevant display/input-method environment.
- Reproducible steps using synthetic content.
- The observed security impact: overwrite, disclosure, integrity loss, denial of service, or another
  concrete outcome.
- Whether the problem affects persistence, recovery, external-file reconciliation, managed images,
  links, clipboard input, export, or the Windows shell.

Please do not attach real `note.md`, clipboard content, usernames, full personal paths, recovery files,
crash dumps, tokens, or screenshots containing private data.

## Security boundaries

StickyMD does not include telemetry, an updater, a runtime network client, remote-image downloads, a
browser engine, a JavaScript runtime, or a database. Raw HTML is displayed literally rather than
executed. These boundaries reduce attack surface but do not replace responsible vulnerability review.

The portable application writes only inside its Program Directory. It must be placed in a directory
writable by the current user and should not be run as administrator.

## Unsigned releases

Version `0.1.0` is distributed without an Authenticode signature. Windows can therefore display a
SmartScreen or reputation warning. Such a warning alone is not evidence of malware.

Verify the published ZIP against `SHA256SUMS.txt`. Advanced users may also verify the GitHub artifact
attestations. Do not disable Defender or SmartScreen merely to run StickyMD.
