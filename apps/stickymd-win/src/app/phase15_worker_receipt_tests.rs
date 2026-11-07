//! One row per asset, export, config and worker-stop receipt arm, through the real completion
//! handler. Phase 15 B moves these decisions out of the shell; this table runs after every batch.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use stickymd_app::flow::PendingAssetPaste;
use stickymd_app::flow::window::state::{LifecycleState, QuitStage, WindowIntent};
use stickymd_core::{Generation, ManagedAssetName, Selection};

use crate::app::headless_tests::{HeadlessApp, fixture};
use crate::app::receipt_tests::{saves, type_letter};
use crate::config::{ConfigRevision, ViewMode};
use stickymd_app::execution::IoCompletion;
use stickymd_app::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRollback, AssetReconcileReport,
    ExportCompletion, FailureKind, StorageFailure, StoredAsset, StoredAssetDisposition,
};

const PASTE_FAILED: &str = "clipboard payload is not an image transaction";
const SYNC_FAILED: &str = "asset payload exceeds the 64 MiB bound";
const CONFIG_FAILED: &str =
    "cannot atomically publish config.toml: target and temporary files must share a directory";
const ASSET: &str = "stickymd-0123456789abcdef0123.png";

fn failure(message: &str) -> StorageFailure {
    StorageFailure::new(FailureKind::Failed, message)
}

fn generation(app: &HeadlessApp) -> Generation {
    app.coordinator.view().generation
}

fn diagnostic(app: &HeadlessApp) -> &str {
    app.diagnostic.as_deref().unwrap_or_default()
}

fn stored_asset() -> AssetPasteRollback {
    AssetPasteRollback {
        stored: vec![StoredAsset {
            name: ManagedAssetName::parse(ASSET).unwrap(),
            disposition: StoredAssetDisposition::Created,
        }],
    }
}

fn paste_failed(app: &HeadlessApp, rollback: AssetPasteRollback) -> IoCompletion {
    IoCompletion::AssetPaste(Err(AssetPasteFailure {
        expected_generation: generation(app),
        error: failure(PASTE_FAILED),
        rollback,
    }))
}

fn pasted(expected_generation: Generation, selection: Selection) -> IoCompletion {
    IoCompletion::AssetPaste(Ok(AssetPasteCompletion {
        expected_generation,
        selection,
        timestamp_ms: 1,
        markdown: "![](images/a.png)".into(),
        rollback: stored_asset(),
    }))
}

fn sync_done(request_id: u64, report: AssetReconcileReport) -> IoCompletion {
    IoCompletion::AssetSync {
        request_id,
        generation: Generation::initial(),
        result: Ok(report),
    }
}

fn sync_failed(request_id: u64) -> IoCompletion {
    IoCompletion::AssetSync {
        request_id,
        generation: Generation::initial(),
        result: Err(failure(SYNC_FAILED)),
    }
}

fn config_failed(revision: ConfigRevision) -> IoCompletion {
    IoCompletion::Config {
        revision,
        result: Err(failure(CONFIG_FAILED)),
    }
}

fn export_done(result: Result<ExportCompletion, StorageFailure>) -> IoCompletion {
    IoCompletion::Export {
        generation: Generation::initial(),
        result,
    }
}

/// A shell with a sync in flight.
fn syncing() -> (HeadlessApp, u64) {
    let mut app = fixture("start");
    let current = generation(&app);
    app.submit_asset_sync(current, Vec::new(), None, false);
    let request_id = app.assets.sync_request_id.unwrap();
    (app, request_id)
}

fn pending_paste(app: &HeadlessApp) -> PendingAssetPaste {
    PendingAssetPaste {
        expected_generation: generation(app),
        selection: Selection::caret(0),
        timestamp_ms: 1,
        payload: stickymd_app::flow::ClipboardPaste::EncodedImage(Vec::new()),
    }
}

fn lifecycle(app: &HeadlessApp) -> LifecycleState {
    app.window_flow.as_ref().unwrap().state().lifecycle()
}

fn request_quit(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(None, WindowIntent::TrayQuitRequested { guards });
}

#[test]
fn a_paste_starts_once_and_a_second_is_refused() {
    let mut app = fixture("start");
    let request = pending_paste(&app);
    app.submit_asset_paste(request);
    assert!(app.assets.paste_pending);
    assert_eq!(diagnostic(&app), "正在准备图片…");
    let request = pending_paste(&app);
    app.submit_asset_paste(request);
    assert_eq!(diagnostic(&app), "上一批图片仍在处理中；本次粘贴未开始。");
}

#[test]
fn a_failed_paste_converges_what_it_published() {
    let mut app = fixture("start");
    app.assets.paste_pending = true;
    let receipt = paste_failed(&app, stored_asset());
    app.handle_io_completion(None, receipt);
    assert!(!app.assets.paste_pending);
    assert!(app.assets.sync_in_flight);
    assert_eq!(
        diagnostic(&app),
        format!("图片粘贴失败；未插入引用，资产正按最新引用状态收敛：{PASTE_FAILED}")
    );
}

