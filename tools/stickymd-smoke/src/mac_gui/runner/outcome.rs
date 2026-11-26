//! How a row of `mac-gui` ends after an error: before the product process was spawned the error
//! is the machine's and stops the run, after it the product's FAIL (gate review D2).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use crate::mac_common::fixture::PreLaunch;
use crate::mac_gui::model::{Row, Trace};

/// How `one` failed. Before the product process was spawned (its session directory, the copy and
/// its check against the evidence, the fixture writes, the spawn itself) the error is the
/// machine's: not a product result (plan 11 #phase-16-macos-host, “记录”; gate review D2). After
/// the spawn it is the product's FAIL, a window that never appears included.
#[derive(Debug)]
pub(super) enum Failure {
    PreLaunch(String),
    Run(String),
}

impl From<PreLaunch> for Failure {
    fn from(error: PreLaunch) -> Self {
        Self::PreLaunch(error.0)
    }
}

/// The row after `one` returned or unwound, and whether the run must stop (`stopped`, which
/// keeps it from being marked finished). An error before the product was spawned, and any error
/// around R03 (review round 3, B4: its session directory, attachments, cleanup), leave the row
/// untested and stop the run; another error is the product's FAIL; an unwind stops the run.
pub(super) fn settle(
    id: &str,
    result: std::thread::Result<Result<(), Failure>>,
    row: &mut Row,
    stopped: &mut Option<String>,
) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(Failure::PreLaunch(error))) => {
            untested_after_error(row, &error);
            *stopped = Some(format!("{id} could not start the product: {error}"));
        }
        Ok(Err(Failure::Run(error))) if id == "R03" => {
            untested_after_error(row, &error);
            *stopped = Some(format!("R03 could not complete: {error}"));
        }
        Ok(Err(Failure::Run(error))) => row.check("execution", false, error),
        Err(payload) => {
            std::mem::forget(payload);
            *stopped = Some("scenario unwound; owned child cleanup attempted".into());
            row.check(
                "execution",
                false,
                "scenario panicked; all later injection stopped",
            );
        }
    }
}

/// R03's row after an error of the machine (review round 3 B4, fix round 4): untested, whatever
/// its checks said before, which stay in its trace for diagnosis.
pub(super) fn untested_after_error(row: &mut Row, error: &str) {
    for check in std::mem::take(&mut row.checks) {
        row.trace.push(Trace {
            ms: 0,
            event: format!(
                "before the error: {}: {}: {}",
                check.name,
                check.verdict.name(),
                check.observation
            ),
        });
    }
    row.skip("execution", error);
}

/// `one`'s outcome from the scenario's and the errors of what follows it (input release, fault
/// restore, attachments, cleanup): an error before the spawn stays one, whatever failed after.
pub(super) fn joined(
    result: Result<(), Failure>,
    after: [Result<(), String>; 4],
) -> Result<(), Failure> {
    let (pre_launch, result) = match result {
        Err(Failure::PreLaunch(error)) => (true, Err(error)),
        Err(Failure::Run(error)) => (false, Err(error)),
        Ok(()) => (false, Ok(())),
    };
    let errors = std::iter::once(result)
        .chain(after)
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    match (errors.is_empty(), pre_launch) {
        (true, _) => Ok(()),
        (false, true) => Err(Failure::PreLaunch(errors.join("; "))),
        (false, false) => Err(Failure::Run(errors.join("; "))),
    }
}

#[cfg(test)]
mod tests {
    use super::{Failure, PreLaunch, Row, joined, settle};
    use crate::mac_gui::model::{Verdict, aggregate};

    /// Gate review D2: an error before the product was spawned (here `copy_fixture`'s) leaves
    /// the row untested and stops the run, so it is not marked finished; it stays one when a
    /// cleanup fails after it. A failure after the spawn, a window that never appeared included,
    /// is the product's FAIL and does not stop the run; R03's errors are untested as before.
    #[test]
    fn an_error_before_the_spawn_is_untested_and_stops_the_run() {
        let copy = || Failure::from(PreLaunch("ditto failed: exit status: 1".into()));
        let mut row = Row::new("F01");
        let mut stopped = None;
        settle("F01", Ok(Err(copy())), &mut row, &mut stopped);
        assert_eq!(aggregate(&row.checks), Verdict::NotTested, "{}", row.json());
        assert!(
            stopped
                .as_deref()
                .is_some_and(|s| s.contains("ditto failed"))
        );
        let after_cleanup = joined(Err(copy()), [Ok(()), Ok(()), Ok(()), Err("rmdir".into())]);
        assert!(
            matches!(&after_cleanup, Err(Failure::PreLaunch(e)) if e.contains("rmdir")),
            "{after_cleanup:?}"
        );
        let window = || Failure::Run("no unique on-screen product window within 20s".into());
        let mut row = Row::new("F01");
        let mut stopped = None;
        settle("F01", Ok(Err(window())), &mut row, &mut stopped);
        assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
        assert_eq!(stopped, None);
        assert!(matches!(
            joined(
                Err(window()),
                [Ok(()), Err("release".into()), Ok(()), Ok(())]
            ),
            Err(Failure::Run(_))
        ));
        let mut row = Row::new("R03");
        settle("R03", Ok(Err(window())), &mut row, &mut stopped);
        assert_eq!(aggregate(&row.checks), Verdict::NotTested);
        assert!(stopped.is_some());
        assert!(joined(Ok(()), [Ok(()), Ok(()), Ok(()), Ok(())]).is_ok());
    }
}
