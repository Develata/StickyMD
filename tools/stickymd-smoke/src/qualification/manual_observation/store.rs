//! Files of a manual observation context: the target receipt, the log, the sidecar lock
//! and the archive. `dist/evidence/` belongs to one worktree, so each worktree has its own
//! context and lock.
//!
//! `approved` answers whether the USER approved a decision key, or why that cannot be read;
//! the release decision receipt supplies it, so this module does not read decisions
//! itself. It is called inside the exclusive lock, so the answer is current.
//!
//! Every change happens under the exclusive lock in this order: copy the current files into
//! the archive, replace the log, then replace the target. The active files are never
//! removed, so an interruption leaves either the old context, or a new log beside the old
//! target (a header mismatch that blocks like a lost log), or an empty orphan log after a
//! first declaration; none of them reads as a fresh start.
//!
//! Replacing a context that may hold observations needs the USER's approved
//! `RESET-MANUAL-LOG-<digest>` for the exact log bytes being replaced, and carries the
//! unresolved FAILs of whatever log still parses, matched or not, into the new context.
//!
//! A recorder session on this machine and an imported observation bundle land the same
//! way: under the exclusive lock, after the same candidate, context and role checks, as
//! one atomic replacement of the log.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use super::super::reserved_paths::MANUAL_ARCHIVE as ARCHIVE;
pub(in crate::qualification) use super::super::reserved_paths::{
    OBSERVATION_LOG, lock_manual_store as lock,
};
use super::log::{self, Carried, Header, Log, MAX_NOTE_BYTES, Note, Observation, Reset};
use super::reduce::{Binding, invalidity, unresolved_fails};
use super::registry;
use super::target::{self, RoleTarget, TARGET_RECEIPT, Target};
use super::transfer::Bundle;
use crate::integrity::sha256_bytes;

const MAX_LOG_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TARGET_BYTES: u64 = 64 * 1024;

/// The promoted candidate a command started with: the receipt digest its context is bound
/// to, and the EXE digest that receipt names, which every recorder-launched instance must
/// have run.
#[derive(Clone, Copy, Debug)]
pub(in crate::qualification) struct Promoted<'a> {
    pub(in crate::qualification) receipt_sha256: &'a str,
    pub(in crate::qualification) exe_sha256: &'a str,
}

/// A target receipt and its log, checked to belong together, to the candidate and to the
/// current registry.
#[derive(Debug)]
pub(in crate::qualification) struct Context {
    pub(in crate::qualification) target: Target,
    /// The target receipt exactly as stored: the log header binds its digest.
    pub(in crate::qualification) target_text: String,
    pub(in crate::qualification) log: Log,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::qualification) enum LoadError {
    /// No target has been declared in this worktree.
    NoContext,
    /// The declared context belongs to another candidate.
    OtherCandidate,
    /// A context exists but cannot be trusted (lost, empty, corrupt or mismatched log).
    Broken(String),
}

impl LoadError {
    pub(in crate::qualification) fn message(&self) -> String {
        match self {
            Self::NoContext => "no manual target receipt for this candidate; declare the targets with `stickymd-smoke acceptance manual target`".to_owned(),
            Self::OtherCandidate => "the manual target receipt belongs to another candidate; declare targets for this one".to_owned(),
            Self::Broken(reason) => format!("manual observation context is unusable: {reason}"),
        }
    }
}

