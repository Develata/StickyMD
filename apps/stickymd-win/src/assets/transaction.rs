//! All-or-nothing clipboard preparation and managed-asset publication.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#paste

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{collections::BTreeSet, collections::HashMap};

use stickymd_core::{Generation, ManagedAssetLocation, Selection};
use stickymd_render::image::{
    MAX_ENCODED_IMAGE_BYTES, PreparedImage, prepare_dib_image, prepare_encoded_image,
};
use thiserror::Error;

use super::{AssetStorage, AssetStorageError, StoredAsset};
use crate::flow::ClipboardPaste;

const MAX_IMAGE_PASTE_BATCH_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct AssetPasteRequest {
    pub expected_generation: Generation,
    pub selection: Selection,
    pub timestamp_ms: u64,
    pub payload: ClipboardPaste,
}

#[derive(Debug)]
pub struct AssetPasteCompletion {
    pub expected_generation: Generation,
    pub selection: Selection,
    pub timestamp_ms: u64,
    pub markdown: String,
    pub rollback: AssetPasteRollback,
}

#[derive(Debug, Default, Clone)]
pub struct AssetPasteRollback {
    pub stored: Vec<StoredAsset>,
}

impl AssetPasteRollback {
    /// Collapse rollback evidence to the latest canonical reference state.
    /// This is used when a completed paste lost its generation race: replaying
    /// the old disposition blindly could move a now-referenced asset to trash.
    pub fn convergence_effects(
        &self,
        references: &HashMap<stickymd_core::ManagedAssetName, usize>,
    ) -> Vec<stickymd_core::AssetEffect> {
        self.stored
            .iter()
            .map(|asset| asset.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|name| {
                let referenced = references.get(&name).copied().unwrap_or(0) > 0;
                let (from, to) = if referenced {
                    (ManagedAssetLocation::Trash, ManagedAssetLocation::Images)
                } else {
                    (ManagedAssetLocation::Images, ManagedAssetLocation::Trash)
                };
                stickymd_core::AssetEffect { name, from, to }
            })
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum AssetPasteError {
    #[error("clipboard file list contains a non-image or unsupported image")]
    MixedOrUnsupportedFiles,
    #[error("clipboard image read failed: {0}")]
    Read(std::io::Error),
    #[error("clipboard image preparation failed: {0}")]
    Image(stickymd_render::image::ImageAssetError),
    #[error("managed image publication failed: {0}")]
    Storage(AssetStorageError),
    #[error("image paste payload exceeds the {limit} byte transaction bound")]
    BatchTooLarge { limit: usize },
    #[error("clipboard payload is not an image transaction")]
    NotImage,
}

/// A failed preparation still returns every asset publication it performed.
/// Only the main coordinator can compare this ledger with the latest canonical
/// references and request a safe convergence; the worker must never replay an
/// old rollback decision by itself.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct AssetPasteFailure {
    pub expected_generation: Generation,
    pub error: AssetPasteError,
    pub rollback: AssetPasteRollback,
}

impl AssetPasteFailure {
    pub fn without_publication(expected_generation: Generation, error: AssetPasteError) -> Self {
        Self {
            expected_generation,
            error,
            rollback: AssetPasteRollback::default(),
        }
    }
}

pub fn prepare_and_store_paste(
    storage: &AssetStorage,
    request: AssetPasteRequest,
) -> Result<AssetPasteCompletion, AssetPasteFailure> {
    let expected_generation = request.expected_generation;
    let mut stored = Vec::new();
    let mut total_bytes = 0usize;
    let result = match request.payload {
        ClipboardPaste::EncodedImage(bytes) => add_batch_bytes(&mut total_bytes, bytes.len())
            .and_then(|()| {
                prepare_encoded_image(&bytes)
                    .map_err(AssetPasteError::Image)
                    .and_then(|image| store_prepared(storage, image, &mut stored))
            }),
        ClipboardPaste::Dib(bytes) => {
            add_batch_bytes(&mut total_bytes, bytes.len()).and_then(|()| {
                prepare_dib_image(&bytes)
                    .map_err(AssetPasteError::Image)
                    .and_then(|image| store_prepared(storage, image, &mut stored))
            })
        }
        ClipboardPaste::Rgba {
            width,
            height,
            bytes,
        } => add_batch_bytes(&mut total_bytes, bytes.len()).and_then(|()| {
            stickymd_render::image::prepare_rgba_image(width, height, bytes)
                .map_err(AssetPasteError::Image)
                .and_then(|image| store_prepared(storage, image, &mut stored))
        }),
        ClipboardPaste::Files(paths) if !paths.is_empty() => {
            let mut result = Ok(());
            for path in paths {
                let next = read_file_bounded(&path)
                    .and_then(|bytes| {
                        add_batch_bytes(&mut total_bytes, bytes.len())?;
                        prepare_encoded_image(&bytes)
                            .map_err(|_| AssetPasteError::MixedOrUnsupportedFiles)
                    })
                    .and_then(|image| store_prepared(storage, image, &mut stored));
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
        return Err(AssetPasteFailure {
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

fn add_batch_bytes(total: &mut usize, bytes: usize) -> Result<(), AssetPasteError> {
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

fn store_prepared(
    storage: &AssetStorage,
    image: PreparedImage,
    stored: &mut Vec<StoredAsset>,
) -> Result<(), AssetPasteError> {
    stored.push(storage.store(&image).map_err(AssetPasteError::Storage)?);
    Ok(())
}

fn read_file_bounded(path: &PathBuf) -> Result<Vec<u8>, AssetPasteError> {
    let file = File::open(path).map_err(AssetPasteError::Read)?;
    let mut bytes = Vec::new();
    file.take((MAX_ENCODED_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(AssetPasteError::Read)?;
    if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
        return Err(AssetPasteError::MixedOrUnsupportedFiles);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use stickymd_core::{
        CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, LineEnding,
        ManagedAssetName, Selection,
    };
    use stickymd_render::image::{inspect_encoded_image, prepare_rgba_image};

    fn fixture() -> (PathBuf, AssetStorage) {
        let root = unique_temp_path("asset-transaction");
        let images = root.join("images");
        let trash = root.join(".trash");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir(&trash).unwrap();
        let storage = AssetStorage::open(&images, &trash).unwrap();
        (root, storage)
    }

    fn png(color: u8) -> PreparedImage {
        prepare_rgba_image(2, 2, vec![color; 16]).unwrap()
    }

    fn request(files: Vec<PathBuf>) -> AssetPasteRequest {
        AssetPasteRequest {
            expected_generation: Generation::initial(),
            selection: Selection::caret(0),
            timestamp_ms: 0,
            payload: ClipboardPaste::Files(files),
        }
    }

    #[test]
    fn phase7_multi_image_paste_is_one_markdown_transaction() {
        let (root, storage) = fixture();
        let first = root.join("first.png");
        let second = root.join("second.png");
        fs::write(&first, png(1).bytes()).unwrap();
        fs::write(&second, png(2).bytes()).unwrap();
        let completion = prepare_and_store_paste(&storage, request(vec![first, second])).unwrap();
        assert_eq!(completion.rollback.stored.len(), 2);
        assert_eq!(completion.markdown.matches("![](images/").count(), 2);
        assert!(completion.markdown.contains("\n\n"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_second_asset_failure_rolls_first_out_of_active_directory() {
        let (root, storage) = fixture();
        let first_image = png(3);
        let second_image = png(4);
        let first = root.join("first.png");
        let second = root.join("second.png");
        fs::write(&first, first_image.bytes()).unwrap();
        fs::write(&second, second_image.bytes()).unwrap();
        let second_hash = second_image.hash().to_hex();
        for length in [20, 32, 64] {
            let collision =
                ManagedAssetName::from_hash_prefix(&second_hash, length, second_image.extension())
                    .unwrap();
            fs::write(root.join("images").join(collision.as_str()), b"untrusted").unwrap();
        }
        let result = prepare_and_store_paste(&storage, request(vec![first, second]));
        let failure = result.unwrap_err();
        assert!(
            matches!(
                &failure.error,
                AssetPasteError::Storage(AssetStorageError::NameCollisionExhausted)
            ),
            "unexpected transaction result: {failure:?}"
        );
        let first_name = ManagedAssetName::from_hash_prefix(
            &first_image.hash().to_hex(),
            20,
            first_image.extension(),
        )
        .unwrap();
        assert!(root.join("images").join(first_name.as_str()).is_file());
        let effects = failure.rollback.convergence_effects(&HashMap::new());
        assert_eq!(effects.len(), 1);
        storage.apply_effect(&effects[0]).unwrap();
        assert!(!root.join("images").join(first_name.as_str()).exists());
        assert!(root.join(".trash").join(first_name.as_str()).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_paste_ledger_converges_against_latest_references_instead_of_blind_rollback() {
        let image = png(9);
        let name =
            ManagedAssetName::from_hash_prefix(&image.hash().to_hex(), 20, image.extension())
                .unwrap();
        let rollback = AssetPasteRollback {
            stored: vec![StoredAsset {
                name: name.clone(),
                disposition: crate::assets::StoredAssetDisposition::Created,
            }],
        };
        let mut referenced = HashMap::new();
        referenced.insert(name.clone(), 1);
        assert_eq!(
            rollback.convergence_effects(&referenced),
            vec![stickymd_core::AssetEffect {
                name: name.clone(),
                from: ManagedAssetLocation::Trash,
                to: ManagedAssetLocation::Images,
            }]
        );
        assert_eq!(
            rollback.convergence_effects(&HashMap::new()),
            vec![stickymd_core::AssetEffect {
                name,
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            }]
        );
    }

    #[test]
    fn image_paste_batch_budget_is_cumulative_and_overflow_safe() {
        let mut total = 0;
        add_batch_bytes(&mut total, 64 * 1024 * 1024).unwrap();
        add_batch_bytes(&mut total, 64 * 1024 * 1024).unwrap();
        assert!(matches!(
            add_batch_bytes(&mut total, 1),
            Err(AssetPasteError::BatchTooLarge { .. })
        ));
        let mut overflow = usize::MAX;
        assert!(matches!(
            add_batch_bytes(&mut overflow, 1),
            Err(AssetPasteError::BatchTooLarge { .. })
        ));
    }

    #[test]
    #[ignore = "Release-only Phase 7 image-paste timing baseline"]
    fn phase7_image_paste_release_baseline() {
        use std::time::{Duration, Instant};

        let (root, _) = fixture();
        let mut files = Vec::new();
        for index in 0..10_u8 {
            let path = root.join(format!("source-{index}.png"));
            fs::write(&path, png(index).bytes()).unwrap();
            files.push(path);
        }
        let mut total_samples = Vec::new();
        let mut capture_samples = Vec::new();
        let mut inspect_samples = Vec::new();
        let mut prepare_samples = Vec::new();
        let mut persist_samples = Vec::new();
        let mut insert_samples = Vec::new();
        for sample in 0..30 {
            let sample_root = root.join(format!("sample-{sample}"));
            let images = sample_root.join("images");
            let trash = sample_root.join(".trash");
            fs::create_dir(&sample_root).unwrap();
            fs::create_dir(&images).unwrap();
            fs::create_dir(&trash).unwrap();
            let storage = AssetStorage::open(&images, &trash).unwrap();
            let started = Instant::now();
            let completion = prepare_and_store_paste(&storage, request(files.clone())).unwrap();
            total_samples.push(started.elapsed());
            assert_eq!(completion.rollback.stored.len(), 10);

            let capture_started = Instant::now();
            let captured = files
                .iter()
                .map(read_file_bounded)
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            capture_samples.push(capture_started.elapsed());

            let inspect_started = Instant::now();
            for bytes in &captured {
                inspect_encoded_image(bytes).unwrap();
            }
            inspect_samples.push(inspect_started.elapsed());

            let prepare_started = Instant::now();
            let prepared = captured
                .iter()
                .map(|bytes| prepare_encoded_image(bytes))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            prepare_samples.push(prepare_started.elapsed());

            let stage_root = root.join(format!("stage-{sample}"));
            let stage_images = stage_root.join("images");
            let stage_trash = stage_root.join(".trash");
            fs::create_dir(&stage_root).unwrap();
            fs::create_dir(&stage_images).unwrap();
            fs::create_dir(&stage_trash).unwrap();
            let stage_storage = AssetStorage::open(&stage_images, &stage_trash).unwrap();
            let persist_started = Instant::now();
            let stored = prepared
                .iter()
                .map(|image| stage_storage.store(image))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            persist_samples.push(persist_started.elapsed());

            let markdown = stored
                .iter()
                .map(|asset| format!("![](images/{})", asset.name))
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut document = DocumentState::empty(LineEnding::Lf);
            let insert_started = Instant::now();
            document
                .edit(EditRequest {
                    expected_generation: document.generation(),
                    range: 0..0,
                    inserted: markdown.clone(),
                    cursor_before: CursorSnapshot::new(Selection::caret(0)),
                    cursor_after: CursorSnapshot::new(Selection::caret(markdown.len())),
                    meta: EditMeta {
                        kind: EditKind::Paste,
                        timestamp_ms: 0,
                    },
                })
                .unwrap();
            insert_samples.push(insert_started.elapsed());
        }
        fn stats(samples: &mut [Duration]) -> (u128, u128, u128) {
            samples.sort_unstable();
            (
                samples[15].as_micros(),
                samples[28].as_micros(),
                samples[29].as_micros(),
            )
        }
        let total = stats(&mut total_samples);
        let capture = stats(&mut capture_samples);
        let inspect = stats(&mut inspect_samples);
        let prepare = stats(&mut prepare_samples);
        let persist = stats(&mut persist_samples);
        let insert = stats(&mut insert_samples);
        println!(
            "phase7 paste 10 PNG files total_median_us={} total_p95_us={} total_max_us={} \
             capture_median_us={} capture_p95_us={} capture_max_us={} \
             inspect_median_us={} inspect_p95_us={} inspect_max_us={} \
             prepare_median_us={} prepare_p95_us={} prepare_max_us={} \
             persist_median_us={} persist_p95_us={} persist_max_us={} \
             insert_median_us={} insert_p95_us={} insert_max_us={}",
            total.0,
            total.1,
            total.2,
            capture.0,
            capture.1,
            capture.2,
            inspect.0,
            inspect.1,
            inspect.2,
            prepare.0,
            prepare.1,
            prepare.2,
            persist.0,
            persist.1,
            persist.2,
            insert.0,
            insert.1,
            insert.2,
        );
        fs::remove_dir_all(root).unwrap();
    }
}
