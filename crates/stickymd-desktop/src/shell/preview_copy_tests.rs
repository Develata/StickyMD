//! Ctrl+C in Preview copies by plan 06 #preview-copy-semantics from the shown frame's own
//! snapshot, and a selection that does not map writes nothing.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use stickymd_app::config::ViewMode;
use stickymd_app::instruction::AppIntent;
use stickymd_core::{EditKind, Selection};
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};
use winit::keyboard::KeyCode;

use crate::shell::headless_tests::{HeadlessApp, fixture};

/// Builds a Preview frame of the current document and shows it, selecting `selection`.
fn show_frame(app: &mut HeadlessApp, selection: impl Fn(usize) -> PreviewSelection) {
    app.app
        .config
        .update(|config| config.view_mode = ViewMode::Preview)
        .unwrap();
    let frame = PreviewPipeline::new()
        .build(
            &app.app.coordinator.snapshot(),
            400,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    app.preview_selection = selection(frame.text().len());
    app.preview_frame = Some(frame);
}

fn press_copy(app: &mut HeadlessApp) {
    app.handle_preview_key(Some(KeyCode::KeyC), true, false);
}

/// P17-C07: a frame that lags the document copies its own snapshot's source.
#[test]
fn a_copy_reads_the_shown_frames_snapshot() {
    let mut app = fixture("old **text** here");
    show_frame(&mut app, |end| PreviewSelection {
        anchor: 0,
        active: end,
    });
    let (generation, length) = {
        let view = app.app.coordinator.view();
        (view.generation, view.text.len())
    };
    app.app
        .coordinator
        .dispatch(AppIntent::Edit {
            expected_generation: generation,
            selection: Selection::new(0, length),
            inserted: "new".to_owned(),
            kind: EditKind::SelectionReplace,
            timestamp_ms: 0,
        })
        .unwrap();
    assert_eq!(app.app.coordinator.view().text, "new");

    press_copy(&mut app);

    assert_eq!(
        app.app
            .coordinator
            .read_clipboard_text()
            .unwrap()
            .as_deref(),
        Some("old **text** here")
    );
    assert_eq!(app.app.timed_notice_texts(), ["已复制"]);
}

/// P17-C01 through the shell: a formula alone copies its TeX.
#[test]
fn a_copy_of_one_formula_writes_its_tex() {
    let mut app = fixture("$$x^2$$");
    show_frame(&mut app, |end| PreviewSelection {
        anchor: end,
        active: 0,
    });

    press_copy(&mut app);

    assert_eq!(
        app.app
            .coordinator
            .read_clipboard_text()
            .unwrap()
            .as_deref(),
        Some("x^2")
    );
}

/// P17-C09, P17-C11: a selection that does not map writes nothing and shows a reminder; the
/// document and the selection stay as they were.
#[test]
fn an_unmapped_copy_writes_nothing_and_says_so() {
    let mut app = fixture("é");
    show_frame(&mut app, |_| PreviewSelection {
        anchor: 0,
        active: 1,
    });
    let before = app.app.coordinator.view().generation;

    press_copy(&mut app);

    assert_eq!(app.app.coordinator.read_clipboard_text().unwrap(), None);
    assert_eq!(
        app.app.timed_notice_texts(),
        ["复制失败：选区无法对应到源码，剪贴板未改动。"]
    );
    assert_eq!(app.app.coordinator.view().generation, before);
    assert_eq!(
        app.preview_selection,
        PreviewSelection {
            anchor: 0,
            active: 1,
        }
    );
}
