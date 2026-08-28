//! Exact legacy-shortcut and math-conversion qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    ChildGuard, io_error, seed_note, wait_for_config, wait_for_layout, wait_note,
};

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
    0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5, 1, 1,
    39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub(super) fn g4_03(_repository: &Path, program: &Path) -> Result<(), String> {
    const TEXT: &str = "traditional clipboard 中文🙂\nsecond line\n";
    seed_note(program, TEXT)?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::focus_source_editor(window)?;

    crate::window_control::clear_clipboard()?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_ctrl_insert(window)?;
    let copied = crate::window_control::clipboard_text()?.unwrap_or_default();
    if normalize_newlines(&copied) != TEXT {
        return Err("Ctrl+Insert did not copy the selected canonical text".to_owned());
    }
    crate::window_control::press_shift_delete(window)?;
    wait_note(program, str::is_empty)?;
    crate::window_control::press_shift_insert(window)?;
    wait_note(program, |text| text == TEXT)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, str::is_empty)?;
    crate::window_control::press_redo(window)?;
    wait_note(program, |text| text == TEXT)?;

    crate::window_control::switch_to_preview(window)?;
    wait_for_config(program, "view_mode = \"preview\"")?;
    crate::window_control::focus_split_preview(window)?;
    crate::window_control::set_clipboard_text("preview must remain read only")?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_shift_delete(window)?;
    crate::window_control::press_shift_insert(window)?;
    thread::sleep(Duration::from_millis(750));
    if fs::read_to_string(program.join("note/note.md")).map_err(io_error)? != TEXT {
        return Err("traditional edit shortcuts mutated the read-only Preview".to_owned());
    }

    crate::window_control::switch_to_source(child.id())?;
    wait_for_config(program, "view_mode = \"source\"")?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_shift_delete(window)?;
    wait_note(program, str::is_empty)?;

    crate::window_control::set_clipboard_dib()?;
    crate::window_control::press_shift_insert(window)?;
    wait_note(program, |text| text.contains("![](images/stickymd-"))?;
    crate::window_control::press_undo(window)?;
    wait_note(program, str::is_empty)?;

    let file = program.join("traditional-file-drop.png");
    fs::write(&file, PNG).map_err(io_error)?;
    crate::window_control::set_clipboard_file_drop(&[file.as_path()])?;
    crate::window_control::press_shift_insert(window)?;
    wait_note(program, |text| text.contains("![](images/stickymd-"))?;
    crate::window_control::press_undo(window)?;
    wait_note(program, str::is_empty)?;
    child.kill_and_wait()
}

pub(super) fn g4_04(_repository: &Path, program: &Path) -> Result<(), String> {
    const ORIGINAL: &str = concat!(
        "# Math conversion\n\n",
        "Inline \\(x^2+中\\).\n\n",
        "\\[\n\\frac{a}{b} + y\n\\]\n\n",
        "`\\(inline code\\)`\n\n",
        "```text\n\\[fenced literal\\]\n```\n",
    );
    const CONVERTED: &str = concat!(
        "# Math conversion\n\n",
        "Inline $x^2+中$.\n\n",
        "$$\n\\frac{a}{b} + y\n$$\n\n",
        "`\\(inline code\\)`\n\n",
        "```text\n\\[fenced literal\\]\n```\n",
    );

    seed_note(program, ORIGINAL)?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::click_math_conversion(window)?;
    if source_projection(window)? != CONVERTED {
        return Err("math conversion did not immediately refresh Source projection".to_owned());
    }
    wait_note(program, |text| text == CONVERTED)?;
    crate::window_control::press_undo(window)?;
    if source_projection(window)? != ORIGINAL {
        return Err("one Undo did not restore the complete pre-conversion source".to_owned());
    }
    wait_note(program, |text| text == ORIGINAL)?;
    child.kill_and_wait()
}

fn source_projection(window: crate::window_control::WindowHandle) -> Result<String, String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::clear_clipboard()?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_copy(window)?;
    let text = crate::window_control::clipboard_text()?.unwrap_or_default();
    crate::window_control::press_document_end(window)?;
    Ok(normalize_newlines(&text).into_owned())
}

pub(super) fn normalize_newlines(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains("\r\n") {
        std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_newlines;

    #[test]
    fn source_probe_only_normalizes_windows_newlines() {
        assert_eq!(normalize_newlines("a\n"), "a\n");
        assert_eq!(normalize_newlines("a\r"), "a\r");
        assert_eq!(normalize_newlines("a\r\nb\r\n"), "a\nb\n");
    }
}
