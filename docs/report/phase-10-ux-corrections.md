# Phase 10 UX Corrections

## Executive Result

The ten USER-approved post-freeze interaction corrections are implemented without adding a second
document, preview, asset, window or configuration authority. Automated semantic and copied-runtime
checks pass. Real keyboard, IME, visual, Alt+Tab/taskbar, tray, pointer and DPI observations remain
`NOT TESTED` in [`phase-10.md`](../acceptance-cases/phase-10.md).

## Contract Ledger

| Correction | Implementation result | Authority / algorithm |
| --- | --- | --- |
| traditional clipboard shortcuts | IMPLEMENTED | aliases emit the existing typed copy/cut/paste intents |
| global content zoom | IMPLEMENTED | constrained integer `ContentZoomPercent` in `ConfigCoordinator` |
| zoom input | IMPLEMENTED | key steps 10%; wheel accumulator emits 5% per complete notch |
| 220×120 DIP minimum | IMPLEMENTED | fixed shell minimum; default remains 520×680 |
| Tool Window identity | IMPLEMENTED | tray-gated Win32 style adapter with readback verification |
| 24 DIP capture | IMPLEMENTED | pure monitor-local DIP geometry |
| nearest edge | IMPLEMENTED | minimum of Top/Left/Right; Bottom has no variant |
| one-DIP tie | IMPLEMENTED | priority Top > Left > Right only inside epsilon |
| expanded dock release | IMPLEMENTED | reducer enters `DockedExpanded`; focus guard suppresses collapse |
| opacity 40–100 | IMPLEMENTED | validated config/control value and whole-window alpha adapter |

## Clipboard Shortcuts

`Ctrl+Insert`, `Shift+Delete` and `Shift+Insert` translate to the same `CopySelection`,
`CutSelection` and `PasteClipboard` intents as Ctrl+C/X/V. Paste therefore retains the single
clipboard priority and managed-asset transaction path. Preview accepts Copy only; Cut and Paste do
not mutate canonical Markdown. Existing clipboard-write failure tests prove that a failed Cut copy
cannot remove canonical text.

## Content Zoom

- durable range: 50..=300%; default 100%; old config without the field loads as 100%;
- keyboard: Ctrl+= / Ctrl+Numpad+ adds 10, Ctrl+- / Ctrl+Numpad- subtracts 10, Ctrl+0 resets;
- wheel: line or physical-pixel deltas accumulate; one 120-unit notch changes 5%;
- wheel persistence: live layout followed by a 250 ms config-save deadline; no per-event write;
- effective document scale: `DPI × content zoom`; toolbar, docking and sensor geometry use DPI only;
- Source: relayouts cosmic-text, caret, selection, hit testing and IME area;
- Preview/Split: relayout from retained semantic tree; Markdown and math parsing are reused;
- image raster admission remains viewport-, pane- and budget-bounded.

The zoom preference never increments document generation, marks Markdown dirty, adds Undo entries
or becomes a save source. One hundred relayouts retain Markdown/math semantic counters, and the
Release 50/100/300 benchmark remains below its 50 ms p95 gate.

## Compact Window

The minimum is 220×120 DIP while the default is unchanged. Source and Preview keep a positive
content pane. Split remains fixed 50/50 with a one-DIP divider and never switches mode or expands
the window. Eight compact controls use stable 25-DIP hit targets and are asserted to remain inside
the 220-DIP toolbar without overlap. The opacity popup is clamped to the current window bounds.

## Tool Window Identity

The exact winit 0.30.13 `skip_taskbar` path was tested and rejected as identity authority: on this
runtime it calls `ITaskbarList::DeleteTab` but does not establish the required `WS_EX_TOOLWINDOW`
and `WS_EX_APPWINDOW` facts. The approved thin Windows adapter therefore:

1. creates the tray recovery route;
2. sets `WS_EX_TOOLWINDOW` and clears `WS_EX_APPWINDOW` before first show;
3. rejects any `WS_EX_NOACTIVATE` regression;
4. reads the style back after each winit visibility/minimize transition;
5. restores taskbar reachability if later reassertion fails.

Copied-Release runtime readback passed both initial show and close-to-tray/second-instance restore:
Tool Window present, App Window absent and NOACTIVATE absent. Real Explorer taskbar and Alt+Tab
appearance remain manual observations.

## Dock and Opacity

The docking selector is a bounded O(1) comparison over exactly three eligible distances. Outside a
one-DIP equality band the nearest edge wins; inside it the stable priority is Top, Left, Right.
Release enters expanded state and keeps the existing 700 ms focus-loss, manual/Esc collapse and
16 DIP detach semantics.

Opacity accepts 40, 70, 96 and 100. At 40% the adapter applies alpha 102 without adding transparent
hit-test styles; at 100% it removes the layered style. The copied runtime readback passes 40% alpha
and compact-window interaction.

## Performance and Memory

The Phase 10-specific Release zoom p95 values are 38.700 ms at 50%, 38.275 ms at 100%, and
34.511 ms at 300%. Five copied Split processes per zoom remain below the 64 MiB hard gate; maximum
private working sets were 25,497,600, 25,878,528 and 27,901,952 bytes. One hundred zoom-in/out
cycles reduced private bytes by 86,016 bytes in the observed process, so no linear growth was found.

## Architecture Review

- canonical text owner remains `DocumentState`;
- source and preview remain disposable projections;
- `ConfigCoordinator` is the sole zoom/opacity preference authority;
- window identity is a platform invariant, not configuration;
- dock selection remains pure geometry and the reducer owns lifecycle state;
- no runtime dependency, thread, network client, database or alternate renderer was added;
- core and render remain platform-independent and unsafe-free.
