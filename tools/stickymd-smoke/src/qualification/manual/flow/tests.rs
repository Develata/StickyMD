//! The scenario loop against a scripted observer, machine and store.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;
use std::rc::Rc;

use super::{Bench, Item, Preparation, Start, Summary, Tested, run};
use crate::qualification::guided::STEPS;
use crate::qualification::manual_observation::log::{
    Instance, Log, Note, Observation, Status, fixtures,
};
use crate::qualification::manual_observation::reduce::Binding;
use crate::qualification::manual_observation::registry::{self, InstanceRule};
use crate::qualification::manual_observation::role::{Facts, fixtures::current};
use crate::qualification::manual_observation::store::{Commit, Context};
use crate::qualification::manual_observation::target::{self, Role, fixtures::target};
use crate::qualification::manual_observation::transfer::unresolved;

const SESSION: &str = "abcdefabcdefabcdefabcdefabcdef00";

/// What happened, in order, across the bench and its instances.
type Events = Rc<RefCell<Vec<String>>>;

struct FakeBench {
    captures: VecDeque<Result<Facts, String>>,
    /// The store fails every write with this error.
    broken_store: Option<String>,
    /// Whether each launched instance still runs alone when a PASS is checked.
    sole: VecDeque<bool>,
    /// The next launched instance starts but does not initialize.
    startup_error: Option<String>,
    preparation_error: Option<String>,
    next_pid: u32,
    events: Events,
    observations: Vec<Observation>,
    notes: Vec<Note>,
}

