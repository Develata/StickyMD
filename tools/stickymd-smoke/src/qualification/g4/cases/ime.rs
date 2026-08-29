//! Exact Microsoft Pinyin and WeType functional qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::borrow::Cow;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    ChildGuard, copy_directory, io_error, seed_note, wait_for_config, wait_for_layout,
    wait_note as wait_raw_note,
};
use crate::window_control::{ImeProfile, ImeProfileGuard, WindowHandle};

const AUTOSAVE_OBSERVATION: Duration = Duration::from_millis(800);
const INPUT_PROJECTION_OBSERVATION: Duration = Duration::from_millis(600);
const IME_COMMIT_SETTLE: Duration = Duration::from_millis(100);
const SOURCE_PREFIX: &str = "StickyMD IME baseline: ";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrdinaryInputRecovery {
    PhysicalModeCorrection,
    ProfileReassertion,
}

impl OrdinaryInputRecovery {
    const fn for_session(composition_confirmed: bool) -> Self {
        if composition_confirmed {
            Self::ProfileReassertion
        } else {
            Self::PhysicalModeCorrection
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::PhysicalModeCorrection => "physical input mode correction",
            Self::ProfileReassertion => "profile reassertion",
        }
    }
}

struct RealImeSession<'guard> {
    profile: ImeProfile,
    profile_guard: &'guard ImeProfileGuard,
    window: WindowHandle,
    mode_corrected: bool,
    composition_confirmed: bool,
}

impl<'guard> RealImeSession<'guard> {
    fn new(
        profile: ImeProfile,
        profile_guard: &'guard ImeProfileGuard,
        window: WindowHandle,
    ) -> Self {
        Self {
            profile,
            profile_guard,
            window,
            mode_corrected: false,
            composition_confirmed: false,
        }
    }

    fn correct_input_mode(&mut self) -> Result<(), String> {
        if self.mode_corrected {
            return Err(format!(
                "{} returned to ordinary ASCII after its physical input mode was already corrected",
                self.profile.name()
            ));
        }
        crate::window_control::press_shift(self.window)?;
        self.mode_corrected = true;
        Ok(())
    }

    fn restore_input_mode(&mut self) -> Result<(), String> {
        if !self.mode_corrected {
            return Ok(());
        }
        crate::window_control::focus_source_editor(self.window)?;
        crate::window_control::press_escape(self.window)?;
        crate::window_control::press_shift(self.window)?;
        self.mode_corrected = false;
        Ok(())
    }

    fn recover_after_ordinary_input(&mut self) -> Result<OrdinaryInputRecovery, String> {
        let recovery = OrdinaryInputRecovery::for_session(self.composition_confirmed);
        match recovery {
            OrdinaryInputRecovery::PhysicalModeCorrection => self.correct_input_mode()?,
            OrdinaryInputRecovery::ProfileReassertion => {
                self.profile_guard.reassert(self.window)?;
            }
        }
        Ok(recovery)
    }

    fn confirm_composition(&mut self) {
        self.composition_confirmed = true;
    }
}

impl Drop for RealImeSession<'_> {
    fn drop(&mut self) {
        let _ = self.restore_input_mode();
    }
}

