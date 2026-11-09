//! The note-file rows of the shell's receipt table on the Windows storage adapters: the same rows
//! `stickymd-desktop` runs on its portable test disk, here behind `win_io_ports` (W1).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::path::Path;

use stickymd_app::execution::PersistenceWorker;
use stickymd_desktop::shell::storage_conformance::{Completion, StoragePaths, StorageSite};

use super::win_io_ports;
use crate::platform::windows::program_dir::RuntimePaths;

/// A portable program directory in `root`, as the Windows host lays it out.
fn windows_storage(root: &Path, on_completion: Completion) -> StorageSite {
    let paths = RuntimePaths::from_executable(&root.join("StickyMD.exe")).unwrap();
    std::fs::create_dir(&paths.note_dir).unwrap();
    let worker = PersistenceWorker::start(win_io_ports(&paths), on_completion).unwrap();
    StorageSite {
        worker,
        paths: StoragePaths {
            note_file: paths.note_file,
            note_tmp: paths.note_tmp,
            note_dir: paths.note_dir,
        },
    }
}

stickymd_desktop::testing::storage_conformance!(windows_storage);
