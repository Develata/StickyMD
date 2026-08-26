# Phase 8 Acceptance Matrix — Native Window, Dock, Tray, Theme and Opacity

This matrix is the frozen verification projection for the Phase 8 Windows shell work. It does not
promote checked-in plans, unexecuted runtime scenarios, synthetic monitor facts, or compile success
into real Windows interaction receipts.

The Rust CLI owns automation. [`phase-08.ps1`](../../tools/smoke/phase-08.ps1) is only a stable
PowerShell entry point. `all --ci` may run headless tests and deterministic Release baselines;
copied-executable runtime and resource measurements are explicit local modes. Every real tray,
monitor-topology, IME, and visual-quality row remains `NOT TESTED` until a checked-in receipt exists.

## Acceptance Task Graph

| ID | Contract / acceptance | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P08-A00 | Phase 8 smoke routing | Automated | Rust CLI P08 parsing, deduplicated headless/performance/runtime/resource plans and runner unit tests | AUTOMATED PASS |
| P08-A01 | AC-019..021 visibility and dock state machine | Automated | `phase8_` tests cover Floating, DockedExpanded, DockedCollapsed and Animating transitions | AUTOMATED PASS |
| P08-A02 | AC-019..022 exact timer and priority rules | Automated | `phase8_` virtual-clock tests cover 100/500/700/140 ms boundaries and event priority | AUTOMATED PASS |
| P08-A03 | AC-019..021 dock geometry | Automated | `phase8_` tests cover the current 24-DIP snap contract, greater-than-16-DIP undock, one-step transitions among all three dock edges, 3-DIP strip and left/right/top work-area geometry | AUTOMATED PASS |
| P08-A04 | AC-028/029 monitor recovery geometry | Automated | `phase8_` synthetic topology tests cover negative coordinates, primary fallback, mixed DPI, missing monitors and full visibility | AUTOMATED PASS |
| P08-A05 | AC-022 focus, IME, popup, conflict and recovery guards | Automated | `phase8_` reducer tests prove guards suppress auto-hide while Esc/manual collapse keeps priority | AUTOMATED PASS |
| P08-A06 | AC-023 lifecycle and tray model | Automated | `phase8_` tests prove Close hides, Show restores, the menu has exactly three commands, note-save failure cancels Quit, and GC/config failure follows the documented warning policy | AUTOMATED PASS |
| P08-A07 | AC-024 opacity reducer and adapter | Automated | `phase8_`/Phase 10 tests cover 40..100 clamp, integer-only commit, live preview, one durable commit and alpha conversion | AUTOMATED PASS |
| P08-A08 | AC-025 theme reducer and adapter | Automated | `phase8_` tests cover Light/System/Dark, default Light, runtime System changes and configuration round-trip | AUTOMATED PASS |
| P08-A09 | Always-on-top and controls projection | Automated | `phase8_` tests cover typed topmost effects and control fade/hit-test behavior without document mutation | AUTOMATED PASS |
| P08-A10 | AC-026 same-directory wake | Automated | `phase8_` coordinator tests plus copied-Release `-Runtime` prove hidden primary wake, secondary exit and no secondary durable write | AUTOMATED PASS |
| P08-A11 | AC-027 different-directory isolation | Automated | copied-Release runtime proves distinct processes, windows, notes and configs; tray-icon distinction remains manual | AUTOMATED PASS |
| P08-A12 | Animation and idle redraw discipline | Automated | `phase8_` tests prove monotone ease-out endpoints, interruption convergence and no post-animation redraw deadline; copied-Release resources cover the cached-caret idle path | AUTOMATED PASS |
| P08-A13 | DPI/config/platform adapter boundaries | Automated | `phase8_` tests and governance cover PerMonitorV2, validated placement values, minimal Win32 features and SAFETY text | AUTOMATED PASS |
| P08-A14 | Source/Preview/persistence/assets regressions | Automated | workspace tests plus `phase8_` lifecycle regressions preserve document authority, safe Quit, split width and DPI cache keys | AUTOMATED PASS |
| P08-A15 | Phase 8 Release performance | Automated | [`phase-08.ps1 -Performance`](../../tools/smoke/phase-08.ps1) runs ignored `phase8_` state/geometry/layout baselines with median/p95/max output | AUTOMATED PASS |
| P08-A16 | Phase 8 copied-Release lifecycle | Automated | [`phase-08.ps1 -Runtime`](../../tools/smoke/phase-08.ps1) uses physical pointer drag plus real shell focus loss for consecutive Left -> Top -> Left -> Right snap-auto-collapse-sensor-reveal without a Floating intermediate state, runs Right while Pin is ON, drives both top-corner priority cases, WM_CLOSE and same-directory wake; exact 700-ms guards and `Top > Left > Right` ties also have deterministic reducer coverage | AUTOMATED PASS |
| P08-A17 | Phase 8 hidden resource gate | Automated | [`phase-08.ps1 -Resources`](../../tools/smoke/phase-08.ps1) measures five hidden-to-tray runs after 30 s and selected 60 s idle CPU; hard gates are 36 MiB and 0.1% | AUTOMATED PASS |
| P08-A18 | CI headless ownership | Automated | `all --ci` includes P08 headless tests and Release baseline exactly once and excludes every P08 runtime/resource task | AUTOMATED PASS |
| P08-A19 | Manual evidence honesty | Automated | governance accepts manual PASS only with a checked-in `receipt:` and every row below is currently `NOT TESTED` | AUTOMATED PASS |
| P08-M01 | AC-019 left dock visual/timing | Manual | Real 100/125/150/200% DPI drag, 3-DIP strip, hover reveal, focus collapse and greater-than-16-DIP undock receipt required | NOT TESTED |
| P08-M02 | AC-020 right dock visual/timing | Manual | Real 100/125/150/200% DPI right-edge drag and timing receipt required | NOT TESTED |
| P08-M03 | AC-021 top dock visual/timing | Manual | Real top-edge horizontal strip, window-width sensor and timing receipt required | NOT TESTED |
| P08-M04 | AC-022 real IME/focus guard | Manual | Microsoft Pinyin and WeChat IME in Source/Split, three dock edges and 40/96/100 opacity receipt required | NOT TESTED |
| P08-M05 | AC-023 real tray lifecycle | Manual | Explorer tray icon and exact Show/Hide, Topmost, Quit menu interaction; failed-save Quit must keep the process alive | NOT TESTED |
| P08-M06 | AC-024 whole-window opacity visual | Manual | Background, text, formulas, images, controls and shadow at 40/70/96/100 with slider/input commit receipt required | NOT TESTED |
| P08-M07 | AC-025 real theme visual | Manual | Light/Dark/System screenshots and live Windows application-theme switch receipt required | NOT TESTED |
| P08-M08 | AC-026 same-directory user flow | Manual | Hidden/minimized/docked first instance must be visibly restored by a second launch under Windows foreground restrictions | NOT TESTED |
| P08-M09 | AC-027 distinct portable instances | Manual | Two copied directories must show independent windows, tray icons, note content and config behavior | NOT TESTED |
| P08-M10 | AC-028 live monitor topology | Manual | Left/right/up/down monitor arrangements, primary switch, disconnect/reconnect, sleep/resume and Remote Desktop receipt required | NOT TESTED |
| P08-M11 | AC-029 mixed-DPI and negative coordinates | Manual | Cross-monitor dock/undock plus real IME candidate, formula and image clarity at mixed DPI and negative coordinates | NOT TESTED |
| P08-M12 | Windows 11 shell visual contract | Manual | Supported Windows 11 builds, rounded corner, shadow, topmost, control fade and accessibility receipt required | NOT TESTED |
| P08-M13 | inherited Phase 3 IME gate | Manual | Microsoft Pinyin and WeChat IME end-to-end matrix remains open | NOT TESTED |
| P08-M14 | inherited Phase 5/6 Preview/math visual gate | Manual | Native Preview selection and RaTeX Light/Dark/DPI visual receipts remain open | NOT TESTED |
| P08-M15 | inherited Phase 7 image/export visual gate | Manual | Native image quality, clipboard producers, export dialog and crash reconciliation receipts remain open | NOT TESTED |

