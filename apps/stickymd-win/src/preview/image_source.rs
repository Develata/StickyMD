//! Windows local image reads for the shared Preview worker.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::io::{BufReader, Read};
use std::path::PathBuf;

use stickymd_render::image::{
    ImageMetadata, MAX_ENCODED_IMAGE_BYTES, PreviewImageSource, inspect_image_reader,
};

use crate::assets::{open_local_image, resolve_local_image};

pub struct LocalImageSource {
    base: PathBuf,
}

impl LocalImageSource {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }
}

impl PreviewImageSource for LocalImageSource {
    fn inspect(&self, destination: &str) -> Result<Option<ImageMetadata>, String> {
        let path =
            resolve_local_image(&self.base, destination).map_err(|error| error.to_string())?;
        let file = match open_local_image(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        inspect_image_reader(BufReader::new(file))
            .map(Some)
            .map_err(|error| error.to_string())
    }

    fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String> {
        if destination
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
            || destination
                .get(..8)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        {
            return Ok(None);
        }
        let path =
            resolve_local_image(&self.base, destination).map_err(|error| error.to_string())?;
        let file = match open_local_image(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let mut bytes = Vec::new();
        file.take((MAX_ENCODED_IMAGE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
            return Err("local image exceeds the 64 MiB preview limit".into());
        }
        Ok(Some(bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};
    use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};

    use super::*;

    #[test]
    fn phase7_local_image_adapter_reads_png_and_rejects_network_placeholders() {
        let root = crate::test_support::unique_temp_path("preview-image-中文 空格");
        std::fs::create_dir(&root).unwrap();
        let image = stickymd_render::image::prepare_rgba_image(2, 2, vec![255; 16]).unwrap();
        let bytes = image.bytes();
        std::fs::write(root.join("数学 图.png"), bytes).unwrap();
        let source = LocalImageSource::new(root.clone());
        let path = "%E6%95%B0%E5%AD%A6%20%E5%9B%BE.png";
        assert!(source.inspect(path).unwrap().is_some());
        assert_eq!(source.load(path).unwrap().as_deref(), Some(bytes));
        assert!(source.inspect("missing.png").unwrap().is_none());
        for remote in [
            "file://server/share/a.png",
            "%5c%5cserver/share/a.png",
            r"\\.\pipe\image",
        ] {
            assert!(source.inspect(remote).is_err());
            assert!(source.load(remote).is_err());
        }
        assert!(
            source
                .load("https://example.invalid/a.png")
                .unwrap()
                .is_none()
        );
        let snapshot = DocumentSnapshot {
            text: Arc::from("![ALT](file://server/share/a.png)"),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
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
        assert_eq!(frame.copy_selection(frame.select_all()), Some("ALT"));
        assert_eq!(std::fs::read(root.join("数学 图.png")).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}
