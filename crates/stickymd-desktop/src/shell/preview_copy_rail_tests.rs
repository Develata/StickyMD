//! The copy rail through the real pointer handlers of the windowless shell: a click copies the
//! object, a drag does nothing, hovering repaints only on entering and leaving, and a frame that
//! lags the document copies what it shows (P17-R03, R04, R05).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-rail

use stickymd_app::config::ViewMode;
use stickymd_app::instruction::AppIntent;
use stickymd_core::{EditKind, Selection};
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton};
use winit::window::CursorIcon;

use super::presentation::PendingRedraw;
use crate::frame::SoftwareSurface;
use crate::shell::headless_tests::{HeadlessApp, fixture};

const VIEW: PhysicalSize<u32> = PhysicalSize::new(520, 400);

/// A shown Preview window over `text` at DPI 1, with the pointer inside it and a Preview frame of
/// the current document blitted into the pane.
fn shown(text: &str) -> HeadlessApp {
    shown_at(text, 1.0)
}

fn shown_at(text: &str, dpi: f64) -> HeadlessApp {
    let mut app = fixture(text);
    app.app
        .config
        .update(|config| config.view_mode = ViewMode::Preview)
        .unwrap();
    app.host.presents_frames = true;
    app.headless_view = Some((VIEW, dpi));
    app.surface = Some(SoftwareSurface::for_host(VIEW.width, VIEW.height).unwrap());
    app.configure_viewports();
    app.sync_control_layout();
    app.pointer_inside_window = true;
    show_current_frame(&mut app);
    app
}

