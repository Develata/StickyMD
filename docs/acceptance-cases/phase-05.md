# Phase 05 Acceptance Matrix

> Verification projection for the Markdown semantic pipeline, owned AST and native preview
> foundation. Automated coverage is owned by the checked-in Phase 5 smoke graph. Visual,
> real-shell and OS-resource observations require their own applicable receipts; headless tests
> cannot establish those observations.

Tracked statuses preserve the source baseline; they are not a current release verdict. Later
exact-candidate receipts belong in ignored `dist/evidence/` under [plan 11](../plan/11_testing_and_release.md),
not in retrospective edits to these rows. Version-specific results and remaining gaps are linked
from the [release checklist](../release-checklist.md).

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P05-A01 | 06 owned AST; AC-013 | Automated | Comrak option-lock and fixture tests through [`phase-05.ps1`](../../tools/smoke/phase-05.ps1) | AUTOMATED PASS |
| P05-A02 | 06 transient Arena and snapshot authority | Automated | owned-tree lifetime and public-API boundary tests | AUTOMATED PASS |
| P05-A03 | 06 source ranges; AC-013 | Automated | Unicode byte-range and literal roundtrip tests | AUTOMATED PASS |
| P05-A04 | 06 GFM blocks/inlines; AC-013 | Automated | stable owned-AST golden fixtures | AUTOMATED PASS |
| P05-A05 | 06 four delimiters; AC-014 | Automated | Comrak math node fixture tests, including escapes/code | AUTOMATED PASS |
| P05-A06 | 06 raw HTML literal; AC-016 | Automated | HTML literal preservation and no-execution dependency scan | AUTOMATED PASS |
| P05-A07 | 06 links; AC-013 | Automated | allowed/blocked target classification and hit-test tests | AUTOMATED PASS |
| P05-A08 | 06 remote images; AC-017 | Automated | image classification and zero-network dependency scan | AUTOMATED PASS |
| P05-A09 | 06 RenderTree separation | Automated | semantic-to-render projection golden tests | AUTOMATED PASS |
| P05-A10 | 06 native layout | Automated | paragraph/list/code/table/math/image layout tests plus checked-in rendering-stress raster matrix at 320/900 px, 50/100/300% content scale and Light/Dark | AUTOMATED PASS |
| P05-A11 | 06 viewport culling | Automated | sorted-block binary-search visibility tests | AUTOMATED PASS |
| P05-A12 | 06 preview selection/copy; AC-013 | Automated | wrapped-row and multiline logical-line selection isolation, grapheme-proportional CJK/emoji highlight clipping, block-copy, Ctrl+A/C routing and clipboard-boundary tests | AUTOMATED PASS |
| P05-A13 | 04/06 read-only preview authority | Automated | input-routing tests proving Preview emits no document edit | AUTOMATED PASS |
| P05-A14 | 04/06 PreviewState generation | Automated | stale result/error rejection tests | AUTOMATED PASS |
| P05-A15 | 06 bounded preview scheduler | Automated | virtual-time debounce, immediate mode and explicit 100-rapid-Split-edit latest-only build test | AUTOMATED PASS |
| P05-A16 | 06 worker lifecycle | Automated | one-worker, bounded mailbox and typed failure tests | AUTOMATED PASS |
| P05-A17 | 06 resource limits | Automated | 5 MiB/depth/node-limit rejection tests | AUTOMATED PASS |
| P05-A18 | 06 robustness | Automated | 10,000 deterministic malformed/random inputs plus 10k code, 100×20 table, 2k math and USER-supplied mixed Markdown/RaTeX stress fixture; code/math, raw-HTML, Mermaid, WikiLink and remote-image boundaries remain literal/non-executing | AUTOMATED PASS |
| P05-A19 | 06 no parse on resize/scroll | Automated | counters prove 100 resizes add 0 parses and 1000 scroll paints add 0 parses/semantic rebuilds | AUTOMATED PASS |
| P05-A20 | 09 Source/Preview/Split | Automated | view-mode, fixed split and independent scroll state tests | AUTOMATED PASS |
| P05-A21 | 10 preview performance | Automated | [`phase-05.ps1 -Performance`](../../tools/smoke/phase-05.ps1) cold plus 20 warm 20 KiB/100 KiB/1 MiB Release baseline | AUTOMATED PASS |
| P05-A22 | 11 dependency/unsafe/CI governance | Automated | Phase smoke governance, cargo tree and forbidden architecture checks | AUTOMATED PASS |
| P05-M01 | AC-013 native visual fidelity | Manual | Current-commit Windows 11 Release visual matrix, including the checked-in rendering-stress fixture, required; non-flat pixels/process survival do not substitute for visual judgement | NOT TESTED |
| P05-M02 | AC-013 preview select/copy/link interaction | Manual | Current-commit mouse/clipboard/Shell receipt required | NOT TESTED |
| P05-M03 | AC-013 Split 50/50 and 1000 ms perceived update | Manual | Current-commit source/preview interaction, selection preservation and independent-scroll receipt required | NOT TESTED |
| P05-M04 | AC-014 math placeholder presentation | Manual | Current-commit four-delimiter visual receipt required | NOT TESTED |
| P05-M05 | AC-016 HTML literal visual safety | Manual | Current-commit literal-style visual receipt required | NOT TESTED |
| P05-M06 | AC-017 remote-image zero-network behavior | Manual | Current-commit network-observation receipt required | NOT TESTED |
| P05-M07 | 10 Source/Preview/Split memory and idle CPU | Automated | Five-run Private Working Set/Private Bytes and 60 s CPU through [`phase-05.ps1 -Resources`](../../tools/smoke/phase-05.ps1), with current-commit receipt in the Phase 5 report | AUTOMATED PASS |
| P05-M08 | inherited Phase 3 real IME gate | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P05-M09 | inherited Phase 4 external/recovery/platform conditions | Manual | [`phase-04 matrix`](phase-04.md) | NOT TESTED |

