//! Shared exact-candidate desktop qualification lifecycle and evidence binding.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::qualification_environment::{self, QualificationEnvironmentStatus};

pub(super) use crate::managed_process::ChildGuard;

use super::exact_checks::{self, DeclaredCase};
use super::receipt;

pub(super) use super::exact_checks::Checks;

mod evidence;

pub(super) const TIMEOUT: Duration = Duration::from_secs(12);
/// A started candidate presents its first frame well within this.
const START_READY_TIMEOUT: Duration = Duration::from_secs(30);
const UIA_HELPER_TIMEOUT: Duration = Duration::from_secs(35);
/// A portable ZIP of a few MiB expands in seconds; a slow disk gets ample room.
const EXPAND_TIMEOUT: Duration = Duration::from_secs(120);
const TASKLIST_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) type CaseOperation = fn(&Path, &Path) -> Result<CaseEvidence, String>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CaseEvidence {
    pub(super) artifacts: Vec<ArtifactEvidence>,
    /// The checks the case recorded; a pass needs exactly the declared set.
    pub(super) checks: Checks,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ArtifactEvidence {
    pub(super) path: String,
    pub(super) sha256: String,
}

pub(super) struct ExactGroup<'a> {
    pub(super) name: &'static str,
    pub(super) default_receipt: &'static str,
    /// The group's declared cases, in run order.
    pub(super) cases: &'static [DeclaredCase],
    /// `operations[i]` runs `cases[i]`.
    pub(super) operations: &'a [CaseOperation],
    pub(super) selected_case: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct CaseResult {
    id: &'static str,
    status: &'static str,
    detail: Option<String>,
    /// Recorded labels, ascending; empty when the case returned an error.
    checks: Vec<String>,
    artifacts: Vec<ArtifactEvidence>,
}

/// The result of one case run: it passes only when it returned without error and recorded
/// exactly its declared checks; a mismatch keeps the recorded labels as a diagnostic.
fn judge(case: &DeclaredCase, outcome: Result<CaseEvidence, String>) -> CaseResult {
    match outcome {
        Ok(evidence) => {
            let checks = evidence.checks.into_sorted();
            let (status, detail) = match exact_checks::mismatch(case.checks, &checks) {
                None => ("PASSED", None),
                Some(difference) => (
                    "FAILED",
                    Some(format!(
                        "recorded checks differ from the declaration: {difference}"
                    )),
                ),
            };
            CaseResult {
                id: case.id,
                status,
                detail,
                checks,
                artifacts: evidence.artifacts,
            }
        }
        Err(error) => CaseResult {
            id: case.id,
            status: "FAILED",
            detail: Some(error),
            checks: Vec::new(),
            artifacts: Vec::new(),
        },
    }
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
    if let Some(path) = evidence_file {
        super::validate_public_evidence_path(repository, path)?;
    }
    if group.cases.len() != group.operations.len() {
        return Err(format!(
            "{} declares {} case(s) but runs {}",
            group.name,
            group.cases.len(),
            group.operations.len()
        ));
    }
    if let Some(selected) = group.selected_case
        && !group.cases.iter().any(|case| case.id == selected)
    {
        return Err(format!(
            "{} does not define selected case {selected}",
            group.name
        ));
    }
    let formal_group_run =
        group.selected_case.is_none() && requested_zip.is_none() && evidence_file.is_none();
    if formal_group_run
        && super::module_ledger::reuse_for_receipt(repository, Path::new(group.default_receipt))?
    {
        return Ok(());
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
    receipt::validate_candidate_against_repository(repository, &candidate)?;
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
    for (case, operation) in group.cases.iter().zip(group.operations) {
        if group
            .selected_case
            .is_some_and(|selected| selected != case.id)
        {
            continue;
        }
        let case_directory = qualification_root.path.join(case.id.to_ascii_lowercase());
        copy_directory(&template_program, &case_directory)?;
        let result = judge(case, operation(repository, &case_directory));
        match &result.detail {
            None => println!("{}=PASSED", result.id),
            Some(error) => println!("{}=FAILED: {error}", result.id),
        }
        results.push(result);
    }

    let harness_commit = receipt::command_text(repository, "git", &["rev-parse", "HEAD"])?;
    let worktree_dirty = !receipt::command_text(
        repository,
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty();
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
    let failed = results
        .iter()
        .filter(|result| result.status != "PASSED")
        .count();
    let document = evidence::render_receipt(
        &candidate,
        &harness_commit,
        worktree_dirty,
        &super::windows_build::current(),
        &environment.summary(),
        &results,
    );
    let outcome = if failed == 0 {
        Ok(())
    } else {
        Err(format!(
            "{} exact qualification failed {failed}/{} cases; preserved {}",
            group.name,
            results.len(),
            qualification_root.path.display()
        ))
    };
    let finalized = evidence::finish(repository, &output, formal_group_run, &document, outcome);
    if finalized.is_err() {
        qualification_root.preserve();
    }
    finalized?;
    println!("{}_EXACT_RECEIPT={}", group.name, output.display());
    Ok(())
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
    let output = run_bounded_output(
        &mut command,
        UIA_HELPER_TIMEOUT,
        &format!("Windows UIA {action} helper"),
    )?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "Windows UIA {action} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Runs `command` within `timeout` with at most `MAX_HELPER_OUTPUT` bytes per stream
/// (`managed_process::run_bounded_output`).
fn run_bounded_output(
    command: &mut Command,
    timeout: Duration,
    label: &str,
) -> Result<Output, String> {
    crate::managed_process::run_bounded_output(command, timeout, label, MAX_HELPER_OUTPUT)
}

/// Longest stdout or stderr accepted from a helper.
const MAX_HELPER_OUTPUT: u64 = 1024 * 1024;

pub(super) fn seed_note(program: &Path, text: &str) -> Result<(), String> {
    let note = program.join("note");
    fs::create_dir_all(note.join("images")).map_err(io_error)?;
    fs::create_dir_all(note.join(".trash")).map_err(io_error)?;
    fs::write(note.join("note.md"), text).map_err(io_error)
}

/// Starts `executable` and waits for its first editor-ready present. Startup reconciliation
/// of managed assets runs before the editor exists, so checks made afterwards observe this
/// start's effects; an existing note/config layout alone does not prove the start got there.
pub(super) fn start_until_editor_ready(executable: &Path) -> Result<ChildGuard, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    // Disjoint from the sequences the startup measurements number from zero.
    static SEQUENCE: AtomicU64 = AtomicU64::new(1 << 32);
    let ready = crate::ready_event::ReadyEvent::create(SEQUENCE.fetch_add(1, Ordering::Relaxed))?;
    let mut command = crate::managed_process::gui_command(executable)?;
    // Signal only: an inherited exit flag or trace path would change the process under test.
    command
        .env("STICKYMD_DIAGNOSTIC_READY_EVENT", ready.name())
        .env_remove("STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY")
        .env_remove("STICKYMD_DIAGNOSTIC_STARTUP_TRACE")
        .env_remove("STICKYMD_DIAGNOSTIC_STARTUP_DETAILS");
    let child = ChildGuard::spawn(
        &mut command,
        &format!("cannot start {}", executable.display()),
    )?;
    ready.wait(START_READY_TIMEOUT)?;
    Ok(child)
}

pub(super) fn wait_for_layout(program: &Path) -> Result<(), String> {
    wait_until("portable note/config layout", || {
        Ok(program.join("note/note.md").is_file() && program.join("note/config.toml").is_file())
    })
}

/// Waits until the durable note reads as text `accepted` admits. A read that fails (a missing
/// file, invalid UTF-8, a sharing violation during an atomic replace) never satisfies the wait;
/// it is retried until the deadline, which then reports the last read error.
pub(super) fn wait_note(program: &Path, accepted: impl Fn(&str) -> bool) -> Result<String, String> {
    let path = program.join("note/note.md");
    let mut observed = None;
    let mut last_error = None;
    wait_until("durable note state", || {
        match accepted_note(fs::read_to_string(&path), &accepted) {
            Ok(text) => {
                last_error = None;
                observed = text;
            }
            Err(error) => last_error = Some(error),
        }
        Ok(observed.is_some())
    })
    .map_err(|timeout| match &last_error {
        Some(error) => format!("{timeout}; last note read failed: {error}"),
        None => timeout,
    })?;
    observed.ok_or_else(|| "durable note wait ended without an accepted note".to_owned())
}

/// The text of a note read that `accepted` admits; a failed read is an error, never text.
fn accepted_note(
    read: std::io::Result<String>,
    accepted: impl Fn(&str) -> bool,
) -> std::io::Result<Option<String>> {
    read.map(|text| accepted(&text).then_some(text))
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

pub(super) fn expand_zip(zip: &Path, destination: &Path, group: &str) -> Result<(), String> {
    let output = run_bounded_output(
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; Expand-Archive -LiteralPath $env:STICKYMD_EXACT_ZIP -DestinationPath $env:STICKYMD_EXACT_DEST -Force",
            ])
            .env("STICKYMD_EXACT_ZIP", zip)
            .env("STICKYMD_EXACT_DEST", destination),
        EXPAND_TIMEOUT,
        &format!("Expand-Archive for {group}"),
    )?;
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

pub(super) fn running_stickymd_processes() -> Result<Vec<u32>, String> {
    let output = run_bounded_output(
        Command::new("tasklist").args(["/FI", "IMAGENAME eq StickyMD.exe", "/FO", "CSV", "/NH"]),
        TASKLIST_TIMEOUT,
        "tasklist enumerating StickyMD processes",
    )?;
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
    use std::fs;
    use std::process::Command;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{
        CaseEvidence, Checks, accepted_note, judge, parse_stickymd_tasklist, run_bounded_output,
    };
    use crate::qualification::exact_checks::DeclaredCase;

    /// A missing or non-UTF-8 note is a read error, never the empty text a wait for an emptied
    /// note would accept.
    #[test]
    fn an_unreadable_note_never_satisfies_a_wait() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("stickymd-note-read-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let note = root.join("note.md");
        assert!(accepted_note(fs::read_to_string(&note), str::is_empty).is_err());
        fs::write(&note, [0xff, 0xfe]).unwrap();
        assert!(accepted_note(fs::read_to_string(&note), str::is_empty).is_err());
        fs::write(&note, "").unwrap();
        assert_eq!(
            accepted_note(fs::read_to_string(&note), str::is_empty).unwrap(),
            Some(String::new())
        );
        fs::write(&note, "text").unwrap();
        assert_eq!(
            accepted_note(fs::read_to_string(&note), str::is_empty).unwrap(),
            None
        );
        fs::remove_dir_all(root).unwrap();
    }

    /// A case passes only with exactly its declared checks; an error or any other set fails,
    /// and a mismatch keeps what was recorded.
    #[test]
    fn a_case_passes_only_with_exactly_its_declared_checks() {
        const CASE: DeclaredCase = DeclaredCase {
            id: "GX-01",
            checks: &["a", "b:k=v"],
        };
        let evidence = |labels: &[(&str, &[(&str, &str)])]| {
            let mut checks = Checks::default();
            for (name, params) in labels {
                checks.record(name, params);
            }
            Ok(CaseEvidence {
                checks,
                ..CaseEvidence::default()
            })
        };
        let passed = judge(
            &CASE,
            evidence(&[("b", &[("k", "v")]), ("a", &[]), ("a", &[])]),
        );
        assert_eq!((passed.status, passed.detail), ("PASSED", None));
        assert_eq!(passed.checks, ["a", "b:k=v"]);

        let short = judge(&CASE, evidence(&[("a", &[])]));
        assert_eq!(short.status, "FAILED");
        assert!(short.detail.unwrap().contains(r#"missing ["b:k=v"]"#));
        assert_eq!(short.checks, ["a"]);
        let extra = judge(
            &CASE,
            evidence(&[("a", &[]), ("b", &[("k", "v")]), ("c", &[])]),
        );
        assert!(extra.detail.unwrap().contains(r#"unexpected ["c"]"#));
        let other_value = judge(&CASE, evidence(&[("a", &[]), ("b", &[("k", "w")])]));
        assert_eq!(other_value.status, "FAILED");

        let failed = judge(&CASE, Err("probe failed".to_owned()));
        assert_eq!(failed.status, "FAILED");
        assert_eq!(failed.detail.as_deref(), Some("probe failed"));
        assert!(failed.checks.is_empty());
    }

    const BOUNDED_CHILD_SENTINEL: &str = "STICKYMD_BOUNDED_HELPER_SENTINEL";
    const NOISY_CHILD: &str = "STICKYMD_BOUNDED_HELPER_NOISY";
    const PIPE_HOLDER_PARENT: &str = "STICKYMD_BOUNDED_HELPER_PIPE_PARENT";
    const PIPE_HOLDER: &str = "STICKYMD_BOUNDED_HELPER_PIPE_HOLDER";

    /// A helper that exits while a descendant still holds its stdout: the call returns
    /// promptly and the descendant does not survive it.
    #[test]
    fn a_descendant_holding_the_output_is_ended_with_the_helper() {
        const NAME: &str = "qualification::exact_desktop::tests::a_descendant_holding_the_output_is_ended_with_the_helper";
        if let Some(sentinel) = std::env::var_os(PIPE_HOLDER) {
            let sentinel = std::path::PathBuf::from(sentinel);
            fs::write(
                sentinel.with_extension("started"),
                std::process::id().to_string(),
            )
            .expect("write pipe holder start");
            thread::sleep(Duration::from_secs(3));
            fs::write(sentinel, b"survived").expect("write pipe holder sentinel");
            return;
        }
        if let Some(sentinel) = std::env::var_os(PIPE_HOLDER_PARENT) {
            // Inherits this process's stdout, the helper pipe, and outlives it.
            #[expect(
                clippy::zombie_processes,
                reason = "the descendant must outlive this helper; the helper's job reaps it"
            )]
            let _holder = Command::new(std::env::current_exe().expect("current test executable"))
                .args(["--exact", NAME])
                .env_remove(PIPE_HOLDER_PARENT)
                .env(PIPE_HOLDER, &sentinel)
                .spawn()
                .expect("start pipe holder");
            // The holder is known to run before this helper exits.
            let started = std::path::PathBuf::from(sentinel).with_extension("started");
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while !started.exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the holder never started"
                );
                thread::sleep(Duration::from_millis(20));
            }
            return;
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-pipe-holder-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create pipe holder fixture");
        let sentinel = root.join("survived.txt");
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command
            .args(["--exact", NAME])
            .env(PIPE_HOLDER_PARENT, &sentinel);
        let started = std::time::Instant::now();
        let output =
            run_bounded_output(&mut command, Duration::from_secs(30), "pipe-holding helper")
                .expect("the helper itself completes");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
        assert!(
            sentinel.with_extension("started").exists(),
            "the descendant never ran"
        );
        thread::sleep(Duration::from_secs(4));
        assert!(!sentinel.exists(), "the descendant outlived its helper");
        fs::remove_dir_all(root).expect("remove pipe holder fixture");
    }

    /// A helper that writes past the output cap is refused, not buffered without bound.
    #[test]
    fn bounded_helper_output_is_capped() {
        if std::env::var_os(NOISY_CHILD).is_some() {
            use std::io::Write;
            let chunk = vec![b'x'; 64 * 1024];
            let mut stdout = std::io::stdout().lock();
            for _ in 0..(super::MAX_HELPER_OUTPUT / chunk.len() as u64 + 2) {
                if stdout.write_all(&chunk).is_err() {
                    return;
                }
            }
            return;
        }
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command
            .args([
                "--exact",
                "qualification::exact_desktop::tests::bounded_helper_output_is_capped",
                "--nocapture",
            ])
            .env(NOISY_CHILD, "1");
        let error = run_bounded_output(&mut command, Duration::from_secs(30), "noisy helper")
            .expect_err("output over the cap");
        assert!(error.contains("more than"), "{error}");
    }

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

    #[test]
    fn bounded_helper_timeout_kills_and_reaps_child() {
        if let Some(sentinel) = std::env::var_os(BOUNDED_CHILD_SENTINEL) {
            thread::sleep(Duration::from_millis(500));
            fs::write(sentinel, b"leaked").expect("write bounded helper sentinel");
            return;
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-bounded-helper-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create bounded helper fixture");
        let sentinel = root.join("leaked.txt");
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command
            .args([
                "--exact",
                "qualification::exact_desktop::tests::bounded_helper_timeout_kills_and_reaps_child",
            ])
            .env(BOUNDED_CHILD_SENTINEL, &sentinel);

        let error = run_bounded_output(
            &mut command,
            Duration::from_millis(50),
            "bounded helper regression child",
        )
        .expect_err("helper must time out");
        assert!(error.contains("killed and reaped"), "{error}");
        thread::sleep(Duration::from_millis(700));
        assert!(!sentinel.exists(), "timed-out helper leaked past its owner");
        fs::remove_dir_all(root).expect("remove bounded helper fixture");
    }
}
