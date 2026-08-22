//! Windows native shell with persistence and read-only Preview coordination.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell
//! plan_ref: docs/plan/07_editor_and_ime.md#ime-semantics
//!
//! This shell translates and presents. It cannot obtain `&mut DocumentState`;
//! all canonical mutations flow through `EditorCoordinator::dispatch`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use stickymd_render::preview::{PreviewFrame, PreviewSelection, SpanAction};
use stickymd_render::source::SourceProjection;
use tiny_skia::Pixmap;
use winit::dpi::PhysicalPosition;
use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;
use winit::window::{Theme, Window};

use crate::config::{ConfigCoordinator, ViewMode};
use crate::flow::window::reducer::WindowShellCoordinator;
use crate::flow::{
    AppEffect, EditorCoordinator, PersistenceCoordinator, PreviewCoordinator, RecoveryCoordinator,
};
use crate::instruction::AppIntent;
use crate::interaction::EditorSession;
use crate::persistence::{IoCompletion, PersistenceWorker};
use crate::platform::windows::ArboardClipboard;
use crate::platform::windows::file_watch::NoteDirectoryWatcher;
use crate::platform::windows::native_message::NativeWindowSignal;
use crate::platform::windows::program_dir::RuntimePaths;
use crate::platform::windows::single_instance::SingleInstanceGuard;
use crate::platform::windows::tray::{TrayController, TrayPlatformEvent};
use crate::preview::{PreviewCompletion, PreviewWorker};
use crate::startup::BootstrapOutcome;
use crate::startup::StartupDiagnostics;
use crate::surface::SoftwareSurface;

mod assets_runtime;
mod caret_runtime;
mod controls;
mod export_runtime;
mod input;
mod lifecycle;
mod persistence_runtime;
mod presentation;
mod preview_input;
mod preview_runtime;
mod reconciliation_runtime;
mod recovery_runtime;
mod toolbar_paint;
mod window_geometry_runtime;
mod window_interaction;
mod window_runtime;

const CARET_BLINK: Duration = Duration::from_millis(550);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalReconcileFollowUp {
    FullProjectionResync,
    RuntimeAssetConvergence,
}

fn external_reconcile_followups(
    reconciled: stickymd_core::Generation,
    current: stickymd_core::Generation,
) -> [ExternalReconcileFollowUp; 2] {
    debug_assert_eq!(reconciled, current);
    [
        ExternalReconcileFollowUp::FullProjectionResync,
        ExternalReconcileFollowUp::RuntimeAssetConvergence,
    ]
}

#[derive(Debug)]
pub enum AppEvent {
    Io(IoCompletion),
    NoteFsHint,
    WatchFailed(String),
    ShowRequested,
    Tray(TrayPlatformEvent),
    Native(NativeWindowSignal),
    Preview(PreviewCompletion),
}

pub struct StickyApp {
    window: Option<Arc<Window>>,
    surface: Option<SoftwareSurface>,
    projection: Option<SourceProjection>,
    source_frame: Option<Pixmap>,
    source_paint_key: Option<presentation::SourcePaintKey>,
    pending_redraw: presentation::PendingRedraw,
    preview_frame: Option<PreviewFrame>,
    preview_worker: Option<PreviewWorker>,
    preview_flow: PreviewCoordinator,
    preview_selection: PreviewSelection,
    preview_scroll_y: f32,
    preview_focused: bool,
    preview_dragging: bool,
    preview_drag_moved: bool,
    preview_press_position: Option<PhysicalPosition<f64>>,
    preview_press_action: Option<SpanAction>,
    coordinator: EditorCoordinator<ArboardClipboard>,
    persistence: PersistenceCoordinator,
    paths: RuntimePaths,
    config: ConfigCoordinator,
    config_persistence_allowed: bool,
    recovery: RecoveryCoordinator,
    worker: PersistenceWorker,
    watcher: Option<NoteDirectoryWatcher>,
    proxy: EventLoopProxy<AppEvent>,
    _instance: SingleInstanceGuard,
    resolving_keep_local: bool,
    asset_paste_pending: bool,
    asset_sync_in_flight: bool,
    asset_sync_sequence: u64,
    asset_sync_request_id: Option<u64>,
    quit_asset_sync_request_id: Option<u64>,
    asset_reconcile_pending: bool,
    export_in_flight: bool,
    session: EditorSession,
    modifiers: ModifiersState,
    cursor_position: PhysicalPosition<f64>,
    pointer_inside_window: bool,
    started: Instant,
    next_blink: Instant,
    native_caret_drawn: bool,
    native_caret_failed: bool,
    diagnostic: Option<String>,
    system_theme: Theme,
    controls: controls::ControlState,
    window_flow: Option<WindowShellCoordinator>,
    tray: Option<TrayController>,
    shell_input_enabled: bool,
    move_resize_active: bool,
    zoom_wheel: input::ZoomWheelAccumulator,
    zoom_config_deadline: Option<u64>,
    startup_diagnostics: StartupDiagnostics,
}

