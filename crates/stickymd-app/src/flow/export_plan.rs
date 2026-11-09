//! The decisions of an export: which copies are the same image, what each is called and how the
//! Markdown refers to it. It holds no file and grants nothing: the host copies, hashes, moves and
//! publishes under its own guarantees, and tells this what happened (plan 03 rule 5).
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use stickymd_core::Hash32;
use stickymd_render::preview::{
    ExportProjectionError, ImageOccurrence, ImageRewrite, rewrite_image_occurrences,
};
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("export asset hash-prefix collision could not be represented safely")]
pub struct ExportNameCollision;

/// What a staged copy turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyPlacement {
    /// The same bytes are already exported under this name; the copy is not needed.
    Duplicate(String),
    /// New content: it takes this name once the host has moved it there.
    New(String),
}

#[derive(Debug)]
pub struct ExportPlan {
    markdown_asset_leaf: String,
    rewrites: Vec<ImageRewrite>,
    copied: HashMap<Hash32, String>,
    occupied_names: HashMap<String, Hash32>,
    /// What each source became. The keys are the host's resolved paths, compared as they are:
    /// which paths name one file is the host's to say.
    resolved_sources: HashMap<PathBuf, String>,
}

impl ExportPlan {
    /// `asset_leaf` is the name of the asset directory the host chose.
    pub fn new(asset_leaf: &str) -> Self {
        Self {
            markdown_asset_leaf: encode_markdown_path_segment(asset_leaf),
            rewrites: Vec::new(),
            copied: HashMap::new(),
            occupied_names: HashMap::new(),
            resolved_sources: HashMap::new(),
        }
    }

    /// The name this source already took, so that it is not copied again.
    pub fn name_for_source(&self, source: &Path) -> Option<&str> {
        self.resolved_sources.get(source).map(String::as_str)
    }

    pub fn place_copy(
        &self,
        hash: Hash32,
        extension: &str,
    ) -> Result<CopyPlacement, ExportNameCollision> {
        if let Some(existing) = self.copied.get(&hash) {
            return Ok(CopyPlacement::Duplicate(existing.clone()));
        }
        choose_name(hash, extension, &self.occupied_names).map(CopyPlacement::New)
    }

    /// The host moved the copy to `name`.
    pub fn confirm_new(&mut self, hash: Hash32, name: String) {
        self.copied.insert(hash, name.clone());
        self.occupied_names.insert(name, hash);
    }

    pub fn remember_source(&mut self, source: PathBuf, name: String) {
        self.resolved_sources.insert(source, name);
    }

    /// This occurrence of an image will refer to `name` in the asset directory.
    pub fn rewrite(&mut self, occurrence: ImageOccurrence, name: &str) {
        self.rewrites.push(ImageRewrite {
            source_range: occurrence.source_range,
            destination: format!("{}/{name}", self.markdown_asset_leaf),
            title: occurrence.title,
            alt: occurrence.alt,
        });
    }

    pub fn copied_assets(&self) -> usize {
        self.copied.len()
    }

    /// The Markdown with every recorded occurrence pointing at its copy.
    pub fn rewritten_markdown(&mut self, source: &str) -> Result<String, ExportProjectionError> {
        rewrite_image_occurrences(source, std::mem::take(&mut self.rewrites))
    }
}

/// The extension an exported copy takes from its source's: a short ASCII one in lower case, or
/// `img` for anything else.
pub fn export_asset_extension(source: &Path) -> String {
    source
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 10
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .map_or_else(|| "img".to_owned(), str::to_ascii_lowercase)
}

