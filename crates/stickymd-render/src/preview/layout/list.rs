//! Shape list markers independently, retaining the child block's native layout.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::{Align, FontSystem, Wrap};

use super::{BlockBuild, ChunkBuild, INDENT_DIP, LayoutContent, metrics_for};
use crate::preview::text_layout::{TextLayoutCache, make_text_chunk};
use crate::preview::{RenderBlock, RenderBlockKind};
use crate::source::FontSelection;

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_marker(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    block: &RenderBlock,
    padding: f32,
    y: f32,
    content_width: f32,
    scale: f32,
    selection_text: &mut String,
    cache: &mut TextLayoutCache,
) -> Option<(f32, ChunkBuild)> {
    if block.list_markers.is_empty() {
        return None;
    }
    let indent = block.indent as f32 * INDENT_DIP * scale;
    let marker = make_text_chunk(
        font_system,
        fonts,
        &block.list_markers,
        padding + indent,
        y,
        (content_width - indent).max(1.0),
        metrics_for(&block.kind, scale),
        Align::Left,
        Wrap::None,
        selection_text,
        cache,
    );
    let width = marker
        .chunks
        .first()
        .map_or(0.0, |chunk| match &chunk.content {
            LayoutContent::Text(text) => text.first_line_metrics(0.0).0,
            _ => 0.0,
        });
    Some((width, marker))
}

pub(super) fn attach_marker(
    mut marker: ChunkBuild,
    block: &mut BlockBuild,
    kind: &RenderBlockKind,
) {
    let line_top = |content: &super::LayoutChunk| match &content.content {
        LayoutContent::Text(_) => Some(content.y),
        _ => None,
    };
    if let Some(offset) = block
        .chunks
        .first()
        .and_then(line_top)
        .zip(marker.chunks.first().and_then(line_top))
        .map(|(body, marker)| body - marker)
    {
        // Matching line metrics and origins keep hit rows aligned even when a
        // checkbox uses an emoji fallback with a slightly different baseline.
        for chunk in &mut marker.chunks {
            chunk.y += offset;
        }
        marker.height += offset.max(0.0);
    }
    if matches!(kind, RenderBlockKind::DisplayMath)
        && let (Some(item), Some(marker_chunk)) = (block.boxes.first_mut(), marker.chunks.first())
    {
        // A short formula's ink rectangle sits inside the line. Its selectable
        // row must include the marker's shaped line box, or the y-based row
        // index would route formula clicks to a separate overlapping marker row.
        let bottom = item.rect.bottom().max(marker_chunk.y + marker.height);
        item.rect.y = item.rect.y.min(marker_chunk.y);
        item.rect.height = bottom - item.rect.y;
    }
    block.height = block.height.max(marker.height);
    block.chunks.extend(marker.chunks);
}