pub(super) fn g4_06(_repository: &Path, program: &Path) -> Result<(), String> {
    let parent = program.parent().ok_or_else(|| {
        format!(
            "G4-06 program directory has no parent: {}",
            program.display()
        )
    })?;
    for (profile, suffix) in [
        (ImeProfile::MicrosoftPinyin, "microsoft-pinyin"),
        (ImeProfile::WeType, "wetype"),
    ] {
        // Each profile owns a fresh portable directory. The first profile's
        // 40% opacity and left-dock coverage must not become the second
        // profile's startup state or move its physical input target off-screen.
        let profile_program = parent.join(format!("g4-06-{suffix}"));
        copy_directory(program, &profile_program)?;
        exercise_profile(&profile_program, profile)?;
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

    crate::window_control::focus_shell_desktop(window)?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_document_end(window)?;
    let profile_guard = ImeProfileGuard::activate(profile, window)?;
    let mut ime = RealImeSession::new(profile, &profile_guard, window);
    let checks = (|| {
        crate::window_control::set_ime_open_status(window, true)?;
        crate::window_control::set_ime_native_mode(window, true)?;
        let term = source_commit_and_undo(program, window, &mut ime)
            .map_err(|error| format!("source commit/Undo: {error}"))?;
        source_cancel_is_non_mutating(program, window, &mut ime)
            .map_err(|error| format!("source cancel: {error}"))?;
        selection_commit_is_one_undo(program, window, &mut ime)
            .map_err(|error| format!("selection commit/Undo: {error}"))?;
        search_fields_accept_real_ime(program, window, &mut ime, &term)
            .map_err(|error| format!("Search query/replacement: {error}"))?;
        runtime_states_accept_real_ime(program, window, child.id(), &mut ime)
            .map_err(|error| format!("runtime state matrix: {error}"))
    })();

    let mode_restore = ime.restore_input_mode();
    drop(ime);
    let child_cleanup = child.kill_and_wait();
    let profile_restore = profile_guard.restore();
    combine_profile_results(
        profile,
        checks,
        mode_restore,
        child_cleanup,
        profile_restore,
    )
}

fn combine_profile_results(
    profile: ImeProfile,
    checks: Result<(), String>,
    mode_restore: Result<(), String>,
    child_cleanup: Result<(), String>,
    profile_restore: Result<(), String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    if let Err(error) = checks {
        failures.push(error);
    }
    if let Err(error) = mode_restore {
        failures.push(format!("physical input mode restore: {error}"));
    }
    if let Err(error) = child_cleanup {
        failures.push(format!("StickyMD cleanup: {error}"));
    }
    if let Err(error) = profile_restore {
        failures.push(format!("original input profile restore: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("{}: {}", profile.name(), failures.join("; ")))
    }
}

fn source_commit_and_undo(
    program: &Path,
    window: WindowHandle,
    ime: &mut RealImeSession<'_>,
) -> Result<String, String> {
    begin_preedit(program, window, SOURCE_PREFIX, "zhongguo", ime)?;
    crate::window_control::press_arrow_left(window)?;
    crate::window_control::press_arrow_right(window)?;
    crate::window_control::press_backspace(window)?;
    crate::window_control::type_ascii_letters(window, "o")?;
    commit_first_candidate(window)?;
    wait_note(program, |text| {
        text.strip_prefix(SOURCE_PREFIX)
            .is_some_and(valid_cjk_commit)
    })
    .map_err(|error| format!("commit did not produce durable CJK text: {error}"))?;
    let committed = read_note(program)?;
    let term = committed
        .strip_prefix(SOURCE_PREFIX)
        .ok_or_else(|| format!("{} commit replaced the source prefix", ime.profile.name()))?
        .to_owned();

    type_mixed_ascii(program, window, &format!("{SOURCE_PREFIX}{term}"))?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == committed)
        .map_err(|error| format!("mixed ASCII Undo did not restore CJK commit: {error}"))?;
    crate::window_control::set_ime_native_mode(window, true)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == SOURCE_PREFIX)
        .map_err(|error| format!("IME commit Undo did not restore source prefix: {error}"))?;
    capture_search_fixture_term(program, window, ime)
}

fn capture_search_fixture_term(
    program: &Path,
    window: WindowHandle,
    ime: &mut RealImeSession<'_>,
) -> Result<String, String> {
    begin_preedit(program, window, SOURCE_PREFIX, "zhongguo", ime)?;
    commit_first_candidate(window)?;
    wait_note(program, |text| {
        text.strip_prefix(SOURCE_PREFIX)
            .is_some_and(valid_cjk_commit)
    })
    .map_err(|error| format!("Search fixture commit did not produce durable CJK text: {error}"))?;
    let committed = read_note(program)?;
    let term = committed
        .strip_prefix(SOURCE_PREFIX)
        .ok_or_else(|| {
            format!(
                "{} Search fixture replaced the source prefix",
                ime.profile.name()
            )
        })?
        .to_owned();
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == SOURCE_PREFIX)
        .map_err(|error| format!("Search fixture Undo did not restore source prefix: {error}"))?;
    Ok(term)
}