impl FakeBench {
    fn new(captures: Vec<Result<Facts, String>>) -> Self {
        Self {
            captures: captures.into(),
            broken_store: None,
            sole: VecDeque::new(),
            startup_error: None,
            preparation_error: None,
            next_pid: 1000,
            events: Events::default(),
            observations: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn events(&self) -> Vec<String> {
        self.events.borrow().clone()
    }
}

struct FakeInstance {
    preparation_error: Option<String>,
    instances: Vec<Instance>,
    sole: VecDeque<bool>,
    startup_error: Option<String>,
    events: Events,
}

impl Tested for FakeInstance {
    fn prepare(
        &mut self,
        item: &Item,
        _: &mut dyn std::io::BufRead,
        _: &mut dyn std::io::Write,
    ) -> Result<Preparation, String> {
        if let Some(error) = &self.preparation_error {
            return Err(error.clone());
        }
        Ok(Preparation {
            declared: crate::qualification::manual_observation::procedure::fixture(
                item.case.id,
                item.scenario,
            ),
            pass_block: None,
        })
    }

    fn instances(&self) -> &[Instance] {
        &self.instances
    }

    fn startup_error(&self) -> Option<&str> {
        self.startup_error.as_deref()
    }

    fn still_exact(&mut self) -> Result<bool, String> {
        Ok(self.sole.pop_front().unwrap_or(true))
    }
}

impl Drop for FakeInstance {
    fn drop(&mut self) {
        self.events
            .borrow_mut()
            .push(format!("stop {}", self.instances[0].pid));
    }
}

impl Bench for FakeBench {
    type Instance = FakeInstance;

    fn capture(&mut self) -> Result<Facts, String> {
        self.events.borrow_mut().push("capture".to_owned());
        self.captures.pop_front().expect("scripted capture")
    }

    fn launch(&mut self, rule: InstanceRule) -> Result<FakeInstance, String> {
        let fixture = match rule {
            InstanceRule::Launched(f) | InstanceRule::Preserved(f) | InstanceRule::TwoCopies(f) => {
                f
            }
            InstanceRule::FirstRunDeclared => panic!("first run cannot launch"),
        };
        self.next_pid += 1;
        self.events
            .borrow_mut()
            .push(format!("launch {} {fixture:?}", self.next_pid));
        Ok(FakeInstance {
            preparation_error: self.preparation_error.clone(),
            instances: (0..crate::qualification::manual_observation::instances::count(rule))
                .map(|index| Instance {
                    exe_sha256: "a".repeat(64),
                    pid: self.next_pid + index as u32,
                })
                .collect(),
            sole: std::mem::take(&mut self.sole),
            startup_error: self.startup_error.take(),
            events: Rc::clone(&self.events),
        })
    }

    fn commit(
        &mut self,
        observation: Observation,
        last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
    ) -> Result<Commit, String> {
        if let Some(error) = &self.broken_store {
            return Err(error.clone());
        }
        if let Some(reason) = last_check()? {
            self.events.borrow_mut().push("refused".to_owned());
            return Ok(Commit::Refused(reason));
        }
        self.events.borrow_mut().push(format!(
            "commit {}/{} {}",
            observation.case,
            observation.scenario,
            observation.status.as_str()
        ));
        self.observations.push(observation);
        Ok(Commit::Written)
    }

    fn annotate(&mut self, note: Note) -> Result<(), String> {
        self.events.borrow_mut().push(format!("note {}", note.seq));
        self.notes.push(note);
        Ok(())
    }

    fn now(&self) -> Result<u64, String> {
        Ok(1_791_900_000)
    }
}

fn context(log: Log) -> Context {
    Context {
        target: target(),
        target_text: target::render(&target()),
        log,
    }
}

fn item(case: &str, scenario: &'static str) -> Item {
    Item {
        case: registry::find(case).unwrap(),
        scenario,
        step: None,
    }
}

fn record(
    bench: &mut FakeBench,
    context: &Context,
    items: &[Item],
    answers: &str,
) -> (Result<Summary, String>, String) {
    let mut output = Vec::new();
    // As the local recorder starts a session from its log.
    let unresolved = unresolved(
        &context.log,
        Binding {
            target: &context.target,
            exe_sha256: fixtures::EXE,
        },
    );
    let start = Start {
        target: &context.target,
        base: context.log.observations.len() as u64,
        unresolved: &unresolved,
    };
    let result = run(
        &mut Cursor::new(answers.as_bytes().to_vec()),
        &mut output,
        bench,
        &start,
        SESSION,
        items,
    );
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn not_tested_writes_nothing() {
    let mut bench = FakeBench::new(vec![Ok(current())]);
    let (result, _) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M25", "formulas")],
        "\nN\n",
    );
    assert_eq!(
        result.unwrap(),
        Summary {
            passed: 0,
            failed: 0,
            unrecorded: 1
        }
    );
    assert!(bench.observations.is_empty() && bench.notes.is_empty());
    assert_eq!(
        bench.events(),
        ["capture", "launch 1001 Rendering", "stop 1001"]
    );
}

/// The FAIL is on disk before the note is asked: input ending there loses nothing.
#[test]
fn a_fail_is_committed_before_its_note_is_asked() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    let (result, output) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M25", "formulas")],
        "\nF\n",
    );
    assert!(result.unwrap_err().contains("end of input"));
    assert!(output.contains("new recorder session"), "{output}");
    assert_eq!(
        bench.events(),
        [
            "capture",
            "launch 1001 Rendering",
            "capture",
            "commit P12-M25/formulas MANUAL_FAIL",
            "stop 1001"
        ]
    );
    let failed = &bench.observations[0];
    assert_eq!(
        (failed.session.as_str(), failed.seq, failed.base),
        (SESSION, 1, 0)
    );
    assert_eq!((failed.role, failed.status), (Role::Current, Status::Fail));
    assert_eq!(failed.observed, current());
    assert_eq!(failed.instances[0].pid, 1001);
}

#[test]
fn a_note_is_its_own_record_after_the_answer() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    let (result, _) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M25", "formulas")],
        "\nF\nerror state not shown\n",
    );
    assert_eq!(result.unwrap().failed, 1);
    assert_eq!(
        bench.notes,
        [Note {
            session: SESSION.to_owned(),
            seq: 1,
            text: "error state not shown".to_owned()
        }]
    );
    let events = bench.events();
    let position = |wanted: &str| events.iter().position(|event| event == wanted).unwrap();
    assert!(position("commit P12-M25/formulas MANUAL_FAIL") < position("note 1"));
}

