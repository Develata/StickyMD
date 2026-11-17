//! Exact-string cases for plan 06 #preview-copy-semantics through the real parse, render-tree and
//! layout path, and a fixed-seed property test of its invariants.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use cosmic_text::FontSystem;
use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

use super::CopySource;
use crate::image::DecodedImageCache;
use crate::math::MathEngine;
use crate::preview::layout::{LayoutResources, layout_document};
use crate::preview::selection::PreviewDocumentProjection;
use crate::preview::{
    PreviewHorizontalOffsets, PreviewParser, PreviewSelection, PreviewTheme, RenderTreeBuilder,
};
use crate::source::FontSelection;

thread_local! {
    static FONTS: RefCell<Option<(FontSystem, FontSelection)>> = const { RefCell::new(None) };
}

fn project_snapshot(snapshot: &DocumentSnapshot) -> Arc<PreviewDocumentProjection> {
    let owned = PreviewParser.parse(snapshot).unwrap();
    let tree = RenderTreeBuilder.build(&owned);
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        let (font_system, selection) = fonts.get_or_insert_with(|| {
            let mut font_system = crate::system_fonts::new_font_system();
            let selection = FontSelection::resolve(&mut font_system);
            (font_system, selection)
        });
        layout_document(
            LayoutResources {
                horizontal_offsets: &PreviewHorizontalOffsets::default(),
                font_system,
                fonts: selection,
                math_engine: &mut MathEngine::new(),
                image_source: None,
                image_cache: &mut DecodedImageCache::default(),
                image_band: (0.0, f32::MAX),
            },
            &tree,
            640,
            1.0,
            PreviewTheme::Light,
        )
        .projection
    })
}

