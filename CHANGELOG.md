# Changelog

All notable changes are recorded here.

## Unreleased

- Give the external-file, recovery-cleanup and canonical-preserve I/O receipts an identity:
  an external observation carries the Durable Fingerprint known when it was requested and is
  superseded (and re-checked) if that changed, and recovery receipts are accepted only for the
  pending recovery operation, closing a drift from the plan 04 generation rules.
- A save submitted after loading external content now supersedes the still-queued cleanup of
  the fixed temp, so a failed save's temp (the only durable copy of newer text) is not deleted;
  restoring over an unusable note.md now first checks it is still the observation that required
  preserving it, and a note.md changed while recovery waited becomes a conflict instead of being
  moved aside.
- At startup a valid note.md.tmp whose content differs from note.md is always offered for
  recovery, whatever the file times; it is no longer deleted for being older (it can hold the
  only copy of text from a save that failed against an external change). Recovery moves an
  unusable note.md aside only after fingerprinting its full content under a handle that denies
  other writers and moving it while that handle is held; a too-large note.md is fingerprinted
  once at startup within a 256 MiB budget, and one that cannot be verified is never moved.
- Make the manual acceptance items that need another machine, a VM or a second display optional:
  the previous Windows 11 role, the Clean VM, dual-monitor, display-disconnect, RDP and
  negative-coordinate cases no longer block a release when untested and need no waiver; an
  observed failure still blocks. Left/right dock under mixed DPI moves from M11/M12 to M36.

## 0.1.3 - 2026-10-23

- Release 0.1.3 with viewport-based formula bitmaps, bounded preview caches, lower image-read
  allocations, and horizontal scrolling for wide tables, code blocks and display mathematics.
- Fix Search replacement/navigation, compact Search layout, IME-owned key routing, opacity popup
  behavior and waking a collapsed dock; improve exact-candidate desktop verification.
- Extend the math conversion button to join standalone equals lines inside complete display blocks.
- Refreshed user, contribution, release and governance documentation after the first public release.

## 0.1.0 - 2026-08-30

- Published the first Windows 11 x64 portable release from exact source
  `64690ab8f86f63f3cbfeabbb0961276978c8f26d`.
- Added the native Source editor, Comrak Preview, RaTeX-compatible mathematics, managed images,
  Markdown export, autosave/recovery/external-conflict handling, tray lifecycle, three-edge docking,
  themes, opacity, content zoom, Split semantic scroll sync, and literal find/replace.
- Added deterministic packaging, checksums, SPDX SBOM, native-runtime import checks, exact-artifact
  qualification, module success ledgers, and independently authorized tag/draft/publish promotion.
- Published the unsigned portable ZIP, `SHA256SUMS.txt`, and `SBOM.spdx.json`; known environment
  qualification gaps and verification hashes are recorded in
  [`docs/release-notes/0.1.0.md`](docs/release-notes/0.1.0.md).
