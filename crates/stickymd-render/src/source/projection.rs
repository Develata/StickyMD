//! Generation-tagged `cosmic-text` source projection.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor
//!
//! The internal `Buffer` is a disposable layout copy. It never becomes a save,
//! clipboard, or edit source and cannot write back into `DocumentState`.

use cosmic_text::{
    Align, Attrs, AttrsList, Buffer, BufferLine, Family, FontSystem,
    LineEnding as BufferLineEnding, Metrics, Scroll, Shaping, SwashCache, Wrap,
};
use std::ops::Range;
use stickymd_core::{DocumentSnapshot, Generation, Selection, TextDelta};
use thiserror::Error;

use super::fonts::{FontSelection, segment_script_runs};

const FONT_SIZE_DIP: f32 = 16.0;
const LINE_HEIGHT_DIP: f32 = 24.8;
pub(super) const PADDING_DIP: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreeditVisual {
    pub text: String,
    pub cursor: Option<Range<usize>>,
    pub replacement: Selection,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SourceProjectionError {
    #[error("projection generation {projected} is newer than canonical generation {canonical}")]
    FutureGeneration {
        projected: Generation,
        canonical: Generation,
    },
    #[error("selection or cursor is not a valid canonical UTF-8 position")]
    InvalidPosition,
    #[error("incremental projection delta did not match the projected source")]
    DeltaMismatch,
    #[error("projection requires an explicit canonical snapshot resynchronization")]
    ResyncRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceInitializationMilestone {
    FontSystemReady,
    SourceBufferReady,
    SourceShaped,
}

pub struct SourceProjection {
    pub(super) font_system: FontSystem,
    pub(super) swash_cache: SwashCache,
    pub(super) buffer: Buffer,
    pub(super) diagnostic_buffer: Buffer,
    pub(super) diagnostic_text: String,
    pub(super) fonts: FontSelection,
    pub(super) canonical: String,
    pub(super) generation: Generation,
    pub(super) line_starts: Vec<usize>,
    pub(super) width_px: u32,
    pub(super) height_px: u32,
    pub(super) scale_factor: f32,
    pub(super) preedit: Option<PreeditVisual>,
}

impl SourceProjection {
    pub fn new(snapshot: &DocumentSnapshot, width_px: u32, height_px: u32, scale: f32) -> Self {
        Self::new_observed(snapshot, width_px, height_px, scale, |_| {})
    }

    pub fn new_observed(
        snapshot: &DocumentSnapshot,
        width_px: u32,
        height_px: u32,
        scale: f32,
        mut observe: impl FnMut(SourceInitializationMilestone),
    ) -> Self {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        observe(SourceInitializationMilestone::FontSystemReady);
        let metrics = scaled_metrics(scale);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut diagnostic_buffer = Buffer::new(
            &mut font_system,
            Metrics::new(13.0 * scale.max(0.5), 20.0 * scale.max(0.5)),
        );
        diagnostic_buffer.set_wrap(Wrap::None);
        buffer.set_wrap(Wrap::WordOrGlyph);
        let mut projection = Self {
            font_system,
            swash_cache: SwashCache::new(),
            buffer,
            diagnostic_buffer,
            diagnostic_text: String::new(),
            fonts,
            canonical: String::new(),
            generation: Generation::initial(),
            line_starts: vec![0],
            width_px,
            height_px,
            scale_factor: scale.max(0.5),
            preedit: None,
        };
        projection.rebuild_buffer(snapshot);
        observe(SourceInitializationMilestone::SourceBufferReady);
        projection
            .buffer
            .shape_until_scroll(&mut projection.font_system, false);
        observe(SourceInitializationMilestone::SourceShaped);
        projection
    }

    pub const fn projected_generation(&self) -> Generation {
        self.generation
    }

    pub fn projected_text(&self) -> &str {
        &self.canonical
    }

    pub const fn fonts(&self) -> &FontSelection {
        &self.fonts
    }

    pub fn resync(&mut self, snapshot: &DocumentSnapshot) -> Result<(), SourceProjectionError> {
        if self.generation > snapshot.generation {
            return Err(SourceProjectionError::FutureGeneration {
                projected: self.generation,
                canonical: snapshot.generation,
            });
        }
        let scroll = self.buffer.scroll();
        self.rebuild_buffer(snapshot);
        self.buffer.set_scroll(scroll);
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        Ok(())
    }

    /// Apply a canonical delta by rebuilding only the affected logical lines.
    ///
    /// Missed generations or inconsistent deltas require an explicit canonical
    /// snapshot resync. A mismatch never writes back to canonical state.
    pub fn apply_delta(
        &mut self,
        generation: Generation,
        delta: &TextDelta,
    ) -> Result<(), SourceProjectionError> {
        if self.generation.checked_next() != Some(generation) {
            return Err(SourceProjectionError::ResyncRequired);
        }

        let range = delta.range();
        if range.end > self.canonical.len()
            || !self.canonical.is_char_boundary(range.start)
            || !self.canonical.is_char_boundary(range.end)
            || &self.canonical[range.clone()] != delta.deleted()
        {
            return Err(SourceProjectionError::DeltaMismatch);
        }
        if delta.inserted().contains('\n') || delta.deleted().contains('\n') {
            return self.apply_line_structure_delta(generation, delta);
        }

        let Some(_expected_len) = self
            .canonical
            .len()
            .checked_sub(range.len())
            .and_then(|length| length.checked_add(delta.inserted().len()))
        else {
            return Err(SourceProjectionError::DeltaMismatch);
        };

        let line = self
            .line_starts
            .partition_point(|start| *start <= range.start)
            - 1;
        if line >= self.buffer.lines.len() {
            return self.apply_line_structure_delta(generation, delta);
        }
        let line_start = self.line_starts[line];
        let line_end = self
            .line_starts
            .get(line + 1)
            .map_or(self.canonical.len(), |next| next.saturating_sub(1));
        if range.end > line_end {
            return Err(SourceProjectionError::ResyncRequired);
        }

        let Some(buffer_line) = self.buffer.lines.get(line) else {
            // cosmic-text may omit a synthetic empty line after a trailing
            // newline. The canonical delta is still valid, but it cannot be
            // applied to a line that has no current layout representation.
            return Err(SourceProjectionError::ResyncRequired);
        };
        let local_range = (range.start - line_start)..(range.end - line_start);
        if local_range.end > buffer_line.text().len() {
            return Err(SourceProjectionError::DeltaMismatch);
        }
        let mut updated_line = String::with_capacity(
            buffer_line.text().len() - local_range.len() + delta.inserted().len(),
        );
        updated_line.push_str(&buffer_line.text()[..local_range.start]);
        updated_line.push_str(delta.inserted());
        updated_line.push_str(&buffer_line.text()[local_range.end..]);
        let attrs = attrs_for_line(&updated_line, &self.fonts);
        let ending = buffer_line.ending();

        // Validate every fallible offset calculation before mutating either
        // the shaped projection or its canonical mirror.
        if delta.inserted().len() >= delta.deleted().len() {
            let added = delta.inserted().len() - delta.deleted().len();
            if self
                .line_starts
                .last()
                .and_then(|start| start.checked_add(added))
                .is_none()
            {
                return Err(SourceProjectionError::DeltaMismatch);
            }
        } else {
            let removed = delta.deleted().len() - delta.inserted().len();
            if self.line_starts[line + 1..]
                .first()
                .is_some_and(|start| *start < removed)
            {
                return Err(SourceProjectionError::DeltaMismatch);
            }
        }

        let Some(buffer_line) = self.buffer.lines.get_mut(line) else {
            return Err(SourceProjectionError::DeltaMismatch);
        };
        buffer_line.set_text(updated_line, ending, attrs);
        buffer_line.set_align(Some(Align::Left));
        self.buffer.set_redraw(true);
        self.buffer.line_layout(&mut self.font_system, line);

        if delta.inserted().len() >= delta.deleted().len() {
            let added = delta.inserted().len() - delta.deleted().len();
            for start in &mut self.line_starts[line + 1..] {
                *start += added;
            }
        } else {
            let removed = delta.deleted().len() - delta.inserted().len();
            for start in &mut self.line_starts[line + 1..] {
                *start -= removed;
            }
        }
        self.canonical.replace_range(range, delta.inserted());
        self.generation = generation;
        Ok(())
    }

    fn apply_line_structure_delta(
        &mut self,
        generation: Generation,
        delta: &TextDelta,
    ) -> Result<(), SourceProjectionError> {
        let range = delta.range();
        let start_line = self
            .line_starts
            .partition_point(|start| *start <= range.start)
            - 1;
        // The endpoint line participates when deleting a newline because its
        // suffix must merge into the first affected line.
        let end_line = self
            .line_starts
            .partition_point(|start| *start <= range.end)
            - 1;
        let affected_start = self.line_starts[start_line];
        let affected_end = self
            .line_starts
            .get(end_line + 1)
            .map_or(self.canonical.len(), |next| next.saturating_sub(1));
        if range.start < affected_start || range.end > affected_end {
            return Err(SourceProjectionError::DeltaMismatch);
        }

        let Some(updated_canonical_len) = self
            .canonical
            .len()
            .checked_sub(range.len())
            .and_then(|length| length.checked_add(delta.inserted().len()))
        else {
            return Err(SourceProjectionError::DeltaMismatch);
        };
        let Some(capacity) = range
            .start
            .checked_sub(affected_start)
            .and_then(|prefix| prefix.checked_add(delta.inserted().len()))
            .and_then(|length| length.checked_add(affected_end - range.end))
        else {
            return Err(SourceProjectionError::DeltaMismatch);
        };
        let mut updated_block = String::with_capacity(capacity);
        updated_block.push_str(&self.canonical[affected_start..range.start]);
        updated_block.push_str(delta.inserted());
        updated_block.push_str(&self.canonical[range.end..affected_end]);

        let parts = updated_block.split('\n').collect::<Vec<_>>();
        let new_line_count = parts.len();
        let old_logical_count = end_line - start_line + 1;
        let mut replacement_starts = Vec::with_capacity(new_line_count);
        replacement_starts.push(affected_start);
        for (offset, byte) in updated_block.bytes().enumerate() {
            if byte == b'\n' {
                let start = affected_start
                    .checked_add(offset + 1)
                    .filter(|start| *start <= updated_canonical_len)
                    .ok_or(SourceProjectionError::DeltaMismatch)?;
                replacement_starts.push(start);
            }
        }
        if replacement_starts.len() != new_line_count {
            return Err(SourceProjectionError::DeltaMismatch);
        }

        let suffix_start = end_line + 1;
        if delta.inserted().len() >= delta.deleted().len() {
            let added = delta.inserted().len() - delta.deleted().len();
            if self.line_starts[suffix_start..]
                .last()
                .and_then(|start| start.checked_add(added))
                .is_none()
                && !self.line_starts[suffix_start..].is_empty()
            {
                return Err(SourceProjectionError::DeltaMismatch);
            }
        } else {
            let removed = delta.deleted().len() - delta.inserted().len();
            if self.line_starts[suffix_start..]
                .first()
                .is_some_and(|start| *start < removed)
            {
                return Err(SourceProjectionError::DeltaMismatch);
            }
        }

        let remove_end = (end_line + 1).min(self.buffer.lines.len());
        if start_line > remove_end {
            return Err(SourceProjectionError::ResyncRequired);
        }
        let resulting_line_count = self
            .buffer
            .lines
            .len()
            .checked_sub(remove_end - start_line)
            .and_then(|count| count.checked_add(new_line_count))
            .ok_or(SourceProjectionError::DeltaMismatch)?;
        if resulting_line_count == 0 {
            return Err(SourceProjectionError::DeltaMismatch);
        }

        let scroll = self.buffer.scroll();
        let scroll_line = if scroll.line > end_line {
            if new_line_count >= old_logical_count {
                scroll
                    .line
                    .checked_add(new_line_count - old_logical_count)
                    .ok_or(SourceProjectionError::DeltaMismatch)?
            } else {
                scroll
                    .line
                    .checked_sub(old_logical_count - new_line_count)
                    .ok_or(SourceProjectionError::DeltaMismatch)?
            }
        } else if scroll.line >= start_line {
            start_line
        } else {
            scroll.line
        }
        .min(resulting_line_count - 1);

        let final_ending = if self.line_starts.get(end_line + 1).is_some() {
            BufferLineEnding::Lf
        } else {
            BufferLineEnding::None
        };
        let last = new_line_count - 1;
        let replacement_lines = parts.into_iter().enumerate().map(|(index, text)| {
            let ending = if index == last {
                final_ending
            } else {
                BufferLineEnding::Lf
            };
            let mut line = BufferLine::new(
                text,
                ending,
                attrs_for_line(text, &self.fonts),
                Shaping::Advanced,
            );
            line.set_align(Some(Align::Left));
            line
        });

        self.buffer
            .lines
            .splice(start_line..remove_end, replacement_lines);
        self.buffer
            .set_scroll(Scroll::new(scroll_line, scroll.vertical, scroll.horizontal));
        self.buffer.set_redraw(true);
        self.canonical.replace_range(range, delta.inserted());
        self.line_starts
            .splice(start_line..=end_line, replacement_starts);
        let adjusted_suffix = start_line + new_line_count;
        if delta.inserted().len() >= delta.deleted().len() {
            let added = delta.inserted().len() - delta.deleted().len();
            for start in &mut self.line_starts[adjusted_suffix..] {
                *start += added;
            }
        } else {
            let removed = delta.deleted().len() - delta.inserted().len();
            for start in &mut self.line_starts[adjusted_suffix..] {
                *start -= removed;
            }
        }
        self.generation = generation;
        Ok(())
    }

    pub fn set_viewport(&mut self, width_px: u32, height_px: u32, scale: f32) {
        self.width_px = width_px;
        self.height_px = height_px;
        self.scale_factor = scale.max(0.5);
        self.buffer.set_metrics_and_size(
            scaled_metrics(self.scale_factor),
            Some(self.content_width()),
            Some(self.content_height()),
        );
        self.diagnostic_buffer.set_metrics_and_size(
            Metrics::new(13.0 * self.scale_factor, 20.0 * self.scale_factor),
            Some((self.width_px as f32 - 24.0 * self.scale_factor).max(1.0)),
            Some(24.0 * self.scale_factor),
        );
        self.buffer.shape_until_scroll(&mut self.font_system, false);
    }

    pub fn set_preedit(&mut self, preedit: Option<PreeditVisual>) {
        self.preedit = preedit;
    }

    pub fn preedit(&self) -> Option<&PreeditVisual> {
        self.preedit.as_ref()
    }

    /// Hit-test the generic two-action diagnostic banner.
    /// `true` is the primary (F6) action and `false` the secondary (F7) action.
    pub fn diagnostic_action_at(&self, x: f32, y: f32) -> Option<bool> {
        let height = 34.0 * self.scale_factor;
        let action_start = self.width_px as f32 * 0.58;
        if !self.diagnostic_text.contains("[F7")
            || y < self.height_px as f32 - height
            || x < action_start
        {
            return None;
        }
        Some(x < action_start + (self.width_px as f32 - action_start) * 0.5)
    }

    fn rebuild_buffer(&mut self, snapshot: &DocumentSnapshot) {
        self.canonical.clear();
        self.canonical.push_str(&snapshot.text);
        self.generation = snapshot.generation;
        self.line_starts = line_starts(&self.canonical);
        self.configure_buffer();

        self.buffer.lines = source_buffer_lines(&self.canonical, &self.fonts);
        self.buffer.set_redraw(true);
    }

    fn configure_buffer(&mut self) {
        self.buffer.set_metrics_and_size(
            scaled_metrics(self.scale_factor),
            Some(self.content_width()),
            Some(self.content_height()),
        );
        self.buffer.set_wrap(Wrap::WordOrGlyph);
        self.diagnostic_buffer.set_metrics_and_size(
            Metrics::new(13.0 * self.scale_factor, 20.0 * self.scale_factor),
            Some((self.width_px as f32 - 24.0 * self.scale_factor).max(1.0)),
            Some(24.0 * self.scale_factor),
        );
    }
}

pub(super) fn scaled_metrics(scale: f32) -> Metrics {
    Metrics::new(
        FONT_SIZE_DIP * scale.max(0.5),
        LINE_HEIGHT_DIP * scale.max(0.5),
    )
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(index, _)| index + 1))
        .collect()
}

