//! Lazy source scrollbar bounds; logical lines remain the scroll coordinate.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use cosmic_text::Scroll;
use stickymd_core::Generation;

use super::SourceProjection;

#[derive(Debug, Clone, Copy)]
pub struct SourceScrollMetrics {
    pub fraction: f64,
    pub viewport_fraction: f64,
}

#[derive(Clone, Copy)]
pub(super) struct SourceScrollExtent {
    key: (Generation, u32, u32, u32),
    max_position: f64,
    viewport_fraction: f64,
}

impl SourceProjection {
    /// Measures only the tail viewport on a generation/geometry change. In
    /// particular, jumping across a long note never shapes intervening lines.
    pub fn scroll_metrics(&mut self) -> SourceScrollMetrics {
        let extent = self.source_scroll_extent();
        let scroll = self.buffer.scroll();
        let height = self.scroll_line_height(scroll.line);
        let position = scroll.line as f64 + f64::from(scroll.vertical / height);
        SourceScrollMetrics {
            fraction: if extent.max_position > 0.0 {
                (position / extent.max_position).clamp(0.0, 1.0)
            } else {
                0.0
            },
            viewport_fraction: extent.viewport_fraction,
        }
    }

    pub fn scroll_to_fraction(&mut self, fraction: f64) -> Scroll {
        let extent = self.source_scroll_extent();
        let position = fraction.clamp(0.0, 1.0) * extent.max_position;
        let line = (position.floor() as usize).min(self.buffer.lines.len().saturating_sub(1));
        let height = self.scroll_line_height(line);
        let vertical = ((position - line as f64) as f32 * height).max(0.0);
        self.buffer.set_scroll(Scroll::new(line, vertical, 0.0));
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        self.buffer.scroll()
    }

    fn source_scroll_extent(&mut self) -> SourceScrollExtent {
        let key = (
            self.generation,
            self.width_px,
            self.height_px,
            self.scale_factor.to_bits(),
        );
        if let Some(cached) = self.scroll_extent.filter(|cached| cached.key == key) {
            return cached;
        }
        #[cfg(test)]
        {
            self.extent_measures += 1;
        }
        let count = self.buffer.lines.len().max(1);
        let mut line = count - 1;
        let mut remaining = self.content_height();
        let max_position = loop {
            let height = self.scroll_line_height(line);
            if height >= remaining {
                break line as f64 + f64::from((height - remaining) / height);
            }
            if line == 0 {
                break 0.0;
            }
            remaining -= height;
            line -= 1;
        };
        let extent = SourceScrollExtent {
            key,
            max_position,
            viewport_fraction: ((count as f64 - max_position) / count as f64).clamp(0.0, 1.0),
        };
        self.scroll_extent = Some(extent);
        extent
    }