impl StickyApp {
    pub fn new(
        paths: RuntimePaths,
        bootstrap: BootstrapOutcome,
        instance: SingleInstanceGuard,
        worker: PersistenceWorker,
        proxy: EventLoopProxy<AppEvent>,
        startup_diagnostics: StartupDiagnostics,
    ) -> Self {
        let now = Instant::now();
        let preview_focused = bootstrap.config.view_mode == ViewMode::Preview;
        let opacity = bootstrap.config.opacity;
        let mut app = Self {
            window: None,
            surface: None,
            projection: None,
            source_frame: None,
            source_paint_key: None,
            pending_redraw: presentation::PendingRedraw::None,
            preview_frame: None,
            preview_worker: None,
            preview_flow: PreviewCoordinator::default(),
            preview_selection: PreviewSelection::default(),
            preview_scroll_y: 0.0,
            preview_focused,
            preview_dragging: false,
            preview_drag_moved: false,
            preview_press_position: None,
            preview_press_action: None,
            coordinator: EditorCoordinator::new(bootstrap.document, ArboardClipboard::new()),
            persistence: PersistenceCoordinator::default(),
            paths,
            config: ConfigCoordinator::loaded(bootstrap.config),
            config_persistence_allowed: bootstrap.config_persistence_allowed,
            recovery: RecoveryCoordinator::new(
                bootstrap.recovery,
                bootstrap.recovery_canonical_requires_preserve,
            ),
            worker,
            watcher: None,
            proxy,
            _instance: instance,
            resolving_keep_local: false,
            asset_paste_pending: false,
            asset_sync_in_flight: false,
            asset_sync_sequence: 0,
            asset_sync_request_id: None,
            quit_asset_sync_request_id: None,
            asset_reconcile_pending: false,
            export_in_flight: false,
            session: EditorSession::default(),
            modifiers: ModifiersState::default(),
            cursor_position: PhysicalPosition::new(0.0, 0.0),
            pointer_inside_window: false,
            started: now,
            next_blink: now + CARET_BLINK,
            native_caret_drawn: false,
            native_caret_failed: false,
            diagnostic: bootstrap.warnings.last().cloned(),
            system_theme: Theme::Light,
            controls: controls::ControlState::new(opacity),
            window_flow: None,
            tray: None,
            shell_input_enabled: true,
            move_resize_active: false,
            zoom_wheel: input::ZoomWheelAccumulator::default(),
            zoom_config_deadline: None,
            startup_diagnostics,
        };
        if app.recovery.is_pending() {
            app.persistence.set_recovery_pending(true);
            app.diagnostic =
                Some("发现未完成保存的内容  |  [F6 恢复临时内容]  [F7 使用当前文件]".into());
        } else {
            app.start_watcher();
        }
        app
    }

    fn document_scale_factor(&self) -> f32 {
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        dpi * self.config.current().content_zoom_percent.factor()
    }

    fn resolved_dark_theme(&self) -> bool {
        match self.config.current().theme {
            crate::config::ThemeMode::Light => false,
            crate::config::ThemeMode::Dark => true,
            crate::config::ThemeMode::System => self.system_theme == Theme::Dark,
        }
    }

    fn dispatch(&mut self, intent: AppIntent) {
        match self.coordinator.dispatch(intent) {
            Ok(effect) => self.apply_effect(effect),
            Err(error) => {
                self.diagnostic = Some(error.to_string());
                self.request_redraw();
            }
        }
    }