## 2026-09-30 implementation review scope

[Current Preview rules and differences](../report/2026-09-30-preview-rendering-rules.md)
maps the actual parser, projection, layout and adapter behavior at source
`be1322e43624908b7ad04c7c7942d2c3ecb9ed09`. This documentation pass reran
`phase5_semantics`, `table_math_pipes`, `phase6_math` and `rendering_stress`: 21 passed.
It does not make P05-A10 a claim of complete layout conformance: code wrapping,
table horizontal scrolling/row backgrounds, font binding and complex list projection
still have the differences recorded there. The local-image UNC read path also needs
a separate boundary review. Manual rows retain their existing status; no new runtime,
resource or candidate receipt was produced.

The [2026-10-01 review and errata](../report/2026-09-30-preview-rendering-rules.md#review-2026-10-01)
correct painting order, Source-to-Preview rebuild behavior and file-URI scope. Headless diagnostics
also reproduce lost table content when a list item's first block is a table, and a local monospace
fallback other than Consolas. These observations do not upgrade any acceptance row.

## Current Phase Gate

Phase 5's checked-in automated surface passes through the Rust smoke task graph. Manual rows are
intentionally not inferred from unit tests, runtime process survival or a single developer
observation and therefore remain `NOT TESTED`. P05-M07 is the exception because a checked-in Rust
measurement runner now owns the complete five-run/60-second protocol and its durable receipt.

## 2026-09-07 maintenance regression coverage

This supplements P05-A11/A19/A21 under native-preview-layout; historical/manual statuses above retain their original evidence scope.

- Preconditions: a text-only note with the production image adapter present; a long fenced code block with distinct offscreen characters; mixed-script decorated and wrapped text.
- Action: scroll beyond the image predecode band and paint fractional viewport boundaries at multiple scales.
- Expected: no additional text-only layout or parse; offscreen rows do not fill the glyph cache; visible pixels match the full-buffer reference, including decorations. Real local images, including table-cell images, still enter the lazy-image path.
- Failure signals: scroll increments text-only layout counters, caches hundreds of invisible characters, or changes visible reference pixels.
- Entry: `cargo test -p stickymd-render --locked`; `phase-05.ps1 -Performance` also selects the new `phase5_preview_release_baseline_*` scrolling measurements, serially. The narrow benchmark command and diagnostic results are in [the dated audit](../report/2026-09-07-runtime-audit.md). Desktop smoothness and OS memory remain separate observations.

### 2026-09-25 review regression

- Preconditions: a shaped paragraph contains stacked combining marks extending several rows above and below their logical line.
- Action: paint viewports on both sides of that line and compare against Cosmic Text's full-buffer draw.
- Expected: identical viewport pixels, including ink originating outside the immediately adjacent rows; the paint margin comes from the existing shaping offsets.
- Failure signal: visible ink disappears when the originating line leaves the fixed overscan range.
- Entry: `cargo test -p stickymd-render --lib --locked viewport_pixels_`; see the dated audit's 2026-09-25 Resolution for current evidence and limits.

## 2026-09-08 table formula source-range regression coverage

This supplements P05-A03/A05/A12 and AC-013/014 under the existing GFM and source-preservation contracts; it does not change historical candidate or manual statuses.

- Preconditions: a six-column Markdown table with an absolute-value formula; escaped pipes before and inside formulas, code, links or images; four math delimiters and CJK/emoji content.
- Action: preview and copy the formula; convert LaTeX delimiters; export an image occurrence. Repeat in table headers/body rows, blockquotes and lists, with CRLF, whitespace and consecutive backslashes.
- Expected: `\lvert...\rvert`, `\left\lvert...\right\rvert` and table-escaped `\|...\|` produce the same six-column structure; source ranges preserve complete canonical formula/image markup. Conversion and export change only their intended source ranges. Unescaped pipes retain standard GFM column behavior.
- Failure signals: lost formula delimiters, ranges landing inside UTF-8 characters, altered neighboring text, or corrections leaking into another cell/paragraph.
- Entry: `cargo test -p stickymd-render --locked --test table_math_pipes`; the existing Rust Phase 05 runner includes this target through the render crate tests. Invalid coordinate rejection is additionally covered by the `preview::source_map` unit tests. Native window appearance remains `NOT TESTED` for this maintenance change.

## 2026-09-25 vertical scrollbar maintenance

- Contract: `09_windows_shell.md#vertical-scrollbars` and `06_markdown_math_rendering.md#split-scroll-sync`.
- Preconditions: Source, Preview and Split contain a long note; repeat with split synchronization enabled/disabled,
  a tiny viewport, multiple DPI scales and a pending Preview paint.
- Action: hover and drag each thumb, click the track, release outside the window, lose focus, resize or switch modes;
  rapidly reverse direction, resize, zoom, change theme or change selection while preview work completes.
- Expected: each pane owns its gutter, with a persistent thin thumb that thickens on hover/drag. The gutter avoids
  text, native resize borders and the Split sync toggle. First/last rows remain reachable, and released/cancelled drags
  stop. Enabled sync uses source anchors; disabled sync preserves the other pane's position. Old paint completions
  cannot overwrite the latest requested viewport (size, scale, theme, scroll and selection). A stale layout cannot
  drive scroll synchronization or new selection/link hit tests. Selection and document bytes remain unchanged by scrolling.
- Failure signals: text covered by a thumb, toggling sync or resizing instead of dragging, stuck drag, stale-position
  snapback, hidden short-note thumb still accepting drag, selection mutation, or cross-pane percentage binding.
- Automated entry: `cargo test -p stickymd-render -p stickymd-win --locked scrollbar`; geometry/paint tests cover
  100/125/150/200% DPI and the worker retains complete unclamped viewport identity. Coalescing keeps the latest
  viewport and the pending document snapshot; repeated scroll updates reuse layout without parsing. Status: `AUTOMATED PASS`.
- Native message probe and remaining limits are recorded in [the scrollbar maintenance report](../report/2026-09-25-vertical-scrollbars.md).
  This local evidence does not qualify a release artifact. Physical dragging, real IME and the complete theme/DPI
  visual matrix remain `NOT TESTED`.

2026-10-16 supplement: the gesture decisions now live in pure code that the runtime only applies.
`app::scrollbar` decides hover and drag emphasis, live drag targets, track paging (90% of the pane), the end of a drag
and which bars are operable (none while the window rejects edits; no Preview thumb for a stale frame), and
`app::preview_runtime::completion_route` lets only a completion for the current document and viewport reach admission.
Automated entry: `cargo test -p stickymd-win --locked -- scrollbar completion_route`. Status: `AUTOMATED PASS`.
The extractions keep the original early exits: bar geometry is read only while the window accepts edits, the Preview
viewport only for a visible completion of the current document.
The winit wiring of these decisions is still not exercised by a test.

2026-10-17 supplement (C2b-3): the gutter under the cursor is decided by the pure `app::scrollbar::gutter_at` (the
`GUTTER_DIP` band right of each pane over its height, scaled by DPI), tested on its own at 100% and 150% on both sides
of each of its four edges for both panes. The scrollbar
tests now also assert, in a Split, that hovering or dragging either pane emphasizes only that pane's bar, and that over all nine hover × drag states a bar is emphasized exactly when it is
hovered or dragged; that the
emphasized thumb is wider than the painted resting thumb over the same rows, inside the track, at every DPI; that inside
the track a drag keeps the grabbed point under the pointer for two grab offsets; that a press on the Preview track pages
the Preview pane, and that on every edge of either track the last pixel inside pages its pane while the first pixel
outside hits no bar, and the thumb's first and last rows start a drag while the track just above it and at its end pages; a quarter-pixel
sweep over and two pixels around each track, for seven bars at 100% DPI whose thumbs start and end between and on pixels, matches the
half-open track and thumb intervals; so does, at 100/125/150/200% DPI with and without the Split control inset over
4 thumb sizes x 41 positions, a quarter-pixel sweep across each track two pixels beyond both sides and down it within two
pixels of the track's and the thumb's ends, plus presses at each stored thumb boundary and the float just below it (that
float is skipped where the thumb boundary meets a track end, since the hit test reads the pointer as `f32`; those track
ends are covered by the whole- and quarter-pixel presses); every press also checks whether it started a drag; and that `scroll_to_fraction(1.0)` / `(0.0)` show the last / first Source row whole inside the viewport,
for many short lines and for one wrapped line. Nine
plan 09 manual obligations are owned by these and the title / persistence unit tests through P14-A81.
Automated entry: `cargo test -p stickymd-win --locked -- app::scrollbar` and
`cargo test -p stickymd-render --lib --locked -- source::scrollbar`. Status: `AUTOMATED PASS`.

2026-10-19 supplement (C2b-4): the runtime's remaining scroll decisions are pure functions it only calls:
`app::scrollbar::view_bars` (the bars of the panes a view shows, from the Source metrics and the Preview frame's cached
document height, offset and currency), `preview_offset_at` (a thumb fraction to a Preview offset) and
`preview_offset_by` (a page within the document). The tests assert, at 100% and 150% DPI in Source, Preview and Split,
that exactly the shown panes get a bar on their own content rect, that a short note (Source fully visible, or a Preview
document no taller than its pane) shows no thumb while a long pane beside it keeps its thumb, and that a stale frame
shows no Preview thumb without affecting the Source bar; that a fraction of 0 / 1 shows the Preview start / last screen,
linearly in between and read back by the bar, and that pages move by exactly their pixels in both directions and stop
at both ends; that the thumb is the visible share of the track, never under 24 DIP at the window's DPI nor the whole
track, moving linearly from top to bottom (1000 visible shares x 4 DPIs x with and without the Split inset); that
`geometry()` reserves exactly the scaled gutter right of each pane in every view, DPI and four window sizes, with each
track inside its gutter, clear of the real resize hit test and of the Split sync button; that on a 2000-line note whose
every line wraps, the Source end is measured once (laying out fewer than 40 lines) and reused until the geometry
changes, and a pointer-like run of jumps lays out only around each target and reads its fraction back to within a
thousandth of a line; that a Source page moves the viewport by exactly its pixels both ways, stopping at both ends; and
that a Preview receipt for an older offset is only relaid out, never shown. Eleven plan 09 scrollbar and opacity
obligations are owned by these and the opacity drag test through P14-A81. Still named gaps: the split sync switch, the
short-note gutter and Content Zoom independence at run time (the owner calls `geometry()` directly), a press in the
gutter, live drag application, the ends of a drag, viewport-only scrolling, the cached Preview document height at run
time, the Preview currency decision (`preview_layout_is_current`) and the receipt entry's full identity check, the
opacity field (it takes at most three unsigned digits; USER decision D5 opens the input and clamps on commit, not yet
implemented), and the
quit-cancelled notice's run-time wiring (its decisions and two lexical source guards are tested, the effect handling
and event-pass settling are not).
Automated entry: `cargo test -p stickymd-win --locked -- app::scrollbar app::window_interaction app::preview_runtime
app::controls app::title app::guards` and `cargo test -p stickymd-render --lib --locked -- source::scrollbar`.
Status: `AUTOMATED PASS`.