/// Reads the context for `candidate_receipt_sha256`. The caller holds a lock.
pub(in crate::qualification) fn load(
    root: &Path,
    candidate_receipt_sha256: &str,
) -> Result<Context, LoadError> {
    let broken = LoadError::Broken;
    let target_bytes =
        read_limited(&root.join(TARGET_RECEIPT), MAX_TARGET_BYTES).map_err(broken)?;
    let log_bytes = read_limited(&root.join(OBSERVATION_LOG), MAX_LOG_BYTES).map_err(broken)?;
    let Some(target_bytes) = target_bytes else {
        return Err(match log_bytes {
            None => LoadError::NoContext,
            Some(_) => {
                LoadError::Broken("an observation log exists without its target receipt".to_owned())
            }
        });
    };
    let target_text = utf8(&target_bytes)
        .map_err(|error| LoadError::Broken(format!("target receipt: {error}")))?
        .to_owned();
    let target = target::parse(&target_text)
        .map_err(|error| LoadError::Broken(format!("target receipt: {error}")))?;
    if target.candidate_receipt_sha256 != candidate_receipt_sha256 {
        return Err(LoadError::OtherCandidate);
    }
    let log_bytes = log_bytes.ok_or_else(|| {
        LoadError::Broken("the observation log is missing (treated as lost)".to_owned())
    })?;
    if log_bytes.is_empty() {
        return Err(LoadError::Broken(
            "the observation log is empty (treated as lost)".to_owned(),
        ));
    }
    let log = utf8(&log_bytes)
        .and_then(log::parse)
        .map_err(|error| LoadError::Broken(format!("observation log: {error}")))?;
    let target_sha256 = sha256_bytes(&target_bytes).map_err(broken)?;
    let registry_sha256 = registry_sha256().map_err(broken)?;
    let header = &log.header;
    for (label, actual, expected) in [
        ("context", &header.context_id, &target.context_id),
        (
            "candidate",
            &header.candidate_receipt_sha256,
            &target.candidate_receipt_sha256,
        ),
        ("target receipt", &header.target_sha256, &target_sha256),
        ("manual registry", &header.registry_sha256, &registry_sha256),
    ] {
        if actual != expected {
            return Err(LoadError::Broken(format!(
                "the observation log's {label} does not match"
            )));
        }
    }
    Ok(Context {
        target,
        target_text,
        log,
    })
}

/// What a commit did when the store itself worked.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::qualification) enum Commit {
    Written,
    /// `last_check` refused; nothing was written.
    Refused(String),
}

/// Appends a recorder session's observations and notes to the context it started in. The
/// session is refused with an error, writing nothing, when the candidate was promoted
/// again, the context was replaced, or an observation's recorded environment does not
/// match the role it claims (plan 11 has recording use the same role predicate as
/// readiness). `last_check` runs under the lock after every other check and immediately
/// before the write, for facts that must hold when the answer lands (a PASS's instance);
/// `Ok(Some(reason))` refuses without writing.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(dead_code, reason = "only the Windows recorder records observations")
)]
pub(in crate::qualification) fn commit(
    root: &Path,
    candidate: Promoted,
    current_candidate: &dyn Fn() -> Result<String, String>,
    context_id: &str,
    observations: Vec<Observation>,
    notes: Vec<Note>,
    last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
) -> Result<Commit, String> {
    let _guard = lock(root, true)?;
    still_promoted(candidate.receipt_sha256, current_candidate)?;
    let context = load(root, candidate.receipt_sha256).map_err(|error| error.message())?;
    if context.target.context_id != context_id {
        return Err(
            "the manual observation context was replaced after this session started; start a new session"
                .to_owned(),
        );
    }
    Ok(
        match append_and_write(
            root,
            context,
            candidate.exe_sha256,
            observations,
            notes,
            last_check,
        )? {
            Appended::Written { .. } => Commit::Written,
            Appended::Refused(reason) => Commit::Refused(reason),
        },
    )
}

/// How many records an import added; records already logged are not counted.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Imported {
    pub(in crate::qualification) observations: usize,
    pub(in crate::qualification) notes: usize,
}

