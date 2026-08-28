//! G5 Markdown, math, image, lazy-scroll, and placeholder runtime evidence.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{CaseEvidence, seed_note};
use super::support;

const STRESS: &str =
    include_str!("../../../../../../crates/stickymd-render/tests/fixtures/rendering-stress.md");

pub(super) fn run(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    let mut fixture = STRESS.to_owned();
    fixture.push_str(concat!(
        "\n\n## G5 image-format appendix\n\n",
        "![PNG](images/g5.png)\n\n",
        "![JPEG](images/g5.jpg)\n\n",
        "![WebP](images/g5.webp)\n\n",
        "![GIF](images/g5.gif)\n\n",
        "![Oversize placeholder](images/g5-oversize.bmp)\n\n",
        "G5_IMAGE_FORMAT_END\n",
    ));
    seed_note(program, &fixture)?;
    seed_images(program)?;
    let before = fs::read(program.join("note/note.md"))
        .map_err(|error| format!("cannot read rendering fixture: {error}"))?;
    let (mut child, window) = support::start_ready(program)?;
    assert_note_unchanged(program, &before, "startup")?;
    support::switch_preview(window, program)?;
    assert_note_unchanged(program, &before, "switch-preview")?;
    support::assert_preview_projection(
        window,
        &[
            "渲染引擎终极暴力测试",
            "STICKYMD_RENDERING_STRESS_END",
            "G5_IMAGE_FORMAT_END",
        ],
    )?;
    assert_note_unchanged(program, &before, "preview-selection")?;
    let mut artifacts = Vec::new();
    support::capture_when_stable(
        repository,
        child.id(),
        "G5-04",
        "preview-top",
        None,
        &mut artifacts,
    )?;
    assert_note_unchanged(program, &before, "preview-top-capture")?;
    crate::window_control::scroll_preview_down(window, 2_000)?;
    assert_note_unchanged(program, &before, "preview-wheel")?;
    let top_sha = artifacts[0].sha256.clone();
    support::capture_when_stable(
        repository,
        child.id(),
        "G5-04",
        "preview-bottom",
        Some(&top_sha),
        &mut artifacts,
    )?;

    support::switch_split(window, program)?;
    assert_note_unchanged(program, &before, "switch-split")?;
    support::assert_source_projection(window, &fixture)?;
    support::assert_preview_projection(
        window,
        &["G5 image-format appendix", "G5_IMAGE_FORMAT_END"],
    )?;
    crate::window_control::scroll_preview_down(window, 2_000)?;
    support::capture_when_stable(
        repository,
        child.id(),
        "G5-04",
        "split-bottom",
        None,
        &mut artifacts,
    )?;
    assert_note_unchanged(program, &before, "split-selection-scroll")?;
    child.kill_and_wait()?;
    Ok(CaseEvidence { artifacts })
}

fn assert_note_unchanged(program: &Path, expected: &[u8], stage: &str) -> Result<(), String> {
    let actual = fs::read(program.join("note/note.md"))
        .map_err(|error| format!("cannot inspect rendering note at {stage}: {error}"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "Preview/Split stage {stage} changed canonical Markdown bytes: expected={} actual={}",
            expected.len(),
            actual.len()
        ))
    }
}

fn seed_images(program: &Path) -> Result<(), String> {
    let images = program.join("note/images");
    fs::create_dir_all(&images)
        .map_err(|error| format!("cannot create G5 image fixture directory: {error}"))?;
    for (name, encoded) in [
        ("stress-top.png", PNG),
        ("stress-bottom.png", PNG),
        ("g5.png", PNG),
        ("g5.jpg", JPEG),
        ("g5.webp", WEBP),
        ("g5.gif", GIF),
    ] {
        let bytes = decode_base64(encoded)?;
        fs::write(images.join(name), bytes)
            .map_err(|error| format!("cannot seed G5 image {name}: {error}"))?;
    }
    let mut oversized = vec![0_u8; 54];
    oversized[0..2].copy_from_slice(b"BM");
    oversized[2..6].copy_from_slice(&54_u32.to_le_bytes());
    oversized[10..14].copy_from_slice(&54_u32.to_le_bytes());
    oversized[14..18].copy_from_slice(&40_u32.to_le_bytes());
    oversized[18..22].copy_from_slice(&100_000_i32.to_le_bytes());
    oversized[22..26].copy_from_slice(&100_000_i32.to_le_bytes());
    oversized[26..28].copy_from_slice(&1_u16.to_le_bytes());
    oversized[28..30].copy_from_slice(&32_u16.to_le_bytes());
    fs::write(images.join("g5-oversize.bmp"), oversized)
        .map_err(|error| format!("cannot seed oversized image fixture: {error}"))
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut chunk = [0_u8; 4];
    let mut used = 0;
    for byte in input.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        chunk[used] = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return Err(format!("invalid base64 byte {byte}")),
        };
        used += 1;
        if used == 4 {
            output.push((chunk[0] << 2) | (chunk[1] >> 4));
            if chunk[2] != 64 {
                output.push((chunk[1] << 4) | (chunk[2] >> 2));
            }
            if chunk[3] != 64 {
                output.push((chunk[2] << 6) | chunk[3]);
            }
            used = 0;
        }
    }
    if used != 0 {
        return Err("base64 fixture length is not divisible by four".to_owned());
    }
    Ok(output)
}

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUAAQEnGONmAAAAAElFTkSuQmCC";
const GIF: &str = "R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
const WEBP: &str = "UklGRiIAAABXRUJQVlA4IBYAAAAwAQCdASoBAAEAAUAmJaQAA3AA/vuUAAA=";
const JPEG: &str = concat!(
    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////",
    "2wBDAf//////////////////////////////////////////////////////////////////////////////////////",
    "wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAX/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIQAxAAAAEf/8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABBQJ//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAwEBPwF//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAgEBPwF//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQAGPwJ//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPyF//9oADAMBAAIAAwAAABD/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oACAEDAQE/EH//xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oACAECAQE/EH//xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oACAEBAAE/EH//2Q=="
);

#[cfg(test)]
mod tests {
    use super::{GIF, JPEG, PNG, WEBP, decode_base64};

    #[test]
    fn embedded_g5_images_have_expected_container_signatures() {
        assert!(decode_base64(PNG).expect("PNG").starts_with(b"\x89PNG"));
        assert!(decode_base64(JPEG).expect("JPEG").starts_with(b"\xff\xd8"));
        assert!(decode_base64(WEBP).expect("WebP").starts_with(b"RIFF"));
        assert!(decode_base64(GIF).expect("GIF").starts_with(b"GIF89a"));
    }
}
