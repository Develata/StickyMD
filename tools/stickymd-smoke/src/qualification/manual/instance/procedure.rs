//! Live manual procedures: own fixture preparation and collect facts, never infer PASS.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

mod wake;

use super::Launched;
use crate::qualification::manual::flow::{Item, Preparation, io_error, read_line};
use crate::qualification::manual_observation::procedure as evidence;
use crate::window_control::{self as window, manual_probe};
use std::io::{BufRead, Write};

pub(super) fn prepare(
    processes: &mut [Launched],
    item: &Item,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<Preparation, String> {
    window::enable_per_monitor_v2_dpi_awareness()?;
    for process in &*processes {
        writeln!(
            output,
            "Observe PID {} from {}",
            process.instance.pid,
            process.program.display()
        )
        .map_err(io_error)?;
    }
    let Some(process) = processes.first_mut() else {
        return Err("no started instance".to_owned());
    };
    if let Some(kind) = evidence::wake_kind(item.case.id, item.scenario) {
        return wake::run(process, kind, input, output);
    }
    if item.case.id == "P12-M05" && item.scenario == "exit-save-failure" {
        return failed_quit(process, input, output);
    }
    if item.case.id == "P12-M35" && matches!(item.scenario, "secondary-right" | "secondary-below") {
        return topology(item.scenario, input, output);
    }
    Ok(Preparation::default())
}

fn ack(input: &mut dyn BufRead, output: &mut dyn Write, message: &str) -> Result<(), String> {
    writeln!(
        output,
        "{message}\nPress Enter when complete (N = leave unrecorded)."
    )
    .map_err(io_error)?;
    output.flush().map_err(io_error)?;
    if read_line(input)?.is_empty() {
        Ok(())
    } else {
        Err("observer left the procedure unrecorded".to_owned())
    }
}

fn confirm(input: &mut dyn BufRead, output: &mut dyn Write, message: &str) -> Result<bool, String> {
    loop {
        writeln!(output, "{message} [Y/N]").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        match read_line(input)?.to_ascii_uppercase().as_str() {
            "Y" | "YES" => return Ok(true),
            "N" | "NO" => return Ok(false),
            _ => writeln!(output, "Enter Y or N explicitly.").map_err(io_error)?,
        }
    }
}

fn fact(prepared: &mut Preparation, key: &str, value: bool) {
    prepared
        .declared
        .insert(key.to_owned(), if value { "yes" } else { "no" }.to_owned());
}

fn failed_quit(
    process: &mut Launched,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<Preparation, String> {
    let hwnd = window::visible_window(process.instance.pid)?;
    ack(
        input,
        output,
        "Save the initial note and verify its clean state before continuing.",
    )?;
    if window::title(hwnd)? != "StickyMD" {
        return Err("the saved baseline is not clean".to_owned());
    }
    let note = process.program.join("note/note.md");
    let baseline = std::fs::read(&note).map_err(io_error)?;
    let mut prepared = crate::qualification::fixture_readonly::with_readonly(&note, || {
        let mut prepared = Preparation::default();
        prepared.declared.insert(
            "baseline-sha256".to_owned(),
            crate::integrity::sha256_bytes(&baseline)?,
        );
        fact(&mut prepared, "readonly-applied", true);
        ack(
            input,
            output,
            "The recorder made only this fixture note read-only. Type a distinct new line in StickyMD, then return here.",
        )?;
        let unchanged = std::fs::read(&note).map_err(io_error)? == baseline;
        if !unchanged || !window::title(hwnd)?.starts_with("StickyMD *") {
            return Err(
                "an acknowledged unsaved edit with an unchanged baseline is required".to_owned(),
            );
        }
        fact(&mut prepared, "before-quit-unchanged", true);
        ack(
            input,
            output,
            "Choose Quit from this instance's tray menu. Observe the save error and retained text, then return here.",
        )?;
        let alive = process.child.is_running()?;
        let visible = alive && window::is_visible(hwnd).unwrap_or(false);
        let title = alive
            && window::title(hwnd).is_ok_and(|title| title == "StickyMD * — 退出已取消：保存失败");
        fact(&mut prepared, "original-alive", alive);
        fact(&mut prepared, "visible", visible);
        fact(&mut prepared, "quit-failure-title", title);
        let retained = confirm(
            input,
            output,
            "Does the original window still show exactly the new unsaved line you entered?",
        )?;
        fact(&mut prepared, "retained-memory-text", retained);
        let same_baseline = std::fs::read(&note).map_err(io_error)? == baseline;
        if !(alive && visible && title && retained && same_baseline) {
            prepared.pass_block = Some("the failed-quit observations did not retain the live visible window, failure title and original text".to_owned());
        }
        Ok(prepared)
    })?;
    // with_readonly explicitly restores and reads back attributes before returning.
    fact(&mut prepared, "readonly-restored", true);
    Ok(prepared)
}

fn topology(
    scenario: &str,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<Preparation, String> {
    let before = manual_probe::monitors()?;
    let primary = before
        .iter()
        .find(|monitor| monitor.primary)
        .ok_or("no primary display")?;
    let values = |r: window::WindowRect| {
        [
            i64::from(r.x),
            i64::from(r.y),
            i64::from(r.width),
            i64::from(r.height),
        ]
    };
    let secondary = before
        .iter()
        .find(|monitor| {
            !monitor.primary
                && evidence::aligned(scenario, values(primary.bounds), values(monitor.bounds))
        })
        .ok_or("no secondary monitor has the required physical arrangement and alignment")?;
    ack(
        input,
        output,
        "Move and use StickyMD on the aligned secondary display; complete this scenario's visual observation without changing the display arrangement.",
    )?;
    if manual_probe::monitors()? != before {
        return Err("display rectangles changed during observation".to_owned());
    }
    let mut prepared = Preparation::default();
    for (key, monitor) in [("primary-rect", primary), ("secondary-rect", secondary)] {
        prepared.declared.insert(
            key.to_owned(),
            values(monitor.bounds)
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    fact(&mut prepared, "topology-retained", true);
    Ok(prepared)
}
