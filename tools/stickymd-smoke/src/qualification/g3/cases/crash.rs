//! Deterministic filesystem milestones for process termination and recovery.
//!
//! plan_ref: docs/plan/05_document_persistence.md#recovery

use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use super::super::super::exact_desktop::{
    Checks, ChildGuard, io_error, seed_note, start_until_editor_ready, wait_for_config, wait_note,
    wait_until,
};
use super::super::super::{fixture_readonly, receipt};
use crate::window_control::{self as window, WindowHandle};

const OLD: &str = "durable before interruption\n";
const NEW: &str = "complete recovery candidate 中文🙂\n";
const PENDING: &str = "恢复选择未完成";

pub(super) fn run(repository: &Path, program: &Path, checks: &mut Checks) -> Result<(), String> {
    note_publication_failure(repository, program, checks)
        .map_err(|error| format!("failed note publication: {error}"))?;
    before_autosave(program, checks)
        .map_err(|error| format!("before-autosave negative control: {error}"))?;
    super::crash_assets::run(program, checks)?;
    recovery_escape(program, checks).map_err(|error| format!("recovery Esc guard: {error}"))
}

pub(super) fn copy(program: &Path, name: &str, text: &str) -> Result<PathBuf, String> {
    let instance = program.join(name);
    fs::create_dir(&instance).map_err(io_error)?;
    fs::copy(program.join("StickyMD.exe"), instance.join("StickyMD.exe")).map_err(io_error)?;
    seed_note(&instance, text)?;
    Ok(instance)
}

pub(super) fn start(program: &Path) -> Result<(ChildGuard, WindowHandle), String> {
    let child = start_until_editor_ready(&program.join("StickyMD.exe"))?;
    let hwnd = window::visible_window(child.id())?;
    window::focus_source_editor(hwnd)?;
    Ok((child, hwnd))
}

pub(super) fn replace(hwnd: WindowHandle, text: &str) -> Result<(), String> {
    window::focus_source_editor(hwnd)?;
    window::press_select_all(hwnd)?;
    window::set_clipboard_text(text)?;
    window::press_paste(hwnd)
}

pub(super) fn exact_bytes(path: &Path, expected: &[u8]) -> Result<(), String> {
    if fs::read(path).map_err(io_error)? == expected {
        Ok(())
    } else {
        Err(format!("unexpected durable bytes in {}", path.display()))
    }
}

fn note_publication_failure(
    repository: &Path,
    program: &Path,
    checks: &mut Checks,
) -> Result<(), String> {
    let instance = copy(program, "publication-failed", OLD)?;
    let note = instance.join("note/note.md");
    let temporary = instance.join("note/note.md.tmp");
    set_modified(&note, SystemTime::now() - Duration::from_secs(60))?;
    let (mut child, hwnd) = start(&instance)?;
    fixture_readonly::with_readonly(&note, || {
        replace(hwnd, NEW)?;
        window::press_save(hwnd)?;
        wait_until(
            "complete temp while canonical publication is refused",
            || Ok(fs::read(&temporary).is_ok_and(|bytes| bytes == NEW.as_bytes())),
        )?;
        // A complete readable temp alone does not prove the publication attempt
        // finished. Request Quit: its fixed failure title acknowledges the note
        // barrier failed after flushing, and the read-only target stays intact.
        super::super::super::exact_desktop::tray::exit(repository, child.id())?;
        wait_until("failed note barrier", || {
            Ok(window::title(hwnd)? == "StickyMD * — 退出已取消：保存失败")
        })?;
        child.kill_and_wait()?;
        exact_bytes(&note, OLD.as_bytes())?;
        exact_bytes(&temporary, NEW.as_bytes())
    })?;
    checks.record("kill-at-publication-failure", &[]);
    let mut restarted = ChildGuard::start(&instance.join("StickyMD.exe"))?;
    let hwnd = window::visible_window(restarted.id())?;
    wait_pending(hwnd)?;
    window::press_f6(hwnd)?;
    wait_note(&instance, |text| text == NEW)?;
    wait_until("recovery temp consumed", || {
        Ok(!temporary.try_exists().map_err(io_error)?)
    })?;
    checks.record("restart-flushed-temp", &[]);
    restarted.kill_and_wait()
}

