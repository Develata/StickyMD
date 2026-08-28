//! Preview selection, hit testing, and safe link activation projection.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-link-safety

use std::ops::Range;
use std::sync::Arc;

use crate::scroll::ScrollAnchor;
use stickymd_core::Generation;

use super::scroll::{PreviewScrollAnchor, PreviewScrollIndex};
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
    pub action: Option<Arc<SpanAction>>,
    /// Short diagnostic exposed only while hovering a failed visual object.
    pub tooltip: Option<Arc<str>>,
    /// Atomic objects such as formulas select as one visual rectangle.
    pub atomic: bool,
    /// Document-space x coordinate of the logical range start boundary.
    /// It can be greater than `end_x` for an RTL shaping cluster.
    pub start_x: f32,
    /// Document-space x coordinate of the logical range end boundary.
    pub end_x: f32,
}

/// Immutable full-document projection shared by successive viewport frames.
#[derive(Debug, Clone)]
pub(super) struct PreviewDocumentProjection {
    generation: Generation,
    text: Arc<str>,
    scroll: PreviewScrollIndex,
}

impl PreviewDocumentProjection {
    pub(super) fn new(
        generation: Generation,
        text: String,
        scroll_anchors: Vec<PreviewScrollAnchor>,
    ) -> Self {
        Self {
            generation,
            text: Arc::from(text),
            scroll: PreviewScrollIndex::new(scroll_anchors),
        }
    }

    #[cfg(test)]
    pub(super) const fn generation(&self) -> Generation {
        self.generation
    }

    #[cfg(test)]
    pub(super) fn text(&self) -> &str {
        &self.text
    }
}

/// Immutable viewport mapping used for selection and safe actions.
#[derive(Debug, Clone)]
pub struct PreviewTextIndex {
    document: Arc<PreviewDocumentProjection>,
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
    #[cfg(test)]
    pub(super) fn new(
        generation: Generation,
        text: String,
        boxes: Vec<PreviewTextBox>,
        scroll_anchors: Vec<PreviewScrollAnchor>,
    ) -> Self {
        Self::from_document(
            Arc::new(PreviewDocumentProjection::new(
                generation,
                text,
                scroll_anchors,
            )),
            boxes,
        )
    }

