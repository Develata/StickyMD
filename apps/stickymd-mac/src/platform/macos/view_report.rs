//! Opt-in view diagnostic for the VM driver (the verification tool's `mac-resources`; review F7,
//! USER 2026-11-24). It is off unless `STICKYMD_DIAGNOSTIC_VIEW` names an output file; off, it
//! never touches the file system. On, it writes once, on the first frame whose every pane shows a
//! real frame: which view that frame's panes show (`PresentFacts::view`, read from the panes laid
//! out, not from the configuration) and the frame's size in pixels. Nothing about the note is
//! recorded. After that one write, each frame costs one check of an `Option`.
//!
//! The file is published atomically and never replaces an existing one: the text is written and
//! synced into a new, uniquely named file beside it, which is then hard-linked to the output name
//! (refused if that name exists) and removed.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use stickymd_app::config::ViewMode;

/// The environment variable naming the output file.
pub const ENV: &str = "STICKYMD_DIAGNOSTIC_VIEW";

/// The report still to write: `None` when the diagnostic is off or the report was written.
#[derive(Debug, Default)]
pub struct ViewReport {
    path: Option<PathBuf>,
}

impl ViewReport {
    /// On when `value` (the environment variable's) names a file; unset or empty, off.
    pub fn from_value(value: Option<OsString>) -> Self {
        Self {
            path: value.filter(|path| !path.is_empty()).map(PathBuf::from),
        }
    }

    pub fn from_environment() -> Self {
        Self::from_value(std::env::var_os(ENV))
    }

    /// A frame reached the window. The first complete one is reported; nothing else is.
    pub fn frame_presented(&mut self, view: ViewMode, width: u32, height: u32, complete: bool) {
        if !complete {
            return;
        }
        let Some(path) = self.path.take() else {
            return;
        };
        if let Err(error) = publish_new(&path, encode(view, width, height).as_bytes()) {
            eprintln!("view diagnostic not written: {error}");
        }
    }
}

fn name(view: ViewMode) -> &'static str {
    match view {
        ViewMode::Source => "source",
        ViewMode::Split => "split",
        ViewMode::Preview => "preview",
    }
}

/// The report: a version line, the view, and the frame's size in pixels.
pub fn encode(view: ViewMode, width: u32, height: u32) -> String {
    format!(
        "stickymd-view 1\nview {}\nsize {width} {height}\n",
        name(view)
    )
}

/// Writes `bytes` to `path` without ever replacing a file there: a fully written and synced new
/// file is linked to `path` (which fails if `path` exists), and the temporary name is removed in
/// every case.
fn publish_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("output path has no file name"))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut temporary_name = name.to_os_string();
    temporary_name.push(format!(".{}.{nanos}.tmp", std::process::id()));
    let temporary = path.with_file_name(temporary_name);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::hard_link(&temporary, path));
    let _ = std::fs::remove_file(&temporary);
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty directory of this test's own.
    fn directory(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stickymd-view-report-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    fn entries(directory: &Path) -> Vec<OsString> {
        let mut names: Vec<OsString> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn unset_or_empty_the_diagnostic_writes_nothing() {
        let directory = directory("off");
        for value in [None, Some(OsString::new())] {
            let mut report = ViewReport::from_value(value);
            // Off: no path, so no frame ever reaches the file system (an empty value names none).
            assert!(report.path.is_none());
            for complete in [false, true, true] {
                report.frame_presented(ViewMode::Preview, 1040, 1360, complete);
            }
        }
        assert!(entries(&directory).is_empty());
        std::fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn the_first_complete_frame_is_reported_once() {
        let directory = directory("on");
        let path = directory.join("view");
        let mut report = ViewReport::from_value(Some(path.clone().into_os_string()));
        // A frame whose preview pane is still a placeholder is not the view yet.
        report.frame_presented(ViewMode::Split, 1800, 1360, false);
        assert!(!path.exists());
        report.frame_presented(ViewMode::Split, 1800, 1360, true);
        report.frame_presented(ViewMode::Source, 1040, 1360, true);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "stickymd-view 1\nview split\nsize 1800 1360\n"
        );
        assert_eq!(entries(&directory), [OsString::from("view")]);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn an_existing_file_is_never_replaced() {
        let directory = directory("exists");
        let path = directory.join("view");
        std::fs::write(&path, "stale").unwrap();
        let mut report = ViewReport::from_value(Some(path.clone().into_os_string()));
        report.frame_presented(ViewMode::Preview, 1040, 1360, true);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "stale");
        // No temporary file is left beside it.
        assert_eq!(entries(&directory), [OsString::from("view")]);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn each_view_has_its_name() {
        for (view, name) in [
            (ViewMode::Source, "source"),
            (ViewMode::Split, "split"),
            (ViewMode::Preview, "preview"),
        ] {
            assert_eq!(
                encode(view, 1, 2),
                format!("stickymd-view 1\nview {name}\nsize 1 2\n")
            );
        }
    }
}
