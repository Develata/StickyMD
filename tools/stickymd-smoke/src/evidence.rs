//! Stable std-only machine-readable evidence projection.

use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvidenceResult {
    pub(crate) id: String,
    pub(crate) status: EvidenceStatus,
    pub(crate) detail: Option<String>,
    pub(crate) measurements: Vec<EvidenceMeasurement>,
    pub(crate) gates: Vec<EvidenceGate>,
    pub(crate) samples: Vec<EvidenceSample>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvidenceMeasurement {
    pub(crate) name: String,
    pub(crate) unit: String,
    pub(crate) value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvidenceGate {
    pub(crate) metric: String,
    pub(crate) comparator: String,
    pub(crate) value: f64,
    pub(crate) unit: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EvidenceSample {
    pub(crate) cohort: String,
    pub(crate) run: usize,
    pub(crate) measurements: Vec<EvidenceMeasurement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EvidenceStatus {
    Passed,
    Failed,
    #[cfg_attr(
        all(windows, not(test)),
        expect(
            dead_code,
            reason = "Windows can execute every runtime gate; non-Windows evidence constructs NOT_TESTED"
        )
    )]
    NotTested,
}

impl EvidenceStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "PASSED",
            Self::Failed => "FAILED",
            Self::NotTested => "NOT_TESTED",
        }
    }
}

pub(crate) fn emit(
    root: &Path,
    suite: &str,
    results: &[EvidenceResult],
    output_file: Option<&Path>,
) -> Result<(), String> {
    let commit = current_commit(root).unwrap_or_else(|_| "UNKNOWN".to_owned());
    let worktree_dirty = current_worktree_dirty(root).unwrap_or(true);
    let artifact = verified_artifact_sha256(root, results);
    let executable = executable_sha256(root);
    let json = render_json(
        &commit,
        worktree_dirty,
        artifact.as_deref(),
        executable.as_deref(),
        suite,
        results,
    );
    if let Some(path) = output_file {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "cannot create evidence directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }
        fs::write(path, json)
            .map_err(|error| format!("cannot write evidence file `{}`: {error}", path.display()))?;
    } else {
        println!("{json}");
    }
    Ok(())
}

