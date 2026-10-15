//! The recorder's scenario loop, independent of the terminal, the tested machine and the
//! store so it runs under test on every platform.
//!
//! Per scenario: the observer prepares the environment; the recorder captures the facts,
//! derives the role (never asked), starts the instance the registry requires and asks for
//! an explicit status. `NOT TESTED` writes nothing. A PASS or FAIL is committed only when
//! a second capture equals the first, and a PASS also needs the launched process still
//! running as the only StickyMD when it lands: checked once to let the observer correct
//! the answer, and again under the store lock just before the write. The answer is
//! committed before the optional note is asked, so an interruption after a FAIL never
//! loses it; the note is its own record.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use super::super::guided::GuidedStep;
use super::super::manual_observation::environment::unchanged;
use super::super::manual_observation::log::{Instance, MAX_NOTE_BYTES, Note, Observation, Status};
use super::super::manual_observation::reduce::unresolved_fails;
use super::super::manual_observation::registry::{
    Case, FIRST_RUN_DECLARATIONS, Fixture, InstanceRule,
};
use super::super::manual_observation::role::{Facts, match_role};
use super::super::manual_observation::store::{Commit, Context};
use super::super::manual_observation::target::Role;

/// Longest declared value the log accepts.
const MAX_DECLARED_BYTES: usize = 128;

/// One scenario to observe, introduced by its guided step in a guided session.
pub(super) struct Item {
    pub(super) case: &'static Case,
    pub(super) scenario: &'static str,
    pub(super) step: Option<&'static GuidedStep>,
}

/// The machine and the store the loop works against.
pub(super) trait Bench {
    type Instance: Tested;
    fn capture(&mut self) -> Result<Facts, String>;
    /// Starts a fresh StickyMD seeded with `fixture`.
    fn launch(&mut self, fixture: Fixture) -> Result<Self::Instance, String>;
    /// Appends one observation to the log. `last_check` runs under the store lock just
    /// before the write and may refuse it; an error ends the session.
    fn commit(
        &mut self,
        observation: Observation,
        last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
    ) -> Result<Commit, String>;
    /// Appends one note; an error ends the session.
    fn annotate(&mut self, note: Note) -> Result<(), String>;
    fn now(&self) -> Result<u64, String>;
}

/// A StickyMD process the recorder started; dropping it stops the process.
pub(super) trait Tested {
    fn instance(&self) -> &Instance;
    /// Why the started candidate did not initialize, if it did not; a PASS is then refused
    /// but the observer may record the FAIL.
    fn startup_error(&self) -> Option<&str>;
    /// Whether this process still runs as the only StickyMD process.
    fn still_sole(&mut self) -> Result<bool, String>;
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Summary {
    pub(super) passed: usize,
    pub(super) failed: usize,
    /// Scenarios skipped, answered `NOT TESTED` or refused.
    pub(super) unrecorded: usize,
}

enum Next {
    Continue,
    Quit,
}

/// Runs `items` as one session with ID `session` that starts from `context`. The session's
/// `base` is the log length now, so its PASS resolves exactly the FAILs logged before it.
pub(super) fn run<B: Bench>(
    input: &mut impl BufRead,
    output: &mut impl Write,
    bench: &mut B,
    context: &Context,
    session: &str,
    items: &[Item],
) -> Result<Summary, String> {
    let mut recorder = Recorder {
        session,
        base: context.log.observations.len() as u64,
        next_seq: 1,
        context,
        unresolved: unresolved_fails(&context.log, Some(&context.target)),
        summary: Summary::default(),
    };
    let mut previous_step = None;
    for (position, item) in items.iter().enumerate() {
        if let Some(step) = item.step
            && previous_step != Some(step.id)
        {
            previous_step = Some(step.id);
            writeln!(
                output,
                "\nGuided {} step {} ({})\nWhat to do: {}\nExpected: {}",
                step.session.as_str(),
                step.id,
                step.case_ids.join(", "),
                step.action,
                step.expected
            )
            .map_err(io_error)?;
        }
        writeln!(
            output,
            "\n[{}/{}] {} [Tier {}] scenario `{}`\nWhat to do: {}\nExpected: {}\n{}",
            position + 1,
            items.len(),
            item.case.id,
            item.case.tier.as_str(),
            item.scenario,
            item.case.action,
            item.case.expected,
            rule_hint(item.case.instance)
        )
        .map_err(io_error)?;
        if let Next::Quit = recorder.scenario(input, output, bench, item)? {
            break;
        }
    }
    Ok(recorder.summary)
}

struct Recorder<'a> {
    session: &'a str,
    base: u64,
    next_seq: u64,
    context: &'a Context,
    unresolved: BTreeMap<(String, Role, String), Vec<u64>>,
    summary: Summary,
}

