//! Headless integration of image-band refresh with the Windows file adapter.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use super::*;
use std::sync::Arc;
use stickymd_core::LineEnding;

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn replace_image(root: &std::path::Path, height: u32, value: u8) {
    let image = stickymd_render::image::prepare_rgba_image(
        64,
        height,
        vec![value; (64 * height * 4) as usize],
    )
    .unwrap();
    let staged = root.join("replacement.png");
    std::fs::write(&staged, image.bytes()).unwrap();
    std::fs::rename(staged, root.join("数学 图.png")).unwrap();
}

#[test]
fn phase7_local_image_band_refresh_survives_file_replacement_in_unicode_space_directory() {
    let root = crate::test_support::unique_temp_path("image-band-中文 空格");
    std::fs::create_dir(&root).unwrap();
    let _fixture = Fixture(root.clone());
    replace_image(&root, 64, 180);
    let source = LocalImageSource::new(root.clone());
    let snapshot = DocumentSnapshot {
        text: Arc::from("text ![图](<数学 图.png>) after\n\n![图](<数学 图.png>)\n\n".repeat(40)),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = PreviewPipeline::new();
    pipeline
        .build_with_image_source(
            &snapshot,
            500,
            200,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&source),
        )
        .unwrap();
    let paint = |pipeline: &mut PreviewPipeline, scroll| {
        pipeline
            .paint_with_image_source(
                snapshot.generation,
                200,
                scroll,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&source),
            )
            .unwrap()
    };
    let first = paint(&mut pipeline, 1_000.0);
    paint(&mut pipeline, 0.0);
    assert_eq!(pipeline.counters().layouts, 1);
    replace_image(&root, 64, 90);
    let replaced = paint(&mut pipeline, 1_000.0);
    assert_eq!(pipeline.counters().layouts, 1);
    assert_ne!(
        stickymd_core::hash_bytes(first.rgba()),
        stickymd_core::hash_bytes(replaced.rgba())
    );
    assert_eq!(first.document_height(), replaced.document_height());
    assert_eq!(first.text(), replaced.text());
    replace_image(&root, 96, 90);
    let resized = paint(&mut pipeline, 0.0);
    assert!(resized.document_height() > replaced.document_height());
    assert_eq!(pipeline.counters().layouts, 2);
    assert!(pipeline.image_cache_bytes() <= stickymd_render::image::IMAGE_CACHE_BUDGET_BYTES);
    std::fs::remove_file(root.join("数学 图.png")).unwrap();
    paint(&mut pipeline, 1_000.0);
    assert_eq!(pipeline.counters().layouts, 3);
    assert!(!root.join("数学 图.png").exists());
}
