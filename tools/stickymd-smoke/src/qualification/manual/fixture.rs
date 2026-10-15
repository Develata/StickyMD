//! Notes a recorder-launched instance starts with. The rendering note is the G5 stress
//! note with every image format and an over-limit image, read from the same fixture files,
//! so a recorder without the repository seeds the same content.

use std::path::Path;

use super::super::manual_observation::registry::Fixture;

const STRESS: &str =
    include_str!("../../../../../crates/stickymd-render/tests/fixtures/rendering-stress.md");
const PNG: &[u8] = include_bytes!(
    "../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.png"
);
const JPEG: &[u8] = include_bytes!(
    "../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.jpg"
);
const WEBP: &[u8] = include_bytes!(
    "../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.webp"
);
const GIF: &[u8] = include_bytes!(
    "../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.gif"
);

pub(super) fn seed(program: &Path, fixture: Fixture) -> Result<(), String> {
    let note = program.join("note");
    for directory in [note.join("images"), note.join(".trash")] {
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    }
    match fixture {
        Fixture::Blank => crate::atomic_evidence::write(&note.join("note.md"), b""),
        Fixture::Rendering => {
            crate::atomic_evidence::write(&note.join("note.md"), rendering_note().as_bytes())?;
            let images = note.join("images");
            for (name, bytes) in [
                ("stress-top.png", PNG),
                ("stress-bottom.png", PNG),
                ("formats.png", PNG),
                ("formats.jpg", JPEG),
                ("formats.webp", WEBP),
                ("formats.gif", GIF),
            ] {
                crate::atomic_evidence::write(&images.join(name), bytes)?;
            }
            crate::atomic_evidence::write(&images.join("over-limit.bmp"), &over_limit_bitmap())
        }
    }
}

fn rendering_note() -> String {
    format!(
        "{STRESS}\n\n## Image formats\n\n![PNG](images/formats.png)\n\n![JPEG](images/formats.jpg)\n\n![WebP](images/formats.webp)\n\n![GIF](images/formats.gif)\n\n![Over the pixel limit](images/over-limit.bmp)\n"
    )
}

/// A header-only BMP declaring 100 000 × 100 000 pixels: over the decode limit, so Preview
/// shows its placeholder.
fn over_limit_bitmap() -> Vec<u8> {
    let mut bytes = vec![0_u8; 54];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&54_u32.to_le_bytes());
    bytes[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&100_000_i32.to_le_bytes());
    bytes[22..26].copy_from_slice(&100_000_i32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&32_u16.to_le_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::{Fixture, seed};

    #[test]
    fn each_fixture_seeds_a_note_and_its_images() {
        let program =
            std::env::temp_dir().join(format!("stickymd-manual-fixture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&program);
        seed(&program, Fixture::Blank).unwrap();
        assert_eq!(std::fs::read(program.join("note/note.md")).unwrap(), b"");
        seed(&program, Fixture::Rendering).unwrap();
        let note = std::fs::read_to_string(program.join("note/note.md")).unwrap();
        for image in [
            "stress-top.png",
            "stress-bottom.png",
            "formats.png",
            "formats.jpg",
            "formats.webp",
            "formats.gif",
            "over-limit.bmp",
        ] {
            assert!(program.join("note/images").join(image).is_file(), "{image}");
        }
        assert!(note.contains("images/over-limit.bmp") && note.contains("images/stress-top.png"));
        // Created exclusively by this test under the temp directory.
        let _ = std::fs::remove_dir_all(program);
    }
}
