# Phase 8 Windows Desktop Shell Report

## Executive Result

| Capability | Result | Evidence boundary |
| --- | --- | --- |
| Desktop shell / state authority | **PASS (automated)** | pure reducer plus copied-Release HWND smoke |
| Docking / auto-hide | **CONDITIONAL** | all geometry/timer logic and one real left-edge lifecycle PASS; real three-edge/DPI visual matrix NOT TESTED |
| Tray lifecycle | **CONDITIONAL** | exact three-command model, close-to-tray, wake and leak cycles PASS; Explorer visual/menu interaction NOT TESTED |
| Theme / opacity / topmost | **CONDITIONAL** | reducer, config and native HWND facts PASS; whole-window visual quality and live Windows theme switch NOT TESTED |
| Multi-monitor / DPI | **CONDITIONAL** | signed synthetic topology and live single-monitor adapter facts PASS; physical mixed-DPI/disconnect/RDP NOT TESTED |
| Persistence integration | **PASS (automated)** | revisioned config, dirty-hide save barrier and ordered Quit barriers |
| Memory / idle CPU | **PASS (automated)** | five copied-Release runs; hidden max 17.95 MiB PWS after the stress cycle, all idle CPU modes below 0.1% |
| Cold startup | **CONDITIONAL** | median 393.911 ms / max 1429.905 ms exceeds the 300 ms hard gate |

The automated Phase 8 scope is implemented without an architectural stop. Cold-start hardening and all
real desktop/IME/visual gates remain explicit conditions for Phase 9.

## Preconditions

```text
Phase 7 recommendation: APPROVE Phase 8 WITH CONDITIONS
USER approval: explicit Phase 8 prompt supplied
starting commit: a7af3a40fa357edd36cd1ae231e1c936f1b763bd
inherited conditions: real IME, Preview/math/image visual-DPI, clipboard producers,
                      native export dialog, crash timing and reparse-point receipts remain open
```

## Repository State Before Work

```text
branch: main
working tree: clean
```

## Environment

```text
OS: Windows 11 Home Chinese 10.0.26200 build 26200
CPU: Intel Core i7-12700H, 20 logical processors
RAM: 16,962,281,472 bytes
disk: ZHITAI TiPlus7100 NVMe, NTFS
Defender: real-time/antivirus/AM service reported disabled
Rust/Cargo: 1.97.1
```

## Desktop Shell

| Item | Implementation |
| --- | --- |
| undecorated | winit decorations disabled |
| drag | one shared toolbar/paper drag region; native drag-loop completion becomes a typed fact |
| resize | native eight-edge/corner resize hit test; minimum 360×240 DIP |
| shadow | winit Windows undecorated shadow enabled |
| corners | Windows small-round corner preference |
| control bar | fixed 34-DIP paint/hit-test model shared by Source/Split/Preview |

No Mica, Acrylic, WebView, settings page, background image or permanent animation loop was introduced.

## Window State Model

`WindowShellState` is the only runtime authority for:

```text
Floating
DockedExpanded(Left | Right | Top)
DockedCollapsed(Left | Right | Top)
HiddenToTray
Animating { from, to, deadline, final state }
LifecycleState::Running | Quitting(QuitStage)
```

The reducer consumes typed intents, monotonic milliseconds and immutable platform facts. It emits typed
effects; it contains no winit, Win32, filesystem or `DocumentState` access.

## Tray

```text
crate/version: tray-icon 0.24.2, default features disabled
menu: Show/Hide, Topmost, Quit — exactly three commands
event delivery: blocking/event-driven dispatcher; no polling
Close: dirty => freeze input, save latest generation, then hide; clean => hide
Quit: paste/assets settle -> latest note save -> safe asset GC -> exact config revision -> exit
```

The dispatcher is joinable and tears down callback ownership before releasing its active-instance guard.
If tray construction fails, Close uses the same safe quit barrier rather than hiding an unreachable window.

## Theme

| Mode | Behavior |
| --- | --- |
| Light | first-run default and fixed light palette |
| System | keeps configured mode as System; consumes runtime winit theme facts |
| Dark | fixed dark palette |

Theme changes relayout/repaint the projection but cannot mutate `DocumentState` or trigger Markdown
reparse. Real Windows live-theme visual acceptance remains `NOT TESTED`.

## Opacity

| Contract | Result |
| --- | --- |
| range | 70–100 inclusive |
| preview | drag updates native alpha without config write |
| commit | release / Enter / input focus loss produces one revisioned config update |
| clamp | invalid numeric endpoints clamp only at commit |
| 70% | native smoke observed alpha 179 |
| default 96% | native smoke observed alpha 245 |
| 100% | removes only `WS_EX_LAYERED`; preserves unrelated extended-style bits |

