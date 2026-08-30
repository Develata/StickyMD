# Phase 3 — Native Source Editor, IME & Interaction Pipeline

## Completion State

Completed

本任务的实现与自动化验证已完成；以下 Manual Gate/Risks 保留 Phase 3 当时仍开放的真实输入法
条件，后续 exact-candidate automation、人工视觉验收和 USER disposition 才负责发布处置。

## Goal

建立 native Source editor、IME session 与 typed interaction pipeline，同时保持
`DocumentState` 为唯一 canonical text authority。

## Inputs

- Phase 0 contracts are present.
- Corrected Phase 2 document authority passes automated and Release gates.
- Rebuilt Phase 1 evidence explicitly keeps real IME as `NOT TESTED`.

## Scope

- Typed interaction intents and effects.
- `EditorCoordinator` as the only shell-to-`DocumentState` mutation gateway.
- Direction-preserving selection, grapheme navigation, and editor session state.
- IME enabled/preedit/commit/disabled state with preedit outside canonical text.
- Text copy/cut/paste; cut deletes only after a successful clipboard write.
- Script-level font runs and system-font fallback.
- `cosmic-text` source projection and incremental single-line updates.
- `tiny-skia` source painting and `softbuffer` presentation.
- Windows development shell clearly labelled `NOT PERSISTED`.

## Out of Scope

Persistence, autosave, file watcher, conflict UI, Preview/Split, Markdown/RaTeX production
integration, assets, export, tray, docking, theme/opacity controls, and release packaging.

## Architecture Mapping

`winit` event → Interaction translation → typed `AppIntent` → `EditorCoordinator` → canonical
`DocumentState` → typed `AppEffect` → generation-tagged `SourceProjection` → `tiny-skia` →
`softbuffer`.

The shell never receives `&mut DocumentState`. `cosmic-text::Buffer` is a disposable projection;
copy/save/edit never read it as authority.

## Modules

| Area | Files | Responsibility |
| --- | --- | --- |
| Instruction | `instruction/intent.rs` | typed UI intent contract |
| Coordination | `flow/editor.rs`, `flow/clipboard.rs` | canonical mutation ordering and clipboard failure paths |
| Interaction | `interaction/session.rs`, `navigation.rs` | selection, grapheme movement, IME session |
| Projection | `stickymd-render/source/*` | fonts, layout, hit testing, selection/caret/preedit paint |
| Platform | `platform/windows/clipboard.rs`, `surface.rs` | text clipboard and framebuffer adapters |
| Shell | `app.rs`, `app/input.rs`, `main.rs` | lifecycle/presentation and input translation kept separate |

## IME Contract

- Preedit changes only `EditorSession`/`PreeditVisual` and never `DocumentState`.
- Commit emits exactly one `EditKind::ImeCommit` intent and one undo entry.
- `Ime::Commit` is the sole composition commit input. StickyMD does not guess that a later ordinary
  keyboard-text event with equal content is a duplicate; winit's Windows backend already separates
  IME result delivery from ordinary key text.
- Candidate area follows the cursor inside current preedit text; a non-collapsed preedit cursor range
  is painted as composition selection.
- Composition blocks ordinary text/backspace/delete/navigation paths; Esc cancels local preedit.

Automated synthetic tests establish state-machine semantics. They cannot certify Windows IME event
ordering or candidate placement for actual input methods.

## Performance

The initial implementation rebuilt the entire cosmic-text projection on every key and failed at
1 MiB (p95 about 107 ms). It was replaced by a generation-checked affected-line delta path;
ordinary line edits and line splits/merges update locally, while missed generations or inconsistent
deltas conservatively request an explicit resync.

The original incremental path still cloned the complete canonical document before and after every
key. `EditorCoordinator` now returns only `generation + TextDelta`; a full immutable snapshot is
created only for initial load or explicit projection resync.

Final Release p95, including mutation, projection, caret layout, and paint:

| Size | End typing | Start typing | Middle IME commit |
| --- | ---: | ---: | ---: |
| 20 KiB | 0.362 ms | 0.822 ms | 0.738 ms |
| 100 KiB | 0.504 ms | 0.621 ms | 0.443 ms |
| 1 MiB | 0.795 ms | 1.123 ms | 1.118 ms |

The same run also measured p95 for editing commands and the conservative full-resync path:

| Size | Backspace | Delete | Selection replace | Newline | Undo | Redo | Full resync |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.410 ms | 0.464 ms | 0.421 ms | 0.550 ms | 1.068 ms | 0.403 ms | 7.066 ms |
| 100 KiB | 0.606 ms | 0.556 ms | 0.442 ms | 0.569 ms | 1.112 ms | 0.567 ms | 11.108 ms |
| 1 MiB | 1.235 ms | 1.065 ms | 1.192 ms | 1.534 ms | 1.567 ms | 0.939 ms | 53.227 ms |

The 1 MiB full resync is an exceptional recovery path, not the ordinary input or newline path. Its
cost remains measured explicitly so a future regression cannot hide O(n) recovery work in the hot
path.

## Deliverables

- Typed intents/effects 与唯一 `EditorCoordinator` mutation gateway。
- Source selection/navigation、IME preedit/commit 与 clipboard failure paths。
- cosmic-text Source projection、tiny-skia painter 与增量 affected-line update。
- Phase 3 smoke、验收矩阵、性能基线和人工 IME checklist。

## Verification

- 稳定入口：`tools/smoke/phase-03.ps1`；`-Performance` 与 `-Runtime` 分别显式运行
  环境敏感性能和 copied-Release 启动 smoke。
- 当前自动/人工状态：`docs/acceptance-cases/phase-03.md`。
- Automated workspace tests: 30 core unit + 5 core integration + 18 render + 20 app = 73 pass;
  one Release-only performance test ignored by ordinary runs and separately passes.
- Incremental projection tests cover local line update, later-line offset adjustment, newline
  split/merge, trailing empty lines, scroll identity, stale generation, inconsistent delta rejection,
  mixed/wrapped hit-test roundtrips, and internal preedit cursor placement.
- A fixed-seed randomized editor/projection test checks canonical text, generation and valid Unicode
  selections after every edit, deletion, replacement, undo, redo, and presentation-only movement.
- Full fmt/clippy/test/release-build and forbidden dependency audits are Phase 3 final gates.

## Manual Gate

Microsoft Pinyin and WeChat IME remain `NOT TESTED`. Execute
`docs/report/phase-03-manual-ime-checklist.md`; do not interpret synthetic tests as a pass.

## Risks

- Real IME composition event ordering and candidate placement remain unverified.
- System font availability varies; no proprietary CJK font is embedded.

## Result

Automated implementation and performance gates pass. Phase 3 is not fully complete and Phase 4
should not begin until USER records the real IME matrix or explicitly accepts that condition.
