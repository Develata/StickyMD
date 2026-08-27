//! Desktop-interaction repetition disposition for exact-candidate diagnostics.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#desktop-repetition-jitter-policy

pub(crate) const MINIMUM_INDEPENDENT_RUNS: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepetitionDisposition {
    PassWithRecordedJitter,
    UserVerificationRequired,
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
    if scaled_successes > scaled_total * 98 {
        RepetitionDisposition::PassWithRecordedJitter
    } else if scaled_successes > scaled_total * 95 {
        RepetitionDisposition::UserVerificationRequired
    } else {
        RepetitionDisposition::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_than_ninety_eight_percent_passes_with_recorded_jitter() {
        assert_eq!(
            classify(100, 1),
            RepetitionDisposition::PassWithRecordedJitter
        );
        assert_eq!(
            classify(1_000, 19),
            RepetitionDisposition::PassWithRecordedJitter
        );
    }

    #[test]
    fn exact_ninety_eight_percent_requires_user_verification() {
        assert_eq!(
            classify(100, 2),
            RepetitionDisposition::UserVerificationRequired
        );
        assert_eq!(
            classify(100, 4),
            RepetitionDisposition::UserVerificationRequired
        );
    }

    #[test]
    fn ninety_five_percent_or_less_fails() {
        assert_eq!(classify(100, 5), RepetitionDisposition::Fail);
        assert_eq!(classify(100, 100), RepetitionDisposition::Fail);
    }

    #[test]
    fn nonzero_jitter_needs_at_least_one_hundred_independent_runs() {
        assert_eq!(classify(99, 1), RepetitionDisposition::InsufficientSamples);
    }
}
