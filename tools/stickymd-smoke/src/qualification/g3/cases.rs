//! G3 exact-candidate cases and their bounded desktop/file assertions.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use super::super::exact_desktop::{
    CaseEvidence, Checks, ChildGuard, assert_sole_stickymd_process, invoke_uia, io_error,
    seed_note, start_until_editor_ready, wait_for_layout, wait_note, wait_until,
};
use super::super::receipt;

mod crash;
mod crash_assets;

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
    0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5, 1, 1,
    39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub(super) fn g3_01(_repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    let mut checks = Checks::default();
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
        let producer_param = [("producer", producer)];
        let pasted = wait_note(&instance, |text| text.contains("![](images/stickymd-"))?;
        checks.record("paste-image-markdown", &producer_param);
        if pasted.contains("TEXT_FORMAT_MUST_NOT_WIN_IMAGE_PRIORITY") {
            return Err(format!("{producer} image lost clipboard format priority"));
        }
        // Only the browser producer also offers text, so only it discriminates priority.
        if producer == "browser" {
            checks.record("paste-format-priority", &producer_param);
        }
        let active = managed_files(&instance.join("note/images"))?;
        let [asset] = active.as_slice() else {
            return Err(format!(
                "{producer} paste published {} active assets",
                active.len()
            ));
        };
        let asset_name = asset
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{producer} asset name is not UTF-8"))?;
        if !pasted.contains(&format!("](images/{asset_name})")) {
            return Err(format!(
                "{producer} paste does not reference its published asset {asset_name}"
            ));
        }
        let asset = asset.clone();
        let asset_digest = receipt::sha256(&asset)?;
        checks.record("paste-single-managed-asset", &producer_param);
        crate::window_control::press_undo(window)?;
        wait_note(&instance, str::is_empty)?;
        wait_until("undo asset disposition", || {
            Ok(managed_files(&instance.join("note/images"))?.is_empty())
        })?;
        checks.record("undo-removes-asset", &producer_param);
        crate::window_control::press_redo(window)?;
        wait_note(&instance, |text| text == pasted)?;
        wait_until("redo asset restoration", || {
            Ok(managed_files(&instance.join("note/images"))? == [asset.clone()])
        })?;
        if receipt::sha256(&asset)? != asset_digest {
            return Err(format!("{producer} redo restored different asset bytes"));
        }
        checks.record("redo-restores-asset", &producer_param);
        child.kill_and_wait()?;
    }
    Ok(checked(checks))
}

/// The evidence of a case whose proof is its recorded checks.
fn checked(checks: Checks) -> CaseEvidence {
    CaseEvidence {
        checks,
        ..CaseEvidence::default()
    }
}

pub(super) fn g3_02(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    let source = "![local](images/user-export.png)\n![remote](https://example.com/remote.png)\n";
    seed_note(program, source)?;
    let user_asset = program.join("note/images/user-export.png");
    fs::write(&user_asset, PNG).map_err(|error| format!("cannot seed export asset: {error}"))?;
    let export_parent = program.join("exports");
    fs::create_dir(&export_parent).map_err(|error| format!("cannot create export dir: {error}"))?;
    let target = export_parent.join("g3-export.md");
    let mut checks = Checks::default();
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::press_export(window)?;
    invoke_uia(repository, "export", child.id(), Some(&target))?;
    wait_until("export publication", || Ok(target.is_file()))?;
    checks.record("export-publishes-file", &[]);
    let exported = fs::read_to_string(&target)
        .map_err(|error| format!("cannot read exported Markdown: {error}"))?;
    let copied = regular_files(&export_parent.join("g3-export-assets"))?;
    let [copy] = copied.as_slice() else {
        return Err(format!(
            "export copied {} local asset(s), expected one",
            copied.len()
        ));
    };
    if fs::read(copy).map_err(io_error)? != PNG {
        return Err("export did not copy the exact local user asset".to_owned());
    }
    checks.record("export-copies-asset-once", &[]);
    let copy_name = copy
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "exported asset name is not UTF-8".to_owned())?;
    // Only the local image changes, and it points at the copy actually written.
    let expected = format!(
        "![local](g3-export-assets/{copy_name})\n![remote](https://example.com/remote.png)\n"
    );
    if exported != expected {
        return Err(format!(
            "export did not rewrite only the local asset reference to its copy: {exported:?}"
        ));
    }
    checks.record("export-rewrites-local-asset-only", &[]);
    if fs::read_to_string(program.join("note/note.md")).map_err(io_error)? != source {
        return Err("export changed the canonical working note".to_owned());
    }
    // An edit after the export must still save to the working note, never to the copy.
    const MARKER: &str = "after export\n";
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_document_end(window)?;
    crate::window_control::set_clipboard_text(MARKER)?;
    crate::window_control::press_paste(window)?;
    wait_note(program, |text| text == format!("{source}{MARKER}"))?;
    if fs::read_to_string(&target).map_err(io_error)? != exported {
        return Err("an edit after export changed the exported copy".to_owned());
    }
    checks.record("export-leaves-working-note", &[]);
    child.kill_and_wait()?;
    Ok(checked(checks))
}

