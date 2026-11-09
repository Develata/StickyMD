# Changelog

All notable changes are recorded here.

## Unreleased

- While a display change is waiting to be applied no placement is confirmed, a topmost value whose
  native bit is unknown can be retried explicitly, and running out of action ids sends no frame.
- Close the review findings on the window receipts: a display-scale change is recovered and read
  only after the native message returns; a placement is read only when one is waiting; a topmost
  read-back failure is an unobservable bit, not a startup failure. Every show, hide, focus and
  topmost request now has a deadline for a host that does not answer, always-on-top settles the
  native bit in one order, a topology change while hidden keeps an unconfirmed drag out of the
  saved placement, and a cancelled quit re-projects input from the window's current state.
- A window placement is now confirmed by reading the window back: a drag's end and a recovery from
  a display change reach the configuration only once the window is seen at the target (within a
  tolerance of 0 pixels, measured on one display at 100%; mixed DPI is unmeasured). A placement
  that fails or is not confirmed within 500 ms shows a diagnostic and writes nothing. Animation
  frames are not read back.
- A window show now commits only after the window reads back as visible and not minimized: the
  tray text, input, the IME and the Preview come back on success, and a rejected show keeps the
  window hidden, reports why and can be retried. Always-on-top reaches the configuration, its
  write and the tray check only after the window's topmost bit reads back; the edge sensor's
  temporary value stays out of the configuration. Neither receipt claims focus, covering or z-order
  against other windows.
- Close the Phase 15 move (D): the session now runs Search's replace transactions, decides when a
  zoom is written, which window step the tray's show/hide command takes, the Preview's scheduling
  and the startup asset boundary; a view change can only be committed as it was allowed. Outside
  test builds the shell reads the session's state only through read-only views, so it cannot step
  around the session's transactions. Behaviour and data formats are unchanged.
- Move the shell out of the Windows crate into `stickymd-desktop` as `DesktopShell<H>`, with the
  winit callbacks in `shell/handler.rs` (Phase 15 C6). The session now decides view changes, when a
  window preference is written, the startup display and how Search opens. The shell's tests run
  on a portable test disk and an in-memory clipboard, also on Linux; the rows that read and write
  the note's files run once on that disk and once on the Windows storage. Behaviour and data
  formats are unchanged.
- Move the toolbar controls, scrollbars, Search layout and paint, Esc order, key table and
  window-independent pointer hit tests into `stickymd-desktop` (Phase 15 C3), and make
  `ControlLayout` the one geometry for paint, hover and press. The old second hit test of the view
  buttons is gone; no press at an integer position reached it.
- A selected toolbar button no longer paints its background into the resize band, the toolbar blank
  shows the normal arrow instead of the hand, and at 125% the Split sync button is painted on
  the rectangle it hit-tests.
- Put every native capability the shell uses behind narrow ports and make the shell generic over
  its host, with `WinHost` as the Windows implementation and `FakeHost` for tests (Phase 15 C5).
  Behaviour and data formats are unchanged.
- Move the shared application state and its business decisions into `AppSession` in
  `stickymd-app`, and the pure interaction rules into `interaction/rules.rs` (Phase 15 C4). The
  shell asks and answers through an ordered effect buffer, and a finished quit ends the event loop
  at the end of its callback. Behaviour and data formats are unchanged.
- Add the `stickymd-desktop` crate and move the view geometry, the title projection and the
  software frame path into it (Phase 15 C). Behaviour and data formats are unchanged; these
  tests now also run on Linux, where the crate selects its winit and softbuffer backends.
- Move the typed intents, coordinators, configuration model and Search and editor sessions into
  the new platform-free `stickymd-app` crate (Phase 15 A). Behaviour and data formats are
  unchanged; the moved tests now also run on Linux.
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
  invalid note.md aside only after checking its full content through a handle that shares only
  reads and renaming that same file through it; a too-large note.md is never read or moved
  automatically, and recovery asks the USER to move it aside first (USER decision).
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
