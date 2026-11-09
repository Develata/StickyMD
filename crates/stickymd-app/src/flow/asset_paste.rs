//! One image paste as an all-or-nothing batch: what counts as an image, the batch limit, the ledger
//! a failure hands back and the Markdown a success inserts. Reading a file and publishing an asset
//! are complete operations of the host.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#paste

use std::path::Path;

use stickymd_core::Generation;
use stickymd_render::image::{
    ImageAssetError, MAX_ENCODED_IMAGE_BYTES, PreparedImage, prepare_dib_image,
    prepare_encoded_image_owned, prepare_rgba_image,
};
use thiserror::Error;

use super::ClipboardPaste;
use crate::ports::{AssetPasteCompletion, AssetPasteRequest, AssetPasteRollback, StoredAsset};

pub const MAX_IMAGE_PASTE_BATCH_BYTES: usize = 128 * 1024 * 1024;

/// What a paste needs from the host.
pub trait PasteTarget {
    type Error: std::error::Error + 'static;

    /// Publishes one prepared image as a managed asset.
    fn store(&mut self, image: &PreparedImage) -> Result<StoredAsset, Self::Error>;

    /// Reads at most `limit` bytes of a file the clipboard named.
    fn read_at_most(&mut self, path: &Path, limit: u64) -> std::io::Result<Vec<u8>>;
}

#[derive(Debug, Error)]
pub enum AssetPasteError<E: std::error::Error + 'static> {
    #[error("clipboard file list contains a non-image or unsupported image")]
    MixedOrUnsupportedFiles,
    #[error("clipboard image read failed: {0}")]
    Read(std::io::Error),
    #[error("clipboard image preparation failed: {0}")]
    Image(ImageAssetError),
    #[error("managed image publication failed: {0}")]
    Storage(E),
    #[error("image paste payload exceeds the {limit} byte transaction bound")]
    BatchTooLarge { limit: usize },
    #[error("clipboard payload is not an image transaction")]
    NotImage,
}

/// A failed preparation still returns every asset publication it performed. Only the main
/// coordinator can compare this ledger with the latest canonical references and request a safe
/// convergence; the worker must never replay an old rollback decision by itself.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct PasteFailure<E: std::error::Error + 'static> {
    pub expected_generation: Generation,
    pub error: AssetPasteError<E>,
    pub rollback: AssetPasteRollback,
}

pub fn prepare_and_store_paste<T: PasteTarget>(
    target: &mut T,
    request: AssetPasteRequest,
) -> Result<AssetPasteCompletion, PasteFailure<T::Error>> {
    let expected_generation = request.expected_generation;
    let mut stored = Vec::new();
    let mut total_bytes = 0usize;
    let result = match request.payload {
        ClipboardPaste::EncodedImage(bytes) => add_batch_bytes(&mut total_bytes, bytes.len())
            .and_then(|()| {
                prepare_encoded_image_owned(bytes)
                    .map_err(AssetPasteError::Image)
                    .and_then(|image| store_prepared(target, image, &mut stored))
            }),
        ClipboardPaste::Dib(bytes) => {
            add_batch_bytes(&mut total_bytes, bytes.len()).and_then(|()| {
                prepare_dib_image(&bytes)
                    .map_err(AssetPasteError::Image)
                    .and_then(|image| store_prepared(target, image, &mut stored))
            })
        }
        ClipboardPaste::Rgba {
            width,
            height,
            bytes,
        } => add_batch_bytes(&mut total_bytes, bytes.len()).and_then(|()| {
            prepare_rgba_image(width, height, bytes)
                .map_err(AssetPasteError::Image)
                .and_then(|image| store_prepared(target, image, &mut stored))
        }),
        ClipboardPaste::Files(paths) if !paths.is_empty() => {
            let mut result = Ok(());
            for path in paths {
                let next = read_file_bounded(target, &path)
                    .and_then(|bytes| {
                        add_batch_bytes(&mut total_bytes, bytes.len())?;
                        prepare_encoded_image_owned(bytes)
                            .map_err(|_| AssetPasteError::MixedOrUnsupportedFiles)
                    })
                    .and_then(|image| store_prepared(target, image, &mut stored));
                if let Err(error) = next {
                    result = Err(error);
                    break;
                }
            }
            result
        }
        ClipboardPaste::Files(_) | ClipboardPaste::Text(_) => Err(AssetPasteError::NotImage),
    };
    if let Err(error) = result {
        return Err(PasteFailure {
            expected_generation,
            error,
            rollback: AssetPasteRollback { stored },
        });
    }
    let markdown = stored
        .iter()
        .map(|asset| format!("![](images/{})", asset.name))
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(AssetPasteCompletion {
        expected_generation: request.expected_generation,
        selection: request.selection,
        timestamp_ms: request.timestamp_ms,
        markdown,
        rollback: AssetPasteRollback { stored },
    })
}

