//! Desktop-interaction repetition disposition for exact-candidate diagnostics.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#desktop-repetition-jitter-policy

pub(crate) const MINIMUM_INDEPENDENT_RUNS: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepetitionDisposition {
    Pass,
    Fail,
    InsufficientSamples,
}

pub(crate) fn classify(total: usize, failures: usize) -> RepetitionDisposition {
    if total < MINIMUM_INDEPENDENT_RUNS {
        return RepetitionDisposition::InsufficientSamples;
    }
    if failures > total {
        return RepetitionDisposition::Fail;
    }
    let successes = total - failures;
    let scaled_successes = (successes as u128) * 100;
    let scaled_total = total as u128;
    if scaled_successes >= scaled_total * 98 {
        RepetitionDisposition::Pass
    } else {
        RepetitionDisposition::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ninety_eight_percent_or_more_passes() {
        assert_eq!(classify(100, 1), RepetitionDisposition::Pass);
        assert_eq!(classify(100, 2), RepetitionDisposition::Pass);
        assert_eq!(classify(1_000, 20), RepetitionDisposition::Pass);
    }

    #[test]
    fn below_ninety_eight_percent_fails() {
        assert_eq!(classify(100, 3), RepetitionDisposition::Fail);
        assert_eq!(classify(1_000, 21), RepetitionDisposition::Fail);
        assert_eq!(classify(100, 100), RepetitionDisposition::Fail);
    }

    #[test]
    fn nonzero_jitter_needs_at_least_one_hundred_independent_runs() {
        assert_eq!(classify(99, 1), RepetitionDisposition::InsufficientSamples);
    }
}
