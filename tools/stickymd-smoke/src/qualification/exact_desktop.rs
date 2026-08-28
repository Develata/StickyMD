//! Shared exact-candidate desktop qualification lifecycle and evidence binding.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::qualification_environment::{self, QualificationEnvironmentStatus};

pub(super) use crate::managed_process::ChildGuard;

use super::receipt;

mod evidence;

pub(super) const TIMEOUT: Duration = Duration::from_secs(12);

pub(super) type CaseOperation = fn(&Path, &Path) -> Result<CaseEvidence, String>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CaseEvidence {
    pub(super) artifacts: Vec<ArtifactEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ArtifactEvidence {
    pub(super) path: String,
    pub(super) sha256: String,
}

#[derive(Clone, Copy)]
pub(super) struct ExactCase {
    pub(super) id: &'static str,
    pub(super) operation: CaseOperation,
}

pub(super) struct ExactGroup<'a> {
    pub(super) name: &'static str,
    pub(super) default_receipt: &'static str,
    pub(super) cases: &'a [ExactCase],
    pub(super) selected_case: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct CaseResult {
    id: &'static str,
    status: &'static str,
    detail: Option<String>,
    artifacts: Vec<ArtifactEvidence>,
}

struct QualificationRoot {
    path: PathBuf,
    preserve: bool,
}

impl QualificationRoot {
    fn create(group: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stickymd-{}-{}-{nonce}",
            group.to_ascii_lowercase(),
            std::process::id()
        ));
        fs::create_dir(&path)
            .map_err(|error| format!("cannot create {group} qualification root: {error}"))?;
        Ok(Self {
            path,
            preserve: false,
        })
    }

    fn preserve(&mut self) {
        self.preserve = true;
    }
}

