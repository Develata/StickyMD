//! P16-I03: holding a letter shows no accent menu, repeats it, and leaves no stray characters.
//! A posted key is not repeated by the system, so the driver injects the repeats itself (key-downs
//! flagged as automatic repeats; USER 2026-11-19); the repeat rate is therefore the driver's, and
//! "repeats at the system's rate" is not shown. The same input first goes to a TextEdit control:
//! the accent menu must appear there (a new on-screen window), or the input does not exercise the
//! press-and-hold path and nothing about StickyMD is judged. In StickyMD no window of that owner
//! may appear, and the note must gain only the held letter, at least twice, then the digit typed
//! after it (a digit would pick an accent if the menu were open).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::{Row, Verdict},
    scenarios::{Backend, Driver, Op, Value},
};

/// The key held (ABC layout `a`), the digit typed after it, and the injected repeats.
pub(super) const KEY_A: u16 = 0;
pub(super) const KEY_2: u16 = 19;
pub(super) const REPEATS: usize = 15;
pub(super) const EVERY_MS: u64 = 80;

/// An on-screen window: number, owner PID, owner name.
pub(super) type OnScreen = (u32, u32, String);

/// Windows in `after` that were not in `before`.
pub(super) fn appeared(before: &[OnScreen], after: &[OnScreen]) -> Vec<OnScreen> {
    after
        .iter()
        .filter(|w| !before.iter().any(|b| b.0 == w.0))
        .cloned()
        .collect()
}

fn record(row: &mut Row, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        row.skip(name, &detail);
    } else {
        row.check(name, verdict == Verdict::Pass, detail);
    }
}

/// The I03 checks: `control` the windows that appeared in TextEdit and its text after the hold;
/// `product` the windows that appeared while StickyMD held the key, and `added` what the note
/// gained (the held letter and the digit after it).
pub(super) fn judge(
    control: (&[OnScreen], &str),
    product: &[OnScreen],
    added: &str,
    row: &mut Row,
) {
    let owners: Vec<&str> = control.0.iter().map(|w| w.2.as_str()).collect();
    let checks = [
        "no accent menu while held",
        "held key repeats without stray characters",
    ];
    if owners.is_empty() {
        for name in checks {
            row.skip(
                name,
                &format!(
                    "the injected hold opened no accent menu in TextEdit (text {:?}): the input \
                     does not exercise press-and-hold",
                    control.1
                ),
            );
        }
        return;
    }
    let menu: Vec<_> = product
        .iter()
        .filter(|w| owners.contains(&w.2.as_str()))
        .collect();
    let verdict = if !menu.is_empty() {
        Verdict::Fail
    } else if !product.is_empty() {
        // Some other window appeared: the scene changed under the check.
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    record(
        row,
        checks[0],
        verdict,
        format!(
            "TextEdit control: windows {:?}, text {:?}; StickyMD: windows {product:?}",
            control.0, control.1
        ),
    );
    let held = added.trim_end_matches('2');
    let ok = added.ends_with('2')
        && added.len() == held.len() + 1
        && held.len() >= 2
        && held.chars().all(|c| c == 'a');
    record(
        row,
        checks[1],
        if ok { Verdict::Pass } else { Verdict::Fail },
        format!(
            "the note gained {added:?}: {} of the held letter for 1 key-down and {REPEATS} \
             injected repeats {EVERY_MS} ms apart (the rate is the driver's, not the system's)",
            held.chars().filter(|c| *c == 'a').count()
        ),
    );
}

fn windows<B: Backend>(d: &mut Driver<'_, B>) -> Result<Vec<OnScreen>, String> {
    match d.op(Op::OnScreen)? {
        Value::Windows(w) => Ok(w),
        v => Err(format!("expected windows, got {v:?}")),
    }
}

pub(super) fn run<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    let (control, text) = match d.op(Op::Control)? {
        Value::Control(windows, text) => (windows, text),
        Value::Text(reason) => {
            d.row.skip("no accent menu while held", &reason);
            d.row
                .skip("held key repeats without stray characters", &reason);
            return Ok(());
        }
        v => return Err(format!("expected the control's result, got {v:?}")),
    };
    d.op(Op::Focus)?;
    d.wait(500)?;
    let before_note = d.text(Op::NoteNow)?;
    let before = windows(d)?;
    d.op(Op::Hold(KEY_A, REPEATS, EVERY_MS))?;
    d.wait(300)?;
    let after = windows(d)?;
    d.op(Op::Chord(vec![KEY_2]))?;
    let note = d.text(Op::Note)?;
    let added = note.strip_prefix(&before_note).unwrap_or(&note).to_owned();
    judge((&control, &text), &appeared(&before, &after), &added, d.row);
    Ok(())
}
