//! Pure observation validity rules; uncertainty never proves an acceptance claim.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::Verdict;

pub(super) fn ax_attribute_present(status: i32) -> Result<bool, String> {
    match status {
        0 => Ok(true),
        -25205 | -25212 => Ok(false), // AttributeUnsupported / NoValue only.
        _ => Err(format!(
            "AX attribute error {status}; inspection incomplete"
        )),
    }
}

/// The window a key is meant for: the note window, or the export dialog it opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum KeyTarget {
    Note,
    Dialog,
}

/// Every title plan 09's title function gives the note window starts with this.
pub(super) const NOTE_TITLE: &str = "StickyMD";
/// AppKit's AX identifier of the save dialog (macOS 27.2).
pub(super) const DIALOG_ID: &str = "save-panel";

/// The Return that confirms Go to Folder goes only to that sheet's path field: the sheet is open
/// and the focused element is a text field holding exactly the path just set. Anything else (the
/// sheet gone and the save dialog focused, where Return would confirm the export) injects nothing.
pub(super) fn goto_field(
    front: Result<u32, String>,
    sheet_open: Result<bool, String>,
    focused: Result<(String, String), String>,
    pid: u32,
    path: &str,
) -> Result<(), String> {
    let refuse = |why: String| Err(format!("{why}; no Return was injected"));
    match (front, sheet_open, focused) {
        (Err(e), ..) | (_, Err(e), _) | (.., Err(e)) => refuse(format!("focus unreadable ({e})")),
        (Ok(front), ..) if front != pid => refuse(format!("PID {front} is frontmost, not {pid}")),
        (_, Ok(false), _) => refuse("the Go to Folder sheet is not open".into()),
        (_, _, Ok((role, value)))
            if matches!(role.as_str(), "AXTextField" | "AXComboBox") && value == path =>
        {
            Ok(())
        }
        (_, _, Ok((role, value))) => refuse(format!(
            "focused {role} holds {value:?}, not the path field with {path:?}"
        )),
    }
}

/// Plan 11: a key goes only to a target window that is frontmost and focused. `front` is the
/// system's focused application and `focused` the (title, AX identifier) of that application's
/// focused window. Each target is recognized positively: the note window by its title, the dialog
/// by its identifier; a window recognized as neither, or an unreadable one, injects nothing.
pub(super) fn key_target(
    front: Result<u32, String>,
    focused: Result<(String, String), String>,
    pid: u32,
    target: KeyTarget,
) -> Result<(), String> {
    let front =
        front.map_err(|e| format!("focused application unreadable ({e}); no key was injected"))?;
    if front != pid {
        return Err(format!(
            "keys would reach PID {front}, not the product (PID {pid}); no key was injected"
        ));
    }
    let (title, identifier) =
        focused.map_err(|e| format!("focused window unreadable ({e}); no key was injected"))?;
    let is_target = match target {
        KeyTarget::Note => title.starts_with(NOTE_TITLE) && identifier.is_empty(),
        KeyTarget::Dialog => identifier == DIALOG_ID,
    };
    if is_target {
        Ok(())
    } else {
        Err(format!(
            "focused window title={title:?} id={identifier:?} is not the {target:?} window; \
             no key was injected"
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Bracket {
    pub old_ms: u64,
    pub new_ms: u64,
}
// A 20ms sampler plus a 16ms animation frame and event delivery may add up to
// 70ms. Only 10ms early tolerance allows clock/geometry rounding, not a shorter
// dwell. A gap over 60ms cannot resolve this timing contract. Require the whole
// bracket inside the allowed window; overlapping boundaries remain unproven.
pub(super) const LATE_MS: u64 = 70;
pub(super) fn transition(b: Bracket, target: u64) -> Verdict {
    if b.new_ms < b.old_ms || b.new_ms - b.old_ms > 60 {
        Verdict::NotTested
    } else if b.new_ms < target - 10 || b.old_ms > target + LATE_MS {
        Verdict::Fail
    } else if b.old_ms >= target - 10 && b.new_ms <= target + LATE_MS {
        Verdict::Pass
    } else {
        Verdict::NotTested
    }
}

/// Status of an AX action whose effect a later observation checks. Opening a status-item menu
/// replies only after menu tracking ends, so the request times out as CannotComplete (-25204)
/// even though the menu opened; that outcome is unconfirmed, not success. Other errors propagate.
pub(super) fn ax_action_confirmed(action: &str, status: i32) -> Result<bool, String> {
    match status {
        0 => Ok(true),
        -25204 => Ok(false),
        _ => Err(format!("AX {action} error {status}")),
    }
}

/// The application serial number from `lsappinfo find pid=N`, which prints exactly one line such
/// as `ASN:0x0-0x9e09e-"StickyMD":`. `lsappinfo info` does not accept `pid=N` on macOS 27.2.
pub(super) fn lsappinfo_asn(found: &str) -> Result<String, String> {
    let mut lines = found.lines().map(str::trim).filter(|l| !l.is_empty());
    let (Some(line), None) = (lines.next(), lines.next()) else {
        return Err(format!("expected one lsappinfo ASN, found {found:?}"));
    };
    let hex = |s: &str| {
        s.strip_prefix("0x")
            .is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    let asn = line
        .strip_prefix("ASN:")
        .and_then(|rest| {
            let mut parts = rest.splitn(3, '-');
            let (high, low) = (parts.next()?, parts.next()?);
            (hex(high) && hex(low)).then(|| format!("ASN:{high}-{low}"))
        })
        .ok_or_else(|| format!("unrecognised lsappinfo ASN line {line:?}"))?;
    Ok(asn)
}

/// The `type="…"` value from `lsappinfo info -only ApplicationType <ASN>`.
pub(super) fn lsappinfo_type(info: &str) -> Option<&str> {
    let start = info
        .match_indices("type=\"")
        .find(|(i, _)| *i == 0 || info.as_bytes()[i - 1].is_ascii_whitespace())?
        .0
        + "type=\"".len();
    let len = info[start..].find('"')?;
    Some(&info[start..start + len])
}

/// How long sampling continues past a completion deadline to establish where the window rests.
pub(super) const REST_MS: u64 = 200;

/// Dates an animation's completion from `(returned_ms, bounds)` samples in time order: the first
/// sample from which every later one equals the last, which is taken as the rest position. Being
/// fully on screen is not being finished: a window still 18 DIP short of its rest is on screen
/// (macOS 27.2 VM, W04). Returns that sample's time and the time of the sample before it.
pub(super) fn rest_from<T: PartialEq>(samples: &[(u64, T)]) -> Option<(u64, Option<u64>)> {
    let (_, rest) = samples.last()?;
    let first = samples
        .iter()
        .rposition(|(_, b)| b != rest)
        .map_or(0, |moving| moving + 1);
    Some((samples[first].0, first.checked_sub(1).map(|i| samples[i].0)))
}

/// Completion verdict: the rest must be the expected end state, first reached by `deadline`.
/// A first rest sample after the deadline is late unless the gap before it exceeds 60 ms and
/// straddles the deadline, in which case the sampling cannot tell.
pub(super) fn completion_verdict(
    rest_is_end_state: bool,
    first_at: u64,
    before: Option<u64>,
    deadline: u64,
) -> Verdict {
    if !rest_is_end_state {
        Verdict::Fail
    } else if first_at <= deadline {
        Verdict::Pass
    } else if before.is_some_and(|b| b < deadline && first_at - b > 60) {
        Verdict::NotTested
    } else {
        Verdict::Fail
    }
}
