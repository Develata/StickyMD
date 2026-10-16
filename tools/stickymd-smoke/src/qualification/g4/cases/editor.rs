//! Exact legacy-shortcut and math-conversion qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    Checks, ChildGuard, io_error, seed_note, wait_for_config, wait_for_layout, wait_note,
    wait_until,
};

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
    0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5, 1, 1,
    39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub(super) fn g4_03(_repository: &Path, program: &Path, checks: &mut Checks) -> Result<(), String> {
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
    checks.record("ctrl-insert-copies-selection", &[]);
    // Replace the copied text so only a real cut can put it back on the clipboard.
    crate::window_control::set_clipboard_text("clipboard before Shift+Delete")?;
    crate::window_control::press_shift_delete(window)?;
    wait_note(program, str::is_empty)?;
    let cut = crate::window_control::clipboard_text()?.unwrap_or_default();
    if normalize_newlines(&cut) != TEXT {
        return Err("Shift+Delete deleted the selection without cutting it".to_owned());
    }
    checks.record("shift-delete-cuts", &[]);
    crate::window_control::press_shift_insert(window)?;
    wait_note(program, |text| text == TEXT)?;
    checks.record("shift-insert-pastes", &[]);
    crate::window_control::press_undo(window)?;
    wait_note(program, str::is_empty)?;
    checks.record("traditional-undo", &[]);
    crate::window_control::press_redo(window)?;
    wait_note(program, |text| text == TEXT)?;
    checks.record("traditional-redo", &[]);

    crate::window_control::switch_to_preview(window)?;
    wait_for_config(program, "view_mode = \"preview\"")?;
    crate::window_control::focus_split_preview(window)?;
    // The Preview must have a frame and a non-empty selection; otherwise an illegal cut would
    // have nothing to cut and pass vacuously.
    wait_until("Preview whole-document selection", || {
        crate::window_control::clear_clipboard()?;
        crate::window_control::press_select_all(window)?;
        crate::window_control::press_ctrl_insert(window)?;
        Ok(
            crate::window_control::clipboard_text()?.is_some_and(|text| {
                text.contains("traditional clipboard") && text.contains("second line")
            }),
        )
    })?;
    // Each shortcut is checked on its own, so an illegal cut cannot be undone by the paste.
    const CUT_SENTINEL: &str = "preview cut must not reach the clipboard";
    const PASTE_SENTINEL: &str = "preview paste must not reach the note";
    crate::window_control::set_clipboard_text(CUT_SENTINEL)?;
    crate::window_control::press_shift_delete(window)?;
    thread::sleep(Duration::from_millis(750));
    if fs::read_to_string(program.join("note/note.md")).map_err(io_error)? != TEXT
        || crate::window_control::clipboard_text()?.as_deref() != Some(CUT_SENTINEL)
    {
        return Err("Shift+Delete cut from the read-only Preview".to_owned());
    }
    crate::window_control::set_clipboard_text(PASTE_SENTINEL)?;
    crate::window_control::press_shift_insert(window)?;
    thread::sleep(Duration::from_millis(750));
    if fs::read_to_string(program.join("note/note.md")).map_err(io_error)? != TEXT {
        return Err("Shift+Insert pasted into the read-only Preview".to_owned());
    }
    checks.record("preview-ignores-traditional-edits", &[]);

    crate::window_control::switch_to_source(child.id())?;
    wait_for_config(program, "view_mode = \"source\"")?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_shift_delete(window)?;
    wait_note(program, str::is_empty)?;

    crate::window_control::set_clipboard_dib()?;
    crate::window_control::press_shift_insert(window)?;
    let pasted = wait_note(program, |text| text.contains("![](images/stickymd-"))?;
    // The shortcut reaches the image paste path: the note is exactly the reference to a
    // published asset. The header is a sanity check, not a decode; the DIB to PNG
    // conversion itself is owned by the stickymd-render image tests.
    if !is_png_of_size(&pasted_image(program, &pasted)?, 2, 2) {
        return Err("Shift+Insert DIB did not publish a 2x2 PNG asset".to_owned());
    }
    checks.record("shift-insert-dib-publishes-asset", &[]);
    crate::window_control::press_undo(window)?;
    wait_note(program, str::is_empty)?;

    let file = program.join("traditional-file-drop.png");
    fs::write(&file, PNG).map_err(io_error)?;
    crate::window_control::set_clipboard_file_drop(&[file.as_path()])?;
    crate::window_control::press_shift_insert(window)?;
    let pasted = wait_note(program, |text| text.contains("![](images/stickymd-"))?;
    // A PNG file is published byte for byte.
    if pasted_image(program, &pasted)? != PNG {
        return Err("Shift+Insert file drop did not publish the dropped PNG".to_owned());
    }
    checks.record("shift-insert-file-drop-publishes-asset", &[]);
    crate::window_control::press_undo(window)?;
    wait_note(program, str::is_empty)?;
    child.kill_and_wait()
}

