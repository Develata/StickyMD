# Phase 10 — UX Corrections, Automation Consolidation and RC Requalification

> Status: Implementation Complete — RC validation incomplete. The later USER-approved Phase 14
> startup policy in `docs/plan` supersedes the historical warm-startup blocker. Real keyboard,
> visual, shell-switcher, IME, tray, sensor and physical-pointer rows remain `NOT TESTED` without a
> current-candidate receipt.

## Automated Contract Matrix

| ID | Plan / AC contract | Mode | Required checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P10-A01 | Phase 10 prompt archive and plan-first amendment | Automated | governance/coverage audit | AUTOMATED PASS |
| P10-A02 | Rust CLI is the automated planning/gate/evidence authority | Automated | CLI task-plan and authority tests | AUTOMATED PASS |
| P10-A03 | Human and JSON evidence share one result model | Automated | schema v1 serialization/golden tests | AUTOMATED PASS |
| P10-A04 | CLI exit 0=passed; nonzero=failed/blocked/not-verifiable request | Automated | subprocess exit-code tests | AUTOMATED PASS |
| P10-A05 | PowerShell Phase 10 wrapper is thin | Automated | source audit + wrapper invocation | AUTOMATED PASS |
| P10-A06 | CI invokes all headless Phase 10 work exactly once | Automated | `all --ci` task graph test + workflow audit | AUTOMATED PASS |
| P10-A07 | AC-031 Ctrl+Insert aliases CopySelection | Automated | keyboard translation/intent-equivalence tests | AUTOMATED PASS |
| P10-A08 | AC-031 Shift+Delete aliases CutSelection with failure atomicity | Automated | keyboard + clipboard-failure tests | AUTOMATED PASS |
| P10-A09 | AC-031 Shift+Insert aliases full PasteClipboard path | Automated | text/image/file-list route-equivalence tests | AUTOMATED PASS |
| P10-A10 | AC-031 Preview traditional shortcut boundary | Automated | Preview copy/no-mutation tests | AUTOMATED PASS |
| P10-A11 | AC-032 ContentZoomPercent 50..=300 default 100 | Automated | value/config validation tests | AUTOMATED PASS |
| P10-A12 | AC-032 keyboard zoom ±10 and Ctrl+0 | Automated | main/numpad keyboard mapping tests | AUTOMATED PASS |
| P10-A13 | AC-032 Ctrl+wheel ±5/notch with high-resolution accumulator | Automated | line/pixel delta accumulator tests | AUTOMATED PASS |
| P10-A14 | AC-032 one ConfigCoordinator authority and coalesced writes | Automated | revision/write-count/debounce tests | AUTOMATED PASS |
| P10-A15 | AC-032 Source/Preview/Split share zoom; Shell is unscaled | Automated | geometry/projection tests | AUTOMATED PASS |
| P10-A16 | AC-032 zoom does not mutate Document or reparse Markdown | Automated | generation/parse-counter tests | AUTOMATED PASS |
| P10-A17 | AC-032 math invalidation and image cache remain bounded | Automated | cache-key/budget/resource tests | AUTOMATED PASS |
| P10-A18 | AC-033 minimum 220×120, default 520×680 | Automated | builder/config/geometry boundary tests | AUTOMATED PASS |
| P10-A19 | AC-033 Source/Preview remain operable at minimum | Automated | layout/hit-test/scroll tests | AUTOMATED PASS |
| P10-A20 | AC-033 Split remains 50/50 with 1-DIP divider and no mode switch | Automated | compact Split layout/regression tests | AUTOMATED PASS |
| P10-A21 | AC-034 platform style is Tool Window without NOACTIVATE | Automated | HWND style readback and source audit | AUTOMATED PASS |
| P10-A22 | AC-034 tray/sensor/second-instance reachability model | Automated | lifecycle/reducer/copied-runtime tests | AUTOMATED PASS |
| P10-A23 | AC-035 capture threshold is 24 DIP in monitor-local DIP | Automated | mixed-DPI geometry boundary tests | AUTOMATED PASS |
| P10-A24 | AC-035 nearest eligible edge, tie epsilon 1 DIP | Automated | non-tie/tie/corner property tests | AUTOMATED PASS |
| P10-A25 | AC-035 Top>Left>Right applies only within tie; Bottom absent | Automated | counterexample tests | AUTOMATED PASS |
| P10-A26 | AC-035 release enters DockedExpanded; legacy collapse/detach remain | Automated | reducer/timer/detach tests | AUTOMATED PASS |
| P10-A27 | AC-036 opacity 40..=100 default 96 | Automated | config/control/reducer/adapter tests | AUTOMATED PASS |
| P10-A28 | AC-036 40% remains interactive; 100% style cleanup | Automated | HWND alpha/style/hit-test facts | AUTOMATED PASS |
| P10-A29 | startup readiness object is unique and previous process fully exits | Automated | startup harness unit/integration tests | AUTOMATED PASS |
| P10-A30 | final cold cohort has >=30 samples and p95 <=300 ms or exact USER waiver | Automated | 30 samples, p95 321.540 ms; original 300 ms USER WAIVED, 400 ms gate passed | AUTOMATED PASS |
| P10-A31 | final warm cohort has >=30 samples and satisfies the current USER-approved startup release policy | Automated | Phase 14 policy regression: 180 ms preferred, 400 ms diagnostic, 550 ms v0.1.0 hard boundary | AUTOMATED PASS |
| P10-A32 | 50/100/300 zoom performance/resource/leak gates | Automated | Release benchmark + copied-runtime resources at `b9f83f1` | AUTOMATED PASS |
| P10-A33 | AC-001..AC-030 automated regressions | Automated | final Rust CLI task graph | AUTOMATED PASS |
| P10-A34 | exact Phase 10 EXE/ZIP/SBOM/checksum/package verification | Automated | verified ZIP `70c8e6f5…fba7e0`, source `9c0e862` | AUTOMATED PASS |
| P10-A35 | Phase 9 artifact is superseded and not reused as final candidate | Automated | Phase 10 filename/hash and obsolete-artifact audit | AUTOMATED PASS |
| P10-A36 | core/render unsafe=0 and forbidden runtime dependencies absent | Automated | source/dependency/governance scan | AUTOMATED PASS |