/// Imports an observation bundle as a whole or not at all. The bundle must name exactly
/// this context: its candidate receipt, context, target receipt and registry digests are
/// those of the log header, and its `base` lies within the log. Then it lands as a local
/// session does, so the role is recomputed from each observation's recorded environment,
/// every instance must have run the candidate EXE, and records already logged with the
/// same content are skipped (importing the same or a grown bundle again is safe).
pub(in crate::qualification) fn import(
    root: &Path,
    candidate: Promoted,
    current_candidate: &dyn Fn() -> Result<String, String>,
    bundle: Bundle,
) -> Result<Imported, String> {
    let _guard = lock(root, true)?;
    still_promoted(candidate.receipt_sha256, current_candidate)?;
    let context = load(root, candidate.receipt_sha256).map_err(|error| error.message())?;
    let header = &context.log.header;
    for (label, bundled, current) in [
        (
            "candidate receipt",
            &bundle.candidate_receipt_sha256,
            &header.candidate_receipt_sha256,
        ),
        ("context", &bundle.context_id, &header.context_id),
        (
            "target receipt",
            &bundle.target_sha256,
            &header.target_sha256,
        ),
        (
            "manual registry",
            &bundle.registry_sha256,
            &header.registry_sha256,
        ),
    ] {
        if bundled != current {
            return Err(format!(
                "the observation bundle's {label} is not this worktree's; it belongs to another candidate or context and was not imported"
            ));
        }
    }
    let length = context.log.observations.len() as u64;
    if bundle.base > length {
        return Err(format!(
            "the observation bundle starts after the log ({} > {length}); it was not exported from this log",
            bundle.base
        ));
    }
    match append_and_write(
        root,
        context,
        candidate.exe_sha256,
        bundle.observations,
        bundle.notes,
        &mut || Ok(None),
    )? {
        Appended::Written {
            observations,
            notes,
        } => Ok(Imported {
            observations,
            notes,
        }),
        Appended::Refused(reason) => Err(reason),
    }
}

enum Appended {
    Written { observations: usize, notes: usize },
    Refused(String),
}

/// Appends to the loaded `context` and replaces the log once; the caller holds the
/// exclusive lock. Every observation must be valid evidence for the role it claims, by the
/// predicate readiness uses.
fn append_and_write(
    root: &Path,
    mut context: Context,
    candidate_exe_sha256: &str,
    observations: Vec<Observation>,
    notes: Vec<Note>,
    last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
) -> Result<Appended, String> {
    let binding = Binding {
        target: &context.target,
        exe_sha256: candidate_exe_sha256,
    };
    if let Some(reason) = observations
        .iter()
        .find_map(|observation| invalidity(observation, binding))
    {
        return Err(reason);
    }
    let observations = context.log.append(observations)?;
    let notes = context.log.annotate(notes)?;
    let text = log::render(&context.log);
    if text.len() as u64 > MAX_LOG_BYTES {
        return Err(format!(
            "the observation log would exceed {MAX_LOG_BYTES} bytes"
        ));
    }
    if let Some(reason) = last_check()? {
        return Ok(Appended::Refused(reason));
    }
    crate::atomic_evidence::write(&root.join(OBSERVATION_LOG), text.as_bytes())?;
    Ok(Appended::Written {
        observations,
        notes,
    })
}

/// Declares new targets for the candidate and starts a fresh context. A context of another
/// candidate, or one of this candidate with no observations, is archived as is; any other
/// existing context needs an approved reset and hands its unresolved FAILs on.
pub(in crate::qualification) fn declare(
    root: &Path,
    candidate: Promoted,
    current_candidate: &dyn Fn() -> Result<String, String>,
    roles: [RoleTarget; 2],
    approved: &dyn Fn(&str) -> Result<bool, String>,
) -> Result<Target, String> {
    let _guard = lock(root, true)?;
    still_promoted(candidate.receipt_sha256, current_candidate)?;
    let carried = match load(root, candidate.receipt_sha256) {
        Err(LoadError::NoContext | LoadError::OtherCandidate) => Replacement::Plain,
        Ok(context) if context.log.observations.is_empty() => Replacement::Plain,
        Ok(context) => Replacement::Reset {
            log: Some(Box::new(context.log)),
            validated_by: Some(Box::new(context.target)),
        },
        // An interrupted first declaration leaves an empty log without a target.
        Err(LoadError::Broken(_)) if orphan_log_is_empty(root) => Replacement::Plain,
        Err(LoadError::Broken(_)) => Replacement::Reset {
            log: parsed_log(root),
            validated_by: None,
        },
    };
    replace(root, candidate, roles, carried, approved)
}

