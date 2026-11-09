//! The Search labels against the machine's fonts. The layout lives in `stickymd-desktop`, whose
//! tests run on machines with none of these fonts, so this one check stays where they are known.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};
use stickymd_desktop::geometry::geometry;
use stickymd_desktop::search::SearchLayout;
use winit::dpi::PhysicalSize;

use stickymd_app::config::ViewMode;

/// `text`'s advance at the 13-DIP UI size in each installed UI font candidate, in physical
/// pixels, shaped as `SourceProjection::paint_ui_text` shapes a label: the machine's
/// first candidate is what it paints with, and any later one is what another machine
/// without the earlier ones paints with.
fn label_advances(fonts: &mut FontSystem, text: &str, scale: f32) -> Vec<(&'static str, f32)> {
    let mut advances = Vec::new();
    for &family in stickymd_render::source::CJK_CANDIDATES {
        let query = Query {
            families: &[Family::Name(family)],
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
        };
        if fonts.db().query(&query).is_none() {
            continue;
        }
        let mut buffer = Buffer::new_empty(Metrics::new(13.0 * scale, 20.0 * scale));
        buffer.set_wrap(Wrap::None);
        let attrs = Attrs::new().family(cosmic_text::Family::Name(family));
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(fonts, false);
        let advance = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max);
        advances.push((family, advance));
    }
    advances
}

/// Plan 07: each button's label, as Cosmic Text shapes it in every installed UI font candidate,
/// fits inside the button's label padding, in the real Source and Split views at six window sizes
/// in DIP and four DPIs (`stickymd_desktop::search`'s own test takes the same views).
#[test]
fn every_button_label_fits_in_each_installed_ui_font() {
    let mut fonts = FontSystem::new();
    let mut measured = 0;
    for scale in [1.0_f32, 1.25, 1.5, 2.0] {
        for (width, height) in [
            (520.0, 680.0),
            (220.0, 120.0),
            (330.0, 360.0),
            (396.0, 360.0),
            (398.0, 360.0),
            (1200.0, 900.0),
        ] {
            let size = PhysicalSize::new(
                (width * scale).round() as u32,
                (height * scale).round() as u32,
            );
            for mode in [ViewMode::Source, ViewMode::Split] {
                let pane = geometry(mode, size, scale).source.unwrap();
                for replace in [false, true] {
                    let layout = SearchLayout::new(pane, scale, replace);
                    let label = format!("{mode:?} {width}x{height} DIP at {scale}, {replace}");
                    for (text, rect, _) in layout.buttons(replace).into_iter().flatten() {
                        let spec = layout.label_spec(rect, scale);
                        for (family, advance) in label_advances(&mut fonts, text, scale) {
                            assert!(
                                advance > 0.0 && advance <= spec.width,
                                "{label}: {text:?} in {family} needs {} DIP of {}",
                                advance / scale,
                                spec.width / scale
                            );
                            measured += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(measured > 0, "no UI font candidate is installed");
}
