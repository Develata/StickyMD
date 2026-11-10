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
