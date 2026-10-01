//! Explicit native investigations reuse the real executors without writing qualification ledgers.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;
use crate::evidence::{self, EvidenceResult, EvidenceStatus};
use crate::repository::command_text;

const OUTPUT_DIRECTORY: &str = "tmp/native-diagnostics";

#[test]
#[ignore = "requires an exclusive Windows desktop and STICKYMD_SMOKE_PROBE_REPOSITORY; diagnostic only"]
fn native_startup_cpu_diagnostic() {
    diagnose("startup-cpu", |repository, root| {
        run_startup_measurement(repository, root)
    });
}

// Shared setup for explicit diagnostic tests. The normal candidate resolver is
// retained: a frozen checkout cannot fall back to an arbitrary local binary.
pub(super) fn diagnose(
    label: &str,
    execute: impl FnOnce(&Path, &Path) -> Result<RuntimeEvidence, String>,
) {
    let repository = PathBuf::from(
        std::env::var_os("STICKYMD_SMOKE_PROBE_REPOSITORY")
            .expect("explicit probe checkout required"),
    );
    assert!(
        std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
            .unwrap_or_default()
            .is_empty()
    );
    let before = identity(&repository).expect("clean diagnostic input identity");
    require_ignored_output(&repository).expect("ignored diagnostic output required before launch");
    let environment = crate::qualification_environment::inspect();
    assert_eq!(
        environment.status,
        crate::qualification_environment::QualificationEnvironmentStatus::Valid
    );
    managed_process::ensure_no_stale_smoke_stickymd().unwrap();
    crate::window_control::enable_per_monitor_v2_dpi_awareness().unwrap();
    let root = create_smoke_root().unwrap();
    let output = repository.join(OUTPUT_DIRECTORY).join(format!(
        "{label}-{}",
        root.file_name().unwrap().to_string_lossy()
    ));
    let started = Instant::now();
    let observed = execute(&repository, &root);
    let elapsed = started.elapsed().as_secs_f64();
    // Keep raw startup traces, including on failure, before removing our fixture.
    let archive = archive_startup_traces(&root, &output);
    let cleanup = cleanup_root(&root);
    let unchanged = identity(&repository).and_then(|after| {
        (after == before)
            .then_some(())
            .ok_or("diagnostic inputs changed during measurement".into())
    });
    let (mut data, mut error) = match observed {
        Ok(data) => {
            let error = data.gate_failure.clone();
            (data, error)
        }
        Err(error) => (RuntimeEvidence::passed(Vec::new()), Some(error)),
    };
    for result in [archive, cleanup, unchanged] {
        if let Err(next) = result {
            error = Some(error.map_or(next.clone(), |old| format!("{old}; {next}")));
        }
    }
    data.measurements.push(EvidenceMeasurement {
        name: "diagnostic.elapsed".into(),
        unit: "seconds".into(),
        value: elapsed,
    });
    let result = EvidenceResult {
        id: format!("native-{label}-diagnostic"),
        status: if error.is_some() {
            EvidenceStatus::Failed
        } else {
            EvidenceStatus::Passed
        },
        detail: Some(format!(
            "DIAGNOSTIC_ONLY; no qualification receipt; harness_sha256={}; {}",
            before.2,
            error.as_deref().unwrap_or("completed")
        )),
        measurements: data.measurements,
        gates: data.gates,
        samples: data.samples,
    };
    let document = evidence::render_json(
        &before.0,
        false,
        None,
        Some(&before.1),
        &format!("native-{label}-diagnostic"),
        &[result],
        Some(&environment),
    );
    // Direct atomic publication cannot update a last-success ledger.
    let path = output.join("observations.json");
    crate::atomic_evidence::write(&path, document.as_bytes()).unwrap();
    eprintln!("NATIVE_DIAGNOSTIC={} seconds={elapsed:.3}", path.display());
    assert!(error.is_none(), "{}", error.unwrap_or_default());
}

fn require_ignored_output(repository: &Path) -> Result<(), String> {
    let path = format!("{OUTPUT_DIRECTORY}/");
    command_text(repository, "git", &["check-ignore", "--", &path])
        .map(|_| ())
        .map_err(|_| format!("native diagnostic output must be ignored by Git: {OUTPUT_DIRECTORY}"))
}

fn identity(repository: &Path) -> Result<(String, String, String), String> {
    if !command_text(
        repository,
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty()
    {
        return Err("native diagnostics require a clean probe checkout".into());
    }
    let commit = command_text(
        repository,
        "git",
        &["rev-parse", "--verify", "HEAD^{commit}"],
    )?;
    let executable = crate::qualification::release_executable(repository)?;
    let harness = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok((
        commit,
        crate::integrity::sha256(&executable)?,
        crate::integrity::sha256(&harness)?,
    ))
}

fn archive_startup_traces(root: &Path, output: &Path) -> Result<(), String> {
    let traces = root.join("phase9-startup");
    if !traces.exists() {
        return Ok(());
    }
    let destination = output.join("traces");
    fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(traces).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| name.starts_with("startup-trace-") && name.ends_with(".txt"))
        {
            let bytes = fs::read(entry.path()).map_err(|e| e.to_string())?;
            crate::atomic_evidence::write_new(&destination.join(name), &bytes)?;
        }
    }
    Ok(())
}

