//! Source-preserving Markdown export with staged local assets.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use stickymd_app::flow::{
    CopyFailure, CopyPlacement, EXPORT_ASSET_DIRECTORY_LIMIT, ExportPlan, copy_hashing,
    export_asset_directory_leaf, export_asset_extension, export_staging_name,
    export_temporary_name,
};
use stickymd_app::ports::ExportCompletion;
use stickymd_core::{DocumentSnapshot, Hash32, LoadedDocument};
use stickymd_render::preview::{ImageKind, PreviewParser, collect_image_occurrences};

use crate::assets::local_image_destination;
use crate::platform::macos::atomic_file::{
    AtomicPublishError, move_file_no_replace, prepare_temporary_exclusive, publish_prepared,
};
use crate::platform::macos::file::{DirectoryHandle, FileObservation};

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct OwnedStagingFile {
    path: PathBuf,
    observation: FileObservation,
}

/// Borrows the Note Directory for one export: the guard and the relative images use the held
/// directory itself, never a path to it.
#[derive(Debug)]
pub struct ExportRequest<'a> {
    pub snapshot: DocumentSnapshot,
    pub note_directory: &'a DirectoryHandle,
    pub target: PathBuf,
}

pub type ExportError = stickymd_app::flow::ExportError<AtomicPublishError>;

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
    if is_working_note_target(request.note_directory, &request.target)? {
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
    let staging_identity = staging
        .as_deref()
        .map(FileObservation::of_path)
        .transpose()
        .map_err(ExportError::StagingCreate)?;
    let mut markdown_temporary = None;
    let asset_directory = choose_asset_directory(parent, stem)?;
    let asset_leaf = asset_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(ExportError::InvalidTargetName)?
        .to_owned();
    let mut plan = ExportPlan::new(&asset_leaf);

    let mut owned_staging_files = Vec::new();
    let result = (|| {
        for (index, occurrence) in local.into_iter().enumerate() {
            let source = local_image_destination(&occurrence.destination)
                .map_err(|error| ExportError::UnsupportedLocalPath(error.to_string()))?;
            let output_name = if let Some(existing) = plan.name_for_source(&source) {
                existing.to_owned()
            } else {
                let staged_temporary = staging
                    .as_ref()
                    .ok_or(ExportError::InvalidTargetName)?
                    .join(format!("asset-{index}.tmp"));
                let (hash, extension, observation) =
                    copy_and_hash(request.note_directory, &source, &staged_temporary)?;
                owned_staging_files.push(OwnedStagingFile {
                    path: staged_temporary.clone(),
                    observation,
                });
                let name = match plan.place_copy(hash, &extension)? {
                    CopyPlacement::Duplicate(existing) => {
                        remove_owned_staging_file(&staged_temporary, observation).map_err(
                            |source| ExportError::LocalImage {
                                path: staged_temporary.clone(),
                                source,
                            },
                        )?;
                        existing
                    }
                    CopyPlacement::New(name) => {
                        let staged_final = staging
                            .as_ref()
                            .ok_or(ExportError::InvalidTargetName)?
                            .join(&name);
                        move_file_no_replace(&staged_temporary, &staged_final).map_err(
                            |source| ExportError::LocalImage {
                                path: staged_temporary.clone(),
                                source,
                            },
                        )?;
                        if let Some(owned) = owned_staging_files.last_mut() {
                            owned.path = staged_final;
                        }
                        plan.confirm_new(hash, name.clone());
                        name
                    }
                };
                plan.remember_source(source, name.clone());
                name
            };
            plan.rewrite(occurrence, &output_name);
        }

        let markdown = plan.rewritten_markdown(&request.snapshot.text)?;
        let durable = LoadedDocument::encode_runtime(&markdown, request.snapshot.line_ending);
        let prepared = prepare_unique_export_temporary(parent, &request.target, &durable)?;
        markdown_temporary = Some(prepared);
        if plan.copied_assets() != 0 {
            move_file_no_replace(
                staging.as_ref().ok_or(ExportError::InvalidTargetName)?,
                &asset_directory,
            )
            .map_err(ExportError::AssetPublish)?;
        }
        publish_prepared(
            &request.target,
            markdown_temporary
                .as_ref()
                .map(|owned: &OwnedStagingFile| owned.path.as_path())
                .ok_or(ExportError::InvalidTargetName)?,
        )
        .map_err(ExportError::MarkdownPublish)?;
        Ok(ExportCompletion {
            generation: request.snapshot.generation,
            target: request.target.clone(),
            copied_assets: plan.copied_assets(),
        })
    })();
    // Never recursively delete a path inside a user-selected destination.
    // Only remove files this invocation created by create_new/rename; if the
    // directory contains anything else, remove_dir fails closed and leaves
    // recoverable clutter rather than touching the extra entry.
    for owned in owned_staging_files.into_iter().rev() {
        let _ = remove_owned_staging_file(&owned.path, owned.observation);
    }
    if let (Some(staging), Some(identity)) = (&staging, &staging_identity)
        && FileObservation::of_path(staging).is_ok_and(|now| now.same_file_as(identity))
    {
        let _ = fs::remove_dir(staging);
    }
    if result.is_err()
        && let Some(markdown_temporary) = &markdown_temporary
    {
        let _ = remove_owned_staging_file(&markdown_temporary.path, markdown_temporary.observation);
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
        let staging = parent.join(export_staging_name(std::process::id(), sequence));
        match fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ExportError::StagingCreate(error)),
        }
    }
    Err(ExportError::StagingNameExhausted)
}

