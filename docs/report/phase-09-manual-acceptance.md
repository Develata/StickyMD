# Phase 9 Manual Acceptance

## Status

No Phase 9 manual acceptance case has been executed against a frozen RC artifact. Every row below
therefore remains `NOT TESTED`. Automated reducer, integration, and copied-executable smoke results
are tracked separately and are not substituted for human IME, visual, physical-display, tray, or
failure-timing observation.

## Available Host Environment

| Field | Observed value |
| --- | --- |
| Windows edition | Microsoft Windows 11 Home, Chinese |
| Version / build | 10.0.26200 / 26200 |
| CPU | 12th Gen Intel Core i7-12700H |
| RAM | 16,962,281,472 bytes reported |
| GPU | NVIDIA GeForce RTX 3060 Laptop GPU; GameViewer Virtual Display Adapter also present |
| Display facts | two WMI monitor parameter records; physical topology and per-monitor DPI not manually verified |
| Language profiles | en-GB and zh-Hans-CN; two Chinese TIP identifiers present |
| Input method versions | not identified or manually exercised |
| Source baseline | exact local RC source commit `eb687b2441a5816111c116ce30a01bb5b0fba8c6` |
| Candidate EXE | packaged exact-commit local RC; not published |
| Candidate EXE SHA-256 | `84057a4322c965dbf48646274f2686464f060059a70aeebe1e72264d260c7831` |

The EXE above is now the common manual candidate identity. No manual row has been executed against
it, so assigning the hash does not advance any result beyond `NOT TESTED`.

## Manual Matrix

| Area | Required coverage | Result |
| --- | --- | --- |
| Microsoft Pinyin | Floating/Docked/Split, 70/96 opacity, available DPI, refocus, hover, selection, atomic undo, candidate rect, no duplicate/phantom commit | NOT TESTED |
| WeChat Input Method | same matrix as Microsoft Pinyin | NOT TESTED |
| Preview visual | paragraph, headings, emphasis, mixed CJK/Latin, list, quote, code, table, raw HTML, image, math | NOT TESTED |
| Math visual | inline baseline, scripts, fractions, roots, large operators/delimiters, matrix, cases, CJK mix, display centering, malformed error | NOT TESTED |
| Light / Dark | representative preview and source page in each forced theme | NOT TESTED |
| System theme | live Light to Dark to Light Windows transition | NOT TESTED |
| Whole-window opacity | 70/85/96/100 including text, math, image, controls, IME | NOT TESTED |
| Docking | left/right/top snap, collapse, Esc/manual, sensor, hover without focus, click focus, detach, resize, restart | NOT TESTED |
| Foreground protection | Notepad remains foreground during sensor hover reveal | NOT TESTED |
| Tray lifecycle | real notification icon, exactly three logical menu actions, all close routes, safe tray quit | NOT TESTED |
| Native export dialog | Chinese/space paths, overwrite confirmation, cancel | NOT TESTED |
| Clipboard sources | Explorer PNG/JPEG, Snipping Tool, Paint, browser image; format/result/Markdown/preview/undo/redo | NOT TESTED |
| User asset safety | user-named file survives restart/edit/undo/redo/GC/export/quit | NOT TESTED |
| Managed-looking fake | wrong-digest managed-looking filename survives destructive boundaries | NOT TESTED |
| Reparse boundary | real junction/symlink cannot redirect destructive asset operations outside note | NOT TESTED |
| Crash kill | before autosave, asset paste, and note temp timing using forced process termination | NOT TESTED |
| Multi-monitor | same/mixed DPI, right/left/above secondary, secondary dock and restart | NOT TESTED |
| Monitor disconnect | secondary removal recovers fully to primary | NOT TESTED |
| Cross-DPI IME | candidate rectangle after moving between different-DPI monitors | NOT TESTED |
| 125/150/200 percent DPI | real rendering, input, docking, strip and visual checks | NOT TESTED |
| Sleep / resume | shell, monitor, tray, editor and persistence recovery | NOT TESTED |
| RDP reconnect | display re-enumeration and window recovery | NOT TESTED |
| Clean Windows 11 VM | standalone ZIP without Rust/VS/Git/additional fonts: launch, note, source, preview, math, image, tray, dock, quit | NOT TESTED |

## Release Effect

Microsoft Pinyin, WeChat IME, preview/math/image visual quality, real tray/docking/multi-monitor,
clipboard sources, crash timing, and Clean VM coverage are explicit release blockers. Environment
availability is not evidence of execution. They remain open unless a later frozen-RC receipt records
`MANUAL PASS`, or the USER explicitly waives the individual gate.
