//! The notice bar in the windowless shell: it is drawn over every pane without repainting either,
//! and its buttons and the toolbar marker hand clicks back to the session.
//!
//! plan_ref: docs/plan/09_windows_shell.md#notices

use stickymd_app::config::ViewMode;
use stickymd_app::flow::SaveFaultKind;
use stickymd_app::flow::notice::{Notice, NoticeTone};
use stickymd_core::{ExternalFileFact, ExternalFileState, FileConflict, LineEnding, hash_bytes};
use stickymd_render::source::SourceProjection;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton};

use crate::controls::NoticeRect;
use crate::frame::SoftwareSurface;
use crate::shell::headless_tests::{HeadlessApp, fixture};

const VIEW: PhysicalSize<u32> = PhysicalSize::new(900, 600);

/// A shown window in `mode` with a canvas, a Source frame and the toolbar laid out.
fn shown(mode: ViewMode) -> HeadlessApp {
    let mut app = fixture("start\n");
    app.app
        .config
        .update(|config| config.view_mode = mode)
        .unwrap();
    app.host.presents_frames = true;
    app.headless_view = Some((VIEW, 1.0));
    app.surface = Some(SoftwareSurface::for_host(VIEW.width, VIEW.height).unwrap());
    let snapshot = app.app.coordinator.snapshot();
    app.projection = Some(SourceProjection::new(
        &snapshot,
        VIEW.width,
        VIEW.height,
        1.0,
    ));
    app.configure_viewports();
    app.sync_control_layout();
    app
}

fn pixel(app: &HeadlessApp, x: u32, y: u32) -> (u8, u8, u8) {
    let pixmap = app.surface.as_ref().unwrap().pixmap();
    let color = pixmap.pixel(x, y).unwrap();
    (color.red(), color.green(), color.blue())
}

fn press_at(app: &mut HeadlessApp, x: f64, y: f64) {
    app.cursor_position = PhysicalPosition::new(x, y);
    app.handle_mouse_button(ElementState::Pressed, MouseButton::Left);
    app.handle_mouse_button(ElementState::Released, MouseButton::Left);
}

fn conflict() -> FileConflict {
    FileConflict {
        external: ExternalFileState::Present(ExternalFileFact {
            fingerprint: hash_bytes(b"outside"),
            text: "outside".into(),
            line_ending: LineEnding::Lf,
            durable_len: 7,
        }),
        detected_at_generation: stickymd_core::Generation::initial(),
        previous_disk_fingerprint: None,
    }
}

/// P17-N10: the bar shows in every view, across the whole window in a Split, and a notice coming
/// or going repaints neither pane: the Source frame keeps its key and its pixels, and no Preview
/// job is asked for.
#[test]
fn the_bar_spans_the_window_and_repaints_no_pane() {
    for mode in [ViewMode::Source, ViewMode::Split, ViewMode::Preview] {
        let mut app = shown(mode);
        assert!(app.draw_frame(None));
        let key = app.source_paint_key.clone();
        let source = app.source_frame.as_ref().map(|frame| frame.data().to_vec());
        let jobs = app
            .app
            .preview_worker
            .as_ref()
            .map(|worker| worker.metrics().unwrap().submitted);
        let bottom = VIEW.height - 5;
        let before = [pixel(&app, 2, bottom), pixel(&app, VIEW.width - 30, bottom)];

        app.notify(Notice::ExportBusy);
        assert!(app.draw_frame(None));

        assert_eq!(
            app.source_paint_key, key,
            "{mode:?}: the Source frame was painted again"
        );
        assert_eq!(
            app.source_frame.as_ref().map(|frame| frame.data().to_vec()),
            source,
            "{mode:?}"
        );
        assert_eq!(
            app.app
                .preview_worker
                .as_ref()
                .map(|worker| worker.metrics().unwrap().submitted),
            jobs,
            "{mode:?}: a notice asked for a Preview job"
        );
        let after = [pixel(&app, 2, bottom), pixel(&app, VIEW.width - 30, bottom)];
        assert_ne!(after[0], before[0], "{mode:?}: no bar on the left");
        assert_ne!(after[1], before[1], "{mode:?}: no bar on the right");
        assert_eq!(after[0], after[1], "{mode:?}: one bar across the window");
        let bar = app.notice_bar_layout().expect("the bar").bar;
        assert_eq!(bar.y, (VIEW.height - 34) as f32);
    }
}

/// P17-N07 in the shell: a button of the bar gives its choice, as its key would; a press on the
/// bar outside the buttons reaches the pane under it.
#[test]
fn a_button_of_the_bar_gives_its_choice() {
    let mut app = shown(ViewMode::Source);
    app.app.persistence.enter_conflict(conflict());
    let layout = app.notice_bar_layout().expect("the waiting conflict");
    let [Some(_), Some((_, keep_local))] = layout.buttons else {
        panic!("two buttons");
    };
    press_at(
        &mut app,
        f64::from(keep_local.x + keep_local.width / 2.0),
        f64::from(keep_local.y + keep_local.height / 2.0),
    );
    assert!(app.app.persistence.keep_local_in_progress());

    let mut app = shown(ViewMode::Source);
    app.app.persistence.enter_conflict(conflict());
    let layout = app.notice_bar_layout().unwrap();
    press_at(&mut app, 5.0, f64::from(layout.bar.y + 5.0));
    assert!(!app.app.persistence.keep_local_in_progress());
    assert!(app.app.persistence.conflict().is_some());
}

/// P17-N11 in the shell: the marker shows only while a save failure is unresolved, and a click on
/// it shows the failure for another 30 s; without a failure its slot moves the window.
#[test]
fn a_click_on_the_marker_shows_the_failure_again() {
    let mut app = shown(ViewMode::Source);
    let slot = app
        .control_layout
        .as_ref()
        .and_then(|layout| layout.fault_marker)
        .expect("the default window has room for the marker");
    let (x, y) = (slot.x + slot.width / 2.0, slot.y + slot.height / 2.0);
    press_at(&mut app, x, y);
    assert!(app.app.timed_notice_texts().is_empty());

    app.app
        .persistence
        .record_save_failure(SaveFaultKind::Failed, "保存失败：disk full".into());
    press_at(&mut app, x, y);
    assert_eq!(app.app.timed_notice_texts(), ["保存失败：disk full"]);
    assert_eq!(
        app.app.notice_view().map(|view| view.tone),
        Some(NoticeTone::Fault)
    );
}

/// The caret's partial redraw would paint over the bar where they meet: there the caret goes with
/// a full frame.
#[test]
fn the_bar_covers_only_its_own_rows() {
    let mut app = shown(ViewMode::Source);
    let row = |y: f32| NoticeRect {
        x: 10.0,
        y,
        width: 2.0,
        height: 20.0,
    };
    assert!(!app.notice_bar_covers(row(VIEW.height as f32 - 25.0)));
    app.notify(Notice::ExportBusy);
    assert!(app.notice_bar_covers(row(VIEW.height as f32 - 25.0)));
    assert!(!app.notice_bar_covers(row(VIEW.height as f32 - 60.0)));
}
