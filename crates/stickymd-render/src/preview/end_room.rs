//! Where the Preview's scrolling ends (plan 09 #vertical-scrollbars, Phase 17): past the last
//! line by half a viewport, so the last line can leave the notice bar. A frame computes its end
//! here once, when it is painted; painting, the wheel, the thumb and its hiding, a view change and
//! Split sync all read that one value from the frame.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

/// The scroll end of one Preview frame, in content pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewEnd {
    /// The largest offset: 0 when the document does not scroll.
    pub max_scroll: f32,
    /// The height the thumb measures against: the document and its end room, or the viewport
    /// when the document does not scroll.
    pub scrollable_height: f32,
}

impl PreviewEnd {
    /// The end of a document `document_height` high in a viewport `viewport_height` high, with
    /// a notice bar `notice_bar_height` high over its bottom.
    ///
    /// Precondition: the heights are finite and not negative. Postconditions: `max_scroll` is 0
    /// exactly when the document fits above the notice bar; otherwise it is
    /// `document_height + R - viewport_height` with `R = viewport_height / 2`, and
    /// `scrollable_height - viewport_height == max_scroll`. At the end the last line stops `R`
    /// above the viewport's bottom, and `R` is never under the bar at any viewport the window
    /// allows (plan 09), so the last line clears it. The jump at the threshold, from 0 to
    /// `R - notice_bar_height` or more, is accepted by plan 09.
    pub fn new(document_height: f32, viewport_height: f32, notice_bar_height: f32) -> Self {
        let viewport = viewport_height.max(0.0);
        if document_height <= viewport - notice_bar_height.max(0.0) {
            return Self {
                max_scroll: 0.0,
                scrollable_height: viewport,
            };
        }
        let room = viewport / 2.0;
        let max_scroll = (document_height + room - viewport).max(0.0);
        Self {
            max_scroll,
            scrollable_height: viewport + max_scroll,
        }
    }

    /// `offset` kept between the document start and this end.
    pub fn clamp(self, offset: f32) -> f32 {
        offset.clamp(0.0, self.max_scroll)
    }

    pub fn scrolls(self) -> bool {
        self.max_scroll > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P17-E01: a document that fits above the notice bar does not scroll, and its thumb would
    /// show the whole height.
    #[test]
    fn a_document_that_fits_above_the_bar_does_not_scroll() {
        for document in [0.0, 1.0, 300.0, 566.0] {
            let end = PreviewEnd::new(document, 600.0, 34.0);
            assert_eq!(end.max_scroll, 0.0, "{document}");
            assert!(!end.scrolls());
            assert_eq!(end.scrollable_height, 600.0);
            assert_eq!(end.clamp(50.0), 0.0);
        }
    }

    /// P17-E02: past the threshold the end is half a viewport below the last line, and the
    /// thumb's height is the document and that room.
    #[test]
    fn past_the_threshold_the_end_is_half_a_viewport_past_the_last_line() {
        let end = PreviewEnd::new(567.0, 600.0, 34.0);
        assert_eq!(end.max_scroll, 567.0 + 300.0 - 600.0);
        let end = PreviewEnd::new(2_000.0, 600.0, 34.0);
        assert_eq!(end.max_scroll, 1_700.0);
        assert_eq!(end.scrollable_height, 2_300.0);
        assert_eq!(end.clamp(5_000.0), 1_700.0);
        assert_eq!(end.clamp(-3.0), 0.0);
        // A document as high as the viewport scrolls now, by the room.
        assert_eq!(PreviewEnd::new(600.0, 600.0, 34.0).max_scroll, 300.0);
    }

    /// P17-E02: painting clamps to the same end, so an offset past it is painted at the end and
    /// handed back as the frame's offset.
    #[test]
    fn a_frame_is_painted_at_its_own_end() {
        let text: String = (0..60).map(|n| format!("paragraph {n}\n\n")).collect();
        let document =
            stickymd_core::DocumentState::loaded(&text, stickymd_core::LineEnding::Lf, None);
        let mut pipeline = crate::preview::PreviewPipeline::new();
        pipeline.set_notice_bar_height(34.0);
        let frame = pipeline
            .build(
                &document.snapshot(),
                400,
                300,
                1.0,
                f32::MAX,
                crate::preview::PreviewSelection::default(),
                crate::preview::PreviewTheme::Light,
            )
            .unwrap();
        let end = frame.end();
        assert_eq!(end, PreviewEnd::new(frame.document_height(), 300.0, 34.0));
        assert!(end.max_scroll > frame.document_height() - 300.0);
        assert_eq!(frame.scroll_y(), end.max_scroll);
    }

    /// At the end the last line clears the bar: the room below it is never under the bar, at
    /// the smallest viewport too (86 DIP of content under a 34 DIP bar, plan 09).
    #[test]
    fn the_room_is_never_less_than_the_bar() {
        for scale in [1.0_f32, 1.5, 2.0, 3.0] {
            let viewport = 86.0 * scale;
            let bar = 34.0 * scale;
            for document in [viewport - bar + 1.0, viewport, viewport * 10.0] {
                let end = PreviewEnd::new(document, viewport, bar);
                let room_below_last_line = end.max_scroll + viewport - document;
                assert!(room_below_last_line >= bar, "{scale} {document}: {end:?}");
            }
        }
    }
}
