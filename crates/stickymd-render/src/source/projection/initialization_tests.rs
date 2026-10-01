//! Compare Source initialization with eager buffers and generic Serif defaults.
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use super::*;
use crate::source::{SourceTheme, UiTextSpec};
use std::sync::Arc;
use stickymd_core::{CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, LineEnding};
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
    // Reconstruct the old default plus explicit script spans, including spans
    // now redundant with the selected Latin default. Empty lines deliberately
    // retain generic Serif here so the comparison does not follow production.
    let runs = segment_script_runs(&snapshot.text);
    let mut offset = 0;
    for line in &mut reference.buffer.lines {
        let mut attrs = AttrsList::new(&Attrs::new().family(Family::Serif));
        let end = offset + line.text().len();
        for run in &runs {
            let start = run.range.start.max(offset);
            let stop = run.range.end.min(end);
            if start < stop {
                attrs.add_span(
                    start - offset..stop - offset,
                    &Attrs::new().family(Family::Name(reference.fonts.family_for(run.class))),
                );
            }
        }
        line.set_attrs_list(attrs);
        offset = end + usize::from(line.ending() != BufferLineEnding::None);
    }
    reference
        .buffer
        .shape_until_scroll(&mut reference.font_system, false);
    reference
}

#[test]
fn source_initialization_matches_eager_buffers_for_pixels_carets_and_auxiliary_text() {
    for text in [
        "",
        "\n",
        "\n\nEnglish office …\n\n中文 paragraph\n\n",
        "  \t  \n\n...中文\n   Latin\n",
        "中文 Latin e\u{301} 🙂\nثابت text\n",
    ] {
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
            for byte in text
                .char_indices()
                .map(|(byte, _)| byte)
                .chain([text.len()])
            {
                assert_eq!(actual.caret_rect(byte), reference.caret_rect(byte));
            }
            for y in (0..height).step_by(29) {
                for x in (0..width).step_by(19) {
                    assert_eq!(
                        actual.hit_test(x as f32, y as f32),
                        reference.hit_test(x as f32, y as f32)
                    );
                }
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

#[test]
fn source_defaults_preserve_generic_fallback_when_preferred_latin_is_unavailable() {
    for (family, found, expected) in [
        ("Times New Roman", true, Family::Name("Times New Roman")),
        ("Georgia", true, Family::Name("Georgia")),
        ("Unavailable Latin family", false, Family::Serif),
    ] {
        let fonts = FontSelection {
            cjk_family: "Unavailable CJK family",
            cjk_found: false,
            latin_family: family,
            latin_found: found,
        };
        let mut buffer = Buffer::new_empty(scaled_metrics(1.0));
        set_source_buffer_text(&mut buffer, "\n中 Latin\n", &fonts);
        for line in &buffer.lines {
            assert_eq!(line.attrs_list().defaults().family, expected);
        }
        assert_eq!(attrs_for_line("", &fonts).defaults().family, expected);
        assert_eq!(
            buffer.lines[1].attrs_list().get_span(0).family,
            Family::Name(fonts.cjk_family)
        );
        assert_eq!(
            buffer.lines[1].attrs_list().get_span("中 ".len()).family,
            Family::Name(family)
        );
    }
}

#[test]
fn source_defaults_survive_edits_and_snapshot_resynchronization() {
    let mut document = DocumentState::loaded("\nLatin\n", LineEnding::Lf, None);
    let mut projection = SourceProjection::new(&document.snapshot(), 480, 240, 1.0);
    let expected = source_default_attrs(&projection.fonts).family;
    for inserted in ["中文\n\n", "", "Latin", "\n"] {
        let outcome = document
            .edit(EditRequest::new(
                document.generation(),
                0..document.text().len(),
                inserted,
                CursorSnapshot::caret(0),
                CursorSnapshot::caret(inserted.len()),
                EditMeta::new(EditKind::Paste, 10),
            ))
            .unwrap();
        projection
            .apply_delta(document.generation(), outcome.delta.as_ref().unwrap())
            .unwrap();
        for line in &projection.buffer.lines {
            assert_eq!(line.attrs_list().defaults().family, expected);
        }
        projection.resync(&document.snapshot()).unwrap();
        for line in &projection.buffer.lines {
            assert_eq!(line.attrs_list().defaults().family, expected);
        }
        assert_eq!(projection.projected_text(), document.text());
        assert_eq!(projection.projected_generation(), document.generation());
    }
}
