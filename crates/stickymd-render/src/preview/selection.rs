//! Preview selection, hit testing, and safe link activation projection.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-link-safety

use std::ops::Range;
use std::sync::Arc;

use stickymd_core::Generation;
use unicode_segmentation::UnicodeSegmentation;

use super::{SourceRange, SpanAction};

/// Geometry in document pixel coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PreviewRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PreviewRect {
    pub const fn right(self) -> f32 {
        self.x + self.width
    }

    pub const fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
}

/// A byte selection in the preview's immutable clipboard projection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewSelection {
    pub anchor: usize,
    pub active: usize,
}

impl PreviewSelection {
    pub const fn caret(byte: usize) -> Self {
        Self {
            anchor: byte,
            active: byte,
        }
    }

    pub const fn normalized(self) -> Range<usize> {
        if self.anchor <= self.active {
            self.anchor..self.active
        } else {
            self.active..self.anchor
        }
    }

    pub const fn is_collapsed(self) -> bool {
        self.anchor == self.active
    }
}

/// One selectable visual span. The range addresses `PreviewTextIndex::text`.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewTextBox {
    pub selection_range: Range<usize>,
    pub source_range: Option<SourceRange>,
    pub rect: PreviewRect,
    pub action: Option<SpanAction>,
}

/// Immutable, renderer-owned mapping used by the shell for selection and links.
#[derive(Debug, Clone)]
pub struct PreviewTextIndex {
    generation: Generation,
    text: Arc<str>,
    boxes: Arc<[PreviewTextBox]>,
    rows: Arc<[HitRow]>,
}

#[derive(Debug, Clone, Copy)]
struct HitRow {
    top: f32,
    bottom: f32,
    start: usize,
    end: usize,
}

impl PreviewTextIndex {
    pub(super) fn new(
        generation: Generation,
        text: String,
        mut boxes: Vec<PreviewTextBox>,
    ) -> Self {
        boxes.sort_by(|left, right| {
            left.rect
                .y
                .total_cmp(&right.rect.y)
                .then_with(|| left.rect.x.total_cmp(&right.rect.x))
        });
        let rows = build_hit_rows(&boxes);
        Self {
            generation,
            text: Arc::from(text),
            boxes: boxes.into(),
            rows: rows.into(),
        }
    }

