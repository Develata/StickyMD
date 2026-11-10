//! Real keyboard/toolbar/save-panel orchestration and file/config oracles.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, OPTION, SHIFT},
    model::Point,
    scenarios::{Backend, Driver, Op},
};
pub(super) fn keys<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    d.op(Op::Activate)?;
    d.type_text("alpha beta")?;
    d.chord(&[CMD, 1])?;
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
pub(super) fn export<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    d.op(Op::Activate)?;
    d.type_text("# Export\nplain bytes")?;
    d.note("# Export\nplain bytes")?;
    d.chord(&[CMD, SHIFT, 1])?;
    d.op(Op::Panel(false))?;
    d.wait(250)?;
    let open = matches!(d.op(Op::PanelOpen)?, super::scenarios::Value::Bool(true));
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