/// Whether `target` is the working note: the note's name in the note's directory, or the note file
/// itself under another name. Both compare the held directory and its file, not paths, so a
/// directory moved and replaced while the panel was open is still recognised.
fn is_working_note_target(directory: &DirectoryHandle, target: &Path) -> Result<bool, ExportError> {
    let Some(parent) = target.parent() else {
        return Ok(false);
    };
    let named_note = target
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("note.md"));
    let note_directory = directory
        .observation()
        .map_err(ExportError::TargetInspection)?;
    if named_note
        && FileObservation::of_followed_path(parent)
            .map_err(ExportError::TargetInspection)?
            .same_file_as(&note_directory)
    {
        return Ok(true);
    }
    let target = match FileObservation::of_followed_path(target) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(ExportError::TargetInspection(error)),
    };
    match directory.open_read("note.md") {
        Ok(note) => Ok(FileObservation::of(&note)
            .map_err(ExportError::TargetInspection)?
            .same_file_as(&target)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ExportError::TargetInspection(error)),
    }
}

fn prepare_unique_export_temporary(
    parent: &Path,
    target: &Path,
    bytes: &[u8],
) -> Result<OwnedStagingFile, ExportError> {
    for _ in 0..128 {
        let sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(export_temporary_name(std::process::id(), sequence));
        match prepare_temporary_exclusive(target, &temporary, bytes) {
            Ok(observation) => {
                return Ok(OwnedStagingFile {
                    path: temporary,
                    observation,
                });
            }
            Err(AtomicPublishError::TempCreate(error))
                if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ExportError::MarkdownPublish(error)),
        }
    }
    Err(ExportError::InvalidTargetName)
}

fn choose_asset_directory(parent: &Path, stem: &str) -> Result<PathBuf, ExportError> {
    for suffix in 1..=EXPORT_ASSET_DIRECTORY_LIMIT {
        let leaf = export_asset_directory_leaf(stem, suffix);
        let candidate = parent.join(leaf);
        if !candidate.try_exists().map_err(ExportError::StagingCreate)? {
            return Ok(candidate);
        }
    }
    Err(ExportError::InvalidTargetName)
}

