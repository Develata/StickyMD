//! The note-file rows of the shell's receipt table, on the macOS storage behind `mac_io_ports`.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::Path;

use stickymd_app::execution::PersistenceWorker;
use stickymd_desktop::shell::storage_conformance::{Completion, StoragePaths, StorageSite};

use super::mac_io_ports;
use crate::paths::RuntimePaths;

fn macos_storage(root: &Path, on_completion: Completion) -> StorageSite {
    let executable = root.join("stickymd");
    std::fs::write(&executable, b"").unwrap();
    let paths = RuntimePaths::from_executable(&executable).unwrap();
    paths.ensure_layout().unwrap();
    let worker = PersistenceWorker::start(mac_io_ports(&paths).unwrap().io, on_completion).unwrap();
    StorageSite {
        worker,
        paths: StoragePaths {
            note_file: paths.note_file,
            note_tmp: paths.note_tmp,
            note_dir: paths.note_dir,
        },
    }
}

stickymd_desktop::testing::storage_conformance!(macos_storage);