pub(super) fn g3_03(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    let mut checks = Checks::default();
    crash::run(repository, program, &mut checks)?;
    for (index, offset_ms) in [0_u64, 15, 75, 250].into_iter().enumerate() {
        let offset = offset_ms.to_string();
        let offset_param = [("offset", offset.as_str())];
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
        checks.record("kill-note-complete", &offset_param);
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
        checks.record("restart-note-complete", &offset_param);
        restarted.kill_and_wait()?;
    }
    Ok(checked(checks))
}

pub(super) fn g3_04(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    let original = "![user](images/user-important.png)\n";
    seed_note(program, original)?;
    let user = program.join("note/images/user-important.png");
    fs::write(&user, PNG).map_err(io_error)?;
    let baseline = receipt::sha256(&user)?;
    let mut checks = Checks::default();
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::set_clipboard_text("plain text\n")?;
    crate::window_control::press_paste(window)?;
    wait_note(program, |text| text == "plain text\n")?;
    checks.record("paste-replaces-note", &[]);
    assert_user_asset(program, &user, &baseline, "paste", &mut checks)?;
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == original)?;
    checks.record("undo-restores-note", &[]);
    assert_user_asset(program, &user, &baseline, "undo", &mut checks)?;
    crate::window_control::press_redo(window)?;
    wait_note(program, |text| text == "plain text\n")?;
    checks.record("redo-replaces-note", &[]);
    crate::window_control::press_undo(window)?;
    wait_note(program, |text| text == original)?;

    let export_parent = program.join("user-export");
    fs::create_dir(&export_parent).map_err(io_error)?;
    let target = export_parent.join("user.md");
    crate::window_control::press_export(window)?;
    invoke_uia(repository, "export", child.id(), Some(&target))?;
    wait_until("user asset export", || Ok(target.is_file()))?;
    checks.record("export-publishes-file", &[]);
    assert_user_asset(program, &user, &baseline, "export", &mut checks)?;
    assert_sole_stickymd_process(child.id())?;
    checks.record("sole-process", &[("stage", "pre-exit")]);
    super::exact_desktop::tray::exit(repository, child.id())?;
    child.wait_for_exit(super::super::exact_desktop::TIMEOUT)?;
    checks.record("tray-exit-closes-process", &[]);
    assert_user_asset(program, &user, &baseline, "tray-exit", &mut checks)?;

    let mut restarted = start_until_editor_ready(&program.join("StickyMD.exe"))?;
    assert_user_asset(program, &user, &baseline, "restart", &mut checks)?;
    restarted.kill_and_wait()?;
    Ok(checked(checks))
}

pub(super) fn g3_05(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    seed_note(program, "safe boundary\n")?;
    let fake = program.join("note/images/stickymd-00000000000000000000.png");
    fs::write(&fake, PNG).map_err(io_error)?;
    let baseline = receipt::sha256(&fake)?;
    let mut checks = Checks::default();
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    assert_sole_stickymd_process(child.id())?;
    checks.record("sole-process", &[("stage", "pre-exit")]);
    super::exact_desktop::tray::exit(repository, child.id())?;
    child.wait_for_exit(super::super::exact_desktop::TIMEOUT)?;
    checks.record("tray-exit-closes-process", &[]);
    assert_fake_asset(program, &fake, &baseline)?;
    checks.record("fake-asset-intact", &[("stage", "tray-exit")]);
    let mut restarted = start_until_editor_ready(&program.join("StickyMD.exe"))?;
    assert_fake_asset(program, &fake, &baseline)?;
    checks.record("fake-asset-intact", &[("stage", "restart")]);
    restarted.kill_and_wait()?;
    Ok(checked(checks))
}

/// The user asset is unchanged and outside managed trash; records that for `stage`.
fn assert_user_asset(
    program: &Path,
    path: &Path,
    baseline: &str,
    stage: &str,
    checks: &mut Checks,
) -> Result<(), String> {
    if !path.is_file() || receipt::sha256(path)? != baseline {
        return Err("user-supplied asset changed or disappeared".to_owned());
    }
    if program.join("note/.trash/user-important.png").exists() {
        return Err("user-supplied asset was moved into managed trash".to_owned());
    }
    checks.record("user-asset-intact", &[("stage", stage)]);
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

/// The regular files directly in `directory`, sorted; a missing directory has none. Any other
/// failure (opening the listing, reading an entry or its type) is an error, so a file that
/// could not be seen is never reported as absent.
fn regular_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let listing_error =
        |error: std::io::Error| format!("cannot list {}: {error}", directory.display());
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(listing_error(error)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(listing_error)?;
        if entry.file_type().map_err(listing_error)?.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::regular_files;

    #[test]
    fn listing_failures_are_errors_and_only_a_missing_directory_is_empty() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-g3-listing-{}-{nonce}",
            std::process::id()
        ));
        assert_eq!(
            regular_files(&root).unwrap(),
            Vec::<std::path::PathBuf>::new()
        );
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("b.png"), b"b").unwrap();
        fs::write(root.join("a.png"), b"a").unwrap();
        assert_eq!(
            regular_files(&root).unwrap(),
            [root.join("a.png"), root.join("b.png")]
        );
        // A path that exists but cannot be listed is an error, not an empty directory.
        assert!(regular_files(&root.join("a.png")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
