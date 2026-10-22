//! Physical numeric input with each real IME and each underlying input owner.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use super::super::opacity;
use super::*;
use crate::window_control::{self as window, qualification_input as input};

pub(super) fn run(
    repository: &Path,
    program: &Path,
    pid: u32,
    hwnd: WindowHandle,
    ime: &mut RealImeSession<'_>,
    term: &str,
) -> Result<(), String> {
    let baseline = format!("Numeric routing: {term} / {term}");
    replace_document(program, hwnd, &baseline)?;
    // Establish an active Chinese mode before observing disable/re-enable. No
    // profile or IME-control setter is used between field focus and its witness.
    commit_and_undo(program, hwnd, &baseline, "shuxue", ime, "numeric baseline")?;
    let mut previous = 100;
    for owner in ["source", "search", "preview"] {
        for commit in ["enter", "blur"] {
            println!(
                "NUMERIC_NATIVE_STEP ime={} owner={owner} commit={commit}",
                ime.profile_key
            );
            window::focus_source_editor(hwnd)?;
            window::press_document_end(hwnd)?;
            if owner == "search" {
                open_empty_search(hwnd)?;
            } else if owner == "preview" {
                window::press_preview_shortcut(hwnd)?;
                wait_for_config(program, "view_mode = \"preview\"")?;
            }
            opacity::open(hwnd)?;
            input::opacity_field(hwnd)?;
            // If IME remained enabled this would be preedit and Backspace would
            // edit that composition. The subsequent digits must become the
            // integer field, never candidate selection, Source or Search input.
            window::type_ascii_letters(hwnd, "nihao")?;
            for _ in 0..5 {
                window::press_backspace(hwnd)?;
            }
            let value = if commit == "enter" { 82 } else { 64 };
            opacity::digits(hwnd, value)?;
            window::press_save(hwnd)?;
            thread::sleep(IME_COMMIT_SETTLE);
            opacity::committed(program, hwnd, previous)?;
            super::super::diagnostics::step(
                repository,
                program,
                pid,
                &format!("numeric-{owner}-{commit}-pending"),
            )?;
            if commit == "enter" {
                // Four physical key-downs followed by one key-up: repeats must
                // stay with the just-committed field, not add newlines or search.
                input::repeated_enter(hwnd)?;
            } else {
                window::focus_shell_desktop(hwnd)?;
                thread::sleep(IME_COMMIT_SETTLE);
                window::request_window_foreground(hwnd)?;
            }
            opacity::committed(program, hwnd, value)?;
            previous = value;
            thread::sleep(AUTOSAVE_OBSERVATION);
            assert_note(
                program,
                &baseline,
                "numeric input/repeated Enter leaked into Source",
            )?;
            window::press_escape(hwnd)?;
            thread::sleep(IME_COMMIT_SETTLE);
            // Real input, no set_ime_allowed/profile/mode repair: this is the
            // positive witness that the native input owner was restored.
            if owner == "search" {
                direct_commit(hwnd)?;
                window::press_find(hwnd)?;
                copy_is(hwnd, term)?;
            } else {
                if owner == "preview" {
                    window::type_ascii_letters(hwnd, "nihao")?;
                    window::press_space(hwnd)?;
                    thread::sleep(AUTOSAVE_OBSERVATION);
                    assert_note(
                        program,
                        &baseline,
                        "Preview accepted input after numeric commit",
                    )?;
                    window::press_source_shortcut(hwnd)?;
                    wait_for_config(program, "view_mode = \"source\"")?;
                    window::focus_source_editor(hwnd)?;
                    window::press_document_end(hwnd)?;
                }
                direct_commit(hwnd)?;
                wait_note(program, |text| text == format!("{baseline}{term}"))?;
                window::press_undo(hwnd)?;
                wait_note(program, |text| text == baseline)?;
            }
            assert_note(program, &baseline, "numeric route changed canonical text")?;
            ime.record(
                "numeric-native-commit",
                &[("commit", commit), ("owner", owner)],
            );
        }
    }
    composition_refuses_focus(repository, program, pid, hwnd, ime, &baseline, term)?;
    export_priority(repository, program, pid, hwnd, ime, &baseline)?;
    Ok(())
}

fn direct_commit(hwnd: WindowHandle) -> Result<(), String> {
    window::type_ascii_letters(hwnd, "shuxue")?;
    commit_first_candidate(hwnd)
}

fn open_empty_search(hwnd: WindowHandle) -> Result<(), String> {
    window::press_find(hwnd)?;
    // Closing Search deliberately retains its query. These fixtures use short
    // queries; move beyond their end and erase them with real field keystrokes.
    for _ in 0..64 {
        window::press_arrow_right(hwnd)?;
    }
    for _ in 0..64 {
        window::press_backspace(hwnd)?;
    }
    window::press_find(hwnd)?;
    window::focus_source_editor(hwnd)?;
    window::press_document_end(hwnd)?;
    // Negative control: opening this query must not select an existing match.
    // Otherwise copying a stale Source selection could pass even if no new IME
    // input reached Search. Close before copying so Search cannot consume Ctrl+C.
    window::press_find(hwnd)?;
    window::press_find(hwnd)?;
    const SENTINEL: &str = "NUMERIC_NO_SELECTED_MATCH";
    window::set_clipboard_text(SENTINEL)?;
    window::press_copy(hwnd)?;
    thread::sleep(IME_COMMIT_SETTLE);
    if window::clipboard_text()?.is_some_and(|text| !text.is_empty() && text != SENTINEL) {
        return Err(
            "numeric Search fixture already selects a match before its IME witness".to_owned(),
        );
    }
    window::press_find(hwnd)
}