#[test]
fn cpu_observation_is_not_wall_time_or_a_new_startup_gate() {
    let sample = StartupSample {
        external: Duration::from_millis(400),
        milestones_us: vec![("editor_ready".into(), 390_000)],
        process_cpu_at_observation: Duration::from_millis(650),
        cpu_observation_lag: Duration::from_micros(50),
    };
    let samples = startup_samples(&[sample], &[]);
    let values = &samples[0].measurements;
    for (name, expected) in [
        ("external", 400.0),
        ("internal", 390.0),
        ("process_cpu_at_ready_observation", 650.0),
        ("cpu_observation_lag", 0.05),
    ] {
        assert!(
            values
                .iter()
                .any(|m| m.name == name && m.unit == "ms" && m.value == expected)
        );
    }
    assert_eq!(startup_gates().len(), 2);
    assert!(
        startup_gates()
            .iter()
            .all(|g| g.metric.ends_with(".p95") && g.value == 550.0)
    );
}

#[test]
fn startup_failure_retains_completed_observations_without_fabricating_percentiles() {
    let cold = [StartupSample {
        external: Duration::from_millis(400),
        milestones_us: vec![("editor_ready".into(), 390_000)],
        process_cpu_at_observation: Duration::from_millis(200),
        cpu_observation_lag: Duration::from_micros(50),
    }];
    let output = incomplete_startup(&cold, &[], "second sample timed out".into());
    assert_eq!(
        output.gate_failure.as_deref(),
        Some("second sample timed out")
    );
    assert_eq!(output.samples.len(), 1);
    assert_eq!(output.samples[0].run, 1);
    assert!(
        output.samples[0]
            .measurements
            .iter()
            .any(|m| { m.name == "process_cpu_at_ready_observation" && m.value == 200.0 })
    );
    assert!(
        output
            .measurements
            .iter()
            .any(|m| m.name == "cold.samples" && m.value == 1.0)
    );
    assert!(
        output
            .measurements
            .iter()
            .any(|m| m.name == "warm.samples" && m.value == 0.0)
    );
    assert!(!output.measurements.iter().any(|m| m.name.ends_with("p95")));
    assert_eq!(output.gates.len(), 2);
}

#[test]
fn diagnostic_output_stays_ignored_in_a_fresh_checkout_without_hiding_other_files() {
    let root = create_smoke_root().unwrap();
    command_text(&root, "git", &["init", "--quiet"]).unwrap();
    // Do not let the user's global excludes mask a missing repository rule.
    let excludes = root.join(".git/empty-excludes");
    crate::atomic_evidence::write_new(&excludes, b"").unwrap();
    command_text(
        &root,
        "git",
        &["config", "core.excludesFile", excludes.to_str().unwrap()],
    )
    .unwrap();
    assert!(require_ignored_output(&root).is_err());
    crate::atomic_evidence::write_new(
        &root.join(".gitignore"),
        include_bytes!("../../../../.gitignore"),
    )
    .unwrap();
    command_text(&root, "git", &["add", ".gitignore"]).unwrap();
    command_text(
        &root,
        "git",
        &[
            "-c",
            "user.name=Smoke",
            "-c",
            "user.email=smoke@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    )
    .unwrap();
    require_ignored_output(&root).unwrap();
    crate::atomic_evidence::write(
        &root.join(OUTPUT_DIRECTORY).join("probe/observations.json"),
        b"{}",
    )
    .unwrap();
    let fixture = root.join("target/fixture");
    crate::atomic_evidence::write(
        &fixture.join("phase9-startup/startup-trace-1.txt"),
        b"retained trace",
    )
    .unwrap();
    archive_startup_traces(&fixture, &root.join(OUTPUT_DIRECTORY).join("probe")).unwrap();
    assert!(
        command_text(
            &root,
            "git",
            &["status", "--porcelain", "--untracked-files=all"]
        )
        .unwrap()
        .is_empty()
    );
    crate::atomic_evidence::write_new(&root.join("tmp/unrelated.txt"), b"keep visible").unwrap();
    assert!(
        command_text(
            &root,
            "git",
            &["status", "--porcelain", "--untracked-files=all"]
        )
        .unwrap()
        .contains("tmp/unrelated.txt")
    );
    cleanup_root(&root).unwrap();
}
