# Phase 03 Acceptance Matrix

> Verification projection for Native Source Editor, IME state modeling and the interaction pipeline.
> Synthetic IME tests never substitute for real input-method acceptance.

Tracked statuses preserve the source baseline; they are not a current release verdict. Later
exact-candidate receipts belong in ignored `dist/evidence/` under [plan 11](../plan/11_testing_and_release.md),
not in retrospective edits to these rows. Version-specific results and remaining gaps are linked
from the [release checklist](../release-checklist.md).

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P03-A01 | AC-002 source editing pipeline | Automated | render/app tests through [`phase-03.ps1`](../../tools/smoke/phase-03.ps1) | AUTOMATED PASS |
| P03-A02 | AC-009 end-to-end intent/undo projection subset | Automated | render/app coordinator tests | AUTOMATED PASS |
| P03-A03 | synthetic preedit/commit/cancel and commit-as-one-undo | Automated | interaction/session and editor-flow tests | AUTOMATED PASS |
| P03-A04 | 20 KiB/100 KiB/1 MiB input pipeline baseline | Automated | [`phase-03.ps1 -Performance`](../../tools/smoke/phase-03.ps1) | AUTOMATED PASS |
| P03-A05 | copied Release native-shell startup smoke | Automated | [`phase-03.ps1 -Runtime`](../../tools/smoke/phase-03.ps1) | AUTOMATED PASS |
| P03-M01 | AC-003 Microsoft Pinyin matrix | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P03-M02 | AC-004 WeChat Input Method matrix | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P03-M03 | candidate positioning at 100/150/200% DPI | Manual | Current-commit visual receipt required | NOT TESTED |
| P03-M04 | selection/composition navigation, refocus and move/resize | Manual | Current-commit interaction receipt required | NOT TESTED |
| P03-M05 | CJK/Latin fallback, caret, selection and preedit visual quality | Manual | Current-commit visual receipt required | NOT TESTED |

## 2026-09-25 clipboard feedback regression

- Preconditions: Source or Split contains a nonempty selection; also exercise copying a Preview selection and a pending error diagnostic.
- Action: copy with Ctrl+C and Ctrl+Insert, then continue reading, scrolling and editing.
- Expected: the clipboard receives the selected text without a success banner covering the source pane; copying does not replace an existing diagnostic. Clipboard failures remain visible and do not mutate the document.
- Failure signals: a persistent `Clipboard updated` overlay, a success notification hiding an error, incorrect copied text, or mutation on a failed copy.
- Automated entry: `cargo test -p stickymd-win --locked`; existing copy/clipboard coordinator tests cover content, generation and failure atomicity. Source/Preview shortcuts converge on the same `ClipboardWritten` shell effect, which now leaves presentation unchanged on success.
- Visual scope: the supplied screenshots reproduce the original obstruction; real desktop verification of the rebuilt application remains `NOT TESTED` until observed. The historical manual rows above are not upgraded by headless tests.

## 2026-09-25 vertical scrollbar maintenance

- Contract: `09_windows_shell.md#vertical-scrollbars`; feature: long-document scrolling in the three views.
- Preconditions: short/empty notes, a 10,000-line note, a single wrapped paragraph, and an active preedit/selection.
- Action: query scroll bounds, drag to both ends and back, edit or replace the note, resize, and change content scale.
- Expected: short notes hide the thumb; long notes reach the first/last visible row without shaping intervening lines;
  bounds follow the current generation and viewport. Text, generation, preedit and selection remain unchanged by scrolling.
  Reserving the gutter does not shift unwrapped text; measuring bounds and a round trip preserve its exact pixels.
- Failure signals: missed document end, stale bounds, whole-document shaping during a jump, altered text/preedit,
  or different pixels after returning to the same viewport.
- Automated entry: `cargo test -p stickymd-render -p stickymd-win --locked scrollbar`, also included in the Rust-owned
  render/app tests through [`phase-03.ps1`](../../tools/smoke/phase-03.ps1). Status: `AUTOMATED PASS`.
- Timing entry: `cargo test -p stickymd-render --lib --release --locked scrollbar_release_baseline -- --ignored --nocapture --test-threads=1`.
  This baseline is also selected by `phase-03.ps1 -Performance`, `modules run render --mode=performance`
  and the complete CI performance shard; each task is deduplicated and measurements run serially per runner.
- Physical mouse/IME/DPI visual matrix: `NOT TESTED`; synthetic native messages and headless pixels do not upgrade the manual rows.

## 2026-10-20 Search across view changes

- Contract: `07_editor_and_ime.md#source-find-replace` (USER decision: an open Search is parked while the view has no
  Source pane and comes back as it was); feature: 查找与替换 in the v1 product behavior.