impl Recorder<'_> {
    fn scenario<B: Bench>(
        &mut self,
        input: &mut impl BufRead,
        output: &mut impl Write,
        bench: &mut B,
        item: &Item,
    ) -> Result<Next, String> {
        self.show_unresolved(output, item)?;
        loop {
            write!(
                output,
                "Prepare the environment, then press Enter to start (S = skip, Q = quit): "
            )
            .map_err(io_error)?;
            output.flush().map_err(io_error)?;
            match read_line(input)?.to_ascii_uppercase().as_str() {
                "" => break,
                "S" => return self.unrecorded(output, "skipped"),
                "Q" => {
                    self.summary.unrecorded += 1;
                    return Ok(Next::Quit);
                }
                _ => writeln!(output, "Press Enter, S or Q.").map_err(io_error)?,
            }
        }
        let start = match bench.capture() {
            Ok(facts) => facts,
            Err(error) => {
                return self.unrecorded(output, &format!("environment capture failed: {error}"));
            }
        };
        let role = match match_role(&start, &self.context.target) {
            Ok(role) => role,
            Err(error) => return self.unrecorded(output, &error),
        };
        writeln!(
            output,
            "Environment: Windows {} build {} {} {} -> role `{}`",
            start.display_version,
            start.build(),
            start.edition_id,
            start.native_machine.as_str(),
            role.as_str()
        )
        .map_err(io_error)?;
        let (declared, mut tested) = match item.case.instance {
            InstanceRule::FirstRunDeclared => match first_run_declarations(input, output)? {
                Some(declared) => (declared, None),
                None => {
                    return self.unrecorded(
                        output,
                        "a changed VM no longer shows the first run's environment",
                    );
                }
            },
            InstanceRule::Launched(fixture) | InstanceRule::Preserved(fixture) => {
                match bench.launch(fixture) {
                    Ok(tested) => {
                        match tested.startup_error() {
                            None => writeln!(
                                output,
                                "Started StickyMD PID {}; observe only this instance.",
                                tested.instance().pid
                            ),
                            Some(error) => writeln!(
                                output,
                                "StickyMD PID {} started but did not initialize: {error}. Record F if the candidate failed, or N; PASS is not accepted.",
                                tested.instance().pid
                            ),
                        }
                        .map_err(io_error)?;
                        (BTreeMap::new(), Some(tested))
                    }
                    Err(error) => {
                        return self.unrecorded(output, &format!("cannot start StickyMD: {error}"));
                    }
                }
            }
        };
        let status = loop {
            let Some(status) = prompt_status(input, output)? else {
                return self.unrecorded(output, "NOT TESTED");
            };
            if status == Status::Pass
                && let Some(tested) = tested.as_mut()
            {
                if tested.startup_error().is_some() {
                    writeln!(
                        output,
                        "PASS refused: StickyMD PID {} did not initialize. Enter F or N.",
                        tested.instance().pid
                    )
                    .map_err(io_error)?;
                    continue;
                }
                match tested.still_sole() {
                    Ok(true) => {}
                    Ok(false) => {
                        writeln!(
                            output,
                            "PASS refused: StickyMD PID {} is no longer running, or another StickyMD runs. Enter F or N.",
                            tested.instance().pid
                        )
                        .map_err(io_error)?;
                        continue;
                    }
                    Err(error) => {
                        return self
                            .unrecorded(output, &format!("cannot check the instance: {error}"));
                    }
                }
            }
            break status;
        };
        let end = match bench.capture() {
            Ok(facts) => facts,
            Err(error) => {
                return self.unrecorded(
                    output,
                    &format!("environment capture at commit failed: {error}"),
                );
            }
        };
        if let Err(error) = unchanged(&start, &end) {
            return self.unrecorded(output, &error);
        }
        let seq = self.next_seq;
        let observation = Observation {
            session: self.session.to_owned(),
            seq,
            base: self.base,
            case: item.case.id.to_owned(),
            scenario: item.scenario.to_owned(),
            role,
            status,
            observed: start,
            declared,
            instance: tested.as_ref().map(|tested| tested.instance().clone()),
            carried_from: Vec::new(),
            recorded_at_unix: bench.now()?,
        };
        let mut last_check = || match tested.as_mut() {
            Some(tested) if status == Status::Pass => Ok((!tested.still_sole()?).then(|| {
                format!(
                    "StickyMD PID {} is no longer running, or another StickyMD runs",
                    tested.instance().pid
                )
            })),
            _ => Ok(None),
        };
        if let Commit::Refused(reason) = bench.commit(observation, &mut last_check)? {
            return self.unrecorded(output, &format!("PASS refused at commit: {reason}"));
        }
        self.next_seq += 1;
        writeln!(
            output,
            "Recorded {} as {}#{seq}.",
            status.as_str(),
            self.session
        )
        .map_err(io_error)?;
        match status {
            Status::Pass => self.summary.passed += 1,
            Status::Fail => {
                self.summary.failed += 1;
                writeln!(
                    output,
                    "A retest needs a new recorder session: only a PASS from a session started after this FAIL resolves it."
                )
                .map_err(io_error)?;
            }
        }
        if let Some(text) = prompt_note(input, output)? {
            bench.annotate(Note {
                session: self.session.to_owned(),
                seq,
                text,
            })?;
        }
        // Dropping `tested` stops the instance.
        drop(tested);
        Ok(Next::Continue)
    }

    fn show_unresolved(&self, output: &mut impl Write, item: &Item) -> Result<(), String> {
        let log = &self.context.log;
        for role in Role::ALL {
            let key = (item.case.id.to_owned(), role, item.scenario.to_owned());
            for index in self.unresolved.get(&key).into_iter().flatten() {
                let failed = &log.observations[*index as usize];
                let note = log
                    .notes
                    .iter()
                    .find(|note| note.session == failed.session && note.seq == failed.seq)
                    .map_or("(no note)", |note| note.text.as_str());
                writeln!(
                    output,
                    "Unresolved FAIL on `{}`: {}#{} (log position {index}): {note}",
                    role.as_str(),
                    failed.session,
                    failed.seq
                )
                .map_err(io_error)?;
            }
        }
        Ok(())
    }

    fn unrecorded(&mut self, output: &mut impl Write, reason: &str) -> Result<Next, String> {
        self.summary.unrecorded += 1;
        writeln!(output, "Not recorded: {reason}.").map_err(io_error)?;
        Ok(Next::Continue)
    }
}

