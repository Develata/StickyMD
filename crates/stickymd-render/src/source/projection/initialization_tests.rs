//! Compare Source initialization with eagerly shaped empty buffers.
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use super::*;
use crate::source::{SourceTheme, UiTextSpec};
use std::sync::Arc;
use stickymd_core::LineEnding;
use tiny_skia::Pixmap;

fn eager_reference(snapshot: &DocumentSnapshot) -> SourceProjection {
    let mut reference = SourceProjection::new(snapshot, 480, 240, 1.0);
    // Recreate the old eager initialization with the same actual font database.
    // Only the reference performs the discarded empty shaping calls.
    reference.buffer = Buffer::new(&mut reference.font_system, scaled_metrics(1.0));
    reference.diagnostic_buffer = Buffer::new(&mut reference.font_system, Metrics::new(13.0, 20.0));
    reference.diagnostic_buffer.set_wrap(Wrap::None);
    reference.ui_buffer = Buffer::new(&mut reference.font_system, Metrics::new(13.0, 20.0));
    reference.ui_buffer.set_wrap(Wrap::None);
    reference.rebuild_buffer(snapshot);
    reference
        .buffer
        .shape_until_scroll(&mut reference.font_system, false);
    reference
}

#[test]
fn source_initialization_matches_eager_buffers_for_pixels_carets_and_auxiliary_text() {
    for text in ["", "\n", "中文 Latin e\u{301} 🙂\nثابت text\n"] {
        let snapshot = DocumentSnapshot {
            text: Arc::from(text),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut actual = SourceProjection::new(&snapshot, 480, 240, 1.0);
        let mut reference = eager_reference(&snapshot);
        for (width, height, scale) in [(480, 240, 1.0), (260, 300, 1.5), (440, 200, 0.5)] {
            for projection in [&mut actual, &mut reference] {
                projection.set_viewport(width, height, scale);
            }
            for diagnostic in [None, Some(""), Some("保存失败 [F6 重试] [F7 取消]")] {
                let mut actual_pixels = Pixmap::new(width, height).unwrap();
                let mut expected_pixels = actual_pixels.clone();
                for (projection, pixels) in [
                    (&mut actual, &mut actual_pixels),
                    (&mut reference, &mut expected_pixels),
                ] {
                    projection
                        .paint(
                            pixels,
                            Selection::new(0, text.len()),
                            true,
                            true,
                            diagnostic,
                            SourceTheme::Light,
                        )
                        .unwrap();
                }
                assert_eq!(
                    actual_pixels.data(),
                    expected_pixels.data(),
                    "{text:?} {diagnostic:?}"
                );
            }
            for byte in [0, text.len()] {
                assert_eq!(actual.caret_rect(byte), reference.caret_rect(byte));
            }
            for x in (0..width).step_by(19) {
                assert_eq!(
                    actual.hit_test(x as f32, 25.0),
                    reference.hit_test(x as f32, 25.0)
                );
            }
            let spec = UiTextSpec {
                x: 12.0,
                y: 10.0,
                width: 100.0,
                scale,
            };
            for field in ["", "查找 e\u{301} 🙂 long text"] {
                let mut actual_pixels = Pixmap::new(width, height).unwrap();
                let mut expected_pixels = actual_pixels.clone();
                assert_eq!(
                    actual.paint_ui_text_field(
                        &mut actual_pixels,
                        field,
                        field.len(),
                        spec,
                        SourceTheme::Dark
                    ),
                    reference.paint_ui_text_field(
                        &mut expected_pixels,
                        field,
                        field.len(),
                        spec,
                        SourceTheme::Dark
                    ),
                );
                assert_eq!(actual_pixels.data(), expected_pixels.data());
                assert_eq!(
                    actual.ui_text_field_hit(field, field.len(), spec, 50.0),
                    reference.ui_text_field_hit(field, field.len(), spec, 50.0)
                );
            }
            assert_eq!(actual.projected_text(), text);
            assert_eq!(actual.projected_generation(), snapshot.generation);
        }
    }
}
