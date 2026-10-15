//! The standalone recorder (`acceptance manual record`) for a tested machine that has only
//! this tool, an exported observation task and the candidate ZIP: no repository, Rust or
//! extra runtime. It checks the ZIP against the candidate receipt the task carries, runs
//! the same scenario loop as the local recorder and writes every answer to an observation
//! bundle before asking the next question, so an interruption keeps what was answered.
//! The main worktree imports the bundle (`acceptance manual import`).

use std::path::{Path, PathBuf};

use super::super::manual_observation::log::{Note, Observation};
use super::super::manual_observation::store::Commit;
use super::super::manual_observation::transfer::{self, Bundle, MAX_BUNDLE_BYTES};

#[cfg(not(windows))]
pub(super) fn record(_: &crate::cli::StandaloneRecord) -> Result<(), String> {
    Err("recording manual observations requires Windows".to_owned())
}

#[cfg(windows)]
pub(super) use live::record;

/// The bundle file of one session and what it holds.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(dead_code, reason = "only the Windows recorder writes bundles")
)]
struct BundleFile {
    path: PathBuf,
    bundle: Bundle,
}

#[cfg_attr(
    all(not(windows), not(test)),
    expect(dead_code, reason = "only the Windows recorder writes bundles")
)]
impl BundleFile {
    /// Writes the empty `bundle` to `path`, which must not exist: an earlier bundle is never
    /// replaced.
    fn claim(path: PathBuf, bundle: Bundle) -> Result<Self, String> {
        crate::atomic_evidence::write_new(&path, transfer::render_bundle(&bundle).as_bytes())?;
        Ok(Self { path, bundle })
    }

    fn add_observation(
        &mut self,
        observation: Observation,
        last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
    ) -> Result<Commit, String> {
        let mut next = self.bundle.clone();
        next.observations.push(observation);
        self.publish(next, last_check)
    }

    fn add_note(&mut self, note: Note) -> Result<(), String> {
        let mut next = self.bundle.clone();
        next.notes.push(note);
        match self.publish(next, &mut || Ok(None))? {
            Commit::Written => Ok(()),
            Commit::Refused(reason) => Err(reason),
        }
    }

    /// Replaces the file with `next`, or refuses when it would exceed what an import
    /// accepts; `last_check` runs immediately before the write.
    fn publish(
        &mut self,
        next: Bundle,
        last_check: &mut dyn FnMut() -> Result<Option<String>, String>,
    ) -> Result<Commit, String> {
        let text = transfer::render_bundle(&next);
        if text.len() as u64 > MAX_BUNDLE_BYTES {
            return Err(format!(
                "the observation bundle would exceed {MAX_BUNDLE_BYTES} bytes, so this answer was not recorded; import {} and export a new task",
                self.path.display()
            ));
        }
        if let Some(reason) = last_check()? {
            return Ok(Commit::Refused(reason));
        }
        crate::atomic_evidence::write(&self.path, text.as_bytes())?;
        self.bundle = next;
        Ok(Commit::Written)
    }
}

