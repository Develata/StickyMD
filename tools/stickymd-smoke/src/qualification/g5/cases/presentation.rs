//! G5 zoom, opacity, and theme mechanics with candidate-bound screenshots.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{
    CaseEvidence, TIMEOUT, seed_note, tray, wait_for_config, wait_until,
};
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
    for opacity in [40_u8, 70, 96, 100] {
        crate::window_control::click_toolbar_physically(
            window,
            crate::window_control::ToolbarControl::Opacity,
        )?;
        let held = crate::window_control::qualification_input::opacity_drag(window, opacity)?;
        assert_alpha(window, opacity)?;
        held.release()?;
        wait_for_config(program, &format!("opacity = {opacity}"))?;
        crate::window_control::press_escape(window)?;
        support::assert_source_projection(window, FIXTURE)?;
        support::capture(
            repository,
            child.id(),
            "G5-03",
            &format!("opacity-{opacity}"),
            &mut artifacts,
        )?;
    }

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
        "opacity = 100",
        "theme = \"light\"",
    ] {
        if !config.contains(required) {
            return Err(format!("presentation config is missing `{required}`"));
        }
    }
    // Persist non-default preferences, exit through the real save barrier, then reopen.
    crate::window_control::qualification_input::control_wheel(window, 15)?;
    wait_for_config(program, "content_zoom_percent = 175")?;
    crate::window_control::click_toolbar_physically(
        window,
        crate::window_control::ToolbarControl::Opacity,
    )?;
    let held = crate::window_control::qualification_input::opacity_drag(window, 70)?;
    held.release()?;
    wait_for_config(program, "opacity = 70")?;
    crate::window_control::press_escape(window)?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Theme)?;
    wait_for_config(program, "theme = \"system\"")?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Theme)?;
    wait_for_config(program, "theme = \"dark\"")?;
    tray::exit(repository, child.id())?;
    child.wait_for_exit(TIMEOUT)?;
    let (mut restarted, restored) = support::start_ready(program)?;
    for required in [
        "content_zoom_percent = 175",
        "opacity = 70",
        "theme = \"dark\"",
    ] {
        wait_for_config(program, required)?;
    }
    assert_alpha(restored, 70)?;
    support::assert_source_projection(restored, FIXTURE)?;
    // These relative actions prove the loaded runtime preferences, not merely file contents.
    crate::window_control::qualification_input::control_wheel(restored, 1)?;
    wait_for_config(program, "content_zoom_percent = 180")?;
    crate::window_control::click_toolbar(restored, crate::window_control::ToolbarControl::Theme)?;
    wait_for_config(program, "theme = \"light\"")?;
    if fs::read_to_string(program.join("note/note.md")).map_err(|e| e.to_string())? != FIXTURE {
        return Err("presentation preferences changed canonical Markdown".to_owned());
    }
    support::capture(
        repository,
        restarted.id(),
        "G5-03",
        "preferences-restored",
        &mut artifacts,
    )?;
    restarted.kill_and_wait()?;
    Ok(CaseEvidence {
        artifacts,
        ..CaseEvidence::default()
    })
}

fn exercise_zoom(
    repository: &Path,
    program: &Path,
    process_id: u32,
    window: crate::window_control::WindowHandle,
    mode: &str,
    artifacts: &mut Vec<super::super::super::exact_desktop::ArtifactEvidence>,
) -> Result<(), String> {
    let geometry = crate::window_control::window_rect(window)?;
    crate::window_control::press_zoom_reset(window)?;
    wait_for_config(program, "content_zoom_percent = 100")?;
    crate::window_control::qualification_input::control_wheel(window, 1)?;
    wait_for_config(program, "content_zoom_percent = 105")?;
    crate::window_control::qualification_input::control_wheel(window, -1)?;
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
    crate::window_control::press_zoom_out(window)?;
    crate::window_control::qualification_input::control_wheel(window, -1)?;
    wait_for_config(program, "content_zoom_percent = 50")?;
    crate::window_control::press_zoom_in(window)?;
    wait_for_config(program, "content_zoom_percent = 60")?;
    crate::window_control::press_zoom_out(window)?;
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
    crate::window_control::press_zoom_in(window)?;
    crate::window_control::qualification_input::control_wheel(window, 1)?;
    wait_for_config(program, "content_zoom_percent = 300")?;
    crate::window_control::press_zoom_out(window)?;
    wait_for_config(program, "content_zoom_percent = 290")?;
    crate::window_control::press_zoom_in(window)?;
    wait_for_config(program, "content_zoom_percent = 300")?;
    support::capture(
        repository,
        process_id,
        "G5-03",
        &format!("{mode}-zoom-300"),
        artifacts,
    )?;
    crate::window_control::press_zoom_reset(window)?;
    wait_for_config(program, "content_zoom_percent = 100")?;
    if crate::window_control::window_rect(window)? != geometry {
        return Err(format!(
            "{mode} content zoom changed native window geometry"
        ));
    }
    if fs::read_to_string(program.join("note/note.md")).map_err(|e| e.to_string())? != FIXTURE {
        return Err(format!("{mode} content zoom changed canonical Markdown"));
    }
    Ok(())
}

fn assert_alpha(window: crate::window_control::WindowHandle, opacity: u8) -> Result<(), String> {
    wait_until(&format!("native whole-window opacity {opacity}"), || {
        let facts = crate::window_control::layered_alpha(window)?;
        let alpha = (f64::from(opacity) * 255.0 / 100.0).round() as u8;
        Ok(if opacity == 100 {
            !facts.layered && facts.alpha.is_none()
        } else {
            facts.layered && facts.alpha == Some(alpha)
        })
    })
}
