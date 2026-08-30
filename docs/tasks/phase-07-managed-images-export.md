# Phase 7 — Managed Images, Clipboard Paste, Asset Transactions, GC & Export

## Completion State

Completed

The automated implementation gate is complete. Every real clipboard, visual, crash-timing,
reparse-point and inherited IME row listed below remains explicitly `NOT TESTED`.

## Goal

建立 managed image、Windows clipboard paste、资产事务/GC、native image Preview 与 Markdown
export，同时保持文档与用户资产 authority、失败原子性和资源上限清晰可证。

## Inputs

- Starting commit: `44e3e08`.
- Phase 6 recommendation: `APPROVE Phase 7 WITH CONDITIONS`.
- USER supplied and authorized the Phase 7 task.

## Inherited Conditions

- Microsoft Pinyin and WeChat IME remain `NOT TESTED`.
- Native Preview/RaTeX Light/Dark/DPI visual rows remain `NOT TESTED`.
- The inherited strict same-process first-formula memory row remains `NOT TESTED`; Phase 7 now
  automates the image-saturated Preview-to-Source release row separately.

## Scope

- Strict managed-image identity, ownership proof and conservative reference tracking.
- File, encoded-image, DIB/bitmap and text clipboard priority on Windows.
- Bounded asynchronous image preparation and all-or-nothing Markdown insertion.
- Pure asset effects attached to private Undo entries and serialized I/O reconciliation.
- Local-image metadata/decode projection, viewport-prefetch and bounded cache.
- Startup/normal-exit reconciliation and proof-gated physical deletion.
- Current-snapshot Markdown export with local-resource staging and source-preserving rewrites.
- Phase 7 Rust CLI smoke, PowerShell entry, CI route and durable acceptance matrix.

## Out of Scope

- Generic attachments, drag-and-drop UI or image editing.
- Remote image download or any network client.
- Animated-image playback.
- Final toolbar/tray/docking/theme/opacity work.
- Claiming real clipboard, native dialog, visual, crash-timing or OS-resource approval without a
  durable receipt.

## Asset Authority

`DocumentState` remains the only Markdown authority. Managed reference counts are a conservative
derivation from canonical text. Filesystem layout is durable state, not a competing reference
authority. Preview and export consume snapshots and cannot mutate the document.

## Ownership Proof

Automatic move/delete requires all of: canonical `images/` or `.trash/` location, ordinary
non-reparse file, strict lowercase managed filename, and a full-file SHA-256 whose prefix matches
the filename. A managed-looking user file with wrong content hash is never operated on.

## Clipboard Pipeline

Windows capture order is CF_HDROP, registered encoded PNG/JPEG/WebP formats, DIB/V5, CF_BITMAP,
then Unicode text. Byte count, dimensions and file count are bounded; clipboard-open retry is
finite. Real Explorer/Snipping Tool/Paint/browser receipts remain manual `NOT TESTED`.

Image files are read, prepared and stored one at a time under a cumulative 128 MiB transaction
cap before any Markdown edit. A failed/stale paste returns its publication ledger to the main
coordinator, which converges those names against the latest canonical references instead of
blindly replaying an obsolete rollback.

## Managed Persistence

Stable PNG/JPEG/WebP/GIF bytes are preserved. BMP/ICO/DIB/raw RGBA normalize to PNG. Content
addressing chooses 20 hexadecimal characters by default and extends to 32/64 on a verified prefix
collision. Existing equal content is reused; unrelated content is never overwritten.

## Reference Scanner

The scanner recognizes only the strict managed basename and intentionally counts literals in code
and raw text. A bounded edit-window algorithm updates the typical single-edit path without parsing
the full document, while full scan remains the reconciliation boundary.

## Undo/Redo Effects

Private Undo entries carry pure `AssetEffect` values. Canonical text changes first; the single I/O
worker then converges active/trash state. I/O failure never rolls back already-correct text. One
multi-image paste is one `TextDelta` and stale-generation commit is rejected.