    pub const fn generation(&self) -> Generation {
        self.generation
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn boxes(&self) -> &[PreviewTextBox] {
        &self.boxes
    }

    pub fn select_all(&self) -> PreviewSelection {
        PreviewSelection {
            anchor: 0,
            active: self.text.len(),
        }
    }

    pub fn copy(&self, selection: PreviewSelection) -> Option<&str> {
        let range = selection.normalized();
        self.text.get(range)
    }

    /// Hit test in document coordinates, returning a valid UTF-8 boundary in
    /// the preview clipboard projection.
    pub fn hit_test(&self, x: f32, y: f32) -> usize {
        let candidate = self
            .boxes_at_y(y)
            .iter()
            .find(|item| item.rect.contains(x, y))
            .or_else(|| self.nearest_box(x, y));
        let Some(item) = candidate else {
            return 0;
        };
        proportional_boundary(
            &self.text,
            item.selection_range.clone(),
            x,
            item.rect.x,
            item.rect.width,
        )
    }

    pub fn action_at(&self, x: f32, y: f32) -> Option<&SpanAction> {
        self.boxes_at_y(y)
            .iter()
            .find(|item| item.rect.contains(x, y))
            .and_then(|item| item.action.as_ref())
    }

    pub fn selection_rects(&self, selection: PreviewSelection) -> Vec<PreviewRect> {
        let selected = selection.normalized();
        if selected.is_empty() {
            return Vec::new();
        }
        self.boxes
            .iter()
            .filter_map(|item| clipped_selection_rect(item, &selected))
            .collect()
    }

    fn nearest_box(&self, x: f32, y: f32) -> Option<&PreviewTextBox> {
        let candidates = self
            .nearest_row(y)
            .map_or(&[][..], |row| &self.boxes[row.start..row.end]);
        candidates.iter().min_by(|left, right| {
            squared_distance(left.rect, x, y).total_cmp(&squared_distance(right.rect, x, y))
        })
    }

    fn boxes_at_y(&self, y: f32) -> &[PreviewTextBox] {
        let index = self.rows.partition_point(|row| row.bottom < y);
        self.rows
            .get(index)
            .filter(|row| row.top <= y)
            .map_or(&[][..], |row| &self.boxes[row.start..row.end])
    }

    fn nearest_row(&self, y: f32) -> Option<&HitRow> {
        let index = self.rows.partition_point(|row| row.bottom < y);
        match (index.checked_sub(1), self.rows.get(index)) {
            (_, Some(row)) if row.top <= y => Some(row),
            (Some(previous), Some(next)) => {
                let previous = &self.rows[previous];
                if y - previous.bottom <= next.top - y {
                    Some(previous)
                } else {
                    Some(next)
                }
            }
            (Some(previous), None) => self.rows.get(previous),
            (None, Some(next)) => Some(next),
            (None, None) => None,
        }
    }
}

fn build_hit_rows(boxes: &[PreviewTextBox]) -> Vec<HitRow> {
    let mut rows: Vec<HitRow> = Vec::new();
    for (index, item) in boxes.iter().enumerate() {
        if let Some(row) = rows.last_mut()
            && (item.rect.y - row.top).abs() <= 0.5
        {
            row.bottom = row.bottom.max(item.rect.bottom());
            row.end = index + 1;
            continue;
        }
        rows.push(HitRow {
            top: item.rect.y,
            bottom: item.rect.bottom(),
            start: index,
            end: index + 1,
        });
    }
    rows
}

fn proportional_boundary(text: &str, range: Range<usize>, x: f32, left: f32, width: f32) -> usize {
    let Some(selected) = text.get(range.clone()) else {
        return range.start.min(text.len());
    };
    let boundaries = selected
        .grapheme_indices(true)
        .map(|(byte, _)| range.start + byte)
        .chain(std::iter::once(range.end))
        .collect::<Vec<_>>();
    if boundaries.len() <= 1 || width <= 0.0 {
        return range.start;
    }
    let ratio = ((x - left) / width).clamp(0.0, 1.0);
    let slot = (ratio * (boundaries.len() - 1) as f32).round() as usize;
    boundaries[slot.min(boundaries.len() - 1)]
}

fn clipped_selection_rect(item: &PreviewTextBox, selected: &Range<usize>) -> Option<PreviewRect> {
    let start = item.selection_range.start.max(selected.start);
    let end = item.selection_range.end.min(selected.end);
    if start >= end || item.selection_range.is_empty() {
        return None;
    }
    let length = item.selection_range.len() as f32;
    let left_ratio = (start - item.selection_range.start) as f32 / length;
    let right_ratio = (end - item.selection_range.start) as f32 / length;
    Some(PreviewRect {
        x: item.rect.x + item.rect.width * left_ratio,
        y: item.rect.y,
        width: item.rect.width * (right_ratio - left_ratio),
        height: item.rect.height,
    })
}

fn squared_distance(rect: PreviewRect, x: f32, y: f32) -> f32 {
    let horizontal = if x < rect.x {
        rect.x - x
    } else if x > rect.right() {
        x - rect.right()
    } else {
        0.0
    };
    let vertical = if y < rect.y {
        rect.y - y
    } else if y > rect.bottom() {
        y - rect.bottom()
    } else {
        0.0
    };
    horizontal.mul_add(horizontal, vertical * vertical)
}

#[cfg(test)]
mod tests {
    use stickymd_core::Generation;

    use super::*;

    fn index() -> PreviewTextIndex {
        PreviewTextIndex::new(
            Generation::initial(),
            "中文🙂abc".into(),
            vec![PreviewTextBox {
                selection_range: 0.."中文🙂abc".len(),
                source_range: None,
                rect: PreviewRect {
                    x: 10.0,
                    y: 10.0,
                    width: 100.0,
                    height: 20.0,
                },
                action: None,
            }],
        )
    }

    #[test]
    fn hit_test_always_returns_a_utf8_grapheme_boundary() {
        let index = index();
        for x in 0..130 {
            let byte = index.hit_test(x as f32, 20.0);
            assert!(index.text().is_char_boundary(byte));
            assert!(
                byte == index.text().len()
                    || index
                        .text()
                        .grapheme_indices(true)
                        .any(|(boundary, _)| boundary == byte)
            );
        }
    }

    #[test]
    fn reverse_selection_copies_and_clips_without_touching_source() {
        let index = index();
        let selection = PreviewSelection {
            anchor: index.text().len(),
            active: "中文".len(),
        };
        assert_eq!(index.copy(selection), Some("🙂abc"));
        let rects = index.selection_rects(selection);
        assert_eq!(rects.len(), 1);
        assert!(rects[0].width > 0.0 && rects[0].width < 100.0);
    }

    #[test]
    fn hit_testing_uses_only_the_binary_selected_visual_row() {
        let boxes = (0..10_000)
            .map(|row| PreviewTextBox {
                selection_range: 0..1,
                source_range: None,
                rect: PreviewRect {
                    x: 0.0,
                    y: row as f32 * 20.0,
                    width: 100.0,
                    height: 18.0,
                },
                action: None,
            })
            .collect();
        let index = PreviewTextIndex::new(Generation::initial(), "x".into(), boxes);
        assert_eq!(index.boxes_at_y(1234.0 * 20.0 + 4.0).len(), 1);
        assert_eq!(index.hit_test(50.0, 1234.0 * 20.0 + 4.0), 1);
    }
}
