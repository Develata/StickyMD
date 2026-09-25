//! License-file selection and reviewed fallbacks, with strict text decoding.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::graph::{Package, ordinal};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn select(root: &Path, package: &Package) -> Result<Vec<PathBuf>, String> {
    let directory = package
        .manifest
        .parent()
        .ok_or("Cargo manifest has no parent")?;
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            // Match Get-ChildItem -File without -Force in the original Windows adapter.
            if entry
                .metadata()
                .map_err(|error| error.to_string())?
                .file_attributes()
                & 0x2
                != 0
            {
                continue;
            }
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or("license filename is not Unicode")?;
        if entry.path().is_file() && license_name(name) {
            files.push(entry.path());
        }
    }
    files.sort_by(|left, right| {
        ordinal(
            left.file_name().unwrap().to_str().unwrap(),
            right.file_name().unwrap().to_str().unwrap(),
        )
    });
    if files.is_empty() {
        let fallback = match package.name.as_str() {
            "clipboard-win" => "Boost-1.0.txt",
            "harfrust" => "HarfRust-MIT.txt",
            "ratex-font" | "ratex-font-loader" | "ratex-katex-fonts" | "ratex-layout"
            | "ratex-lexer" | "ratex-parser" | "ratex-types" | "ratex-unicode-font" => {
                "RaTeX-MIT.txt"
            }
            _ => {
                return Err(format!(
                    "Runtime package {} {} contains no license notice and has no reviewed fallback",
                    package.name, package.version
                ));
            }
        };
        let fallback = root.join("assets/licenses").join(fallback);
        if !fallback.is_file() {
            return Err(format!(
                "Reviewed license fallback is missing for {}: {}",
                package.name,
                fallback.display()
            ));
        }
        files.push(fallback);
    }
    Ok(files)
}

fn license_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    ["LICENSE", "COPYING", "NOTICE"].iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['.', '_', '-']))
    })
}

pub(super) fn text(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    decode(&bytes).map_err(|error| format!("invalid license text {}: {error}", path.display()))
}

fn decode(bytes: &[u8]) -> Result<String, String> {
    // File.ReadAllText's BOM detection is part of the existing generated-byte semantics.
    if bytes.starts_with(&[0xff, 0xfe, 0, 0]) || bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if !(bytes.len() - 4).is_multiple_of(4) {
            return Err("truncated UTF-32".to_owned());
        }
        return bytes[4..]
            .chunks_exact(4)
            .map(|chunk| {
                let word: [u8; 4] = chunk.try_into().unwrap();
                char::from_u32(if little {
                    u32::from_le_bytes(word)
                } else {
                    u32::from_be_bytes(word)
                })
                .ok_or_else(|| "invalid UTF-32".to_owned())
            })
            .collect();
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if !(bytes.len() - 2).is_multiple_of(2) {
            return Err("truncated UTF-16".to_owned());
        }
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| {
                if little {
                    u16::from_le_bytes([chunk[0], chunk[1]])
                } else {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                }
            })
            .collect::<Vec<_>>();
        return String::from_utf16(&units).map_err(|error| error.to_string());
    }
    String::from_utf8(
        bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(bytes)
            .to_vec(),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_license_notice_names_are_selected_and_decoding_is_strict() {
        for name in ["LICENSE", "license-MIT", "COPYING.txt", "NOTICE_中文"] {
            assert!(license_name(name));
        }
        for name in ["LICENSES", "LICENSE2", "README", "src/LICENSE"] {
            assert!(!license_name(name));
        }
        assert_eq!(decode(b"\xef\xbb\xbfnotice").unwrap(), "notice");
        assert_eq!(decode(&[0xff, 0xfe, 0x2d, 0x4e]).unwrap(), "中");
        assert!(decode(&[0xff]).is_err());
        assert!(decode(&[0xff, 0xfe, 0]).is_err());
    }
}
