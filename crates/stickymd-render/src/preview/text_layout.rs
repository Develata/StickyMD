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
struct Segment {
    visual_range: Range<usize>,
    selection_range: Range<usize>,
    source_range: Option<super::SourceRange>,
    action: Option<SpanAction>,
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
        segments.push(Segment {
            visual_range: visual_start..visual_end,
            selection_range: selection_start..selection_end,
            source_range: span.source_range,
            action: span.action.clone(),
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
    let height = buffer
        .layout_runs()
        .map(|run| run.line_top + run.line_height)
        .fold(metrics.line_height, f32::max);
    let boxes = boxes_for_buffer(&buffer, &segments, x, y);
    ChunkBuild {
        chunks: vec![LayoutChunk {
            content: LayoutContent::Text(buffer),
            x,
            y,
        }],
        height,
        boxes,
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

fn boxes_for_buffer(buffer: &Buffer, segments: &[Segment], x: f32, y: f32) -> Vec<PreviewTextBox> {
    let mut boxes = Vec::new();
    let mut logical_line = 0;
    let mut logical_line_start = 0usize;
    for run in buffer.layout_runs() {
        while logical_line < run.line_i {
            let Some(line) = buffer.lines.get(logical_line) else {
                break;
            };
            logical_line_start = logical_line_start
                .saturating_add(line.text().len())
                .saturating_add(line.ending().as_str().len());
            logical_line += 1;
        }
        if logical_line != run.line_i {
            continue;
        }
        let mut extents = vec![None::<(f32, f32, usize, usize)>; segments.len()];
        for glyph in run.glyphs {
            let Some(index) = glyph.metadata.checked_sub(1) else {
                continue;
            };
            let Some(extent) = extents.get_mut(index) else {
                continue;
            };
            let left = glyph.x.min(glyph.x + glyph.w);
            let right = glyph.x.max(glyph.x + glyph.w);
            // cosmic-text exposes glyph byte offsets relative to each logical
            // BufferLine. Preview selection ranges address the complete
            // immutable clipboard projection, so first restore the paragraph
            // byte offset. Wrapped visual runs on the same logical line reuse
            // this base; a following logical line advances it exactly once.
            let visual_start = logical_line_start
                .saturating_add(glyph.start)
                .max(segments[index].visual_range.start);
            let visual_end = logical_line_start
                .saturating_add(glyph.end)
                .min(segments[index].visual_range.end);
            if visual_start >= visual_end {
                continue;
            }
            *extent = Some(extent.map_or(
                (left, right, visual_start, visual_end),
                |(current_left, current_right, current_start, current_end)| {
                    (
                        current_left.min(left),
                        current_right.max(right),
                        current_start.min(visual_start),
                        current_end.max(visual_end),
                    )
                },
            ));
        }
        for (index, extent) in extents.into_iter().enumerate() {
            let (left, right, visual_start, visual_end) = match extent {
                Some(extent) => extent,
                None => continue,
            };
            let segment = &segments[index];
            let selection_range =
                selection_range_for_visual_line(segment, visual_start..visual_end);
            if selection_range.is_empty() {
                continue;
            }
            boxes.push(PreviewTextBox {
                selection_range,
                source_range: segment.source_range,
                rect: PreviewRect {
                    x: x + left,
                    y: y + run.line_top,
                    width: (right - left).max(1.0),
                    height: run.line_height,
                },
                action: segment.action.clone(),
                tooltip: None,
                atomic: false,
            });
        }
    }
    boxes
}

fn selection_range_for_visual_line(segment: &Segment, visual: Range<usize>) -> Range<usize> {
    if segment.visual_range.len() != segment.selection_range.len() {
        return segment.selection_range.clone();
    }
    let start = visual.start.saturating_sub(segment.visual_range.start);
    let end = visual.end.saturating_sub(segment.visual_range.start);
    (segment.selection_range.start + start)..(segment.selection_range.start + end)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cosmic_text::{Align, FontSystem, Metrics, Wrap};
    use stickymd_core::Generation;

    use super::{TextLayoutCache, make_text_chunk};
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
        assert!(!built.boxes.is_empty());
        assert!(built.boxes.iter().all(|item| {
            item.source_range == SourceRange::new(100, 104)
                && matches!(
                    &item.action,
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
        assert_eq!(
            built
                .boxes
                .iter()
                .map(|item| item.selection_range.clone())
                .collect::<Vec<_>>(),
            [0..5, 6..10, 11..16]
        );
        let index = PreviewTextIndex::new(Generation::initial(), selection_text, built.boxes);
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
}
