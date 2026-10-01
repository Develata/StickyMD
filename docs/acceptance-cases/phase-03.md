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
