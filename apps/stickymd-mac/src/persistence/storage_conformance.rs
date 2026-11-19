//! The note-file rows of the shell's receipt table, on the macOS storage behind `mac_io_ports`.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::Path;

use stickymd_app::execution::PersistenceWorker;
use stickymd_desktop::shell::storage_conformance::{Completion, StoragePaths, StorageSite};

use super::mac_io_ports;
use crate::paths::{RuntimePaths, open_note_directory};
use crate::platform::macos::file::DirectoryHandle;

fn macos_storage(root: &Path, on_completion: Completion) -> StorageSite {
    let note = open_note_directory(&DirectoryHandle::open(root).unwrap()).unwrap();
    let paths = RuntimePaths::of(root);
    let worker =
        PersistenceWorker::start(mac_io_ports(&note, &paths).unwrap().io, on_completion).unwrap();
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
