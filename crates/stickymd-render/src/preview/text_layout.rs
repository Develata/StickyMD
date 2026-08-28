//! Attributed paragraph shaping and selection-box projection.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use cosmic_text::{
    Align, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, UnderlineStyle, Weight, Wrap,
};

use crate::source::{FontSelection, ScriptClass, segment_script_runs};

use super::layout::{ChunkBuild, LayoutChunk, LayoutContent};
use super::{PreviewRect, PreviewTextBox, RenderSpan, RenderStyle, SpanAction};

const MAX_TEXT_LAYOUT_CACHE_ENTRIES: usize = 1_024;
const MAX_TEXT_LAYOUT_CACHE_TEXT_BYTES: usize = 1_024;

#[derive(Debug, Clone)]
pub(super) struct TextSegment {
    visual_range: Range<usize>,
    selection_range: Range<usize>,
    source_range: Option<super::SourceRange>,
    action: Option<std::sync::Arc<SpanAction>>,
    atomic: bool,
    tooltip: Option<std::sync::Arc<str>>,
}

pub(super) struct TextLayout {
    pub buffer: Buffer,
    segments: Vec<TextSegment>,
    rows: Vec<TextLayoutRow>,
}

#[derive(Debug, Clone, Copy)]
struct TextLayoutRow {
    logical_line: usize,
    layout_row: usize,
    logical_byte_start: usize,
    top: f32,
    height: f32,
}

impl TextLayoutRow {
    fn bottom(self) -> f32 {
        self.top + self.height
    }
}

impl TextLayout {
    fn new(buffer: Buffer, segments: Vec<TextSegment>) -> Self {
        let mut logical_byte_starts = Vec::with_capacity(buffer.lines.len());
        let mut logical_byte_start = 0usize;
        for line in &buffer.lines {
            logical_byte_starts.push(logical_byte_start);
            logical_byte_start = logical_byte_start
                .saturating_add(line.text().len())
                .saturating_add(line.ending().as_str().len());
        }

        let mut next_layout_row = vec![0usize; buffer.lines.len()];
        let rows = buffer
            .layout_runs()
            .filter_map(|run| {
                let layout_row = next_layout_row.get_mut(run.line_i)?;
                let row = TextLayoutRow {
                    logical_line: run.line_i,
                    layout_row: *layout_row,
                    logical_byte_start: *logical_byte_starts.get(run.line_i)?,
                    top: run.line_top,
                    height: run.line_height,
                };
                *layout_row = layout_row.saturating_add(1);
                Some(row)
            })
            .collect();
        Self {
            buffer,
            segments,
            rows,
        }
    }

    fn height(&self, fallback: f32) -> f32 {
        self.rows.last().map_or(fallback, |row| row.bottom())
    }

