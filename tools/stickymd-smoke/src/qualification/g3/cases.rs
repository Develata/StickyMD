//! G3 exact-candidate cases and their bounded desktop/file assertions.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use super::super::exact_desktop::{
    ChildGuard, assert_sole_stickymd_process, invoke_uia, io_error, seed_note, wait_for_layout,
    wait_note, wait_until,
};
use super::super::receipt;

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
    0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5, 1, 1,
    39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub(super) fn g3_01(_repository: &Path, program: &Path) -> Result<(), String> {
    let fixture = program.join("clipboard-source.png");
    fs::write(&fixture, PNG).map_err(|error| format!("cannot seed clipboard fixture: {error}"))?;
    for producer in ["explorer", "snipping", "browser"] {
        let instance = program.join(producer);
        fs::create_dir(&instance)
            .map_err(|error| format!("cannot create {producer} instance: {error}"))?;
        fs::copy(program.join("StickyMD.exe"), instance.join("StickyMD.exe"))
            .map_err(|error| format!("cannot copy G3-01 executable: {error}"))?;
        seed_note(&instance, "")?;
        let executable = instance.join("StickyMD.exe");
        let mut child = ChildGuard::start(&executable)?;
        wait_for_layout(&instance)?;
        let window = crate::window_control::visible_window(child.id())?;
        crate::window_control::focus_source_editor(window)?;
        match producer {
            "explorer" => crate::window_control::set_clipboard_file_drop(&[fixture.as_path()])?,
            "snipping" => crate::window_control::set_clipboard_dib()?,
            "browser" => crate::window_control::set_clipboard_png_with_text(
                PNG,
                "TEXT_FORMAT_MUST_NOT_WIN_IMAGE_PRIORITY",
            )?,
            _ => unreachable!(),
        }
        crate::window_control::press_paste(window)?;
        let pasted = wait_note(&instance, |text| text.contains("![](images/stickymd-"))?;
        if pasted.contains("TEXT_FORMAT_MUST_NOT_WIN_IMAGE_PRIORITY") {
            return Err(format!("{producer} image lost clipboard format priority"));
        }
        let active = managed_files(&instance.join("note/images"))?;
        if active.len() != 1 {
            return Err(format!(
                "{producer} paste published {} active assets",
                active.len()
            ));
        }
        crate::window_control::press_undo(window)?;
        wait_note(&instance, str::is_empty)?;
        wait_until("undo asset disposition", || {
            Ok(managed_files(&instance.join("note/images"))?.is_empty())
        })?;
        crate::window_control::press_redo(window)?;
        wait_note(&instance, |text| text == pasted)?;
        wait_until("redo asset restoration", || {
            Ok(managed_files(&instance.join("note/images"))?.len() == 1)
        })?;
        child.kill_and_wait()?;
    }
    Ok(())
}

pub(super) fn g3_02(repository: &Path, program: &Path) -> Result<(), String> {
    let source = "![local](images/user-export.png)\n![remote](https://example.com/remote.png)\n";
    seed_note(program, source)?;
    let user_asset = program.join("note/images/user-export.png");
    fs::write(&user_asset, PNG).map_err(|error| format!("cannot seed export asset: {error}"))?;
    let export_parent = program.join("exports");
    fs::create_dir(&export_parent).map_err(|error| format!("cannot create export dir: {error}"))?;
    let target = export_parent.join("g3-export.md");
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::press_export(window)?;
    invoke_uia(repository, "export", child.id(), Some(&target))?;
    wait_until("export publication", || Ok(target.is_file()))?;
    let exported = fs::read_to_string(&target)
        .map_err(|error| format!("cannot read exported Markdown: {error}"))?;
    if !exported.contains("https://example.com/remote.png")
        || exported.contains("images/user-export.png")
        || !exported.contains("g3-export-assets/")
    {
        return Err("export did not rewrite only the local asset reference".to_owned());
    }
    let copied = regular_files(&export_parent.join("g3-export-assets"))?;
    if copied.len() != 1 || fs::read(&copied[0]).map_err(io_error)? != PNG {
        return Err("export did not copy the exact local user asset once".to_owned());
    }
    if fs::read_to_string(program.join("note/note.md")).map_err(io_error)? != source {
        return Err("export changed the canonical working note".to_owned());
    }
    child.kill_and_wait()
}

