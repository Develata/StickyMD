//! Windows-backed note-directory watch hint adapter.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-file-watch

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

pub struct NoteDirectoryWatcher {
    _watcher: RecommendedWatcher,
    hint_pending: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum FileWatchSignal {
    NoteHint,
    Failed(String),
}

impl NoteDirectoryWatcher {
    pub fn start<F>(note_dir: &Path, mut on_signal: F) -> Result<Self, FileWatchError>
    where
        F: FnMut(FileWatchSignal) + Send + 'static,
    {
        let hint_pending = Arc::new(AtomicBool::new(false));
        let callback_pending = Arc::clone(&hint_pending);
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| match result {
                Ok(event)
                    if event.paths.iter().any(|path| is_relevant(path))
                        && claim_hint(&callback_pending) =>
                {
                    on_signal(FileWatchSignal::NoteHint);
                }
                Ok(_) => {}
                Err(error) => on_signal(FileWatchSignal::Failed(error.to_string())),
            },
            Config::default(),
        )
        .map_err(FileWatchError::Create)?;
        watcher
            .watch(note_dir, RecursiveMode::NonRecursive)
            .map_err(FileWatchError::Watch)?;
        Ok(Self {
            _watcher: watcher,
            hint_pending,
        })
    }

    /// Let the backend enqueue one more hint after the UI has consumed the
    /// current one. This bounds the user-event queue during filesystem storms.
    pub fn acknowledge_hint(&self) {
        self.hint_pending.store(false, Ordering::Release);
    }
}

fn claim_hint(pending: &AtomicBool) -> bool {
    pending
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

fn is_relevant(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        name.eq_ignore_ascii_case("note.md") || name.eq_ignore_ascii_case("note.md.tmp")
    })
}

#[derive(Debug, Error)]
pub enum FileWatchError {
    #[error("cannot create the filesystem watcher: {0}")]
    Create(notify::Error),
    #[error("cannot watch the note directory: {0}")]
    Watch(notify::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn watch_filter_is_narrow() {
        assert!(is_relevant(Path::new("C:\\x\\note.md")));
        assert!(is_relevant(Path::new("C:\\x\\NOTE.MD.TMP")));
        assert!(!is_relevant(Path::new("C:\\x\\config.toml")));
        assert!(!is_relevant(Path::new("C:\\x\\image.png")));
    }

    #[test]
    fn only_one_hint_can_be_pending_until_acknowledged() {
        let pending = AtomicBool::new(false);
        assert!(claim_hint(&pending));
        for _ in 0..1000 {
            assert!(!claim_hint(&pending));
        }
        pending.store(false, Ordering::Release);
        assert!(claim_hint(&pending));
    }

    #[test]
    fn windows_backend_emits_a_hint_for_external_note_write() {
        let root = unique_temp_path("watch");
        fs::create_dir(&root).unwrap();
        let (sender, receiver) = mpsc::channel();
        let watcher = NoteDirectoryWatcher::start(&root, move |signal| {
            let _ = sender.send(signal);
        })
        .unwrap();
        fs::write(root.join("note.md"), b"external").unwrap();
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
            FileWatchSignal::NoteHint
        ));
        drop(watcher);
        fs::remove_dir_all(root).unwrap();
    }
}
