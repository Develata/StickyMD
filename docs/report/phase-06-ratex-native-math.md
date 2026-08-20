# Phase 6 RaTeX Native Math Report

## Executive Result

| Capability | Result | Evidence |
| --- | --- | --- |
| RaTeX Parser | **PASS** | exact `ratex-parser 0.1.14`; 50+ representative fixtures and 10,000 deterministic inputs |
| RaTeX Layout | **PASS** | exact `ratex-layout 0.1.14`; Inline uses `MathStyle::Text`, Display uses `MathStyle::Display` |
| Direct Native Painter | **PASS (automated)** | all `DisplayItem` and `PathCommand` variants plus embedded-font raster golden |
| Math Fonts | **PASS (automated)** | embedded KaTeX fonts, lazy first use, CJK fallback and OFL notice |
| Inline Math | **PASS (automated)** | atomic inline box, non-splitting wrap and mixed text containers |
| Display Math | **PASS (automated)** | independent centered block and safe over-width clipping |
| Baseline Alignment | **PASS (automated), visual conditional** | ascent/descent line-box tests pass; actual DPI appearance remains `NOT TESTED` |
| Math Error Isolation | **PASS** | parse/geometry/paint/resource errors preserve literal source and never fail the document |
| Math Selection/Copy | **PASS (automated)** | atomic formula selection and exact delimiter-bearing copy |
| Math Cache | **PASS** | 512-entry layout cache; raster <=8 MiB including source/metadata estimate; bounded outline cache |
| Memory | **PASS (automated local)** | six-state, five-run Windows resource matrix below |
| Idle CPU | **PASS (automated local)** | 60-second Source/Preview/Split math intervals below |
| Visual | **NOT TESTED** | current-commit 100/125/150/200% Light/Dark visual matrix remains manual |

Phase 6 completes the native math projection, not full v1. `DocumentState` remains the only source
authority; formula visuals and caches are disposable Preview projections.

## Preconditions and Environment

- Starting commit: `c527c4a2e20cde29a33cb8dfcb0eabf0e7c58c68`.
- Phase 5 recommendation: `APPROVE Phase 6 WITH CONDITIONS`; USER supplied and authorized Phase 6.
- Inherited Microsoft Pinyin, WeChat IME and native visual rows remain `NOT TESTED`.
- Windows 11 Home Chinese build 26200; Intel Core i7-12700H; 20 logical processors;
  16,962,281,472 bytes RAM; NTFS; 2560×1440 at 96 DPI.
- Rust/Cargo 1.97.1; Release uses fat LTO, one codegen unit, abort panic and stripped symbols.

## Phase 5 Preflight Baseline

Measured before RaTeX production dependencies were built, using the checked-in five-run Rust
resource harness:

| Mode | Private Working Set median / max | Private Bytes median / max | Idle CPU |
| --- | ---: | ---: | ---: |
| Source | 7,692,288 / 26,009,600 B | 8,572,928 / 28,581,888 B | 0.000000% |
| Preview, 20 placeholders | 17,145,856 / 17,240,064 B | 18,382,848 / 18,460,672 B | 0.000000% |
| Split, 20 placeholders | 18,120,704 / 18,161,664 B | 19,316,736 / 19,333,120 B | 0.000000% |

## Dependency Strategy

Production `stickymd-render` adds exact `ratex-parser/layout/types/font/font-loader/unicode-font`
0.1.14 and `ab_glyph 0.2.32`. `ratex-font-loader` disables defaults and enables only
`embed-fonts`. `ratex-render` is absent from both normal and dev dependency graphs; no PNG codec,
browser engine, JavaScript runtime or second tiny-skia version was added. Full license/runtime
analysis is in [`phase-06-dependency-delta.md`](phase-06-dependency-delta.md).

## Painter Strategy

StickyMD uses the approved thin-painter option. RaTeX owns parsing, semantic layout and
`DisplayList`; StickyMD only converts that display list to the existing tiny-skia framebuffer.

