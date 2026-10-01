//! Image band refresh equivalence, invalidation and measured scrolling cost.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#image-safety-limits

use super::*;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::time::Instant;
use stickymd_core::LineEnding;

struct Images {
    bytes: RefCell<Option<Vec<u8>>>,
    inspections: Cell<usize>,
    loads: Cell<usize>,
}

impl Images {
    fn new() -> Self {
        Self {
            bytes: RefCell::new(Some(image_bytes(64, 64, 180))),
            inspections: Cell::new(0),
            loads: Cell::new(0),
        }
    }
}

fn image_bytes(width: u32, height: u32, value: u8) -> Vec<u8> {
    crate::image::prepare_rgba_image(width, height, vec![value; (width * height * 4) as usize])
        .unwrap()
        .bytes()
        .to_vec()
}

impl PreviewImageSource for Images {
    fn inspect(&self, _: &str) -> Result<Option<crate::image::ImageMetadata>, String> {
        self.inspections.set(self.inspections.get() + 1);
        self.bytes
            .borrow()
            .as_deref()
            .map(|bytes| crate::image::inspect_encoded_image(bytes).map_err(|e| e.to_string()))
            .transpose()
    }

    fn load(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        self.loads.set(self.loads.get() + 1);
        Ok(self.bytes.borrow().clone())
    }
}

fn fixture() -> DocumentSnapshot {
    let mut text = String::new();
    for index in 0..60 {
        text.push_str(&format!("## Section {index}\n\n"));
        text.push_str(&"中文 Preview text with **bold** and $x^2 + y^2$. ".repeat(4));
        text.push_str("\n\n");
        text.push_str(match index % 3 {
            0 => "![独立](image.png)\n\n",
            1 => "- before ![列表](image.png) after\n\n",
            _ => "| cell |\n| --- |\n| before ![表格](image.png) after |\n\n",
        });
    }
    DocumentSnapshot {
        text: Arc::from(text),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

fn build(document: &DocumentSnapshot, images: &dyn PreviewImageSource) -> PreviewPipeline {
    build_with_budget(document, images, crate::image::IMAGE_CACHE_BUDGET_BYTES)
}

fn build_with_budget(
    document: &DocumentSnapshot,
    images: &dyn PreviewImageSource,
    budget: usize,
) -> PreviewPipeline {
    let mut pipeline = PreviewPipeline::new();
    pipeline.image_cache = DecodedImageCache::new(budget);
    pipeline
        .build_with_image_source(
            document,
            640,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(images),
        )
        .unwrap();
    pipeline
}

fn paint(
    pipeline: &mut PreviewPipeline,
    images: &dyn PreviewImageSource,
    scroll: f32,
) -> PreviewFrame {
    pipeline
        .paint_with_image_source(
            Generation::initial(),
            300,
            scroll,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(images),
        )
        .unwrap()
}

#[test]
fn phase7_image_band_matches_full_layout_pixels_selection_and_anchors() {
    let images = Images::new();
    let document = fixture();
    let mut refreshed = build(&document, &images);
    let mut reference = build(&document, &images);
    let projection = Arc::clone(&refreshed.layout.as_ref().unwrap().projection);
    let math = refreshed.math_counters();
    let bottom = (refreshed.layout.as_ref().unwrap().height_px - 300.0).max(0.0);
    for scroll in [bottom, 0.0, bottom * 0.5, bottom, 0.0] {
        let frame = paint(&mut refreshed, &images, scroll);
        // Force the existing full-layout path as an independent oracle.
        reference.layout = None;
        let expected = reference
            .relayout_with_image_source(
                document.generation,
                640,
                300,
                1.0,
                scroll,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        assert!(frame.rgba() == expected.rgba(), "pixels at scroll {scroll}");
        assert_eq!(frame.text(), expected.text());
        assert_eq!(frame.document_height(), expected.document_height());
        assert_eq!(
            frame.scroll_anchor_at_y(scroll),
            expected.scroll_anchor_at_y(scroll)
        );
        assert_eq!(
            frame.copy_selection(frame.select_all()),
            expected.copy_selection(expected.select_all())
        );
        for x in [24.0, 50.0, 200.0, 600.0] {
            assert_eq!(
                frame.hit_test(x, scroll + 50.0),
                expected.hit_test(x, scroll + 50.0)
            );
        }
    }
    assert_eq!(refreshed.counters().layouts, 1);
    assert_eq!(refreshed.math_counters(), math);
    assert!(Arc::ptr_eq(
        &projection,
        &refreshed.layout.as_ref().unwrap().projection
    ));
}

fn assert_matches_full(
    pipeline: &mut PreviewPipeline,
    images: &dyn PreviewImageSource,
    scroll: f32,
) -> PreviewFrame {
    let actual = paint(pipeline, images, scroll);
    let mut reference = build(&fixture(), images);
    reference.layout = None;
    let expected = reference
        .relayout_with_image_source(
            Generation::initial(),
            640,
            300,
            1.0,
            scroll,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(images),
        )
        .unwrap();
    assert!(
        actual.rgba() == expected.rgba(),
        "refresh and full layout pixels differ"
    );
    assert_eq!(actual.document_height(), expected.document_height());
    assert_eq!(actual.text(), expected.text());
    actual
}

#[test]
fn phase7_image_band_refreshes_same_size_content_without_geometry_changes() {
    let images = Images::new();
    let mut pipeline = build(&fixture(), &images);
    let old = paint(&mut pipeline, &images, 1_000.0);
    paint(&mut pipeline, &images, 0.0);
    *images.bytes.borrow_mut() = Some(image_bytes(64, 64, 90));
    let frame = assert_matches_full(&mut pipeline, &images, 1_000.0);
    assert_ne!(
        stickymd_core::hash_bytes(old.rgba()),
        stickymd_core::hash_bytes(frame.rgba())
    );
    assert_eq!(pipeline.counters().layouts, 1);
}

#[test]
fn phase7_image_band_relayouts_changed_missing_or_corrupt_files_and_recovers() {
    for replacement in [
        Some(image_bytes(32, 96, 90)),
        None,
        Some(b"corrupt image".to_vec()),
    ] {
        let images = Images::new();
        let mut pipeline = build(&fixture(), &images);
        *images.bytes.borrow_mut() = replacement;
        assert_matches_full(&mut pipeline, &images, 1_000.0);
        assert_eq!(pipeline.counters().layouts, 2);
        *images.bytes.borrow_mut() = Some(image_bytes(64, 64, 180));
        assert_matches_full(&mut pipeline, &images, 0.0);
        assert_eq!(pipeline.counters().layouts, 3);
        paint(&mut pipeline, &images, 1_000.0);
        assert_eq!(
            pipeline.counters().layouts,
            3,
            "recovered geometry should be reusable"
        );
    }
}

struct ChangedAfterInspection<'a> {
    images: &'a Images,
    loaded: Option<Vec<u8>>,
}

impl PreviewImageSource for ChangedAfterInspection<'_> {
    fn inspect(&self, destination: &str) -> Result<Option<crate::image::ImageMetadata>, String> {
        self.images.inspect(destination)
    }
    fn load(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.loaded.clone())
    }
}