Whole-window visual coverage of background, text, formula, image, controls and shadow remains manual.

## Always On Top

`RuntimeConfig.always_on_top` is the configured state. A separate non-durable temporary sensor-topmost
flag keeps a 3-DIP collapsed strip reachable when configured topmost is false. Neither flag can mutate
the other. The toolbar and tray issue the same typed preference update; `SetWindowPos` uses
`SWP_NOACTIVATE`.

## Dock Geometry

```text
snap threshold: <= 12 DIP
undock threshold: inward distance > 16 DIP
visible sensor: 3 DIP, scaled with max(1 px, round(DIP * scale))
edges: left / right / top only
offset: normalized ratio along the dock edge
work area: signed physical rcWork; taskbar excluded
```

The geometry solver is O(1), allocation-free for transitions, and clamps every recovered frame fully
inside a selected work area. A deterministic 1000-case property test covers negative coordinates,
mixed scale, missing monitors and oversized placements.

## Auto Hide

| Trigger | Delay / behavior |
| --- | --- |
| sensor hover | 100 ms then expand without requesting focus |
| focus loss | 700 ms then collapse if no guard blocks it |
| unfocused hover leave | 500 ms then collapse |
| manual button | immediate transition |
| Esc | immediate only for a docked window; floating remains floating |
| animation | 140 ms cubic ease-out, exact endpoints, no overshoot |

Focused, IME-composing, popup, conflict, recovery and active drag/resize facts cancel automatic
collapse. Cancelled deadlines cannot commit from later stale ticks.

## Focus / Input Safety

```text
collapsed typing: rejected
hidden typing: rejected
unfocused hover-reveal typing: rejected until focus is acquired
IME preedit: cancelled before manual collapse/hide; never becomes canonical text
```

The shell only gates typed intents. `DocumentState` remains the mutation authority, and hidden/cache
effects cannot synthesize text deltas.

Theme, opacity, topmost and native drag/resize actions also cross a typed instruction boundary before
coordination or platform execution. Pointer callbacks no longer mutate durable preferences or invoke
window movement capabilities directly.

## Multi Monitor

| Item | Automated evidence | Manual boundary |
| --- | --- | --- |
| identity source | CCD target device-path SHA-256; GDI-name fallback is case-stable | physical device persistence NOT TESTED |
| fallback | missing identity selects primary then first monitor | live disconnect NOT TESTED |
| negative coordinates | signed synthetic work areas and `rcWork` test | real negative topology NOT TESTED |
| work area | `GetMonitorInfoW(MONITORINFO.rcWork)` | taskbar arrangements NOT TESTED |
| disconnect/resume | event path maps to one topology recovery | real disconnect/sleep/RDP NOT TESTED |
| primary recovery | size/edge preserved and frame fully visible | physical primary switch NOT TESTED |

`HMONITOR` is never persisted and never enters the pure state model.

## DPI

100%, 125%, 150% and 200% scaling are automated in pure geometry and Preview/layout tests. Manifest
inspection proves PerMonitorV2 and asInvoker are embedded. Physical mixed-DPI dragging, the real IME
candidate rectangle and formula/image visual sharpness remain `NOT TESTED`.

## Persistence

The revisioned configuration persists only stable logical state:

```text
window width/height in DIP
monitor identity
dock edge and offset ratio
floating x/y ratios
theme
committed opacity
configured topmost
view mode
```

Collapsed off-screen coordinates, animation frames, hover state, system-theme resolution and temporary
sensor topmost are not durable.

## Config Write Behavior

`ConfigCoordinator` is the single mutable preference authority. It uses a monotonic `ConfigRevision`,
at most one in-flight write and one coalesced latest pending snapshot. Equal updates are no-ops; stale or
failed acknowledgements cannot clear dirty state. Resize commits only at a completed native move loop;
animation and opacity preview produce zero writes.

## Acceptance

| Acceptance | Automated result | Manual boundary |
| --- | --- | --- |
| AC-019 Left Dock | PASS: reducer + copied-Release left lifecycle | real visual/DPI NOT TESTED |
| AC-020 Right Dock | PASS: geometry/reducer | real visual/DPI NOT TESTED |
| AC-021 Top Dock | PASS: geometry/reducer | real visual/DPI NOT TESTED |
| AC-022 focus/IME guard | PASS: synthetic guards and input authority | Microsoft Pinyin/WeChat NOT TESTED |
| AC-023 tray lifecycle | PASS: model, copied runtime, 100 wake cycles | Explorer menu visual NOT TESTED |
| AC-024 opacity | PASS: reducer/config/native HWND facts | whole-window visual NOT TESTED |
| AC-025 theme | PASS: mode/config/runtime event path | real live-system visual NOT TESTED |
| AC-026 same directory | PASS: hidden primary wake, secondary exit, no durable write | foreground restrictions manual NOT TESTED |
| AC-027 different directories | PASS: distinct process/window/note/config | two tray icons manual NOT TESTED |
| AC-028 topology recovery | PASS: deterministic synthetic topology | live disconnect/sleep/RDP NOT TESTED |
| AC-029 mixed DPI | PASS: pure scaling/cache invalidation | physical mixed-DPI visual/IME NOT TESTED |

