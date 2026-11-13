//! The shell with persistence and read-only Preview coordination, generic over its Platform Host.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell
//! plan_ref: docs/plan/07_editor_and_ime.md#ime-semantics
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor
//!
//! This shell translates and presents. It cannot obtain `&mut DocumentState`;
//! all canonical mutations flow through `EditorCoordinator::dispatch`.

use std::sync::Arc;
use std::time::Instant;

use stickymd_render::preview::{PreviewFrame, PreviewSelection, SpanAction};
use stickymd_render::source::SourceProjection;
use tiny_skia::Pixmap;
use winit::dpi::PhysicalPosition;
use winit::keyboard::ModifiersState;
use winit::window::{Theme, Window};

use crate::controls::{ControlLayout, ControlState};
use crate::frame::SoftwareSurface;
use crate::host::{EventSink, Host};
use crate::scroll::ScrollbarInteraction;
use crate::title::{PLAIN, QuitCancelNotice};
use stickymd_app::config::ViewMode;
use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::ports::{PreviewViewport, TrayView};
use stickymd_app::session::{AppSession, ShellEffects};

/// Where a press ended, for the routed map of the shell (`shell_pins_tests`): an armed probe
/// records it here and stops the press before it acts. Test builds only.
macro_rules! press_probe {
    ($app:expr, $route:expr $(, $value:expr)?) => {
        #[cfg(test)]
        if $app.press_probe.stops($route) {
            return $($value)?;
        }
    };
}

mod caret_runtime;
mod escape;
mod export_runtime;
mod guards;
mod handler;
#[cfg(any(test, feature = "testing"))]
mod headless_tests;
#[cfg(test)]
mod hidden_frames_tests;
#[cfg(test)]
mod hot_path_tests;
mod input;
mod lifecycle;
mod opacity_field;
mod persistence_runtime;
#[cfg(test)]
mod phase15_host_tests;
#[cfg(test)]
mod phase15_receipt_tests;
#[cfg(test)]
mod phase15_session_tests;
#[cfg(test)]
mod phase15_worker_receipt_tests;
mod pointer;
mod presentation;
mod preview_horizontal;
mod preview_input;
mod preview_runtime;
#[cfg(test)]
mod receipt_tests;
mod render;
mod scroll_application;
mod scroll_runtime;
mod search_controller;
mod shell_effects;
#[cfg(test)]
mod shell_pins_tests;
mod startup;
#[cfg(test)]
mod startup_tests;
#[cfg(any(test, feature = "testing"))]
pub mod storage_conformance;
#[cfg(test)]
mod title;
mod window_geometry_runtime;
mod window_interaction;
mod window_runtime;

use crate::host::CARET_BLINK;

pub struct DesktopShell<H: Host> {
    window: Option<Arc<Window>>,
    surface: Option<SoftwareSurface>,
    projection: Option<SourceProjection>,
    source_frame: Option<Pixmap>,
    source_paint_key: Option<render::SourcePaintKey>,
    /// The frame storage is released while the window is hidden (`Host::releases_hidden_frames`):
    /// no frame is painted and nothing window-sized is allocated until the show rebuilds it.
    frames_released: bool,
    pending_redraw: presentation::PendingRedraw,
    preview_frame: Option<PreviewFrame>,
    preview_frame_viewport: Option<PreviewViewport>,
    preview_selection: PreviewSelection,
    preview_horizontal: preview_horizontal::HorizontalSession,
    preview_scroll_y: f32,
    scrollbars: ScrollbarInteraction,
    preview_focused: bool,
    preview_dragging: bool,
    preview_drag_moved: bool,
    preview_press_position: Option<PhysicalPosition<f64>>,
    preview_press_action: Option<SpanAction>,
    app: AppSession,
    /// The output of session steps, reused from event to event (`shell_effects.rs`).
    effects: ShellEffects,
    host: H,
    sink: EventSink,
    quit_notice: QuitCancelNotice,
    /// The title last written to the window, so unchanged projections skip the native call.
    shown_title: String,
    /// Every window effect the shell applied, in order, for the order tests.
    #[cfg(test)]
    applied_window_effects: Vec<String>,
    /// The client size and DPI a windowless test shell reports, so Preview work reaches its worker
    /// and a press takes the path it takes in a window.
    #[cfg(test)]
    headless_view: Option<(winit::dpi::PhysicalSize<u32>, f64)>,
    /// Whether a windowless test shell's native drag or resize starts; `None` has no window to ask.
    #[cfg(test)]
    headless_window_gesture: Option<bool>,
    /// See `press_probe!`.
    #[cfg(test)]
    press_probe: shell_pins_tests::PressProbe,
    modifiers: ModifiersState,
    cursor_position: PhysicalPosition<f64>,
    pointer_inside_window: bool,
    /// The instant the session clock counts from, to turn its deadlines into native wake times.
    clock_origin: Instant,
    next_blink: Instant,
    native_caret_overlay: Option<PhysicalRect>,
    native_caret_failed: bool,
    /// The host's overlay blinks by itself (`CaretOverlay::blinks_itself`).
    overlay_blinks: bool,
    diagnostic: Option<String>,
    system_theme: Theme,
    controls: ControlState,
    /// The one layout paint, hover and press read (`sync_control_layout`).
    control_layout: Option<ControlLayout>,
    /// What the tray was last told.
    tray_view: TrayView,
    /// The quit barriers finished: the callback that got here ends the event loop.
    exit_pending: bool,
    /// The startup ended early: the callbacks do nothing more and the exit code reports it.
    startup_failed: bool,
    shell_input_enabled: bool,
    move_resize_active: bool,
    /// A display change waits for the native message that raised it to return.
    display_recovery_due: bool,
    zoom_wheel: pointer::ZoomWheelAccumulator,
}

