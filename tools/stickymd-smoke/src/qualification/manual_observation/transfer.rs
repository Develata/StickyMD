//! Observations recorded on another machine: the observation task the main worktree exports
//! and the observation bundle the standalone recorder writes from it.
//!
//! - The task carries the candidate receipt and the target receipt as their exact text, so
//!   a recorder without the repository checks the candidate ZIP against the same bytes
//!   whose digests the log header binds. It also names the manual registry by digest (the
//!   recorder must be built from the same registry), the session `base` and the selected
//!   scenarios' unresolved FAILs, all from one log snapshot.
//! - The bundle is a partial log: the four digests of the log header, one recorder
//!   session's observations and notes. Import compares the digests with the current log
//!   header and lands the records as a local session does (`store::import`).
//!
//! Both files are parsed strictly and bounded before reading.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use std::collections::BTreeSet;

use super::super::json::{self, Value};
use super::super::receipt::{self, Candidate};
use super::log::{
    self, Log, MAX_NOTE_BYTES, Note, Observation, parse_note, parse_observation, render_note,
    render_observation, validate_note, validate_observation,
};
use super::reduce::{Binding, unresolved_fails};
use super::registry;
use super::target::{self, Role, Target, is_context_id};
use crate::cli::ManualSelection;
use crate::integrity::{sha256_bytes, validate_sha256};

pub(in crate::qualification) const MAX_TASK_BYTES: u64 = 1024 * 1024;
pub(in crate::qualification) const MAX_BUNDLE_BYTES: u64 = 1024 * 1024;

const TASK_KIND: &str = "stickymd-manual-observation-task";
const BUNDLE_KIND: &str = "stickymd-manual-observation-bundle";

/// A FAIL that a PASS of a session starting now resolves; shown before its retest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Unresolved {
    pub(in crate::qualification) case: String,
    pub(in crate::qualification) role: Role,
    pub(in crate::qualification) scenario: String,
    pub(in crate::qualification) session: String,
    pub(in crate::qualification) seq: u64,
    /// The FAIL's position in the log.
    pub(in crate::qualification) position: u64,
    pub(in crate::qualification) note: Option<String>,
}

/// The unresolved FAILs of `log` in reduction order, with their notes.
pub(in crate::qualification) fn unresolved(log: &Log, binding: Binding) -> Vec<Unresolved> {
    let mut entries = Vec::new();
    for ((case, role, scenario), positions) in unresolved_fails(log, Some(binding)) {
        for position in positions {
            let failed = &log.observations[position as usize];
            entries.push(Unresolved {
                case: case.clone(),
                role,
                scenario: scenario.clone(),
                session: failed.session.clone(),
                seq: failed.seq,
                position,
                note: log
                    .notes
                    .iter()
                    .find(|note| note.session == failed.session && note.seq == failed.seq)
                    .map(|note| note.text.clone()),
            });
        }
    }
    entries
}

/// What a recorder on another machine needs, taken from one snapshot of the context.
#[derive(Debug)]
pub(in crate::qualification) struct Task {
    pub(in crate::qualification) context_id: String,
    pub(in crate::qualification) base: u64,
    pub(in crate::qualification) selection: ManualSelection,
    pub(in crate::qualification) registry_sha256: String,
    pub(in crate::qualification) candidate_receipt: String,
    pub(in crate::qualification) candidate_receipt_sha256: String,
    #[cfg_attr(
        all(not(windows), not(test)),
        expect(dead_code, reason = "read by the Windows standalone recorder")
    )]
    pub(in crate::qualification) candidate: Candidate,
    pub(in crate::qualification) target_receipt: String,
    pub(in crate::qualification) target_sha256: String,
    #[cfg_attr(
        all(not(windows), not(test)),
        expect(dead_code, reason = "read by the Windows standalone recorder")
    )]
    pub(in crate::qualification) target: Target,
    pub(in crate::qualification) unresolved: Vec<Unresolved>,
    pub(in crate::qualification) exported_at_unix: u64,
}

