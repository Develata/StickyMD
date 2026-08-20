# Phase 5 — Markdown Semantic Pipeline, Owned AST & Native Preview Foundation

## Status

Completed — awaiting USER review

## Prerequisites

- USER accepted the Phase 4 recommendation with its manual conditions still open.
- `DocumentState` remains the only canonical text authority.
- Source editor and portable persistence continue to use their existing typed coordination paths.

## Inherited Conditions

- Microsoft Pinyin, WeChat IME and candidate-position acceptance remain `NOT TESTED`.
- Real Notepad, ACL/read-only, kill-during-publish, long-path and live rare ReplaceFileW conditions
  remain `NOT TESTED` in the Phase 4 matrix.

## Purpose

Turn immutable `DocumentSnapshot` values into a native, read-only Markdown preview without leaking
Comrak Arena nodes, parser types or a second text authority into application state.

## Scope

- Locked Comrak 0.54 semantic adapter and project-owned AST.
- Source mapping, link/image classification and raw-HTML literal safety.
- RenderTree, native layout, viewport culling and preview selection mapping.
- One bounded preview worker, deterministic scheduling and stale-generation rejection.
- Source/Preview/Split view modes and config persistence of the selected mode.
- Repeatable semantic, robustness and Release performance smoke.

## Out of Scope

RaTeX production layout/raster, image decoding, assets/export, tray/dock, final theme/opacity and
RichEdit fallback.

## Authority Model

`DocumentState` owns canonical Markdown. `DocumentSnapshot`, OwnedDocumentTree, RenderTree and
LaidOutDocument are immutable, generation-tagged projections. No preview object can emit a source
mutation or persistence request.

## Deliverables

- Production semantic/layout modules in `stickymd-render`.
- Preview coordination and Windows shell projection wiring.
- `tools/smoke/phase-05.ps1` plus Rust CLI/CI integration.
- `docs/acceptance-cases/phase-05.md`, dependency delta, result report and coverage update.

## Verification

- Stable entry: `tools/smoke/phase-05.ps1`.
- Release measurement: `tools/smoke/phase-05.ps1 -Performance`.
- Environment-sensitive window smoke: `tools/smoke/phase-05.ps1 -Runtime`.
- Current evidence state: `docs/acceptance-cases/phase-05.md`.

## Risks

- Comrak source positions need exact UTF-8 inclusive-end conversion.
- cosmic-text ownership/Send constraints must not force unsafe sharing or UI-thread parsing.
- Full native text selection must remain a projection and cannot reuse source editor mutation state.

## Result

The owned semantic pipeline, native read-only preview, bounded worker, typed link activation,
phase-specific Rust smoke, golden/robustness tests and Release performance gates are implemented.
All automated Phase 5 matrix rows pass. Formal visual, memory/idle CPU and inherited manual gates
remain `NOT TESTED`; recommendation is `APPROVE Phase 6 WITH CONDITIONS`.