- Preconditions: Search open in Source and Split, Find-only and with the replacement row, with typed field cursors,
  a non-first active match and the case option on; a Search-field composition; a closed Search with a kept query.
- Action: switch to Preview (toolbar or Ctrl+3) and back to Source or Split (toolbar, Ctrl+1/2, Ctrl+F or Ctrl+H);
  change the document while in Preview; switch while the Search field composes; use Ctrl+1/2/3 and keyboard zoom
  while Search is visibly open.
- Expected: entering Preview parks an open, non-composing Search: the live session is closed, so no key, IME event,
  press, Esc or auto-collapse guard finds it, and it holds no match projection. Leaving Preview restores its query,
  replacement, case option, replacement row, focused field, cursors and active match place, rescanned against the
  current generation, and the Source selection follows the active match as on opening and navigation. In Preview, Ctrl+F restores a parked Search
  without rewriting its query and Ctrl+H also expands or focuses its replacement row; a closed one opens as before,
  and none opens when the view did not change. A composing Search refuses the switch to Preview with the same
  diagnostic as Ctrl+F/H. Source and Split keep an open Search. View and zoom shortcuts act while Search is open;
  during a Search-field composition they stay with the composition like every other key.
- Failure signals: an undrawn Search that takes keys, IME, Esc or blocks auto-collapse; a lost or altered Search after
  the round trip; matches kept while parked or stale after the document changed; a selection other than the active
  match after the round trip (Replace Current would then replace an unselected match); a query rewritten by Ctrl+F in
  Preview; a switch to Preview during a Search composition; Ctrl+1/2/3 or zoom swallowed by a non-composing panel.
- Automated entry: `cargo test -p stickymd-win --locked -- interaction::search app::search_controller app::input`,
  also in the Rust-owned render/app tests through [`phase-03.ps1`](../../tools/smoke/phase-03.ps1):
  `a_parked_search_comes_back_as_it_was_without_holding_matches`, `a_resumed_search_rescans_the_current_document`,
  `only_an_open_search_is_parked_and_a_preedit_does_not_survive_it`,
  `a_view_without_a_source_pane_parks_search_and_one_with_it_resumes_it`,
  `view_and_zoom_shortcuts_take_no_key_that_search_binds` and `window_keys_pass_an_open_search_but_not_its_composition`.
  Status: `AUTOMATED PASS`.
- Limits: the `StickyApp` wiring (`apply_view_mode` deciding before any side effect and applying after the view is
  committed, the selection following the active match on resume, the two `open_search` guards, the key order) is not
  executed by these tests. Desktop steps, `NOT TESTED`: with Search open and the replacement row expanded, toolbar
  Preview then Source, then Ctrl+3 / Ctrl+1 and Ctrl+3 / Ctrl+F and Ctrl+3 / Ctrl+H; in Preview press Esc, type, let
  the window lose focus for 700 ms; with `a X a` and the second `a` active, park, change the file on disk to `a a X`,
  return and press Ctrl+Enter; start a Microsoft Pinyin and a WeType composition in the query field and press
  toolbar Preview, Ctrl+3 and Ctrl+=, then toolbar Split and Source (the composition must go on and commit once,
  with the note unchanged); with Search open, hide to tray and show again in each view. Candidate-window
  placement remains a visual check.

## 2026-10-21 Replace Current follows the selection

- Contract: `07_editor_and_ime.md#source-find-replace` (USER decision: select first, then replace); feature: 查找与替换.
- Preconditions: Search open with the replacement row; `a X a` with the second `a` active and selected.
- Action: change the file on disk to `a a X` and press Ctrl+Enter twice; separately, click or drag elsewhere in the
  note (a caret, another match, a range around the match) and press Ctrl+Enter or the Replace button; apply the toolbar
  `$` conversion while Search is open and press Ctrl+Enter.
- Expected: Replace Current replaces only when the Source selection is exactly the active match. Otherwise the first
  press selects and reveals the active match and changes nothing (text, generation, dirty and Undo unchanged); the next
  press replaces it. Navigation, opening Search, the case toggle and a replacement (Ctrl+Enter or the Replace button)
  select the active match, so repeated Ctrl+Enter or Replace clicks replace one match per press as before. After a
  replacement (or a replacement equal to the match, which changes nothing) the next active match is the first one
  starting at or after the end of the inserted text, wrapping to the first: replacing `a` by `aa` in `a b a` moves on
  to the second `a` instead of growing the first. With no later match it wraps, which can land in text inserted
  earlier (a lone `a` becomes `aa`, then `aaa`); with a truncated projection (`+`) it moves and wraps within the
  retained matches, as navigation does. Replace All is unchanged.
- Failure signals: a replacement of text that was not selected; a first press after a reload that changes the
  document; a normal replace sequence, by keyboard or by mouse, needing two presses per match.