fn executable_sha256(root: &Path) -> Option<String> {
    let executable = root.join("target/release/stickymd-win.exe");
    if !executable.is_file() {
        return None;
    }
    #[cfg(windows)]
    let output = Command::new("certutil")
        .args(["-hashfile"])
        .arg(&executable)
        .arg("SHA256")
        .output()
        .ok()?;
    #[cfg(not(windows))]
    let output = Command::new("sha256sum").arg(&executable).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

fn verified_artifact_sha256(root: &Path, results: &[EvidenceResult]) -> Option<String> {
    let verified = results.iter().any(|result| {
        result.id == "portable package verification" && result.status == EvidenceStatus::Passed
    });
    if !verified {
        return None;
    }
    fs::read_to_string(root.join("dist/SHA256SUMS.txt"))
        .ok()?
        .lines()
        .find_map(|line| {
            let (hash, name) = line.split_once(" *")?;
            (name.ends_with("-windows-x64-portable.zip")
                && hash.len() == 64
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
        })
}

fn current_commit(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start git rev-parse: {error}"))?;
    if !output.status.success() {
        return Err("git rev-parse HEAD failed".to_owned());
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("git commit output is not UTF-8: {error}"))
}

fn current_worktree_dirty(root: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start git status: {error}"))?;
    if !output.status.success() {
        return Err("git status --porcelain failed".to_owned());
    }
    Ok(!output.stdout.is_empty())
}

fn render_json(
    commit: &str,
    worktree_dirty: bool,
    artifact_sha256: Option<&str>,
    executable_sha256: Option<&str>,
    suite: &str,
    results: &[EvidenceResult],
) -> String {
    let mut output = format!(
        "{{\"schema_version\":2,\"suite_version\":\"2\",\"commit\":\"{}\",\"worktree_dirty\":{},\"artifact_sha256\":{},\"executable_sha256\":{},\"suite\":\"{}\",\"results\":[",
        escape_json(commit),
        worktree_dirty,
        artifact_sha256
            .map(|hash| format!("\"{}\"", escape_json(hash)))
            .unwrap_or_else(|| "null".to_owned()),
        executable_sha256
            .map(|hash| format!("\"{}\"", escape_json(hash)))
            .unwrap_or_else(|| "null".to_owned()),
        escape_json(suite)
    );
    for (index, result) in results.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"id\":\"{}\",\"status\":\"{}\",\"detail\":{},\"measurements\":[",
            escape_json(&result.id),
            result.status.as_str(),
            result
                .detail
                .as_deref()
                .map(|detail| format!("\"{}\"", escape_json(detail)))
                .unwrap_or_else(|| "null".to_owned())
        ));
        for (measurement_index, measurement) in result.measurements.iter().enumerate() {
            if measurement_index > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"name\":\"{}\",\"unit\":\"{}\",\"value\":{:.6}}}",
                escape_json(&measurement.name),
                escape_json(&measurement.unit),
                measurement.value,
            ));
        }
        output.push_str("],\"gates\":[");
        for (gate_index, gate) in result.gates.iter().enumerate() {
            if gate_index > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"metric\":\"{}\",\"comparator\":\"{}\",\"value\":{:.6},\"unit\":\"{}\",\"source\":\"{}\"}}",
                escape_json(&gate.metric),
                escape_json(&gate.comparator),
                gate.value,
                escape_json(&gate.unit),
                escape_json(&gate.source),
            ));
        }
        output.push_str("],\"samples\":[");
        for (sample_index, sample) in result.samples.iter().enumerate() {
            if sample_index > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"cohort\":\"{}\",\"run\":{},\"measurements\":[",
                escape_json(&sample.cohort),
                sample.run,
            ));
            for (measurement_index, measurement) in sample.measurements.iter().enumerate() {
                if measurement_index > 0 {
                    output.push(',');
                }
                output.push_str(&format!(
                    "{{\"name\":\"{}\",\"unit\":\"{}\",\"value\":{:.6}}}",
                    escape_json(&measurement.name),
                    escape_json(&measurement.unit),
                    measurement.value,
                ));
            }
            output.push_str("]}");
        }
        output.push_str("]}");
    }
    output.push_str("]}");
    output
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        EvidenceGate, EvidenceMeasurement, EvidenceResult, EvidenceSample, EvidenceStatus,
        render_json, verified_artifact_sha256,
    };

    #[test]
    fn phase11_json_schema_exposes_gates_samples_and_suite_identity() {
        let json = render_json(
            "abc",
            true,
            None,
            Some(&"b".repeat(64)),
            "phase-10",
            &[
                EvidenceResult {
                    id: "task\"one".to_owned(),
                    status: EvidenceStatus::Failed,
                    detail: Some("line\nfailed".to_owned()),
                    measurements: vec![EvidenceMeasurement {
                        name: "p95".to_owned(),
                        unit: "ms".to_owned(),
                        value: 12.5,
                    }],
                    gates: vec![EvidenceGate {
                        metric: "warm.p95".to_owned(),
                        comparator: "<=".to_owned(),
                        value: 400.0,
                        unit: "ms".to_owned(),
                        source: "docs/plan/10_performance_reliability.md".to_owned(),
                    }],
                    samples: vec![EvidenceSample {
                        cohort: "warm".to_owned(),
                        run: 1,
                        measurements: vec![EvidenceMeasurement {
                            name: "external".to_owned(),
                            unit: "ms".to_owned(),
                            value: 12.5,
                        }],
                    }],
                },
                EvidenceResult {
                    id: "manual capability".to_owned(),
                    status: EvidenceStatus::NotTested,
                    detail: Some("capability unavailable".to_owned()),
                    measurements: Vec::new(),
                    gates: Vec::new(),
                    samples: Vec::new(),
                },
            ],
        );
        assert!(json.starts_with("{\"schema_version\":2,\"suite_version\":\"2\","));
        assert!(json.contains("\"worktree_dirty\":true"));
        assert!(json.contains("\"artifact_sha256\":null"));
        assert!(json.contains(&format!("\"executable_sha256\":\"{}\"", "b".repeat(64))));
        assert!(json.contains("\"suite\":\"phase-10\""));
        assert!(json.contains("task\\\"one"));
        assert!(json.contains("line\\nfailed"));
        assert!(json.contains("\"status\":\"NOT_TESTED\""));
        assert!(
            json.contains(
                "\"measurements\":[{\"name\":\"p95\",\"unit\":\"ms\",\"value\":12.500000}]"
            )
        );
        assert!(json.contains("\"metric\":\"warm.p95\""));
        assert!(json.contains("\"cohort\":\"warm\",\"run\":1"));
        assert!(json.ends_with("]}"));
    }

    #[test]
    fn artifact_hash_is_emitted_only_after_package_verification_passes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("stickymd-evidence-{}-{nonce}", std::process::id()));
        fs::create_dir_all(root.join("dist")).expect("create evidence fixture");
        let hash = "a".repeat(64);
        fs::write(
            root.join("dist/SHA256SUMS.txt"),
            format!("{hash} *StickyMD-0.1.0-local-rc-windows-x64-portable.zip\n"),
        )
        .expect("write evidence fixture");
        let failed = [EvidenceResult {
            id: "portable package verification".to_owned(),
            status: EvidenceStatus::Failed,
            detail: None,
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        }];
        assert_eq!(verified_artifact_sha256(&root, &failed), None);
        let passed = [EvidenceResult {
            id: "portable package verification".to_owned(),
            status: EvidenceStatus::Passed,
            detail: None,
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        }];
        assert_eq!(verified_artifact_sha256(&root, &passed), Some(hash));
        fs::remove_dir_all(root).expect("remove evidence fixture");
    }
}