pub(super) fn g4_04(_repository: &Path, program: &Path, checks: &mut Checks) -> Result<(), String> {
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
    checks.record("math-convert-source-refresh", &[]);
    wait_note(program, |text| text == CONVERTED)?;
    checks.record("math-convert-durable", &[]);
    crate::window_control::press_undo(window)?;
    if source_projection(window)? != ORIGINAL {
        return Err("one Undo did not restore the complete pre-conversion source".to_owned());
    }
    checks.record("math-undo-one-step", &[]);
    wait_note(program, |text| text == ORIGINAL)?;
    checks.record("math-undo-durable", &[]);
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

/// The bytes of the asset a single-image paste into an empty note published.
fn pasted_image(program: &Path, text: &str) -> Result<Vec<u8>, String> {
    let name = pasted_image_name(text)
        .ok_or_else(|| format!("the note is not exactly one managed image: {text:?}"))?;
    fs::read(program.join("note/images").join(name))
        .map_err(|error| format!("referenced image {name} is not published: {error}"))
}

/// The managed file name when `text` is exactly the Markdown a single-image paste inserts,
/// `![](images/<name>)` (`assets::transaction`), and nothing else.
fn pasted_image_name(text: &str) -> Option<&str> {
    let name = text.strip_prefix("![](images/")?.strip_suffix(')')?;
    (name.starts_with("stickymd-")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.')))
    .then_some(name)
}

/// A PNG whose header declares `width` x `height`.
fn is_png_of_size(bytes: &[u8], width: u32, height: u32) -> bool {
    bytes.len() >= 24
        && bytes[..8] == [137, 80, 78, 71, 13, 10, 26, 10]
        && &bytes[12..16] == b"IHDR"
        && bytes[16..20] == width.to_be_bytes()
        && bytes[20..24] == height.to_be_bytes()
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
    use super::{PNG, is_png_of_size, normalize_newlines, pasted_image_name};

    #[test]
    fn a_pasted_image_note_is_exactly_one_managed_reference() {
        assert_eq!(
            pasted_image_name("![](images/stickymd-ab12.png)"),
            Some("stickymd-ab12.png")
        );
        for rejected in [
            "",
            "![](images/user.png)",
            "![](images/stickymd-ab12.png",
            "`![](images/stickymd-ab12.png)`",
            "![](images/stickymd-ab12.png)\n",
            "text ![](images/stickymd-ab12.png)",
            "![](images/stickymd-a.png)\n\n![](images/stickymd-b.png)",
            "![](images/stickymd-a/../b.png)",
        ] {
            assert_eq!(pasted_image_name(rejected), None, "{rejected:?}");
        }
    }

    #[test]
    fn the_png_header_check_reads_the_declared_size() {
        assert!(is_png_of_size(PNG, 1, 1));
        assert!(!is_png_of_size(PNG, 2, 2));
        assert!(!is_png_of_size(&PNG[..20], 1, 1));
        assert!(!is_png_of_size(b"GIF89a.................", 1, 1));
    }

    #[test]
    fn source_probe_only_normalizes_windows_newlines() {
        assert_eq!(normalize_newlines("a\n"), "a\n");
        assert_eq!(normalize_newlines("a\r"), "a\r");
        assert_eq!(normalize_newlines("a\r\nb\r\n"), "a\nb\n");
    }
}
