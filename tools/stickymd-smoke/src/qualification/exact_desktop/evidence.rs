//! Exact desktop qualification receipt serialization.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use crate::qualification::{json, receipt};

use super::CaseResult;

pub(super) fn render_receipt(
    candidate: &receipt::Candidate,
    harness_commit: &str,
    worktree_dirty: bool,
    windows_build: &str,
    environment: &str,
    results: &[CaseResult],
) -> String {
    let status = if results.iter().all(|result| result.status == "PASSED") {
        "PASSED"
    } else {
        "FAILED"
    };
    let mut output = format!(
        concat!(
            "{{\"schema_version\":1,\"status\":\"{}\",",
            "\"source_commit\":\"{}\",\"harness_commit\":\"{}\",",
            "\"worktree_dirty\":{},",
            "\"version\":\"{}\",\"windows\":\"{}\",",
            "\"exe_sha256\":\"{}\",\"zip_sha256\":\"{}\",",
            "\"qualification_environment\":\"{}\",\"results\":["
        ),
        status,
        json::escape(&candidate.source_commit),
        json::escape(harness_commit),
        worktree_dirty,
        json::escape(&candidate.version),
        json::escape(windows_build),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        json::escape(environment),
    );
    for (index, result) in results.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        let detail = result
            .detail
            .as_deref()
            .map(|value| format!("\"{}\"", json::escape(value)))
            .unwrap_or_else(|| "null".to_owned());
        output.push_str(&format!(
            "{{\"id\":\"{}\",\"status\":\"{}\",\"detail\":{detail},\"artifacts\":[",
            result.id, result.status
        ));
        for (artifact_index, artifact) in result.artifacts.iter().enumerate() {
            if artifact_index > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"path\":\"{}\",\"sha256\":\"{}\"}}",
                json::escape(&artifact.path),
                json::escape(&artifact.sha256),
            ));
        }
        output.push_str("]}");
    }
    output.push_str("]}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::render_receipt;
    use crate::qualification::exact_desktop::{ArtifactEvidence, CaseResult};
    use crate::qualification::receipt::Candidate;

    #[test]
    fn exact_receipt_binds_candidate_harness_and_results() {
        let candidate = Candidate {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            exe_sha256: "c".repeat(64),
            zip_sha256: "d".repeat(64),
            sbom_sha256: "e".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        };
        let document = render_receipt(
            &candidate,
            &candidate.source_commit,
            false,
            "Windows test",
            "VALID",
            &[CaseResult {
                id: "GX-01",
                status: "PASSED",
                detail: None,
                artifacts: vec![ArtifactEvidence {
                    path: "dist/evidence/gx.png".to_owned(),
                    sha256: "f".repeat(64),
                }],
            }],
        );
        assert!(document.contains("\"status\":\"PASSED\""));
        assert!(document.contains(&format!("\"zip_sha256\":\"{}\"", candidate.zip_sha256)));
        assert_eq!(document.matches("\"id\":\"GX-").count(), 1);
        assert!(document.contains("\"path\":\"dist/evidence/gx.png\""));
        assert!(document.contains(&format!("\"sha256\":\"{}\"", "f".repeat(64))));
    }
}