- Production adapter: 548 source lines across `painter.rs` and the cohesive `path_painter.rs` split.
- Covered variants: `GlyphPath`, `Line` (solid/dashed), `Rect`, `Path`; every `PathCommand`.
- Attribution: module header and `THIRD_PARTY_NOTICES.md` identify RaTeX 0.1.14 MIT adaptation.
- Golden: fraction, radical, matrix, large delimiter and color use fixed embedded fonts at 17 px;
  width/height/baseline, alpha coverage and stable raster hash are checked.
- Hot path: direct RGBA raster; no PNG encode/decode or intermediate SVG/PDF.

## Font Evidence

- Embedded KaTeX fonts are loaded only on the first formula raster request.
- Source mode does not start Preview work or request math fonts.
- `\text{中文}` exercises the native CJK fallback and produces non-empty native alpha coverage.
- KaTeX font assets are correctly identified as SIL OFL 1.1, not MIT. The full OFL and notice are
  checked in under `assets/licenses/` and referenced by `THIRD_PARTY_NOTICES.md`.

## Formula Coverage

| Category | Automated result |
| --- | --- |
| Fractions, nested fractions, binomial | PASS |
| Roots, indexed roots | PASS |
| Superscript/subscript | PASS |
| Sum/product/integral/limit/operators | PASS |
| Stretchy delimiters, floor/ceil/angle | PASS |
| Matrix/pmatrix/bmatrix/vmatrix | PASS |
| Cases/aligned | PASS |
| Greek, relations, sets, arrows | PASS |
| `mathbb`, `mathbf`, `mathcal`, `mathfrak`, `mathrm`, `mathit` | PASS |
| Text and CJK text | PASS |
| Accents, braces, overset/underset | PASS |
| Color and boxed/cancel | PASS |
| Four Markdown delimiters | PASS; delimiter ownership remains Comrak |
| Code/HTML literal non-math behavior | PASS |

No required fixture exposed an upstream RaTeX limitation. This is an automated semantic/raster
statement, not a claim that visual fidelity has been manually approved.

## Error and Resource Evidence

- Malformed formula, 65 KiB+ source, formula 2001+, invalid geometry and oversized raster are typed
  local errors.
- The preview retains the exact original formula literal, adds an error border/marker and stores a
  sanitized 160-character maximum hover diagnostic.
- 10,000 deterministic inputs do not panic.
- A single raster is rejected before allocation if a side exceeds 16,384 pixels or the entry would
  exceed the 8 MiB budget.

## Cache Evidence

| Cache | Bound | Key / accounting | Lifecycle |
| --- | ---: | --- | --- |
| Math layout | 512 entries | shared source + Inline/Display + baked foreground | retained across resize/scroll and Source transition |
| Math raster | 8 MiB hard | pixel bytes + source bytes + fixed metadata estimate; font size/theme included | cleared on DPI/theme change and Source transition |
| Glyph outline | 4 MiB | curve payload + fixed metadata estimate | worker-owned, bounded font projection |

The cache implementation uses `HashMap + monotonic recency stamp`: hits are average O(1), while
the small bounded map is scanned only when an eviction is actually required. Identical formula
reuse over 100 calls produces exactly 100 layout hits and 100 raster hits after one initial
parse/layout/raster; 600 unique formulas stay within all budgets. One hundred resizes and 1,000
scrolls add zero math parse/layout/raster calls. Cache generation is intentionally absent from
keys.

## Selection and Layout Evidence

- Inline formula is one atomic line box, baseline-aligned with native text and moved whole when it
  cannot fit the remainder of a line.
- Display formula is centered as an independent block; over-wide formula is clipped locally.
- List, quote, table and heading containers are covered.
- Selection treats formula geometry atomically and `Ctrl+C` retains the original delimiter pair.
- Malformed formula remains selectable/copyable as its exact source.

## Performance

All values are local Release measurements. Formula warm values are p50 / p95 / max over 100 cache
hits. The cold formula is a matrix containing fraction, root, sum and integral.