    fn apply_effect(&mut self, effect: AppEffect) {
        match effect {
            AppEffect::DocumentChanged {
                generation,
                selection,
                delta,
                asset_effects,
            } => {
                self.persistence
                    .on_document_changed(self.timestamp_ms(), generation);
                self.on_preview_document_changed(generation);
                self.session.accept_document_selection(selection);
                if let Some(projection) = &mut self.projection
                    && let Err(error) = projection.apply_delta(generation, &delta)
                {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "source projection resync: error={error} target_generation={}",
                        generation.value()
                    );
                    let snapshot = self.coordinator.snapshot();
                    if let Err(resync_error) = projection.resync(&snapshot) {
                        self.diagnostic = Some(format!(
                            "projection update failed: {error}; resync failed: {resync_error}"
                        ));
                    }
                }
                self.after_presentation_change();
                self.update_window_title();
                if !asset_effects.is_empty() {
                    self.submit_asset_sync(generation, asset_effects, None, false);
                }
            }
            AppEffect::AssetPasteRequested(request) => self.submit_asset_paste(request),
            AppEffect::ExternalDocumentReconciled { generation } => {
                for followup in
                    external_reconcile_followups(generation, self.coordinator.view().generation)
                {
                    match followup {
                        ExternalReconcileFollowUp::FullProjectionResync => {
                            self.full_projection_resync()
                        }
                        ExternalReconcileFollowUp::RuntimeAssetConvergence => self
                            .submit_asset_sync(
                                self.coordinator.view().generation,
                                Vec::new(),
                                Some(crate::assets::AssetReconcileMode::Runtime),
                                false,
                            ),
                    }
                }
            }
            AppEffect::ClipboardWritten => {
                self.diagnostic = Some("Clipboard updated".to_owned());
                self.request_redraw();
            }
            AppEffect::NoOp => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use stickymd_core::{DocumentState, EditKind, LineEnding, Selection};
    use tiny_skia::Pixmap;
    use unicode_segmentation::UnicodeSegmentation;

    use super::*;
    use crate::flow::{ClipboardError, ClipboardPort};
    use crate::interaction::ImeSignal;

    #[test]
    fn external_reconcile_handler_requires_projection_resync_and_asset_convergence() {
        let generation = stickymd_core::Generation::initial();
        assert_eq!(
            external_reconcile_followups(generation, generation),
            [
                ExternalReconcileFollowUp::FullProjectionResync,
                ExternalReconcileFollowUp::RuntimeAssetConvergence,
            ]
        );
    }

    #[derive(Default)]
    struct MemoryClipboard;

    impl ClipboardPort for MemoryClipboard {
        fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
            Ok(None)
        }

        fn write_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
            Ok(())
        }
    }

    #[test]
    fn synthetic_ime_pipeline_mutates_only_on_commit() {
        let mut coordinator = EditorCoordinator::empty(MemoryClipboard);
        let mut session = EditorSession::default();
        let before = coordinator.snapshot();

        assert!(
            session
                .handle_ime(ImeSignal::Enabled, before.generation, 0)
                .is_none()
        );
        assert!(
            session
                .handle_ime(
                    ImeSignal::Preedit {
                        text: "nihao".to_owned(),
                        cursor: Some(5..5),
                    },
                    before.generation,
                    1,
                )
                .is_none()
        );
        assert_eq!(coordinator.snapshot(), before);
        assert!(
            session
                .handle_keyboard_text("nihao", before.generation, 2)
                .is_none()
        );

        let intent = session
            .handle_ime(ImeSignal::Commit("你好".to_owned()), before.generation, 3)
            .unwrap();
        let effect = coordinator.dispatch(intent).unwrap();
        assert!(matches!(effect, AppEffect::DocumentChanged { .. }));
        let committed = coordinator.snapshot();
        assert_eq!(&*committed.text, "你好");
        assert_eq!(committed.generation.value(), 1);
        coordinator.dispatch(AppIntent::Undo).unwrap();
        let undone = coordinator.snapshot();
        assert!(undone.text.is_empty());
        assert_eq!(undone.generation.value(), 2);
    }

    #[test]
    fn ime_commit_replaces_collapsed_forward_reverse_cjk_and_emoji_selections() {
        let cases = [
            ("ab", Selection::caret(1), "a中b"),
            ("abc", Selection::new(0, 3), "中"),
            ("abc", Selection::new(3, 0), "中"),
            ("你好", Selection::new(0, "你好".len()), "中"),
            ("a🙂b", Selection::new(1, 1 + "🙂".len()), "a中b"),
        ];

        for (source, selection, expected) in cases {
            let document = DocumentState::loaded(source, LineEnding::Lf, None);
            let mut coordinator = EditorCoordinator::new(document, MemoryClipboard);
            let generation = coordinator.view().generation;
            let mut session = EditorSession {
                selection,
                ..EditorSession::default()
            };
            session.handle_ime(ImeSignal::Enabled, generation, 0);
            session.handle_ime(
                ImeSignal::Preedit {
                    text: "zhong".to_owned(),
                    cursor: Some(5..5),
                },
                generation,
                1,
            );
            let intent = session
                .handle_ime(ImeSignal::Commit("中".to_owned()), generation, 2)
                .unwrap();
            coordinator.dispatch(intent).unwrap();
            assert_eq!(coordinator.view().text, expected);
        }
    }

    #[test]
    fn deterministic_editor_sequence_keeps_projection_and_authority_synchronized() {
        let mut coordinator = EditorCoordinator::empty(MemoryClipboard);
        let initial = coordinator.snapshot();
        let mut projection = SourceProjection::new(&initial, 500, 300, 1.0);
        let mut session = EditorSession::default();
        let mut seed = 0xA11C_E5E1_D123_4567u64;
        let inserts = ["a", "中", "🙂", "e\u{301}", "\n"];

        for step in 0..160u64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let boundaries = coordinator
                .view()
                .text
                .grapheme_indices(true)
                .map(|(byte, _)| byte)
                .chain(std::iter::once(coordinator.view().text.len()))
                .collect::<Vec<_>>();
            let first = boundaries[(seed as usize) % boundaries.len()];
            let second = boundaries[((seed >> 16) as usize) % boundaries.len()];
            let selection = Selection::new(first, second);
            let view = coordinator.view();
            let intent = match seed % 7 {
                0 | 1 => Some(AppIntent::Edit {
                    expected_generation: view.generation,
                    selection: Selection::caret(first),
                    inserted: inserts[((seed >> 24) as usize) % inserts.len()].to_owned(),
                    kind: EditKind::Typing,
                    timestamp_ms: step * 1_000,
                }),
                2 => Some(AppIntent::Edit {
                    expected_generation: view.generation,
                    selection,
                    inserted: "替".to_owned(),
                    kind: EditKind::SelectionReplace,
                    timestamp_ms: step * 1_000,
                }),
                3 if boundaries.len() > 1 => {
                    let index = (seed as usize) % (boundaries.len() - 1);
                    Some(AppIntent::Edit {
                        expected_generation: view.generation,
                        selection: Selection::new(boundaries[index], boundaries[index + 1]),
                        inserted: String::new(),
                        kind: EditKind::DeleteForward,
                        timestamp_ms: step * 1_000,
                    })
                }
                4 => Some(AppIntent::Undo),
                5 => Some(AppIntent::Redo),
                _ => {
                    session.selection = Selection::caret(first);
                    None
                }
            };

            if let Some(intent) = intent
                && let Ok(effect) = coordinator.dispatch(intent)
                && let AppEffect::DocumentChanged {
                    generation,
                    selection,
                    delta,
                    ..
                } = effect
            {
                session.accept_document_selection(selection);
                if let Err(error) = projection.apply_delta(generation, &delta) {
                    assert_eq!(
                        error,
                        stickymd_render::source::SourceProjectionError::ResyncRequired,
                        "step={step} delta={delta:?} projected={:?} canonical={:?}",
                        projection.projected_text(),
                        coordinator.view().text,
                    );
                    projection.resync(&coordinator.snapshot()).unwrap();
                }
            }

            let view = coordinator.view();
            assert!(view.text.is_char_boundary(session.selection.anchor.byte));
            assert!(view.text.is_char_boundary(session.selection.active.byte));
            assert_eq!(projection.projected_generation(), view.generation);
            assert_eq!(projection.projected_text(), view.text);
        }
    }

    struct Stats {
        p50: Duration,
        p95: Duration,
        max: Duration,
    }

    struct PipelineStats {
        total: Stats,
        mutation: Stats,
        projection: Stats,
        caret: Stats,
        paint: Stats,
    }

    struct OperationStats {
        backspace: Stats,
        delete_forward: Stats,
        selection_replace: Stats,
        newline: Stats,
        undo: Stats,
        redo: Stats,
        full_resync: Stats,
    }

    #[derive(Clone, Copy)]
    enum EditLocation {
        Start,
        Middle,
        End,
    }

    fn stats(mut samples: Vec<Duration>) -> Stats {
        samples.sort_unstable();
        let len = samples.len();
        Stats {
            p50: samples[len / 2],
            p95: samples[((len as f64) * 0.95).ceil() as usize - 1],
            max: samples[len - 1],
        }
    }

    fn fixture(bytes: usize) -> String {
        let source = include_str!("../../../tests/fixtures/performance/typical-note-seed.md");
        let mut text = String::with_capacity(bytes + source.len());
        while text.len() < bytes {
            text.push_str(source);
        }
        while text.len() > bytes {
            text.pop();
        }
        text
    }

    fn run_pipeline_samples(
        bytes: usize,
        kind: EditKind,
        inserted: &str,
        location: EditLocation,
    ) -> PipelineStats {
        let document = DocumentState::loaded(&fixture(bytes), LineEnding::Lf, None);
        let mut coordinator = EditorCoordinator::new(document, MemoryClipboard);
        let initial = coordinator.snapshot();
        let mut projection = SourceProjection::new(&initial, 800, 600, 1.0);
        let mut pixmap = Pixmap::new(800, 600).unwrap();
        let mut total_samples = Vec::with_capacity(30);
        let mut mutation_samples = Vec::with_capacity(30);
        let mut projection_samples = Vec::with_capacity(30);
        let mut caret_samples = Vec::with_capacity(30);
        let mut paint_samples = Vec::with_capacity(30);

        for iteration in 0..35u64 {
            let view = coordinator.view();
            let position = match location {
                EditLocation::Start => 0,
                EditLocation::Middle => {
                    let mut byte = view.text.len() / 2;
                    while !view.text.is_char_boundary(byte) {
                        byte -= 1;
                    }
                    byte
                }
                EditLocation::End => view.text.len(),
            };
            let intent = AppIntent::Edit {
                expected_generation: view.generation,
                selection: Selection::caret(position),
                inserted: inserted.to_owned(),
                kind,
                timestamp_ms: iteration * 1_000,
            };
            let started = Instant::now();
            let mutation_started = Instant::now();
            let AppEffect::DocumentChanged {
                generation,
                selection,
                delta,
                ..
            } = coordinator.dispatch(intent).unwrap()
            else {
                panic!("benchmark edit unexpectedly produced no document change");
            };
            let mutation = mutation_started.elapsed();
            let projection_started = Instant::now();
            projection.apply_delta(generation, &delta).unwrap();
            let projection_elapsed = projection_started.elapsed();
            let caret_started = Instant::now();
            projection
                .ensure_caret_visible(selection.active.byte)
                .unwrap();
            let caret = caret_started.elapsed();
            let paint_started = Instant::now();
            projection
                .paint(
                    &mut pixmap,
                    selection,
                    true,
                    true,
                    None,
                    stickymd_render::source::SourceTheme::Light,
                )
                .unwrap();
            let paint = paint_started.elapsed();
            if iteration >= 5 {
                total_samples.push(started.elapsed());
                mutation_samples.push(mutation);
                projection_samples.push(projection_elapsed);
                caret_samples.push(caret);
                paint_samples.push(paint);
            }
        }
        PipelineStats {
            total: stats(total_samples),
            mutation: stats(mutation_samples),
            projection: stats(projection_samples),
            caret: stats(caret_samples),
            paint: stats(paint_samples),
        }
    }

    fn middle_grapheme_range(text: &str) -> std::ops::Range<usize> {
        let middle = text.len() / 2;
        text.grapheme_indices(true)
            .find_map(|(start, grapheme)| {
                let end = start + grapheme.len();
                (end >= middle).then_some(start..end)
            })
            .unwrap_or(text.len()..text.len())
    }

    fn apply_timed_effect(
        coordinator: &mut EditorCoordinator<MemoryClipboard>,
        projection: &mut SourceProjection,
        pixmap: &mut Pixmap,
        intent: AppIntent,
    ) -> Duration {
        let started = Instant::now();
        let AppEffect::DocumentChanged {
            generation,
            selection,
            delta,
            ..
        } = coordinator.dispatch(intent).unwrap()
        else {
            panic!("benchmark operation unexpectedly produced no document change");
        };
        if projection.apply_delta(generation, &delta).is_err() {
            projection.resync(&coordinator.snapshot()).unwrap();
        }
        projection
            .ensure_caret_visible(selection.active.byte)
            .unwrap();
        projection
            .paint(
                pixmap,
                selection,
                true,
                true,
                None,
                stickymd_render::source::SourceTheme::Light,
            )
            .unwrap();
        started.elapsed()
    }

    fn run_operation_samples(bytes: usize) -> OperationStats {
        let document = DocumentState::loaded(&fixture(bytes), LineEnding::Lf, None);
        let mut coordinator = EditorCoordinator::new(document, MemoryClipboard);
        let mut projection = SourceProjection::new(&coordinator.snapshot(), 800, 600, 1.0);
        let mut pixmap = Pixmap::new(800, 600).unwrap();
        let mut backspace = Vec::with_capacity(20);
        let mut delete_forward = Vec::with_capacity(20);
        let mut selection_replace = Vec::with_capacity(20);
        let mut newline = Vec::with_capacity(20);
        let mut undo = Vec::with_capacity(20);
        let mut redo = Vec::with_capacity(20);
        let mut full_resync = Vec::with_capacity(20);

        for iteration in 0..25u64 {
            let view = coordinator.view();
            let range = middle_grapheme_range(view.text);
            let generation = view.generation;
            let timestamp_ms = iteration * 10_000;
            let backspace_elapsed = apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Edit {
                    expected_generation: generation,
                    selection: Selection::new(range.start, range.end),
                    inserted: String::new(),
                    kind: EditKind::Backspace,
                    timestamp_ms,
                },
            );
            let undo_elapsed = apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Undo,
            );
            let redo_elapsed = apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Redo,
            );
            apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Undo,
            );

            let view = coordinator.view();
            let range = middle_grapheme_range(view.text);
            let generation = view.generation;
            let delete_elapsed = apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Edit {
                    expected_generation: generation,
                    selection: Selection::new(range.start, range.end),
                    inserted: String::new(),
                    kind: EditKind::DeleteForward,
                    timestamp_ms: timestamp_ms + 1_000,
                },
            );
            apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Undo,
            );

            let view = coordinator.view();
            let range = middle_grapheme_range(view.text);
            let generation = view.generation;
            let replace_elapsed = apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Edit {
                    expected_generation: generation,
                    selection: Selection::new(range.start, range.end),
                    inserted: "替".to_owned(),
                    kind: EditKind::SelectionReplace,
                    timestamp_ms: timestamp_ms + 2_000,
                },
            );
            apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Undo,
            );

            let view = coordinator.view();
            let position = middle_grapheme_range(view.text).start;
            let generation = view.generation;
            let newline_elapsed = apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Edit {
                    expected_generation: generation,
                    selection: Selection::caret(position),
                    inserted: "\n".to_owned(),
                    kind: EditKind::Newline,
                    timestamp_ms: timestamp_ms + 3_000,
                },
            );
            apply_timed_effect(
                &mut coordinator,
                &mut projection,
                &mut pixmap,
                AppIntent::Undo,
            );

            let rebuild_started = Instant::now();
            projection.resync(&coordinator.snapshot()).unwrap();
            projection
                .paint(
                    &mut pixmap,
                    Selection::caret(0),
                    true,
                    true,
                    None,
                    stickymd_render::source::SourceTheme::Light,
                )
                .unwrap();
            let rebuild_elapsed = rebuild_started.elapsed();

            if iteration >= 5 {
                backspace.push(backspace_elapsed);
                delete_forward.push(delete_elapsed);
                selection_replace.push(replace_elapsed);
                newline.push(newline_elapsed);
                undo.push(undo_elapsed);
                redo.push(redo_elapsed);
                full_resync.push(rebuild_elapsed);
            }
        }

        OperationStats {
            backspace: stats(backspace),
            delete_forward: stats(delete_forward),
            selection_replace: stats(selection_replace),
            newline: stats(newline),
            undo: stats(undo),
            redo: stats(redo),
            full_resync: stats(full_resync),
        }
    }

    fn duration_ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1_000.0
    }

    #[test]
    #[ignore = "release-only Phase 3 latency baseline"]
    fn phase3_source_pipeline_release_baseline() {
        for (label, bytes) in [
            ("20 KiB", 20 * 1024),
            ("100 KiB", 100 * 1024),
            ("1 MiB", 1024 * 1024),
        ] {
            let typing_end = run_pipeline_samples(bytes, EditKind::Typing, "x", EditLocation::End);
            let typing_start =
                run_pipeline_samples(bytes, EditKind::Typing, "x", EditLocation::Start);
            let ime_middle =
                run_pipeline_samples(bytes, EditKind::ImeCommit, "中", EditLocation::Middle);
            let operations = run_operation_samples(bytes);
            eprintln!(
                "{label}: end typing total p50={:.3}ms p95={:.3}ms max={:.3}ms [mutation={:.3} projection={:.3} caret={:.3} paint={:.3} p95]; start typing p95={:.3}ms [mutation={:.3} projection={:.3} caret={:.3} paint={:.3}]; middle IME p95={:.3}ms [mutation={:.3} projection={:.3} caret={:.3} paint={:.3}]",
                duration_ms(typing_end.total.p50),
                duration_ms(typing_end.total.p95),
                duration_ms(typing_end.total.max),
                duration_ms(typing_end.mutation.p95),
                duration_ms(typing_end.projection.p95),
                duration_ms(typing_end.caret.p95),
                duration_ms(typing_end.paint.p95),
                duration_ms(typing_start.total.p95),
                duration_ms(typing_start.mutation.p95),
                duration_ms(typing_start.projection.p95),
                duration_ms(typing_start.caret.p95),
                duration_ms(typing_start.paint.p95),
                duration_ms(ime_middle.total.p95),
                duration_ms(ime_middle.mutation.p95),
                duration_ms(ime_middle.projection.p95),
                duration_ms(ime_middle.caret.p95),
                duration_ms(ime_middle.paint.p95),
            );
            eprintln!(
                "{label}: backspace={:.3}ms delete={:.3}ms selection-replace={:.3}ms newline={:.3}ms undo={:.3}ms redo={:.3}ms full-resync={:.3}ms p95",
                duration_ms(operations.backspace.p95),
                duration_ms(operations.delete_forward.p95),
                duration_ms(operations.selection_replace.p95),
                duration_ms(operations.newline.p95),
                duration_ms(operations.undo.p95),
                duration_ms(operations.redo.p95),
                duration_ms(operations.full_resync.p95),
            );
            let typing_limit = match bytes {
                n if n <= 20 * 1024 => 16.0,
                n if n <= 100 * 1024 => 25.0,
                _ => 50.0,
            };
            assert!(
                duration_ms(typing_end.total.p95) <= typing_limit,
                "{label} end typing p95 exceeded {typing_limit}ms"
            );
            assert!(
                duration_ms(typing_start.total.p95) <= typing_limit,
                "{label} start typing p95 exceeded {typing_limit}ms"
            );
            assert!(
                duration_ms(ime_middle.total.p95) <= typing_limit,
                "{label} middle IME commit p95 exceeded {typing_limit}ms"
            );
            for (operation, measured) in [
                ("backspace", operations.backspace.p95),
                ("delete", operations.delete_forward.p95),
                ("selection replace", operations.selection_replace.p95),
                ("newline", operations.newline.p95),
                ("undo", operations.undo.p95),
                ("redo", operations.redo.p95),
            ] {
                assert!(
                    duration_ms(measured) <= typing_limit,
                    "{label} {operation} p95 exceeded {typing_limit}ms"
                );
            }
            let rebuild_limit = if bytes <= 100 * 1024 { 50.0 } else { 200.0 };
            assert!(
                duration_ms(operations.full_resync.p95) <= rebuild_limit,
                "{label} full resync p95 exceeded {rebuild_limit}ms"
            );
        }
    }
}
