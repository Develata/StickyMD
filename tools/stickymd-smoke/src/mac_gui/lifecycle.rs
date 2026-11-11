//! Quitting while the note cannot be saved: Cmd+Q (W11) and the system's termination request
//! (W12, `NSRunningApplication.terminate` to the owned PID, never by name). The save fails because
//! the fixture's note directory loses its write permission (USER 2026-11-11, `fault.rs`); the
//! runner gives it back before cleanup, on unwind too. Logout is not driven: it would end the VM's
//! GUI session.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, SHIFT},
    scenarios::{Backend, Driver, Op, Value, exits_within, quit},
};

/// The fixture note for W11/W12: durable bytes a save that truncates before failing would destroy.
pub(super) const DURABLE: &str = "durable before the fault\n";
/// Plan 09: the title of a quit that a failed note save cancelled.
pub(super) const QUIT_CANCELLED: &str = "退出已取消：保存失败";
/// Bound for the failed save to cancel the quit and reach the title.
pub(super) const CANCEL_MS: u64 = 5000;
/// A process that exits this long after the cancelled title still fails the check.
const STAY_MS: u64 = 1000;
/// Bound for a quit that waited for the export dialog: the dialog unwinds, then the barrier saves.
pub(super) const MODAL_QUIT_MS: u64 = 10_000;
const PANEL_MS: u64 = 3000;
const SELECT_ALL: [u16; 2] = [CMD, 0];

/// W11: Cmd+Q with a failing save keeps the app, its text and the old bytes on disk; once the save
/// works, the kept text is saved and Cmd+Q exits.
pub(super) fn command_quit<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    const TEXT: &str = "kept while the save fails";
    d.op(Op::Activate)?;
    let titled = title_observable(d)?;
    d.op(Op::Fault(true))?;
    d.chord(&SELECT_ALL)?;
    d.type_text(TEXT)?;
    d.chord(&[CMD, 12])?;
    stays_up(d, titled, "Cmd+Q")?;
    d.op(Op::Fault(false))?;
    d.op(Op::Activate)?;
    d.chord(&[CMD, 1])?;
    d.note(TEXT)?;
    cleared(d, titled)?;
    d.chord(&[CMD, 12])?;
    quit(d)
}

/// W12: the system's termination request. With a failing save it is answered no and the app stays
/// up; with the export dialog open the dialog ends as a cancel and the quit runs after it.
pub(super) fn system_termination<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    const TEXT: &str = "saved by the system quit";
    d.op(Op::Activate)?;
    let titled = title_observable(d)?;
    d.op(Op::Fault(true))?;
    d.chord(&SELECT_ALL)?;
    d.type_text(TEXT)?;
    request(d, "termination with a failing save")?;
    stays_up(d, titled, "termination")?;
    d.op(Op::Fault(false))?;
    d.op(Op::Activate)?;
    d.chord(&[CMD, SHIFT, 1])?;
    let (open, ms) = panel_opens(d)?;
    d.row
        .check("export dialog open", open, format!("after {ms}ms"));
    // Aim the dialog at the one file the export check reads, so "nothing exported" is about the
    // file a confirmed export would have written.
    let aimed = open && d.op(Op::PanelTarget).is_ok();
    // Whether the old bytes are still on disk just before the request; the final check needs it.
    let pending = d.text(Op::NoteNow)? == DURABLE;
    request(d, "termination with the export dialog open")?;
    let (closed, ms) = panel_closes(d)?;
    d.row.check(
        "the request ends the export dialog",
        closed,
        format!("closed={closed} after {ms}ms"),
    );
    let mut gone = closed;
    if !closed {
        // Learn whether the request still runs once the dialog is gone: end it by hand, and time
        // the exit only once the dialog is seen closed.
        d.op(Op::Panel(false))?;
        (gone, _) = panel_closes(d)?;
    }
    let exit_name = "the quit exits once the dialog is gone";
    let exited = if gone {
        let (exited, ms) = exits_within(d, MODAL_QUIT_MS)?;
        d.row.check(
            exit_name,
            exited,
            format!(
                "exited={exited} within {ms}ms of the dialog closing; cancelled by hand={}",
                !closed
            ),
        );
        exited
    } else {
        d.row.skip(
            exit_name,
            "the dialog was not seen closed, even after cancelling it",
        );
        false
    };
    let export_name = "the ended dialog exports nothing to the file it was aimed at";
    if aimed {
        let export = d.text(Op::Export)?;
        d.row.check(export_name, export == "<absent>", export);
    } else {
        d.row.skip(
            export_name,
            "the dialog could not be aimed at the session's export.md",
        );
    }
    // Old bytes just before the request and the new text after the exit. An autosave racing the
    // request cannot be ruled out, so this does not prove the barrier wrote them.
    let name = "text unsaved before the request is on disk after the exit";
    if pending && exited {
        let disk = d.text(Op::Note)?;
        d.row.check(
            name,
            disk == TEXT,
            format!("expected={TEXT:?}; actual={disk:?}"),
        );
    } else {
        d.row.skip(
            name,
            &format!("dirty at the request={pending}; exited={exited}"),
        );
    }
    d.row.skip(
        "logout",
        "a logout would end the VM's GUI session; it stays a manual observation",
    );
    Ok(())
}

