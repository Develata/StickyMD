//! macOS local image reads for the shared Preview worker.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::fs::File;
use std::io::BufReader;
#[cfg(test)]
use std::path::PathBuf;

use stickymd_render::image::{
    ImageMetadata, PreviewImageSource, inspect_image_reader, is_network_destination,
    read_encoded_image,
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
        if is_network_destination(destination) {
            return Ok(None);
        }
        let Some(file) = self.open(destination)? else {
            return Ok(None);
        };
        read_encoded_image(file).map(Some)
    }
}

#[cfg(test)]
mod tests {
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
        stickymd_desktop::testing::contracts::local_image_source_contract(
            crate::test_support::unique_temp_path("preview-image-中文 空格"),
            LocalImageSource::new,
        );
    }
}
