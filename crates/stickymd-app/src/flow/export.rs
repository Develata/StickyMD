//! Pure export naming, stream copying and failures shared by the platform hosts.
//! File ownership, durability and publication remain in each host.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use std::io::{Read, Write};
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use stickymd_core::Hash32;
use thiserror::Error;

use super::ExportNameCollision;
use crate::ports::{FailureKind, StorageFailure};

/// The last asset directory candidate; hosts probe candidates from 1 through this bound.
pub const EXPORT_ASSET_DIRECTORY_LIMIT: u32 = 10_000;

pub fn export_staging_name(pid: u32, sequence: u64) -> String {
    format!(".stickymd-export-{pid}-{sequence}.staging")
}

pub fn export_temporary_name(pid: u32, sequence: u64) -> String {
    format!(".stickymd-export-{pid}-{sequence}.tmp")
}

/// Names candidate `n` in `1..=EXPORT_ASSET_DIRECTORY_LIMIT`; existence is the host's decision.
pub fn export_asset_directory_leaf(stem: &str, n: u32) -> String {
    if n == 1 {
        format!("{stem}-assets")
    } else {
        format!("{stem}-assets-{n}")
    }
}

/// Which stream failed in [`copy_hashing`], so the host can name the matching file.
#[derive(Debug)]
pub enum CopyFailure {
    Read(std::io::Error),
    Write(std::io::Error),
}

/// Copies and hashes the same bytes, without flushing or inspecting either stream.
pub fn copy_hashing(input: &mut impl Read, output: &mut impl Write) -> Result<Hash32, CopyFailure> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(CopyFailure::Read)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(CopyFailure::Write)?;
    }
    Ok(Hash32::new(digest.finalize().into()))
}

/// Export failures with the host's atomic Markdown publish error as their only platform type.
#[derive(Debug, Error)]
pub enum ExportError<P> {
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
    MarkdownPublish(#[source] P),
    #[error("{0}")]
    AssetNameCollision(#[from] ExportNameCollision),
}

impl<P: std::fmt::Display> From<ExportError<P>> for StorageFailure {
    fn from(error: ExportError<P>) -> Self {
        Self::new(FailureKind::Failed, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stickymd_core::hash_bytes;

    #[test]
    fn export_failure_conversion_preserves_failed_kind_and_message() {
        for error in [
            ExportError::WorkingNoteTarget,
            ExportError::LocalImage {
                path: PathBuf::from("image.png"),
                source: std::io::Error::other("read failed"),
            },
            ExportError::MarkdownPublish("publish failed"),
        ] {
            let message = error.to_string();
            let failure = StorageFailure::from(error);
            assert_eq!(failure.kind(), FailureKind::Failed);
            assert_eq!(failure.to_string(), message);
        }
    }

    #[test]
    fn export_names_preserve_pid_sequence_and_suffix() {
        assert_eq!(export_staging_name(42, 7), ".stickymd-export-42-7.staging");
        assert_eq!(export_temporary_name(42, 7), ".stickymd-export-42-7.tmp");
        assert_eq!(export_staging_name(0, 0), ".stickymd-export-0-0.staging");
        assert_eq!(
            export_temporary_name(u32::MAX, u64::MAX),
            ".stickymd-export-4294967295-18446744073709551615.tmp"
        );
    }

    #[test]
    fn asset_directory_sequence_preserves_the_unsuffixed_first_candidate_and_bound() {
        assert_eq!(EXPORT_ASSET_DIRECTORY_LIMIT, 10_000);
        assert_eq!(export_asset_directory_leaf("my note", 1), "my note-assets");
        for n in 2..=EXPORT_ASSET_DIRECTORY_LIMIT {
            assert_eq!(
                export_asset_directory_leaf("my note", n),
                format!("my note-assets-{n}")
            );
        }
    }

    #[test]
    fn copy_hashing_preserves_bytes_and_digest_across_buffer_boundaries() {
        let bytes = (0..(64 * 1024 * 2 + 17))
            .map(|n| (n % 251) as u8)
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        let digest = copy_hashing(&mut bytes.as_slice(), &mut output).unwrap();
        assert_eq!(digest, hash_bytes(&bytes));
        assert_eq!(output, bytes);
    }

    #[test]
    fn copy_hashing_accepts_empty_input() {
        let mut output = Vec::new();
        assert_eq!(
            copy_hashing(&mut &b""[..], &mut output).unwrap(),
            hash_bytes(b"")
        );
        assert!(output.is_empty());
    }

    #[test]
    fn copy_hashing_handles_short_reads_and_writes_without_flushing() {
        struct ShortReader<'a>(&'a [u8]);
        impl Read for ShortReader<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let limit = buffer.len().min(3);
                self.0.read(&mut buffer[..limit])
            }
        }
        struct ShortWriter(Vec<u8>);
        impl Write for ShortWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let count = bytes.len().min(2);
                self.0.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                panic!("durability belongs to the host")
            }
        }
        let bytes = b"short reads and writes";
        let mut input = ShortReader(bytes);
        let mut output = ShortWriter(Vec::new());
        assert_eq!(
            copy_hashing(&mut input, &mut output).unwrap(),
            hash_bytes(bytes)
        );
        assert_eq!(output.0, bytes);
    }

    #[test]
    fn copy_hashing_returns_read_errors_without_retrying_or_writing() {
        struct FailedRead;
        impl Read for FailedRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "read failed",
                ))
            }
        }
        let mut output = Vec::new();
        let CopyFailure::Read(error) = copy_hashing(&mut FailedRead, &mut output).unwrap_err()
        else {
            panic!("a read failure is reported as one");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        assert_eq!(error.to_string(), "read failed");
        assert!(output.is_empty());
    }

    #[test]
    fn copy_hashing_returns_write_errors() {
        struct FailedWrite;
        impl Write for FailedWrite {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "write failed",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                panic!("durability belongs to the host")
            }
        }
        let CopyFailure::Write(error) =
            copy_hashing(&mut &b"data"[..], &mut FailedWrite).unwrap_err()
        else {
            panic!("a write failure is reported as one");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "write failed");
    }
}