#[test]
fn phase7_image_band_rechecks_actual_bytes_and_decode_failure_after_inspection() {
    for loaded in [
        Some(image_bytes(48, 80, 90)),
        None,
        Some(b"bad pixels".to_vec()),
    ] {
        let images = Images::new();
        let mut pipeline = build(&fixture(), &images);
        let changed = ChangedAfterInspection {
            images: &images,
            loaded,
        };
        assert_matches_full(&mut pipeline, &changed, 1_000.0);
        assert_eq!(pipeline.counters().layouts, 2);
    }
}

#[test]
fn phase7_image_band_drops_old_raster_leases_before_cache_admission() {
    let images = Images::new();
    let budget = 64 * 64 * 4 + crate::image::IMAGE_CACHE_ENTRY_OVERHEAD_BYTES;
    let mut pipeline = build_with_budget(&fixture(), &images, budget);
    let old = pipeline
        .layout
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|b| &b.chunks)
        .find_map(|chunk| match &chunk.content {
            super::super::layout::LayoutContent::Image(image) => {
                image.raster.as_ref().map(Arc::downgrade)
            }
            _ => None,
        })
        .unwrap();
    *images.bytes.borrow_mut() = Some(image_bytes(64, 64, 90));
    paint(&mut pipeline, &images, 1_000.0);
    assert_eq!(
        pipeline.counters().layouts,
        1,
        "old leases must not block new admission"
    );
    assert!(old.upgrade().is_none());
    assert!(pipeline.image_cache_counters().evictions > 0);
    assert_eq!(pipeline.image_cache_entries(), 1);
    assert!(pipeline.image_cache_bytes() <= budget);
    pipeline.release_document_projection();
    assert!(pipeline.layout.is_none());
    assert_eq!(pipeline.image_cache_bytes(), 0);
}

