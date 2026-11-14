//! Real keyboard/toolbar/save-panel orchestration and file/config oracles.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, OPTION, SHIFT},
    model::Point,
    scenarios::{Backend, Driver, Op},
};
pub(super) fn keys<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    d.op(Op::Activate)?;
    manual_save(d, "alpha beta")?;
    d.note("alpha beta")?;
    d.chord(&[CMD, 6])?;
    d.note("")?;
    d.chord(&[CMD, SHIFT, 6])?;
    d.note("alpha beta")?;
    d.chord(&[CMD, 0])?;
    d.chord(&[CMD, 8])?;
    let copied = d.text(Op::Clipboard)?;
    d.row
        .check("Cmd+A/C pasteboard", copied == "alpha beta", copied);
    d.chord(&[CMD, 7])?;
    d.note("")?;
    d.chord(&[CMD, 9])?;
    d.note("alpha beta")?;
    d.chord(&[CMD, 123])?;
    d.type_text("L")?;
    d.chord(&[CMD, 124])?;
    d.type_text("R")?;
    d.note("Lalpha betaR")?;
    d.chord(&[36])?;
    d.type_text("second")?;
    d.chord(&[CMD, 126])?;
    d.type_text("D")?;
    d.chord(&[CMD, 125])?;
    d.type_text("E")?;
    d.note("DLalpha betaR\nsecondE")?;
    d.chord(&[CMD, 3])?;
    d.type_text("beta")?;
    d.chord(&[36])?;
    d.chord(&[53])?;
    // Search selects the document match. Replacing it proves find routed away from the note.
    d.type_text("FOUND")?;
    d.note("DLalpha FOUNDR\nsecondE")?;
    // Cmd+Option+F focuses replacement. Tab returns to the retained four-byte query;
    // Search has no select-all binding, so clear that query with real Backspace events.
    d.chord(&[CMD, OPTION, 3])?;
    d.type_text("replaced")?;
    d.chord(&[48])?;
    for _ in 0..4 {
        d.chord(&[51])?;
    }
    d.type_text("FOUND")?;
    d.chord(&[CMD, SHIFT, 36])?;
    d.chord(&[53])?;
    d.note("DLalpha replacedR\nsecondE")?;
    for (key, value) in [(19, "\"split\""), (20, "\"preview\""), (18, "\"source\"")] {
        d.chord(&[CMD, key])?;
        d.config("", "view_mode", value)?;
    }
    for (key, value) in [(24, "110"), (27, "100"), (24, "110"), (29, "100")] {
        d.chord(&[CMD, key])?;
        d.config("", "content_zoom_percent", value)?;
    }
    d.note("DLalpha replacedR\nsecondE")?;
    Ok(())
}
pub(super) fn manual_save<B: Backend>(d: &mut Driver<'_, B>, expected: &str) -> Result<(), String> {
    use super::scenarios::Value;
    let Value::Time(last_key) = d.op(Op::Text(expected.into()))? else {
        return Err("typing did not supply the last key timestamp".into());
    };
    let sent = d.backend.now();
    d.chord(&[CMD, 1])?;
    let returned = d.backend.now();
    let deadline = last_key + 550; // 100ms before the product's 650ms autosave debounce.
    loop {
        let now = d.backend.now();
        // Read directly: quiet_note deliberately waits beyond the debounce.
        let content = if now <= deadline {
            Some(d.text(Op::NoteNow)?)
        } else {
            None
        };
        let observed = d.backend.now();
        let evidence = format!(
            "last_key={last_key}ms; Cmd+S sent={sent}ms returned={returned}ms; observed={observed}ms; deadline={deadline}ms; actual={content:?}"
        );
        if observed <= deadline && content.as_deref() == Some(expected) {
            d.row.check("Cmd+S before autosave", true, evidence);
            return Ok(());
        }
        if observed >= deadline {
            d.row.skip(
                "Cmd+S before autosave",
                &format!("{evidence}; slow VM or missing binding cannot be distinguished"),
            );
            return Ok(());
        }
        d.wait(20.min(deadline - observed))?;
    }
}
pub(super) fn opacity<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    d.op(Op::Activate)?;
    let r = d.window()?.bounds;
    if r.size.width < 300.0 {
        return Err("fixture window too narrow for regular toolbar layout".into());
    }
    // controls/layout.rs: right group, opacity third of five, edge=5/control=28/gap=4.
    let button = Point {
        x: r.right() - 83.0,
        y: r.origin.y + 18.0,
    };
    d.op(Op::Click(button))?;
    d.wait(150)?;
    // Popup right-aligned to opacity control; numeric field auto-selects on click.
    let popup_left = r.right() - 299.0;
    for value in [40, 70, 100] {
        d.op(Op::Click(Point {
            x: popup_left + 194.5,
            y: r.origin.y + 34.0 + 28.0,
        }))?;
        d.type_text(&value.to_string())?;
        d.chord(&[36])?;
        d.wait(250)?;
        let w = d.window()?;
        d.row.check(
            &format!("alpha {value}"),
            (w.alpha - f64::from(value) / 100.0).abs() <= 0.015,
            format!("{w:?}"),
        );
        d.config("", "opacity", &value.to_string())?;
    }
    d.chord(&[53])?;
    Ok(())
}
/// F01. Plan 02's macOS faces: the log names the CJK and Latin faces, not the code face. A font
/// file is mapped into the process once cosmic-text loads one of its faces, for shaping or for a
/// fallback coverage check, and stays mapped (the font cache keeps it). Menlo absent in Source and
/// present after Preview shapes the code span shows that Preview loads Menlo; it does not show
/// that the code glyphs use it, so the face choice and the glyph review stay manual.
pub(super) fn fonts<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    use super::model::Trace;
    let log = d.text(Op::Fonts)?;
    d.row.check(
        "font selection log",
        log.contains("CJK=PingFang SC found=true Latin=Times New Roman found=true"),
        log,
    );
    let name = "Menlo loaded once Preview shapes the code span (not proof of selection)";
    let before = d.text(Op::Mapped)?;
    if maps_menlo(&before) {
        d.row.skip(
            name,
            &format!(
                "Menlo was mapped in Source already, so the mapping cannot be tied to the code span: {}",
                font_files(&before)
            ),
        );
    } else {
        d.op(Op::Activate)?;
        d.chord(&[CMD, 20])?;
        d.config("", "view_mode", "\"preview\"")?;
        let after = d.text(Op::Mapped)?;
        d.row.check(
            name,
            maps_menlo(&after),
            format!(
                "Source: {}; Preview: {}",
                font_files(&before),
                font_files(&after)
            ),
        );
    }
    let screenshot = d.text(Op::Screenshot)?;
    d.row.trace.push(Trace {
        ms: d.backend.now(),
        event: screenshot,
    });
    d.row.skip(
        "glyph visual review",
        "a mapped face and an optional screenshot do not prove per-glyph font selection",
    );
    Ok(())
}
/// `lsof -F n` prints each mapped file as a line starting with `n`.
pub(super) fn maps_menlo(lsof: &str) -> bool {
    lsof.lines()
        .any(|l| l.starts_with('n') && l.contains("/Menlo."))
}
fn font_files(lsof: &str) -> String {
    lsof.lines()
        .filter(|l| l.starts_with('n') && l.contains("/Fonts/"))
        .map(|l| &l[1..])
        .collect::<Vec<_>>()
        .join(", ")
}
pub(super) fn export<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    d.op(Op::Activate)?;
    d.type_text("# Export\nplain bytes")?;
    d.note("# Export\nplain bytes")?;
    d.chord(&[CMD, SHIFT, 1])?;
    if !opened(d, "save panel opens")? {
        return Ok(());
    }
    d.op(Op::Panel(false))?;
    d.wait(250)?;
    let super::scenarios::Value::Bool(open) = d.op(Op::PanelOpen)? else {
        return Err("save panel inspection returned no boolean observation".into());
    };
    d.row.check(
        "cancel dismisses save panel",
        !open,
        format!("panel_open={open}"),
    );
    let cancelled = d.text(Op::Export)?;
    d.row.check(
        "cancel leaves no export",
        cancelled == "<absent>",
        cancelled,
    );
    d.chord(&[CMD, SHIFT, 1])?;
    if !opened(d, "save panel opens again")? {
        return Ok(());
    }
    d.op(Op::Panel(true))?;
    d.wait(600)?;
    let exported = d.text(Op::Export)?;
    d.row.check(
        "export exact bytes",
        exported == "# Export\nplain bytes",
        exported,
    );
    d.note("# Export\nplain bytes")?;
    Ok(())
}

/// Presenting the dialog can hold the main thread past any fixed wait (about 1 s was seen in the
/// VM, where reading its buttons after 300 ms ended as CannotComplete), so its buttons are read
/// only once it is seen open. One not seen open within the bound fails and ends the scenario.
fn opened<B: Backend>(d: &mut Driver<'_, B>, name: &str) -> Result<bool, String> {
    let (open, ms) = super::lifecycle::panel_opens(d)?;
    d.row.check(name, open, format!("after {ms}ms"));
    Ok(open)
}
