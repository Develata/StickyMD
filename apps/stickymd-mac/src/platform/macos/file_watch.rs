//! FSEvents-backed note-directory watch hints.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-file-watch

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use stickymd_app::flow::names_the_working_note;
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
        let watched: PathBuf = note_dir.to_owned();
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| match result {
                Ok(event) if names_the_watched_directory(&event, &watched) && !watched.exists() => {
                    // FSEvents goes quiet once its directory moves: nothing else would tell.
                    on_signal(FileWatchSignal::Failed(
                        "the note directory was moved or removed".to_owned(),
                    ));
                }
                Ok(event)
                    if event.paths.iter().any(|path| names_the_working_note(path))
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

    /// Lets the backend enqueue one more hint after the UI consumed the last one, which bounds
    /// the event queue during a storm of file events.
    pub fn acknowledge_hint(&self) {
        self.hint_pending.store(false, Ordering::Release);
    }
}

fn names_the_watched_directory(event: &Event, watched: &Path) -> bool {
    event.paths.iter().any(|path| path == watched)
}

fn claim_hint(pending: &AtomicBool) -> bool {
    pending
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
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

    fn canonical_directory(label: &str) -> PathBuf {
        let root = unique_temp_path(label);
        fs::create_dir(&root).unwrap();
        // FSEvents reports real paths; the temporary directory is under a symlink.
        fs::canonicalize(root).unwrap()
    }

    #[test]
    fn only_one_hint_can_be_pending_until_acknowledged() {
        let pending = AtomicBool::new(false);
        assert!(claim_hint(&pending));
        assert!(!claim_hint(&pending));
        pending.store(false, Ordering::Release);
        assert!(claim_hint(&pending));
    }

    #[test]
    fn an_external_write_gives_a_hint() {
        let root = canonical_directory("watch-write");
        let (sender, receiver) = mpsc::channel();
        let watcher = NoteDirectoryWatcher::start(&root, move |signal| {
            let _ = sender.send(signal);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        fs::write(root.join("note.md"), b"external").unwrap();
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            FileWatchSignal::NoteHint
        ));
        drop(watcher);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn moving_the_directory_away_is_a_watch_failure() {
        let root = canonical_directory("watch-moved");
        let (sender, receiver) = mpsc::channel();
        let watcher = NoteDirectoryWatcher::start(&root, move |signal| {
            let _ = sender.send(signal);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let moved = root.with_extension("moved");
        fs::rename(&root, &moved).unwrap();
        let signal = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(signal, FileWatchSignal::Failed(_)), "{signal:?}");
        drop(watcher);
        fs::remove_dir_all(moved).unwrap();
    }
}