/// A bundle written inside a repository obeys that repository's evidence path guard, so it
/// cannot replace the manual store or release decisions there. The guard is applied to the
/// path as given and to its existing directory with links resolved: a link outside every
/// repository can still lead into one. The directory must exist.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(dead_code, reason = "only the Windows recorder writes bundles")
)]
fn guard_inside_repository(path: &Path) -> Result<(), String> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(format!("{} names no bundle file", path.display()));
    };
    let resolved = std::fs::canonicalize(parent)
        .map_err(|error| format!("cannot resolve {}: {error}", parent.display()))?;
    for (directory, bundle) in [
        (parent.to_path_buf(), path.to_path_buf()),
        (resolved.clone(), resolved.join(name)),
    ] {
        if let Ok(root) = crate::governance::find_repository_root(&directory) {
            crate::qualification::validate_public_evidence_path(&root, &bundle)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
mod live {
    use std::io::{self, Write};

    use crate::cli::StandaloneRecord;
    use crate::integrity::sha256_bytes;
    use crate::qualification_environment::{self, QualificationEnvironmentStatus};

    use super::super::super::manual_observation::log::{Note, Observation};
    use super::super::super::manual_observation::registry::{Fixture, InstanceRule};
    use super::super::super::manual_observation::role::Facts;
    use super::super::super::manual_observation::store::{self, Commit};
    use super::super::super::manual_observation::transfer::{self, Bundle, MAX_TASK_BYTES};
    use super::super::super::manual_observation::{environment, target};
    use super::super::super::receipt;
    use super::super::flow::{self, Bench, Start, io_error};
    use super::super::instance::{Launched, Workshop};
    use super::super::recorder::items;
    use super::{BundleFile, guard_inside_repository};

    pub(in super::super) fn record(options: &StandaloneRecord) -> Result<(), String> {
        super::super::require_terminal()?;
        let task_bytes = store::read_limited(&options.task, MAX_TASK_BYTES)?
            .ok_or_else(|| format!("{} does not exist", options.task.display()))?;
        let task_sha256 = sha256_bytes(&task_bytes)?;
        let task = std::str::from_utf8(&task_bytes)
            .map_err(|error| format!("{} is not UTF-8: {error}", options.task.display()))
            .and_then(transfer::parse_task)
            .map_err(|error| format!("{}: {error}", options.task.display()))?;
        if task.registry_sha256 != store::registry_sha256()? {
            return Err(
                "this stickymd-smoke was built from another manual registry than the task's; build it from the exporting worktree's commit"
                    .to_owned(),
            );
        }
        let bundle_path = std::path::absolute(&options.bundle)
            .map_err(|error| format!("cannot resolve {}: {error}", options.bundle.display()))?;
        guard_inside_repository(&bundle_path)?;
        let items = items(task.selection)?;
        let desktop = qualification_environment::inspect();
        if desktop.status != QualificationEnvironmentStatus::Valid {
            return Err(format!(
                "manual observation needs an unlocked interactive Windows desktop: {}",
                desktop.summary()
            ));
        }
        let launches = items
            .iter()
            .any(|item| item.case.instance != InstanceRule::FirstRunDeclared);
        let workshop = if launches {
            Some(Workshop::prepare(&options.zip, &task.candidate)?)
        } else {
            // Only the M34 first-run declaration: nothing starts, but the ZIP on this
            // machine must still be the candidate's.
            if receipt::sha256(&options.zip)? != task.candidate.zip_sha256 {
                return Err(format!(
                    "{} does not match the candidate ZIP {}",
                    options.zip.display(),
                    task.candidate.zip_sha256
                ));
            }
            None
        };
        let session = target::new_context_id();
        let mut bench = BundleBench {
            file: BundleFile::claim(
                bundle_path,
                Bundle::start(&task, task_sha256, session.clone()),
            )?,
            workshop,
        };
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut output = io::stdout().lock();
        writeln!(
            output,
            "Standalone manual observation session {session}: context {}, task base {}. Every answer is written to {} at once.",
            task.context_id,
            task.base,
            bench.file.path.display()
        )
        .map_err(io_error)?;
        let start = Start {
            target: &task.target,
            base: task.base,
            unresolved: &task.unresolved,
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
            "MANUAL_BUNDLE={} passed={} failed={} unrecorded={}\nImport it in the exporting worktree: stickymd-smoke acceptance manual import --bundle=<path>",
            bench.file.path.display(),
            summary.passed,
            summary.failed,
            summary.unrecorded
        )
        .map_err(io_error)
    }

    struct BundleBench {
        file: BundleFile,
        workshop: Option<Workshop>,
    }

    impl Bench for BundleBench {
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
            self.file.add_observation(observation, last_check)
        }

        fn annotate(&mut self, note: Note) -> Result<(), String> {
            self.file.add_note(note)
        }

        fn now(&self) -> Result<u64, String> {
            super::super::unix_now()
        }
    }
}

#[cfg(test)]
mod tests;
