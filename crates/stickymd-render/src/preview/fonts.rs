//! Preview-only code font policy; ordinary text keeps its script-based families.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::FontSystem;
use cosmic_text::fontdb::{Family, Query};

pub(super) fn configure_code_font(fonts: &mut FontSystem) {
    let query = Query {
        families: &[Family::Name("Consolas")],
        ..Query::default()
    };
    if fonts.db().query(&query).is_some() {
        fonts.db_mut().set_monospace_family("Consolas");
    }
    // Missing fonts retain the platform's existing monospace fallback. No font
    // download, embedded proprietary font, or change to Source's database.
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

    fn code_faces(fonts: &mut FontSystem) -> Vec<cosmic_text::fontdb::ID> {
        let mut buffer = Buffer::new(fonts, Metrics::new(15.3, 23.0));
        buffer.set_size(Some(500.0), None);
        buffer.set_text(
            "plain code 0123456789",
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(fonts, false);
        buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.font_id))
            .collect()
    }

    #[test]
    #[cfg(windows)]
    fn phase5_code_shaping_selects_real_consolas_faces() {
        let mut fonts = FontSystem::new();
        configure_code_font(&mut fonts);
        let faces = code_faces(&mut fonts);
        assert!(!faces.is_empty());
        for id in faces {
            assert!(
                fonts
                    .db()
                    .face(id)
                    .unwrap()
                    .families
                    .iter()
                    .any(|(name, _)| name == "Consolas")
            );
        }
    }

    #[test]
    fn phase5_code_font_missing_keeps_existing_monospace_fallback() {
        let mut db = FontSystem::new().db().clone();
        let removed: Vec<_> = db
            .faces()
            .filter(|face| face.families.iter().any(|(name, _)| name == "Consolas"))
            .map(|face| face.id)
            .collect();
        for id in removed {
            db.remove_face(id);
        }
        let expected = db.family_name(&Family::Monospace).to_owned();
        let has_fonts = !db.is_empty();
        let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        configure_code_font(&mut fonts);
        assert_eq!(fonts.db().family_name(&Family::Monospace), expected);
        if has_fonts {
            let faces = code_faces(&mut fonts);
            assert!(!faces.is_empty());
            assert!(faces.iter().all(|id| {
                fonts
                    .db()
                    .face(*id)
                    .unwrap()
                    .families
                    .iter()
                    .all(|(name, _)| name != "Consolas")
            }));
        }
    }
}
