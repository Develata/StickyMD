# Phase 6 — RaTeX Native Math

## Status

Implementation Complete — manual verification incomplete; awaiting USER review.

## Prerequisites

- Phase 5 recommendation: `APPROVE Phase 6 WITH CONDITIONS`.
- USER explicitly supplied and authorized the Phase 6 task.
- Phase 5 source/preview/split resource preflight completed before the RaTeX production graph was
  built; no structural blocker was observed.

## Inherited Conditions

- Native Preview visual fidelity remains `NOT TESTED`.
- Microsoft Pinyin and WeChat IME manual gates remain `NOT TESTED`.
- Phase 6 formula visual/DPI/theme and first-formula OS-memory rows remain `NOT TESTED` until
  durable current-commit receipts exist.

## Preflight Baseline

Before RaTeX entered the production graph, Source/Preview/Split five-run medians were
7,692,288 / 17,145,856 / 18,120,704 bytes Private Working Set. All three 60-second idle CPU
observations were 0.000000%. The exact max and Private Bytes values are retained in the report.

## Scope

- RaTeX parser/layout/font integration in `stickymd-render` only.
- Direct native DisplayList painter; no PNG encode/decode or WebView.
- Inline/display formula geometry, exact copy mapping, failure isolation and hover diagnostics.
- Bounded layout, raster and glyph-outline caches.
- Source-mode raster release, DPI/theme invalidation and preview-worker ownership.
- Phase 6 Rust smoke, PowerShell entry, CI headless task and acceptance matrix.
- License notices and Release performance/resource evidence.

## Out of Scope

- Images/assets/export.
- Tray/docking/final theme controls.
- Editing math in Preview.
- TeX parser/layout reimplementation.
- Formula visual golden approval without a manual receipt.

## Authority Model

Comrak owns delimiters. RaTeX owns math parsing/layout. StickyMD owns only projection, cache,
native painting and selection geometry. `DocumentState` remains canonical; formula raster and
DisplayList values are disposable projections.

## RaTeX Version

All production RaTeX crates are exact-pinned to 0.1.14. `ratex-font-loader` disables default
features and enables only embedded KaTeX fonts.

## Dependency Strategy

Only parser/layout/types/font/font-loader/unicode-font plus `ab_glyph` enter render. The public PNG
renderer is absent, and no browser, JavaScript runtime or second tiny-skia version enters the
normal graph.

## Math Adapter

Comrak-owned math literals flow to RaTeX parse/layout/DisplayList and then a thin StickyMD painter.
No TeX tokenization, semantic AST or layout rule is reimplemented.

## Painter

The painter covers glyph paths, solid/dashed lines, rectangles and arbitrary display paths. It is
split into cohesive display-list and path-command modules and has deterministic embedded-font
raster golden coverage.

## Font Strategy

KaTeX-compatible fonts are embedded for portable Release builds and loaded on first math raster.
CJK text uses native fallback. MIT/OFL notices are checked into the release source tree.

## Layout Integration

Inline formulas are atomic baseline-aligned boxes; display formulas are centered independent
blocks. Tall/overwide formulas stay locally bounded and never alter canonical source.

## Cache

Layout is capped at 512 entries. Raster accounting is capped at 8 MiB and includes pixels, source
and metadata. Glyph outlines are capped at 4 MiB. Average hits are O(1); bounded scans occur only
on eviction.

## Selection

Formula hit testing and selection are whole-object. Copy always returns the exact source range,
including the original delimiter pair.

## Error Handling

Malformed, oversized, over-count and invalid-geometry formulas become local literal error boxes
with bounded diagnostics; they cannot abort the document or mutate its text.

## Resource Guards

Each source is capped at 64 KiB, each document at 2,000 formulas, each raster side at 16,384 pixels,
and all three caches are bounded. Ten thousand deterministic formula inputs are panic-free.

## Performance

Release formula/document, source-concurrency, six-state memory and 60-second CPU receipts are
recorded in `docs/report/phase-06-ratex-native-math.md` and owned by the Rust smoke CLI.

## Manual Verification

Real 100/125/150/200% DPI Light/Dark visual appearance, same-process first-formula memory,
Preview-to-Source working-set release and inherited real IME rows remain `NOT TESTED`.

## Deliverables

- `stickymd-render/src/math/{engine,painter,cache}.rs`.
- Preview mixed inline/display placement and failure presentation.
- Representative and adversarial formula fixtures.
- `tools/smoke/phase-06.ps1`, Rust smoke graph and `phase-06.md` acceptance matrix.
- Phase 6 dependency delta, task/report, plan projection and third-party notices.

## Verification

- Headless unit/integration tests cover delimiter ownership, 50+ representative formulas, 10,000
  deterministic inputs, malformed/oversize/count guards, cache budgets, DPI/theme, selection/copy,
  resize/scroll counters and worker release ordering.
- Release benchmarks cover cold/warm formula stages and 20 KiB/100 KiB/1 MiB math documents.
- Copied Release runtime smoke checks Preview/Split survival and byte-exact source preservation.
- Manual visual and same-process memory items remain separate in the acceptance matrix; the
  repeatable six-state Windows resource matrix is automated by the Rust smoke CLI.

## Risks

- RaTeX 0.1.14 is still an early-version dependency; exact pins and a thin replaceable adapter limit
  migration surface.
- Direct painter fidelity cannot be declared complete until the manual DPI/theme matrix is signed.
- Embedded fonts increase binary and first-formula memory; current data is recorded in the Phase 6
  report rather than hidden behind a generic performance claim.

## Result

Implementation and automated contracts are complete. Recommendation and measured conditional/manual
items are recorded in `docs/report/phase-06-ratex-native-math.md`.