| Formula | Cold parse / layout / DisplayList / font+raster / total | Warm p50 | Warm p95 | Max |
| --- | ---: | ---: | ---: | ---: |
| simple `x^2` | shared cold row below | 0.1 µs | 0.2 µs | 3.1 µs |
| fraction | shared cold row below | 0.2 µs | 0.2 µs | 0.4 µs |
| matrix | shared cold row below | 0.2 µs | 0.2 µs | 0.4 µs |
| complex | 0.656 / 0.097 / 0.030 / 0.566 / **1.349 ms** | 0.2 µs | 0.3 µs | 0.8 µs |

Whole-document values are warm p50 / p95 / max after one cold build:

| Fixture | Cold | Comrak | Owned | RenderTree | Layout | Paint | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB + 20 | 205.268 ms | 0.638 / 0.783 / 0.844 ms | 0.403 / 0.500 / 0.506 ms | 0.282 / 0.342 / 0.388 ms | 7.824 / 8.431 / 8.675 ms | 1.203 / 1.260 / 1.618 ms | **10.464 / 11.189 / 11.629 ms** |
| 100 KiB + 100 | 261.338 ms | 2.649 / 3.105 / 3.126 ms | 1.651 / 1.881 / 1.887 ms | 1.028 / 1.244 / 1.466 ms | 37.264 / 39.259 / 39.371 ms | 1.115 / 1.314 / 1.549 ms | **44.477 / 46.324 / 46.990 ms** |
| 1 MiB + 500 | 766.213 ms | 42.887 / 44.712 / 44.996 ms | 29.909 / 31.756 / 32.190 ms | 16.767 / 18.548 / 19.577 ms | 597.717 / 612.688 / 613.053 ms | 1.315 / 1.772 / 2.327 ms | **697.152 / 712.252 / 713.258 ms** |

All warm totals pass 100/400/2000 ms gates. The 100-edit 1 MiB Source benchmark while the math
worker was building measured p95 **0.8 µs**, max **298.7 µs**, proving no preview lock or math work
entered the source mutation path. Cold values are reported separately and run in the background.

Compared with the Phase 5 plain Preview p95 totals (13.240 / 87.570 / 824.282 ms), the current
math fixtures measured 11.189 / 46.324 / 712.252 ms. Fixture content differs, so the negative delta
is evidence that RaTeX did not break the gates, not a claim that formulas make arbitrary Markdown
faster.

| Fixture | Phase 5 plain p95 | Phase 6 math p95 | Observed delta |
| --- | ---: | ---: | ---: |
| 20 KiB + 20 | 13.240 ms | 11.189 ms | -2.051 ms |
| 100 KiB + 100 | 87.570 ms | 46.324 ms | -41.246 ms |
| 1 MiB + 500 | 824.282 ms | 712.252 ms | -112.030 ms |

## Memory

The final six-state resource matrix is generated by `tools/smoke/phase-06.ps1 -Resources`; every
state has five fresh copied-Release processes after 30-second warmup. CPU rows use a separate
60-second interval after the same warmup.

| State | Private Working Set median | PWS max | Private Bytes median | Private Bytes max | 60 s idle CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Source, 20 formulas lazy | 7,700,480 B | 7,716,864 B | 8,581,120 B | 8,609,792 B | 0.001302% |
| Preview, no math | 16,965,632 B | 17,063,936 B | 18,051,072 B | 18,157,568 B | not measured |
| Preview, 1 formula | 17,100,800 B | 17,186,816 B | 18,272,256 B | 18,309,120 B | not measured |
| Preview, 20 formulas | 17,207,296 B | 17,326,080 B | 18,391,040 B | 18,530,304 B | 0.000000% |
| Split, 20 formulas | 18,153,472 B | 18,186,240 B | 19,312,640 B | 19,353,600 B | 0.000000% |
| Preview, 200 unique | 20,590,592 B | 20,676,608 B | 21,856,256 B | 21,913,600 B | not measured |
| Source after math, same process | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |

Against the pre-RaTeX Phase 5 medians, the comparable 20-formula Preview PWS increased 61,440 B
and Split PWS increased 32,768 B; their Private Bytes changed by +8,192 B and -4,096 B
respectively. The independent no-math/one-formula processes differ by 135,168 B PWS, but that is
not substituted for the stricter same-process first-formula observation below.