    fn scroll_line_height(&mut self, line: usize) -> f32 {
        // Source uses one fixed line height, including script-specific fonts;
        // no per-span height overrides. Cached wrap count is O(1) to query.
        let line_height = self.buffer.metrics().line_height;
        self.buffer
            .line_layout(&mut self.font_system, line)
            .map_or(line_height, |layout| {
                layout.len().max(1) as f32 * line_height
            })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stickymd_core::{DocumentSnapshot, LineEnding};

    use super::*;

    fn projection(text: &str, width: u32, height: u32) -> SourceProjection {
        SourceProjection::new(
            &DocumentSnapshot {
                text: Arc::from(text),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            width,
            height,
            1.0,
        )
    }

    #[test]
    fn scrollbar_jumps_to_both_ends_without_shaping_intervening_lines() {
        let text = "行 text\n".repeat(10_000);
        let mut source = projection(&text, 300, 220);
        let generation = source.projected_generation();
        assert!(source.scroll_metrics().viewport_fraction < 0.01);
        source.scroll_to_fraction(1.0);
        // The document end is shown whole, not merely laid out at the bottom edge.
        let end = source
            .caret_rect(text.len())
            .expect("the last line is laid out");
        assert!(
            end.y >= 0.0 && end.y + end.height <= 220.0,
            "the last line {end:?} lies inside the 220 px viewport"
        );
        assert!((source.scroll_metrics().fraction - 1.0).abs() < 0.00001);
        source.scroll_to_fraction(0.0);
        assert_eq!(source.scroll().line, 0);
        assert_eq!(source.scroll().vertical, 0.0);
        assert!(
            source
                .buffer
                .lines
                .iter()
                .filter(|line| line.layout_opt().is_some())
                .count()
                < 40
        );
        assert_eq!(source.projected_text(), text);
        assert_eq!(source.projected_generation(), generation);
    }

    fn laid_out_lines(source: &SourceProjection) -> usize {
        source
            .buffer
            .lines
            .iter()
            .filter(|line| line.layout_opt().is_some())
            .count()
    }

    /// Plan 09 Source mapping, on a long note whose every logical line wraps: the thumb
    /// position is the logical line plus the offset inside its wrapped rows (moving to a
    /// fraction and reading it back agree, and some positions land inside a line); measuring
    /// the end lays out only the lines of the last screen and caches that boundary, reused by
    /// later measures until the geometry changes; and each pointer-event-like jump lays out
    /// only around its target, never the whole note.
    #[test]
    fn the_source_maps_wrapped_offsets_and_measures_only_the_tail_once() {
        let line = "wrapped 中文 source line that is long enough to wrap twice or more ".repeat(3);
        let text = format!("{line}\n").repeat(2000);
        let mut source = projection(&text, 300, 220);
        let total = source.buffer.lines.len();
        assert!(total >= 2000);
        let metrics = source.scroll_metrics();
        assert!(metrics.viewport_fraction < 0.01);
        let after_measure = laid_out_lines(&source);
        assert!(
            after_measure < 40,
            "the end measure laid out {after_measure} of {total} lines"
        );
        let key = source
            .scroll_extent
            .map(|extent| (extent.key, extent.max_position));
        assert!(key.is_some(), "the boundary is cached");
        assert_eq!(source.extent_measures, 1);
        // Repeated measures reuse it and lay out nothing more.
        for _ in 0..5 {
            source.scroll_metrics();
        }
        assert_eq!(source.extent_measures, 1, "the boundary was measured again");
        assert_eq!(laid_out_lines(&source), after_measure);
        assert_eq!(
            source
                .scroll_extent
                .map(|extent| (extent.key, extent.max_position)),
            key
        );
        let max_position = key.unwrap().1;
        // A pointer drag is a run of jumps; each lays out only around its target.
        let mut inside_a_line = false;
        for step in 0..=60 {
            let fraction = f64::from(step) / 60.0;
            let before = laid_out_lines(&source);
            let scroll = source.scroll_to_fraction(fraction);
            let added = laid_out_lines(&source) - before;
            assert!(
                added <= 20,
                "a jump to {fraction} laid out {added} more lines"
            );
            inside_a_line |= scroll.vertical > 0.0;
            let read_back = source.scroll_metrics().fraction;
            // The offset inside a wrapped line counts: a thousandth of a line at most is lost.
            assert!(
                (read_back - fraction).abs() <= 1e-3 / max_position,
                "{fraction} read back as {read_back}"
            );
        }
        assert!(inside_a_line, "some positions fall inside a wrapped line");
        assert_eq!(source.extent_measures, 1, "a drag reuses the boundary");
        assert!(
            laid_out_lines(&source) < total / 4,
            "the drag never laid out the whole note"
        );
        // A new geometry measures the end again under a new key.
        source.set_viewport(280, 220, 1.0);
        source.scroll_metrics();
        assert_eq!(source.extent_measures, 2);
        assert_ne!(
            source.scroll_extent.map(|extent| extent.key),
            key.map(|(key, _)| key)
        );
    }

    /// Plan 09 track paging in Source: a page (the scrollbar's 90% of the pane) moves the
    /// viewport down or up by exactly its pixels, and paging stops at the document start and at
    /// the end, where the last line is shown whole.
    #[test]
    fn a_source_page_moves_the_viewport_by_its_pixels_within_the_document() {
        let text = "行 text
"
        .repeat(400);
        let mut source = projection(&text, 300, 220);
        let line_height = source.buffer.metrics().line_height;
        let position = |source: &SourceProjection| {
            let scroll = source.scroll();
            scroll.line as f32 * line_height + scroll.vertical
        };
        let page = 0.9 * 220.0;
        let mut previous = position(&source);
        assert_eq!(previous, 0.0);
        let mut pages = 0;
        loop {
            source.scroll_by(page);
            let now = position(&source);
            if now == previous {
                break;
            }
            let moved = now - previous;
            assert!(
                moved > 0.0 && moved <= page + 0.01,
                "a page down moved {moved}"
            );
            if moved < page - 0.01 {
                // Only the last page may be short: it stops at the end.
                source.scroll_by(page);
                assert_eq!(position(&source), now, "paging continued past the end");
                previous = now;
                break;
            }
            previous = now;
            pages += 1;
            assert!(pages < 1000);
        }
        assert!(pages > 10, "the note spans many pages");
        let end = source.caret_rect(text.len()).expect("the end is shown");
        assert!(end.y >= 0.0 && end.y + end.height <= 220.0 + 0.1);
        loop {
            source.scroll_by(-page);
            let now = position(&source);
            let moved = previous - now;
            // Every page up moves by exactly its pixels, except the one that reaches the start.
            assert!(
                (moved - page).abs() <= 0.01 || (now == 0.0 && moved <= page + 0.01),
                "a page up moved {moved}"
            );
            if now == previous || now == 0.0 {
                break;
            }
            previous = now;
        }
        source.scroll_by(-page);
        assert_eq!(position(&source), 0.0, "paging stops at the start");
        assert_eq!(source.scroll().line, 0);
    }

    #[test]
    fn scrollbar_handles_a_single_wrapped_line_and_resized_short_notes() {
        let text = "中文 e\u{301} text ".repeat(200);
        // The real layout, read through `caret_rect`, checks the wrapped line's height the
        // mapping uses, at 1x and at 2x (where a pixel / DIP mix-up would halve it).
        for scale in [1.0_f32, 2.0] {
            let side = (160.0 * scale) as u32;
            let mut source = projection(&text, side, side);
            source.set_viewport(side, side, scale);
            assert!(source.scroll_metrics().viewport_fraction < 1.0, "{scale}");
            source.scroll_to_fraction(0.5);
            assert!(source.scroll().vertical > 0.0, "{scale}");
            assert!(
                (source.scroll_metrics().fraction - 0.5).abs() < 0.001,
                "{scale}"
            );
            source.scroll_to_fraction(1.0);
            let last = source
                .caret_rect(text.len())
                .expect("last wrapped row visible");
            assert!(
                last.y >= 0.0 && last.y + last.height <= source.height_px as f32 + 0.1,
                "{scale}: the last row at {last:?} is outside the viewport"
            );
            assert!(
                (source.scroll_metrics().fraction - 1.0).abs() < 0.001,
                "{scale}"
            );
            // Back at the start the single logical line is shown from its first row.
            source.scroll_to_fraction(0.0);
            assert_eq!(source.scroll().line, 0);
            assert_eq!(source.scroll().vertical, 0.0);
            let first = source.caret_rect(0).expect("first wrapped row visible");
            assert!(first.y >= 0.0 && first.y + first.height <= source.height_px as f32 + 0.1);
        }
        // The long line's bounds are measured and cached first, so the short note after it
        // proves the cache is dropped on a new generation.
        let mut source = projection(&text, 160, 160);
        assert!(source.scroll_metrics().viewport_fraction < 1.0);
        source
            .resync(&DocumentSnapshot {
                text: Arc::from("short"),
                generation: Generation::initial().checked_next().unwrap(),
                line_ending: LineEnding::Lf,
            })
            .unwrap();
        assert_eq!(source.scroll_metrics().viewport_fraction, 1.0);
        source.set_viewport(80, 40, 2.0);
        assert!(source.scroll_metrics().viewport_fraction < 1.0);
        // A note that fits the 2x content height is fully visible: the available height is
        // physical pixels like the line heights (a DIP mix-up would show it a thumb).
        let mut three = projection("a\nb\nc", 320, 320);
        three.set_viewport(320, 320, 2.0);
        assert_eq!(three.scroll_metrics().viewport_fraction, 1.0);
        let mut empty = projection("", 300, 220);
        assert_eq!(empty.scroll_metrics().viewport_fraction, 1.0);
        empty.scroll_to_fraction(1.0);
        assert_eq!(empty.scroll().line, 0);
    }

    #[test]
    fn scrollbar_bounds_follow_edits_and_preserve_composition() {
        use stickymd_core::{
            CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, Selection,
        };
        let mut document = DocumentState::loaded("short", LineEnding::Lf, None);
        let mut source = SourceProjection::new(&document.snapshot(), 300, 220, 1.0);
        assert_eq!(source.scroll_metrics().viewport_fraction, 1.0);
        for inserted in [" and wrapped text".repeat(200), "\nnew line".repeat(400)] {
            let end = document.text().len();
            let outcome = document
                .edit(EditRequest::new(
                    document.generation(),
                    end..end,
                    &inserted,
                    CursorSnapshot::caret(end),
                    CursorSnapshot::caret(end + inserted.len()),
                    EditMeta::new(EditKind::Paste, 1),
                ))
                .unwrap();
            source
                .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
                .unwrap();
            assert!(source.scroll_metrics().viewport_fraction < 1.0);
            let preedit = super::super::PreeditVisual {
                text: "nihao".to_owned(),
                cursor: Some(2..2),
                replacement: Selection::new(0, 3),
            };
            source.set_preedit(Some(preedit.clone()));
            source.scroll_to_fraction(1.0);
            assert!(source.caret_rect(document.text().len()).is_some());
            assert_eq!(source.preedit(), Some(&preedit));
            assert_eq!(source.projected_generation(), document.generation());
            assert_eq!(source.projected_text(), document.text());
        }
    }

    #[test]
    #[ignore = "Release-only 1 MiB scrollbar jump and source paint timing"]
    fn scrollbar_release_baseline() {
        use super::super::SourceTheme;
        use std::time::{Duration, Instant};
        use stickymd_core::Selection;
        let text = "中文 Source scrollbar long-note jump and paint.\n".repeat(22_000);
        assert!(text.len() >= 1024 * 1024);
        let mut source = projection(&text, 500, 646);
        let mut pixmap = tiny_skia::Pixmap::new(500, 646).unwrap();
        let mut samples = Vec::with_capacity(100);
        for index in 0..100 {
            let start = Instant::now();
            source.scroll_to_fraction(f64::from((index * 37) % 100) / 99.0);
            source.scroll_metrics();
            source
                .paint(
                    &mut pixmap,
                    Selection::caret(0),
                    false,
                    false,
                    None,
                    SourceTheme::Light,
                )
                .unwrap();
            samples.push(start.elapsed());
        }
        samples.sort_unstable();
        println!(
            "scrollbar source bytes={} samples=100 median={:?} p95={:?} max={:?}",
            text.len(),
            samples[49],
            samples[94],
            samples[99]
        );
        assert!(samples[94] < Duration::from_millis(50));
    }

    #[test]
    fn scrollbar_metrics_do_not_change_visible_text_or_scroll() {
        use super::super::SourceTheme;
        use stickymd_core::Selection;
        let text = format!(
            "# Scrollbar test document\n\n{}",
            (1..1600)
                .map(|i| format!("Line {i} - source and preview scrollbar verification.\n\n"))
                .collect::<String>()
        );
        let mut source = projection(&text, 880, 646);
        let mut before = tiny_skia::Pixmap::new(880, 646).unwrap();
        source
            .paint(
                &mut before,
                Selection::caret(0),
                false,
                false,
                None,
                SourceTheme::Light,
            )
            .unwrap();
        let mut wide = projection(&text, 900, 646);
        let mut wide_pixels = tiny_skia::Pixmap::new(900, 646).unwrap();
        wide.paint(
            &mut wide_pixels,
            Selection::caret(0),
            false,
            false,
            None,
            SourceTheme::Light,
        )
        .unwrap();
        for (row, wide_row) in before
            .data()
            .chunks_exact(880 * 4)
            .zip(wide_pixels.data().chunks_exact(900 * 4))
        {
            assert_eq!(
                row,
                &wide_row[..880 * 4],
                "reserved gutter changed unwrapped text pixels"
            );
        }
        let scroll = source.scroll();
        source.scroll_metrics();
        assert_eq!(source.scroll(), scroll);
        let mut after = before.clone();
        source
            .paint(
                &mut after,
                Selection::caret(0),
                false,
                false,
                None,
                SourceTheme::Light,
            )
            .unwrap();
        assert_eq!(before.data(), after.data());
        source.scroll_to_fraction(1.0);
        source.scroll_to_fraction(0.0);
        source
            .paint(
                &mut after,
                Selection::caret(0),
                false,
                false,
                None,
                SourceTheme::Light,
            )
            .unwrap();
        assert_eq!(before.data(), after.data());
    }
}
