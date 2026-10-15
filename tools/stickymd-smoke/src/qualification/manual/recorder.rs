//! Wires the scenario loop to the terminal, this machine and the observation store:
//! `acceptance manual run` walks the selected cases, `acceptance manual guided` the Tier A
//! guided steps; both ask for every case and scenario on its own.

pub(super) use crate::cli::ManualSelection as Selection;

use super::super::guided;
use super::super::manual_observation::registry::{self, CASES};
use super::flow::Item;

/// Every scenario of the selection, in registry or guided-step order.
pub(super) fn items(selection: Selection) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    match selection {
        Selection::Cases(session) => {
            for case in CASES
                .iter()
                .filter(|case| session.is_none_or(|session| case.session == session))
            {
                items.extend(case.scenarios.iter().map(|scenario| Item {
                    case,
                    scenario,
                    step: None,
                }));
            }
        }
        Selection::Guided(session) => {
            for step in guided::STEPS
                .iter()
                .filter(|step| session.is_none_or(|session| step.session == session))
            {
                for id in step.case_ids {
                    let case = registry::find(id).ok_or_else(|| {
                        format!("guided step {} names unknown case {id}", step.id)
                    })?;
                    items.extend(case.scenarios.iter().map(|scenario| Item {
                        case,
                        scenario,
                        step: Some(step),
                    }));
                }
            }
        }
    }
    if items.is_empty() {
        return Err("the selected manual session has no scenarios".to_owned());
    }
    Ok(items)
}

#[cfg(not(windows))]
pub(super) fn record(_: &std::path::Path, selection: Selection) -> Result<(), String> {
    items(selection)?;
    Err("recording manual observations requires Windows".to_owned())
}

#[cfg(windows)]
pub(super) use live::record;

/// The recorder against this machine: the Windows desktop, registry and candidate.
#[cfg(windows)]
mod live {
    use std::io::{self, Write};
    use std::path::Path;

    use crate::qualification_environment::{self, QualificationEnvironmentStatus};

    use super::super::super::manual_observation::log::{Note, Observation};
    use super::super::super::manual_observation::reduce::Binding;
    use super::super::super::manual_observation::registry::{Fixture, InstanceRule};
    use super::super::super::manual_observation::role::Facts;
    use super::super::super::manual_observation::store::Commit;
    use super::super::super::manual_observation::{environment, store, target, transfer};
    use super::super::super::receipt;
    use super::super::flow::{self, Bench, Start, io_error};
    use super::super::instance::{Launched, Workshop};
    use super::{Selection, items};

    pub(in super::super) fn record(root: &Path, selection: Selection) -> Result<(), String> {
        super::super::require_terminal()?;
        let items = items(selection)?;
        let desktop = qualification_environment::inspect();
        if desktop.status != QualificationEnvironmentStatus::Valid {
            return Err(format!(
                "manual observation needs an unlocked interactive Windows desktop: {}",
                desktop.summary()
            ));
        }
        let (candidate, digest) = receipt::read_candidate_snapshot(root)?;
        receipt::validate_candidate_against_repository(root, &candidate)?;
        let context = {
            let _guard = store::lock(root, false)?;
            store::load(root, &digest).map_err(|error| error.message())?
        };
        let workshop = items
            .iter()
            .any(|item| item.case.instance != InstanceRule::FirstRunDeclared)
            .then(|| Workshop::prepare(&receipt::candidate_zip(root, &candidate), &candidate))
            .transpose()?;
        let session = target::new_context_id();
        let mut bench = LiveBench {
            root,
            digest: &digest,
            exe_sha256: &candidate.exe_sha256,
            context_id: &context.target.context_id,
            workshop,
        };
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut output = io::stdout().lock();
        writeln!(
        output,
        "Manual observation session {session}: context {}, starting after {} logged observations.",
        context.target.context_id,
        context.log.observations.len()
    )
    .map_err(io_error)?;
        let unresolved = transfer::unresolved(
            &context.log,
            Binding {
                target: &context.target,
                exe_sha256: &candidate.exe_sha256,
            },
        );
        let start = Start {
            target: &context.target,
            base: context.log.observations.len() as u64,
            unresolved: &unresolved,
        };
        let summary = flow::run(
            &mut input,
            &mut output,
            &mut bench,
            &start,
            &session,
            &items,
        )?;
        writeln!(
            output,
            "MANUAL_OBSERVATIONS={} passed={} failed={} unrecorded={}",
            root.join(store::OBSERVATION_LOG).display(),
            summary.passed,
            summary.failed,
            summary.unrecorded
        )
        .map_err(io_error)
    }

    struct LiveBench<'a> {
        root: &'a Path,
        digest: &'a str,
        exe_sha256: &'a str,
        context_id: &'a str,
        workshop: Option<Workshop>,
    }

    impl LiveBench<'_> {
        fn append(
            &self,
            observations: Vec<Observation>,
            notes: Vec<Note>,
            last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
        ) -> Result<Commit, String> {
            store::commit(
                self.root,
                store::Promoted {
                    receipt_sha256: self.digest,
                    exe_sha256: self.exe_sha256,
                },
                &|| super::super::promoted_digest(self.root),
                self.context_id,
                observations,
                notes,
                last_check,
            )
        }
    }

    impl Bench for LiveBench<'_> {
        type Instance = Launched;

        fn capture(&mut self) -> Result<Facts, String> {
            environment::capture()
        }

        fn launch(&mut self, fixture: Fixture) -> Result<Launched, String> {
            self.workshop
                .as_mut()
                .ok_or("this session prepared no candidate instance")?
                .launch(fixture)
        }

        fn commit(
            &mut self,
            observation: Observation,
            last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
        ) -> Result<Commit, String> {
            self.append(vec![observation], Vec::new(), last_check)
        }

        fn annotate(&mut self, note: Note) -> Result<(), String> {
            match self.append(Vec::new(), vec![note], &mut || Ok(None))? {
                Commit::Written => Ok(()),
                Commit::Refused(reason) => Err(reason),
            }
        }

        fn now(&self) -> Result<u64, String> {
            super::super::unix_now()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Selection, items};
    use crate::cli::{GuidedSession, ManualSession};

    #[test]
    fn selections_cover_every_scenario_of_their_cases() {
        let all = items(Selection::Cases(None)).unwrap();
        let scenarios = crate::qualification::manual_observation::registry::CASES
            .iter()
            .map(|case| case.scenarios.len())
            .sum::<usize>();
        assert_eq!(all.len(), scenarios);
        let m5 = items(Selection::Cases(Some(ManualSession::M5))).unwrap();
        assert!(m5.iter().all(|item| item.case.session == ManualSession::M5));
        // Guided sessions cover the 14 Tier A cases, every scenario once.
        let guided = items(Selection::Guided(None)).unwrap();
        let tier_a = crate::qualification::manual_observation::registry::CASES
            .iter()
            .filter(|case| case.tier == crate::qualification::manual_observation::registry::Tier::A)
            .map(|case| case.scenarios.len())
            .sum::<usize>();
        assert_eq!(guided.len(), tier_a);
        let g1 = items(Selection::Guided(Some(GuidedSession::G1))).unwrap();
        assert!(
            g1.iter()
                .all(|item| item.step.unwrap().session == GuidedSession::G1)
        );
    }
}