fn type_mixed_ascii(
    program: &Path,
    window: WindowHandle,
    committed_prefix: &str,
) -> Result<(), String> {
    let expected = format!("{committed_prefix}rust");
    crate::window_control::set_ime_open_status(window, true)?;
    crate::window_control::set_ime_native_mode(window, true)?;
    crate::window_control::type_ascii_letters(window, "rust")?;
    if !wait_for_dirty_projection(window)? {
        // Some Simplified-Chinese TIPs keep alphabetic input in composition
        // while native mode is active. Enter is the user-level operation that
        // commits that raw romanization; unlike a candidate-selection Space,
        // it deterministically yields `rust`.
        crate::window_control::press_enter(window)?;
        thread::sleep(IME_COMMIT_SETTLE);
    }
    wait_note(program, |text| text == expected)
        .map(|_| ())
        .map_err(|error| format!("mixed ASCII segment did not become durable: {error}"))
}

fn source_cancel_is_non_mutating(
    program: &Path,
    window: WindowHandle,
    ime: &mut RealImeSession<'_>,
) -> Result<(), String> {
    begin_preedit(program, window, SOURCE_PREFIX, "nihao", ime)?;
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
    ime: &mut RealImeSession<'_>,
) -> Result<(), String> {
    const SELECTED: &str = "selection target";
    replace_document(program, window, SELECTED)?;
    crate::window_control::press_select_all(window)?;
    begin_preedit(program, window, SELECTED, "nihao", ime)?;
    commit_first_candidate(window)?;
    wait_note(program, valid_cjk_commit)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == SELECTED).map(|_| ())
}

fn search_fields_accept_real_ime(
    program: &Path,
    window: WindowHandle,
    ime: &mut RealImeSession<'_>,
    term: &str,
) -> Result<(), String> {
    let document = format!("{term}\n分隔\n{term}");
    replace_document(program, window, &document)?;

    crate::window_control::press_find(window)?;
    begin_preedit(program, window, &document, "zhongguo", ime)?;
    commit_first_candidate(window)?;
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
            ime.profile.name()
        ));
    }

    crate::window_control::press_replace(window)?;
    begin_preedit(program, window, &document, "nihao", ime)?;
    commit_first_candidate(window)?;
    crate::window_control::press_control_enter(window)?;
    wait_note(program, |text| text != document && text.contains(term))?;
    let replaced = read_note(program)?;
    if replaced.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Err(format!(
            "{} replacement field left uncommitted romanization in the document",
            ime.profile.name()
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
    ime: &mut RealImeSession<'_>,
) -> Result<(), String> {
    const BASELINE: &str = "真实输入法状态基线";

    replace_document(program, window, BASELINE)?;
    crate::window_control::switch_to_split(window)?;
    wait_for_config(program, "view_mode = \"split\"")?;
    commit_and_undo(program, window, BASELINE, "nihao", ime, "Split")?;

    crate::window_control::switch_to_source(process_id)?;
    wait_for_config(program, "view_mode = \"source\"")?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    crate::window_control::commit_opacity_slider(window, 40)?;
    wait_for_config(program, "opacity = 40")?;
    commit_and_undo(program, window, BASELINE, "shijie", ime, "40% opacity")?;

    crate::window_control::move_to_primary_edge(
        window,
        crate::window_control::PrimaryDockEdge::Left,
    )?;
    wait_for_config(program, "dock_edge = \"left\"")?;
    crate::window_control::focus_source_editor(window)
        .map_err(|error| format!("left-dock focus: {error}"))?;
    crate::window_control::press_document_end(window)?;
    begin_preedit(program, window, BASELINE, "zhongwen", ime)
        .map_err(|error| format!("left-dock preedit: {error}"))?;
    super::dock::assert_edge(
        window,
        crate::window_control::PrimaryDockEdge::Left,
        false,
        "real IME typing guard",
    )?;
    commit_first_candidate(window)?;
    wait_note(program, |text| {
        text.strip_prefix(BASELINE).is_some_and(valid_cjk_commit)
    })?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == BASELINE)?;

    crate::window_control::move_to_primary_floating(window)?;
    wait_for_config(program, "dock_edge = \"none\"")?;
    begin_preedit(program, window, BASELINE, "nihao", ime)
        .map_err(|error| format!("pre-refocus composition: {error}"))?;
    crate::window_control::focus_shell_desktop(window)?;
    // GetForegroundWindow proves the native focus owner changed, but winit's
    // target event loop has no cross-process acknowledgement for reducing the
    // paired Focused(false) and set_ime_allowed(false) transition. Keep one
    // bounded dispatch interval before requesting focus back; this fixture is
    // floating, so the 700 ms dock-collapse contract cannot race this wait.
    thread::sleep(Duration::from_millis(100));
    crate::window_control::request_window_foreground(window)
        .map_err(|error| format!("post-composition native refocus: {error}"))?;
    crate::window_control::focus_source_editor(window)
        .map_err(|error| format!("post-composition refocus: {error}"))?;
    assert_note(
        program,
        BASELINE,
        "focus loss committed or discarded the active preedit into canonical text",
    )?;
    crate::window_control::press_document_end(window)?;
    begin_preedit(program, window, BASELINE, "zaijian", ime)
        .map_err(|error| format!("refocus preedit: {error}"))?;
    commit_first_candidate(window)?;
    wait_note(program, |text| {
        text.strip_prefix(BASELINE).is_some_and(valid_cjk_commit)
    })?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == BASELINE).map(|_| ())
}

