//! P16-I03: holding a letter shows no accent menu, repeats it, and leaves no stray characters.
//! A posted key is not repeated by the system, so the driver injects the repeats itself (key-downs
//! flagged as automatic repeats; USER 2026-11-19); the repeat rate is therefore the driver's, and
//! "repeats at the system's rate" is not shown. The same input first goes to a TextEdit control:
//! the accent menu must appear there as a new on-screen window of TextEdit's own process (the menu
//! is drawn in-process: macOS 27.2 VM), or the input does not exercise the press-and-hold path
//! and nothing about StickyMD is judged. In StickyMD no new window of its own process may appear,
//! and the note must gain only the held letter, at least twice, then the digit typed after it (a
//! digit would pick an accent if the menu were open).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::{Row, Verdict},
    scenarios::{Backend, Driver, Op, Value},
};

/// The key held (ABC layout `a`), the digit typed after it, and the injected repeats. The note
/// must gain between `REPEATS` and `REPEATS + 1` held letters (one per key-down; the VM runs
/// show `REPEATS`), so neither lost nor extra repeats pass.
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

/// The I03 checks: `control` the windows that appeared while TextEdit (PID `control.1`) held the
/// key, and its text after; `product` the windows that appeared while StickyMD (PID `product.1`)
/// held it; `added` what the note gained (the held letter and the digit after it).
pub(super) fn judge(
    control: (&[OnScreen], u32, &str),
    product: (Result<&[OnScreen], &str>, u32),
    added: &str,
    row: &mut Row,
) {
    let owners: Vec<&OnScreen> = control.0.iter().filter(|w| w.1 == control.1).collect();
    // The menu must be seen and must have answered the digit with an accented letter.
    let picked = control
        .2
        .chars()
        .any(|c| !matches!(c, 'a' | '2') && c.is_alphabetic());
    let checks = [
        "no accent menu while held",
        "held key repeats without stray characters",
    ];
    if owners.is_empty() || !picked {
        for name in checks {
            row.skip(
                name,
                &format!(
                    "the injected hold opened no accent menu in TextEdit (new windows {:?}, text \
                     {:?}): the input does not exercise press-and-hold",
                    control.0, control.2
                ),
            );
        }
        return;
    }
    let verdict = match product.0 {
        // No window list while the key was held: nothing seen either way.
        Err(_) => Verdict::NotTested,
        Ok(new) if new.iter().any(|w| w.1 == product.1) => Verdict::Fail,
        // Some other window appeared: the scene changed under the check.
        Ok(new) if !new.is_empty() => Verdict::NotTested,
        Ok(_) => Verdict::Pass,
    };
    record(
        row,
        checks[0],
        verdict,
        format!(
            "TextEdit control (PID {}): menu windows {owners:?}, text {:?}; StickyMD (PID {}): new \
             windows {:?}",
            control.1, control.2, product.1, product.0
        ),
    );
    let held = added.trim_end_matches('2');
    let ok = added.ends_with('2')
        && added.len() == held.len() + 1
        && (REPEATS..=REPEATS + 1).contains(&held.len())
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

pub(super) fn run<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    let checks = [
        "no accent menu while held",
        "held key repeats without stray characters",
    ];
    let (control, control_pid, text) = match d.op(Op::Control)? {
        Value::Control(windows, pid, text) => (windows, pid, text),
        Value::Text(reason) => {
            for name in checks {
                d.row.skip(name, &reason);
            }
            return Ok(());
        }
        v => return Err(format!("expected the control's result, got {v:?}")),
    };
    // Without a working control, StickyMD is not held at all.
    let menu = control.iter().any(|w| w.1 == control_pid);
    let picked = text
        .chars()
        .any(|c| !matches!(c, 'a' | '2') && c.is_alphabetic());
    if !(menu && picked) {
        judge((&control, control_pid, &text), (Ok(&[]), d.pid), "", d.row);
        return Ok(());
    }
    d.op(Op::Focus)?;
    d.wait(500)?;
    let before_note = d.text(Op::NoteNow)?;
    let before = match d.op(Op::OnScreen)? {
        Value::Windows(w) => w,
        Value::Text(reason) => {
            for name in checks {
                d.row.skip(name, &reason);
            }
            return Ok(());
        }
        v => return Err(format!("expected windows, got {v:?}")),
    };
    let during = match d.op(Op::Hold(KEY_A, REPEATS, EVERY_MS))? {
        Value::Windows(w) => Ok(w),
        Value::Text(reason) => Err(reason),
        v => return Err(format!("expected windows, got {v:?}")),
    };
    d.op(Op::Chord(vec![KEY_2]))?;
    let note = d.text(Op::Note)?;
    let added = note.strip_prefix(&before_note).unwrap_or(&note).to_owned();
    let pid = d.pid;
    let new = during.map(|w| appeared(&before, &w));
    judge(
        (&control, control_pid, &text),
        (new.as_deref().map_err(String::as_str), pid),
        &added,
        d.row,
    );
    Ok(())
}