fn before_autosave(program: &Path, checks: &mut Checks) -> Result<(), String> {
    let instance = copy(program, "before-autosave", OLD)?;
    let (mut child, hwnd) = start(&instance)?;
    replace(hwnd, NEW)?;
    wait_until("unsaved edit reached the document", || {
        Ok(window::title(hwnd)?.starts_with("StickyMD *"))
    })?;
    // No save request or focus transition; the positive dirty observation comes
    // before the 650 ms debounce. A raced save fails this negative control.
    child.kill_and_wait()?;
    exact_bytes(&instance.join("note/note.md"), OLD.as_bytes())?;
    if instance
        .join("note/note.md.tmp")
        .try_exists()
        .map_err(io_error)?
    {
        return Err("negative control reached a save transaction before termination".to_owned());
    }
    let (mut restarted, _) = start(&instance)?;
    exact_bytes(&instance.join("note/note.md"), OLD.as_bytes())?;
    restarted.kill_and_wait()?;
    checks.record("kill-before-autosave-keeps-old", &[]);
    Ok(())
}

fn recovery_escape(program: &Path, checks: &mut Checks) -> Result<(), String> {
    let instance = copy(program, "recovery-escape", OLD)?;
    let (mut setup, hwnd) = start(&instance)?;
    window::move_to_primary_edge(hwnd, window::PrimaryDockEdge::Left)?;
    wait_for_config(&instance, "dock_edge = \"left\"")?;
    setup.kill_and_wait()?;
    let note = instance.join("note/note.md");
    let temporary = instance.join("note/note.md.tmp");
    use std::io::Write;
    let mut file = File::create_new(&temporary).map_err(io_error)?;
    file.write_all(NEW.as_bytes()).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    drop(file);
    let now = SystemTime::now();
    set_modified(&note, now - Duration::from_secs(60))?;
    set_modified(&temporary, now)?;
    if fs::metadata(&temporary)
        .and_then(|meta| meta.modified())
        .map_err(io_error)?
        <= fs::metadata(&note)
            .and_then(|meta| meta.modified())
            .map_err(io_error)?
    {
        return Err("recovery fixture did not retain temp.mtime > note.mtime".to_owned());
    }
    let note_hash = receipt::sha256(&note)?;
    let temp_hash = receipt::sha256(&temporary)?;
    let mut child = ChildGuard::start(&instance.join("StickyMD.exe"))?;
    let hwnd = window::visible_window(child.id())?;
    wait_pending(hwnd)?;
    let before = window::window_rect(hwnd)?;
    let work = window::primary_work_area()?;
    if (before.x - work.x).abs() > 1 {
        return Err("recovery guard fixture is not expanded on the left edge".to_owned());
    }
    window::press_escape(hwnd)?;
    thread::sleep(Duration::from_millis(250)); // Longer than the dock animation.
    if !window::title(hwnd)?.contains(PENDING)
        || window::window_rect(hwnd)? != before
        || receipt::sha256(&note)? != note_hash
        || receipt::sha256(&temporary)? != temp_hash
    {
        return Err("Esc escaped the pending recovery guard".to_owned());
    }
    checks.record("recovery-escape-keeps-pending", &[]);
    window::press_f6(hwnd)?;
    wait_note(&instance, |text| text == NEW)?;
    wait_until("recovery choice completed", || {
        Ok(!window::title(hwnd)?.contains(PENDING))
    })?;
    window::press_escape(hwnd)?;
    wait_until("positive control collapses after recovery", || {
        let rect = window::window_rect(hwnd)?;
        let right = i64::from(rect.x) + i64::from(rect.width);
        Ok(rect.x < work.x && (1..=16).contains(&(right - i64::from(work.x))))
    })?;
    checks.record("recovery-choice-enables-dock-escape", &[]);
    child.kill_and_wait()
}

fn set_modified(path: &Path, time: SystemTime) -> Result<(), String> {
    File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_times(FileTimes::new().set_modified(time)))
        .map_err(io_error)
}

pub(super) fn wait_pending(hwnd: WindowHandle) -> Result<(), String> {
    wait_until("recovery choice UI", || {
        Ok(window::title(hwnd)?.contains(PENDING))
    })
}
