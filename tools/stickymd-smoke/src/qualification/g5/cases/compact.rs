//! G5 compact Source/Preview/Split mechanics and screenshot evidence.
//!
//! plan_ref: docs/plan/09_windows_shell.md#window-geometry

use std::path::Path;

use super::super::super::exact_desktop::{CaseEvidence, seed_note, wait_note};
use super::support;

const FIXTURE: &str = concat!(
    "# Compact exact probe\n\n",
    "Selectable compact preview text.\n\n",
    "[Allowed link](https://example.com)\n\n",
    "- one\n- two\n- three\n\n",
    "Compact tail sentinel.\n",
);

pub(super) fn run(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    seed_note(program, FIXTURE)?;
    let (mut child, window) = support::start_ready(program)?;
    crate::window_control::resize_to_dip(window, 220, 120)?;
    support::assert_source_projection(window, FIXTURE)?;
    crate::window_control::set_clipboard_text("COMPACT_SOURCE_INPUT\n")?;
    crate::window_control::press_paste(window)?;
    let expected = format!("{FIXTURE}COMPACT_SOURCE_INPUT\n");
    wait_note(program, |text| text == expected)?;

    let mut artifacts = Vec::new();
    support::capture(
        repository,
        child.id(),
        "G5-02",
        "source-220x120",
        &mut artifacts,
    )?;

    support::switch_preview(window, program)?;
    support::assert_preview_projection(window, &["Compact exact probe", "Compact tail sentinel"])?;
    crate::window_control::scroll_preview_down(window, 8)?;
    support::capture(
        repository,
        child.id(),
        "G5-02",
        "preview-compact-bottom",
        &mut artifacts,
    )?;

    support::switch_split(window, program)?;
    support::assert_source_projection(window, &expected)?;
    support::assert_preview_projection(window, &["Compact exact probe", "COMPACT_SOURCE_INPUT"])?;
    crate::window_control::scroll_preview_down(window, 8)?;
    let rect = crate::window_control::window_rect(window)?;
    if rect.width < 220 || rect.height < 120 {
        return Err(format!("Split compact geometry became unusable: {rect:?}"));
    }
    support::capture(
        repository,
        child.id(),
        "G5-02",
        "split-compact",
        &mut artifacts,
    )?;
    support::switch_source(child.id(), program)?;
    support::assert_source_projection(window, &expected)?;
    child.kill_and_wait()?;
    Ok(CaseEvidence { artifacts })
}