    pub(super) fn mark_atomic_with_tooltip(&mut self, tooltip: std::sync::Arc<str>) {
        for segment in &mut self.segments {
            segment.atomic = true;
            segment.tooltip = Some(std::sync::Arc::clone(&tooltip));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextLayoutKey {
    visual: String,
    span_shapes: Vec<(usize, u8)>,
    width_bits: u32,
    font_size_bits: u32,
    line_height_bits: u32,
    align: u8,
    wrap: u8,
}

/// Ephemeral, document-scoped shaping reuse. It is dropped after one layout
/// and therefore cannot become another document or preview authority.
#[derive(Default)]
pub(super) struct TextLayoutCache {
    buffers: HashMap<TextLayoutKey, Buffer>,
    seen_once: HashSet<TextLayoutKey>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_text_chunk(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    spans: &[RenderSpan],
    x: f32,
    y: f32,
    width: f32,
    metrics: Metrics,
    align: Align,
    wrap: Wrap,
    selection_text: &mut String,
    cache: &mut TextLayoutCache,
) -> ChunkBuild {
    let mut visual = String::new();
    let mut segments = Vec::with_capacity(spans.len());
    let mut span_shapes = Vec::with_capacity(spans.len());
    for span in spans {
        let visual_start = visual.len();
        visual.push_str(&span.text);
        let visual_end = visual.len();
        let selection_start = selection_text.len();
        selection_text.push_str(&span.copy_text);
        let selection_end = selection_text.len();
        segments.push(TextSegment {
            visual_range: visual_start..visual_end,
            selection_range: selection_start..selection_end,
            source_range: span.source_range,
            action: span.action.clone().map(std::sync::Arc::new),
            atomic: false,
            tooltip: None,
        });
        span_shapes.push((visual_end, style_key(span.style)));
    }
    if visual.is_empty() {
        visual.push(' ');
        span_shapes.push((1, 0));
    }
    let key = TextLayoutKey {
        visual,
        span_shapes,
        width_bits: width.to_bits(),
        font_size_bits: metrics.font_size.to_bits(),
        line_height_bits: metrics.line_height.to_bits(),
        align: align_key(align),
        wrap: wrap_key(wrap),
    };
    let buffer = if let Some(buffer) = cache.buffers.get(&key) {
        buffer.clone()
    } else {
        let mut attributed = Vec::new();
        let mut start = 0;
        for (index, span) in spans.iter().enumerate() {
            let end = key.span_shapes[index].0;
            append_attributed_runs(
                &key.visual,
                start..end,
                index + 1,
                span.style,
                metrics,
                fonts,
                &mut attributed,
            );
            start = end;
        }
        if spans.is_empty() {
            attributed.push((0..1, Attrs::new().metrics(metrics)));
        }
        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_size(Some(width), None);
        buffer.set_wrap(wrap);
        let default = Attrs::new().family(Family::Serif).metrics(metrics);
        buffer.set_rich_text(
            attributed
                .iter()
                .map(|(range, attrs)| (&key.visual[range.clone()], attrs.clone())),
            &default,
            Shaping::Advanced,
            Some(align),
        );
        buffer.shape_until_scroll(font_system, false);
        if key.visual.len() <= MAX_TEXT_LAYOUT_CACHE_TEXT_BYTES {
            if cache.seen_once.remove(&key) {
                if cache.buffers.len() < MAX_TEXT_LAYOUT_CACHE_ENTRIES {
                    cache.buffers.insert(key, buffer.clone());
                }
            } else if cache.buffers.len() + cache.seen_once.len() < MAX_TEXT_LAYOUT_CACHE_ENTRIES {
                cache.seen_once.insert(key);
            }
        }
        buffer
    };
    let layout = TextLayout::new(buffer, segments);
    let height = layout.height(metrics.line_height);
    ChunkBuild {
        chunks: vec![LayoutChunk {
            content: LayoutContent::Text(layout),
            x,
            y,
        }],
        height,
        boxes: Vec::new(),
        decorations: Vec::new(),
    }
}

fn style_key(style: RenderStyle) -> u8 {
    u8::from(style.strong)
        | (u8::from(style.emphasis) << 1)
        | (u8::from(style.strikethrough) << 2)
        | (u8::from(style.code) << 3)
        | (u8::from(style.link) << 4)
        | (u8::from(style.html_literal) << 5)
        | (u8::from(style.math_placeholder) << 6)
        | (u8::from(style.image_placeholder) << 7)
}

const fn align_key(align: Align) -> u8 {
    match align {
        Align::Left => 0,
        Align::Right => 1,
        Align::Center => 2,
        Align::Justified => 3,
        Align::End => 4,
    }
}

const fn wrap_key(wrap: Wrap) -> u8 {
    match wrap {
        Wrap::None => 0,
        Wrap::Glyph => 1,
        Wrap::Word => 2,
        Wrap::WordOrGlyph => 3,
    }
}

#[allow(clippy::too_many_arguments)]
fn append_attributed_runs(
    visual: &str,
    range: Range<usize>,
    metadata: usize,
    style: RenderStyle,
    metrics: Metrics,
    fonts: &FontSelection,
    output: &mut Vec<(Range<usize>, Attrs<'static>)>,
) {
    let text = &visual[range.clone()];
    if text.is_empty() {
        return;
    }
    if style.code || style.math_placeholder || style.html_literal {
        output.push((
            range,
            styled_attrs(Family::Monospace, metadata, style, metrics),
        ));
        return;
    }
    for run in segment_script_runs(text) {
        let family = match run.class {
            ScriptClass::Cjk => Family::Name(fonts.cjk_family),
            ScriptClass::Latin => Family::Name(fonts.latin_family),
        };
        output.push((
            (range.start + run.range.start)..(range.start + run.range.end),
            styled_attrs(family, metadata, style, metrics),
        ));
    }
}

fn styled_attrs(
    family: Family<'static>,
    metadata: usize,
    style: RenderStyle,
    metrics: Metrics,
) -> Attrs<'static> {
    let mut attrs = Attrs::new()
        .family(family)
        .metadata(metadata)
        .metrics(metrics);
    if style.strong {
        attrs = attrs.weight(Weight::BOLD);
    }
    if style.emphasis {
        attrs = attrs.style(Style::Italic);
    }
    if style.strikethrough {
        attrs = attrs.strikethrough();
    }
    if style.link {
        attrs = attrs.underline(UnderlineStyle::Single);
    }
    attrs
}

pub(super) fn project_visible_text_boxes(
    layout: &TextLayout,
    x: f32,
    y: f32,
    viewport_top: f32,
    viewport_bottom: f32,
) -> Vec<PreviewTextBox> {
    let buffer = &layout.buffer;
    let segments = &layout.segments;
    let mut boxes: Vec<PreviewTextBox> = Vec::new();
    let local_top = viewport_top - y;
    let local_bottom = viewport_bottom - y;
    let first_row = layout.rows.partition_point(|row| row.bottom() < local_top);
    let last_row = layout.rows.partition_point(|row| row.top <= local_bottom);
    if last_row <= first_row {
        return boxes;
    }
    let mut atomic_extents = vec![None::<(f32, f32)>; segments.len()];
    let mut touched_atomic = Vec::new();
    for row in &layout.rows[first_row..last_row] {
        let Some(line) = buffer.lines.get(row.logical_line) else {
            continue;
        };
        let Some(layout_line) = line.layout_opt().and_then(|rows| rows.get(row.layout_row)) else {
            continue;
        };
        let row_top = y + row.top;
        for glyph in &layout_line.glyphs {
            let Some(index) = glyph.metadata.checked_sub(1) else {
                continue;
            };
            let Some(segment) = segments.get(index) else {
                continue;
            };
            let left = glyph.x.min(glyph.x + glyph.w);
            let right = glyph.x.max(glyph.x + glyph.w);
            // cosmic-text exposes glyph byte offsets relative to each logical
            // BufferLine. Preview selection ranges address the complete
            // immutable clipboard projection, so first restore the paragraph
            // byte offset. Wrapped visual runs on the same logical line reuse
            // this base; a following logical line advances it exactly once.
            let visual_start = row
                .logical_byte_start
                .saturating_add(glyph.start)
                .max(segments[index].visual_range.start);
            let visual_end = row
                .logical_byte_start
                .saturating_add(glyph.end)
                .min(segments[index].visual_range.end);
            if visual_start >= visual_end {
                continue;
            }
            let selection_range =
                selection_range_for_visual_cluster(segment, visual_start..visual_end);
            if selection_range.is_empty() {
                continue;
            }
            let atomic =
                segment.atomic || segment.visual_range.len() != segment.selection_range.len();
            if atomic {
                let extent = &mut atomic_extents[index];
                if extent.is_none() {
                    touched_atomic.push(index);
                }
                *extent = Some(
                    extent.map_or((left, right), |(current_left, current_right)| {
                        (current_left.min(left), current_right.max(right))
                    }),
                );
                continue;
            }
            let rtl = glyph.level.is_rtl();
            let start_x = x + if rtl { right } else { left };
            let end_x = x + if rtl { left } else { right };
            if let Some(previous) = boxes.last_mut()
                && previous.selection_range == selection_range
                && (previous.rect.y - row_top).abs() <= 0.5
            {
                let merged_left = previous.rect.x.min(x + left);
                let merged_right = previous.rect.right().max(x + right);
                previous.rect.x = merged_left;
                previous.rect.width = (merged_right - merged_left).max(1.0);
                if rtl {
                    previous.start_x = previous.start_x.max(start_x);
                    previous.end_x = previous.end_x.min(end_x);
                } else {
                    previous.start_x = previous.start_x.min(start_x);
                    previous.end_x = previous.end_x.max(end_x);
                }
                continue;
            }
            boxes.push(PreviewTextBox {
                selection_range,
                source_range: segment.source_range,
                rect: PreviewRect {
                    x: x + left,
                    y: row_top,
                    width: (right - left).max(1.0),
                    height: row.height,
                },
                action: segment.action.clone(),
                tooltip: segment.tooltip.clone(),
                atomic: false,
                start_x,
                end_x,
            });
        }
        for index in touched_atomic.drain(..) {
            let Some((left, right)) = atomic_extents[index].take() else {
                continue;
            };
            let segment = &segments[index];
            boxes.push(PreviewTextBox {
                selection_range: segment.selection_range.clone(),
                source_range: segment.source_range,
                rect: PreviewRect {
                    x: x + left,
                    y: row_top,
                    width: (right - left).max(1.0),
                    height: row.height,
                },
                action: segment.action.clone(),
                tooltip: segment.tooltip.clone(),
                atomic: true,
                start_x: x + left,
                end_x: x + right,
            });
        }
    }
    boxes
}

fn selection_range_for_visual_cluster(segment: &TextSegment, visual: Range<usize>) -> Range<usize> {
    if segment.visual_range.len() != segment.selection_range.len() {
        return segment.selection_range.clone();
    }
    let start = visual.start.saturating_sub(segment.visual_range.start);
    let end = visual.end.saturating_sub(segment.visual_range.start);
    (segment.selection_range.start + start)..(segment.selection_range.start + end)
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use cosmic_text::{Align, FontSystem, Metrics, Wrap};
    use stickymd_core::Generation;

    use super::{TextLayoutCache, make_text_chunk, project_visible_text_boxes};
    use crate::preview::layout::LayoutContent;
    use crate::preview::{
        LinkKind, PreviewSelection, PreviewTextIndex, RenderSpan, RenderStyle, SourceRange,
        SpanAction,
    };
    use crate::source::FontSelection;

    fn linked_span(destination: &str, source_start: usize) -> RenderSpan {
        let text: std::sync::Arc<str> = std::sync::Arc::from("link");
        RenderSpan {
            text: std::sync::Arc::clone(&text),
            copy_text: text,
            source_range: SourceRange::new(source_start, source_start + 4),
            style: RenderStyle {
                link: true,
                ..RenderStyle::default()
            },
            action: Some(SpanAction::OpenLink {
                destination: destination.to_owned(),
                kind: LinkKind::Https,
            }),
            math: None,
            image: None,
            hard_break: false,
        }
    }

    #[test]
    fn shaping_cache_reuses_geometry_but_reprojects_source_and_actions() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut cache = TextLayoutCache::default();
        let mut selection_text = String::new();
        let first = linked_span("https://first.example", 0);
        let second = linked_span("https://second.example", 100);

        make_text_chunk(
            &mut font_system,
            &fonts,
            &[first],
            0.0,
            0.0,
            300.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::WordOrGlyph,
            &mut selection_text,
            &mut cache,
        );
        let built = make_text_chunk(
            &mut font_system,
            &fonts,
            &[second],
            0.0,
            30.0,
            300.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::WordOrGlyph,
            &mut selection_text,
            &mut cache,
        );

        assert_eq!(cache.buffers.len(), 1);
        assert_eq!(selection_text, "linklink");
        let LayoutContent::Text(layout) = &built.chunks[0].content else {
            panic!("text chunk expected");
        };
        let boxes = project_visible_text_boxes(layout, 0.0, 30.0, 0.0, 300.0);
        assert!(!boxes.is_empty());
        assert!(boxes.iter().all(|item| {
            item.source_range == SourceRange::new(100, 104)
                && matches!(
                    item.action.as_deref(),
                    Some(SpanAction::OpenLink { destination, .. })
                        if destination == "https://second.example"
                )
        }));
    }

    #[test]
    fn multiline_text_boxes_keep_disjoint_selection_ranges() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut cache = TextLayoutCache::default();
        let mut selection_text = String::new();
        let text: Arc<str> = Arc::from("alpha\nbeta\ngamma");
        let span = RenderSpan {
            text: Arc::clone(&text),
            copy_text: text,
            source_range: SourceRange::new(0, 16),
            style: RenderStyle {
                code: true,
                ..RenderStyle::default()
            },
            action: None,
            math: None,
            image: None,
            hard_break: false,
        };

        let built = make_text_chunk(
            &mut font_system,
            &fonts,
            &[span],
            0.0,
            0.0,
            300.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::WordOrGlyph,
            &mut selection_text,
            &mut cache,
        );

        assert_eq!(selection_text, "alpha\nbeta\ngamma");
        let LayoutContent::Text(layout) = &built.chunks[0].content else {
            panic!("text chunk expected");
        };
        let boxes = project_visible_text_boxes(layout, 0.0, 0.0, 0.0, 300.0);
        assert_eq!(
            boxes
                .iter()
                .map(|item| item.selection_range.clone())
                .collect::<Vec<_>>(),
            [
                0..1,
                1..2,
                2..3,
                3..4,
                4..5,
                6..7,
                7..8,
                8..9,
                9..10,
                11..12,
                12..13,
                13..14,
                14..15,
                15..16,
            ]
        );
        let index = PreviewTextIndex::new(Generation::initial(), selection_text, boxes, Vec::new());
        assert_eq!(
            index
                .selection_rects(PreviewSelection {
                    anchor: 6,
                    active: 10,
                })
                .len(),
            1,
            "a single logical-line selection painted unrelated visual rows"
        );
    }

    #[test]
    fn variable_width_clusters_roundtrip_hit_highlight_and_copy() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut selection_text = String::new();
        let text: Arc<str> = Arc::from("iiii WWWW 中文 🙂 e\u{301}");
        let span = RenderSpan {
            text: Arc::clone(&text),
            copy_text: text,
            source_range: SourceRange::new(0, "iiii WWWW 中文 🙂 e\u{301}".len()),
            style: RenderStyle::default(),
            action: None,
            math: None,
            image: None,
            hard_break: false,
        };
        let built = make_text_chunk(
            &mut font_system,
            &fonts,
            &[span],
            10.0,
            20.0,
            600.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::None,
            &mut selection_text,
            &mut TextLayoutCache::default(),
        );
        let LayoutContent::Text(layout) = &built.chunks[0].content else {
            panic!("text chunk expected");
        };
        let boxes = project_visible_text_boxes(layout, 10.0, 20.0, 0.0, 200.0);
        assert!(boxes.len() > 8);
        assert!(
            boxes
                .windows(2)
                .all(|pair| pair[0].selection_range != pair[1].selection_range),
            "multiple glyphs from one shaping cluster must be merged"
        );
        let widths = boxes.iter().map(|item| item.rect.width).collect::<Vec<_>>();
        assert!(
            widths.iter().any(|width| (*width - widths[0]).abs() > 1.0),
            "fixture must retain variable shaping widths"
        );
        let index = PreviewTextIndex::new(
            Generation::initial(),
            selection_text,
            boxes.clone(),
            Vec::new(),
        );
        for item in &boxes {
            assert_eq!(
                index.hit_test(item.start_x, item.rect.y + 1.0),
                item.selection_range.start
            );
            assert_eq!(
                index.hit_test(item.end_x, item.rect.y + 1.0),
                item.selection_range.end
            );
            let selection = PreviewSelection {
                anchor: item.selection_range.start,
                active: item.selection_range.end,
            };
            assert_eq!(
                index.copy(selection),
                index.text().get(item.selection_range.clone())
            );
            assert_eq!(index.selection_rects(selection), vec![item.rect]);
        }
        let combining_start = index.text().find("e\u{301}").unwrap();
        assert_eq!(
            boxes
                .iter()
                .filter(|item| item.selection_range.start == combining_start)
                .count(),
            1,
            "combining sequence must not duplicate one shaping cluster"
        );
    }

    #[test]
    fn bidi_clusters_preserve_logical_boundary_direction() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut selection_text = String::new();
        let text: Arc<str> = Arc::from("abc אבג");
        let span = RenderSpan {
            text: Arc::clone(&text),
            copy_text: text,
            source_range: SourceRange::new(0, "abc אבג".len()),
            style: RenderStyle::default(),
            action: None,
            math: None,
            image: None,
            hard_break: false,
        };
        let built = make_text_chunk(
            &mut font_system,
            &fonts,
            &[span],
            0.0,
            0.0,
            400.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::None,
            &mut selection_text,
            &mut TextLayoutCache::default(),
        );
        let LayoutContent::Text(layout) = &built.chunks[0].content else {
            panic!("text chunk expected");
        };
        let boxes = project_visible_text_boxes(layout, 0.0, 0.0, 0.0, 100.0);
        let rtl = boxes
            .iter()
            .find(|item| item.start_x > item.end_x)
            .expect("Hebrew fixture must expose an RTL cluster")
            .clone();
        let index = PreviewTextIndex::new(Generation::initial(), selection_text, boxes, Vec::new());
        let toward_end = (rtl.end_x - rtl.start_x).signum() * 0.1;
        assert_eq!(
            index.hit_test(rtl.start_x + toward_end, rtl.rect.y + 1.0),
            rtl.selection_range.start
        );
        assert_eq!(
            index.hit_test(rtl.end_x - toward_end, rtl.rect.y + 1.0),
            rtl.selection_range.end
        );
    }

