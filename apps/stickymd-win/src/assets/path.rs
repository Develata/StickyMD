//! Local Markdown image destination resolution shared by Preview and Export.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LocalImagePathError {
    #[error("local image path has invalid percent encoding: {0}")]
    InvalidPercentEncoding(String),
    #[error("local image path is not valid UTF-8 after percent decoding: {0}")]
    InvalidUtf8(String),
}

/// Resolve a Comrak image destination for read-only local access. Relative
/// paths remain relative to `note/`; `file:///C:/...` and file UNC URLs are
/// converted to native Windows paths. The result is never used as a write or
/// managed-asset ownership boundary.
pub fn resolve_local_image(
    note_dir: &Path,
    destination: &str,
) -> Result<PathBuf, LocalImagePathError> {
    let destination = destination.trim();
    let decoded = if destination
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:///"))
    {
        percent_decode(&destination[8..])?
    } else if destination
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
    {
        format!(
            "\\\\{}",
            percent_decode(&destination[7..])?.replace('/', "\\")
        )
    } else {
        percent_decode(destination).unwrap_or_else(|_| destination.to_owned())
    };
    let path = PathBuf::from(decoded);
    Ok(if path.is_absolute() {
        path
    } else {
        note_dir.join(path)
    })
}

fn percent_decode(value: &str) -> Result<String, LocalImagePathError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| LocalImagePathError::InvalidPercentEncoding(value.to_owned()))?;
            let high = hex(pair[0])
                .ok_or_else(|| LocalImagePathError::InvalidPercentEncoding(value.to_owned()))?;
            let low = hex(pair[1])
                .ok_or_else(|| LocalImagePathError::InvalidPercentEncoding(value.to_owned()))?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| LocalImagePathError::InvalidUtf8(value.to_owned()))
}

const fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_parent_unicode_and_percent_encoded_paths_resolve_from_note() {
        let note = Path::new("C:/Sticky/note");
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
    fn file_urls_are_strict_but_plain_windows_percent_is_literal() {
        assert_eq!(
            resolve_local_image(Path::new("note"), "file:///C:/My%20Notes/a.png").unwrap(),
            PathBuf::from("C:/My Notes/a.png")
        );
        assert_eq!(
            resolve_local_image(Path::new("note"), "file://server/share/a.png").unwrap(),
            PathBuf::from(r"\\server\share\a.png")
        );
        assert_eq!(
            resolve_local_image(Path::new("note"), "100%.png").unwrap(),
            Path::new("note").join("100%.png")
        );
        assert!(resolve_local_image(Path::new("note"), "file:///C:/bad%2.png").is_err());
    }
}