- Automated entry: `app::search_controller::tests::{replace_current_replaces_only_the_selected_active_match,
  consecutive_replacements_move_past_text_that_contains_the_query}` (real replacements through `EditorCoordinator`,
  including a no-op and a normalized `\r\n`) and
  `interaction::search::tests::the_match_after_a_position_becomes_active_and_wraps_to_the_first`, in the Rust-owned
  render/app tests through [`phase-03.ps1`](../../tools/smoke/phase-03.ps1); the command boundary's
  `literal_range_matches` check still rejects a range that is not a match. Status: `AUTOMATED PASS`.
- Limits: `replace_current_search` applying the decision and selecting the next active match after a replacement, and
  the selection reaching that point unchanged after a reload, conversion or click, are `StickyApp` wiring that these
  tests do not execute; the desktop steps above, including three consecutive Replace clicks on `a a a`, are `NOT TESTED`.

Shared entry compatibility (parameter scope, routing, failure and caller-state restoration)
is verified by [P00-A11](phase-00.md); this does not change this phase's manual status.

## Source buffer initialization

- Preconditions: empty/trailing-newline notes, blank paragraphs, whitespace-only
  lines and mixed CJK, Latin, combining marks, emoji and RTL text; exercise different
  viewport sizes and content scales, and absent preferred font families.
- Action: compare initialization without discarded empty shaping against buffers
  initialized by the previous eager constructor and generic Serif defaults. Paint
  Source selection/caret, empty and nonempty diagnostic banners, and empty/mixed shell
  text fields; edit blank lines and resynchronize from a canonical snapshot.
- Expected: identical pixels, caret geometry and hit results; unchanged text and
  generation. Source text is still shaped before its ready milestone, and auxiliary
  buffers are shaped before use. Blank Source lines reuse the already selected Latin
  family; unavailable preferred Latin families retain generic Serif fallback. Explicit
  script runs and IME/window ordering stay intact. Edits and full resync use the same defaults.
- Failure signals: missing first-frame text, changed wrapping/hit targets, changed
  initialization milestone order, or diagnostics presented as formal startup acceptance.
- Automated entry: `source::projection::initialization_tests` and existing ordered
  initialization tests through the render module / Phase 03 tests. Copied Release A/B
  measurements remain diagnostics; the manual IME/DPI rows above are unchanged.

## Preedit overlay initialization

- Preconditions: empty and mixed-script Source notes; empty, Latin, CJK, combining,
  emoji, RTL and long preedit text; collapsed/reverse replacement selections and different scales.
- Action: compare the transient overlay with the previous eager buffer constructor;
  move the composition cursor, select composition text, cancel, and supply invalid replacement ranges.
- Expected: identical unscrolled glyph pixels and selection spans; candidate rectangles
  remain identical while they fit in the pane. The real
  composition is shaped before use; preparing it does not shape a discarded empty line.
  Invalid replacement ranges are rejected without changing canonical text or generation;
  cancellation restores the canonical caret. Commit/undo and font policy remain unchanged.
- Failure signals: changed geometry/pixels, canonical mutation from preedit, stale composition
  after cancellation, or synthetic timings reported as physical IME acceptance.
- Automated entry: `source::geometry::preedit_tests` through the render module / Phase 03;
  existing editor-flow tests cover commit/cancel/undo. Release projection timings are diagnostics;
  the physical IME and manual visual rows above retain their existing status.

### Preedit horizontal reveal regression

- Preconditions: long Latin, CJK/combining/emoji and RTL compositions in a narrow Source
  pane; empty notes and a reverse replacement after existing text; 50/150/300% content
  scales and both themes. Make the canonical replacement caret visible first.
- Action: move/select the composition cursor at the start, middle and end, query the IME
  rectangle, paint the frame, and cancel the composition.
- Expected: the entire candidate caret stays within the Source pane's horizontal bounds. Text, selection,
  underline and painted caret use the same horizontal reveal; no pixels escape its row
  or overwrite text to its left. Cancellation restores the original frame, canonical
  text/generation and Source scroll remain unchanged. Hidden preedit cursors stay hidden.
- Wrapped replacement boundaries: after a space disappears at a soft wrap, revealing
  the replacement caret still returns its visible geometry. Repeating reveal is stable;
  aligning a semantic anchor retains the vertical scroll needed to reach that wrapped row.
- Failure signals: old/new equality that preserves an off-pane caret, a mismatch between
  candidate and painted caret, unclipped glyphs/selection, or document/scroll mutation.
- Automated entry: `source::geometry::preedit_tests`, including the failing-before-fix
  150 px pane case and whole-frame caret composition checks. Physical candidate-window
  placement and DPI visual acceptance remain `NOT TESTED`.