fn copy_and_hash(
    note_directory: &DirectoryHandle,
    source: &Path,
    destination: &Path,
) -> Result<(Hash32, String, FileObservation), ExportError> {
    let mut input = note_directory
        .open_local_image(source)
        .map_err(|source_error| ExportError::LocalImage {
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
    // A read failure names the source image, a write failure the copy.
    let hash = copy_hashing(&mut input, &mut output).map_err(|failure| {
        let (path, source_error) = match failure {
            CopyFailure::Read(error) => (source, error),
            CopyFailure::Write(error) => (destination, error),
        };
        ExportError::LocalImage {
            path: path.to_owned(),
            source: source_error,
        }
    })?;
    output
        .sync_all()
        .map_err(|source_error| ExportError::LocalImage {
            path: destination.to_owned(),
            source: source_error,
        })?;
    let observation =
        FileObservation::of(&output).map_err(|source_error| ExportError::LocalImage {
            path: destination.to_owned(),
            source: source_error,
        })?;
    Ok((hash, export_asset_extension(source), observation))
}

/// The file is identified by what it is, not by the moment it was seen: moving it into place
/// changed its metadata but not its identity.
fn remove_owned_staging_file(path: &Path, expected: FileObservation) -> std::io::Result<()> {
    if !FileObservation::of_path(path)?.same_file_as(&expected) {
        return Err(std::io::Error::other(
            "export staging identity changed before cleanup",
        ));
    }
    fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::resolve_local_image;
    use crate::test_support::unique_temp_path;
    use std::sync::Arc;
    use stickymd_core::{Generation, LineEnding};
    use stickymd_desktop::testing::contracts;

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
        let directory = DirectoryHandle::open(&root).unwrap();
        let (_, _, observation) = copy_and_hash(&directory, &source, &staging).unwrap();
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
    fn export_copies_only_real_local_images_and_preserves_other_source() {
        contracts::export_copies_only_real_local_images_and_preserves_other_source(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    #[test]
    fn export_missing_local_image_fails_before_any_publish() {
        contracts::export_missing_local_image_fails_before_any_publish(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    #[test]
    fn export_denied_local_paths_preserve_existing_output() {
        contracts::export_denied_local_paths_preserve_existing_output(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    #[test]
    fn export_deduplicates_sources_suffixes_existing_assets_and_replaces_markdown() {
        contracts::export_deduplicates_sources_suffixes_existing_assets_and_replaces_markdown(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    #[test]
    fn export_without_images_preserves_markdown_bytes_and_creates_no_asset_dir() {
        contracts::export_without_images_preserves_markdown_bytes_and_creates_no_asset_dir(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    #[test]
    fn export_rejects_the_working_note_and_hard_link_alias_without_modifying_either() {
        contracts::export_rejects_the_working_note_and_hard_link_alias_without_modifying_either(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    /// The directory moved and another took its old path after the handle was opened: the guard
    /// still finds the working note where it went, and the impostor is an ordinary target.
    #[test]
    fn export_recognises_the_working_note_after_its_directory_moved() {
        let root = fixture();
        let note = root.join("note");
        let moved = root.join("moved");
        fs::create_dir(&note).unwrap();
        fs::write(note.join("note.md"), b"durable working note").unwrap();
        fs::write(note.join("a.png"), b"original image").unwrap();
        let directory = DirectoryHandle::open(&note).unwrap();
        fs::rename(&note, &moved).unwrap();
        fs::create_dir(&note).unwrap();
        fs::write(note.join("a.png"), b"impostor image").unwrap();
        let snapshot = |text: &str| DocumentSnapshot {
            text: Arc::from(text),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        for target in [moved.join("note.md"), moved.join("Note.MD")] {
            let result = export_snapshot(ExportRequest {
                snapshot: snapshot("replacement"),
                note_directory: &directory,
                target,
            });
            assert!(matches!(result, Err(ExportError::WorkingNoteTarget)));
        }
        assert_eq!(
            fs::read(moved.join("note.md")).unwrap(),
            b"durable working note"
        );
        export_snapshot(ExportRequest {
            snapshot: snapshot("![a](a.png)"),
            note_directory: &directory,
            target: note.join("note.md"),
        })
        .unwrap();
        let copied = fs::read_dir(note.join("note-assets"))
            .unwrap()
            .map(|entry| fs::read(entry.unwrap().path()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(copied, [b"original image".to_vec()]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_percent_encodes_asset_directory_and_remains_parseable_and_resolvable() {
        contracts::export_percent_encodes_asset_directory_and_remains_parseable_and_resolvable(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
            resolve_local_image,
        );
    }

    #[test]
    fn export_snapshot_cannot_mutate_document_authority_or_history() {
        contracts::export_snapshot_cannot_mutate_document_authority_or_history(
            fixture(),
            |note_dir, snapshot, target| {
                export_snapshot(ExportRequest {
                    snapshot,
                    note_directory: &DirectoryHandle::open(note_dir).unwrap(),
                    target,
                })
            },
        );
    }

    #[test]
    fn non_uri_unicode_destination_is_resolved_without_panicking() {
        let note_dir = Path::new("note");
        assert_eq!(
            resolve_local_image(note_dir, "数学图像.png").unwrap(),
            note_dir.join("数学图像.png")
        );
    }

    #[test]
    #[ignore = "Release-only Phase 7 export timing baseline"]
    fn export_release_baseline() {
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
                note_directory: &DirectoryHandle::open(&note).unwrap(),
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