/// Adds to the batch's running total, or fails once it would pass the limit.
pub fn add_batch_bytes<E: std::error::Error + 'static>(
    total: &mut usize,
    bytes: usize,
) -> Result<(), AssetPasteError<E>> {
    *total = total
        .checked_add(bytes)
        .ok_or(AssetPasteError::BatchTooLarge {
            limit: MAX_IMAGE_PASTE_BATCH_BYTES,
        })?;
    if *total > MAX_IMAGE_PASTE_BATCH_BYTES {
        Err(AssetPasteError::BatchTooLarge {
            limit: MAX_IMAGE_PASTE_BATCH_BYTES,
        })
    } else {
        Ok(())
    }
}

fn store_prepared<T: PasteTarget>(
    target: &mut T,
    image: PreparedImage,
    stored: &mut Vec<StoredAsset>,
) -> Result<(), AssetPasteError<T::Error>> {
    stored.push(target.store(&image).map_err(AssetPasteError::Storage)?);
    Ok(())
}

fn read_file_bounded<T: PasteTarget>(
    target: &mut T,
    path: &Path,
) -> Result<Vec<u8>, AssetPasteError<T::Error>> {
    let bytes = target
        .read_at_most(path, (MAX_ENCODED_IMAGE_BYTES + 1) as u64)
        .map_err(AssetPasteError::Read)?;
    if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
        return Err(AssetPasteError::MixedOrUnsupportedFiles);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use stickymd_core::{ManagedAssetName, Selection};

    use super::*;
    use crate::ports::StoredAssetDisposition;

    fn name(index: usize) -> ManagedAssetName {
        ManagedAssetName::parse(&format!("stickymd-{index:020x}.png")).unwrap()
    }

    #[derive(Debug, thiserror::Error)]
    #[error("the store refused")]
    struct Refused;

    #[derive(Default)]
    struct FakeTarget {
        files: HashMap<PathBuf, Vec<u8>>,
        refuse_after: Option<usize>,
        stored: Vec<StoredAsset>,
        asked_limits: Vec<u64>,
    }

    impl PasteTarget for FakeTarget {
        type Error = Refused;

        fn store(&mut self, image: &PreparedImage) -> Result<StoredAsset, Refused> {
            if self.refuse_after == Some(self.stored.len()) {
                return Err(Refused);
            }
            assert!(!image.bytes().is_empty());
            let asset = StoredAsset {
                name: name(self.stored.len()),
                disposition: StoredAssetDisposition::Created,
            };
            self.stored.push(asset.clone());
            Ok(asset)
        }

        fn read_at_most(&mut self, path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
            self.asked_limits.push(limit);
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
    }

    fn request(payload: ClipboardPaste) -> AssetPasteRequest {
        AssetPasteRequest {
            expected_generation: Generation::initial(),
            selection: Selection::caret(0),
            timestamp_ms: 7,
            payload,
        }
    }

    fn rgba() -> ClipboardPaste {
        ClipboardPaste::Rgba {
            width: 1,
            height: 1,
            bytes: vec![0, 0, 0, 255],
        }
    }

    fn png() -> Vec<u8> {
        stickymd_render::image::prepare_rgba_image(1, 1, vec![0, 0, 0, 255])
            .unwrap()
            .bytes()
            .to_vec()
    }

    #[test]
    fn a_success_inserts_a_reference_for_each_asset_and_keeps_the_ledger() {
        let mut target = FakeTarget::default();
        let done = prepare_and_store_paste(&mut target, request(rgba())).unwrap();
        assert_eq!(done.markdown, format!("![](images/{})", name(0)));
        assert_eq!(done.rollback.stored, target.stored);
        assert_eq!(done.timestamp_ms, 7);
    }

    #[test]
    fn several_files_become_references_separated_by_a_blank_line() {
        let mut target = FakeTarget::default();
        target.files.insert("a.png".into(), png());
        target.files.insert("b.png".into(), png());
        let payload = ClipboardPaste::Files(vec!["a.png".into(), "b.png".into()]);
        let done = prepare_and_store_paste(&mut target, request(payload)).unwrap();
        assert_eq!(
            done.markdown,
            format!("![](images/{})\n\n![](images/{})", name(0), name(1))
        );
        assert_eq!(
            target.asked_limits,
            [(MAX_ENCODED_IMAGE_BYTES + 1) as u64; 2]
        );
    }

    /// The main coordinator needs every publication that happened before the failure.
    #[test]
    fn a_failure_part_way_returns_what_was_already_published() {
        let mut target = FakeTarget {
            refuse_after: Some(1),
            ..FakeTarget::default()
        };
        target.files.insert("a.png".into(), png());
        target.files.insert("b.png".into(), png());
        let payload = ClipboardPaste::Files(vec!["a.png".into(), "b.png".into()]);
        let failure = prepare_and_store_paste(&mut target, request(payload)).unwrap_err();
        assert!(matches!(failure.error, AssetPasteError::Storage(Refused)));
        assert_eq!(failure.rollback.stored, target.stored);
        assert_eq!(failure.rollback.stored.len(), 1);
        assert_eq!(
            failure.to_string(),
            "managed image publication failed: the store refused"
        );
    }

    #[test]
    fn what_is_not_an_image_is_refused_without_touching_the_store() {
        let mut target = FakeTarget::default();
        for payload in [
            ClipboardPaste::Text("x".into()),
            ClipboardPaste::Files(Vec::new()),
        ] {
            let failure = prepare_and_store_paste(&mut target, request(payload)).unwrap_err();
            assert!(matches!(failure.error, AssetPasteError::NotImage));
        }
        assert!(target.stored.is_empty());
    }

    #[test]
    fn a_file_that_is_no_image_or_too_big_is_unsupported_and_a_read_error_is_a_read_error() {
        let mut target = FakeTarget::default();
        target.files.insert("notes.txt".into(), b"text".to_vec());
        target
            .files
            .insert("huge.png".into(), vec![0; MAX_ENCODED_IMAGE_BYTES + 1]);
        for name in ["notes.txt", "huge.png"] {
            let payload = ClipboardPaste::Files(vec![name.into()]);
            let failure = prepare_and_store_paste(&mut target, request(payload)).unwrap_err();
            assert!(
                matches!(failure.error, AssetPasteError::MixedOrUnsupportedFiles),
                "{name}: {:?}",
                failure.error
            );
        }

        let payload = ClipboardPaste::Files(vec!["gone.png".into()]);
        let failure = prepare_and_store_paste(&mut target, request(payload)).unwrap_err();
        assert!(matches!(failure.error, AssetPasteError::Read(_)));
    }

    #[test]
    fn an_undecodable_payload_is_an_image_error_not_a_file_error() {
        let mut target = FakeTarget::default();
        let payload = ClipboardPaste::EncodedImage(b"not an image".to_vec());
        let failure = prepare_and_store_paste(&mut target, request(payload)).unwrap_err();
        assert!(matches!(failure.error, AssetPasteError::Image(_)));
    }

    #[test]
    fn the_batch_limit_counts_every_part_and_survives_overflow() {
        let mut total = 0;
        add_batch_bytes::<Refused>(&mut total, MAX_IMAGE_PASTE_BATCH_BYTES).unwrap();
        assert!(matches!(
            add_batch_bytes::<Refused>(&mut total, 1),
            Err(AssetPasteError::BatchTooLarge {
                limit: MAX_IMAGE_PASTE_BATCH_BYTES
            })
        ));
        let mut near_overflow = usize::MAX;
        assert!(matches!(
            add_batch_bytes::<Refused>(&mut near_overflow, 1),
            Err(AssetPasteError::BatchTooLarge { .. })
        ));
    }
}