pub(in crate::qualification) fn render_task(task: &Task) -> String {
    let unresolved = task
        .unresolved
        .iter()
        .map(|entry| {
            let note = entry.note.as_ref().map_or_else(
                || "null".to_owned(),
                |note| format!("\"{}\"", json::escape(note)),
            );
            format!(
                "{{\"case\":\"{}\",\"role\":\"{}\",\"scenario\":\"{}\",\"session\":\"{}\",\"seq\":{},\"position\":{},\"note\":{note}}}",
                json::escape(&entry.case),
                entry.role.as_str(),
                json::escape(&entry.scenario),
                entry.session,
                entry.seq,
                entry.position,
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\"schema_version\":1,\"kind\":\"{TASK_KIND}\",\"context_id\":\"{}\",\"base\":{},\"selection\":\"{}\",\"registry_sha256\":\"{}\",\"candidate_receipt_sha256\":\"{}\",\"candidate_receipt\":\"{}\",\"target_sha256\":\"{}\",\"target_receipt\":\"{}\",\"unresolved\":[\n{unresolved}\n],\"exported_at_unix\":{}}}\n",
        task.context_id,
        task.base,
        task.selection.as_str(),
        task.registry_sha256,
        task.candidate_receipt_sha256,
        json::escape(&task.candidate_receipt),
        task.target_sha256,
        json::escape(&task.target_receipt),
        task.exported_at_unix,
    )
}

/// Parses a task and checks that its parts belong together: each receipt's text has its
/// stated digest and parses, the target names the context and the candidate, and every
/// unresolved FAIL names a known scenario before `base`.
pub(in crate::qualification) fn parse_task(text: &str) -> Result<Task, String> {
    let document = json::parse_object(text)?;
    json::exact_keys(
        &document,
        &[
            "schema_version",
            "kind",
            "context_id",
            "base",
            "selection",
            "registry_sha256",
            "candidate_receipt_sha256",
            "candidate_receipt",
            "target_sha256",
            "target_receipt",
            "unresolved",
            "exported_at_unix",
        ],
    )?;
    if json::u64_field(&document, "schema_version")? != 1
        || json::string_field(&document, "kind")? != TASK_KIND
    {
        return Err("not a schema 1 manual observation task".to_owned());
    }
    let context_id = json::string_field(&document, "context_id")?;
    if !is_context_id(&context_id) {
        return Err(format!("task context `{context_id}` is not 32 hex digits"));
    }
    let base = json::u64_field(&document, "base")?;
    if base > log::MAX_OBSERVATIONS as u64 {
        return Err(format!("task base {base} exceeds the log limit"));
    }
    let candidate_receipt = json::string_field(&document, "candidate_receipt")?;
    let candidate_receipt_sha256 = digest_field(&document, "candidate_receipt_sha256")?;
    if sha256_bytes(candidate_receipt.as_bytes())? != candidate_receipt_sha256 {
        return Err("the task's candidate receipt does not have its stated digest".to_owned());
    }
    let candidate = receipt::parse_candidate(&candidate_receipt)
        .map_err(|error| format!("the task's candidate receipt: {error}"))?;
    let target_receipt = json::string_field(&document, "target_receipt")?;
    let target_sha256 = digest_field(&document, "target_sha256")?;
    if sha256_bytes(target_receipt.as_bytes())? != target_sha256 {
        return Err("the task's target receipt does not have its stated digest".to_owned());
    }
    let target = target::parse(&target_receipt)
        .map_err(|error| format!("the task's target receipt: {error}"))?;
    if target.context_id != context_id
        || target.candidate_receipt_sha256 != candidate_receipt_sha256
    {
        return Err("the task's target receipt names another context or candidate".to_owned());
    }
    let mut unresolved = Vec::new();
    for object in json::objects(&document, "unresolved")? {
        unresolved.push(parse_unresolved(object, base)?);
    }
    Ok(Task {
        context_id,
        base,
        selection: ManualSelection::parse(&json::string_field(&document, "selection")?)?,
        registry_sha256: digest_field(&document, "registry_sha256")?,
        candidate_receipt,
        candidate_receipt_sha256,
        candidate,
        target_receipt,
        target_sha256,
        target,
        unresolved,
        exported_at_unix: json::u64_field(&document, "exported_at_unix")?,
    })
}

fn parse_unresolved(object: &Value, base: u64) -> Result<Unresolved, String> {
    json::exact_keys(
        object,
        &[
            "case", "role", "scenario", "session", "seq", "position", "note",
        ],
    )?;
    let note = match json::field(object, "note")? {
        Value::Null => None,
        _ => Some(json::string_field(object, "note")?),
    };
    let entry = Unresolved {
        case: json::string_field(object, "case")?,
        role: Role::parse(&json::string_field(object, "role")?)?,
        scenario: json::string_field(object, "scenario")?,
        session: json::string_field(object, "session")?,
        seq: json::u64_field(object, "seq")?,
        position: json::u64_field(object, "position")?,
        note,
    };
    let known = registry::find(&entry.case)
        .is_some_and(|case| case.scenarios.contains(&entry.scenario.as_str()));
    if !known
        || !is_context_id(&entry.session)
        || entry.seq == 0
        || entry.position >= base
        || entry
            .note
            .as_ref()
            .is_some_and(|note| note.is_empty() || note.len() > MAX_NOTE_BYTES)
    {
        return Err(format!(
            "the task's unresolved FAIL {}#{} on {}/{} is malformed",
            entry.session, entry.seq, entry.case, entry.scenario
        ));
    }
    Ok(entry)
}

/// One standalone recorder session's records, bound to the context its task named.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Bundle {
    pub(in crate::qualification) task_sha256: String,
    pub(in crate::qualification) context_id: String,
    pub(in crate::qualification) candidate_receipt_sha256: String,
    pub(in crate::qualification) target_sha256: String,
    pub(in crate::qualification) registry_sha256: String,
    pub(in crate::qualification) base: u64,
    pub(in crate::qualification) session: String,
    pub(in crate::qualification) observations: Vec<Observation>,
    pub(in crate::qualification) notes: Vec<Note>,
}

impl Bundle {
    /// An empty bundle for session `session` started from `task`, whose text has digest
    /// `task_sha256`.
    #[cfg_attr(
        all(not(windows), not(test)),
        expect(dead_code, reason = "used by the Windows standalone recorder")
    )]
    pub(in crate::qualification) fn start(
        task: &Task,
        task_sha256: String,
        session: String,
    ) -> Self {
        Self {
            task_sha256,
            context_id: task.context_id.clone(),
            candidate_receipt_sha256: task.candidate_receipt_sha256.clone(),
            target_sha256: task.target_sha256.clone(),
            registry_sha256: task.registry_sha256.clone(),
            base: task.base,
            session,
            observations: Vec::new(),
            notes: Vec::new(),
        }
    }
}

