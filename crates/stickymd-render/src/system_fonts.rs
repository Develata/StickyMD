//! The installed-font database every projection is built on, and the default families a Platform
//! Host chooses for it.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates
//! plan_ref: docs/plan/02_positioning_and_scope.md#macos-host

use std::sync::OnceLock;

use cosmic_text::FontSystem;

/// The families the projections ask for, in preference order (plan 03, rule 9). A host that does
/// not choose gets [`FontDefaults::WINDOWS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontDefaults {
    pub cjk: &'static [&'static str],
    pub latin: &'static [&'static str],
    pub code: &'static str,
}

impl FontDefaults {
    pub const WINDOWS: Self = Self {
        cjk: crate::source::CJK_CANDIDATES,
        latin: &["Times New Roman", "Georgia"],
        code: "Consolas",
    };

    pub const MACOS: Self = Self {
        cjk: &["PingFang SC"],
        latin: &["Times New Roman", "Georgia"],
        code: "Menlo",
    };
}

static DEFAULTS: OnceLock<FontDefaults> = OnceLock::new();

/// Called once by the host before any projection exists; a later call changes nothing and returns
/// `false`, because projections already built would disagree with new ones.
pub fn set_font_defaults(defaults: FontDefaults) -> bool {
    DEFAULTS.set(defaults).is_ok()
}

pub(crate) fn font_defaults() -> &'static FontDefaults {
    DEFAULTS.get().unwrap_or(&FontDefaults::WINDOWS)
}

/// The font system over the installed fonts, without the faces that cannot be laid out.
pub fn new_font_system() -> FontSystem {
    let fonts = FontSystem::new();
    #[cfg(target_os = "macos")]
    let fonts = without_faces_lacking_head(fonts);
    fonts
}

/// macOS ships a bitmap-only face (`GB18030 Bitmap`) whose `bhed` replaces `head`: with no units
/// per em its advances are infinite, and because it is marked monospaced it is chosen as the CJK
/// fallback in code. The set of faces decides the fallback lists, which are computed when the
/// font system is built, so the system is rebuilt rather than edited.
#[cfg(target_os = "macos")]
fn without_faces_lacking_head(fonts: FontSystem) -> FontSystem {
    let (locale, mut db) = fonts.into_locale_and_db();
    let unusable: Vec<_> = db
        .faces()
        .filter(|face| {
            // A face that cannot be read is cosmic-text's to skip; only a proven lack removes it.
            db.with_face_data(face.id, has_head_table) == Some(false)
        })
        .map(|face| face.id)
        .collect();
    for id in unusable {
        db.remove_face(id);
    }
    FontSystem::new_with_locale_and_db(locale, db)
}

#[cfg(target_os = "macos")]
fn has_head_table(data: &[u8], index: u32) -> bool {
    let u32_at = |at: usize| -> Option<usize> {
        let bytes = data.get(at..at + 4)?;
        Some(u32::from_be_bytes(bytes.try_into().ok()?) as usize)
    };
    let directory = if data.get(..4) == Some(b"ttcf".as_slice()) {
        match u32_at(12 + 4 * index as usize) {
            Some(offset) => offset,
            None => return false,
        }
    } else {
        0
    };
    let Some(count) = data.get(directory + 4..directory + 6) else {
        return false;
    };
    let tables = usize::from(u16::from_be_bytes([count[0], count[1]]));
    (0..tables).any(|table| {
        let entry = directory + 12 + 16 * table;
        data.get(entry..entry + 4) == Some(b"head".as_slice())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_that_does_not_choose_gets_the_windows_families() {
        assert_eq!(FontDefaults::WINDOWS.code, "Consolas");
        assert_eq!(FontDefaults::WINDOWS.cjk, crate::source::CJK_CANDIDATES);
    }

    #[cfg(target_os = "macos")]
    mod macos {
        use super::super::*;

        fn sfnt(tags: &[&[u8; 4]]) -> Vec<u8> {
            let mut data = vec![0, 1, 0, 0, 0, tags.len() as u8, 0, 0, 0, 0, 0, 0];
            for tag in tags {
                data.extend_from_slice(*tag);
                data.extend_from_slice(&[0; 12]);
            }
            data
        }

        #[test]
        fn a_face_is_usable_only_with_a_head_table() {
            assert!(has_head_table(&sfnt(&[b"cmap", b"head"]), 0));
            assert!(!has_head_table(&sfnt(&[b"bhed", b"cmap"]), 0));
            assert!(!has_head_table(&[0, 1], 0));
        }

        #[test]
        fn a_collection_is_read_at_the_requested_face() {
            let face = sfnt(&[b"head"]);
            let mut collection = b"ttcf".to_vec();
            collection.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 1]);
            collection.extend_from_slice(&16u32.to_be_bytes());
            collection.extend_from_slice(&face);
            assert!(has_head_table(&collection, 0));
            assert!(!has_head_table(&collection, 7));
        }

        /// The face behind the baseline failures: installed on this system, and gone from ours.
        #[test]
        fn the_system_database_has_no_face_that_lacks_head() {
            let fonts = new_font_system();
            let db = fonts.db();
            for face in db.faces() {
                let usable = db.with_face_data(face.id, has_head_table);
                assert_ne!(usable, Some(false), "{}", face.post_script_name);
            }
        }
    }
}
