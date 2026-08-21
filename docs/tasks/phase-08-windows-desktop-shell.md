# Phase 08 — Windows Desktop Shell

## Status

Completed — awaiting USER review.

## Preconditions

- Starting commit: `a7af3a40fa357edd36cd1ae231e1c936f1b763bd`.
- Phase 7 recommendation: `APPROVE Phase 8 WITH CONDITIONS`.
- USER supplied and authorized the frozen Phase 8 prompt.

## Inherited Conditions

- Microsoft Pinyin and WeChat IME remain `NOT TESTED`.
- Native Preview, RaTeX and local-image visual/DPI matrices remain `NOT TESTED`.
- Real clipboard producers, native export dialog, crash timing and real junction/symlink receipts
  remain `NOT TESTED`.

## Scope

- One explicit `WindowShellState` authority for visibility, docking, animation, timers and quit
  staging.
- Undecorated resizable paper window with a fixed native top-control row.
- Left, right and top docking; 3-DIP sensor strip; deterministic hover/focus timers.
- Tray lifecycle, close-to-tray, safe tray quit, topmost, fixed themes and whole-window opacity.
- Stable monitor identity, signed geometry, mixed-DPI conversion and missing-monitor recovery.
- A single `ConfigCoordinator` with monotonic revisions and coalesced durable writes.
- Rust-owned headless smoke, a thin Phase 08 PowerShell entry, and a frozen acceptance matrix.

## Out of Scope

- Bottom docking, draggable split divider, configurable visual effects or animation durations.
- Installer, auto-start, global shortcut, Windows 10, ARM64, network or update behavior.
- Advancing any manual visual, IME, physical-monitor or tray receipt without checked-in evidence.

## Authority Model

```text
DocumentState       = canonical Markdown authority
WindowShellState    = runtime window/dock/lifecycle authority
ConfigCoordinator   = committed preference authority
config.toml          = durable projection
winit / Win32 / tray = platform facts and effect adapters only
```

## Implementation Slices

1. Pure geometry, timers, visibility and lifecycle reducer.
2. Revisioned runtime configuration projection.
3. Minimal Windows opacity, display identity/work-area, tray and manifest adapters.
4. Winit event translation and typed effect integration.
5. Shared top-control paint/hit-test model.
6. Static source-frame cache with bounded caret overlay for idle blinking.
7. Rust smoke, performance/resource routes and frozen acceptance projection.

## Safety Gates

- Dirty close freezes document mutation and hides only after the latest save succeeds.
- Tray quit waits for paste, latest note save, safe managed-asset GC and config acknowledgement.
- Note-save failure cancels Quit and keeps the process alive; asset-GC/config failure preserves the
  note and evidence, emits a warning, and follows the approved best-effort exit policy.
- Focus, IME, popup, conflict, recovery and active drag guard automatic collapse.
- Hidden/collapsed/hover-revealed-unfocused states cannot commit keyboard or IME text.
- Caret blinking is scheduled only while the window state accepts source mutation; collapsed and
  hidden states do not retain an editor redraw timer.
- Programmatic animation and recovery geometry never produce per-frame config writes.

## Verification

- `tools/smoke/phase-08.ps1`
- `tools/smoke/phase-08.ps1 -Performance`
- `tools/smoke/phase-08.ps1 -Runtime`
- `tools/smoke/phase-08.ps1 -Resources`
- workspace fmt, clippy, tests, Release build, cargo-deny and diff checks.

## Result

- Pure window/dock/timer/lifecycle reducer: automated PASS.
- Revisioned/coalesced configuration authority: automated PASS.
- Windows tray, opacity, topmost, monitor identity/work-area and PerMonitorV2 adapters: automated PASS.
- Copied-Release Dock/Close/wake/isolation lifecycle: automated PASS.
- Five-run visible/collapsed/hidden resources and repeated-cycle leak checks: automated PASS.
- Cold startup: conditional; median 393.911 ms and max 1429.905 ms exceed the 300 ms hard gate.
- All inherited real IME, visual, tray and physical-monitor rows remain `NOT TESTED`.

Recommendation: `APPROVE Phase 9 WITH CONDITIONS`; see
[`RISK-source-font-startup.md`](../report/RISK-source-font-startup.md).
