//! Exact Microsoft Pinyin and WeType functional qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    ChildGuard, io_error, seed_note, wait_for_config, wait_for_layout, wait_note,
};
use crate::window_control::{ImeProfile, ImeProfileGuard, WindowHandle};

const AUTOSAVE_OBSERVATION: Duration = Duration::from_millis(800);
const SOURCE_PREFIX: &str = "StickyMD IME baseline: ";

pub(super) fn g4_06(_repository: &Path, program: &Path) -> Result<(), String> {
    for profile in [ImeProfile::MicrosoftPinyin, ImeProfile::WeType] {
        exercise_profile(program, profile)?;
    }
    Ok(())
}

fn exercise_profile(program: &Path, profile: ImeProfile) -> Result<(), String> {
    seed_note(program, SOURCE_PREFIX)?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_document_end(window)?;

    let profile_guard = ImeProfileGuard::activate(profile)?;
    let term = source_commit_and_undo(program, window, profile)?;
    source_cancel_is_non_mutating(program, window, profile)?;
    selection_commit_is_one_undo(program, window, profile)?;
    search_fields_accept_real_ime(program, window, profile, &term)?;
    runtime_states_accept_real_ime(program, window, child.id(), profile)?;

    child.kill_and_wait()?;
    profile_guard.restore().map_err(|error| {
        format!(
            "{} functional checks passed but the original input profile was not restored: {error}",
            profile.name()
        )
    })
}

fn source_commit_and_undo(
    program: &Path,
    window: WindowHandle,
    profile: ImeProfile,
) -> Result<String, String> {
    begin_preedit(program, window, SOURCE_PREFIX, "zhongguo", profile)?;
    crate::window_control::press_arrow_left(window)?;
    crate::window_control::press_arrow_right(window)?;
    crate::window_control::press_backspace(window)?;
    crate::window_control::type_ascii_letters(window, "o")?;
    crate::window_control::press_enter(window)?;
    wait_note(program, |text| {
        text.strip_prefix(SOURCE_PREFIX)
            .is_some_and(valid_cjk_commit)
    })?;
    let committed = read_note(program)?;
    let term = committed
        .strip_prefix(SOURCE_PREFIX)
        .ok_or_else(|| format!("{} commit replaced the source prefix", profile.name()))?
        .to_owned();

    crate::window_control::press_shift(window)?;
    crate::window_control::type_ascii_letters(window, "rust")?;
    wait_note(program, |text| text == format!("{SOURCE_PREFIX}{term}rust"))?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == committed)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == SOURCE_PREFIX)?;
    Ok(term)
}

fn source_cancel_is_non_mutating(
    program: &Path,
    window: WindowHandle,
    profile: ImeProfile,
) -> Result<(), String> {
    begin_preedit(program, window, SOURCE_PREFIX, "nihao", profile)?;
    crate::window_control::press_escape(window)?;
    thread::sleep(AUTOSAVE_OBSERVATION);
    assert_note(
        program,
        SOURCE_PREFIX,
        "Escape cancel mutated canonical text",
    )
}

fn selection_commit_is_one_undo(
    program: &Path,
    window: WindowHandle,
    profile: ImeProfile,
) -> Result<(), String> {
    const SELECTED: &str = "selection target";
    replace_document(program, window, SELECTED)?;
    crate::window_control::press_select_all(window)?;
    begin_preedit(program, window, SELECTED, "nihao", profile)?;
    crate::window_control::press_enter(window)?;
    wait_note(program, valid_cjk_commit)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == SELECTED).map(|_| ())
}

fn search_fields_accept_real_ime(
    program: &Path,
    window: WindowHandle,
    profile: ImeProfile,
    term: &str,
) -> Result<(), String> {
    let document = format!("{term}\n分隔\n{term}");
    replace_document(program, window, &document)?;

    crate::window_control::press_find(window)?;
    begin_preedit(program, window, &document, "zhongguo", profile)?;
    crate::window_control::press_enter(window)?;
    crate::window_control::press_control_enter(window)?;
    thread::sleep(AUTOSAVE_OBSERVATION);
    assert_note(
        program,
        &document,
        "Find-only accepted a replacement command",
    )?;
    crate::window_control::press_arrow_down(window)?;
    crate::window_control::press_arrow_up(window)?;
    crate::window_control::press_arrow_down(window)?;
    crate::window_control::press_find(window)?;
    crate::window_control::clear_clipboard()?;
    crate::window_control::press_copy(window)?;
    let selected = crate::window_control::clipboard_text()?.unwrap_or_default();
    if selected != term {
        return Err(format!(
            "{} Search IME query did not select its committed source term: expected={term:?} observed={selected:?}",
            profile.name()
        ));
    }

    crate::window_control::press_replace(window)?;
    crate::window_control::press_tab(window)?;
    begin_preedit(program, window, &document, "nihao", profile)?;
    crate::window_control::press_enter(window)?;
    crate::window_control::press_control_enter(window)?;
    wait_note(program, |text| text != document && text.contains(term))?;
    let replaced = read_note(program)?;
    if replaced.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Err(format!(
            "{} replacement field left uncommitted romanization in the document",
            profile.name()
        ));
    }
    crate::window_control::press_find(window)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == document).map(|_| ())
}