impl<H: Host> DesktopShell<H> {
    pub fn host(&self) -> &H {
        &self.host
    }

    /// Whether the startup ended early; the event loop has returned by the time this matters.
    pub fn startup_failed(&self) -> bool {
        self.startup_failed
    }

    pub fn new(
        mut host: H,
        session: AppSession,
        sink: EventSink,
        clock_origin: Instant,
        diagnostic: Option<String>,
    ) -> Self {
        let config = session.config().current();
        let preview_focused = config.view_mode == ViewMode::Preview;
        let opacity = config.opacity;
        let always_on_top = config.always_on_top;
        let caret_in_frame = host.caret_overlay().is_none();
        let overlay_blinks = host
            .caret_overlay()
            .is_some_and(|overlay| overlay.blinks_itself());
        let mut app = Self {
            window: None,
            surface: None,
            projection: None,
            source_frame: None,
            source_paint_key: None,
            frames_released: false,
            pending_redraw: presentation::PendingRedraw::None,
            preview_frame: None,
            preview_frame_viewport: None,
            preview_selection: PreviewSelection::default(),
            preview_horizontal: preview_horizontal::HorizontalSession::default(),
            preview_scroll_y: 0.0,
            scrollbars: ScrollbarInteraction::default(),
            preview_focused,
            preview_dragging: false,
            preview_drag_moved: false,
            preview_press_position: None,
            preview_press_action: None,
            app: session,
            effects: ShellEffects::default(),
            host,
            sink,
            quit_notice: QuitCancelNotice::default(),
            shown_title: PLAIN.to_owned(),
            #[cfg(test)]
            applied_window_effects: Vec::new(),
            #[cfg(test)]
            headless_view: None,
            #[cfg(test)]
            headless_window_gesture: None,
            #[cfg(test)]
            press_probe: Default::default(),
            modifiers: ModifiersState::default(),
            cursor_position: PhysicalPosition::new(0.0, 0.0),
            pointer_inside_window: false,
            clock_origin,
            next_blink: clock_origin + CARET_BLINK,
            native_caret_overlay: None,
            native_caret_failed: caret_in_frame,
            overlay_blinks,
            diagnostic,
            system_theme: Theme::Light,
            controls: ControlState::new(opacity),
            control_layout: None,
            tray_view: TrayView {
                window_visible: true,
                always_on_top,
            },
            exit_pending: false,
            startup_failed: false,
            shell_input_enabled: true,
            move_resize_active: false,
            display_recovery_due: false,
            zoom_wheel: pointer::ZoomWheelAccumulator::default(),
        };
        if app.app.recovery().is_pending() {
            app.diagnostic =
                Some("发现未完成保存的内容  |  [F6 恢复临时内容]  [F7 使用当前文件]".into());
        } else {
            app.start_watcher();
        }
        app
    }

    /// The client size and DPI input and paint are laid out for; `None` without a window.
    fn shell_view(&self) -> Option<(winit::dpi::PhysicalSize<u32>, f64)> {
        #[cfg(test)]
        if let Some(view) = self.headless_view {
            return Some(view);
        }
        let window = self.window.as_ref()?;
        Some((window.inner_size(), window.scale_factor()))
    }

    /// The DPI alone, for callers that do not need the client size (each size read is a native
    /// query); `None` without a window.
    fn shell_dpi(&self) -> Option<f64> {
        #[cfg(test)]
        if let Some((_, dpi)) = self.headless_view {
            return Some(dpi);
        }
        self.window.as_ref().map(|window| window.scale_factor())
    }

    fn document_scale_factor(&self) -> f32 {
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        dpi * self.app.config().current().content_zoom_percent.factor()
    }

    fn resolved_dark_theme(&self) -> bool {
        match self.app.config().current().theme {
            stickymd_app::config::ThemeMode::Light => false,
            stickymd_app::config::ThemeMode::Dark => true,
            stickymd_app::config::ThemeMode::System => self.system_theme == Theme::Dark,
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
    use stickymd_app::flow::{AppEffect, ClipboardError, ClipboardPort, EditorCoordinator};
    use stickymd_app::instruction::AppIntent;
    use stickymd_app::interaction::{EditorSession, ImeSignal};

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
        let source = include_str!("../../../../tests/fixtures/performance/typical-note-seed.md");
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
