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
