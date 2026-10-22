# Changelog

All notable changes are recorded here.

## Unreleased

- Prepare 0.1.3 with viewport-based formula bitmaps, bounded preview caches, lower image-read
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
