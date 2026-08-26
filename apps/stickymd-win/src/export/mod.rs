//! Source-preserving Markdown export with staged local assets.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use stickymd_core::{DocumentSnapshot, Hash32, LoadedDocument};
use stickymd_render::preview::{
    ImageKind, ImageRewrite, PreviewParser, collect_image_occurrences, rewrite_image_occurrences,
};
use thiserror::Error;

use crate::assets::resolve_local_image;
use crate::platform::windows::atomic_file::{
    AtomicPublishError, move_file_no_replace, prepare_temporary_exclusive, publish_prepared,
};
use crate::platform::windows::file_identity::{
    OpenFileObservation, observe_open_file, same_existing_file,
};
use crate::platform::windows::managed_file::{delete_open_file, open_for_managed_mutation};

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct OwnedStagingFile {
    path: PathBuf,
    observation: OpenFileObservation,
}

#[derive(Debug)]
pub struct ExportRequest {
    pub snapshot: DocumentSnapshot,
    pub note_dir: PathBuf,
    pub target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCompletion {
    pub generation: stickymd_core::Generation,
    pub target: PathBuf,
    pub copied_assets: usize,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("export target has no parent directory")]
    MissingParent,
    #[error("export target has no usable file stem")]
    InvalidTargetName,
    #[error("export target is the canonical working note")]
    WorkingNoteTarget,
    #[error("cannot inspect export target identity: {0}")]
    TargetInspection(std::io::Error),
    #[error("cannot parse Markdown for export: {0}")]
    Parse(#[from] stickymd_render::preview::PreviewParseError),
    #[error("cannot project Markdown images for export: {0}")]
    Projection(#[from] stickymd_render::preview::ExportProjectionError),
    #[error("local image path is unsupported: {0}")]
    UnsupportedLocalPath(String),
    #[error("referenced local image is unavailable: {path}: {source}")]
    LocalImage {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot create export staging directory: {0}")]
    StagingCreate(std::io::Error),
    #[error("cannot allocate a unique export staging directory name")]
    StagingNameExhausted,
    #[error("cannot publish export asset directory: {0}")]
    AssetPublish(std::io::Error),
    #[error("cannot publish exported Markdown: {0}")]
    MarkdownPublish(#[from] AtomicPublishError),
    #[error("export asset hash-prefix collision could not be represented safely")]
    AssetNameCollision,
}

/// Export one immutable snapshot. Local files are staged and published before
/// the Markdown file; a failure can therefore never publish Markdown whose
/// generated asset references are absent.
pub fn export_snapshot(request: ExportRequest) -> Result<ExportCompletion, ExportError> {
    let parent = request.target.parent().ok_or(ExportError::MissingParent)?;
    let stem = request
        .target
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or(ExportError::InvalidTargetName)?;
    let working_note = request.note_dir.join("note.md");
    if is_working_note_target(&request.note_dir, &working_note, &request.target)? {
        return Err(ExportError::WorkingNoteTarget);
    }
    let tree = PreviewParser.parse(&request.snapshot)?;
    let occurrences = collect_image_occurrences(&tree)?;
    let local = occurrences
        .into_iter()
        .filter(|image| {
            matches!(
                image.kind,
                ImageKind::LocalRelative | ImageKind::LocalAbsolute
            )
        })
        .collect::<Vec<_>>();

    let staging = (!local.is_empty())
        .then(|| create_unique_export_staging(parent))
        .transpose()?;
    let mut markdown_temporary = None;
    let asset_directory = choose_asset_directory(parent, stem)?;
    let asset_leaf = asset_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(ExportError::InvalidTargetName)?
        .to_owned();
    let markdown_asset_leaf = encode_markdown_path_segment(&asset_leaf);

    let mut owned_staging_files = Vec::new();
    let result = (|| {
        let mut rewrites = Vec::with_capacity(local.len());
        let mut copied = HashMap::<Hash32, String>::new();
        let mut occupied_names = HashMap::<String, Hash32>::new();
        let mut resolved_sources = HashMap::<PathBuf, String>::new();
        for (index, occurrence) in local.into_iter().enumerate() {
            let source = resolve_local_image(&request.note_dir, &occurrence.destination)
                .map_err(|error| ExportError::UnsupportedLocalPath(error.to_string()))?;
            let output_name = if let Some(existing) = resolved_sources.get(&source) {
                existing.clone()
            } else {
                let staged_temporary = staging
                    .as_ref()
                    .ok_or(ExportError::InvalidTargetName)?
                    .join(format!("asset-{index}.tmp"));
                let (hash, extension, observation) = copy_and_hash(&source, &staged_temporary)?;
                owned_staging_files.push(OwnedStagingFile {
                    path: staged_temporary.clone(),
                    observation,
                });
                let name = if let Some(existing) = copied.get(&hash) {
                    remove_owned_staging_file(&staged_temporary, observation).map_err(
                        |source| ExportError::LocalImage {
                            path: staged_temporary.clone(),
                            source,
                        },
                    )?;
                    existing.clone()
                } else {
                    let name = choose_export_asset_name(hash, &extension, &occupied_names)?;
                    let staged_final = staging
                        .as_ref()
                        .ok_or(ExportError::InvalidTargetName)?
                        .join(&name);
                    move_file_no_replace(&staged_temporary, &staged_final).map_err(|source| {
                        ExportError::LocalImage {
                            path: staged_temporary.clone(),
                            source,
                        }
                    })?;
                    if let Some(owned) = owned_staging_files.last_mut() {
                        owned.path = staged_final;
                    }
                    copied.insert(hash, name.clone());
                    occupied_names.insert(name.clone(), hash);
                    name
                };
                resolved_sources.insert(source, name.clone());
                name
            };
            rewrites.push(ImageRewrite {
                source_range: occurrence.source_range,
                destination: format!("{markdown_asset_leaf}/{output_name}"),
                title: occurrence.title,
                alt: occurrence.alt,
            });
        }

        let markdown = rewrite_image_occurrences(&request.snapshot.text, rewrites)?;
        let durable = LoadedDocument::encode_runtime(&markdown, request.snapshot.line_ending);
        let prepared = prepare_unique_export_temporary(parent, &request.target, &durable)?;
        markdown_temporary = Some(prepared);
        if !copied.is_empty() {
            move_file_no_replace(
                staging.as_ref().ok_or(ExportError::InvalidTargetName)?,
                &asset_directory,
            )
            .map_err(ExportError::AssetPublish)?;
        }
        publish_prepared(
            &request.target,
            markdown_temporary
                .as_deref()
                .ok_or(ExportError::InvalidTargetName)?,
        )?;
        Ok(ExportCompletion {
            generation: request.snapshot.generation,
            target: request.target.clone(),
            copied_assets: copied.len(),
        })
    })();
    // Never recursively delete a path inside a user-selected destination.
    // Only remove files this invocation created by create_new/rename; if the
    // directory contains anything else, remove_dir fails closed and leaves
    // recoverable clutter rather than touching the extra entry.
    for owned in owned_staging_files.into_iter().rev() {
        let _ = remove_owned_staging_file(&owned.path, owned.observation);
    }
    if let Some(staging) = &staging {
        let _ = fs::remove_dir(staging);
    }
    if result.is_err()
        && let Some(markdown_temporary) = &markdown_temporary
    {
        let _ = fs::remove_file(markdown_temporary);
    }
    result
}

fn create_unique_export_staging(parent: &Path) -> Result<PathBuf, ExportError> {
    create_unique_export_staging_with_sequence(parent, &EXPORT_SEQUENCE)
}

fn create_unique_export_staging_with_sequence(
    parent: &Path,
    sequence: &AtomicU64,
) -> Result<PathBuf, ExportError> {
    for _ in 0..128 {
        let sequence = sequence.fetch_add(1, Ordering::Relaxed);
        let staging = parent.join(format!(
            ".stickymd-export-{}-{sequence}.staging",
            std::process::id()
        ));
        match fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ExportError::StagingCreate(error)),
        }
    }
    Err(ExportError::StagingNameExhausted)
}

fn is_working_note_target(
    note_dir: &Path,
    working_note: &Path,
    target: &Path,
) -> Result<bool, ExportError> {
    if working_note
        .try_exists()
        .map_err(ExportError::TargetInspection)?
        && target.try_exists().map_err(ExportError::TargetInspection)?
        && same_existing_file(working_note, target).map_err(ExportError::TargetInspection)?
    {
        return Ok(true);
    }
    let Some(parent) = target.parent() else {
        return Ok(false);
    };
    let canonical_note_dir = fs::canonicalize(note_dir).map_err(ExportError::TargetInspection)?;
    let canonical_parent = fs::canonicalize(parent).map_err(ExportError::TargetInspection)?;
    Ok(canonical_note_dir == canonical_parent
        && target
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("note.md")))
}

fn prepare_unique_export_temporary(
    parent: &Path,
    target: &Path,
    bytes: &[u8],
) -> Result<PathBuf, ExportError> {
    for _ in 0..128 {
        let sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".stickymd-export-{}-{sequence}.tmp",
            std::process::id()
        ));
        match prepare_temporary_exclusive(target, &temporary, bytes) {
            Ok(()) => return Ok(temporary),
            Err(AtomicPublishError::TempCreate(error))
                if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ExportError::MarkdownPublish(error)),
        }
    }
    Err(ExportError::InvalidTargetName)
}