/// Starts a new context with the same targets after an approved reset, carrying the old
/// log's unresolved FAILs when it is still readable.
pub(in crate::qualification) fn reset(
    root: &Path,
    candidate: Promoted,
    current_candidate: &dyn Fn() -> Result<String, String>,
    approved: &dyn Fn(&str) -> Result<bool, String>,
) -> Result<Target, String> {
    let _guard = lock(root, true)?;
    still_promoted(candidate.receipt_sha256, current_candidate)?;
    let target_bytes = read_limited(&root.join(TARGET_RECEIPT), MAX_TARGET_BYTES)?
        .ok_or("no manual target receipt to reset; declare targets instead")?;
    let target = target::parse(utf8(&target_bytes)?)?;
    if target.candidate_receipt_sha256 != candidate.receipt_sha256 {
        return Err(
            "the manual context belongs to another candidate; declare targets for this one"
                .to_owned(),
        );
    }
    let replacement = match load(root, candidate.receipt_sha256) {
        Ok(context) => Replacement::Reset {
            log: Some(Box::new(context.log)),
            validated_by: Some(Box::new(context.target)),
        },
        Err(_) => Replacement::Reset {
            log: parsed_log(root),
            validated_by: None,
        },
    };
    replace(root, candidate, target.roles, replacement, approved)
}

/// The caller read the candidate before waiting for the lock (or for the USER's answers);
/// a Promote in between must not let a stale declaration replace the new candidate's
/// context. `current_candidate` re-reads the candidate receipt digest under the lock.
fn still_promoted(
    expected: &str,
    current_candidate: &dyn Fn() -> Result<String, String>,
) -> Result<(), String> {
    let current = current_candidate()?;
    if current == expected {
        Ok(())
    } else {
        Err("the promoted candidate changed while this command ran; run it again".to_owned())
    }
}

enum Replacement {
    /// Nothing worth keeping: archive and start over.
    Plain,
    /// The old log may hold observations. `log` is `Some` when it still parses;
    /// `validated_by` is the target it is bound to, `None` when the binding does not hold,
    /// in which case no PASS is trusted to resolve a FAIL.
    Reset {
        log: Option<Box<Log>>,
        validated_by: Option<Box<Target>>,
    },
}

fn replace(
    root: &Path,
    candidate: Promoted,
    roles: [RoleTarget; 2],
    replacement: Replacement,
    approved: &dyn Fn(&str) -> Result<bool, String>,
) -> Result<Target, String> {
    let (reset, carried, archive_label) = match replacement {
        Replacement::Plain => (None, (Vec::new(), Vec::new()), "replaced".to_owned()),
        Replacement::Reset { log, validated_by } => {
            // Streamed and unbounded: an oversized log can still be named and replaced.
            let path = root.join(OBSERVATION_LOG);
            let digest = if path.exists() {
                crate::integrity::sha256(&path)?
            } else {
                sha256_bytes(b"")?
            };
            let decision = format!("RESET-MANUAL-LOG-{digest}");
            if !approved(&decision)? {
                return Err(format!(
                    "the manual context may hold observations; replacing it needs USER decision {decision} approved"
                ));
            }
            let previous_context_id = log
                .as_ref()
                .map_or_else(|| "0".repeat(32), |log| log.header.context_id.clone());
            let carried = log
                .as_deref()
                .map(|log| {
                    carry(
                        log,
                        validated_by.as_deref().map(|target| Binding {
                            target,
                            exe_sha256: candidate.exe_sha256,
                        }),
                    )
                })
                .unwrap_or_default();
            let label = previous_context_id.clone();
            (
                Some(Reset {
                    decision,
                    previous_context_id,
                }),
                carried,
                label,
            )
        }
    };
    let target = Target {
        context_id: target::new_context_id(),
        candidate_receipt_sha256: candidate.receipt_sha256.to_owned(),
        roles,
    };
    let target_text = target::render(&target);
    let mut log = Log {
        header: Header {
            context_id: target.context_id.clone(),
            candidate_receipt_sha256: candidate.receipt_sha256.to_owned(),
            target_sha256: sha256_bytes(target_text.as_bytes())?,
            registry_sha256: registry_sha256()?,
            reset,
        },
        observations: Vec::new(),
        notes: Vec::new(),
    };
    let (carried, carried_notes) = carried;
    log.append(carried)?;
    log.annotate(carried_notes)?;
    let log_text = log::render(&log);
    if log_text.len() as u64 > MAX_LOG_BYTES {
        return Err(format!(
            "the new observation log would exceed {MAX_LOG_BYTES} bytes"
        ));
    }
    archive(root, &archive_label)?;
    crate::atomic_evidence::write(&root.join(OBSERVATION_LOG), log_text.as_bytes())?;
    crate::atomic_evidence::write(&root.join(TARGET_RECEIPT), target_text.as_bytes())?;
    Ok(target)
}