#[test]
fn phase7_image_band_source_availability_changes_relayout() {
    let images = Images::new();
    let mut pipeline = build(&fixture(), &images);
    pipeline
        .paint(
            Generation::initial(),
            300,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert_eq!(pipeline.counters().layouts, 2);
    assert_matches_full(&mut pipeline, &images, 0.0);
    assert_eq!(pipeline.counters().layouts, 3);
}

#[test]
fn phase7_image_band_rejects_wrong_generation_before_io_or_raster_release() {
    let images = Images::new();
    let mut pipeline = build(&fixture(), &images);
    let before = (
        images.inspections.get(),
        images.loads.get(),
        pipeline.counters(),
    );
    assert!(matches!(
        pipeline.paint_with_image_source(
            Generation::initial().checked_next().unwrap(),
            300,
            1_000.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&images),
        ),
        Err(PreviewPipelineError::GenerationMismatch { .. })
    ));
    assert_eq!(
        before,
        (
            images.inspections.get(),
            images.loads.get(),
            pipeline.counters()
        )
    );
}

#[test]
fn phase7_image_band_admission_failure_keeps_budget_and_retries_unresolved_images() {
    struct DistinctImages {
        distinct: Cell<bool>,
        shared: Vec<u8>,
        other: Vec<u8>,
    }
    impl PreviewImageSource for DistinctImages {
        fn inspect(&self, _: &str) -> Result<Option<crate::image::ImageMetadata>, String> {
            Ok(Some(
                crate::image::inspect_encoded_image(&self.shared).unwrap(),
            ))
        }
        fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(Some(
                if self.distinct.get() && destination == "b.png" {
                    &self.other
                } else {
                    &self.shared
                }
                .clone(),
            ))
        }
    }
    let images = DistinctImages {
        distinct: Cell::new(false),
        shared: image_bytes(64, 64, 180),
        other: image_bytes(64, 64, 90),
    };
    let document = DocumentSnapshot {
        text: Arc::from(format!(
            "![old](old.png)\n\n{}\n\n![a](a.png)\n\n![b](b.png)",
            "text\n\n".repeat(60)
        )),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let budget = 64 * 64 * 4 + crate::image::IMAGE_CACHE_ENTRY_OVERHEAD_BYTES;
    let mut pipeline = build_with_budget(&document, &images, budget);
    images.distinct.set(true);
    let frame = paint(&mut pipeline, &images, f32::MAX);
    assert!(
        frame.text().contains('b'),
        "failed admission retains alt text"
    );
    assert_eq!(pipeline.counters().layouts, 2);
    assert!(pipeline.image_cache_bytes() <= budget);
    assert_eq!(pipeline.image_cache_entries(), 1);
    images.distinct.set(false);
    paint(&mut pipeline, &images, 0.0);
    assert_eq!(
        pipeline.counters().layouts,
        3,
        "unresolved image must be retried"
    );
    paint(&mut pipeline, &images, f32::MAX);
    assert_eq!(pipeline.counters().layouts, 3);
}

#[test]
#[ignore = "release-only image-band benchmark; run serially with --nocapture"]
fn phase7_image_band_release_baseline() {
    if cfg!(debug_assertions) {
        panic!("run with --release");
    }
    let images = Images::new();
    let document = fixture();
    let mut pipeline = build(&document, &images);
    let bottom = (pipeline.layout.as_ref().unwrap().height_px - 300.0).max(0.0);
    for scroll in [bottom, 0.0, bottom, 0.0] {
        paint(&mut pipeline, &images, scroll);
    }
    let counters = pipeline.counters();
    let inspections = images.inspections.get();
    let loads = images.loads.get();
    let mut samples = Vec::new();
    for iteration in 0..30 {
        let started = Instant::now();
        paint(
            &mut pipeline,
            &images,
            if iteration % 2 == 0 { bottom } else { 0.0 },
        );
        samples.push(started.elapsed());
    }
    samples.sort();
    println!(
        "image_band bytes={} samples=30 ms_median={:.3} ms_p95={:.3} ms_max={:.3} layouts={} inspections={} loads={} cache_bytes={}",
        document.text.len(),
        samples[14].as_secs_f64() * 1_000.0,
        samples[28].as_secs_f64() * 1_000.0,
        samples[29].as_secs_f64() * 1_000.0,
        pipeline.counters().layouts - counters.layouts,
        images.inspections.get() - inspections,
        images.loads.get() - loads,
        pipeline.image_cache_bytes(),
    );
    assert_eq!(pipeline.counters().parses, counters.parses);
    assert_eq!(
        pipeline.counters().render_tree_builds,
        counters.render_tree_builds
    );
    assert!(pipeline.image_cache_bytes() <= crate::image::IMAGE_CACHE_BUDGET_BYTES);
}
