//! The pure half of the Note Root rule: where a bundle sits decides where the note lives, its
//! name decides which note it is. Nothing here touches the file system; `note_root` feeds it what
//! it read.
//!
//! plan_ref: docs/plan/05_document_persistence.md#note-root

use std::ffi::OsStr;
use std::path::Path;

use unicode_normalization::UnicodeNormalization;

/// A directory's file-system identity, `(st_dev, st_ino)`; two paths name the same directory
/// exactly when their identities are equal, whatever links or firmlinks lie between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub device: u64,
    pub inode: u64,
}

/// What looking at one candidate `Applications` directory found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Candidate {
    /// It exists; its identity after following links.
    Found(Identity),
    /// `ENOENT` or `ENOTDIR`: nothing there, a dangling link, or not a directory. The bundle's
    /// parent exists, so it cannot be that.
    Absent,
    /// Any other failure, with what to tell the user.
    Unreadable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The bundle sits directly in a candidate `Applications` directory.
    Installed,
    /// Anywhere else: the note lives next to the bundle.
    Portable,
    /// No candidate matched but one could not be read, or the home directory is unknown: the
    /// program says why and stops rather than guess a location.
    Undecidable(String),
}

/// `<X>` and the bundle's directory name when `executable` (already canonical) has the shape
/// `<X>/<name>.app/Contents/MacOS/<exe>`; `None` for a development build.
pub fn bundle_of(executable: &Path) -> Option<(&Path, &OsStr)> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let in_bundle = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        // By the raw name, so a bundle named just `.app` (no extension to Rust) still reaches the
        // key check, which refuses its empty key.
        && bundle.file_name()?.as_encoded_bytes().ends_with(b".app");
    in_bundle.then(|| Some((bundle.parent()?, bundle.file_name()?)))?
}

/// Installed when the bundle's parent is one of the candidates: a hit wins over another
/// candidate's error. Otherwise any error, the unknown home directory included, leaves the
/// placement undecidable; with none it is portable.
///
/// `candidates` is the system `Applications` and, when the home directory is known, the one in
/// it; `home_unknown` says why the home directory is not.
pub fn placement(
    parent: Identity,
    candidates: &[Candidate],
    home_unknown: Option<&str>,
) -> Placement {
    if candidates.contains(&Candidate::Found(parent)) {
        return Placement::Installed;
    }
    if let Some(reason) = home_unknown {
        return Placement::Undecidable(reason.to_owned());
    }
    candidates
        .iter()
        .find_map(|candidate| match candidate {
            Candidate::Unreadable(reason) => Some(Placement::Undecidable(reason.clone())),
            Candidate::Found(_) | Candidate::Absent => None,
        })
        .unwrap_or(Placement::Portable)
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("the app's name is not valid UTF-8")]
    NotUtf8,
    #[error("the app's name `{0}` names no note")]
    Unusable(String),
}

/// The installed note's key: the bundle's actual directory name without `.app`, in Unicode NFC.
/// Whether two keys share a note is then up to the volume `Application Support` is on.
pub fn note_key(bundle_name: &OsStr) -> Result<String, KeyError> {
    let name = bundle_name.to_str().ok_or(KeyError::NotUtf8)?;
    let stem = name.strip_suffix(".app").unwrap_or(name);
    let key: String = stem.nfc().collect();
    if key.is_empty() || key == "." || key == ".." || key.contains(['/', '\0']) {
        return Err(KeyError::Unusable(key));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    const PARENT: Identity = Identity {
        device: 1,
        inode: 10,
    };
    const ELSEWHERE: Identity = Identity {
        device: 1,
        inode: 11,
    };

    #[test]
    fn a_bundle_yields_its_parent_and_name_and_a_bare_executable_none() {
        assert_eq!(
            bundle_of(Path::new("/x/y/StickyMD.app/Contents/MacOS/StickyMD")),
            Some((Path::new("/x/y"), OsStr::new("StickyMD.app")))
        );
        assert_eq!(bundle_of(Path::new("/x/target/debug/stickymd")), None);
        // Only the whole shape is a bundle.
        assert_eq!(bundle_of(Path::new("/x/Contents/MacOS/stickymd")), None);
        assert_eq!(
            bundle_of(Path::new("/x/StickyMD.APP/Contents/MacOS/StickyMD")),
            None
        );
        assert_eq!(
            bundle_of(Path::new("/x/.app/Contents/MacOS/StickyMD")),
            Some((Path::new("/x"), OsStr::new(".app")))
        );
    }

    #[test]
    fn a_hit_on_either_candidate_is_installed_and_wins_over_an_error() {
        for candidates in [
            vec![Candidate::Found(PARENT), Candidate::Absent],
            vec![Candidate::Absent, Candidate::Found(PARENT)],
            vec![
                Candidate::Unreadable("denied".into()),
                Candidate::Found(PARENT),
            ],
            vec![
                Candidate::Found(PARENT),
                Candidate::Unreadable("denied".into()),
            ],
        ] {
            assert_eq!(
                placement(PARENT, &candidates, None),
                Placement::Installed,
                "{candidates:?}"
            );
        }
        // The system candidate alone may hit while the home directory is unknown.
        assert_eq!(
            placement(PARENT, &[Candidate::Found(PARENT)], Some("no home")),
            Placement::Installed
        );
    }

    #[test]
    fn missing_dangling_or_other_directories_are_portable() {
        // A subfolder of /Applications, /System/Applications, a link elsewhere: all differ.
        for candidates in [
            vec![Candidate::Absent, Candidate::Absent],
            vec![Candidate::Found(ELSEWHERE), Candidate::Absent],
            vec![Candidate::Found(ELSEWHERE), Candidate::Found(ELSEWHERE)],
        ] {
            assert_eq!(
                placement(PARENT, &candidates, None),
                Placement::Portable,
                "{candidates:?}"
            );
        }
    }

    #[test]
    fn without_a_hit_an_error_or_an_unknown_home_is_undecidable() {
        assert_eq!(
            placement(
                PARENT,
                &[
                    Candidate::Found(ELSEWHERE),
                    Candidate::Unreadable("denied".into())
                ],
                None
            ),
            Placement::Undecidable("denied".into())
        );
        assert_eq!(
            placement(PARENT, &[Candidate::Absent], Some("no home")),
            Placement::Undecidable("no home".into())
        );
    }

    #[test]
    fn the_key_is_the_name_without_app_in_nfc() {
        assert_eq!(note_key(OsStr::new("StickyMD.app")).unwrap(), "StickyMD");
        assert_eq!(
            note_key(OsStr::new("StickyMD copy.app")).unwrap(),
            "StickyMD copy"
        );
        // "é" decomposed (e + U+0301) and precomposed are one key.
        let decomposed = note_key(OsStr::new("Caf\u{65}\u{301}.app")).unwrap();
        assert_eq!(decomposed, "Caf\u{e9}");
        assert_eq!(decomposed, note_key(OsStr::new("Caf\u{e9}.app")).unwrap());
        // Case is kept: whether it matters is the volume's decision.
        assert_eq!(note_key(OsStr::new("stickymd.app")).unwrap(), "stickymd");
    }

    #[test]
    fn a_name_that_names_no_note_is_refused() {
        for name in [".app", "..app", "...app"] {
            assert!(
                matches!(note_key(OsStr::new(name)), Err(KeyError::Unusable(_))),
                "{name}"
            );
        }
        assert_eq!(
            note_key(OsStr::from_bytes(b"St\xffcky.app")),
            Err(KeyError::NotUtf8)
        );
    }
}
