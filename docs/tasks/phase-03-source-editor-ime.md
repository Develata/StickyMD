# Phase 3 — Native Source Editor, IME & Interaction Pipeline

## Status

Implementation Complete — verification incomplete; awaiting USER manual IME acceptance.

## Prerequisites

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
1 MiB (p95 about 107 ms). It was replaced by a generation-checked single-line delta path; newline,
missed generation, and non-local edits conservatively rebuild.

The original incremental path still cloned the complete canonical document before and after every
key. `EditorCoordinator` now returns only `generation + TextDelta`; a full immutable snapshot is
created only for initial load or explicit projection resync.

Final Release p95, including mutation, projection, caret layout, and paint:

| Size | End typing | Start typing | Middle IME commit |
| --- | ---: | ---: | ---: |
| 20 KiB | 0.382 ms | 0.499 ms | 0.789 ms |
| 100 KiB | 0.444 ms | 0.585 ms | 0.460 ms |
| 1 MiB | 0.922 ms | 1.184 ms | 1.076 ms |

The same run also measured p95 for editing commands and the conservative full-resync path:

| Size | Backspace | Delete | Selection replace | Undo | Redo | Full resync |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.387 ms | 0.446 ms | 0.401 ms | 1.121 ms | 0.416 ms | 6.815 ms |
| 100 KiB | 0.747 ms | 0.615 ms | 0.487 ms | 1.113 ms | 0.553 ms | 10.983 ms |
| 1 MiB | 1.665 ms | 1.371 ms | 1.399 ms | 2.028 ms | 1.278 ms | 52.710 ms |

The 1 MiB full resync is an exceptional recovery/newline path, not the ordinary input path. Its
cost is now measured explicitly so future work can optimize it only if real workloads justify it.

## Verification

- Automated workspace tests: 30 core unit + 5 core integration + 15 render + 20 app = 70 pass;
  one Release-only performance test ignored by ordinary runs and separately passes.
- Incremental projection tests cover local line update, later-line offset adjustment, newline
  resync (including a trailing empty line), stale generation, inconsistent delta rejection,
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
