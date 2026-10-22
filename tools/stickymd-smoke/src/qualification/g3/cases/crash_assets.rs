//! Crash after an owned image publication or reversible trash move.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{
    Checks, ChildGuard, io_error, start_until_editor_ready, wait_note, wait_until,
};
use super::super::super::{fixture_readonly, receipt};
use super::crash::{copy, exact_bytes, replace, start, wait_pending};
use crate::window_control as window;

pub(super) fn run(program: &Path, checks: &mut Checks) -> Result<(), String> {
    after_publication(program, checks).map_err(|error| format!("image publication: {error}"))?;
    after_trash_move(program, checks).map_err(|error| format!("image trash move: {error}"))
}

fn after_publication(program: &Path, checks: &mut Checks) -> Result<(), String> {
    let instance = copy(program, "asset-published", "")?;
    let (mut child, hwnd) = start(&instance)?;
    window::set_clipboard_png_with_text(super::PNG, "IMAGE_PRIORITY")?;
    window::press_paste(hwnd)?;
    let images = instance.join("note/images");
    let mut observed = None;
    wait_until("published image milestone", || {
        let files = super::managed_files(&images)?;
        match files.as_slice() {
            [] => Ok(false),
            [file] => {
                observed = Some(file.clone());
                Ok(true)
            }
            _ => Err("fresh image paste published multiple assets".to_owned()),
        }
    })?;
    // Observe only the final name, then terminate immediately. This is at or
    // after publication, never a claim to interrupt a particular system call.
    child.kill_and_wait()?;
    let asset = observed.ok_or("missing observed published asset")?;
    let digest = receipt::sha256(&asset)?;
    let name = asset
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid asset name")?;
    let hash_prefix = name
        .strip_prefix("stickymd-")
        .and_then(|name| name.split_once('.'))
        .map(|(hash, _)| hash)
        .ok_or("published asset lacks managed name")?;
    if !digest.starts_with(hash_prefix) || hash_prefix.len() < 20 {
        return Err("published asset bytes do not prove managed ownership".to_owned());
    }
    checks.record("kill-after-asset-publication", &[]);
    let durable = fs::read_to_string(instance.join("note/note.md")).map_err(io_error)?;
    if !durable.is_empty() && durable != format!("![](images/{name})") {
        return Err(format!(
            "image publication left unexpected durable text: {durable:?}"
        ));
    }
    let mut restarted = restart_using_current(&instance)?;
    exact_bytes(&instance.join("note/note.md"), durable.as_bytes())?;
    let trash = instance.join("note/.trash").join(name);
    if durable.is_empty() {
        wait_until("unreferenced published image reconciled", || {
            Ok(!asset.try_exists().map_err(io_error)? && !trash.try_exists().map_err(io_error)?)
        })?;
    } else {
        wait_until("referenced published image retained", || {
            asset.try_exists().map_err(io_error)
        })?;
        if receipt::sha256(&asset)? != digest || trash.try_exists().map_err(io_error)? {
            return Err("restart changed or duplicated the referenced image".to_owned());
        }
    }
    checks.record("restart-after-asset-publication", &[]);
    restarted.kill_and_wait()
}

fn after_trash_move(program: &Path, checks: &mut Checks) -> Result<(), String> {
    let instance = copy(program, "trash-moved", "")?;
    let (mut child, hwnd) = start(&instance)?;
    window::set_clipboard_png_with_text(super::PNG, "IMAGE_PRIORITY")?;
    window::press_paste(hwnd)?;
    let durable = wait_note(&instance, |text| text.starts_with("![](images/stickymd-"))?;
    let files = super::managed_files(&instance.join("note/images"))?;
    let [asset] = files.as_slice() else {
        return Err("expected one baseline image".to_owned());
    };
    let name = asset.file_name().ok_or("asset lacks a filename")?;
    let trash = instance.join("note/.trash").join(name);
    let digest = receipt::sha256(asset)?;
    let note = instance.join("note/note.md");
    fixture_readonly::with_readonly(&note, || {
        replace(hwnd, "removed in memory\n")?;
        wait_until("managed image moved to trash", || {
            trash.try_exists().map_err(io_error)
        })?;
        child.kill_and_wait()?;
        exact_bytes(&note, durable.as_bytes())?;
        if asset.try_exists().map_err(io_error)? || receipt::sha256(&trash)? != digest {
            return Err("trash milestone did not retain exactly the owned image".to_owned());
        }
        Ok(())
    })?;
    checks.record("kill-after-trash-move", &[]);
    let mut restarted = restart_using_current(&instance)?;
    wait_until("durable reference restores trashed image", || {
        Ok(asset.try_exists().map_err(io_error)? && !trash.try_exists().map_err(io_error)?)
    })?;
    exact_bytes(&note, durable.as_bytes())?;
    if receipt::sha256(asset)? != digest {
        return Err("referenced trash recovery changed asset bytes".to_owned());
    }
    checks.record("restart-referenced-trash", &[]);
    restarted.kill_and_wait()
}

fn restart_using_current(program: &Path) -> Result<ChildGuard, String> {
    let note = fs::read(program.join("note/note.md")).map_err(io_error)?;
    let temporary = program.join("note/note.md.tmp");
    let pending = if temporary.try_exists().map_err(io_error)? {
        fs::read(&temporary).map_err(io_error)? != note
    } else {
        false
    };
    let executable = program.join("StickyMD.exe");
    if !pending {
        return start_until_editor_ready(&executable);
    }
    let child = ChildGuard::start(&executable)?;
    let hwnd = window::visible_window(child.id())?;
    wait_pending(hwnd)?;
    window::press_f7(hwnd)?;
    wait_until("keep-current recovery choice", || {
        Ok(!window::title(hwnd)?.contains("恢复选择未完成")
            && !temporary.try_exists().map_err(io_error)?)
    })?;
    Ok(child)
}