    pub(super) fn from_document(
        document: Arc<PreviewDocumentProjection>,
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
            document,
            boxes: boxes.into(),
            rows: rows.into(),
        }
    }

    pub fn generation(&self) -> Generation {
        self.document.generation
    }

    pub fn text(&self) -> &str {
        &self.document.text
    }

    pub fn boxes(&self) -> &[PreviewTextBox] {
        &self.boxes
    }

    /// Maps a document-space y position to a canonical semantic anchor.
    pub fn scroll_anchor_at_y(&self, y: f32) -> Option<ScrollAnchor> {
        self.document.scroll.anchor_at_y(y)
    }

    /// Maps a canonical semantic anchor into Preview document-space y.
    pub fn y_for_scroll_anchor(&self, anchor: ScrollAnchor) -> Option<f32> {
        self.document.scroll.y_for_anchor(anchor)
    }

    pub fn select_all(&self) -> PreviewSelection {
        PreviewSelection {
            anchor: 0,
            active: self.document.text.len(),
        }
    }

    pub fn copy(&self, selection: PreviewSelection) -> Option<&str> {
        let range = selection.normalized();
        self.document.text.get(range)
    }

    /// Hit test in document coordinates, returning a valid UTF-8 boundary in
    /// the preview clipboard projection.
    pub fn hit_test(&self, x: f32, y: f32) -> usize {
        let candidate = self
            .nearest_row(y)
            .and_then(|row| self.nearest_box_in_row(row, x, y));
        let Some(item) = candidate else {
            return 0;
        };
        if (x - item.start_x).abs() <= (x - item.end_x).abs() {
            item.selection_range.start
        } else {
            item.selection_range.end
        }
    }

    pub fn action_at(&self, x: f32, y: f32) -> Option<&SpanAction> {
        self.row_at_y(y)
            .and_then(|row| self.nearest_box_in_row(row, x, y))
            .filter(|item| item.rect.contains(x, y))
            .and_then(|item| item.action.as_deref())
    }

    pub fn tooltip_at(&self, x: f32, y: f32) -> Option<&str> {
        self.row_at_y(y)
            .and_then(|row| self.nearest_box_in_row(row, x, y))
            .filter(|item| item.rect.contains(x, y))
            .and_then(|item| item.tooltip.as_deref())
    }

    pub fn selection_rects(&self, selection: PreviewSelection) -> Vec<PreviewRect> {
        self.selection_rects_in_y_range(selection, f32::NEG_INFINITY, f32::INFINITY)
    }

    /// Selection geometry restricted to the visible vertical document band.
    ///
    /// Painting uses the row index to avoid scanning and allocating geometry
    /// for the entire document on every selection frame.
    pub(crate) fn selection_rects_in_y_range(
        &self,
        selection: PreviewSelection,
        top: f32,
        bottom: f32,
    ) -> Vec<PreviewRect> {
        let selected = selection.normalized();
        if selected.is_empty() || bottom < top {
            return Vec::new();
        }
        let first_row = self.rows.partition_point(|row| row.bottom < top);
        let last_row = self.rows.partition_point(|row| row.top <= bottom);
        if last_row <= first_row {
            return Vec::new();
        }
        let Some(first) = self.rows.get(first_row) else {
            return Vec::new();
        };
        let boxes_end = self
            .rows
            .get(last_row.saturating_sub(1))
            .map_or(first.start, |row| row.end);
        let rectangles = self.boxes[first.start..boxes_end]
            .iter()
            .filter(|item| ranges_intersect(&item.selection_range, &selected))
            .map(|item| item.rect)
            .collect::<Vec<_>>();
        merge_adjacent_rects(rectangles)
    }

    fn nearest_box_in_row(&self, row: &HitRow, x: f32, y: f32) -> Option<&PreviewTextBox> {
        let boxes = &self.boxes[row.start..row.end];
        let split = boxes.partition_point(|item| item.rect.x <= x);
        match (split.checked_sub(1), boxes.get(split)) {
            (Some(previous), Some(next)) => {
                let previous = &boxes[previous];
                if squared_distance(previous.rect, x, y) < squared_distance(next.rect, x, y) {
                    Some(previous)
                } else {
                    Some(next)
                }
            }
            (Some(previous), None) => boxes.get(previous),
            (None, Some(next)) => Some(next),
            (None, None) => None,
        }
    }

    #[cfg(test)]
    fn boxes_at_y(&self, y: f32) -> &[PreviewTextBox] {
        self.row_at_y(y)
            .map_or(&[][..], |row| &self.boxes[row.start..row.end])
    }

    fn row_at_y(&self, y: f32) -> Option<&HitRow> {
        let index = self.rows.partition_point(|row| row.bottom < y);
        self.rows.get(index).filter(|row| row.top <= y)
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

fn ranges_intersect(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

fn merge_adjacent_rects(rectangles: Vec<PreviewRect>) -> Vec<PreviewRect> {
    let mut merged: Vec<PreviewRect> = Vec::with_capacity(rectangles.len());
    for rectangle in rectangles {
        if let Some(previous) = merged.last_mut()
            && (previous.y - rectangle.y).abs() <= 0.5
            && (previous.height - rectangle.height).abs() <= 0.5
            && rectangle.x <= previous.right() + 0.5
        {
            let right = previous.right().max(rectangle.right());
            previous.x = previous.x.min(rectangle.x);
            previous.width = right - previous.x;
            continue;
        }
        merged.push(rectangle);
    }
    merged
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
    use unicode_segmentation::UnicodeSegmentation;

    use super::*;

    fn index() -> PreviewTextIndex {
        let text = "中文🙂abc";
        let widths = [24.0, 24.0, 30.0, 10.0, 10.0, 10.0];
        let mut x = 10.0;
        let boxes = text
            .grapheme_indices(true)
            .zip(widths)
            .map(|((start, grapheme), width)| {
                let end = start + grapheme.len();
                let item = PreviewTextBox {
                    selection_range: start..end,
                    source_range: None,
                    rect: PreviewRect {
                        x,
                        y: 10.0,
                        width,
                        height: 20.0,
                    },
                    action: None,
                    tooltip: None,
                    atomic: false,
                    start_x: x,
                    end_x: x + width,
                };
                x += width;
                item
            })
            .collect();
        PreviewTextIndex::new(Generation::initial(), text.into(), boxes, Vec::new())
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
    fn selection_geometry_uses_exact_cluster_widths() {
        let index = PreviewTextIndex::new(
            Generation::initial(),
            "中a🙂".into(),
            vec![
                PreviewTextBox {
                    selection_range: 0.."中".len(),
                    source_range: None,
                    rect: PreviewRect {
                        x: 10.0,
                        y: 10.0,
                        width: 48.0,
                        height: 20.0,
                    },
                    action: None,
                    tooltip: None,
                    atomic: false,
                    start_x: 10.0,
                    end_x: 58.0,
                },
                PreviewTextBox {
                    selection_range: "中".len().."中a".len(),
                    source_range: None,
                    rect: PreviewRect {
                        x: 58.0,
                        y: 10.0,
                        width: 12.0,
                        height: 20.0,
                    },
                    action: None,
                    tooltip: None,
                    atomic: false,
                    start_x: 58.0,
                    end_x: 70.0,
                },
                PreviewTextBox {
                    selection_range: "中a".len().."中a🙂".len(),
                    source_range: None,
                    rect: PreviewRect {
                        x: 70.0,
                        y: 10.0,
                        width: 30.0,
                        height: 20.0,
                    },
                    action: None,
                    tooltip: None,
                    atomic: false,
                    start_x: 70.0,
                    end_x: 100.0,
                },
            ],
            Vec::new(),
        );

        let rects = index.selection_rects(PreviewSelection {
            anchor: 0,
            active: "中".len(),
        });

        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].x, 10.0);
        assert_eq!(rects[0].width, 48.0);
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
                tooltip: None,
                atomic: false,
                start_x: 0.0,
                end_x: 100.0,
            })
            .collect();
        let index = PreviewTextIndex::new(Generation::initial(), "x".into(), boxes, Vec::new());
        assert_eq!(index.boxes_at_y(1234.0 * 20.0 + 4.0).len(), 1);
        assert_eq!(index.hit_test(60.0, 1234.0 * 20.0 + 4.0), 1);
    }

    #[test]
    fn viewport_selection_geometry_only_visits_visible_rows() {
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
                tooltip: None,
                atomic: false,
                start_x: 0.0,
                end_x: 100.0,
            })
            .collect();
        let index = PreviewTextIndex::new(Generation::initial(), "x".into(), boxes, Vec::new());
        let rects = index.selection_rects_in_y_range(
            PreviewSelection {
                anchor: 0,
                active: 1,
            },
            1_234.0 * 20.0,
            1_234.0 * 20.0 + 18.0,
        );
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].y, 1_234.0 * 20.0);
    }

    #[test]
    fn atomic_formula_hit_selection_and_tooltip_use_the_whole_object() {
        let index = PreviewTextIndex::new(
            Generation::initial(),
            "$x^2$".into(),
            vec![PreviewTextBox {
                selection_range: 0..5,
                source_range: None,
                rect: PreviewRect {
                    x: 10.0,
                    y: 10.0,
                    width: 80.0,
                    height: 24.0,
                },
                action: None,
                tooltip: Some(Arc::from("formula parse failed")),
                atomic: true,
                start_x: 10.0,
                end_x: 90.0,
            }],
            Vec::new(),
        );
        assert_eq!(index.hit_test(20.0, 20.0), 0);
        assert_eq!(index.hit_test(80.0, 20.0), 5);
        assert_eq!(
            index.selection_rects(PreviewSelection {
                anchor: 1,
                active: 2,
            }),
            vec![index.boxes()[0].rect]
        );
        assert_eq!(index.tooltip_at(20.0, 20.0), Some("formula parse failed"));
    }
}