/// One FAIL per `(case, role, scenario)` that still has an unresolved failure, based on
/// its latest one and citing every unresolved original, with that one's note. The merge
/// keeps the new log far below its size limit whatever the old one held.
fn carry(old: &Log, validated_by: Option<Binding>) -> (Vec<Observation>, Vec<Note>) {
    let session = target::new_context_id();
    let mut notes = Vec::new();
    let observations = unresolved_fails(old, validated_by)
        .into_values()
        .zip(1_u64..)
        .map(|(indices, seq)| {
            let latest = *indices
                .iter()
                .max()
                .expect("unresolved FAILs are non-empty");
            let original = &old.observations[latest as usize];
            if let Some(note) = old
                .notes
                .iter()
                .find(|note| note.session == original.session && note.seq == original.seq)
            {
                notes.push(Note {
                    session: session.clone(),
                    seq,
                    text: truncate(
                        &format!(
                            "carried from context {}: {}",
                            old.header.context_id, note.text
                        ),
                        MAX_NOTE_BYTES,
                    ),
                });
            }
            let mut carried = original.clone();
            carried.session.clone_from(&session);
            carried.seq = seq;
            carried.base = 0;
            carried.carried_from = original
                .carried_from
                .iter()
                .cloned()
                .chain(indices.iter().map(|index| Carried {
                    context_id: old.header.context_id.clone(),
                    index: *index,
                }))
                .collect();
            carried
        })
        .collect();
    (observations, notes)
}

fn truncate(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Copies the current target receipt and log, whatever their state, into the archive.
/// They stay active until the new files replace them.
fn archive(root: &Path, label: &str) -> Result<(), String> {
    let present = [TARGET_RECEIPT, OBSERVATION_LOG]
        .into_iter()
        .filter(|relative| root.join(relative).exists())
        .collect::<Vec<_>>();
    if present.is_empty() {
        return Ok(());
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    let directory = root.join(ARCHIVE).join(format!("{label}-{nanos}"));
    fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    for relative in present {
        let source = root.join(relative);
        let name = source.file_name().expect("store files have names");
        fs::copy(&source, directory.join(name))
            .map_err(|error| format!("cannot archive {}: {error}", source.display()))?;
    }
    Ok(())
}

fn orphan_log_is_empty(root: &Path) -> bool {
    !root.join(TARGET_RECEIPT).exists()
        && parsed_log(root).is_some_and(|log| log.observations.is_empty())
}

/// The current log if it parses, whether or not its header matches the target.
fn parsed_log(root: &Path) -> Option<Box<Log>> {
    read_limited(&root.join(OBSERVATION_LOG), MAX_LOG_BYTES)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| log::parse_for_reset(&text).ok())
        .map(Box::new)
}

pub(in crate::qualification) fn registry_sha256() -> Result<String, String> {
    sha256_bytes(registry::canonical().as_bytes())
}

/// The file's bytes, `None` when it does not exist; more than `limit` bytes is an error,
/// checked while reading so a growing file cannot slip past.
pub(in crate::qualification) fn read_limited(
    path: &Path,
    limit: u64,
) -> Result<Option<Vec<u8>>, String> {
    refuse_link(path)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot open {}: {error}", path.display())),
    };
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{} exceeds {limit} bytes", path.display()));
    }
    Ok(Some(bytes))
}

/// Store files are plain files: a link would let another writer change what the store
/// reads without taking its lock.
fn refuse_link(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "{} is a link and is not trusted as manual observation evidence",
            path.display()
        )),
        _ => Ok(()),
    }
}

fn utf8(bytes: &[u8]) -> Result<&str, String> {
    std::str::from_utf8(bytes).map_err(|error| format!("not UTF-8: {error}"))
}

#[cfg(test)]
mod tests;