#[test]
fn a_failed_paste_with_nothing_published_starts_no_sync() {
    let mut app = fixture("start");
    let receipt = paste_failed(&app, AssetPasteRollback::default());
    app.handle_io_completion(None, receipt);
    assert!(!app.assets.sync_in_flight);
}

#[test]
fn a_paste_for_an_older_generation_inserts_nothing_and_converges() {
    let mut app = fixture("start");
    let older = generation(&app);
    type_letter(&mut app);
    let text = app.coordinator.view().text.to_string();
    app.handle_io_completion(None, pasted(older, Selection::caret(0)));
    assert_eq!(app.coordinator.view().text, text);
    assert!(app.assets.sync_in_flight);
    assert_eq!(
        diagnostic(&app),
        "图片准备期间文档已变化；未插入引用，资产正按最新引用状态收敛。"
    );
}

#[test]
fn a_prepared_paste_is_committed_to_the_document() {
    let mut app = fixture("start");
    let current = generation(&app);
    app.handle_io_completion(None, pasted(current, Selection::caret(0)));
    assert_eq!(app.coordinator.view().text, "![](images/a.png)start");
    assert!(app.coordinator.view().dirty);
    assert!(app.persistence.autosave_deadline().is_some());
}

#[test]
fn a_paste_that_cannot_commit_converges() {
    let mut app = fixture("start");
    let current = generation(&app);
    app.handle_io_completion(None, pasted(current, Selection::caret(999)));
    assert_eq!(app.coordinator.view().text, "start");
    assert!(app.assets.sync_in_flight);
    assert!(
        diagnostic(&app).starts_with("图片已准备但文档事务未提交；资产正按最新引用状态收敛："),
        "{}",
        diagnostic(&app)
    );
}

#[test]
fn a_settled_paste_moves_the_quit_on() {
    let mut app = fixture("start");
    app.assets.paste_pending = true;
    request_quit(&mut app);
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::AwaitingPaste)
    );
    app.assets.paste_pending = false;
    let receipt = paste_failed(&app, AssetPasteRollback::default());
    app.handle_io_completion(None, receipt);
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::AwaitingAssetGc)
    );
}

#[test]
fn the_sync_report_names_what_it_could_not_do() {
    let (mut app, request_id) = syncing();
    let report = AssetReconcileReport {
        physical_delete_deferred: true,
        ..AssetReconcileReport::default()
    };
    app.handle_io_completion(None, sync_done(request_id, report));
    assert!(!app.assets.sync_in_flight);
    assert_eq!(app.assets.sync_request_id, None);
    assert_eq!(
        diagnostic(&app),
        "便签磁盘状态正在变化；已保留回收站证据并延后物理清理。"
    );

    let (mut app, request_id) = syncing();
    let report = AssetReconcileReport {
        physical_delete_deferred: true,
        missing_references: vec![ManagedAssetName::parse(ASSET).unwrap()],
        ..AssetReconcileReport::default()
    };
    app.handle_io_completion(None, sync_done(request_id, report));
    assert_eq!(
        diagnostic(&app),
        "有 1 个受管图片引用缺少文件；Markdown 未修改。"
    );

    let (mut app, request_id) = syncing();
    app.handle_io_completion(None, sync_failed(request_id));
    assert_eq!(
        diagnostic(&app),
        format!("图片整理未完成；用户文件与文档均未删除：{SYNC_FAILED}")
    );
}

/// A coalesced request's receipt does not end the newer one, but its report still shows.
#[test]
fn a_superseded_sync_receipt_keeps_the_newer_request_in_flight() {
    let (mut app, request_id) = syncing();
    app.handle_io_completion(None, sync_failed(request_id + 1));
    assert!(app.assets.sync_in_flight);
    assert_eq!(app.assets.sync_request_id, Some(request_id));
    assert_eq!(
        diagnostic(&app),
        format!("图片整理未完成；用户文件与文档均未删除：{SYNC_FAILED}")
    );
}

#[test]
fn the_last_sync_after_recovery_enables_watching_and_recreates_a_missing_note() {
    let mut app = fixture("start");
    app.sync_assets_after_recovery();
    assert!(app.assets.reconcile_pending);
    let request_id = app.assets.sync_request_id.unwrap();
    let before = saves(&app);
    app.handle_io_completion(None, sync_done(request_id, AssetReconcileReport::default()));
    assert!(!app.assets.reconcile_pending);
    assert!(app.watcher.is_some());
    assert_eq!(saves(&app), before + 1);
    assert!(app.persistence.durability_required());
}