#[test]
fn an_environment_that_changes_or_matches_no_role_records_nothing() {
    let mut updated = current();
    updated.ubr = 7020;
    for (captures, launched) in [
        // Changed between start and commit.
        (vec![Ok(current()), Ok(updated.clone())], true),
        // Failed at commit.
        (vec![Ok(current()), Err("registry".to_owned())], true),
        // Failed at start, or an unapproved build: nothing is launched.
        (vec![Err("registry".to_owned())], false),
        (vec![Ok(updated.clone())], false),
    ] {
        let mut bench = FakeBench::new(captures);
        let (result, output) = record(
            &mut bench,
            &context(fixtures::empty()),
            &[item("P12-M25", "formulas")],
            "\nF\n",
        );
        assert_eq!(result.unwrap().unrecorded, 1, "{output}");
        assert!(bench.observations.is_empty(), "{output}");
        assert_eq!(
            bench
                .events()
                .iter()
                .any(|event| event.starts_with("launch")),
            launched
        );
    }
}

#[test]
fn a_pass_needs_the_launched_instance_still_running_alone() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    bench.sole = VecDeque::from([false]);
    let (result, output) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M41", "sleep-resume")],
        "\nP\nF\n\n",
    );
    assert_eq!(result.unwrap().failed, 1);
    assert!(output.contains("PASS refused"), "{output}");
    assert_eq!(bench.observations[0].status, Status::Fail);
}

/// The instance can exit while the answer is being committed: the check under the store
/// lock refuses the PASS even though the earlier check passed.
#[test]
fn a_pass_is_rechecked_when_it_lands() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    bench.sole = VecDeque::from([true, false]);
    let (result, output) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M24", "representative-markdown")],
        "\nP\n",
    );
    assert_eq!(result.unwrap().unrecorded, 1);
    assert!(output.contains("PASS refused at commit"), "{output}");
    assert!(bench.observations.is_empty());
    assert!(bench.events().contains(&"refused".to_owned()));
}

/// A store that cannot write ends the session with its error: the command exits non-zero.
#[test]
fn a_failed_write_ends_the_session() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    bench.broken_store = Some("disk full".to_owned());
    let (result, _) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[
            item("P12-M25", "formulas"),
            item("P12-M26", "image-formats"),
        ],
        "\nF\n\n\nP\n\n",
    );
    assert_eq!(result.unwrap_err(), "disk full");
    // The second scenario never started, and the instance was stopped.
    assert_eq!(
        bench.events(),
        ["capture", "launch 1001 Rendering", "capture", "stop 1001"]
    );
}

/// A candidate that starts but never initializes may be the product failing: the observer
/// can record that FAIL against the started instance, but not a PASS.
#[test]
fn a_started_candidate_that_does_not_initialize_can_be_failed_not_passed() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    bench.startup_error = Some("timed out waiting for portable note/config layout".to_owned());
    let (result, output) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M24", "representative-markdown")],
        "\nP\nF\nno window appeared\n",
    );
    assert_eq!(result.unwrap().failed, 1);
    assert!(
        output.contains("started but did not initialize") && output.contains("PASS refused"),
        "{output}"
    );
    let failed = &bench.observations[0];
    assert_eq!(failed.status, Status::Fail);
    assert_eq!(failed.instances[0].pid, 1001);
    assert_eq!(bench.notes[0].text, "no window appeared");
}

#[test]
fn the_first_run_is_declared_without_an_instance() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    let (result, _) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M34", "clean-vm-first-run")],
        "\nWin11 25H2 eval image\nbrowser download\nY\nP\n\n",
    );
    assert_eq!(result.unwrap().passed, 1);
    let observed = &bench.observations[0];
    assert!(observed.instances.is_empty());
    assert_eq!(observed.declared["unchanged-since-first-run"], "yes");
    assert_eq!(observed.declared["package-source"], "browser download");
    assert!(
        !bench
            .events()
            .iter()
            .any(|event| event.starts_with("launch"))
    );

    // A VM changed since the first run cannot speak for it.
    let mut bench = FakeBench::new(vec![Ok(current())]);
    let (result, _) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[item("P12-M34", "clean-vm-first-run")],
        "\nimage\ncopy\nN\n",
    );
    assert_eq!(result.unwrap().unrecorded, 1);
    assert!(bench.observations.is_empty());
}

/// A scenario whose recorder procedure has not landed is refused before any prompt, capture
/// or launch, so no observation can claim a procedure the recorder did not run.
#[test]
fn missing_procedure_evidence_is_unrecorded_and_stops_owned_instances() {
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current()), Ok(current())]);
    bench.preparation_error = Some("required precondition missing".to_owned());
    let (result, output) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[
            item("P12-M05", "second-launch-hidden"),
            item("P12-M05", "exit-save-failure"),
            item("P12-M35", "secondary-below"),
        ],
        "\n\n\n",
    );
    assert_eq!(result.unwrap().unrecorded, 3);
    assert!(bench.observations.is_empty());
    assert_eq!(
        bench
            .events()
            .iter()
            .filter(|event| event.starts_with("stop "))
            .count(),
        3
    );
    assert_eq!(output.matches("procedure evidence unavailable").count(), 3);
}