fn commit_and_undo(
    program: &Path,
    window: WindowHandle,
    baseline: &str,
    romanization: &str,
    ime: &mut RealImeSession<'_>,
    context: &str,
) -> Result<(), String> {
    crate::window_control::focus_source_editor(window)
        .map_err(|error| format!("{context} focus: {error}"))?;
    crate::window_control::press_document_end(window)?;
    begin_preedit(program, window, baseline, romanization, ime)
        .map_err(|error| format!("{context} preedit: {error}"))?;
    commit_first_candidate(window)?;
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
    ime: &mut RealImeSession<'_>,
) -> Result<(), String> {
    let _ = crate::window_control::focus_window(window)?;
    ime.profile_guard.reassert(window)?;
    crate::window_control::set_ime_open_status(window, true)?;
    crate::window_control::set_ime_native_mode(window, true)?;
    crate::window_control::type_ascii_letters(window, romanization)?;
    let ordinary_edit = wait_for_dirty_projection(window)?;
    if !ordinary_edit {
        thread::sleep(AUTOSAVE_OBSERVATION.saturating_sub(INPUT_PROJECTION_OBSERVATION));
        assert_note(
            program,
            durable_before,
            &format!(
                "{} preedit crossed into durable text before commit",
                ime.profile.name()
            ),
        )?;
        ime.confirm_composition();
        return Ok(());
    }

    // A newly activated TIP can consume the tail of the first key sequence as
    // composition after committing its leading key as ordinary text. Cancel
    // that residual composition before Ctrl+Z; otherwise the IME, rather than
    // DocumentState, consumes the recovery shortcut.
    crate::window_control::press_escape(window)?;
    thread::sleep(Duration::from_millis(100));
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == durable_before).map_err(|error| {
        format!(
            "{} could not roll back ordinary text emitted while opening composition: {error}",
            ime.profile.name()
        )
    })?;
    let recovery = ime.recover_after_ordinary_input()?.label();
    crate::window_control::type_ascii_letters(window, romanization)?;
    if wait_for_dirty_projection(window)? {
        crate::window_control::press_escape(window)?;
        crate::window_control::press_undo(window)?;
        let _ = wait_note(program, |text| text == durable_before);
        return Err(format!(
            "{} emitted ordinary ASCII instead of an active composition after one {recovery}",
            ime.profile.name(),
        ));
    }
    thread::sleep(AUTOSAVE_OBSERVATION.saturating_sub(INPUT_PROJECTION_OBSERVATION));
    assert_note(
        program,
        durable_before,
        &format!(
            "{} preedit crossed into durable text after {recovery}",
            ime.profile.name(),
        ),
    )?;
    ime.confirm_composition();
    Ok(())
}

