//! The toolbar Opacity button closes the popup it finds open, through the copied Release's real
//! window messages (the decision alone is `stickymd-win`'s
//! `the_opacity_button_closes_the_popup_it_finds_open`).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;
use crate::window_control::{ToolbarControl, WindowHandle, click_toolbar, commit_opacity_slider};

/// How long a press that must change nothing is given to change something: a press on an open
/// slider previews the window alpha and its release submits the config write in the same event
/// pass, so either shows well within this.
const SETTLE: Duration = Duration::from_millis(500);

/// Entered and left with the popup open at a committed 100%. A press on the button while the
/// popup is open closes it, so a press where the slider was then lands in the editor and changes
/// nothing: once with nothing edited, once after that press committed digits typed into the field.
/// Assumes the input method types an ASCII digit (an English layout, or a Chinese IME in
/// half-width mode): before D5 the field takes no IME commit, so a full-width digit fails the
/// check, as does a digit that reaches the note.
pub(super) fn verify_button_closes_open_popup(
    program_directory: &Path,
    window: WindowHandle,
) -> Result<(), String> {
    let note_path = program_directory.join("note/note.md");
    let note = fs::read(&note_path).map_err(|error| format!("cannot read the note: {error}"))?;
    click_toolbar(window, ToolbarControl::Opacity)?;
    assert_popup_closed(program_directory, window, 100, None, "a second press")?;

    // Typing needs StickyMD in the foreground, and activating it clicks the editor: do that
    // while the popup is closed.
    crate::window_control::focus_window(window)?;
    click_toolbar(window, ToolbarControl::Opacity)?;
    commit_opacity_slider(window, 70)?;
    wait_for_config_field(program_directory, "opacity = 70")?;
    wait_for_layered_alpha(window, Some(179))?;
    crate::window_control::click_opacity_field(window)?;
    // The field then holds just "4", which commits as 40 (clamped), before and after D5. The
    // press is physical so it follows the digit: a window message would overtake queued input.
    crate::window_control::press_digit(window, 4)?;
    crate::window_control::click_toolbar_physically(window, ToolbarControl::Opacity)?;
    wait_for_config_field(program_directory, "opacity = 40")
        .map_err(|error| format!("the typed 4 was not committed from the field: {error}"))?;
    wait_for_layered_alpha(window, Some(102))?;
    assert_popup_closed(
        program_directory,
        window,
        40,
        Some(102),
        "a second press that committed the field",
    )?;

    click_toolbar(window, ToolbarControl::Opacity)?;
    commit_opacity_slider(window, 100)?;
    wait_for_config_field(program_directory, "opacity = 100")?;
    wait_for_layered_alpha(window, None)?;
    // Wait out the editor's 650 ms autosave debounce, so a digit typed into the note is on disk.
    thread::sleep(Duration::from_millis(1_500));
    if fs::read(&note_path).map_err(|error| format!("cannot reread the note: {error}"))? != note {
        return Err("the opacity popup check changed the note".to_owned());
    }
    runtime_report!("Phase 8 runtime opacity button closed the open popup twice");
    Ok(())
}

/// A press where the slider's 70% point was: with the popup closed it lands in the editor, so
/// neither the window alpha nor the committed opacity changes from `committed` / `alpha`.
fn assert_popup_closed(
    program_directory: &Path,
    window: WindowHandle,
    committed: u8,
    alpha: Option<u8>,
    after: &str,
) -> Result<(), String> {
    commit_opacity_slider(window, 70)?;
    thread::sleep(SETTLE);
    let observed = crate::window_control::layered_alpha(window)?;
    let alpha_kept = match alpha {
        Some(value) => observed.layered && observed.alpha == Some(value),
        None => !observed.layered && observed.alpha.is_none(),
    };
    let config = fs::read_to_string(program_directory.join("note/config.toml"))
        .map_err(|error| format!("cannot read config after {after}: {error}"))?;
    let committed_line = format!("opacity = {committed}");
    let opacity_line = config.lines().find(|line| line.starts_with("opacity ="));
    if alpha_kept && opacity_line == Some(committed_line.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "the opacity popup stayed open after {after}: a press where its slider was left \
             alpha={observed:?} and config {opacity_line:?}, expected alpha {alpha:?} and {committed_line:?}"
        ))
    }
}