fn choose_export_asset_name(
    hash: Hash32,
    extension: &str,
    occupied: &HashMap<String, Hash32>,
) -> Result<String, ExportError> {
    let hex = hash.to_hex();
    for length in [20usize, 32, 64] {
        let name = format!("stickymd-{}.{}", &hex[..length], extension);
        match occupied.get(&name) {
            None => return Ok(name),
            Some(existing) if *existing == hash => return Ok(name),
            Some(_) => {}
        }
    }
    Err(ExportError::AssetNameCollision)
}

fn encode_markdown_path_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            output.push('%');
            output.push(char::from(HEX[(byte >> 4) as usize]));
            output.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    output
}

fn choose_asset_directory(parent: &Path, stem: &str) -> Result<PathBuf, ExportError> {
    for suffix in 1_u32..=10_000 {
        let leaf = if suffix == 1 {
            format!("{stem}-assets")
        } else {
            format!("{stem}-assets-{suffix}")
        };
        let candidate = parent.join(leaf);
        if !candidate.try_exists().map_err(ExportError::StagingCreate)? {
            return Ok(candidate);
        }
    }
    Err(ExportError::InvalidTargetName)
}

fn copy_and_hash(
    source: &Path,
    destination: &Path,
) -> Result<(Hash32, String, OpenFileObservation), ExportError> {
    let mut input = File::open(source).map_err(|source_error| ExportError::LocalImage {
        path: source.to_owned(),
        source: source_error,
    })?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|source_error| ExportError::LocalImage {
            path: destination.to_owned(),
            source: source_error,
        })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|source_error| ExportError::LocalImage {
                path: source.to_owned(),
                source: source_error,
            })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|source_error| ExportError::LocalImage {
                path: destination.to_owned(),
                source: source_error,
            })?;
    }
    output
        .sync_all()
        .map_err(|source_error| ExportError::LocalImage {
            path: destination.to_owned(),
            source: source_error,
        })?;
    let observation =
        observe_open_file(&output).map_err(|source_error| ExportError::LocalImage {
            path: destination.to_owned(),
            source: source_error,
        })?;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 10
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "img".to_owned());
    Ok((
        Hash32::new(digest.finalize().into()),
        extension,
        observation,
    ))
}