fn runtime_states_accept_real_ime(
    program: &Path,
    window: WindowHandle,
    process_id: u32,
    profile: ImeProfile,
) -> Result<(), String> {
    const BASELINE: &str = "真实输入法状态基线";

    replace_document(program, window, BASELINE)?;
    crate::window_control::switch_to_split(window)?;
    wait_for_config(program, "view_mode = \"split\"")?;
    commit_and_undo(program, window, BASELINE, "nihao", profile, "Split")?;

    crate::window_control::switch_to_source(process_id)?;
    wait_for_config(program, "view_mode = \"source\"")?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    crate::window_control::commit_opacity_slider(window, 40)?;
    wait_for_config(program, "opacity = 40")?;
    commit_and_undo(program, window, BASELINE, "shijie", profile, "40% opacity")?;

    crate::window_control::move_to_primary_edge(
        window,
        crate::window_control::PrimaryDockEdge::Left,
    )?;
    wait_for_config(program, "dock_edge = \"left\"")?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_document_end(window)?;
    begin_preedit(program, window, BASELINE, "zhongwen", profile)?;
    super::dock::assert_edge(
        window,
        crate::window_control::PrimaryDockEdge::Left,
        false,
        "real IME typing guard",
    )?;
    crate::window_control::press_enter(window)?;
    wait_note(program, |text| {
        text.strip_prefix(BASELINE).is_some_and(valid_cjk_commit)
    })?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == BASELINE)?;

    begin_preedit(program, window, BASELINE, "nihao", profile)?;
    crate::window_control::focus_shell_desktop(window)?;
    thread::sleep(Duration::from_millis(100));
    crate::window_control::focus_source_editor(window)?;
    replace_document(program, window, BASELINE)?;
    commit_and_undo(program, window, BASELINE, "zaijian", profile, "refocus")
}

fn commit_and_undo(
    program: &Path,
    window: WindowHandle,
    baseline: &str,
    romanization: &str,
    profile: ImeProfile,
    context: &str,
) -> Result<(), String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_document_end(window)?;
    begin_preedit(program, window, baseline, romanization, profile)?;
    crate::window_control::press_enter(window)?;
    wait_note(program, |text| {
        text.strip_prefix(baseline).is_some_and(valid_cjk_commit)
    })?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == baseline)
        .map(|_| ())
        .map_err(|error| format!("{context} IME roundtrip failed: {error}"))
}

fn begin_preedit(
    program: &Path,
    window: WindowHandle,
    durable_before: &str,
    romanization: &str,
    profile: ImeProfile,
) -> Result<(), String> {
    crate::window_control::type_ascii_letters(window, romanization)?;
    thread::sleep(AUTOSAVE_OBSERVATION);
    if read_note(program)? == durable_before {
        return Ok(());
    }

    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == durable_before)?;
    crate::window_control::press_shift(window)?;
    crate::window_control::type_ascii_letters(window, romanization)?;
    thread::sleep(AUTOSAVE_OBSERVATION);
    assert_note(
        program,
        durable_before,
        &format!(
            "{} emitted ordinary ASCII instead of an active composition after one mode correction",
            profile.name()
        ),
    )
}

fn replace_document(program: &Path, window: WindowHandle, text: &str) -> Result<(), String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::set_clipboard_text(text)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_paste(window)?;
    wait_note(program, |observed| observed == text).map(|_| ())
}

fn read_note(program: &Path) -> Result<String, String> {
    fs::read_to_string(program.join("note/note.md")).map_err(io_error)
}

fn assert_note(program: &Path, expected: &str, context: &str) -> Result<(), String> {
    let observed = read_note(program)?;
    if observed == expected {
        Ok(())
    } else {
        Err(format!(
            "{context}: expected={expected:?} observed={observed:?}"
        ))
    }
}

fn valid_cjk_commit(text: &str) -> bool {
    !text.is_empty()
        && text.chars().any(is_cjk)
        && !text.bytes().any(|byte| byte.is_ascii_alphabetic())
}

fn is_cjk(character: char) -> bool {
    matches!(character, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}')
}

#[cfg(test)]
mod tests {
    use super::valid_cjk_commit;

    #[test]
    fn commit_predicate_rejects_romanization_and_accepts_cjk() {
        assert!(valid_cjk_commit("中国"));
        assert!(valid_cjk_commit("你好🙂"));
        assert!(!valid_cjk_commit(""));
        assert!(!valid_cjk_commit("zhongguo"));
        assert!(!valid_cjk_commit("中国a"));
    }
}
