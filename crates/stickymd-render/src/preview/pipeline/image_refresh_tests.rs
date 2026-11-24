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
            frame.selected_text(frame.select_all()),
            expected.selected_text(expected.select_all())
        );
        for x in [24.0, 50.0, 200.0, 600.0] {
            assert_eq!(
                frame.hit_test(x, scroll + 50.0),
                expected.hit_test(x, scroll + 50.0)
            );
        }
    }
    assert_eq!(refreshed.counters().layouts, 1);
    assert_eq!(
        refreshed.math_counters().parse_layout_calls,
        math.parse_layout_calls
    );
    assert_eq!(
        refreshed.math_counters().rasterizations,
        math.rasterizations
    );
    assert!(Arc::ptr_eq(
        &projection,
        &refreshed.layout.as_ref().unwrap().projection
    ));
}

#[test]
fn repeated_image_requests_share_one_read_per_refresh_and_revalidate_next_time() {
    let images = Images::new();
    let document = DocumentSnapshot {
        text: Arc::from("![same](image.png)\n\n".repeat(30)),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = build(&document, &images);
    assert_eq!(images.inspections.get(), 1);
    assert_eq!(images.loads.get(), 1);
    let first = paint(&mut pipeline, &images, 0.0);
    paint(&mut pipeline, &images, f32::MAX);
    assert_eq!(images.inspections.get(), 2);
    assert_eq!(images.loads.get(), 2);
    *images.bytes.borrow_mut() = Some(image_bytes(64, 64, 40));
    let updated = paint(&mut pipeline, &images, 0.0);
    assert_eq!(images.inspections.get(), 3);
    assert_eq!(images.loads.get(), 3);
    assert_eq!(first.document_height(), updated.document_height());
    assert_ne!(first.rgba(), updated.rgba());
    assert_eq!(pipeline.counters().layouts, 1);
}

#[test]
fn equal_image_targets_share_validated_bytes_across_different_container_bounds() {
    let images = Images::new();
    let mut pipeline = build(&fixture(), &images);
    assert_eq!(images.inspections.get(), 1);
    assert_eq!(images.loads.get(), 1);
    paint(&mut pipeline, &images, f32::MAX);
    assert_eq!(images.inspections.get(), 2);
    assert_eq!(images.loads.get(), 2);
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

/// An image at the end of a long paragraph is admitted at its final position, so the file read
/// after the probe is checked there: other dimensions are used, a failure falls back to text,
/// and the copy text is that of the source either way.
#[test]
fn an_image_placed_at_the_end_of_a_long_paragraph_uses_actual_bytes_or_falls_back() {
    let source = format!("{}![i](image.png)", "$$x$$\n".repeat(120));
    let document = DocumentSnapshot {
        text: Arc::from(source.as_str()),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let reference_images = Images::new();
    let reference = PreviewPipeline::new()
        .build_with_image_source(
            &document,
            400,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&reference_images),
        )
        .unwrap()
        .text()
        .to_owned();
    for loaded in [
        Some(image_bytes(48, 80, 90)),
        None,
        Some(b"bad pixels".to_vec()),
    ] {
        let images = Images::new();
        let changed = ChangedAfterInspection {
            images: &images,
            loaded: loaded.clone(),
        };
        let mut pipeline = PreviewPipeline::new();
        let top = pipeline
            .build_with_image_source(
                &document,
                400,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&changed),
            )
            .unwrap();
        let bottom = top.document_height() - 300.0;
        let frame = pipeline
            .build_with_image_source(
                &document,
                400,
                300,
                1.0,
                bottom,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&changed),
            )
            .unwrap();
        let sizes: Vec<(u32, u32, bool)> = pipeline
            .layout
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.chunks)
            .filter_map(|chunk| match &chunk.content {
                super::super::layout::LayoutContent::Image(image) => {
                    Some((image.width, image.height, image.raster.is_some()))
                }
                _ => None,
            })
            .collect();
        if loaded.as_deref().is_some_and(|bytes| bytes.len() > 20)
            && loaded != Some(b"bad pixels".to_vec())
        {
            assert_eq!(
                sizes,
                [(48, 80, true)],
                "the actual decoded size is laid out"
            );
        } else {
            assert!(
                sizes.is_empty(),
                "a failed load falls back to text: {sizes:?}"
            );
        }
        assert_eq!(
            frame.copy_text(frame.select_all()).unwrap(),
            source,
            "the copy text survives laying the chunk out again"
        );
        assert_eq!(
            frame.text(),
            reference,
            "the display text is not doubled by laying the chunk out again"
        );
    }
}