#[test]
fn two_copy_pass_requires_both_original_processes_at_commit() {
    for checks in [vec![true, true], vec![true, false], vec![false]] {
        let accepted = checks == [true, true];
        let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
        bench.sole = checks.into();
        let (result, _) = record(
            &mut bench,
            &context(fixtures::empty()),
            &[item("P12-M45", "tray-identity")],
            "\nP\nN\n",
        );
        assert_eq!(result.unwrap().passed, usize::from(accepted));
        if accepted {
            let observed = &bench.observations[0];
            assert_eq!(observed.instances.len(), 2);
            assert_ne!(observed.instances[0].pid, observed.instances[1].pid);
            crate::qualification::manual_observation::log::validate_observation(observed, 0)
                .unwrap();
        } else {
            assert!(bench.observations.is_empty());
        }
    }
}

/// A guided step that covers several cases asks for each case and scenario on its own.
#[test]
fn a_guided_step_is_answered_case_by_case() {
    let step = STEPS.iter().find(|step| step.id == "G2-04").unwrap();
    let items = step
        .case_ids
        .iter()
        .flat_map(|case| {
            let case = registry::find(case).unwrap();
            case.scenarios.iter().map(move |scenario| Item {
                case,
                scenario,
                step: Some(step),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 4);
    let mut bench = FakeBench::new(vec![Ok(current()); 8]);
    let (result, output) = record(
        &mut bench,
        &context(fixtures::empty()),
        &items,
        "\nP\n\n\nP\n\n\nF\n\n\nP\n\n",
    );
    assert_eq!(
        result.unwrap(),
        Summary {
            passed: 3,
            failed: 1,
            unrecorded: 0
        }
    );
    assert_eq!(
        output.matches("Guided G2 step G2-04").count(),
        1,
        "{output}"
    );
    let answered = bench
        .observations
        .iter()
        .map(|observation| {
            (
                observation.case.as_str(),
                observation.scenario.as_str(),
                observation.seq,
                observation.status,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        answered,
        [
            ("P12-M11", "left-dock-mixed-dpi", 1, Status::Pass),
            ("P12-M11", "undock-16dip", 2, Status::Pass),
            ("P12-M12", "right-dock-mixed-dpi", 3, Status::Fail),
            ("P12-M12", "undock-16dip", 4, Status::Pass),
        ]
    );
}

/// The observer sees the FAILs a retest is for, and the retest's `base` covers them.
#[test]
fn a_retest_shows_the_unresolved_fail_and_starts_after_it() {
    let mut log = fixtures::empty();
    log.append(vec![fixtures::observation(
        1,
        0,
        "P12-M25",
        "formulas",
        Role::Current,
        Status::Fail,
    )])
    .unwrap();
    log.annotate(vec![Note {
        session: fixtures::SESSION.to_owned(),
        seq: 1,
        text: "matrix misaligned".to_owned(),
    }])
    .unwrap();
    let mut bench = FakeBench::new(vec![Ok(current()), Ok(current())]);
    let (result, output) = record(
        &mut bench,
        &context(log),
        &[item("P12-M25", "formulas")],
        "\nP\n\n",
    );
    assert_eq!(result.unwrap().passed, 1);
    assert!(
        output.contains("Unresolved FAIL on `current`") && output.contains("matrix misaligned"),
        "{output}"
    );
    assert_eq!(bench.observations[0].base, 1);
}

#[test]
fn skip_and_quit_record_nothing() {
    let mut bench = FakeBench::new(Vec::new());
    let (result, _) = record(
        &mut bench,
        &context(fixtures::empty()),
        &[
            item("P12-M24", "representative-markdown"),
            item("P12-M25", "formulas"),
            item("P12-M26", "image-formats"),
        ],
        "S\nQ\n",
    );
    assert_eq!(result.unwrap().unrecorded, 2);
    assert!(bench.events().is_empty());
}
