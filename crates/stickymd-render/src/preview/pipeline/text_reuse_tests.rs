//! Same-input layout measurements for repeated attributed text and a unique-text control.
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use super::*;
use std::{collections::hash_map::DefaultHasher, hash::Hasher, sync::Arc, time::Instant};
use stickymd_core::LineEnding;

struct Image(Vec<u8>);

impl PreviewImageSource for Image {
    fn inspect(&self, _: &str) -> Result<Option<crate::image::ImageMetadata>, String> {
        crate::image::inspect_encoded_image(&self.0)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    fn load(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(Some(self.0.clone()))
    }
}

fn fixture(target_bytes: usize, mixed: bool) -> DocumentSnapshot {
    let mut source = String::new();
    let mut index = 0;
    while source.len() < target_bytes {
        if mixed {
            source.push_str(&format!(
                "中文 e\u{301} **bold** [link](https://example.com/{index}) \
                 English words with $x^2+y^2$ and ![图片](image.png) tail text.\n\n"
            ));
        } else {
            source.push_str(&format!(
                "Unique paragraph {index:04}: sequence {:08x}, ordinary wrapping text.\n\n",
                index * 7919
            ));
        }
        index += 1;
    }
    DocumentSnapshot {
        text: Arc::from(source),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

// A local same-compiler/font geometry/projection/pixel comparison, not evidence identity.
fn fingerprint(pipeline: &mut PreviewPipeline, document: &mut LaidOutDocument) -> u64 {
    let mut hash = DefaultHasher::new();
    hash.write(document.projection.text().as_bytes());
    hash.write_u32(document.height_px.to_bits());
    for block in &document.blocks {
        hash.write_u32(block.top.to_bits());
        hash.write_u32(block.bottom.to_bits());
        for chunk in &block.chunks {
            hash.write_u32(chunk.x.to_bits());
            hash.write_u32(chunk.y.to_bits());
            if let super::super::layout::LayoutContent::Text(layout) = &chunk.content {
                let boxes = super::super::text_layout::project_visible_text_boxes(
                    layout,
                    chunk.x,
                    chunk.y,
                    0.0,
                    f32::MAX,
                );
                hash.write(format!("{boxes:?}").as_bytes());
            }
        }
    }
    for scroll in [0.0, document.height_px / 2.0, document.height_px] {
        let frame = paint_document(
            &mut pipeline.font_system,
            &mut pipeline.swash_cache,
            document,
            300,
            scroll,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
        hash.write(frame.rgba());
    }
    hash.finish()
}

#[test]
#[ignore = "Release-only shaping reuse measurement; run serially with --nocapture"]
fn phase5_preview_release_baseline_text_reuse() {
    if cfg!(debug_assertions) {
        panic!("run with --release");
    }
    let image = crate::image::prepare_rgba_image(16, 16, vec![180; 16 * 16 * 4]).unwrap();
    let image = Image(image.bytes().to_vec());
    for (bytes, mixed) in [(20 * 1024, true), (100 * 1024, true), (20 * 1024, false)] {
        let snapshot = fixture(bytes, mixed);
        let owned = PreviewParser.parse(&snapshot).unwrap();
        let tree = RenderTreeBuilder.build(&owned);
        let mut pipeline = PreviewPipeline::new();
        let mut samples = Vec::new();
        let mut last = None;
        for iteration in 0..24 {
            drop(last.take());
            let started = Instant::now();
            let document = layout_document(
                LayoutResources {
                    font_system: &mut pipeline.font_system,
                    fonts: &pipeline.fonts,
                    math_engine: &mut pipeline.math_engine,
                    image_source: Some(&image),
                    image_cache: &mut pipeline.image_cache,
                    image_band: (0.0, 600.0),
                },
                &tree,
                640,
                1.0,
                PreviewTheme::Light,
            );
            let elapsed = started.elapsed();
            if iteration >= 4 {
                samples.push(elapsed);
            }
            last = Some(std::hint::black_box(document));
        }
        samples.sort_unstable();
        let fingerprint = fingerprint(&mut pipeline, last.as_mut().unwrap());
        println!(
            "text_reuse mixed={mixed} bytes={} samples=20 layout_ms_p50={:.3} layout_ms_p95={:.3} layout_ms_max={:.3} fingerprint={fingerprint:016x}",
            snapshot.text.len(),
            samples[9].as_secs_f64() * 1000.0,
            samples[18].as_secs_f64() * 1000.0,
            samples[19].as_secs_f64() * 1000.0,
        );
        assert!(pipeline.image_cache_bytes() <= crate::image::IMAGE_CACHE_BUDGET_BYTES);
    }
}