2026-10-20 supplement (USER decision D5): the opacity field keeps every character it is given (a key whose text is a
control character, such as Tab, is not a typing key); focusing it, or the slider or a commit writing it, selects its
value, so the next key or Backspace replaces all of it; a commit reads only an integer (`-` and ASCII digits) and clamps
it into 40–100 whatever its length, and any other entry, such as AC-024's 96.5, commits nothing; while focused the field
paints the end of the typed text (digits, `-` and `.` as themselves, any other character as a question mark). While it is
edited the field takes every key that Esc, F6/F7, save and export leave (the place Search has in the key order): Enter
commits (not Ctrl+Enter) and keeps its auto-repeats and release, Backspace deletes, text without Ctrl, Alt or Win is typed
and any other key is dropped, so none reaches the note or Search; the keys winit synthesizes when the window gains or
loses focus are ignored. The field is numeric: it is not focused while the note or Search is composing, the IME is off
while it is edited, and an IME commit that still arrives is typed and a composition dropped, while the end of a
composition goes on to its owner. The IME follows whoever takes the keyboard in that key order (the edited field: off; a
drawn Search: on, while a Search left open in Preview mode is not drawn and gets none; focused Preview: off; the note:
on), at every site that changes it (window focus, input going away or coming back, a press on Preview or the note,
opening Search or closing it by any of its ways, a view change, the field's edit starting or ending). The open popup takes
every press inside it and shows its cursor there, refreshed when it opens or closes while the pointer is over the window
and when the pointer comes back; a press that lands on no pane changes no focus; a press anywhere else in the
popup, the window losing focus, hiding or quitting ends the edit and commits an integer (else the value is shown again).
The C2b-4 opacity `range-and-input` gap is now owned by `app::controls::phase8_control_tests::{
the_opacity_range_is_40_to_100_in_integer_steps, the_opacity_field_edits_like_a_selected_text_field,
the_opacity_field_takes_keys_while_edited_and_commits_when_it_ends, the_key_that_ends_an_edit_keeps_its_repeats_and_release}`
and `app::window_interaction::phase8_interaction_tests::{the_edited_opacity_field_types_only_plain_text,
the_edited_opacity_field_leaves_the_end_of_a_composition_to_its_owner}` through P14-A81, because exact runs cannot stably
produce that state space; `the_open_popup_takes_every_press_inside_it` proves where a press lands and
`the_ime_follows_whoever_takes_the_keyboard` the IME decision. The key order, the Enter claim, the synthetic-key filter,
the focus refusal, applying the IME decision at each site, the cursor refresh and the blur, hide and quit commits are wired
in `StickyApp` and are not executed by these tests, and delivering real keys stays with the G4 numeric-input obligations
(P08-D022). Automated entry: `cargo test -p stickymd-win --locked -- app::controls app::window_interaction
app::toolbar_paint`. Status: `AUTOMATED PASS`.

Shared entry compatibility (parameter scope, routing, failure and caller-state restoration)
is verified by [P00-A11](phase-00.md); this does not change this phase's manual status.

## 2026-10-01 list projection and code font regression

This supplements the existing plan 06 semantic/layout/selection coverage; historical matrix rows
and manual statuses remain unchanged.

- Preconditions: Comrak inputs whose first list block is a table, heading, code block, quote,
  formula, nested list, task item or empty item; installed Consolas and a separate font database
  with Consolas removed.
- Action: build the RenderTree and real Preview frame; check copy text and cluster hit boundaries;
  shape code through the Preview monospace policy and exercise the missing-font fallback.
- Expected: list markers do not replace the child's block kind or discard table cells; marker and
  content hit targets stay distinct, copied text is complete and source bytes remain unchanged.
  Available Consolas supplies code glyphs; its absence preserves the existing local fallback.
- Failure signals: marker-only table output, lost block styling, marker clicks routed to content,
  missing formula/task hit targets, altered source text, or generic monospace ignoring Consolas.
- Automated entry: `cargo test -p stickymd-render --locked --test phase5_semantics` and
  `cargo test -p stickymd-render --locked --lib phase5_`. The existing `phase-05.ps1` Rust task
  includes these targets; no wrapper parameter or routing changes are needed.
- Limits: generated raster inspection and headless hit tests do not qualify physical mouse,
  clipboard, full theme/DPI or desktop visual acceptance. Other documented layout differences
  remain open; see the dated [Preview implementation report](../report/2026-09-30-preview-rendering-rules.md).

## Document-scoped text geometry reuse

This supplements P05-A10/A12/A19/A21 under plan 06's native-preview-layout and
preview-scheduling contracts. Historical manual and candidate statuses are unchanged.

- Preconditions: repeated attributed text with distinct source ranges, links, selection
  offsets and tooltips; CJK, combining characters, emoji, BiDi and wrapped lines; a unique
  text control and mixed math/image notes.
- Action: exercise admitted cache hits and compare them with independently shaped text;
  vary width, font metrics, alignment, wrapping, style and span boundaries. Drop the lookup
  index and then the live chunks; repeat with a new layout and over-limit inputs.
- Expected: exact copy text and cluster geometry, occurrence-local actions/tooltips, and
  unchanged pixels. Immutable shaping and row locators may be shared within one layout;
  no lookup survives that layout pass. Second-occurrence admission, 1024 total keys and
  the 1024-byte per-key limit remain unchanged. Live chunks own their geometry until dropped.
- Failure signals: stale source/actions, tooltip leakage, geometry reuse across incompatible
  keys or layout passes, changed wrapping/pixels, unbounded keys or retained dead geometry.
- Automated entry: `cargo test -p stickymd-render --lib --locked phase5_text_reuse` plus
  existing text-layout/viewport pixel tests. The existing Phase 05 `-Performance` Rust
  filter includes `phase5_preview_release_baseline_text_reuse`; it measures 20 warm layout
  samples after 4 warmups, with identical 20/100 KiB mixed fixtures and a 20 KiB unique-text
  control. Parsing, font setup, paint and teardown are outside the measured layout interval.
  Local timing/fingerprint comparisons are diagnostics, not portable pixel goldens or GUI
  performance acceptance.