fn commit_first_candidate(window: WindowHandle) -> Result<(), String> {
    crate::window_control::press_space(window)?;
    // The physical key returns before the target event loop has necessarily
    // reduced the asynchronous Ime::Commit into its Source/Search session.
    // There is no cross-process projection of that private state, so keep one
    // narrow bounded settle here rather than retrying a non-idempotent action.
    thread::sleep(IME_COMMIT_SETTLE);
    Ok(())
}

fn wait_for_dirty_projection(window: WindowHandle) -> Result<bool, String> {
    let deadline = std::time::Instant::now() + INPUT_PROJECTION_OBSERVATION;
    loop {
        let title = crate::window_control::title(window)?;
        if title_is_dirty(&title) {
            return Ok(true);
        }
        if title != "StickyMD" {
            return Err(format!(
                "unexpected StickyMD title while classifying IME input: {title:?}"
            ));
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn title_is_dirty(title: &str) -> bool {
    title == "StickyMD *" || title.starts_with("StickyMD * — ")
}

fn replace_document(program: &Path, window: WindowHandle, text: &str) -> Result<(), String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::set_clipboard_text(text)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_paste(window)?;
    wait_note(program, |observed| observed == text).map(|_| ())
}

fn read_note(program: &Path) -> Result<String, String> {
    fs::read_to_string(program.join("note/note.md"))
        .map(|text| canonical_line_endings(&text).into_owned())
        .map_err(io_error)
}

fn wait_note(program: &Path, accepted: impl Fn(&str) -> bool) -> Result<String, String> {
    wait_raw_note(program, |text| accepted(&canonical_line_endings(text)))
        .map(|text| canonical_line_endings(&text).into_owned())
}

fn canonical_line_endings(text: &str) -> Cow<'_, str> {
    if text.contains("\r\n") {
        Cow::Owned(text.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(text)
    }
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
    use super::{
        ImeProfile, OrdinaryInputRecovery, canonical_line_endings, combine_profile_results,
        title_is_dirty, valid_cjk_commit,
    };

    #[test]
    fn commit_predicate_rejects_romanization_and_accepts_cjk() {
        assert!(valid_cjk_commit("中国"));
        assert!(valid_cjk_commit("你好🙂"));
        assert!(!valid_cjk_commit(""));
        assert!(!valid_cjk_commit("zhongguo"));
        assert!(!valid_cjk_commit("中国a"));
    }

    #[test]
    fn profile_failure_reports_all_cleanup_and_restore_failures_together() {
        let error = combine_profile_results(
            ImeProfile::MicrosoftPinyin,
            Err("functional failure".to_owned()),
            Err("mode restore failure".to_owned()),
            Err("child failure".to_owned()),
            Err("restore failure".to_owned()),
        )
        .expect_err("combined failures must fail");
        assert!(error.contains("functional failure"));
        assert!(error.contains("mode restore failure"));
        assert!(error.contains("child failure"));
        assert!(error.contains("restore failure"));
    }

    #[test]
    fn ime_projection_classifier_distinguishes_canonical_dirty_from_preedit() {
        assert!(title_is_dirty("StickyMD *"));
        assert!(title_is_dirty("StickyMD * — 外部修改冲突"));
        assert!(!title_is_dirty("StickyMD"));
    }

    #[test]
    fn durable_note_probe_normalizes_only_windows_line_endings() {
        assert_eq!(canonical_line_endings("a\r\nb\r"), "a\nb\r");
        assert_eq!(canonical_line_endings("a\nb"), "a\nb");
    }

    #[test]
    fn ordinary_input_recovery_never_toggles_after_composition_was_proven() {
        assert_eq!(
            OrdinaryInputRecovery::for_session(false),
            OrdinaryInputRecovery::PhysicalModeCorrection
        );
        assert_eq!(
            OrdinaryInputRecovery::for_session(true),
            OrdinaryInputRecovery::ProfileReassertion
        );
    }
}
