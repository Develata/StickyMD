//! Capability Ports: what the application asks a Platform Host to execute and the receipts it
//! gets back. A host implements a port with its native adapter and reports facts; decisions stay
//! in `flow`. A port covers one whole safe operation, so no caller holds a path to act on.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

mod assets;
mod config;
mod export;
mod failure;
mod monitor;
mod note;
mod preview;
mod shell;
mod window;

pub use assets::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetPasteRollback,
    AssetReconcileMode, AssetReconcileReport, AssetSyncRequest, ManagedAssetStore, StoredAsset,
    StoredAssetDisposition,
};
pub use config::ConfigStorage;
pub use export::{ExportCompletion, ExportRequest, ExportSink};
pub use failure::{FailureKind, HostFailure, StorageFailure};
pub use monitor::{MonitorSnapshot, MonitorSource};
pub use note::{
    NoteReceipt, NoteStorage, PersistMode, PersistRequest, PersistResult, PreserveOutcome,
    TemporaryCleanup,
};
pub use preview::{PreviewCompletion, PreviewImages, PreviewJob, PreviewViewport};
pub use shell::{ExportTargetPicker, FatalNotice, ShellLauncher, TrayCommand, TrayView};
pub use window::{FrameFact, TopmostFact, Visibility, WindowHost};
