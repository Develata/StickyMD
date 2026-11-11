//! Windows local image reads for the shared Preview worker.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::io::BufReader;
use std::path::PathBuf;

use stickymd_render::image::{
    ImageMetadata, PreviewImageSource, inspect_image_reader, is_network_destination,
    read_encoded_image,
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
        if is_network_destination(destination) {
            return Ok(None);
        }
        let path =
            resolve_local_image(&self.base, destination).map_err(|error| error.to_string())?;
        let file = match open_local_image(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        read_encoded_image(file).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase7_local_image_adapter_reads_png_and_rejects_network_placeholders() {
        stickymd_desktop::testing::contracts::local_image_source_contract(
            crate::test_support::unique_temp_path("preview-image-中文 空格"),
            LocalImageSource::new,
        );
    }
}