/// Two files by name: `inspect` answers from the probe bytes, `load` from the actual bytes and
/// counts reads per destination.
struct PerFile {
    probe: Vec<(&'static str, Vec<u8>)>,
    actual: Vec<(&'static str, Vec<u8>)>,
    loads: RefCell<Vec<(String, usize)>>,
}

impl PerFile {
    fn new(files: &[(&'static str, Vec<u8>, Vec<u8>)]) -> Self {
        Self {
            probe: files.iter().map(|(n, p, _)| (*n, p.clone())).collect(),
            actual: files.iter().map(|(n, _, a)| (*n, a.clone())).collect(),
            loads: RefCell::default(),
        }
    }

    fn loads_of(&self, name: &str) -> usize {
        self.loads
            .borrow()
            .iter()
            .find_map(|(n, count)| (n == name).then_some(*count))
            .unwrap_or(0)
    }
}

fn bytes_of<'a>(files: &'a [(&'static str, Vec<u8>)], name: &str) -> Option<&'a Vec<u8>> {
    files
        .iter()
        .find_map(|(n, bytes)| (*n == name).then_some(bytes))
}

impl PreviewImageSource for PerFile {
    fn inspect(&self, name: &str) -> Result<Option<crate::image::ImageMetadata>, String> {
        bytes_of(&self.probe, name)
            .map(|bytes| crate::image::inspect_encoded_image(bytes).map_err(|e| e.to_string()))
            .transpose()
    }

    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let mut loads = self.loads.borrow_mut();
        match loads.iter_mut().find(|(n, _)| n == name) {
            Some((_, count)) => *count += 1,
            None => loads.push((name.to_owned(), 1)),
        }
        Ok(bytes_of(&self.actual, name).cloned())
    }
}

/// `(top y, width, height, raster held)` of every image of the first block, in layout order.
fn images_of(pipeline: &PreviewPipeline) -> Vec<(f32, u32, u32, bool)> {
    pipeline.layout.as_ref().unwrap().blocks[0]
        .chunks
        .iter()
        .filter_map(|chunk| match &chunk.content {
            super::super::layout::LayoutContent::Image(image) => {
                Some((chunk.y, image.width, image.height, image.raster.is_some()))
            }
            _ => None,
        })
        .collect()
}

fn build_at(
    pipeline: &mut PreviewPipeline,
    text: &str,
    (width, height): (u32, u32),
    scroll_y: f32,
    images: &dyn PreviewImageSource,
) {
    pipeline
        .build_with_image_source(
            &DocumentSnapshot {
                text: Arc::from(text),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            width,
            height,
            1.0,
            scroll_y,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(images),
        )
        .unwrap();
}

/// When an earlier image turns out taller than probed, a later one is not read at the position
/// the stale lines gave it: the lines are placed again first, and it ends outside the band.
#[test]
fn a_later_image_is_not_read_at_a_position_an_earlier_resize_invalidated() {
    let small = image_bytes(16, 16, 120);
    let tall = image_bytes(16, 96, 120);
    let text = |k: usize| format!("![a](a.png)\\\n{}![b](b.png)", "t\\\n".repeat(k));
    // A line count that puts B's probed top just inside the band's lower edge (0..300 + 300).
    let k = (0..60)
        .find(|&k| {
            let consistent = PerFile::new(&[
                ("a.png", small.clone(), small.clone()),
                ("b.png", small.clone(), small.clone()),
            ]);
            let mut pipeline = PreviewPipeline::new();
            build_at(&mut pipeline, &text(k), (400, 300), 0.0, &consistent);
            images_of(&pipeline)
                .get(1)
                .is_some_and(|b| (560.0..=600.0).contains(&b.0))
        })
        .expect("a line count that puts B at the band edge");
    let files = PerFile::new(&[
        ("a.png", small.clone(), tall),
        ("b.png", small.clone(), small),
    ]);
    let mut pipeline = PreviewPipeline::new();
    build_at(&mut pipeline, &text(k), (400, 300), 0.0, &files);
    let images = images_of(&pipeline);
    assert_eq!(
        images[0].1..=images[0].2,
        16..=96,
        "A is laid out at its actual size"
    );
    assert!(images[1].0 > 600.0, "B ends below the band: {images:?}");
    assert!(!images[1].3, "and holds no raster");
    assert_eq!(files.loads_of("a.png"), 1, "A is read once");
    assert_eq!(files.loads_of("b.png"), 0, "B is never read");
}

/// An image that ends outside the band after a re-placement does not keep a cache lease that
/// would refuse the visible one: with room for one raster, the visible image is drawn.
#[test]
fn an_image_that_leaves_the_band_does_not_block_the_visible_one() {
    let tall = image_bytes(16, 100, 120);
    let small = image_bytes(16, 16, 120);
    // A different file, so B cannot share A's cache entry.
    let other = image_bytes(16, 16, 60);
    let mut text = String::from("![a](a.png)\\\nt\\\nt\\\n![b](b.png)");
    text.push_str(&"\\\nt".repeat(40));
    let files = PerFile::new(&[
        ("a.png", tall, small.clone()),
        ("b.png", other.clone(), other),
    ]);
    let budget = 16 * 16 * 4 + crate::image::IMAGE_CACHE_ENTRY_OVERHEAD_BYTES;
    let mut pipeline = PreviewPipeline::new();
    pipeline.image_cache = DecodedImageCache::new(budget);
    // The band starts at 380 - 300: A's probed bottom (124) is inside it, its real one (40) is not.
    build_at(&mut pipeline, &text, (400, 300), 380.0, &files);
    let images = images_of(&pipeline);
    assert_eq!(
        images.len(),
        2,
        "both are images, not text fallbacks: {images:?}"
    );
    assert!(!images[0].3, "A is off the band");
    assert!(images[1].3, "B is in the band and drawn: {images:?}");
}

/// A file that is exactly what was probed lays out in one pass at the fitted size: fitting the
/// original size once gives 101 x 50, not the 100 x 50 that fitting the fitted size gives.
#[test]
fn a_stable_image_is_fitted_once() {
    let wide = image_bytes(200, 100, 120);
    let files = PerFile::new(&[("a.png", wide.clone(), wide)]);
    let mut pipeline = PreviewPipeline::new();
    // 149 px wide leaves a 101 px column; the four-line cap is 105 px.
    build_at(&mut pipeline, "![a](a.png)", (149, 300), 0.0, &files);
    assert_eq!(
        images_of(&pipeline)
            .iter()
            .map(|i| (i.1, i.2, i.3))
            .collect::<Vec<_>>(),
        [(101, 50, true)]
    );
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

#[test]
fn wide_table_images_release_hidden_columns_and_load_new_columns_without_relayout() {
    struct Columns {
        bytes: Vec<Vec<u8>>,
        loads: Cell<usize>,
    }
    impl PreviewImageSource for Columns {
        fn inspect(
            &self,
            destination: &str,
        ) -> Result<Option<crate::image::ImageMetadata>, String> {
            let index: usize = destination.trim_end_matches(".png").parse().unwrap();
            Ok(Some(
                crate::image::inspect_encoded_image(&self.bytes[index]).unwrap(),
            ))
        }
        fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String> {
            self.loads.set(self.loads.get() + 1);
            let index: usize = destination.trim_end_matches(".png").parse().unwrap();
            Ok(Some(self.bytes[index].clone()))
        }
    }
    let images = Columns {
        bytes: (0..24).map(|i| image_bytes(64, 64, 100 + i)).collect(),
        loads: Cell::new(0),
    };
    let source = format!(
        "|{}|\n|{}|\n|{}|\n\n```\n{}\n```",
        (0..24)
            .map(|i| format!("C{i}"))
            .collect::<Vec<_>>()
            .join("|"),
        vec!["---"; 24].join("|"),
        (0..24)
            .map(|i| format!("![image{i}]({i}.png)"))
            .collect::<Vec<_>>()
            .join("|"),
        "code ".repeat(100)
    );
    let document = DocumentSnapshot {
        text: Arc::from(source),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = build_with_budget(&document, &images, 180_000);
    let first = paint(&mut pipeline, &images, 0.0);
    let target = first.horizontal_target_at(25.0, 25.0).unwrap();
    let table_images = |pipeline: &PreviewPipeline| {
        pipeline.layout.as_ref().unwrap().blocks[0]
            .chunks
            .iter()
            .filter_map(|chunk| match &chunk.content {
                super::super::layout::LayoutContent::Image(image) => Some(image.raster.is_some()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let initial = table_images(&pipeline);
    assert_eq!(
        initial.len(),
        24,
        "offscreen images keep geometry, never budget-error text"
    );
    assert!(initial[0] && !initial[23]);
    assert!(images.loads.get() < 12);
    pipeline.set_horizontal_offsets(
        super::super::PreviewHorizontalOffsets::default().with_offset(target, target.max_offset()),
    );
    let last = paint(&mut pipeline, &images, 0.0);
    let final_images = table_images(&pipeline);
    assert!(!final_images[0] && final_images[23]);
    assert_eq!(first.text(), last.text());
    assert_eq!(first.document_height(), last.document_height());
    assert_eq!(pipeline.counters().layouts, 1);
    assert!(pipeline.image_cache_bytes() <= 180_000);
    // Scrolling the separate code block must not reread unrelated table images.
    let code_y = pipeline.layout.as_ref().unwrap().blocks[1].top + 1.0;
    let code = last.horizontal_target_at(25.0, code_y).unwrap();
    let loads = images.loads.get();
    pipeline.set_horizontal_offsets(pipeline.horizontal_offsets.with_offset(code, 60.0));
    paint(&mut pipeline, &images, 0.0);
    assert_eq!(images.loads.get(), loads);
}