fn copy_is(hwnd: WindowHandle, expected: &str) -> Result<(), String> {
    window::clear_clipboard()?;
    window::press_copy(hwnd)?;
    wait_until(
        "Search query survives numeric routing and accepts restored IME",
        || Ok(window::clipboard_text()?.as_deref() == Some(expected)),
    )
}

fn composition_refuses_focus(
    repository: &Path,
    program: &Path,
    pid: u32,
    hwnd: WindowHandle,
    ime: &mut RealImeSession<'_>,
    baseline: &str,
    term: &str,
) -> Result<(), String> {
    // Native TIP surfaces include invisible hit-test margins. Keep both Source
    // and left-pane Search candidates away from the right toolbar/field before
    // starting either composition; never move or dismiss the active candidate.
    window::resize_to_dip(hwnd, 1200, 680)?;
    for owner in ["source", "search"] {
        println!(
            "NUMERIC_COMPOSITION_STEP ime={} owner={owner}",
            ime.profile_key
        );
        if owner == "search" {
            // Put Search in the left pane before composition begins. In the
            // narrow Source view the TIP's candidate surface covers the opacity
            // field (even its visually exposed top edge). A wide Split fixture
            // leaves the real numeric target on the right, without dismissing
            // candidates or relaxing the native input-target guard.
            window::press_split_shortcut(hwnd)?;
            wait_for_config(program, "view_mode = \"split\"")?;
        }
        window::focus_source_editor(hwnd)?;
        window::press_document_end(hwnd)?;
        if owner == "search" {
            open_empty_search(hwnd)?;
        }
        // Leave the final letter for after the refused focus request. A query
        // committed by the click must not masquerade as a preserved composition.
        begin_preedit(program, hwnd, baseline, "shuxu", ime)?;
        super::super::diagnostics::step(
            repository,
            program,
            pid,
            &format!("numeric-{owner}-composition-before-popup"),
        )?;
        let open = opacity::open(hwnd);
        super::super::diagnostics::step(
            repository,
            program,
            pid,
            &format!("numeric-{owner}-composition-popup"),
        )?;
        open.map_err(|error| format!("{owner} composing popup open: {error}"))?;
        let focus = input::opacity_field(hwnd);
        super::super::diagnostics::step(
            repository,
            program,
            pid,
            &format!("numeric-{owner}-composition-refused"),
        )?;
        focus.map_err(|error| format!("{owner} composing numeric focus: {error}"))?;
        thread::sleep(AUTOSAVE_OBSERVATION);
        assert_note(
            program,
            baseline,
            "numeric focus committed preedit prematurely",
        )?;
        if title_is_dirty(&window::title(hwnd)?) {
            return Err("numeric focus dirtied the note during preedit".to_owned());
        }
        window::type_ascii_letters(hwnd, "e")?;
        commit_first_candidate(hwnd)?;
        // Toggle the popup closed after the candidate commit. Esc while the
        // candidate is active would cancel it in the TIP before app routing.
        opacity::open(hwnd)?;
        if owner == "search" {
            window::press_find(hwnd)?;
            copy_is(hwnd, term)?;
        } else {
            wait_note(program, |text| text == format!("{baseline}{term}"))?;
            window::press_undo(hwnd)?;
            wait_note(program, |text| text == baseline)?;
        }
        ime.record("numeric-composition-refuses-focus", &[("owner", owner)]);
    }
    window::press_source_shortcut(hwnd)?;
    wait_for_config(program, "view_mode = \"source\"")?;
    window::resize_to_dip(hwnd, 520, 680)?;
    Ok(())
}

fn export_priority(
    repository: &Path,
    program: &Path,
    pid: u32,
    hwnd: WindowHandle,
    ime: &mut RealImeSession<'_>,
    baseline: &str,
) -> Result<(), String> {
    window::focus_source_editor(hwnd)?;
    window::press_document_end(hwnd)?;
    opacity::open(hwnd)?;
    input::opacity_field(hwnd)?;
    opacity::digits(hwnd, 79)?;
    let exported = program.join("numeric-export.md");
    window::press_export(hwnd)?;
    super::super::super::super::exact_desktop::invoke_uia(
        repository,
        "export",
        pid,
        Some(&exported),
    )?;
    wait_until(
        "numeric field export publication",
        || Ok(exported.is_file()),
    )?;
    let text = fs::read_to_string(exported).map_err(io_error)?;
    if canonical_line_endings(&text) != baseline {
        return Err(
            "export shortcut captured numeric field text instead of canonical note".to_owned(),
        );
    }
    opacity::committed(program, hwnd, 79)?;
    window::request_window_foreground(hwnd)?;
    window::press_escape(hwnd)?;
    assert_note(program, baseline, "numeric export changed canonical text")?;
    ime.record("numeric-idle-export-priority", &[]);
    Ok(())
}