## Required Manual UX Matrix

| ID | Required observation | Mode | Required receipt | Status |
| --- | --- | --- | --- | --- |
| UX10-01 | Ctrl+Insert copies real Source/Preview selection | Manual | current-candidate keyboard/clipboard receipt | NOT TESTED |
| UX10-02 | Shift+Delete cuts real Source selection | Manual | current-candidate keyboard/clipboard receipt | NOT TESTED |
| UX10-03 | Shift+Insert pastes Unicode text | Manual | current-candidate keyboard/clipboard receipt | NOT TESTED |
| UX10-04 | Shift+Insert pastes real image/file clipboard through asset transaction | Manual | current-candidate clipboard/preview/undo receipt | NOT TESTED |
| UX10-05 | 50% Source/Preview/Split visual and input | Manual | screenshots + actual/expected receipt | NOT TESTED |
| UX10-06 | Ctrl+0 restores 100% across all views | Manual | current-candidate interaction receipt | NOT TESTED |
| UX10-07 | 300% Source/Preview/Split, math/image and IME visual | Manual | screenshots + DPI/IME receipt | NOT TESTED |
| UX10-08 | Ctrl+wheel real mouse/trackpad behavior and ordinary wheel scroll | Manual | current-candidate device receipt | NOT TESTED |
| UX10-09 | 220×120 Source usability | Manual | screenshot/input/controls receipt | NOT TESTED |
| UX10-10 | 220×120 Preview usability | Manual | screenshot/scroll/selection receipt | NOT TESTED |
| UX10-11 | 220×120 Split remains Split | Manual | screenshot/input/scroll receipt | NOT TESTED |
| UX10-12 | taskbar item absent | Manual | Explorer shell observation receipt | NOT TESTED |
| UX10-13 | Alt+Tab/Win+Tab item absent | Manual | Windows switcher observation receipt | NOT TESTED |
| UX10-14 | Alt+Tab from focused StickyMD switches to another app | Manual | focused-switch sequence receipt | NOT TESTED |
| UX10-15 | tray restores hidden Tool Window | Manual | tray interaction receipt | NOT TESTED |
| UX10-16 | edge sensor restores collapsed Tool Window without focus theft | Manual | pointer/foreground receipt | NOT TESTED |
| UX10-17 | same-directory second instance restores hidden/collapsed Tool Window | Manual | copied-candidate process receipt | NOT TESTED |
| UX10-18 | 24 DIP capture feel at real DPI | Manual | three-edge pointer receipt | NOT TESTED |
| UX10-19 | nearest eligible edge wins outside tie | Manual | corner non-tie receipt | NOT TESTED |
| UX10-20 | Top wins <=1 DIP tie | Manual | corner tie receipt | NOT TESTED |
| UX10-21 | Left wins Right only in <=1 DIP tie | Manual | synthetic/manual topology receipt | NOT TESTED |
| UX10-22 | Bottom never docks or blocks a valid side edge | Manual | bottom/corner receipt | NOT TESTED |
| UX10-23 | 40% opacity is visually uniform, clickable, focusable and IME-capable | Manual | whole-window/IME receipt | NOT TESTED |

## Inherited Manual Gates

All Phase 9 manual rows remain `NOT TESTED` unless a later current-Phase-10-candidate receipt updates
the owning matrix. This Phase does not infer or synthesize manual success from HWND style readback,
unit tests, screenshots without an interaction protocol, or prior candidate evidence.