fn snapshot(source: &str) -> DocumentSnapshot {
    DocumentSnapshot {
        text: Arc::from(source),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

fn project(source: &str) -> Arc<PreviewDocumentProjection> {
    project_snapshot(&snapshot(source))
}

/// The display range of `needle`, which must occur exactly once in the display text.
fn find(display: &str, needle: &str) -> Range<usize> {
    let mut found = display.match_indices(needle);
    let (start, _) = found
        .next()
        .unwrap_or_else(|| panic!("{needle:?} is not in {display:?}"));
    assert!(
        found.next().is_none(),
        "{needle:?} occurs twice in {display:?}"
    );
    start..start + needle.len()
}

fn copy_range(projection: &PreviewDocumentProjection, range: Range<usize>) -> String {
    projection
        .copy_text(PreviewSelection {
            anchor: range.end,
            active: range.start,
        })
        .unwrap()
}

/// Copies the display text `selected` of `source`.
fn copy(source: &str, selected: &str) -> String {
    let projection = project(source);
    copy_range(&projection, find(projection.text(), selected))
}

fn copy_all(source: &str) -> String {
    let projection = project(source);
    copy_range(&projection, 0..projection.text().len())
}

/// P17-C01: a formula alone copies its TeX without delimiters, block or inline, rendered or not.
#[test]
fn a_formula_alone_copies_its_tex() {
    assert_eq!(copy_all("$$x^2$$"), "x^2");
    assert_eq!(copy("a $x+1$ b", "$x+1$"), "x+1");
    assert_eq!(copy_all("$$\\notacommand{x}$$"), "\\notacommand{x}");
    assert_eq!(copy("a $\\frac{$ b", "$\\frac{$"), "\\frac{");
    // Comrak shifts this paragraph's positions past the definition, so the formula has no
    // source range; it is still an object.
    assert_eq!(copy("[中]: /x\n$x$", "x"), "x");
}

/// P17-C02: inside a code or Raw HTML block, the selected code without fences or prefixes.
#[test]
fn inside_a_code_block_copies_the_selected_code() {
    assert_eq!(
        copy("```rust\nfn a() {}\nlet b;\n```", "a() {}\nlet"),
        "a() {}\nlet"
    );
    assert_eq!(copy("> ```\n> one\n> two\n> ```", "one\ntwo"), "one\ntwo");
    assert_eq!(copy("- ```\n  one\n  two\n  ```", "ne\ntw"), "ne\ntw");
    assert_eq!(copy("    indented\n    code", "dented\nco"), "dented\nco");
    assert_eq!(copy("<div>\nhello\n</div>", "hello"), "hello");
}

/// P17-C03: an image alone copies its alt text.
#[test]
fn an_image_alone_copies_its_alt_text() {
    assert_eq!(copy_all("![ALT](x.png)"), "ALT");
    assert_eq!(copy("see ![the alt](x.png) here", "the alt"), "the alt");
}

/// P17-C04: a selection across objects copies the source, delimiters and markers included.
#[test]
fn a_selection_across_objects_copies_source() {
    let source = "# Title\n\n**bold** and [link](http://x)\n\n- item\n\n> quote";
    let projection = project(source);
    let display = projection.text();
    let range = find(display, "Title").start..find(display, "quote").end;
    assert_eq!(copy_range(&projection, range), source);
    assert_eq!(
        copy(source, "bold and link"),
        "**bold** and [link](http://x)"
    );
}

/// P17-C05: a cut fenced code block copies its open fence line, the selected code and its close
/// fence line; an unclosed one copies a plain slice.
#[test]
fn a_cut_fenced_code_block_keeps_its_fence_lines() {
    let source = "para\n\n```\nl1\nl2\nl3\n```\n\nafter";
    assert_eq!(copy(source, "ara\nl1\nl"), "ara\n\n```\nl1\nl\n```");
    assert_eq!(copy(source, "2\nl3\n\naft"), "```\n2\nl3\n```\n\naft");
    let quoted = "> para\n>\n> ```\n> l1\n> l2\n> ```";
    assert_eq!(
        copy(quoted, "ara\nl1\nl"),
        "ara\n>\n> ```\n> l1\n> l\n> ```"
    );
    let nested = "> - q\n>   ```\n>   f1\n>   f2\n>   ```";
    assert_eq!(
        copy(nested, "q\nf1\nf"),
        "q\n>   ```\n>   f1\n>   f\n>   ```"
    );
    let unclosed = "para\n\n```\nl1\nl2\n";
    assert_eq!(copy(unclosed, "ara\nl1\nl"), "ara\n\n```\nl1\nl");
    // Indented four spaces, the last line is code, not a close fence: Comrak says unclosed.
    let indented = "p\n\n~~~\nx\n    ~~~";
    assert_eq!(copy(indented, "p\nx\n    ~~"), "p\n\n~~~\nx\n    ~~");
}

/// P17-C06: a cut inline structure copies whole.
#[test]
fn a_cut_inline_structure_copies_whole() {
    assert_eq!(copy("x **abcd** y", "x ab"), "x **abcd**");
    assert_eq!(copy("x **abcd** y", "cd y"), "**abcd** y");
    assert_eq!(copy("x [link](http://a) y", "x li"), "x [link](http://a)");
    assert_eq!(copy("x ~~gone~~ y", "ne y"), "~~gone~~ y");
    assert_eq!(copy("x ![alt](p.png) y", "x alt"), "x ![alt](p.png)");
}

/// P17-C07: a frame copies from its own snapshot, the very allocation the worker parsed.
#[test]
fn a_frame_copies_from_its_own_snapshot() {
    let old = snapshot("old **text** here");
    let projection = project_snapshot(&old);
    let newer = project("new");
    assert!(std::ptr::eq(
        projection.copy_map().source().as_ptr(),
        old.text.as_ptr()
    ));
    assert_eq!(
        copy_range(&projection, 0..projection.text().len()),
        "old **text** here"
    );
    assert_eq!(copy_range(&newer, 0..newer.text().len()), "new");
}

/// P17-C08: select all is an ordinary selection: one formula gives its TeX, a mixed document its
/// source from the first block's start to the last block's end.
#[test]
fn select_all_is_an_ordinary_selection() {
    assert_eq!(copy_all("$$\\alpha$$\n"), "\\alpha");
    assert_eq!(copy_all("```\ncode\n```"), "code\n");
    let mixed = "# Head\n\nText with $x$.\n\n$$y$$\n\n- a\n- b\n";
    assert_eq!(copy_all(mixed), mixed.trim_end());
}

/// P17-C09: untrusted positions: a shifted paragraph snaps to its lines, escapes and entities
/// round to their span, a list marker snaps to its item's start, and a bad mapping fails closed.
#[test]
fn untrusted_positions_round_outward() {
    let shifted = "[a]: http://x\n[b]: http://y\npara **bold** text";
    assert_eq!(copy(shifted, "bol"), copy(shifted, "para bold text"));
    assert_eq!(copy("a \\*b\\* c", "*b"), "a \\*b\\* c");
    assert_eq!(copy("x &amp; y", "&"), "x &amp; y");
    assert_eq!(copy("one\ntwo", "one "), "one\n");
    assert_eq!(copy("one\ntwo", " tw"), "\ntw");
    assert_eq!(copy("- a\n- b", " b"), "- b");
    assert_eq!(copy("1. a\n2. b", ". b"), "2. b");
    // Rounding to a span on the block's edge puts the endpoint on that edge (rule 6, then 2).
    assert_eq!(copy("# a&amp;b", "&"), "# a&amp;b");
    assert_eq!(copy("> x &amp; y", "&"), "> x &amp; y");
    // So does snapping a shifted paragraph to its lines: the quote around it starts a line earlier.
    let shifted_in_quote = ">\n> [r]: /x\n> a&amp;b";
    assert_eq!(copy(shifted_in_quote, "&"), shifted_in_quote);
    // A hard break shows a newline for `  \n`: an endpoint in it rounds to its edges.
    assert_eq!(copy("a  \nb", "a\n"), "a  \n");
    assert_eq!(copy("a  \nb", "\nb"), "  \nb");
    assert_eq!(copy("- [x] done", "☑ done"), "- [x] done");
    assert_eq!(copy("- [x] done", "done"), "done");
    assert_eq!(copy("-\n- b", "• \n• b"), "-\n- b");
    // Comrak 0.59 gives this item end column 0; the parser repairs it (`zero_end_item_range`).
    assert_eq!(
        copy("- one\n- two\n  - deep\n\nafter", "• two\n• deep"),
        "- two\n  - deep"
    );
    let projection = project("é");
    assert!(
        projection
            .copy_text(PreviewSelection {
                anchor: 0,
                active: 1,
            })
            .is_err()
    );
}

/// P17-C09: where Comrak gives a block no range or one cut short, an endpoint that cannot map
/// through it rounds outward to the neighbouring blocks, so select all never fails.
#[test]
fn blocks_without_a_trustworthy_range_round_outward() {
    // An unclosed fence followed by blank lines: no range at all.
    assert_eq!(copy_all("a\n\n```\ncode\n\n"), "a\n\n```\ncode\n\n");
    assert_eq!(copy("a\n\n```\ncode\n\n", "a\ncod"), "a\n\n```\ncode\n\n");
    assert_eq!(copy("a\n\n```\ncode\n\n", "od"), "od");
    // An unclosed fence in a list item at the end: the list, item and block all end at `- \``.
    assert_eq!(copy_all("- ```\n  x\n\n"), "- ```\n  x\n\n");
    // An empty item shows only its marker; nothing is lost, so its range holds.
    assert_eq!(copy_all("- \n\n"), "- ");
    // Select all on documents whose last block Comrak leaves open or ends with blank lines: from
    // the first block's start to the last block's end, nothing shown left out. A thematic break
    // shows no text, so a selection cannot end on it.
    for (source, expected) in [
        ("a\n\n    indented\n\n\n", "a\n\n    indented"),
        ("a\n\n<div>\nx\n\n", "a\n\n<div>\nx\n"),
        ("a\n\n$$\nx\n", "a\n\n$$\nx"),
        ("| a |\n| - |\n| b |\n\n\n", "| a |\n| - |\n| b |"),
        ("> - q\n>\n\n\n", "> - q\n>"),
        ("- a\n\n  b\n\n\n", "- a\n\n  b"),
        (
            "1. one\n   - two\n\n   three\n\n",
            "1. one\n   - two\n\n   three",
        ),
        ("# h\n\n---\n\n", "# h"),
        ("```\n", ""),
        ("> ```\n> x\n", "x\n"),
        ("| | b |\n| - | - |\n| | |\n", "| | b |\n| - | - |\n| | |"),
    ] {
        assert_eq!(copy_all(source), expected, "{source:?}");
    }
}

/// P17-C09: shown text whose source cannot be reached is rounded past outward, never skipped:
/// here the formula's range is withdrawn from a real map, as Comrak sometimes withholds one.
#[test]
fn text_that_cannot_be_mapped_is_rounded_past() {
    let source = "a $x$ b";
    let projection = project(source);
    let display = projection.text();
    let mut map = projection.copy_map().clone();
    let mut pieces = map.pieces.to_vec();
    for piece in &mut pieces {
        if matches!(piece.kind, super::display::PieceKind::Formula(_)) {
            piece.source_start = u32::MAX;
            piece.source_end = u32::MAX;
        }
    }
    map.pieces = pieces.into();
    let slices = |range: Range<usize>| match map.resolve(display, range).unwrap() {
        CopySource::Slices(slices) => slices
            .into_iter()
            .map(|slice| &source[slice])
            .collect::<String>(),
        CopySource::Object(text) => panic!("object {text:?}"),
    };
    assert_eq!(slices(find(display, "x$ b")), "$x$ b");
    assert_eq!(slices(find(display, "a $x")), "a $x$");
    // Alone, it is still an object.
    assert_eq!(
        map.resolve(display, find(display, "$x$")).unwrap(),
        CopySource::Object("x")
    );
}

/// P17-C13: both endpoints inside one inline structure copy only its text; cut nested structures
/// expand level by level.
#[test]
fn inside_one_structure_copies_its_text() {
    assert_eq!(copy("**abcd**", "bc"), "bc");
    assert_eq!(copy("x **ab *cd* ef** y", "b c"), "b *cd*");
    assert_eq!(copy("x **ab *cd* ef** y", "x ab c"), "x **ab *cd* ef**");
}

/// P17-C14: block edges.
#[test]
fn block_edges_take_the_outermost_blocks_source() {
    assert_eq!(copy("# abcd ###", "abcd"), "# abcd ###");
    assert_eq!(copy_all("abcd\n===="), "abcd\n====");
    assert_eq!(copy("abcd\n====", "bc"), "bc");
    assert_eq!(copy("- abcd", "• abcd"), "- abcd");
    assert_eq!(copy("- abcd", "abcd"), "abcd");
    assert_eq!(copy_all("> ab\ncd"), "> ab\ncd");
    let table = "| a | b |\n| - | - |\n| c | d |";
    assert_eq!(copy_all(table), table);
    assert_eq!(copy(table, "a"), "| a");
    assert_eq!(copy(table, "b\nc"), "b |\n| - | - |\n| c");
    assert_eq!(copy("[x][r]\n\n[r]: http://a", "x"), "[x][r]");
}

/// P17-C15: an endpoint on a structure's visible edge is outside it.
#[test]
fn an_endpoint_on_a_structure_edge_is_outside_it() {
    assert_eq!(copy("x **abcd** y", "abcd"), "**abcd**");
    assert_eq!(copy("x **abcd** y", "x abcd"), "x **abcd**");
    assert_eq!(copy("x **abcd** y", "abcd y"), "**abcd** y");
    assert_eq!(copy("x `abcd` y", "abcd"), "`abcd`");
    assert_eq!(copy("x `abcd` y", "x abcd"), "x `abcd`");
}

/// P17-C16: a link whose reported range does not end in `)` snaps to its block's lines.
#[test]
fn an_unclosed_expansion_snaps_to_whole_lines() {
    let source = "see [a](http://x\n) end";
    assert_eq!(copy(source, "e a"), "e [a](http://x\n) end");
    // The snapped endpoint is on its block's edge, so the quote around it still counts (rule 2).
    let in_quote = ">\n> see [a](http://x\n> ) end";
    assert_eq!(copy(in_quote, "a end"), in_quote);
    // An autolink closes with its own `>`, and a bare one has no delimiters to check: a cut through
    // either copies the whole link (USER decision 2026-11-17).
    assert_eq!(copy("x www.auto.com y\nz", "x www.au"), "x www.auto.com");
    assert_eq!(
        copy("x <http://a.io> y\nz", "x http://a"),
        "x <http://a.io>"
    );
    assert_eq!(
        copy("see <http://a.io> now", "http://a.io"),
        "<http://a.io>"
    );
    assert_eq!(copy("x <a@b.c> y", "b.c y"), "<a@b.c> y");
    assert_eq!(copy("x a@b.co y", "x a@"), "x a@b.co");
    // Trailing punctuation is not part of a bare autolink, so it is not pulled in.
    assert_eq!(copy("x https://a.com. y", "x https://a"), "x https://a.com");
}

/// P17-C17: normalized inline code rounds to its content edges, then takes its backticks;
/// unnormalized code maps byte for byte.
#[test]
fn normalized_inline_code_rounds_to_its_edges() {
    assert_eq!(copy("x ` a ` y", "a"), "` a `");
    assert_eq!(copy("x `a\nb` y", "a"), "`a\nb`");
    assert_eq!(copy("x `abcd` y", "bc"), "bc");
    // A formula in the paragraph makes layout split long code into words; they map as one.
    let long = "`abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ` $x$";
    assert_eq!(copy(long, "bc"), "bc");
    assert_eq!(copy(long, "xyz ABC"), "xyz ABC");
}

/// P17-C10: for fixed-seed random selections over documents mixing every construct, slices
/// are in bounds, on UTF-8 boundaries, increasing and disjoint; object text is a formula's TeX
/// or the selected display text; nothing fails.
#[test]
fn random_selections_copy_ordered_snapshot_bytes() {
    const CORPUS: [&str; 19] = [
        "# Head *em* ###\n\nPara **bold *nest* x** `code` [link](http://a) \\* &amp; $x^2$ \
         ![alt](p.png) é中\n\n> quote\nlazy\n\n- one\n- two\n  - deep\n\n1. [ ] task\n\n\
         | a | b |\n| - | - |\n| `c` | $d$ |\n\n```rust\nfn x() {}\n```\n\n    indented\n\n\
         <div>\nhtml\n</div>\n\n$$\\sum_i i$$\n\n---\n\ntext  \nbreak\n",
        "[r]: http://r\n[s]: http://s\nshifted *a* [b][r] c\n\n> - quoted item\n>   ```\n>   fenced\n\
         >   ```\n>\n> > nested\n\n-\n- [x] done\n\n| | empty |\n| - | - |\n| | |\n\n``` \nunclosed",
        "Setext\n======\n\nsee [a](http://x\n) end ` a ` ``b`` ~~s~~ <span>i</span> www.auto.com \
         <http://angle>\n\n1. one\n   2. two\n\n   para\n\n$$\nx\n$$\n",
        "中文**粗体**与`代码`和$\\alpha$。\n\n> 引用 **跨\n> 行** 文字\n\n- 列表 [链接](http://a)\n",
        "[中]: /x\n$x$ `abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ` $y$\n\n# a&amp;b\n\n\
         p\n\n~~~\nx\n    ~~~",
        "a\n\n```\ncode\n\n",
        "a\n\n    indented\n\n\n",
        "a\n\n<div>\nx\n\n",
        "a\n\n$$\nx\n",
        "| a |\n| - |\n| b |\n\n\n",
        "> - q\n>\n\n\n",
        "- a\n\n  b\n\n\n",
        "1. one\n   - two\n\n   three\n\n",
        "- \n\n",
        "# h\n\n---\n\n",
        "```\n",
        "> ```\n> x\n",
        "- ```\n  x\n\n",
        "| | b |\n| - | - |\n| | |\n",
    ];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for source in CORPUS {
        let projection = project(source);
        let display = projection.text();
        let map = projection.copy_map();
        let boundaries: Vec<usize> = (0..=display.len())
            .filter(|&index| display.is_char_boundary(index))
            .collect();
        // Select all first: before M3 it could not fail, and it must not now.
        assert!(map.resolve(display, 0..display.len()).is_ok(), "{source:?}");
        for _ in 0..2000 {
            let a = boundaries[(next() % boundaries.len() as u64) as usize];
            let b = boundaries[(next() % boundaries.len() as u64) as usize];
            let range = a.min(b)..a.max(b);
            let selected = &display[range.clone()];
            match map.resolve(display, range.clone()) {
                Ok(CopySource::Object(text)) => assert!(
                    text == selected || map.formulas.iter().any(|tex| &**tex == text),
                    "{selected:?} copied {text:?}"
                ),
                Ok(CopySource::Slices(slices)) => {
                    let mut previous_end = None;
                    for slice in &slices {
                        assert!(!slice.is_empty(), "{selected:?}: {slices:?}");
                        assert!(
                            source.get(slice.clone()).is_some(),
                            "{selected:?}: {slices:?}"
                        );
                        if let Some(end) = previous_end {
                            assert!(end < slice.start, "{selected:?}: {slices:?}");
                        }
                        previous_end = Some(slice.end);
                    }
                }
                Err(error) => panic!("{selected:?} in {source:?}: {error}"),
            }
        }
    }
}

/// P17-C12 (in-process part): the copy map's heap on the Phase 5 benchmark document, against the
/// display text it maps. Process memory is measured by the resource runs.
#[test]
fn the_copy_map_stays_linear_in_the_document() {
    let seed = include_str!("../../../../../tests/fixtures/performance/typical-note-seed.md");
    for bytes in [20 * 1024, 1024 * 1024] {
        let mut source = seed.repeat(bytes / seed.len() + 1);
        let mut boundary = bytes;
        while !source.is_char_boundary(boundary) {
            boundary -= 1;
        }
        source.truncate(boundary);
        let projection = project(&source);
        let map = projection.copy_map();
        let heap = map.heap_bytes();
        println!(
            "copy_map source={bytes} display={} pieces={} blocks={} structures={} heap={heap} \
             per_source_byte={:.2}",
            projection.text().len(),
            map.pieces.len(),
            map.outline.blocks.len(),
            map.outline.structures.len(),
            heap as f64 / bytes as f64,
        );
        assert!(
            heap <= bytes * 4,
            "{heap} heap bytes for {bytes} source bytes"
        );
    }
}