## Preview Image Pipeline

The Preview worker resolves local image bytes through an injected source, checks actual format and
dimensions, decodes only the viewport plus 300 DIP, preserves aspect ratio, never upscales and
renders the first frame only. Remote images remain non-fetching placeholders. Standalone,
mixed-text and table-cell local images use native raster projection; missing/corrupt/budget-deferred
images remain safe placeholders.

## Cache

Decoded RGBA is bounded by both 16 MiB and 512 entries. Accounting includes pixels, metadata and
rasters leased by current layout chunks; leased entries cannot be fake-evicted while their `Arc`
memory remains live. Source mode releases decoded image state. No global generic resource cache was
introduced.

## Startup Reconciliation

Only after note/recovery resolution, the latest canonical text is scanned. Runtime reconcile is
strictly reversible. Physical delete is allowed only at startup/normal-exit safe boundaries while a
stable `note.md` read handle still matches the expected durable fingerprint; durable and latest
runtime references are unioned. Uncertainty defers deletion. User and wrong-hash files stay.

## GC

Runtime 1→0 transitions are logical moves to `.trash`, not physical deletion. Note persistence has
priority on the single worker. Failed final note save prevents destructive exit reconciliation.
Unique request IDs prevent an old same-generation completion from satisfying a newer quit request;
pending paste and input are gated during shutdown.

## Export

Ctrl+Shift+S captures the current immutable snapshot and invokes a thin native save dialog. Comrak
identifies actual image nodes; local files are streamed, hashed, deduplicated and staged; remote
URLs remain unchanged. Rewrites are non-overlapping and applied in reverse source order. Assets are
published before the atomically written Markdown. The active document/path/generation/dirty/history
are not changed. If asset-directory publication succeeds but Markdown publication fails, the safe
orphan directory is retained rather than risking deletion of user files.

## Security

- No network dependency or remote-image request path.
- All write/delete names are StickyMD-generated and proof-gated.
- Reparse directories/files fail closed for automatic destructive operations.
- Untrusted image allocations use encoded-size, side, pixel and checked-byte limits.
- `stickymd-core` and `stickymd-render` remain `forbid(unsafe_code)`.

## Performance

Release measurements cover 1 MiB full/incremental scanning, ten-image paste, scaled decode and
twenty-reference export. OS process-resource automation has a stable `-Resources` entry; results
are reported separately from algorithmic cache invariants and include a same-process
image-saturated Preview-to-Source release receipt.

## Deliverables

- Managed image identity、ownership proof、reference tracker 与 asset-effect Undo/Redo。
- Bounded clipboard preparation、local-image decode/cache、startup/exit reconciliation 与 safe GC。
- Snapshot-based Markdown export、Windows adapter、Phase 7 smoke 与 acceptance matrix。

## Verification

- `tools/smoke/phase-07.ps1`
- `tools/smoke/phase-07.ps1 -Performance`
- `tools/smoke/phase-07.ps1 -Runtime`
- `tools/smoke/phase-07.ps1 -Resources`
- workspace fmt、Clippy、tests、Release build、dependency/unsafe/forbidden-architecture scans。

## Manual Verification

Real Windows clipboard sources, visible paste/undo/redo/restart, local-image visual/DPI behavior,
native export dialog, forced-crash timing and inherited IME/visual rows remain `NOT TESTED` in the
Phase 7 acceptance matrix.

## Risks

- `image` decoders process untrusted bytes; strict limits and current dependency audit reduce but do
  not eliminate upstream decoder risk.
- Inline raster layout is native but its final visual fidelity remains a manual acceptance row.
- Export intentionally leaves a uniquely named asset directory if final Markdown publication fails
  after asset publication; this is recoverable clutter, not data loss.

## Result

Implementation and repeatable headless/runtime automation are complete. Manual gates remain open;
the final recommendation is recorded in `docs/report/phase-07-managed-images-export.md`.