fn choose_name(
    hash: Hash32,
    extension: &str,
    occupied: &HashMap<String, Hash32>,
) -> Result<String, ExportNameCollision> {
    let hex = hash.to_hex();
    for length in [20usize, 32, 64] {
        let name = format!("stickymd-{}.{}", &hex[..length], extension);
        match occupied.get(&name) {
            None => return Ok(name),
            Some(existing) if *existing == hash => return Ok(name),
            Some(_) => {}
        }
    }
    Err(ExportNameCollision)
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

#[cfg(test)]
mod tests {
    use stickymd_core::hash_bytes;
    use stickymd_render::preview::{ImageKind, SourceRange};

    use super::*;

    fn occurrence(start: usize, end: usize) -> ImageOccurrence {
        ImageOccurrence {
            destination: "a.png".into(),
            title: String::new(),
            alt: String::new(),
            kind: ImageKind::LocalRelative,
            source_range: SourceRange { start, end },
        }
    }

    #[test]
    fn the_extension_is_short_ascii_in_lower_case_or_img() {
        for (path, expected) in [
            ("a.PNG", "png"),
            ("a.jpeg", "jpeg"),
            ("a", "img"),
            ("a.中文", "img"),
            ("a.abcdefghijk", "img"),
            ("a.b-c", "img"),
        ] {
            assert_eq!(export_asset_extension(Path::new(path)), expected, "{path}");
        }
    }

    #[test]
    fn the_same_source_is_copied_once() {
        let mut plan = ExportPlan::new("x-assets");
        assert_eq!(plan.name_for_source(Path::new("a.png")), None);
        plan.remember_source("a.png".into(), "n.png".into());
        assert_eq!(plan.name_for_source(Path::new("a.png")), Some("n.png"));
    }

    /// Sources are compared as the host resolved them: folding case here would merge two files on
    /// a file system that tells them apart.
    #[test]
    fn sources_differing_only_in_case_stay_different() {
        let mut plan = ExportPlan::new("x-assets");
        plan.remember_source("A.png".into(), "n.png".into());
        assert_eq!(plan.name_for_source(Path::new("a.png")), None);
    }

    /// The first copy's name wins, including its extension.
    #[test]
    fn the_same_bytes_under_another_extension_reuse_the_first_name() {
        let hash = hash_bytes(b"image");
        let mut plan = ExportPlan::new("x-assets");
        let CopyPlacement::New(name) = plan.place_copy(hash, "png").unwrap() else {
            panic!("the first copy is new");
        };
        assert_eq!(
            name,
            format!("stickymd-{}.png", &hash.to_hex()[..20]),
            "the shortest prefix"
        );
        plan.confirm_new(hash, name.clone());

        assert_eq!(
            plan.place_copy(hash, "jpg").unwrap(),
            CopyPlacement::Duplicate(name)
        );
        assert_eq!(plan.copied_assets(), 1);
    }

    #[test]
    fn nothing_counts_as_copied_until_the_host_has_moved_it() {
        let hash = hash_bytes(b"image");
        let plan = ExportPlan::new("x-assets");
        let first = plan.place_copy(hash, "png").unwrap();
        assert_eq!(plan.place_copy(hash, "png").unwrap(), first);
        assert_eq!(plan.copied_assets(), 0);
    }

    #[test]
    fn a_taken_prefix_grows_and_running_out_is_an_error() {
        let wanted = hash_bytes(b"wanted");
        let other = hash_bytes(b"other");
        let hex = wanted.to_hex();
        let mut occupied = HashMap::new();
        for (length, expected) in [(20usize, 32usize), (32, 64)] {
            occupied.insert(format!("stickymd-{}.png", &hex[..length]), other);
            assert_eq!(
                choose_name(wanted, "png", &occupied).unwrap(),
                format!("stickymd-{}.png", &hex[..expected])
            );
        }
        occupied.insert(format!("stickymd-{hex}.png"), other);
        assert_eq!(
            choose_name(wanted, "png", &occupied),
            Err(ExportNameCollision)
        );
    }

    #[test]
    fn references_point_into_the_encoded_asset_directory() {
        let mut plan = ExportPlan::new("my notes-assets");
        plan.rewrite(occurrence(0, 12), "n.png");
        let markdown = plan.rewritten_markdown("![](a.png) tail").unwrap();
        assert!(markdown.contains("my%20notes-assets/n.png"), "{markdown}");
    }
}