Exact same-process first-formula and Preview-to-Source working-set deltas remain manual
`NOT TESTED`; automated cache/projection release is covered independently.

## Binary Size

| Artifact | Bytes |
| --- | ---: |
| Phase 5 stripped Release EXE | 3,495,424 |
| Phase 6 stripped Release EXE | 6,930,944 |
| Delta | +3,435,520 (+3.276 MiB, +98.29%) |

The increase is dominated by portable embedded math fonts. It is below the Phase 6 +8 MiB review
trigger and leaves substantial room under the v1 30 MiB portable ZIP hard limit.

## Runtime and Regression Evidence

- Copied Release executable survives native RaTeX Preview and Split fixtures, including a malformed
  formula; source bytes remain exactly unchanged.
- Phase 6 smoke reruns Windows autosave, external reload/conflict, stale generation and recovery
  regressions together with math projection tests.
- Source transition releases raster projection; stale worker completion cannot replace a newer
  generation.

## Architecture Authority

| Question | Answer |
| --- | --- |
| Formula source owner | `DocumentState` only |
| Delimiter authority | Comrak |
| Math semantics/layout/geometry | RaTeX parser/layout |
| DisplayList painter | thin StickyMD tiny-skia adapter |
| Can math mutate `DocumentState`? | no |
| Can cache become authority? | no; every entry is disposable projection |

`stickymd-core` contains no RaTeX type or dependency. The Preview worker owns `MathEngine`; the UI
receives generation-tagged immutable frames only.

## Unsafe and Windows API

- `stickymd-core` runtime unsafe: **0**.
- `stickymd-render` runtime unsafe: **0**.
- Phase 6 added no Windows API and no product unsafe block.
- Development-only `stickymd-smoke` uses two documented Win32 process-metric calls for repeatable
  memory/CPU evidence; these do not enter the product executable.

## Architecture Drift

None. The painter remains a display adapter, the caches remain bounded projections, and neither
RaTeX nor platform types leak into the canonical document domain.

## Acceptance and Manual Conditions

The durable matrix is [`docs/acceptance-cases/phase-06.md`](../acceptance-cases/phase-06.md), driven
by [`tools/smoke/phase-06.ps1`](../../tools/smoke/phase-06.ps1) and the Rust CLI. All headless rows
that are portable and suitable for hosted CI are in the CI task graph; copied-GUI runtime and
Windows resource rows remain stable local automation entry points. These remain `NOT TESTED`:

- representative formula visual fidelity;
- real 100/125/150/200% DPI Light/Dark baseline/centering/error presentation;
- same-process first-formula and Preview-to-Source working-set deltas;
- Microsoft Pinyin, WeChat IME and inherited Phase 5 visual interaction.

| Visual/manual item | Result |
| --- | --- |
| Representative formula fidelity | NOT TESTED |
| 100/125/150/200% DPI Light/Dark baseline and centering | NOT TESTED |
| Malformed formula hover/readability | NOT TESTED |
| Same-process first formula memory | NOT TESTED |
| Same-process Preview to Source release | NOT TESTED |
| Microsoft Pinyin / WeChat IME | NOT TESTED |

## Verification

| Command / entry | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `cargo build --workspace --release --locked` | PASS |
| `cargo test -p stickymd-core --release --locked` | PASS |
| `cargo test -p stickymd-render --release --locked` | PASS |
| `cargo test -p stickymd-win --release --locked` | PASS |
| `cargo deny check` | PASS; duplicate-version warnings documented in dependency delta |
| `tools/smoke/phase-06.ps1` | PASS |
| `tools/smoke/phase-06.ps1 -Performance` | PASS |
| `tools/smoke/phase-06.ps1 -Runtime` | PASS |
| `tools/smoke/phase-06.ps1 -Resources` | PASS |
| `tools/smoke/all.ps1 -Ci` | PASS |
| `git diff --check` | PASS |

## Recommendation

**APPROVE Phase 7 WITH CONDITIONS**: retain every manual row above as `NOT TESTED`; do not weaken
the Comrak/RaTeX authority boundary, bounded cache accounting, generation checks or direct native
painter contract.