fn request<B: Backend>(d: &mut Driver<'_, B>, name: &str) -> Result<(), String> {
    let reply = d.text(Op::Terminate)?;
    d.row.check(
        &format!("{name}: request sent"),
        reply.trim() == "true",
        reply,
    );
    Ok(())
}

fn alive<B: Backend>(d: &mut Driver<'_, B>) -> Result<bool, String> {
    Ok(matches!(d.op(Op::Alive)?, Value::Bool(true)))
}

fn has_title(title: &str) -> bool {
    title.lines().any(|t| !t.trim().is_empty())
}

/// Whether AX reports a window title at all; without one (or with a failed read) the title checks
/// cannot be made.
fn title_observable<B: Backend>(d: &mut Driver<'_, B>) -> Result<bool, String> {
    Ok(matches!(d.op(Op::Title), Ok(Value::Text(t)) if has_title(&t)))
}

/// The quit is cancelled: the title shows it within `CANCEL_MS` (when AX reports titles), the
/// process is still running `STAY_MS` later, and the old bytes are still on disk. A title read can
/// fail while the barrier holds the main thread: polling continues, and only a readable title at
/// the deadline without the cancelled state fails; none observed by then is not tested.
fn stays_up<B: Backend>(d: &mut Driver<'_, B>, titled: bool, via: &str) -> Result<(), String> {
    let start = d.backend.now();
    let mut title = Err(String::from("not read"));
    let mut seen = None;
    if titled {
        loop {
            // A read that returns no title at all is as unobserved as a failed one.
            title = match d.op(Op::Title) {
                Ok(Value::Text(t)) if has_title(&t) => Ok(t),
                Ok(v) => Err(format!("no readable title: {v:?}")),
                Err(e) => Err(e),
            };
            let elapsed = d.backend.now().saturating_sub(start);
            if title.as_ref().is_ok_and(|t| t.contains(QUIT_CANCELLED)) && elapsed <= CANCEL_MS {
                seen = Some(elapsed);
                break;
            }
            if elapsed >= CANCEL_MS {
                break;
            }
            d.wait(100)?;
        }
    }
    d.wait(if titled { STAY_MS } else { CANCEL_MS })?;
    let running = alive(d)?;
    d.row.check(
        &format!("{via}: process keeps running"),
        running,
        format!("alive={running}"),
    );
    let name = format!("{via}: title shows the cancelled quit");
    match (titled, seen, title) {
        (false, ..) => d.row.skip(&name, "AX reports no window title"),
        (true, Some(ms), _) => d.row.check(&name, true, format!("after {ms}ms")),
        (true, None, Ok(t)) => d
            .row
            .check(&name, false, format!("titles at the deadline: {t:?}")),
        (true, None, Err(e)) => d
            .row
            .skip(&name, &format!("title unreadable at the deadline: {e}")),
    }
    let disk = d.text(Op::NoteNow)?;
    d.row.check(
        &format!("{via}: old bytes still on disk"),
        disk == DURABLE,
        format!("{disk:?}"),
    );
    Ok(())
}

/// A readable title without the cancelled state. A title that can no longer be read is not a
/// cleared one.
fn cleared<B: Backend>(d: &mut Driver<'_, B>, titled_before: bool) -> Result<(), String> {
    let name = "title clears once the save succeeds";
    if !titled_before {
        d.row.skip(name, "AX reports no window title");
        return Ok(());
    }
    match d.op(Op::Title) {
        Ok(Value::Text(title)) if has_title(&title) => {
            d.row
                .check(name, !title.contains(QUIT_CANCELLED), format!("{title:?}"));
        }
        Ok(v) => d.row.skip(name, &format!("no readable title: {v:?}")),
        Err(e) => d.row.skip(name, &format!("title read failed: {e}")),
    }
    Ok(())
}

/// Closed means an answered read without the dialog, or the process gone (its dialog with it).
/// An unanswered read proves nothing either way.
fn panel_closes<B: Backend>(d: &mut Driver<'_, B>) -> Result<(bool, u64), String> {
    let start = d.backend.now();
    loop {
        let closed = !alive(d)? || matches!(d.op(Op::PanelOpen), Ok(Value::Bool(false)));
        let elapsed = d.backend.now().saturating_sub(start);
        if closed || elapsed >= PANEL_MS {
            return Ok((closed && elapsed <= PANEL_MS, elapsed));
        }
        d.wait(100)?;
    }
}

/// While the dialog is being presented its modal loop holds the main thread, and an AX read can
/// end as CannotComplete; such a sample is unobserved, not closed, and polling continues.
fn panel_opens<B: Backend>(d: &mut Driver<'_, B>) -> Result<(bool, u64), String> {
    let start = d.backend.now();
    loop {
        let open = matches!(d.op(Op::PanelOpen), Ok(Value::Bool(true)));
        let elapsed = d.backend.now().saturating_sub(start);
        if open || elapsed >= PANEL_MS {
            return Ok((open && elapsed <= PANEL_MS, elapsed));
        }
        d.wait(100)?;
    }
}