## Inherited Manual Verification

```text
Microsoft Pinyin final shell: NOT TESTED
WeChat IME final shell: NOT TESTED
Light/Dark/System visual: NOT TESTED
whole-window opacity visual: NOT TESTED
Preview/math/image visual: NOT TESTED
Left/Right/Top Dock visual: NOT TESTED
3-DIP sensor and hover no-focus visual: NOT TESTED
Explorer tray visual/menu: NOT TESTED
native Export dialog: NOT TESTED
real 125/150/200% DPI: NOT TESTED
real dual/mixed-DPI monitor: NOT TESTED
real disconnect/negative topology/sleep/RDP: NOT TESTED
```

## Performance

Release tests; each algorithm row executes 100,000 operations.

| Operation | Median | p95 | Max |
| --- | ---: | ---: | ---: |
| window reducer transitions | 2.1057 ms | 2.5933 ms | 2.7239 ms |
| geometry/topology recovery | 72.9 µs | 94.3 µs | 149.6 µs |
| control layout + hit test | 652.4 µs | 783.9 µs | 948.7 µs |

These are O(1) per-operation algorithms and do not justify a more complex spatial/indexing structure.

Caret blinking uses a caret-free static source-frame cache plus a bounded caret rectangle overlay.
The cache key includes document generation, selection, preedit, diagnostics, theme, viewport, DPI and
scroll state; blink visibility is intentionally excluded. The blink deadline is armed only while the
window state accepts source mutations, so a focused-but-collapsed 3-DIP sensor does not wake or paint
the editor. This removes full-document glyph shape/raster work from the 550 ms idle blink path without
creating another text authority.

## Memory

Five independent copied-Release runs, 30-second warm-up per state.

| State | PWS median / max | Private Bytes median / max | Peak WS / Peak Private max |
| --- | ---: | ---: | ---: |
| visible Source | 13.07 / 13.17 MiB | 15.20 / 15.28 MiB | 38.10 / 30.67 MiB |
| docked collapsed | 13.07 / 13.20 MiB | 15.17 / 15.25 MiB | 38.37 / 30.67 MiB |
| hidden to tray | 11.75 / 17.95 MiB | 13.86 / 20.32 MiB | 44.95 / 30.67 MiB |

Phase 7's comparable Source/no-image median was 7.49 MiB PWS and 8.36 MiB Private Bytes. Phase 8
therefore adds approximately 5.58 MiB PWS and 6.84 MiB Private Bytes, within the exploratory +8 MiB
persistence/shell delta gate. Different reports use different process states; the delta is diagnostic,
not a universal allocator attribution. The hidden maximum is the first-run sample immediately after
the explicit 1000-animation/100-tray stress sequence; the other four fresh hidden samples remain near
11.7 MiB PWS.

## Idle CPU

| State | 60-second normalized average |
| --- | ---: |
| visible Source | 0.022135% |
| docked collapsed | 0.001302% |
| hidden to tray | 0.000000% |

All are below the 0.1% hard gate. During 1000 deliberately continuous animation cycles, single-core
CPU averaged 47.414%; this is active animation cost, not idle behavior. An earlier same-code-path
attempt measured 0.316405% while collapsed and exposed that native focus alone kept the caret timer
armed. Binding blink eligibility to `WindowShellState::accepts_editor_mutation()` fixed the root cause;
the final five-run resource route above is the post-fix receipt.

## Startup

```text
samples:         1429.905 / 325.825 / 435.860 / 393.911 / 381.413 ms
five-run median: 393.911 ms
five-run max:    1429.905 ms
hard gate:       300 ms cold start
result:          CONDITIONAL / hard gate not met
```

See [`RISK-source-font-startup.md`](RISK-source-font-startup.md). No risky fallback-font shortcut was
introduced to conceal the result.

## Binary Size

The final Release executable is 8,267,776 bytes (7.89 MiB), below the 30 MiB portable-EXE budget.

## Dependencies Added

| Crate | Version | License | Purpose | Runtime implication |
| --- | --- | --- | --- | --- |
| `tray-icon` | 0.24.2 | MIT OR Apache-2.0 | native tray icon | one native icon and event dispatcher |
| `crossbeam-channel` | 0.5.16 | MIT OR Apache-2.0 | blocking tray hand-off | no polling / no async runtime |
| `muda` | 0.19.3 transitive | Apache-2.0 OR MIT | native three-item menu | Windows menu objects only |