impl Drop for QualificationRoot {
    fn drop(&mut self) {
        if !self.preserve {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn run(
    repository: &Path,
    requested_zip: Option<&Path>,
    evidence_file: Option<&Path>,
    group: ExactGroup<'_>,
) -> Result<(), String> {
    if let Some(selected) = group.selected_case
        && !group.cases.iter().any(|case| case.id == selected)
    {
        return Err(format!(
            "{} does not define selected case {selected}",
            group.name
        ));
    }
    let environment = qualification_environment::inspect();
    if environment.status != QualificationEnvironmentStatus::Valid {
        return Err(format!(
            "{} exact qualification requires an unlocked interactive Windows desktop: {}",
            group.name,
            environment.summary()
        ));
    }
    let existing = running_stickymd_processes()?;
    if !existing.is_empty() {
        return Err(format!(
            "{} exact qualification requires exclusive StickyMD process ownership; close existing PID(s): {}",
            group.name,
            existing
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }

    let candidate = receipt::read_candidate(repository)?;
    let zip = requested_zip
        .map(|path| absolute(repository, path))
        .unwrap_or_else(|| receipt::candidate_zip(repository, &candidate));
    if receipt::sha256(&zip)? != candidate.zip_sha256 {
        return Err(format!(
            "{} ZIP does not match exact candidate {}",
            group.name, candidate.zip_sha256
        ));
    }

    let mut qualification_root = QualificationRoot::create(group.name)?;
    let template = qualification_root.path.join("template");
    expand_zip(&zip, &template, group.name)?;
    let template_program = template.join("StickyMD");
    let template_executable = template_program.join("StickyMD.exe");
    if receipt::sha256(&template_executable)? != candidate.exe_sha256 {
        return Err(format!(
            "extracted {} executable does not match candidate EXE identity",
            group.name
        ));
    }

    let mut results = Vec::new();
    for case in group.cases {
        if group
            .selected_case
            .is_some_and(|selected| selected != case.id)
        {
            continue;
        }
        let case_directory = qualification_root.path.join(case.id.to_ascii_lowercase());
        copy_directory(&template_program, &case_directory)?;
        match (case.operation)(repository, &case_directory) {
            Ok(evidence) => {
                println!("{}=PASSED", case.id);
                results.push(CaseResult {
                    id: case.id,
                    status: "PASSED",
                    detail: None,
                    artifacts: evidence.artifacts,
                });
            }
            Err(error) => {
                println!("{}=FAILED: {error}", case.id);
                results.push(CaseResult {
                    id: case.id,
                    status: "FAILED",
                    detail: Some(error),
                    artifacts: Vec::new(),
                });
            }
        }
    }

    let harness_commit = receipt::command_text(repository, "git", &["rev-parse", "HEAD"])?;
    let worktree_dirty = !receipt::command_text(
        repository,
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty();
    let document = evidence::render_receipt(
        &candidate,
        &harness_commit,
        worktree_dirty,
        &super::manual_receipt::windows_build(),
        &environment.summary(),
        &results,
    );
    let output = evidence_file.map_or_else(
        || match group.selected_case {
            Some(case) => repository.join(format!(
                "dist/evidence/{}-exact-qualification-{}.json",
                group.name.to_ascii_lowercase(),
                case.to_ascii_lowercase()
            )),
            None => repository.join(group.default_receipt),
        },
        |path| absolute(repository, path),
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {} evidence directory: {error}", group.name))?;
    }
    fs::write(&output, document)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;

    let failed = results
        .iter()
        .filter(|result| result.status != "PASSED")
        .count();
    if failed == 0 {
        println!("{}_EXACT_RECEIPT={}", group.name, output.display());
        Ok(())
    } else {
        qualification_root.preserve();
        Err(format!(
            "{} exact qualification failed {failed}/{} cases; preserved {}",
            group.name,
            results.len(),
            qualification_root.path.display()
        ))
    }
}

pub(super) fn assert_sole_stickymd_process(expected: u32) -> Result<(), String> {
    let processes = running_stickymd_processes()?;
    if processes == [expected] {
        Ok(())
    } else {
        Err(format!(
            "desktop automation requires sole StickyMD PID {expected}; observed {}",
            processes
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ))
    }
}

pub(super) fn invoke_uia(
    repository: &Path,
    action: &str,
    process_id: u32,
    path: Option<&Path>,
) -> Result<String, String> {
    let helper = repository.join("tools/stickymd-smoke/helpers/windows-uia.ps1");
    let mut command = Command::new("powershell");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(helper)
        .args(["-Action", action, "-ProcessId", &process_id.to_string()]);
    if let Some(path) = path {
        command.arg("-Path").arg(path);
    }
    let output = command
        .output()
        .map_err(|error| format!("cannot start Windows UIA helper: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "Windows UIA {action} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(super) fn seed_note(program: &Path, text: &str) -> Result<(), String> {
    let note = program.join("note");
    fs::create_dir_all(note.join("images")).map_err(io_error)?;
    fs::create_dir_all(note.join(".trash")).map_err(io_error)?;
    fs::write(note.join("note.md"), text).map_err(io_error)
}

pub(super) fn wait_for_layout(program: &Path) -> Result<(), String> {
    wait_until("portable note/config layout", || {
        Ok(program.join("note/note.md").is_file() && program.join("note/config.toml").is_file())
    })
}

pub(super) fn wait_note(program: &Path, accepted: impl Fn(&str) -> bool) -> Result<String, String> {
    let path = program.join("note/note.md");
    let mut observed = String::new();
    wait_until("durable note state", || {
        observed = fs::read_to_string(&path).unwrap_or_default();
        Ok(accepted(&observed))
    })?;
    Ok(observed)
}

pub(super) fn wait_for_config(program: &Path, expected: &str) -> Result<(), String> {
    let path = program.join("note/config.toml");
    wait_until(&format!("config field {expected}"), || {
        Ok(fs::read_to_string(&path).is_ok_and(|text| text.contains(expected)))
    })
}

pub(super) fn wait_until(
    label: &str,
    mut accepted: impl FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if accepted()? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for {label}"));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn run_cargo_test(repository: &Path, filter: &str) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["test", "-p", "stickymd-win", "--locked", filter])
        .current_dir(repository)
        .output()
        .map_err(|error| format!("cannot start targeted Rust test `{filter}`: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "targeted Rust test `{filter}` failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(super) fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

fn expand_zip(zip: &Path, destination: &Path, group: &str) -> Result<(), String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; Expand-Archive -LiteralPath $env:STICKYMD_EXACT_ZIP -DestinationPath $env:STICKYMD_EXACT_DEST -Force",
        ])
        .env("STICKYMD_EXACT_ZIP", zip)
        .env("STICKYMD_EXACT_DEST", destination)
        .output()
        .map_err(|error| format!("cannot start Expand-Archive for {group}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cannot extract exact candidate ZIP for {group}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(super) fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(io_error)?;
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().map_err(io_error)?.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(io_error)?;
        }
    }
    Ok(())
}

fn running_stickymd_processes() -> Result<Vec<u32>, String> {
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq StickyMD.exe", "/FO", "CSV", "/NH"])
        .output()
        .map_err(|error| format!("cannot enumerate StickyMD processes: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "tasklist failed while checking exact process isolation: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(parse_stickymd_tasklist(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_stickymd_tasklist(stdout: &str) -> Vec<u32> {
    let mut processes = stdout
        .lines()
        .filter(|line| line.to_ascii_lowercase().contains("stickymd.exe"))
        .filter_map(|line| line.split(',').nth(1))
        .filter_map(|field| field.trim().trim_matches('"').parse::<u32>().ok())
        .collect::<Vec<_>>();
    processes.sort_unstable();
    processes.dedup();
    processes
}

fn absolute(repository: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repository.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_stickymd_tasklist;

    #[test]
    fn exact_process_isolation_parser_is_locale_independent_and_deduplicated() {
        let tasklist = concat!(
            "\"StickyMD.exe\",\"42\",\"Console\",\"1\",\"10,000 K\"\r\n",
            "\"stickymd.exe\",\"7\",\"Console\",\"1\",\"11,000 K\"\r\n",
            "\"StickyMD.exe\",\"42\",\"Console\",\"1\",\"10,000 K\"\r\n",
            "INFO: No tasks are running which match the specified criteria.\r\n",
        );
        assert_eq!(parse_stickymd_tasklist(tasklist), vec![7, 42]);
    }
}
