//! The main worktree's side of recording on another machine: `export` writes an observation
//! task for the standalone recorder, `import` lands the bundle it wrote.

use std::collections::BTreeSet;
use std::path::Path;

use super::super::manual_observation::reduce::Binding;
use super::super::manual_observation::store::{self, Imported};
use super::super::manual_observation::transfer::{self, MAX_BUNDLE_BYTES, MAX_TASK_BYTES, Task};
use super::super::receipt;
use super::recorder::{Selection, items};
use super::{current_candidate, promoted_digest, unix_now};

/// Writes a task for `selection` to `task_path`. The candidate receipt, the context, `base`
/// and the selected scenarios' unresolved FAILs come from one snapshot under the shared
/// lock (a Promote takes the exclusive lock, so the candidate cannot change inside it).
pub(super) fn export(root: &Path, selection: Selection, task_path: &Path) -> Result<(), String> {
    let (_, digest) = current_candidate(root)?;
    let output = std::path::absolute(task_path)
        .map_err(|error| format!("cannot resolve {}: {error}", task_path.display()))?;
    crate::qualification::validate_public_evidence_path(root, &output)?;
    let items = items(selection)?;
    let selected = items
        .iter()
        .map(|item| (item.case.id, item.scenario))
        .collect::<BTreeSet<_>>();
    let task = {
        let _guard = store::lock(root, false)?;
        let (candidate, candidate_receipt_sha256, candidate_receipt) =
            receipt::read_candidate_text(root)?;
        if candidate_receipt_sha256 != digest {
            return Err(
                "the promoted candidate changed while this command ran; run it again".to_owned(),
            );
        }
        let context = store::load(root, &digest).map_err(|error| error.message())?;
        let binding = Binding {
            target: &context.target,
            exe_sha256: &candidate.exe_sha256,
        };
        let unresolved = transfer::unresolved(&context.log, binding)
            .into_iter()
            .filter(|failed| selected.contains(&(failed.case.as_str(), failed.scenario.as_str())))
            .collect();
        Task {
            context_id: context.target.context_id.clone(),
            base: context.log.observations.len() as u64,
            selection,
            registry_sha256: context.log.header.registry_sha256.clone(),
            candidate_receipt,
            candidate_receipt_sha256,
            candidate,
            target_receipt: context.target_text,
            target_sha256: context.log.header.target_sha256.clone(),
            target: context.target,
            unresolved,
            exported_at_unix: unix_now()?,
        }
    };
    let text = transfer::render_task(&task);
    if text.len() as u64 > MAX_TASK_BYTES {
        return Err(format!(
            "the observation task would exceed {MAX_TASK_BYTES} bytes; select fewer scenarios"
        ));
    }
    // A recorder must never meet a task it cannot read.
    transfer::parse_task(&text)?;
    crate::atomic_evidence::write(&output, text.as_bytes())?;
    println!(
        "MANUAL_TASK={} context={} base={} scenarios={} unresolved={}",
        output.display(),
        task.context_id,
        task.base,
        items.len(),
        task.unresolved.len()
    );
    Ok(())
}

/// Imports the bundle at `bundle_path` as a whole or not at all (`store::import`).
pub(super) fn import(root: &Path, bundle_path: &Path) -> Result<(), String> {
    let (candidate, digest) = current_candidate(root)?;
    let bytes = store::read_limited(bundle_path, MAX_BUNDLE_BYTES)?
        .ok_or_else(|| format!("{} does not exist", bundle_path.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", bundle_path.display()))?;
    let bundle = transfer::parse_bundle(&text)
        .map_err(|error| format!("{}: {error}", bundle_path.display()))?;
    let (session, task_sha256, recorded, explained) = (
        bundle.session.clone(),
        bundle.task_sha256.clone(),
        bundle.observations.len(),
        bundle.notes.len(),
    );
    let Imported {
        observations,
        notes,
    } = store::import(
        root,
        store::Promoted {
            receipt_sha256: &digest,
            exe_sha256: &candidate.exe_sha256,
        },
        &|| promoted_digest(root),
        bundle,
    )?;
    println!(
        "MANUAL_IMPORTED={} session={session} task={task_sha256} observations={observations} of {recorded} notes={notes} of {explained} (records already logged are skipped)",
        bundle_path.display()
    );
    Ok(())
}