fn show_current_frame(app: &mut HeadlessApp) {
    let pane = app.view_geometry().unwrap().preview.unwrap();
    let scale = app.headless_view.unwrap().1 as f32;
    let frame = PreviewPipeline::new()
        .build(
            &app.app.coordinator.snapshot(),
            pane.width,
            pane.height,
            scale,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    app.preview_frame = Some(frame);
}

/// The window position of the middle of the first rail, offset by `(dx, dy)`.
fn on_rail(app: &HeadlessApp, dx: f64, dy: f64) -> PhysicalPosition<f64> {
    let pane = app.view_geometry().unwrap().preview.unwrap();
    let frame = app.preview_frame.as_ref().unwrap();
    let rect = frame.copy_rails()[0].rect();
    PhysicalPosition::new(
        f64::from(pane.x) + f64::from(rect.x + rect.width / 2.0) + dx,
        f64::from(pane.y) + f64::from(rect.y + rect.height / 2.0 - frame.scroll_y()) + dy,
    )
}

fn press(app: &mut HeadlessApp, at: PhysicalPosition<f64>) {
    app.handle_cursor_moved(at);
    app.handle_mouse_button(ElementState::Pressed, MouseButton::Left);
}

fn release(app: &mut HeadlessApp) {
    app.handle_mouse_button(ElementState::Released, MouseButton::Left);
}

fn clipboard(app: &mut HeadlessApp) -> Option<String> {
    app.app.coordinator.read_clipboard_text().unwrap()
}

const SELECTED: PreviewSelection = PreviewSelection {
    anchor: 0,
    active: 2,
};

/// P17-R03: a click on the rail copies the whole object, without fences or quote prefixes, and
/// says so; the press never starts a selection, and the selection stays as it was.
#[test]
fn a_click_on_a_rail_copies_its_object_and_leaves_the_selection() {
    let mut app = shown("para\n\n> ```\n> one\n> two\n> ```\n");
    app.preview_selection = SELECTED;
    let at = on_rail(&app, 0.0, 0.0);

    press(&mut app, at);
    assert!(!app.preview_dragging, "a rail press starts no selection");
    assert!(
        app.preview_focused,
        "a press on Preview gives it the keyboard"
    );
    release(&mut app);

    assert_eq!(clipboard(&mut app).as_deref(), Some("one\ntwo\n"));
    assert_eq!(app.app.timed_notice_texts(), ["已复制"]);
    assert_eq!(app.preview_selection, SELECTED);
    assert!(app.copy_rail.press.is_none());
}

/// P17-R03: moving at least 3 DIP during the press makes it a drag, which neither copies nor
/// selects; a smaller move is still a click. At 2x the threshold is 6 pixels.
#[test]
fn a_drag_from_a_rail_neither_copies_nor_selects() {
    for (dpi, jitter, drag) in [(1.0, 2.0, 3.0), (2.0, 5.0, 6.0)] {
        let mut app = shown_at("```\ncode\n```\n", dpi);
        app.preview_selection = SELECTED;

        let at = on_rail(&app, 0.0, 0.0);

        press(&mut app, at);
        let at = on_rail(&app, 0.0, drag);
        app.handle_cursor_moved(at);
        release(&mut app);
        assert_eq!(clipboard(&mut app), None, "{dpi}x: a drag copies nothing");
        assert_eq!(app.preview_selection, SELECTED, "{dpi}x: nor selects");
        assert!(!app.preview_dragging);

        let at = on_rail(&app, 0.0, 0.0);

        press(&mut app, at);
        let at = on_rail(&app, jitter, 0.0);
        app.handle_cursor_moved(at);
        release(&mut app);
        assert_eq!(clipboard(&mut app).as_deref(), Some("code\n"), "{dpi}x");
    }
}

/// P17-R04: entering and leaving a rail each ask for one redraw, moving within it none; the rail
/// is painted brighter while hovered and the cursor is the hand.
#[test]
fn hovering_a_rail_repaints_only_on_entering_and_leaving() {
    let mut app = shown("```\ncode\n```\n");
    let middle = on_rail(&app, 0.0, 0.0);
    let pixel = |app: &HeadlessApp| {
        let pixmap = app.surface.as_ref().unwrap().pixmap();
        pixmap
            .pixel(middle.x as u32, middle.y as u32)
            .map(|color| (color.red(), color.green(), color.blue()))
    };
    let off = on_rail(&app, -40.0, 0.0);
    app.handle_cursor_moved(off);
    app.draw_frame(None);
    let at_rest = pixel(&app);
    app.pending_redraw = PendingRedraw::None;

    app.handle_cursor_moved(off);
    assert_eq!(
        app.pending_redraw,
        PendingRedraw::None,
        "no rail, no redraw"
    );
    app.handle_cursor_moved(middle);
    assert_eq!(app.pending_redraw, PendingRedraw::Full, "entering redraws");
    assert_eq!(app.cursor_at_pointer(), CursorIcon::Pointer);
    app.draw_frame(None);
    let hovered = pixel(&app);
    assert_ne!(hovered, at_rest, "the hovered rail is painted brighter");
    app.pending_redraw = PendingRedraw::None;

    let at = on_rail(&app, 1.0, 3.0);

    app.handle_cursor_moved(at);
    assert_eq!(app.pending_redraw, PendingRedraw::None, "moving within it");

    app.handle_cursor_moved(off);
    assert_eq!(app.pending_redraw, PendingRedraw::Full, "leaving redraws");
    assert_ne!(app.cursor_at_pointer(), CursorIcon::Pointer);
    app.draw_frame(None);
    assert_eq!(pixel(&app), at_rest);
}

/// P17-R05: on a frame that lags the document, a click on a rail copies what that frame shows.
#[test]
fn a_rail_on_a_lagging_frame_copies_what_that_frame_shows() {
    let mut app = shown("```\nold\n```\n");
    let (generation, length) = {
        let view = app.app.coordinator.view();
        (view.generation, view.text.len())
    };
    app.app
        .coordinator
        .dispatch(AppIntent::Edit {
            expected_generation: generation,
            selection: Selection::new(0, length),
            inserted: "```\nnew\n```\n".to_owned(),
            kind: EditKind::SelectionReplace,
            timestamp_ms: 0,
        })
        .unwrap();
    assert_ne!(
        app.preview_frame.as_ref().unwrap().generation(),
        app.app.coordinator.view().generation
    );

    let at = on_rail(&app, 0.0, 0.0);

    press(&mut app, at);
    release(&mut app);

    assert_eq!(clipboard(&mut app).as_deref(), Some("old\n"));
}

/// A frame not blitted into the pane (its size is not the pane's) shows no rails, so none is hit
/// and the press starts a selection as before.
#[test]
fn a_frame_that_is_not_shown_has_no_rail_to_hit() {
    let mut app = shown("```\ncode\n```\n");
    let at = on_rail(&app, 0.0, 0.0);
    app.headless_view = Some((PhysicalSize::new(VIEW.width + 40, VIEW.height), 1.0));

    app.handle_cursor_moved(at);
    assert_eq!(app.copy_rail_under_pointer(), None);
    app.handle_mouse_button(ElementState::Pressed, MouseButton::Left);
    assert!(app.copy_rail.press.is_none());
    release(&mut app);
    assert_eq!(clipboard(&mut app), None);
}
