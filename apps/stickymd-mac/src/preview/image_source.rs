//! macOS local image reads for the shared Preview worker.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::fs::File;
use std::io::{BufReader, Read};
#[cfg(test)]
use std::path::PathBuf;

use stickymd_render::image::{
    ImageMetadata, MAX_ENCODED_IMAGE_BYTES, PreviewImageSource, inspect_image_reader,
};

use crate::assets::local_image_destination;
use crate::platform::macos::file::DirectoryHandle;

pub struct LocalImageSource {
    /// The Note Directory the note is saved in, wherever it has moved since the start: relative
    /// destinations are opened from it, so a new directory at the old path is never read.
    directory: DirectoryHandle,
}

impl LocalImageSource {
    #[cfg(test)]
    pub fn new(base: PathBuf) -> Self {
        Self::following(DirectoryHandle::open(&base).expect("test image directory"))
    }

    pub fn following(directory: DirectoryHandle) -> Self {
        Self { directory }
    }

    /// What `destination` names; a missing file is `None`.
    fn open(&self, destination: &str) -> Result<Option<File>, String> {
        let path = local_image_destination(destination).map_err(|error| error.to_string())?;
        match self.directory.open_local_image(&path) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

impl PreviewImageSource for LocalImageSource {
    fn inspect(&self, destination: &str) -> Result<Option<ImageMetadata>, String> {
        let Some(file) = self.open(destination)? else {
            return Ok(None);
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
        let Some(file) = self.open(destination)? else {
            return Ok(None);
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

    /// A note directory moved away and replaced at its old path: images come from the moved one,
    /// where the note is still saved.
    #[test]
    fn images_come_from_the_directory_the_note_is_in() {
        let root = crate::test_support::unique_temp_path("preview-following");
        let note = root.join("note");
        std::fs::create_dir_all(&note).unwrap();
        std::fs::write(note.join("a.png"), b"original").unwrap();
        let source = LocalImageSource::following(DirectoryHandle::open(&note).unwrap());
        std::fs::rename(&note, root.join("moved")).unwrap();
        std::fs::create_dir(&note).unwrap();
        std::fs::write(note.join("a.png"), b"impostor").unwrap();
        assert_eq!(
            source.load("a.png").unwrap().as_deref(),
            Some(&b"original"[..])
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_image_adapter_reads_png_and_rejects_network_placeholders() {
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