fn rule_hint(rule: InstanceRule) -> &'static str {
    match rule {
        InstanceRule::Launched(_) => {
            "The recorder starts a fresh StickyMD from the verified candidate for this scenario."
        }
        InstanceRule::Preserved(_) => {
            "The recorder starts StickyMD once; keep that process through the disconnect, sleep or reconnect. Do not restart it or replace its note."
        }
        InstanceRule::FirstRunDeclared => {
            "Answer for the first run in the Clean VM, made before this tool was copied in; the recorder then captures the VM's facts."
        }
    }
}

/// The M34 declarations, or `None` when the VM changed after the first run.
fn first_run_declarations(
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let [baseline, source, unchanged] = FIRST_RUN_DECLARATIONS;
    let mut declared = BTreeMap::new();
    for (key, question) in [
        (
            baseline,
            "VM baseline (image, build and how it was prepared)",
        ),
        (source, "How the StickyMD ZIP reached the VM"),
    ] {
        let answer = loop {
            write!(output, "{question}: ").map_err(io_error)?;
            output.flush().map_err(io_error)?;
            let answer = read_line(input)?;
            if !answer.is_empty() && answer.len() <= MAX_DECLARED_BYTES {
                break answer;
            }
            writeln!(output, "Enter 1 to {MAX_DECLARED_BYTES} bytes.").map_err(io_error)?;
        };
        declared.insert(key.to_owned(), answer);
    }
    loop {
        write!(
            output,
            "Is the VM unchanged since the first run (no updates, installs or settings changes)? [Y/N]: "
        )
        .map_err(io_error)?;
        output.flush().map_err(io_error)?;
        match read_line(input)?.to_ascii_uppercase().as_str() {
            "Y" | "YES" => {
                declared.insert(unchanged.to_owned(), "yes".to_owned());
                return Ok(Some(declared));
            }
            "N" | "NO" => return Ok(None),
            _ => writeln!(output, "Enter Y or N.").map_err(io_error)?,
        }
    }
}

/// An explicit status; `None` is `NOT TESTED`, which is never recorded.
pub(super) fn prompt_status(
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Option<Status>, String> {
    loop {
        write!(output, "Status [P=PASS, F=FAIL, N=NOT TESTED]: ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        match read_line(input)?.to_ascii_uppercase().as_str() {
            "P" | "PASS" => return Ok(Some(Status::Pass)),
            "F" | "FAIL" => return Ok(Some(Status::Fail)),
            "N" | "NOT TESTED" | "NOT_TESTED" => return Ok(None),
            _ => writeln!(output, "Enter P, F, or N explicitly.").map_err(io_error)?,
        }
    }
}

fn prompt_note(
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Option<String>, String> {
    loop {
        write!(output, "Note (optional; Enter to leave blank): ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        let note = read_line(input)?;
        if note.len() <= MAX_NOTE_BYTES {
            return Ok((!note.is_empty()).then_some(note));
        }
        writeln!(output, "Notes hold at most {MAX_NOTE_BYTES} bytes.").map_err(io_error)?;
    }
}

pub(super) fn read_line(input: &mut impl BufRead) -> Result<String, String> {
    let mut line = String::new();
    let bytes = input.read_line(&mut line).map_err(io_error)?;
    if bytes == 0 {
        return Err("manual input reached end of input".to_owned());
    }
    Ok(line.trim().to_owned())
}

pub(super) fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests;
