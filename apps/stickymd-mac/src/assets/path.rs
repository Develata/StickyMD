//! Local Markdown image destination resolution shared by Preview and Export.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use stickymd_render::preview::{DestinationDecodeError, decode_destination};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LocalImagePathError {
    #[error("local image path has invalid percent encoding: {0}")]
    InvalidPercentEncoding(String),
    #[error("local image path is not valid UTF-8 after percent decoding: {0}")]
    InvalidUtf8(String),
    #[error("image destination is not an ordinary local path: {0}")]
    NonLocalPath(String),
}

/// Where a destination points, as a path under `note_dir`: what the tests compare.
#[cfg(test)]
pub fn resolve_local_image(
    note_dir: &Path,
    destination: &str,
) -> Result<PathBuf, LocalImagePathError> {
    Ok(note_dir.join(local_image_destination(destination)?))
}

/// Decodes a Comrak image destination for read-only local access. A relative path stays relative,
/// to be opened from the Note Directory's handle, so a moved directory is followed with no path
/// resolved in between; a `file:` URL names a path on this machine. This never grants write or
/// managed-asset ownership authority; the adapter that opens the file refuses links and network
/// volumes.
pub fn local_image_destination(destination: &str) -> Result<PathBuf, LocalImagePathError> {
    let destination = destination.trim();
    let decoded = if destination
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:///"))
    {
        format!("/{}", percent_decode(&destination[8..])?)
    } else if destination
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        return Err(LocalImagePathError::NonLocalPath(destination.to_owned()));
    } else {
        percent_decode(destination).unwrap_or_else(|_| destination.to_owned())
    };
    validate_destination(&decoded)?;
    Ok(PathBuf::from(decoded))
}

fn has_uri_scheme(value: &str) -> bool {
    value.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && !scheme.contains('/')
            && scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

fn validate_destination(value: &str) -> Result<(), LocalImagePathError> {
    if value.contains('\0')
        || value.starts_with("//")
        || value.starts_with("\\\\")
        || has_uri_scheme(value)
    {
        return Err(LocalImagePathError::NonLocalPath(value.to_owned()));
    }
    Ok(())
}

fn percent_decode(value: &str) -> Result<String, LocalImagePathError> {
    decode_destination(value).map_err(|error| match error {
        DestinationDecodeError::InvalidEncoding => {
            LocalImagePathError::InvalidPercentEncoding(value.to_owned())
        }
        DestinationDecodeError::InvalidUtf8 => LocalImagePathError::InvalidUtf8(value.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_parent_unicode_and_percent_encoded_paths_resolve_from_note() {
        let note = Path::new("/Sticky/note");
        assert_eq!(
            resolve_local_image(note, "images/%E6%95%B0%E5%AD%A6%20%E5%9B%BE.png").unwrap(),
            note.join("images/数学 图.png")
        );
        assert_eq!(
            resolve_local_image(note, "../shared/a.png").unwrap(),
            note.join("../shared/a.png")
        );
    }

    #[test]
    fn file_urls_are_strict_but_a_plain_percent_is_literal() {
        assert_eq!(
            resolve_local_image(Path::new("note"), "file:///Users/me/My%20Notes/a.png").unwrap(),
            PathBuf::from("/Users/me/My Notes/a.png")
        );
        assert_eq!(
            resolve_local_image(Path::new("note"), "100%.png").unwrap(),
            Path::new("note").join("100%.png")
        );
        assert!(resolve_local_image(Path::new("note"), "file:///a/bad%2.png").is_err());
    }

    #[test]
    fn remote_and_scheme_destinations_are_rejected_without_io() {
        for destination in [
            "//server/share/a.png",
            "file://server/share/a.png",
            "FILE://server/share/a.png",
            "file:a.png",
            "http://example.com/a.png",
            "smb://server/a.png",
            "images/a%00.png",
            r"\\server\share\a.png",
        ] {
            assert!(
                resolve_local_image(Path::new("/Sticky/note"), destination).is_err(),
                "must reject {destination:?} before opening a file"
            );
        }
    }

    #[test]
    fn a_colon_later_in_a_path_is_an_ordinary_character() {
        let note = Path::new("/Sticky/note");
        assert_eq!(
            resolve_local_image(note, "images/12:30.png").unwrap(),
            note.join("images/12:30.png")
        );
    }
}