fn remove_owned_staging_file(path: &Path, expected: OpenFileObservation) -> std::io::Result<()> {
    let file = open_for_managed_mutation(path)?;
    let observed = observe_open_file(&file)?;
    if observed != expected {
        return Err(std::io::Error::other(
            "export staging identity changed before cleanup",
        ));
    }
    delete_open_file(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::sync::Arc;
    use stickymd_core::{
        CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, Generation, LineEnding,
    };

    fn fixture() -> PathBuf {
        let root = unique_temp_path("export");
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn stale_export_staging_directory_is_preserved_and_skipped() {
        let root = fixture();
        let sequence = AtomicU64::new(7);
        let occupied = root.join(format!(".stickymd-export-{}-7.staging", std::process::id()));
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("recovery-evidence"), b"keep").unwrap();

        let chosen = create_unique_export_staging_with_sequence(&root, &sequence).unwrap();
        assert_ne!(chosen, occupied);
        assert_eq!(
            fs::read(occupied.join("recovery-evidence")).unwrap(),
            b"keep"
        );

        fs::remove_dir(chosen).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_cleanup_never_deletes_a_path_replaced_after_ownership_proof() {
        let root = fixture();
        let source = root.join("source.png");
        let staging = root.join("asset.tmp");
        let original = root.join("original-owned.tmp");
        fs::write(&source, b"owned export bytes").unwrap();
        let (_, _, observation) = copy_and_hash(&source, &staging).unwrap();
        fs::rename(&staging, &original).unwrap();
        fs::write(&staging, b"external replacement").unwrap();

        assert!(remove_owned_staging_file(&staging, observation).is_err());
        assert_eq!(fs::read(&staging).unwrap(), b"external replacement");
        assert_eq!(fs::read(&original).unwrap(), b"owned export bytes");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exhausted_export_staging_names_return_a_typed_error() {
        let root = fixture();
        let sequence = AtomicU64::new(0);
        for number in 0..128 {
            fs::create_dir(root.join(format!(
                ".stickymd-export-{}-{number}.staging",
                std::process::id()
            )))
            .unwrap();
        }

        assert!(matches!(
            create_unique_export_staging_with_sequence(&root, &sequence),
            Err(ExportError::StagingNameExhausted)
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_export_copies_only_real_local_images_and_preserves_other_source() {
        let root = fixture();
        let note = root.join("note");
        let out = root.join("out");
        fs::create_dir(&note).unwrap();
        fs::create_dir(&out).unwrap();
        fs::write(note.join("a.png"), b"local-image").unwrap();
        let source = "before\n\n![x](a.png \"T\")\n\n![remote](https://example/a.png)\n\n`![code](missing.png)`\n";
        let result = export_snapshot(ExportRequest {
            snapshot: DocumentSnapshot {
                text: Arc::from(source),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            note_dir: note,
            target: out.join("copy.md"),
        })
        .unwrap();
        assert_eq!(result.copied_assets, 1);
        let markdown = fs::read_to_string(out.join("copy.md")).unwrap();
        assert!(markdown.contains("![x](copy-assets/stickymd-"));
        assert!(markdown.contains("![remote](https://example/a.png)"));
        assert!(markdown.contains("`![code](missing.png)`"));
        assert_eq!(fs::read_dir(out.join("copy-assets")).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_export_missing_local_image_fails_before_any_publish() {
        let root = fixture();
        let target = root.join("copy.md");
        fs::write(root.join("present.png"), b"present").unwrap();
        let result = export_snapshot(ExportRequest {
            snapshot: DocumentSnapshot {
                text: Arc::from("![ok](present.png)\n\n![x](missing.png)"),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            note_dir: root.clone(),
            target: target.clone(),
        });
        assert!(matches!(result, Err(ExportError::LocalImage { .. })));
        assert!(!target.exists());
        assert!(!root.join("copy-assets").exists());
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".stickymd-export-")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_export_deduplicates_sources_suffixes_existing_assets_and_replaces_markdown() {
        let root = fixture();
        let note = root.join("note");
        let out = root.join("out");
        fs::create_dir(&note).unwrap();
        fs::create_dir(&out).unwrap();
        let first = note.join("first.png");
        let second = note.join("second.png");
        fs::write(&first, b"identical bytes").unwrap();
        fs::write(&second, b"identical bytes").unwrap();
        fs::create_dir(out.join("copy-assets")).unwrap();
        fs::write(out.join("copy-assets/user.txt"), b"keep").unwrap();
        fs::write(out.join("copy.md"), b"old").unwrap();
        let absolute = second.to_string_lossy().replace('\\', "/");
        let source = format!(
            "prefix\n\n![one](first.png)\n\n![two]({absolute})\n\n![again](first.png)\n\nsuffix\n"
        );
        let completion = export_snapshot(ExportRequest {
            snapshot: DocumentSnapshot {
                text: Arc::from(source),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            note_dir: note,
            target: out.join("copy.md"),
        })
        .unwrap();
        assert_eq!(completion.copied_assets, 1);
        assert_eq!(fs::read(out.join("copy-assets/user.txt")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(out.join("copy-assets-2")).unwrap().count(), 1);
        let markdown = fs::read_to_string(out.join("copy.md")).unwrap();
        assert_eq!(markdown.matches("copy-assets-2/stickymd-").count(), 3);
        assert!(markdown.starts_with("prefix\n\n"));
        assert!(markdown.ends_with("\n\nsuffix\n"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_export_without_images_preserves_markdown_bytes_and_creates_no_asset_dir() {
        let root = fixture();
        let source = "# Exact\n\n`code`  \n\nraw <x>\n";
        export_snapshot(ExportRequest {
            snapshot: DocumentSnapshot {
                text: Arc::from(source),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            note_dir: root.clone(),
            target: root.join("copy.md"),
        })
        .unwrap();
        assert_eq!(fs::read(root.join("copy.md")).unwrap(), source.as_bytes());
        assert!(!root.join("copy-assets").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_rejects_the_working_note_and_hard_link_alias_without_modifying_either() {
        let root = fixture();
        let note = root.join("note");
        let out = root.join("out");
        fs::create_dir(&note).unwrap();
        fs::create_dir(&out).unwrap();
        let working = note.join("note.md");
        let alias = out.join("alias.md");
        fs::write(&working, b"durable working note").unwrap();
        fs::hard_link(&working, &alias).unwrap();
        for target in [working.clone(), alias.clone()] {
            let result = export_snapshot(ExportRequest {
                snapshot: DocumentSnapshot {
                    text: Arc::from("replacement"),
                    generation: Generation::initial(),
                    line_ending: LineEnding::Lf,
                },
                note_dir: note.clone(),
                target,
            });
            assert!(matches!(result, Err(ExportError::WorkingNoteTarget)));
            assert_eq!(fs::read(&working).unwrap(), b"durable working note");
            assert_eq!(fs::read(&alias).unwrap(), b"durable working note");
        }
        fs::remove_file(&alias).unwrap();
        fs::remove_file(&working).unwrap();
        let absent_result = export_snapshot(ExportRequest {
            snapshot: DocumentSnapshot {
                text: Arc::from("replacement"),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            note_dir: note.clone(),
            target: note.join("NOTE.MD"),
        });
        assert!(matches!(absent_result, Err(ExportError::WorkingNoteTarget)));
        assert!(!note.join("NOTE.MD").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_percent_encodes_asset_directory_and_remains_parseable_and_resolvable() {
        let root = fixture();
        let note = root.join("note");
        let out = root.join("out");
        fs::create_dir(&note).unwrap();
        fs::create_dir(&out).unwrap();
        fs::write(note.join("source.png"), b"asset bytes").unwrap();
        let target = out.join("my note.md");
        export_snapshot(ExportRequest {
            snapshot: DocumentSnapshot {
                text: Arc::from("before ![alt](source.png) after"),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            note_dir: note,
            target: target.clone(),
        })
        .unwrap();
        let markdown = fs::read_to_string(&target).unwrap();
        assert!(markdown.contains("my%20note-assets/stickymd-"));
        let parsed = PreviewParser
            .parse(&DocumentSnapshot {
                text: Arc::from(markdown),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            })
            .unwrap();
        let occurrence = collect_image_occurrences(&parsed).unwrap().remove(0);
        let resolved = resolve_local_image(&out, &occurrence.destination).unwrap();
        assert!(resolved.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_asset_name_expands_when_twenty_hex_prefix_is_occupied() {
        let mut first = [0_u8; 32];
        first[10] = 1;
        let mut second = [0_u8; 32];
        second[10] = 2;
        let first = Hash32::new(first);
        let second = Hash32::new(second);
        let occupied = HashMap::from([(format!("stickymd-{}.png", &first.to_hex()[..20]), first)]);
        let name = choose_export_asset_name(second, "png", &occupied).unwrap();
        assert_eq!(name, format!("stickymd-{}.png", &second.to_hex()[..32]));
    }

    #[test]
    fn phase7_export_snapshot_cannot_mutate_document_authority_or_history() {
        let root = fixture();
        let mut document = DocumentState::loaded("before", LineEnding::Lf, None);
        document
            .edit(EditRequest::new(
                document.generation(),
                6..6,
                " after",
                CursorSnapshot::caret(6),
                CursorSnapshot::caret(12),
                EditMeta::new(EditKind::Typing, 0),
            ))
            .unwrap();
        let before = (
            document.text().to_owned(),
            document.generation(),
            document.saved_generation(),
            document.is_dirty(),
            document.can_undo(),
            document.can_redo(),
        );

        export_snapshot(ExportRequest {
            snapshot: document.snapshot(),
            note_dir: root.clone(),
            target: root.join("copy.md"),
        })
        .unwrap();

        assert_eq!(
            (
                document.text().to_owned(),
                document.generation(),
                document.saved_generation(),
                document.is_dirty(),
                document.can_undo(),
                document.can_redo(),
            ),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_non_uri_unicode_destination_is_resolved_without_panicking() {
        let note_dir = Path::new("note");
        assert_eq!(
            resolve_local_image(note_dir, "数学图像.png").unwrap(),
            note_dir.join("数学图像.png")
        );
    }

    #[test]
    #[ignore = "Release-only Phase 7 export timing baseline"]
    fn phase7_export_release_baseline() {
        use std::time::Instant;

        let root = fixture();
        let note = root.join("note");
        let out = root.join("out");
        fs::create_dir(&note).unwrap();
        fs::create_dir(&out).unwrap();
        fs::write(note.join("asset.png"), vec![7_u8; 1024 * 1024]).unwrap();
        let source = (0..20)
            .map(|index| format!("![image-{index}](asset.png)\n\n"))
            .collect::<String>();
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut samples = Vec::new();
        for index in 0..30 {
            let started = Instant::now();
            let completion = export_snapshot(ExportRequest {
                snapshot: snapshot.clone(),
                note_dir: note.clone(),
                target: out.join(format!("copy-{index}.md")),
            })
            .unwrap();
            samples.push(started.elapsed());
            assert_eq!(completion.copied_assets, 1);
        }
        samples.sort_unstable();
        println!(
            "phase7 export 20-references/1MiB-shared-asset median_us={} p95_us={} max_us={}",
            samples[15].as_micros(),
            samples[28].as_micros(),
            samples[29].as_micros()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