pub(super) fn g3_03(_repository: &Path, program: &Path) -> Result<(), String> {
    for (index, offset_ms) in [0_u64, 15, 75, 250].into_iter().enumerate() {
        let instance = program.join(format!("kill-{index}"));
        fs::create_dir(&instance).map_err(io_error)?;
        fs::copy(program.join("StickyMD.exe"), instance.join("StickyMD.exe")).map_err(io_error)?;
        let old = format!("old-complete-{index}\n");
        let new = format!("new-complete-{index}-中文🙂\n");
        seed_note(&instance, &old)?;
        let mut child = ChildGuard::start(&instance.join("StickyMD.exe"))?;
        wait_for_layout(&instance)?;
        let window = crate::window_control::visible_window(child.id())?;
        crate::window_control::focus_source_editor(window)?;
        crate::window_control::press_select_all(window)?;
        crate::window_control::set_clipboard_text(&new)?;
        crate::window_control::press_paste(window)?;
        crate::window_control::press_save(window)?;
        thread::sleep(Duration::from_millis(offset_ms));
        child.kill_and_wait()?;

        let note = instance.join("note/note.md");
        let temporary = instance.join("note/note.md.tmp");
        let note_bytes = fs::read(&note).map_err(io_error)?;
        let note_text = std::str::from_utf8(&note_bytes)
            .map_err(|error| format!("kill offset {offset_ms} left invalid note UTF-8: {error}"))?;
        if note_text != old && note_text != new {
            return Err(format!(
                "kill offset {offset_ms} left a partial canonical note"
            ));
        }
        if temporary.is_file() {
            let bytes = fs::read(&temporary).map_err(io_error)?;
            std::str::from_utf8(&bytes).map_err(|error| {
                format!("kill offset {offset_ms} left invalid recovery UTF-8: {error}")
            })?;
        }

        let mut restarted = ChildGuard::start(&instance.join("StickyMD.exe"))?;
        let restarted_window = crate::window_control::visible_window(restarted.id())?;
        if crate::window_control::title(restarted_window)?.contains("恢复选择未完成") {
            crate::window_control::press_f6(restarted_window)?;
            wait_until("recovery resolution", || {
                let text = fs::read_to_string(&note).map_err(io_error)?;
                Ok(text == new || text == old)
            })?;
        }
        let final_text = fs::read_to_string(&note).map_err(io_error)?;
        if final_text != old && final_text != new {
            return Err(format!(
                "kill offset {offset_ms} did not restart from complete text"
            ));
        }
        restarted.kill_and_wait()?;
    }
    Ok(())
}

pub(super) fn g3_04(repository: &Path, program: &Path) -> Result<(), String> {
    let original = "![user](images/user-important.png)\n";
    seed_note(program, original)?;
    let user = program.join("note/images/user-important.png");
    fs::write(&user, PNG).map_err(io_error)?;
    let baseline = receipt::sha256(&user)?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::set_clipboard_text("plain text\n")?;
    crate::window_control::press_paste(window)?;
    wait_note(program, |text| text == "plain text\n")?;
    assert_user_asset(program, &user, &baseline)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == original)?;
    assert_user_asset(program, &user, &baseline)?;
    crate::window_control::press_redo(window)?;
    wait_note(program, |text| text == "plain text\n")?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == original)?;

    let export_parent = program.join("user-export");
    fs::create_dir(&export_parent).map_err(io_error)?;
    let target = export_parent.join("user.md");
    crate::window_control::press_export(window)?;
    invoke_uia(repository, "export", child.id(), Some(&target))?;
    wait_until("user asset export", || Ok(target.is_file()))?;
    assert_user_asset(program, &user, &baseline)?;
    assert_sole_stickymd_process(child.id())?;
    invoke_uia(repository, "tray-exit", child.id(), None)?;
    child.wait_for_exit()?;
    assert_user_asset(program, &user, &baseline)?;

    let mut restarted = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    assert_user_asset(program, &user, &baseline)?;
    restarted.kill_and_wait()
}

pub(super) fn g3_05(repository: &Path, program: &Path) -> Result<(), String> {
    seed_note(program, "safe boundary\n")?;
    let fake = program.join("note/images/stickymd-00000000000000000000.png");
    fs::write(&fake, PNG).map_err(io_error)?;
    let baseline = receipt::sha256(&fake)?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    assert_sole_stickymd_process(child.id())?;
    invoke_uia(repository, "tray-exit", child.id(), None)?;
    child.wait_for_exit()?;
    assert_fake_asset(program, &fake, &baseline)?;
    let mut restarted = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    assert_fake_asset(program, &fake, &baseline)?;
    restarted.kill_and_wait()
}

fn assert_user_asset(program: &Path, path: &Path, baseline: &str) -> Result<(), String> {
    if !path.is_file() || receipt::sha256(path)? != baseline {
        return Err("user-supplied asset changed or disappeared".to_owned());
    }
    if program.join("note/.trash/user-important.png").exists() {
        return Err("user-supplied asset was moved into managed trash".to_owned());
    }
    Ok(())
}

fn assert_fake_asset(program: &Path, path: &Path, baseline: &str) -> Result<(), String> {
    if !path.is_file() || receipt::sha256(path)? != baseline {
        return Err("managed-looking unowned file changed or disappeared".to_owned());
    }
    if program
        .join("note/.trash/stickymd-00000000000000000000.png")
        .exists()
    {
        return Err("managed-looking unowned file was moved into trash".to_owned());
    }
    Ok(())
}

fn managed_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(regular_files(directory)?
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("stickymd-"))
        })
        .collect())
}

fn regular_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = fs::read_dir(directory)
        .map_err(io_error)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}
