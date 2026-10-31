//! The manual verdict per `(case, role)`, reduced from the observation log in one pass.
//!
//! A scenario is FAIL exactly when some FAIL observation of it has no PASS whose `base`
//! includes it (`fail.index < pass.base`): the observer of that PASS had the failure in
//! front of them and retested. The rule ignores registration order, so a late old PASS
//! cannot clear a newer FAIL and a late old FAIL blocks until a retest. Otherwise any PASS
//! makes the scenario PASS, and a scenario with neither is uncovered.

use std::collections::BTreeMap;

use super::log::{Log, Observation, Status};
use super::registry::{self, Case};
use super::role::match_role;
use super::target::{Role, Target};

/// What an observation must match to count as evidence: the declared targets, by its
/// recorded environment, and the promoted candidate's EXE, by the instance it observed (the
/// M34 first-run declaration has no instance).
#[derive(Clone, Copy, Debug)]
pub(in crate::qualification) struct Binding<'a> {
    pub(in crate::qualification) target: &'a Target,
    pub(in crate::qualification) exe_sha256: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) enum Verdict {
    Pass,
    /// Scenarios with an unresolved FAIL.
    Fail(Vec<&'static str>),
    /// Some required scenario has no observation; `covered` lists those that passed.
    NotTested {
        covered: Vec<&'static str>,
        missing: Vec<&'static str>,
    },
    /// An observation for this cell is not valid evidence.
    Invalid(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Cell {
    pub(in crate::qualification) case: &'static Case,
    pub(in crate::qualification) role: Role,
    pub(in crate::qualification) verdict: Verdict,
}

#[derive(Default)]
struct ScenarioState {
    /// Highest `base` among PASS observations, i.e. how much of the log the latest
    /// retest saw.
    max_pass_base: Option<u64>,
    /// Indices of FAIL observations.
    fails: Vec<u64>,
}

impl ScenarioState {
    fn record(&mut self, index: u64, base: u64, status: Status) {
        match status {
            Status::Pass => {
                self.max_pass_base = Some(self.max_pass_base.map_or(base, |seen| seen.max(base)));
            }
            Status::Fail => self.fails.push(index),
        }
    }

    /// FAILs that no PASS has seen: `index >= base` for every PASS.
    fn unresolved(&self) -> impl Iterator<Item = u64> + '_ {
        let resolved_below = self.max_pass_base.unwrap_or(0);
        self.fails
            .iter()
            .copied()
            .filter(move |index| *index >= resolved_below)
    }
}

/// Why an observation is not valid evidence for the role it claims, or `None` when it is.
/// A carried FAIL keeps the role it was validated under in its own context: it can only
/// block, and a later valid PASS that saw it resolves it even if the targets changed.
pub(in crate::qualification) fn invalidity(
    observation: &Observation,
    binding: Binding,
) -> Option<String> {
    if !observation.carried_from.is_empty() {
        return None;
    }
    if let Err(error) = super::procedure::validate(
        &observation.case,
        &observation.scenario,
        observation.status,
        &observation.declared,
    ) {
        return Some(format!(
            "observation {}#{}: {error}",
            observation.session, observation.seq
        ));
    }
    let case = match super::registry::find(&observation.case) {
        Some(case) => case,
        None => return Some(format!("unknown observation case {}", observation.case)),
    };
    if let Err(error) = super::instances::validate(
        case.instance,
        &observation.instances,
        Some(binding.exe_sha256),
    ) {
        return Some(format!(
            "observation {}#{}: {error}",
            observation.session, observation.seq
        ));
    }
    match match_role(&observation.observed, binding.target) {
        Ok(role) if role == observation.role => None,
        Ok(role) => Some(format!(
            "observation {}#{} claims role {} but its environment matches {}",
            observation.session,
            observation.seq,
            observation.role.as_str(),
            role.as_str()
        )),
        Err(error) => Some(format!(
            "observation {}#{}: {error}",
            observation.session, observation.seq
        )),
    }
}

/// Unresolved FAIL indices per `(case, role, scenario)`, for a reset to carry into the next
/// context. Every FAIL counts whatever its environment, but a PASS resolves one only when
/// it is valid under `target`; without a target that can validate the log, no PASS counts
/// and every FAIL is carried.
pub(in crate::qualification) fn unresolved_fails(
    log: &Log,
    binding: Option<Binding>,
) -> BTreeMap<(String, Role, String), Vec<u64>> {
    let mut states: BTreeMap<(String, Role, String), ScenarioState> = BTreeMap::new();
    for (index, observation) in log.observations.iter().enumerate() {
        let counts = match observation.status {
            Status::Fail => true,
            Status::Pass => {
                binding.is_some_and(|binding| invalidity(observation, binding).is_none())
            }
        };
        if counts {
            states
                .entry((
                    observation.case.clone(),
                    observation.role,
                    observation.scenario.clone(),
                ))
                .or_default()
                .record(index as u64, observation.base, observation.status);
        }
    }
    states
        .into_iter()
        .filter_map(|(key, state)| {
            let fails = state.unresolved().collect::<Vec<_>>();
            (!fails.is_empty()).then_some((key, fails))
        })
        .collect()
}

/// One cell per registered case and role, in registry order then `current`, `previous`.
/// Preconditions: `log` parsed and its header matched to `target` and the registry by
/// the caller; every observation names a registered case and scenario (parse checks it).
pub(in crate::qualification) fn reduce(log: &Log, binding: Binding) -> Vec<Cell> {
    let mut states: BTreeMap<(&str, Role, &str), ScenarioState> = BTreeMap::new();
    let mut invalid: BTreeMap<(&str, Role), String> = BTreeMap::new();
    for (index, observation) in log.observations.iter().enumerate() {
        if let Some(reason) = invalidity(observation, binding) {
            invalid
                .entry((observation.case.as_str(), observation.role))
                .or_insert(reason);
            continue;
        }
        states
            .entry((
                observation.case.as_str(),
                observation.role,
                observation.scenario.as_str(),
            ))
            .or_default()
            .record(index as u64, observation.base, observation.status);
    }
    let mut cells = Vec::with_capacity(registry::CASES.len() * Role::ALL.len());
    for case in registry::CASES {
        for role in Role::ALL {
            let verdict = if let Some(reason) = invalid.remove(&(case.id, role)) {
                Verdict::Invalid(reason)
            } else {
                scenario_verdict(case, role, &states)
            };
            cells.push(Cell {
                case,
                role,
                verdict,
            });
        }
    }
    cells
}

fn scenario_verdict(
    case: &'static Case,
    role: Role,
    states: &BTreeMap<(&str, Role, &str), ScenarioState>,
) -> Verdict {
    let mut failed = Vec::new();
    let mut covered = Vec::new();
    let mut missing = Vec::new();
    for scenario in case.scenarios {
        match states.get(&(case.id, role, *scenario)) {
            None => missing.push(*scenario),
            Some(state) => {
                if state.unresolved().next().is_some() {
                    failed.push(*scenario);
                } else if state.max_pass_base.is_some() {
                    covered.push(*scenario);
                } else {
                    missing.push(*scenario);
                }
            }
        }
    }
    if !failed.is_empty() {
        Verdict::Fail(failed)
    } else if missing.is_empty() {
        Verdict::Pass
    } else {
        Verdict::NotTested { covered, missing }
    }
}

#[cfg(test)]
mod tests {
    use super::{Binding, Verdict, reduce};
    use crate::qualification::manual_observation::log::{Log, Status, fixtures};
    use crate::qualification::manual_observation::target::Role::{self, Current, Previous};
    use crate::qualification::manual_observation::target::{Target, fixtures::target};

    fn binding(target: &Target) -> Binding<'_> {
        Binding {
            target,
            exe_sha256: fixtures::EXE,
        }
    }

    fn verdict(log: &Log, case: &str, role: Role) -> Verdict {
        reduce(log, binding(&target()))
            .into_iter()
            .find(|cell| cell.case.id == case && cell.role == role)
            .unwrap()
            .verdict
    }

    fn log_of(entries: &[(u64, &str, &str, Role, Status)]) -> Log {
        let mut log = fixtures::empty();
        log.observations = entries
            .iter()
            .enumerate()
            .map(|(index, (base, case, scenario, role, status))| {
                fixtures::observation(index as u64 + 1, *base, case, scenario, *role, *status)
            })
            .collect();
        log
    }

    #[test]
    fn zero_partial_and_full_coverage() {
        let empty = fixtures::empty();
        assert_eq!(
            verdict(&empty, "P12-M36", Current),
            Verdict::NotTested {
                covered: Vec::new(),
                missing: vec![
                    "drag",
                    "dock",
                    "left-dock-sensor",
                    "right-dock-sensor",
                    "ime",
                    "preview"
                ]
            }
        );
        let partial = log_of(&[(0, "P12-M36", "dock", Current, Status::Pass)]);
        assert_eq!(
            verdict(&partial, "P12-M36", Current),
            Verdict::NotTested {
                covered: vec!["dock"],
                missing: vec![
                    "drag",
                    "left-dock-sensor",
                    "right-dock-sensor",
                    "ime",
                    "preview"
                ]
            }
        );
        // The same observations on the other role cover nothing there.
        assert!(matches!(
            verdict(&partial, "P12-M36", Previous),
            Verdict::NotTested { .. }
        ));
        let full = log_of(&[
            (0, "P12-M36", "drag", Current, Status::Pass),
            (0, "P12-M36", "dock", Current, Status::Pass),
            (0, "P12-M36", "left-dock-sensor", Current, Status::Pass),
            (0, "P12-M36", "right-dock-sensor", Current, Status::Pass),
            (0, "P12-M36", "ime", Current, Status::Pass),
            (0, "P12-M36", "preview", Current, Status::Pass),
        ]);
        assert_eq!(verdict(&full, "P12-M36", Current), Verdict::Pass);
    }

    #[test]
    fn a_fail_holds_until_a_pass_from_a_session_that_saw_it() {
        // Session A (base 0) records FAIL at index 0; its own later PASS also has base 0.
        let same_session = log_of(&[
            (
                0,
                "P12-M24",
                "representative-markdown",
                Current,
                Status::Fail,
            ),
            (
                0,
                "P12-M24",
                "representative-markdown",
                Current,
                Status::Pass,
            ),
        ]);
        assert_eq!(
            verdict(&same_session, "P12-M24", Current),
            Verdict::Fail(vec!["representative-markdown"])
        );
        // A retest started after the FAIL was logged (base 1) resolves it.
        let retest = log_of(&[
            (
                0,
                "P12-M24",
                "representative-markdown",
                Current,
                Status::Fail,
            ),
            (
                1,
                "P12-M24",
                "representative-markdown",
                Current,
                Status::Pass,
            ),
        ]);
        assert_eq!(verdict(&retest, "P12-M24", Current), Verdict::Pass);
    }

    #[test]
    fn registration_order_cannot_clear_or_resurrect_a_failure() {
        // A late bundle's old PASS (base 0) arrives after a newer FAIL: the FAIL stays.
        let late_pass = log_of(&[
            (0, "P12-M25", "formulas", Current, Status::Fail),
            (0, "P12-M25", "formulas", Current, Status::Pass),
        ]);
        assert!(matches!(
            verdict(&late_pass, "P12-M25", Current),
            Verdict::Fail(_)
        ));
        // After a valid retest cleared the FAIL, a late old PASS changes nothing.
        let after_retest = log_of(&[
            (0, "P12-M25", "formulas", Current, Status::Fail),
            (1, "P12-M25", "formulas", Current, Status::Pass),
            (0, "P12-M25", "formulas", Current, Status::Pass),
        ]);
        assert_eq!(verdict(&after_retest, "P12-M25", Current), Verdict::Pass);
        // A late old FAIL imported after the retest blocks until a new retest sees it.
        let late_fail = log_of(&[
            (0, "P12-M25", "formulas", Current, Status::Pass),
            (0, "P12-M25", "formulas", Current, Status::Fail),
        ]);
        assert!(matches!(
            verdict(&late_fail, "P12-M25", Current),
            Verdict::Fail(_)
        ));
    }

    #[test]
    fn a_mismatched_or_unapproved_environment_invalidates_its_cell() {
        let mut log = log_of(&[(
            0,
            "P12-M24",
            "representative-markdown",
            Current,
            Status::Pass,
        )]);
        log.observations[0].role = Previous; // facts are the current role's
        assert!(matches!(
            verdict(&log, "P12-M24", Previous),
            Verdict::Invalid(reason) if reason.contains("matches current")
        ));
        let mut log = log_of(&[(
            0,
            "P12-M24",
            "representative-markdown",
            Current,
            Status::Pass,
        )]);
        log.observations[0].observed.ubr = 1;
        assert!(matches!(
            verdict(&log, "P12-M24", Current),
            Verdict::Invalid(_)
        ));
        // Other cells are unaffected.
        assert!(matches!(
            verdict(&log, "P12-M25", Current),
            Verdict::NotTested { .. }
        ));
    }

    #[test]
    fn every_registered_case_has_one_cell_per_role() {
        let cells = reduce(&fixtures::empty(), binding(&target()));
        assert_eq!(
            cells.len(),
            crate::qualification::manual_observation::registry::CASES.len() * 2
        );
    }

    #[test]
    fn an_invalid_pass_resolves_nothing_at_readiness_or_reset() {
        use super::unresolved_fails;
        let mut log = log_of(&[
            (0, "P12-M25", "formulas", Current, Status::Fail),
            (1, "P12-M25", "formulas", Current, Status::Pass),
        ]);
        log.observations[1].observed.ubr = 1; // not an approved build
        assert!(matches!(
            verdict(&log, "P12-M25", Current),
            Verdict::Invalid(_)
        ));
        let carried = unresolved_fails(&log, Some(binding(&target())));
        assert_eq!(carried.len(), 1, "{carried:?}");
        // Without a target to validate against, no PASS resolves anything.
        let mut valid = log.clone();
        valid.observations[1].observed.ubr = 7019;
        assert!(unresolved_fails(&valid, Some(binding(&target()))).is_empty());
        assert_eq!(unresolved_fails(&valid, None).len(), 1);
    }

    /// An instance that ran another EXE than the candidate's is not evidence: the cell is
    /// invalid, and such a PASS resolves no FAIL. The M34 first-run declaration has no
    /// instance and is unaffected.
    #[test]
    fn an_observation_of_another_exe_is_invalid_and_resolves_nothing() {
        use super::unresolved_fails;
        let mut log = log_of(&[
            (0, "P12-M25", "formulas", Current, Status::Fail),
            (1, "P12-M25", "formulas", Current, Status::Pass),
            (0, "P12-M34", "clean-vm-first-run", Current, Status::Pass),
        ]);
        log.observations[1].instances[0].exe_sha256 = "b".repeat(64);
        assert!(matches!(
            verdict(&log, "P12-M25", Current),
            Verdict::Invalid(reason) if reason.contains("not the candidate EXE")
        ));
        assert_eq!(unresolved_fails(&log, Some(binding(&target()))).len(), 1);
        assert_eq!(verdict(&log, "P12-M34", Current), Verdict::Pass);
    }

    #[test]
    fn a_carried_fail_keeps_its_role_and_yields_to_a_valid_retest() {
        let mut log = log_of(&[
            (0, "P12-M25", "formulas", Current, Status::Fail),
            (1, "P12-M25", "formulas", Current, Status::Pass),
        ]);
        // Carried from a context whose approved build is no longer approved.
        log.observations[0].observed.ubr = 1;
        log.observations[0].carried_from =
            vec![crate::qualification::manual_observation::log::Carried {
                context_id: "ffffffffffffffffffffffffffffffff".to_owned(),
                index: 4,
            }];
        assert_eq!(verdict(&log, "P12-M25", Current), Verdict::Pass);
        log.observations.truncate(1);
        assert!(matches!(
            verdict(&log, "P12-M25", Current),
            Verdict::Fail(_)
        ));
    }
}
