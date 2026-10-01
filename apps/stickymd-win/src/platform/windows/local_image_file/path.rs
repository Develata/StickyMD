//! Pure path admission using observed DOS-device mappings.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};

use super::denied;

pub(super) fn native_path(
    path: &Path,
    mut query: impl FnMut(u8) -> io::Result<OsString>,
) -> io::Result<OsString> {
    if path.has_root() && !path.is_absolute() {
        return Err(denied("local image path must specify its drive"));
    }
    if matches!(path.components().next(), Some(Component::Prefix(_))) && !path.is_absolute() {
        return Err(denied("drive-relative image paths are ambiguous"));
    }
    // absolute performs lexical expansion only; canonicalize would follow links.
    let mut absolute = std::path::absolute(path)?;
    for _ in 0..8 {
        let (drive, tail) = disk_path(&absolute)?;
        let target = query(drive)?;
        let target = target
            .to_str()
            .ok_or_else(|| denied("invalid DOS device mapping"))?;
        if local_device(target) {
            let mut native = OsString::from(target);
            native.push("\\");
            native.push(tail);
            return Ok(native);
        }
        // SUBST and other DOS aliases are expanded as names, never by opening
        // their targets. Cycles and unknown devices fail closed with a bound.
        let alias = target
            .strip_prefix(r"\??\")
            .ok_or_else(|| denied("network or unknown image drive is not allowed"))?;
        let alias = Path::new(alias);
        disk_path(alias)?;
        absolute = alias.join(tail);
    }
    Err(denied(
        "local image drive alias chain is cyclic or too deep",
    ))
}

fn disk_path(path: &Path) -> io::Result<(u8, PathBuf)> {
    let mut components = path.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(denied("network and device image paths are not allowed")),
        },
        _ => return Err(denied("image drive alias is not an absolute disk path")),
    };
    if components.next() != Some(Component::RootDir) {
        return Err(denied("image drive alias has no root"));
    }
    let mut tail = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(name) => {
                use std::os::windows::ffi::OsStrExt;
                if name.encode_wide().any(|c| c == 0 || c == u16::from(b':')) {
                    return Err(denied("invalid image filename or alternate data stream"));
                }
                tail.push(name);
            }
            Component::ParentDir => {
                tail.pop();
            }
            Component::CurDir => {}
            _ => return Err(denied("invalid local image path component")),
        }
    }
    Ok((drive.to_ascii_uppercase(), tail))
}

fn local_device(target: &str) -> bool {
    [
        r"\Device\HarddiskVolume",
        r"\Device\CdRom",
        r"\Device\Floppy",
    ]
    .iter()
    .any(|prefix| {
        target
            .get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
            && target.get(prefix.len()..).is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|c| c.is_ascii_digit())
            })
    })
}