#[test]
fn the_sync_sequence_stops_when_it_is_used_up() {
    let mut app = fixture("start");
    app.assets.sync_sequence = u64::MAX;
    let current = generation(&app);
    app.submit_asset_sync(current, Vec::new(), None, false);
    assert!(!app.assets.sync_in_flight);
    assert_eq!(diagnostic(&app), "图片事务序号已耗尽；已停止自动整理。");
}

/// The quit sync's receipt moves the quit on: the fixture has no config to flush, so it reaches
/// the end, and the shell has no event loop to exit with.
#[test]
fn a_quit_sync_receipt_completes_the_asset_barrier() {
    let mut app = fixture("start");
    request_quit(&mut app);
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::AwaitingAssetGc)
    );
    let request_id = app.assets.quit_sync_request_id.unwrap();
    app.handle_io_completion(None, sync_done(request_id, AssetReconcileReport::default()));
    assert_eq!(app.assets.quit_sync_request_id, None);
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::ReadyToExit)
    );
    assert_eq!(diagnostic(&app), "退出屏障完成，但当前事件没有退出权限。");
}

#[test]
fn an_export_receipt_ends_the_export_and_reports_it() {
    let mut app = fixture("start");
    app.export_in_flight = true;
    app.handle_io_completion(
        None,
        export_done(Ok(ExportCompletion {
            generation: Generation::initial(),
            target: "out.md".into(),
            copied_assets: 2,
        })),
    );
    assert!(!app.export_in_flight);
    assert_eq!(diagnostic(&app), "导出完成：out.md（2 个本地资源）");

    app.export_in_flight = true;
    app.handle_io_completion(
        None,
        export_done(Err(failure("export target has no parent directory"))),
    );
    assert!(!app.export_in_flight);
    assert_eq!(
        diagnostic(&app),
        "导出失败；工作文档未改变：export target has no parent directory"
    );
}

#[test]
fn a_failed_config_write_reports_and_keeps_the_change_dirty() {
    let mut app = fixture("start");
    app.config
        .update(|config| config.view_mode = ViewMode::Split)
        .unwrap();
    let request = app.config.begin_persist().unwrap();
    app.handle_io_completion(None, config_failed(request.revision));
    assert!(app.config.is_dirty());
    assert!(!app.config.is_saving());
    assert_eq!(
        diagnostic(&app),
        format!("配置保存失败；笔记保存不受影响：{CONFIG_FAILED}")
    );
}

#[test]
fn a_newer_config_is_submitted_after_the_receipt() {
    let mut app = fixture("start");
    app.config_persistence_allowed = true;
    app.config
        .update(|config| config.view_mode = ViewMode::Split)
        .unwrap();
    let request = app.config.begin_persist().unwrap();
    app.config
        .update(|config| config.view_mode = ViewMode::Preview)
        .unwrap();
    app.handle_io_completion(
        None,
        IoCompletion::Config {
            revision: request.revision,
            result: Ok(()),
        },
    );
    assert!(app.config.is_saving());
}

#[test]
fn the_config_receipt_completes_the_quit() {
    let mut app = fixture("start");
    app.config_persistence_allowed = true;
    app.config
        .update(|config| config.view_mode = ViewMode::Split)
        .unwrap();
    request_quit(&mut app);
    let request_id = app.assets.quit_sync_request_id.unwrap();
    app.handle_io_completion(None, sync_done(request_id, AssetReconcileReport::default()));
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::AwaitingConfig)
    );
    assert!(app.config.is_saving());
    let revision = app.config.revision();
    app.handle_io_completion(None, config_failed(revision));
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::ReadyToExit)
    );
}

#[test]
fn a_stopped_worker_fails_every_barrier_and_says_so() {
    let mut app = fixture("start");
    type_letter(&mut app);
    request_quit(&mut app);
    assert_eq!(
        lifecycle(&app),
        LifecycleState::Quitting(QuitStage::AwaitingNoteSave)
    );
    app.assets.sync_in_flight = true;
    app.assets.sync_request_id = Some(9);
    app.assets.quit_sync_request_id = Some(9);
    app.assets.paste_pending = true;
    app.handle_io_completion(None, IoCompletion::WorkerStopped);
    assert!(!app.assets.paste_pending);
    assert!(!app.assets.sync_in_flight);
    assert_eq!(app.assets.sync_request_id, None);
    assert_eq!(app.assets.quit_sync_request_id, None);
    assert_eq!(lifecycle(&app), LifecycleState::Running);
    assert!(app.quit_notice.active());
    assert_eq!(diagnostic(&app), "退出已取消：NoteSave 未安全完成。");

    let mut app = fixture("start");
    app.handle_io_completion(None, IoCompletion::WorkerStopped);
    assert_eq!(
        diagnostic(&app),
        "持久化工作线程已停止；不会假装自动保存仍可用。"
    );
}
