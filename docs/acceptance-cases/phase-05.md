# Phase 05 Acceptance Matrix

> Verification projection for the Markdown semantic pipeline, owned AST and native preview
> foundation. Automated rows remain `BLOCKED` until their checked-in Phase 5 smoke entry passes on
> the current commit. Visual, real-shell and OS-resource observations remain `NOT TESTED` until a
> complete current-commit receipt is checked in.

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
  rapidly reverse direction while preview work completes.
- Expected: each pane owns its gutter, with a persistent thin thumb that thickens on hover/drag. The gutter avoids
  text, native resize borders and the Split sync toggle. First/last rows remain reachable, and released/cancelled drags
  stop. Enabled sync uses source anchors; disabled sync preserves the other pane's position. Old paint completions
  cannot overwrite the latest requested scroll position. Selection and document bytes remain unchanged.
- Failure signals: text covered by a thumb, toggling sync or resizing instead of dragging, stuck drag, stale-position
  snapback, hidden short-note thumb still accepting drag, selection mutation, or cross-pane percentage binding.
- Automated entry: `cargo test -p stickymd-render -p stickymd-win --locked scrollbar`; geometry/paint tests cover
  100/125/150/200% DPI and the worker retains unclamped request identity. Status: `AUTOMATED PASS`.
- Native message probe and remaining limits are recorded in [the scrollbar maintenance report](../report/2026-09-25-vertical-scrollbars.md).
  This local evidence does not qualify a release artifact. Physical dragging, real IME and the complete theme/DPI
  visual matrix remain `NOT TESTED`.
