# Phase 5 — Markdown Semantic Pipeline, Owned AST & Native Preview Foundation

## Completion State

In Progress

The original foundation is complete. The USER-approved 2026-10-22 maintenance spans existing
Phases 05/06/07/10 and uses their existing verification entries; it does not add a runtime layer.

## 2026-10-22 maintenance execution

1. Update Plans 04/06/08/10, behavior, AC-039/040/041 and phase matrices before implementation.
2. Capture the same-fixture baseline and candidate math budgets; remove redundant text initialization.
3. Separate math measurement from raster residency; keep full geometry and use the existing worker
   for visible drawing, with a bounded transient path when cache admission is unavailable.
4. Deduplicate image requests within one refresh; measure before deciding on streaming validation.
5. Implement block horizontal scrolling for tables/code with one paint/hit-test/selection transform.
6. Attribute fixed-scale font/math-cache growth, apply justified local policies, then run targeted and
   workspace checks. Record actual performance, memory and remaining manual observations.

Boundary: preserve DocumentState authority, fixed worker count and existing file adapters. No global
resource manager, cross-thread font lock or general scrolling framework. Runtime changes stay on
`codex/preview-resource-optimization` in the independent checkout; preserve other sessions' work.

## Inputs

- USER accepted the Phase 4 recommendation with its manual conditions still open.
- `DocumentState` remains the only canonical text authority.
- Source editor and portable persistence continue to use their existing typed coordination paths.

## Inherited Conditions

- Microsoft Pinyin, WeChat IME and candidate-position acceptance remain `NOT TESTED`.
- Real Notepad, ACL/read-only, kill-during-publish, long-path and live rare ReplaceFileW conditions
  remain `NOT TESTED` in the Phase 4 matrix.

## Goal

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