## Frozen Definition-of-Done Trace

These rows keep individual Phase 8 obligations visible. An automated-capable obligation would remain
`BLOCKED` until its named `phase8_` test or opt-in smoke had actually been checked in and executed;
environment-sensitive behavior is never relabelled automated merely because a synthetic model passes.

| ID | Frozen DoD obligation | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P08-D001 | USER approved Phase 8 | Automated | Phase 8 task/report precondition must record the approval | AUTOMATED PASS |
| P08-D002 | Phase 7 inherited conditions are recorded completely | Automated | M13-M15 preserve the open gates; task/report inheritance record is recorded | AUTOMATED PASS |
| P08-D003 | main window is formally undecorated | Automated | A13 shell-construction test PASS | AUTOMATED PASS |
| P08-D004 | paper shell is complete | Manual | real shell visual receipt required by M12 | NOT TESTED |
| P08-D005 | fixed rounded corner is complete | Manual | real Windows 11 visual receipt required by M12 | NOT TESTED |
| P08-D006 | fixed shadow is complete | Manual | real Windows 11 visual receipt required by M12 | NOT TESTED |
| P08-D007 | custom drag is complete | Manual | real pointer drag receipt required by M01-M03 | NOT TESTED |
| P08-D008 | custom resize is complete | Manual | real edge/corner resize receipt required by M12 | NOT TESTED |
| P08-D009 | native Snap does not contaminate logical placement | Automated | A03/A04 placement-state tests PASS | AUTOMATED PASS |
| P08-D010 | Source/Split/Preview controls are integrated | Manual | real shell mode-control receipt required by M12 | NOT TESTED |
| P08-D011 | no settings page is introduced | Automated | A13 source/surface audit PASS | AUTOMATED PASS |
| P08-D012 | Always-on-top button works | Manual | real z-order/button receipt required by M12 | NOT TESTED |
| P08-D013 | Always-on-top stays synchronized with tray | Manual | real tray/button synchronization receipt required by M05 | NOT TESTED |
| P08-D014 | theme control has exactly Light/System/Dark | Automated | A08 enum/reducer tests PASS | AUTOMATED PASS |
| P08-D015 | first-run theme is Light | Automated | A08 default-config test PASS | AUTOMATED PASS |
| P08-D016 | System mode responds to runtime theme changes | Manual | synthetic event is A08; real Windows change remains M07 | NOT TESTED |
| P08-D017 | theme changes do not change Document generation | Automated | A08/A14 authority tests PASS | AUTOMATED PASS |
| P08-D018 | theme changes do not reparse Markdown | Automated | A08/A14 effect-count tests PASS | AUTOMATED PASS |
| P08-D019 | opacity range is 40..100 | Automated | A07 reducer tests PASS | AUTOMATED PASS |
| P08-D020 | opacity applies to the whole window | Manual | real whole-window receipt required by M06 | NOT TESTED |
| P08-D021 | opacity slider previews live | Manual | reducer is A07; real slider receipt required by M06 | NOT TESTED |
| P08-D022 | opacity has an integer numeric input | Manual | real input receipt required by M06 | NOT TESTED |
| P08-D023 | opacity clamps to 40..100 | Automated | A07 boundary tests PASS | AUTOMATED PASS |
| P08-D024 | opacity writes config only on release, Enter or focus loss | Automated | A07 effect-count tests PASS | AUTOMATED PASS |
| P08-D025 | opacity 100 removes unnecessary layered style | Automated | A07 adapter-style tests PASS | AUTOMATED PASS |
| P08-D026 | opacity changes do not mutate Document | Automated | A07/A14 authority tests PASS | AUTOMATED PASS |
| P08-D027 | tray icon is created | Manual | real Explorer tray receipt required by M05 | NOT TESTED |
| P08-D028 | tray has only Show/Hide, Topmost and Quit | Manual | exact real menu receipt required by M05 | NOT TESTED |
| P08-D029 | tray events do not use polling | Automated | A06/A13 scheduler/adapter tests PASS | AUTOMATED PASS |
| P08-D030 | close button hides to tray | Automated | A06 reducer and A16 copied-Release smoke PASS | AUTOMATED PASS |
| P08-D031 | Alt+F4 hides to tray | Manual | real keyboard/system-command receipt required by M05 | NOT TESTED |
| P08-D032 | hiding a dirty document saves safely first | Automated | A06/A14 ordering tests PASS | AUTOMATED PASS |
| P08-D033 | hide-save failure keeps the window visible | Automated | A06 failure-state tests PASS | AUTOMATED PASS |
| P08-D034 | hidden state purges caches | Automated | A06 cache-effect test and A17 process measurement PASS | AUTOMATED PASS |
| P08-D035 | Tray Show restores expanded state | Automated | A06 reducer and A16 copied-Release smoke PASS | AUTOMATED PASS |
| P08-D036 | Tray Quit is the only normal user exit | Automated | A06 lifecycle-model test PASS | AUTOMATED PASS |
| P08-D037 | Tray Quit orders save, GC and config correctly | Automated | A06/A14 ordering regressions PASS | AUTOMATED PASS |
| P08-D038 | same-directory launch wakes a hidden first instance | Automated | A10/A16 copied-Release lifecycle PASS | AUTOMATED PASS |
| P08-D039 | left dock works | Manual | real receipt required by M01 | NOT TESTED |
| P08-D040 | right dock works | Manual | real receipt required by M02 | NOT TESTED |
| P08-D041 | top dock works | Manual | real receipt required by M03 | NOT TESTED |
| P08-D042 | bottom dock is absent | Automated | A03 edge-enum tests PASS | AUTOMATED PASS |
| P08-D043 | snap threshold is 24 DIP | Automated | A03 exact-boundary tests PASS | AUTOMATED PASS |
| P08-D044 | detach threshold is greater than 16 DIP | Automated | A03 exact-boundary tests PASS | AUTOMATED PASS |
| P08-D045 | collapsed sensor is 3 DIP | Automated | A03 DPI geometry tests PASS | AUTOMATED PASS |
| P08-D046 | top sensor retains window width | Automated | A03 geometry tests PASS | AUTOMATED PASS |
| P08-D047 | left/right sensor retains window height | Automated | A03 geometry tests PASS | AUTOMATED PASS |
| P08-D048 | primary architecture uses one window as sensor | Automated | A06/A13 architecture tests PASS | AUTOMATED PASS |
| P08-D049 | sensor hover waits 100 ms | Automated | A02 virtual-clock test PASS | AUTOMATED PASS |
| P08-D050 | hover reveal does not take focus | Automated | A05 reducer/platform-effect test PASS | AUTOMATED PASS |
| P08-D051 | hover reveal does not steal foreground | Manual | real foreground behavior receipt required by M01-M03 | NOT TESTED |
| P08-D052 | hover leave waits 500 ms | Automated | A02 virtual-clock test PASS | AUTOMATED PASS |
| P08-D053 | focus loss waits 700 ms | Automated | A02 virtual-clock test PASS | AUTOMATED PASS |
| P08-D054 | focused window never auto-collapses | Automated | A05 guard test PASS | AUTOMATED PASS |
| P08-D055 | IME composition never auto-collapses | Manual | synthetic guard is A05; real IMEs remain M04 | NOT TESTED |
| P08-D056 | dragging/resizing never auto-collapses | Automated | A05 guard test PASS | AUTOMATED PASS |
| P08-D057 | popup interaction never auto-collapses | Automated | A05 guard test PASS | AUTOMATED PASS |
| P08-D058 | manual collapse has no delay | Automated | A02/A05 priority test PASS | AUTOMATED PASS |
| P08-D059 | Esc collapses a docked window | Automated | A05 transition test PASS | AUTOMATED PASS |
| P08-D060 | Esc does not edge-collapse a floating window | Automated | A05 transition test PASS | AUTOMATED PASS |
| P08-D061 | collapse/reveal animation lasts 140 ms | Automated | A02/A12 timing tests PASS | AUTOMATED PASS |
| P08-D062 | animation does not create a permanent 60 FPS loop | Automated | A12 scheduler test PASS | AUTOMATED PASS |
| P08-D063 | animation reuses the existing scheduler | Automated | A12 event-loop integration test PASS | AUTOMATED PASS |
| P08-D064 | stale timer tokens cannot commit | Automated | A02 token/race tests PASS | AUTOMATED PASS |
| P08-D065 | programmatic moves do not persist placement | Automated | A03/A04 effect tests PASS | AUTOMATED PASS |
| P08-D066 | collapsed physical rect is not persisted | Automated | A03/A04 config projection tests PASS | AUTOMATED PASS |
| P08-D067 | temporary sensor topmost is separate from configured topmost | Automated | A06/A09 state tests PASS | AUTOMATED PASS |
| P08-D068 | collapsed sensor remains hoverable when configured topmost is false | Manual | real covered-window hover receipt required by M01-M03 | NOT TESTED |
| P08-D069 | collapsed, hidden and unfocused-hover states cannot edit Document | Automated | A05/A14 input-authority tests PASS | AUTOMATED PASS |
| P08-D070 | clicking a hover-revealed window restores focus and editor input | Manual | real mouse/focus/typing receipt required by M01-M04 | NOT TESTED |
| P08-D071 | monitor identity does not persist HMONITOR | Automated | A04/A13 type/boundary tests PASS | AUTOMATED PASS |
| P08-D072 | CCD stable identity is implemented | Automated | A04 monitor-identity tests PASS | AUTOMATED PASS |
| P08-D073 | QueryDisplayConfig runs only when necessary | Automated | A04 adapter-call-count test PASS | AUTOMATED PASS |
| P08-D074 | monitor work area uses rcWork | Automated | A04 adapter/geometry test PASS | AUTOMATED PASS |
| P08-D075 | negative monitor coordinates are supported | Automated | A04 synthetic topology tests PASS | AUTOMATED PASS |
| P08-D076 | floating relative position is supported | Automated | A04 round-trip tests PASS | AUTOMATED PASS |
| P08-D077 | dock relative offset is supported | Automated | A04 round-trip tests PASS | AUTOMATED PASS |
| P08-D078 | size is persisted in DIP | Automated | A04 config tests PASS | AUTOMATED PASS |
| P08-D079 | 100% DPI geometry is correct | Automated | A03 geometry tests PASS | AUTOMATED PASS |
| P08-D080 | 125% DPI geometry is correct | Automated | A03 geometry tests PASS | AUTOMATED PASS |
| P08-D081 | 150% DPI geometry is correct | Automated | A03 geometry tests PASS | AUTOMATED PASS |
| P08-D082 | 200% DPI geometry is correct | Automated | A03 geometry tests PASS | AUTOMATED PASS |
| P08-D083 | missing monitor at startup falls back to primary | Automated | A04 recovery tests PASS | AUTOMATED PASS |
| P08-D084 | runtime monitor disconnect falls back to primary | Manual | synthetic recovery is A04; real disconnect remains M10 | NOT TESTED |
| P08-D085 | dock recovery preserves the same edge | Automated | A04 recovery tests PASS | AUTOMATED PASS |
| P08-D086 | visible disconnect recovery restores expanded state | Automated | A04 recovery-state tests PASS | AUTOMATED PASS |
| P08-D087 | hidden disconnect recovers correctly on next Show | Automated | A04/A06 recovery-state tests PASS | AUTOMATED PASS |
| P08-D088 | DPI change does not mutate Document | Automated | A04/A14 authority test PASS | AUTOMATED PASS |
| P08-D089 | DPI change updates IME rectangle | Manual | coordinate projection is A04; real candidate receipt remains M11 | NOT TESTED |
| P08-D090 | DPI change refreshes required math/image rasters | Automated | A14 cache-key/invalidation tests PASS | AUTOMATED PASS |
| P08-D091 | display-topology change uses events, not polling | Automated | A04/A13 scheduler/adapter tests PASS | AUTOMATED PASS |
| P08-D092 | config persistence has no write flood | Automated | A07-A09 coalescing/effect-count tests PASS | AUTOMATED PASS |
| P08-D093 | shell memory is measured | Automated | Phase 8 resource matrix extension PASS | AUTOMATED PASS |
| P08-D094 | hidden memory is measured | Automated | A17 five-run hidden-to-tray route PASS | AUTOMATED PASS |
| P08-D095 | collapsed memory is measured | Automated | Phase 8 resource matrix extension PASS | AUTOMATED PASS |
| P08-D096 | tray memory delta is measured | Automated | Phase 8 before/after process matrix PASS | AUTOMATED PASS |
| P08-D097 | startup timing is measured | Automated | Phase 8 copied-Release timing route PASS | AUTOMATED PASS |
| P08-D098 | idle CPU is measured in each state | Automated | Phase 8 state resource matrix PASS | AUTOMATED PASS |
| P08-D099 | animation CPU is measured | Automated | Phase 8 animation resource route PASS | AUTOMATED PASS |
| P08-D100 | 1000 dock cycles show no obvious leak | Automated | repeated-cycle resource route PASS | AUTOMATED PASS |
| P08-D101 | 100 tray cycles show no obvious leak | Automated | repeated-cycle resource route PASS | AUTOMATED PASS |
| P08-D102 | dependency delta is complete | Automated | Phase 8 dependency report and governance PASS | AUTOMATED PASS |
| P08-D103 | Windows API delta is complete | Automated | Phase 8 API report and adapter audit PASS | AUTOMATED PASS |
| P08-D104 | core unsafe count remains zero | Automated | final workspace governance PASS | AUTOMATED PASS |
| P08-D105 | render unsafe count remains zero | Automated | final workspace governance PASS | AUTOMATED PASS |
| P08-D106 | no WebView is introduced | Automated | final dependency/source governance PASS | AUTOMATED PASS |
| P08-D107 | no Tauri runtime is introduced | Automated | final dependency/source governance PASS | AUTOMATED PASS |
| P08-D108 | no Tokio is introduced | Automated | final dependency/source governance PASS | AUTOMATED PASS |
| P08-D109 | no network client is introduced | Automated | final dependency/source governance PASS | AUTOMATED PASS |
| P08-D110 | no Mica or Acrylic is introduced | Automated | source governance PASS; real visual remains M12 | AUTOMATED PASS |
| P08-D111 | no automatic startup is introduced | Automated | source/surface governance PASS | AUTOMATED PASS |
| P08-D112 | documentation is updated | Automated | Phase 8 documentation set complete | AUTOMATED PASS |
| P08-D113 | coverage matrix is updated | Automated | coverage projection complete | AUTOMATED PASS |
| P08-D114 | Phase 8 task and report are complete | Automated | task/report complete | AUTOMATED PASS |
| P08-D115 | CI and all Phase 8 smoke paths pass | Automated | headless/product/runtime/resource receipts remain PASS | AUTOMATED PASS |
| P08-D116 | Phase 9 was not started automatically | Automated | final repository scope audit is recorded | AUTOMATED PASS |
| P08-D117 | Microsoft Pinyin final shell | Manual | real receipt required by M04/M13 | NOT TESTED |
| P08-D118 | WeChat IME final shell | Manual | real receipt required by M04/M13 | NOT TESTED |
| P08-D119 | Light real visual | Manual | real receipt required by M07 | NOT TESTED |
| P08-D120 | Dark real visual | Manual | real receipt required by M07 | NOT TESTED |
| P08-D121 | System theme live change | Manual | real receipt required by M07 | NOT TESTED |
| P08-D122 | whole-window opacity visual | Manual | real receipt required by M06 | NOT TESTED |
| P08-D123 | math visual | Manual | inherited real receipt required by M14 | NOT TESTED |
| P08-D124 | image visual | Manual | inherited real receipt required by M15 | NOT TESTED |
| P08-D125 | Source/Preview/Split visual | Manual | real receipt required by M12 | NOT TESTED |
| P08-D126 | Left Dock visual | Manual | real receipt required by M01 | NOT TESTED |
| P08-D127 | Right Dock visual | Manual | real receipt required by M02 | NOT TESTED |
| P08-D128 | Top Dock visual | Manual | real receipt required by M03 | NOT TESTED |
| P08-D129 | 3-DIP sensor visual | Manual | real receipt required by M01-M03 | NOT TESTED |
| P08-D130 | hover no-focus visual | Manual | real receipt required by M01-M03 | NOT TESTED |
| P08-D131 | tray visual and menu | Manual | real receipt required by M05 | NOT TESTED |
| P08-D132 | native Export dialog | Manual | inherited real receipt required by M15 | NOT TESTED |
| P08-D133 | real 125/150/200% DPI | Manual | real receipt required by M01-M03/M11 | NOT TESTED |
| P08-D134 | real dual monitor | Manual | real receipt required by M10 | NOT TESTED |
| P08-D135 | real mixed-DPI monitor | Manual | real receipt required by M11 | NOT TESTED |
| P08-D136 | real monitor disconnect | Manual | real receipt required by M10 | NOT TESTED |
| P08-D137 | real negative monitor geometry when available | Manual | real receipt required by M10/M11 | NOT TESTED |
| P08-D138 | sleep/resume when available | Manual | real receipt required by M10 | NOT TESTED |
| P08-D139 | RDP reconnect when available | Manual | real receipt required by M10 | NOT TESTED |

## Current Phase Gate

Phase 8 automated acceptance is **PASS** after the checked-in headless, Release-performance,
copied-Release window lifecycle and resource routes complete. `-Runtime` and `-Resources` remain
deliberately opt-in because they manipulate real windows and collect machine-specific process counters.
M01-M15 remain `NOT TESTED`; no synthetic or copied-Release smoke may close those manual gates.