pub(in crate::qualification) fn render_bundle(bundle: &Bundle) -> String {
    let observations = bundle
        .observations
        .iter()
        .map(render_observation)
        .collect::<Vec<_>>()
        .join(",\n");
    let notes = bundle
        .notes
        .iter()
        .map(render_note)
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\"schema_version\":2,\"kind\":\"{BUNDLE_KIND}\",\"task_sha256\":\"{}\",\"context_id\":\"{}\",\"candidate_receipt_sha256\":\"{}\",\"target_sha256\":\"{}\",\"registry_sha256\":\"{}\",\"base\":{},\"session\":\"{}\",\"observations\":[\n{observations}\n],\"notes\":[\n{notes}\n]}}\n",
        bundle.task_sha256,
        bundle.context_id,
        bundle.candidate_receipt_sha256,
        bundle.target_sha256,
        bundle.registry_sha256,
        bundle.base,
        bundle.session,
    )
}

/// Parses a bundle strictly: one session whose observations are numbered 1, 2, … in order,
/// all start at the bundle's `base`, none is carried, and every note explains one of them,
/// at most one note each.
pub(in crate::qualification) fn parse_bundle(text: &str) -> Result<Bundle, String> {
    let document = json::parse_object(text)?;
    json::exact_keys(
        &document,
        &[
            "schema_version",
            "kind",
            "task_sha256",
            "context_id",
            "candidate_receipt_sha256",
            "target_sha256",
            "registry_sha256",
            "base",
            "session",
            "observations",
            "notes",
        ],
    )?;
    if json::u64_field(&document, "schema_version")? != 2
        || json::string_field(&document, "kind")? != BUNDLE_KIND
    {
        return Err("not a schema 2 manual observation bundle".to_owned());
    }
    let context_id = json::string_field(&document, "context_id")?;
    let session = json::string_field(&document, "session")?;
    if !is_context_id(&context_id) || !is_context_id(&session) {
        return Err("the bundle's context or session is not 32 hex digits".to_owned());
    }
    let base = json::u64_field(&document, "base")?;
    if base > log::MAX_OBSERVATIONS as u64 {
        return Err(format!("the bundle's base {base} exceeds the log limit"));
    }
    let objects = json::objects(&document, "observations")?;
    if objects.len() > log::MAX_OBSERVATIONS {
        return Err("the bundle holds more observations than a log can".to_owned());
    }
    let mut observations = Vec::with_capacity(objects.len());
    for (position, object) in objects.iter().enumerate() {
        let observation = parse_observation(object)?;
        validate_observation(&observation, base)?;
        if observation.session != session
            || observation.seq != position as u64 + 1
            || observation.base != base
            || !observation.carried_from.is_empty()
        {
            return Err(format!(
                "bundle observation {}#{} is not the session's observation {} from base {base}",
                observation.session,
                observation.seq,
                position + 1
            ));
        }
        observations.push(observation);
    }
    let observed = observations
        .iter()
        .map(|observation| (observation.session.as_str(), observation.seq))
        .collect::<BTreeSet<_>>();
    let mut noted = BTreeSet::new();
    let mut notes = Vec::new();
    for object in json::objects(&document, "notes")? {
        let note = parse_note(object)?;
        validate_note(&note, &observed)?;
        if !noted.insert(note.seq) {
            return Err(format!("bundle observation #{} has two notes", note.seq));
        }
        notes.push(note);
    }
    drop(observed);
    Ok(Bundle {
        task_sha256: digest_field(&document, "task_sha256")?,
        context_id,
        candidate_receipt_sha256: digest_field(&document, "candidate_receipt_sha256")?,
        target_sha256: digest_field(&document, "target_sha256")?,
        registry_sha256: digest_field(&document, "registry_sha256")?,
        base,
        session,
        observations,
        notes,
    })
}

fn digest_field(object: &Value, key: &str) -> Result<String, String> {
    let value = json::string_field(object, key)?;
    validate_sha256(&value, key)?;
    Ok(value)
}

#[cfg(test)]
mod tests;