    #[test]
    fn viewport_projection_does_not_retain_offscreen_clusters() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut selection_text = String::new();
        let text: Arc<str> = Arc::from(
            (0..500)
                .map(|row| format!("row-{row}\n"))
                .collect::<String>(),
        );
        let span = RenderSpan {
            text: Arc::clone(&text),
            copy_text: text,
            source_range: None,
            style: RenderStyle::default(),
            action: None,
            math: None,
            image: None,
            hard_break: false,
        };
        let built = make_text_chunk(
            &mut font_system,
            &fonts,
            &[span],
            0.0,
            0.0,
            400.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::WordOrGlyph,
            &mut selection_text,
            &mut TextLayoutCache::default(),
        );
        let LayoutContent::Text(layout) = &built.chunks[0].content else {
            panic!("text chunk expected");
        };
        let all = project_visible_text_boxes(layout, 0.0, 0.0, 0.0, f32::MAX);
        let viewport = project_visible_text_boxes(layout, 0.0, 0.0, 200.0, 500.0);
        assert!(!viewport.is_empty());
        assert!(viewport.len() * 10 < all.len());
        assert!(
            viewport
                .iter()
                .all(|item| item.rect.bottom() >= 200.0 && item.rect.y <= 500.0)
        );
    }

    #[test]
    #[ignore = "Release-only Phase 14 viewport selection performance receipt"]
    fn phase14_preview_selection_geometry_release_baseline() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut selection_text = String::new();
        let text: Arc<str> = Arc::from(
            (0..5_000)
                .map(|row| format!("row-{row:04} iiii WWWW 中文 🙂 e\u{301}\n"))
                .collect::<String>(),
        );
        let span = RenderSpan {
            text: Arc::clone(&text),
            copy_text: text,
            source_range: None,
            style: RenderStyle::default(),
            action: None,
            math: None,
            image: None,
            hard_break: false,
        };
        let built = make_text_chunk(
            &mut font_system,
            &fonts,
            &[span],
            0.0,
            0.0,
            600.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            Wrap::WordOrGlyph,
            &mut selection_text,
            &mut TextLayoutCache::default(),
        );
        let LayoutContent::Text(layout) = &built.chunks[0].content else {
            panic!("text chunk expected");
        };
        let viewport_top = layout.rows[layout.rows.len() / 2].top;
        let viewport_bottom = viewport_top + 720.0;
        let project =
            || project_visible_text_boxes(layout, 0.0, 0.0, viewport_top, viewport_bottom);
        let boxes = project();
        assert!(!boxes.is_empty());

        let row_locator_bytes = layout.rows.len() * size_of::<super::TextLayoutRow>();
        let viewport_geometry_bytes = boxes.len() * size_of::<crate::preview::PreviewTextBox>();
        assert!(
            viewport_geometry_bytes < 512 * 1024,
            "viewport geometry retained {viewport_geometry_bytes} bytes"
        );

        let mut projection_samples = Vec::with_capacity(100);
        for _ in 0..100 {
            let started = Instant::now();
            std::hint::black_box(project());
            projection_samples.push(started.elapsed());
        }
        projection_samples.sort_unstable();
        let projection_p95 = projection_samples[projection_samples.len() * 95 / 100];

        let index = PreviewTextIndex::new(Generation::initial(), selection_text, boxes, Vec::new());
        let hit_started = Instant::now();
        for sample in 0..10_000 {
            let x = (sample % 600) as f32;
            let y = viewport_top + (sample % 700) as f32;
            std::hint::black_box(index.hit_test(x, y));
        }
        let hit_batch = hit_started.elapsed();

        println!(
            "phase14 preview_selection rows={} row_locator_bytes={} visible_clusters={} viewport_geometry_bytes={} project_median={:?} project_p95={projection_p95:?} project_max={:?} hit_10000={hit_batch:?}",
            layout.rows.len(),
            row_locator_bytes,
            index.boxes().len(),
            viewport_geometry_bytes,
            projection_samples[projection_samples.len() / 2],
            projection_samples.last().copied().unwrap_or_default(),
        );
        if !cfg!(debug_assertions) {
            assert!(
                projection_p95 <= Duration::from_millis(10),
                "viewport cluster projection p95 {projection_p95:?} exceeds 10ms"
            );
            assert!(
                hit_batch <= Duration::from_millis(20),
                "10,000 viewport hits took {hit_batch:?}"
            );
        }
    }
}