Full audit: [`phase-08-dependency-delta.md`](phase-08-dependency-delta.md).

## Windows APIs Added

`QueryDisplayConfig`, `DisplayConfigGetDeviceInfo`, `GetMonitorInfoW`, `Get/SetWindowLongPtrW`,
`SetLayeredWindowAttributes`, `SetWindowPos`, winit's Windows message hook and the native tray APIs.
See [`phase-08-windows-api-delta.md`](phase-08-windows-api-delta.md).

## Unsafe

```text
stickymd-core runtime unsafe = 0
stickymd-render runtime unsafe = 0
Phase 8 Windows shell unsafe = confined to monitor.rs, window_opacity.rs and window_topmost.rs
```

Every handwritten block has an adjacent `SAFETY:` invariant. `tray.rs` contains no handwritten unsafe.
Phase 14 later added a bounded `native_message.rs` repair for winit's malformed non-client drag
payload; see [`phase-08-windows-api-delta.md`](phase-08-windows-api-delta.md#phase-14-native-drag-addendum).

## Architecture Authority

```text
Canonical text owner: DocumentState
Window/dock/lifecycle owner: WindowShellState
Committed preference owner: ConfigCoordinator / RuntimeConfig
Durable configuration: config.toml projection
Platform facts: winit/Win32/tray adapters
Save source: immutable DocumentSnapshot only
```

The shell does not read files, mutate canonical text, interpret watcher events or retain raw Windows
handles in core state.

## Resource / Leak Testing

The copied Release route completed 1000 collapse/reveal cycles, 100 hide/wake cycles, 100 topmost
toggles, 102 theme transitions and 100 opacity commits. Private Bytes changed from 15,785,984 to
22,654,976 bytes (+6.55 MiB retained capacity). Observable objects changed from 372/19/26 to
374/19/27 (HANDLE/GDI/USER), below the frozen leak thresholds; the four subsequent fresh processes
returned to the normal 13.8 MiB hidden Private-Bytes band, so the receipt does not show linear
cross-process or per-cycle growth.

## Architecture Drift

No approved authority or platform boundary drift was found. One open performance risk is recorded in
[`RISK-source-font-startup.md`](RISK-source-font-startup.md).

## Verification

Fresh receipts on the final product code:

| Command / audit | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS — 335 passed, 0 failed, 12 ignored Release-only baselines |
| `cargo build --workspace --release --locked` | PASS |
| `cargo test -p stickymd-core --release --locked` | PASS — 52 passed, 1 ignored |
| `cargo test -p stickymd-render --release --locked` | PASS — 90 passed, 4 ignored |
| `cargo test -p stickymd-win --release --locked` | PASS — 178 passed, 7 ignored |
| `cargo deny check` | PASS — advisories/bans/licenses/sources; duplicate-version warnings remain transitive audit notes |
| `tools/smoke/all.ps1 -Ci` | PASS — 13 headless tasks; no desktop runtime/resource task |
| `tools/smoke/phase-08.ps1` | PASS — 55 passed, 2 ignored Release-only baselines |
| `tools/smoke/phase-08.ps1 -Performance` | PASS — 2 Release baselines |
| `tools/smoke/phase-08.ps1 -Runtime` | PASS — copied Release close/hide/wake lifecycle |
| `tools/smoke/phase-08.ps1 -Resources` | PASS — five process runs and repeated-cycle gate |
| dependency/source forbidden-architecture scan | PASS |
| core/render unsafe scan and Windows adapter audit | PASS — platform-independent runtime unsafe = 0 |
| Release manifest inspection | PASS — `PerMonitorV2`, `asInvoker` |
| Phase 08 PowerShell parser | PASS |
| manual PASS audit | PASS — no manual row is marked PASS |
| `git diff --check` | PASS |

## Documentation

```text
task: docs/tasks/phase-08-windows-desktop-shell.md
report: docs/report/phase-08-windows-desktop-shell.md
acceptance: docs/acceptance-cases/phase-08.md
coverage: docs/coverage-matrix.md
overview: docs/overview/architecture.md
plan: docs/plan/09_windows_shell.md
dependency delta: docs/report/phase-08-dependency-delta.md
Windows API delta: docs/report/phase-08-windows-api-delta.md
risk: docs/report/RISK-source-font-startup.md
README: README.md
```

## Git

```text
commits: one local cohesive Phase 8 commit; hash reported in the final handoff
push = no
```

## Recommendation

**APPROVE Phase 9 WITH CONDITIONS**

Conditions: keep every manual row `NOT TESTED` until a checked-in receipt exists, and close or obtain
USER disposition for the cold-start hard-gate risk before RC.

> Awaiting USER review. Do not start Phase 9 automatically.
