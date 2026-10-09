//! Complete, passing evidence documents for tests that need a recorded module run.

use std::fs;
use std::path::Path;

use super::{G3_CASES, G4_CASES, G5_SCREENSHOTS};
use crate::evidence::{EvidenceResult, EvidenceStatus};
use crate::qualification::json;
use crate::qualification::module_registry::ModuleId;
use crate::qualification::receipt::Candidate;

/// A passing document for `module` run on `candidate`. `input_fingerprint` is the
/// resource input digest (other modules ignore it); `marker` tells runs apart. G5
/// screenshots are written below `root` so the run can be recorded from it.
pub(in crate::qualification) fn valid_document(
    root: &Path,
    module: ModuleId,
    candidate: &Candidate,
    input_fingerprint: &str,
    marker: &str,
) -> String {
    match module {
        ModuleId::Runtime | ModuleId::Performance => smoke(module, candidate, marker),
        ModuleId::Resource(group) => {
            let result = crate::resource_plan::tests::valid_resource_result(group);
            let document = crate::resource_plan::tests::document(group, &result)
                .replace(
                    &format!("\"commit\":\"{}\"", "a".repeat(40)),
                    &format!("\"commit\":\"{}\"", candidate.source_commit),
                )
                .replace(
                    &format!("\"executable_sha256\":\"{}\"", "c".repeat(64)),
                    &format!("\"executable_sha256\":\"{}\"", candidate.exe_sha256),
                );
            format!(
                "{{\"resource_input_fingerprint\":\"{input_fingerprint}\",{}",
                &document[1..]
            )
        }
        ModuleId::G3 => exact(candidate, &cases(&G3_CASES, marker, |_| String::new())),
        ModuleId::G4 => exact(candidate, &cases(&G4_CASES, marker, |_| String::new())),
        ModuleId::G5 => {
            let ids = G5_SCREENSHOTS.map(|(case, _)| case);
            exact(
                candidate,
                &cases(&ids, marker, |case| screenshots(root, case, marker)),
            )
        }
    }
}

/// An exact receipt with the given result objects.
pub(in crate::qualification) fn exact(candidate: &Candidate, results: &str) -> String {
    format!(
        concat!(
            "{{\"schema_version\":1,\"status\":\"PASSED\",",
            "\"source_commit\":\"{commit}\",\"harness_commit\":\"{commit}\",",
            "\"worktree_dirty\":false,\"version\":\"{version}\",",
            "\"windows\":\"Windows test\",\"exe_sha256\":\"{exe}\",",
            "\"zip_sha256\":\"{zip}\",\"qualification_environment\":\"VALID\",",
            "\"results\":[{results}]}}\n"
        ),
        commit = candidate.source_commit,
        version = json::escape(&candidate.version),
        exe = candidate.exe_sha256,
        zip = candidate.zip_sha256,
        results = results,
    )
}

fn cases(ids: &[&str], marker: &str, artifacts: impl Fn(&str) -> String) -> String {
    ids.iter()
        .map(|case| {
            format!(
                "{{\"id\":\"{case}\",\"status\":\"PASSED\",\"detail\":\"{}\",\"artifacts\":[{}]}}",
                json::escape(marker),
                artifacts(case)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Write the minimum number of screenshots for one G5 case and list them.
fn screenshots(root: &Path, case: &str, marker: &str) -> String {
    let minimum = G5_SCREENSHOTS
        .iter()
        .find(|(id, _)| *id == case)
        .map_or(0, |(_, minimum)| *minimum);
    (0..minimum)
        .map(|index| {
            let relative = format!("dist/evidence/g5-artifacts/{case}-{index}-{marker}.png");
            let path = root.join(&relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, format!("{case} screenshot {index} {marker}")).unwrap();
            let sha256 = crate::integrity::sha256(&path).unwrap();
            format!("{{\"path\":\"{relative}\",\"sha256\":\"{sha256}\"}}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn smoke(module: ModuleId, candidate: &Candidate, marker: &str) -> String {
    let results = crate::runner::formal_task_labels(module == ModuleId::Runtime)
        .unwrap()
        .into_iter()
        .map(|id| EvidenceResult {
            id: id.into(),
            status: EvidenceStatus::Passed,
            detail: Some(marker.to_owned()),
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        })
        .collect::<Vec<_>>();
    crate::evidence::render_json(
        &candidate.source_commit,
        false,
        None,
        Some(&candidate.exe_sha256),
        "phase-14",
        &results,
        None,
    )
}