pub(super) fn selection_valid(text: &str, selection: Selection) -> bool {
    selection.anchor.byte <= text.len()
        && selection.active.byte <= text.len()
        && text.is_char_boundary(selection.anchor.byte)
        && text.is_char_boundary(selection.active.byte)
}

fn attrs_for_line(text: &str, fonts: &FontSelection) -> AttrsList {
    let default = Attrs::new().family(Family::Serif);
    let mut attrs = AttrsList::new(&default);
    for run in segment_script_runs(text) {
        let run_attrs = Attrs::new().family(Family::Name(fonts.family_for(run.class)));
        attrs.add_span(run.range, &run_attrs);
    }
    attrs
}

fn source_buffer_lines(text: &str, fonts: &FontSelection) -> Vec<BufferLine> {
    let mut source = text.split('\n').peekable();
    let mut lines = Vec::with_capacity(text.bytes().filter(|byte| *byte == b'\n').count() + 1);
    while let Some(line_text) = source.next() {
        let ending = if source.peek().is_some() {
            BufferLineEnding::Lf
        } else {
            BufferLineEnding::None
        };
        let mut line = BufferLine::new(
            line_text,
            ending,
            attrs_for_line(line_text, fonts),
            Shaping::Advanced,
        );
        line.set_align(Some(Align::Left));
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::source::SourceTheme;
    use stickymd_core::{
        CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, LineEnding,
    };
    use tiny_skia::Pixmap;
    use unicode_segmentation::UnicodeSegmentation;

    fn snapshot(text: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::from(text),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        }
    }

    #[test]
    fn projection_is_generation_tagged_and_resyncs_from_snapshot() {
        let first = snapshot("这是 Rust 测试");
        let mut projection = SourceProjection::new(&first, 600, 400, 1.0);
        assert_eq!(projection.projected_text(), &*first.text);
        assert_eq!(projection.projected_generation(), first.generation);

        let second = DocumentSnapshot {
            text: Arc::from("new"),
            generation: first.generation.checked_next().unwrap(),
            line_ending: LineEnding::Lf,
        };
        projection.resync(&second).unwrap();
        assert_eq!(projection.projected_text(), "new");
        assert_eq!(projection.projected_generation(), second.generation);
    }

    #[test]
    fn hit_test_returns_grapheme_boundary_for_mixed_text() {
        let text = "中e\u{301}👨‍👩‍👧‍👦 Latin";
        let projection = SourceProjection::new(&snapshot(text), 800, 400, 1.0);
        for x in (0..800).step_by(7) {
            let byte = projection.hit_test(x as f32, 30.0);
            assert!(text.is_char_boundary(byte));
            assert!(
                byte == text.len()
                    || text
                        .grapheme_indices(true)
                        .any(|(boundary, _)| boundary == byte)
            );
        }
    }

    #[test]
    fn hit_test_roundtrips_line_edges_mixed_fonts_and_wrapped_lines() {
        let text = "中文 Rust mixed boundary end\nwrap 中文 Latin ".repeat(8);
        let mut projection = SourceProjection::new(&snapshot(&text), 260, 600, 1.0);
        let probes = [
            0,
            "中文 ".len(),
            text.find("Rust").unwrap() + 2,
            text.find('\n').unwrap(),
            text.len() / 2,
        ];
        for mut byte in probes {
            while !text.is_char_boundary(byte) {
                byte -= 1;
            }
            projection.ensure_caret_visible(byte).unwrap();
            let caret = projection.caret_rect(byte).unwrap();
            assert_eq!(
                projection.hit_test(caret.x, caret.y + caret.height * 0.5),
                byte
            );
        }
    }

    #[test]
    fn resize_does_not_change_projected_generation_or_text() {
        let source = snapshot("wrapped line wrapped line wrapped line");
        let mut projection = SourceProjection::new(&source, 200, 200, 1.0);
        projection.set_viewport(800, 600, 2.0);
        assert_eq!(projection.projected_generation(), source.generation);
        assert_eq!(projection.projected_text(), &*source.text);
    }

    #[test]
    fn phase10_content_zoom_relayouts_without_claiming_document_authority() {
        let source = snapshot("中文 Rust 🙂\nsecond line");
        let mut projection = SourceProjection::new(&source, 440, 240, 1.0);
        let generation = projection.projected_generation();
        let text = projection.projected_text().to_owned();
        for scale in [0.5, 1.0, 3.0, 1.25] {
            projection.set_viewport(440, 240, scale);
            assert_eq!(projection.projected_generation(), generation);
            assert_eq!(projection.projected_text(), text);
            assert!(projection.caret_rect(0).is_some());
        }
    }

    #[test]
    fn paint_rejects_invalid_selection_without_panicking() {
        let source = snapshot("中");
        let mut projection = SourceProjection::new(&source, 400, 300, 1.0);
        let mut pixmap = Pixmap::new(400, 300).unwrap();
        assert_eq!(
            projection.paint(
                &mut pixmap,
                Selection::new(0, 1),
                true,
                true,
                None,
                SourceTheme::Light,
            ),
            Err(SourceProjectionError::InvalidPosition)
        );
    }

    #[test]
    fn caret_overlay_changes_only_the_cached_frame_projection() {
        let source = snapshot("caret");
        let mut projection = SourceProjection::new(&source, 400, 300, 1.0);
        let mut pixmap = Pixmap::new(400, 300).unwrap();
        projection
            .paint(
                &mut pixmap,
                Selection::caret(0),
                true,
                false,
                None,
                SourceTheme::Light,
            )
            .unwrap();
        let without_caret = pixmap.data().to_vec();

        projection
            .paint_caret_overlay(&mut pixmap, 0, 0.0, 0.0, SourceTheme::Light)
            .unwrap();

        assert_ne!(pixmap.data(), without_caret);
        assert_eq!(projection.projected_text(), "caret");
        assert_eq!(projection.projected_generation(), source.generation);
    }

    #[test]
    fn caret_overlay_rejects_non_utf8_boundary() {
        let source = snapshot("中");
        let projection = SourceProjection::new(&source, 400, 300, 1.0);
        let mut pixmap = Pixmap::new(400, 300).unwrap();

        assert_eq!(
            projection.paint_caret_overlay(&mut pixmap, 1, 0.0, 0.0, SourceTheme::Dark),
            Err(SourceProjectionError::InvalidPosition)
        );
    }

    #[test]
    fn older_snapshot_cannot_overwrite_newer_projection() {
        let first = DocumentSnapshot {
            text: Arc::from("newer"),
            generation: Generation::initial().checked_next().unwrap(),
            line_ending: LineEnding::Lf,
        };
        let older = snapshot("older");
        let mut projection = SourceProjection::new(&first, 400, 300, 1.0);
        assert!(matches!(
            projection.resync(&older),
            Err(SourceProjectionError::FutureGeneration { .. })
        ));
        assert_eq!(projection.projected_text(), "newer");
    }

    #[test]
    fn preedit_paints_without_changing_projected_text_or_generation() {
        let source = snapshot("ABC 中文");
        let mut projection = SourceProjection::new(&source, 500, 300, 1.0);
        projection.set_preedit(Some(PreeditVisual {
            text: "你好".to_owned(),
            cursor: Some(6..6),
            replacement: Selection::new(0, 3),
        }));
        let mut pixmap = Pixmap::new(500, 300).unwrap();
        projection
            .paint(
                &mut pixmap,
                Selection::new(0, 3),
                true,
                true,
                None,
                SourceTheme::Dark,
            )
            .unwrap();
        assert_eq!(projection.projected_text(), "ABC 中文");
        assert_eq!(projection.projected_generation(), source.generation);
    }

    #[test]
    fn ime_candidate_rect_follows_cursor_inside_preedit() {
        let source = snapshot("");
        let mut projection = SourceProjection::new(&source, 500, 300, 1.0);
        projection.set_preedit(Some(PreeditVisual {
            text: "abc".to_owned(),
            cursor: Some(0..0),
            replacement: Selection::caret(0),
        }));
        let at_start = projection.ime_caret_rect(0).unwrap();
        projection.set_preedit(Some(PreeditVisual {
            text: "abc".to_owned(),
            cursor: Some(3..3),
            replacement: Selection::caret(0),
        }));
        let at_end = projection.ime_caret_rect(0).unwrap();
        assert!(at_end.x > at_start.x);
    }

    #[test]
    fn document_resync_preserves_source_scroll() {
        let text = (0..200)
            .map(|line| format!("line {line} 中文\n"))
            .collect::<String>();
        let source = snapshot(&text);
        let mut projection = SourceProjection::new(&source, 500, 200, 1.0);
        let before = projection.scroll_by(2_000.0);
        assert!(before.line > 0 || before.vertical > 0.0);

        let next = DocumentSnapshot {
            text: Arc::from(format!("{text}tail")),
            generation: source.generation.checked_next().unwrap(),
            line_ending: LineEnding::Lf,
        };
        projection.resync(&next).unwrap();
        assert_eq!(projection.scroll(), before);
    }

    #[test]
    fn single_line_delta_updates_only_the_affected_projection_line() {
        let mut document = DocumentState::loaded("first\nsecond line\nthird", LineEnding::Lf, None);
        let initial = document.snapshot();
        let mut projection = SourceProjection::new(&initial, 600, 400, 1.0);
        let start = document.text().find("line").unwrap();
        let request = EditRequest::new(
            document.generation(),
            start..start + "line".len(),
            "行",
            CursorSnapshot::caret(start),
            CursorSnapshot::caret(start + "行".len()),
            EditMeta::new(EditKind::ImeCommit, 10),
        );
        let outcome = document.edit(request).unwrap();
        projection
            .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
            .unwrap();

        assert_eq!(projection.projected_text(), "first\nsecond 行\nthird");
        assert_eq!(projection.projected_generation(), document.generation());
        assert_eq!(projection.buffer.lines[1].text(), "second 行");
        let third_start = document.text().find("third").unwrap();
        let cursor = projection.cursor_for_global(third_start).unwrap();
        assert_eq!((cursor.line, cursor.index), (2, 0));
    }

    #[test]
    fn newline_delta_splits_only_the_affected_projection_line() {
        let mut document = DocumentState::loaded("first\nthird", LineEnding::Lf, None);
        let initial = document.snapshot();
        let mut projection = SourceProjection::new(&initial, 600, 400, 1.0);
        let start = "first\n".len();
        let request = EditRequest::new(
            document.generation(),
            start..start,
            "second\n",
            CursorSnapshot::caret(start),
            CursorSnapshot::caret(start + "second\n".len()),
            EditMeta::new(EditKind::Paste, 10),
        );
        let outcome = document.edit(request).unwrap();
        projection
            .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
            .unwrap();

        assert_eq!(projection.projected_text(), "first\nsecond\nthird");
        assert_eq!(projection.buffer.lines.len(), 3);
        assert_eq!(projection.buffer.lines[1].text(), "second");
        let cursor = projection.cursor_for_global(document.text().len()).unwrap();
        assert_eq!((cursor.line, cursor.index), (2, "third".len()));
    }

    #[test]
    fn deleting_newline_merges_adjacent_projection_lines() {
        let mut document = DocumentState::loaded("first\nsecond\nthird", LineEnding::Lf, None);
        let mut projection = SourceProjection::new(&document.snapshot(), 600, 400, 1.0);
        let newline = document.text().find('\n').unwrap();
        let request = EditRequest::new(
            document.generation(),
            newline..newline + 1,
            "",
            CursorSnapshot::caret(newline + 1),
            CursorSnapshot::caret(newline),
            EditMeta::new(EditKind::Backspace, 10),
        );
        let outcome = document.edit(request).unwrap();

        projection
            .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
            .unwrap();

        assert_eq!(projection.projected_text(), "firstsecond\nthird");
        assert_eq!(projection.buffer.lines.len(), 2);
        assert_eq!(projection.buffer.lines[0].text(), "firstsecond");
    }

    #[test]
    fn line_structure_delta_keeps_the_same_scrolled_content_visible() {
        let text = (0..200)
            .map(|line| format!("line {line} 中文\n"))
            .collect::<String>();
        let mut document = DocumentState::loaded(&text, LineEnding::Lf, None);
        let mut projection = SourceProjection::new(&document.snapshot(), 500, 200, 1.0);
        let before = projection.scroll_by(2_000.0);
        assert!(before.line > 0);
        let request = EditRequest::new(
            document.generation(),
            0..0,
            "\n",
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(1),
            EditMeta::new(EditKind::Newline, 10),
        );
        let outcome = document.edit(request).unwrap();

        projection
            .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
            .unwrap();

        let after = projection.scroll();
        assert_eq!(after.line, before.line + 1);
        assert_eq!(after.vertical, before.vertical);
        assert_eq!(after.horizontal, before.horizontal);
    }

    #[test]
    fn insertion_after_trailing_newline_materializes_empty_projection_line() {
        let mut document = DocumentState::loaded("first\n", LineEnding::Lf, None);
        let mut projection = SourceProjection::new(&document.snapshot(), 600, 400, 1.0);
        let end = document.text().len();
        let request = EditRequest::new(
            document.generation(),
            end..end,
            "tail",
            CursorSnapshot::caret(end),
            CursorSnapshot::caret(end + "tail".len()),
            EditMeta::new(EditKind::Typing, 10),
        );
        let outcome = document.edit(request).unwrap();

        projection
            .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
            .unwrap();

        assert_eq!(projection.projected_text(), "first\ntail");
        assert_eq!(projection.buffer.lines.len(), 2);
        assert_eq!(projection.buffer.lines[1].text(), "tail");
    }

    #[test]
    fn phase9_source_initialization_preserves_logical_line_endings() {
        let source = snapshot("first\n中文\n");
        let projection = SourceProjection::new(&source, 600, 400, 1.0);

        assert_eq!(projection.buffer.lines.len(), 3);
        assert_eq!(projection.buffer.lines[0].text(), "first");
        assert_eq!(projection.buffer.lines[0].ending(), BufferLineEnding::Lf);
        assert_eq!(projection.buffer.lines[1].text(), "中文");
        assert_eq!(projection.buffer.lines[1].ending(), BufferLineEnding::Lf);
        assert_eq!(projection.buffer.lines[2].text(), "");
        assert_eq!(projection.buffer.lines[2].ending(), BufferLineEnding::None);
        assert_eq!(projection.projected_text(), source.text.as_ref());
    }

    #[test]
    fn phase9_source_initialization_reports_ordered_milestones() {
        let source = snapshot("English 中文");
        let mut milestones = Vec::new();

        let projection = SourceProjection::new_observed(&source, 600, 400, 1.0, |milestone| {
            milestones.push(milestone);
        });

        assert_eq!(
            milestones,
            [
                SourceInitializationMilestone::FontSystemReady,
                SourceInitializationMilestone::SourceBufferReady,
                SourceInitializationMilestone::SourceShaped,
            ]
        );
        assert_eq!(projection.projected_text(), source.text.as_ref());
    }

    #[test]
    fn mismatched_incremental_delta_is_rejected_without_projection_mutation() {
        let initial = snapshot("abc");
        let mut projection = SourceProjection::new(&initial, 600, 400, 1.0);
        let mut document = DocumentState::loaded("adc", LineEnding::Lf, None);
        let request = EditRequest::new(
            document.generation(),
            1..2,
            "x",
            CursorSnapshot::caret(1),
            CursorSnapshot::caret(2),
            EditMeta::new(EditKind::Typing, 10),
        );
        let outcome = document.edit(request).unwrap();

        assert_eq!(
            projection.apply_delta(document.generation(), outcome.delta.as_ref().unwrap()),
            Err(SourceProjectionError::DeltaMismatch)
        );
        assert_eq!(projection.projected_text(), "abc");
        assert_eq!(projection.projected_generation(), initial.generation);
    }
}
