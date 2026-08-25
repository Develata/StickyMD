# Phase 06 Acceptance Matrix

> Verification projection for RaTeX-native math layout, direct tiny-skia painting and bounded
> formula caches. Automated rows are owned by the checked-in Rust smoke graph. Visual quality and
> real-DPI appearance remain `NOT TESTED`; repeatable OS resource observations are kept separate
> from manual visual acceptance.

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P06-A01 | 06 delimiter ownership; AC-014 | Automated | Comrak four-delimiter tests prove RaTeX receives delimiter-free literals | AUTOMATED PASS |
| P06-A02 | 06 RaTeX parser/layout authority | Automated | representative formula fixture, 10,000 deterministic inputs and checked-in rendering-stress corpus: 32 math nodes / 27 unique formula-layout keys all parse, layout and raster through [`phase-06.ps1`](../../tools/smoke/phase-06.ps1) | AUTOMATED PASS |
| P06-A03 | 06 direct native painter | Automated | exhaustive DisplayItem/PathCommand tests, embedded-font raster golden and dependency scan proving no PNG/WebView hot path | AUTOMATED PASS |
| P06-A04 | 06 inline/display geometry | Automated | baseline alignment, display centering, container and overwide-formula tests | AUTOMATED PASS |
| P06-A05 | 06 formula failure isolation; AC-015 | Automated | malformed/oversize/count-limit tests preserve original literal, atomic selection, border marker and hover detail | AUTOMATED PASS |
| P06-A06 | 06 exact copy semantics; AC-013/014 | Automated | four-delimiter, formula-only and select-all clipboard projection tests | AUTOMATED PASS |
| P06-A07 | 06 DPI/theme cache keys | Automated | 100/125/150/200% scale and Light/Dark rebuild tests | AUTOMATED PASS |
| P06-A08 | 06 bounded caches | Automated | 512-entry layout, 8 MiB raster and 4 MiB glyph-outline eviction tests | AUTOMATED PASS |
| P06-A09 | 06 duplicate reuse | Automated | counters prove repeated formula layout/raster hits | AUTOMATED PASS |
| P06-A10 | 06 resize/scroll stability | Automated | counters prove 100 resizes and 1,000 scrolls add no parse or raster work; stress fixture clamps `f32::MAX` overscroll and admits the deep local image only after the bottom viewport becomes visible | AUTOMATED PASS |
| P06-A11 | 06 hidden cache policy | Automated | Source transition and worker tests release raster projection while retaining reusable layout/font state | AUTOMATED PASS |
| P06-A12 | 06 math resources | Automated | 64 KiB/formula, 2,000 formulas/document and pathological raster allocation guards | AUTOMATED PASS |
| P06-A13 | 06 CJK/Unicode fallback | Automated | native `\\text{中文}` raster test with non-empty alpha output | AUTOMATED PASS |
| P06-A14 | 10 math performance | Automated | cold/warm formula plus 20 KiB/100 KiB/1 MiB math-document Release baseline via [`phase-06.ps1 -Performance`](../../tools/smoke/phase-06.ps1) | AUTOMATED PASS |
| P06-A15 | 11 runtime source safety | Automated | checked-in rendering-stress tests plus copied Release Preview/Split process-survival and byte-exact source tests; Phase 5 copied-runtime uses the same stress source while focused Phase 6 runtime retains malformed-formula isolation via [`phase-06.ps1 -Runtime`](../../tools/smoke/phase-06.ps1) | AUTOMATED PASS |
| P06-A16 | 11 dependency/unsafe/CI governance | Automated | Rust smoke governance, cargo-tree denylist, core/render unsafe scan and CI Phase 6 task | AUTOMATED PASS |
| P06-M01 | AC-014 representative visual fidelity | Manual | Current-commit Windows 11 Release screenshot matrix for focused and rendering-stress formula fixtures required | NOT TESTED |
| P06-M02 | AC-014 inline baseline and display centering | Manual | 100/125/150/200% DPI Light/Dark mixed-typography receipt required | NOT TESTED |
| P06-M03 | AC-015 error border/icon/hover presentation | Manual | malformed-formula hover and readability receipt required | NOT TESTED |
| P06-A17 | 10 formula memory and idle CPU | Automated | Six-state, five-run Private Working Set/Private Bytes matrix and 60 s CPU intervals through [`phase-06.ps1 -Resources`](../../tools/smoke/phase-06.ps1), with current-commit receipt in the Phase 6 report | AUTOMATED PASS |
| P06-A18 | 10 binary-size delta | Automated | Phase 5/Phase 6 stripped Release artifact byte count in the Phase 6 report | AUTOMATED PASS |
| P06-A19 | 10 source latency while math builds | Automated | 100 canonical 1 MiB source edits while the dedicated math worker is building, measured by the Phase 6 Release baseline | AUTOMATED PASS |
| P06-A20 | 04/05 persistence and reconciliation regression | Automated | Phase 6 Rust smoke reruns autosave, external clean reload/conflict, stale generation and byte-exact copied-runtime tests alongside math projection tests | AUTOMATED PASS |
| P06-M04 | 10 first-formula working-set delta | Manual | OS-level before/after first-formula memory observation required | NOT TESTED |
| P06-M05 | 10 Source-after-math cache release | Manual | Same-process Preview → Source working-set receipt required; automated cache/projection release remains covered by P06-A11 | NOT TESTED |
| P06-M06 | inherited Phase 3 real IME gate | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P06-M07 | inherited Phase 5 native preview visual gate | Manual | [`phase-05 matrix`](phase-05.md) | NOT TESTED |

## Current Phase Gate

The Rust CLI owns all headless Phase 6 checks. Passing them does not upgrade visual, IME, OS
or real first-formula memory observations; those rows deliberately remain `NOT TESTED`. P06-A17 is
the resource exception because it is a repeatable Rust-owned measurement, not a one-off manual log.
