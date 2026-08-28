//! G5 zoom, opacity, and theme mechanics with candidate-bound screenshots.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{CaseEvidence, seed_note, wait_for_config};
use super::support;

const FIXTURE: &str = concat!(
    "# Presentation exact probe\n\n",
    "中文 Latin **bold** and $x^2+y^2=1$.\n\n",
    "- zoom source\n- zoom preview\n- zoom split\n",
);

pub(super) fn run(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    seed_note(program, FIXTURE)?;
    let (mut child, window) = support::start_ready(program)?;
    let mut artifacts = Vec::new();

    support::assert_source_projection(window, FIXTURE)?;
    exercise_zoom(
        repository,
        program,
        child.id(),
        window,
        "source",
        &mut artifacts,
    )?;
    support::switch_preview(window, program)?;
    support::assert_preview_projection(window, &["Presentation exact probe", "x^2+y^2=1"])?;
    exercise_zoom(
        repository,
        program,
        child.id(),
        window,
        "preview",
        &mut artifacts,
    )?;
    support::switch_split(window, program)?;
    support::assert_source_projection(window, FIXTURE)?;
    support::assert_preview_projection(window, &["Presentation exact probe"])?;
    exercise_zoom(
        repository,
        program,
        child.id(),
        window,
        "split",
        &mut artifacts,
    )?;

    support::switch_source(child.id(), program)?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    crate::window_control::commit_opacity_slider(window, 40)?;
    wait_for_config(program, "opacity = 40")?;
    let alpha = crate::window_control::layered_alpha(window)?;
    if !alpha.layered || alpha.alpha.is_none_or(|value| value.abs_diff(102) > 1) {
        return Err(format!(
            "opacity 40 did not reach whole-window alpha 102: {alpha:?}"
        ));
    }
    support::assert_source_projection(window, FIXTURE)?;
    support::capture(
        repository,
        child.id(),
        "G5-03",
        "opacity-40",
        &mut artifacts,
    )?;

    for expected in ["system", "dark", "light"] {
        crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Theme)?;
        wait_for_config(program, &format!("theme = \"{expected}\""))?;
        support::capture(
            repository,
            child.id(),
            "G5-03",
            &format!("theme-{expected}"),
            &mut artifacts,
        )?;
    }
    let config = fs::read_to_string(program.join("note/config.toml"))
        .map_err(|error| format!("cannot inspect presentation config: {error}"))?;
    for required in [
        "content_zoom_percent = 100",
        "opacity = 40",
        "theme = \"light\"",
    ] {
        if !config.contains(required) {
            return Err(format!("presentation config is missing `{required}`"));
        }
    }
    child.kill_and_wait()?;
    Ok(CaseEvidence { artifacts })
}

fn exercise_zoom(
    repository: &Path,
    program: &Path,
    process_id: u32,
    window: crate::window_control::WindowHandle,
    mode: &str,
    artifacts: &mut Vec<super::super::super::exact_desktop::ArtifactEvidence>,
) -> Result<(), String> {
    crate::window_control::press_zoom_reset(window)?;
    wait_for_config(program, "content_zoom_percent = 100")?;
    support::capture(
        repository,
        process_id,
        "G5-03",
        &format!("{mode}-zoom-100"),
        artifacts,
    )?;
    for _ in 0..5 {
        crate::window_control::press_zoom_out(window)?;
    }
    wait_for_config(program, "content_zoom_percent = 50")?;
    support::capture(
        repository,
        process_id,
        "G5-03",
        &format!("{mode}-zoom-50"),
        artifacts,
    )?;
    for _ in 0..25 {
        crate::window_control::press_zoom_in(window)?;
    }
    wait_for_config(program, "content_zoom_percent = 300")?;
    support::capture(
        repository,
        process_id,
        "G5-03",
        &format!("{mode}-zoom-300"),
        artifacts,
    )?;
    crate::window_control::press_zoom_reset(window)?;
    wait_for_config(program, "content_zoom_percent = 100")
}
