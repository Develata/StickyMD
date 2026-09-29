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
    for (name, bytes) in [
        ("stress-top.png", PNG),
        ("stress-bottom.png", PNG),
        ("g5.png", PNG),
        ("g5.jpg", JPEG),
        ("g5.webp", WEBP),
        ("g5.gif", GIF),
    ] {
        crate::atomic_evidence::write(&images.join(name), bytes)?;
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
    crate::atomic_evidence::write(&images.join("g5-oversize.bmp"), &oversized)
}

// These exact bytes are decoded by the render crate's qualification_images test.
// Keep visible color regions so desktop evidence can distinguish images from fallback text.
const PNG: &[u8] = include_bytes!(
    "../../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.png"
);
const JPEG: &[u8] = include_bytes!(
    "../../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.jpg"
);
const WEBP: &[u8] = include_bytes!(
    "../../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.webp"
);
const GIF: &[u8] = include_bytes!(
    "../../../../../../crates/stickymd-render/tests/fixtures/qualification-images/g5.gif"
);
