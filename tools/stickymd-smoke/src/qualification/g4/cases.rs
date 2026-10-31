//! G4 exact-candidate case routing.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod diagnostics;
mod dock;
mod editor;
mod escape;
mod identity;
mod ime;
mod opacity;
mod preview_scroll;
mod scroll;
mod search;
mod tray;
mod tray_topmost;

use std::path::Path;
use std::time::{Duration, Instant};

/// How long a negative check holds after its input ("nothing changed"), longer than the 140 ms
/// dock animation. It is not a bound on the asynchronous work (the config write shares the I/O
/// worker): a wrong reaction that starts after it is still missed.
pub(super) const NEGATIVE_SETTLE: Duration = Duration::from_millis(800);
/// How often a held negative check is sampled.
const NEGATIVE_SAMPLE: Duration = Duration::from_millis(50);

/// Holds a negative check over `NEGATIVE_SETTLE`, from right after the input, by discrete
/// sampling: at once, then `NEGATIVE_SAMPLE` after each sample returns (so the real spacing is
/// the interval plus the check's own time, about 20 ms more for `assert_edge`), and once at or
/// after the end; the first sample that does not hold fails. A wrong state still present at some
/// sample fails, including one that appears and reverts before the end, which the single final
/// read this replaces missed. One shorter than the spacing can fall between samples, and one that
/// starts after the window is not seen.
pub(super) fn hold(label: &str, check: impl FnMut() -> Result<(), String>) -> Result<(), String> {
    hold_with(
        NEGATIVE_SETTLE,
        NEGATIVE_SAMPLE,
        Instant::now,
        std::thread::sleep,
        label,
        check,
    )
}

/// `hold` with its clock and sleep supplied. Invariant: every sample before the last is taken
/// before `window` has elapsed, the last at or after it, and consecutive samples are at most
/// `interval` (plus the check's own time) apart.
fn hold_with(
    window: Duration,
    interval: Duration,
    now: impl Fn() -> Instant,
    mut sleep: impl FnMut(Duration),
    label: &str,
    mut check: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let start = now();
    let mut samples = 0_usize;
    loop {
        let elapsed = now().saturating_duration_since(start);
        samples += 1;
        check().map_err(|error| {
            format!(
                "{label}: sample {samples} at {} ms of {} ms: {error}",
                elapsed.as_millis(),
                window.as_millis()
            )
        })?;
        if elapsed >= window {
            return Ok(());
        }
        sleep(interval.min(window - elapsed));
    }
}

use super::super::exact_desktop::{CaseEvidence, Checks};

pub(super) fn g4_01(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| tray::g4_01(repository, program, checks))
}

pub(super) fn g4_02(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| {
        dock::g4_02(repository, program, checks)?;
        scroll::run(program, checks)?;
        preview_scroll::run(repository, program, checks)
    })
}

pub(super) fn g4_03(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| {
        editor::g4_03(repository, program, checks)?;
        search::run(repository, program, checks)
    })
}

pub(super) fn g4_04(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| editor::g4_04(repository, program, checks))
}

pub(super) fn g4_05(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| identity::g4_05(repository, program, checks))
}

pub(super) fn g4_06(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| ime::g4_06(repository, program, checks))
}

/// Runs `case` with a fresh recorder; its evidence is the checks it recorded.
fn checked(case: impl FnOnce(&mut Checks) -> Result<(), String>) -> Result<CaseEvidence, String> {
    let mut checks = Checks::default();
    case(&mut checks)?;
    Ok(CaseEvidence {
        checks,
        ..CaseEvidence::default()
    })
}

#[cfg(test)]
mod hold_tests {
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    use super::hold_with;

    /// Runs `hold_with` on a fake clock that only sleeping advances; `wrong` says whether the
    /// state is wrong at a given elapsed time. Returns the result and the sample times.
    fn held(wrong: impl Fn(Duration) -> bool) -> (Result<(), String>, Vec<Duration>) {
        let start = Instant::now();
        let clock = Cell::new(start);
        let samples = std::cell::RefCell::new(Vec::new());
        let result = hold_with(
            Duration::from_millis(800),
            Duration::from_millis(50),
            || clock.get(),
            |pause| clock.set(clock.get() + pause),
            "negative",
            || {
                let elapsed = clock.get() - start;
                samples.borrow_mut().push(elapsed);
                if wrong(elapsed) {
                    Err("changed".to_owned())
                } else {
                    Ok(())
                }
            },
        );
        (result, samples.into_inner())
    }

    /// On the fake clock checks take no time, so this is the schedule itself: 17 samples from 0
    /// to 800 ms, 50 ms apart.
    #[test]
    fn a_state_that_holds_is_sampled_across_the_whole_window() {
        let (result, samples) = held(|_| false);
        assert_eq!(result, Ok(()));
        assert_eq!(samples.first(), Some(&Duration::ZERO));
        assert_eq!(samples.last(), Some(&Duration::from_millis(800)));
        assert_eq!(samples.len(), 17);
        assert!(
            samples
                .windows(2)
                .all(|pair| pair[1] - pair[0] <= Duration::from_millis(50))
        );
    }

    /// A wrong state that appears and reverts, which the single final read this replaces missed,
    /// fails at a sample it spans; a late one fails from the first sample after it appears.
    #[test]
    fn a_late_or_transient_wrong_state_fails() {
        let late = held(|elapsed| elapsed >= Duration::from_millis(600)).0;
        assert!(late.as_ref().unwrap_err().contains("at 600 ms"), "{late:?}");
        let transient = held(|elapsed| {
            (Duration::from_millis(300)..Duration::from_millis(400)).contains(&elapsed)
        })
        .0;
        assert!(transient.unwrap_err().contains("at 300 ms"));
    }

    /// The documented limits: a wrong state between two samples, and one that starts after the
    /// window, are not seen.
    #[test]
    fn a_wrong_state_between_samples_or_after_the_window_is_not_seen() {
        let between = |elapsed: Duration| {
            (Duration::from_millis(10)..Duration::from_millis(40)).contains(&elapsed)
        };
        assert_eq!(held(between).0, Ok(()));
        assert_eq!(
            held(|elapsed| elapsed > Duration::from_millis(800)).0,
            Ok(())
        );
    }
}
