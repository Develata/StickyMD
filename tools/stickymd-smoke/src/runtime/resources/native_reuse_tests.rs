//! Opt-in live cohorts for cache review; no synthetic observations or formal receipts.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;
use crate::{
    evidence::{self, EvidenceResult, EvidenceStatus},
    qualification::resource_diagnostics::Store,
    resource_plan::progress::Console,
};

#[test]
#[ignore = "takes the Windows desktop for five minutes; requires STICKYMD_SMOKE_PROBE_REPOSITORY; diagnostic only"]
fn native_resource_alias_and_batch_reuse() {
    let repository = std::path::PathBuf::from(
        std::env::var_os("STICKYMD_SMOKE_PROBE_REPOSITORY").expect("explicit probe checkout"),
    );
    assert!(
        std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
            .unwrap_or_default()
            .is_empty()
    );
    let environment = crate::qualification_environment::inspect();
    assert_eq!(
        environment.status,
        crate::qualification_environment::QualificationEnvironmentStatus::Valid
    );
    managed_process::ensure_no_stale_smoke_stickymd().unwrap();
    let root = create_smoke_root().unwrap();
    let mut results = Vec::new();
    let outcome = collect(&repository, &root, &mut results);
    let cleanup = cleanup_root(&root);
    if let Err(error) = outcome.and(cleanup) {
        results.push(EvidenceResult {
            id: "native cache review failed".into(),
            status: EvidenceStatus::Failed,
            detail: Some(error.clone()),
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        });
        emit(&repository, &results, &environment).unwrap();
        panic!("{error}");
    }
    emit(&repository, &results, &environment).unwrap();
}

fn emit(
    repository: &Path,
    results: &[EvidenceResult],
    environment: &crate::qualification_environment::QualificationEnvironment,
) -> Result<(), String> {
    evidence::emit(
        repository,
        "native-resource-reuse-diagnostic",
        results,
        Some(environment),
        Some(Path::new(
            "target/acceptance-profiling/resource-native-reuse-review.json",
        )),
    )
}

fn collect(repository: &Path, root: &Path, report: &mut Vec<EvidenceResult>) -> Result<(), String> {
    let source = crate::qualification::release_executable(repository)?;
    let store = Store::open(repository)?;
    probe::run(repository, root)?;
    let cases = [
        ResourceModule::Math.cases()[1],
        ResourceModule::Math.cases()[2],
    ];
    let mut progress = Console {
        remaining: cases.iter().map(|case| case.minimum_wait_seconds()).sum(),
    };
    let mut fresh = Vec::new();
    let start = Instant::now();
    for case in cases {
        store.verify(repository)?;
        let mut observed = Output::default();
        let timer = Instant::now();
        let outcome = measure_case(&source, root, case, &mut observed, &mut progress);
        let elapsed = timer.elapsed().as_secs_f64();
        let result = EvidenceResult {
            id: case.label.into(),
            status: if outcome.is_ok() {
                EvidenceStatus::Passed
            } else {
                EvidenceStatus::Failed
            },
            detail: outcome.as_ref().err().cloned(),
            measurements: observed.measurements,
            gates: observed.gates,
            samples: observed.samples,
        };
        report.push(labelled("fresh", &result));
        outcome?;
        store.save(repository, case.into(), &result, elapsed)?;
        fresh.push(result);
    }
    let fresh_seconds = start.elapsed().as_secs_f64();
    let next = Store::open(repository)?;
    let timer = Instant::now();
    let singles = cases
        .iter()
        .map(|case| {
            next.load(repository, (*case).into())?
                .ok_or_else(|| "missing real case after reopening Store".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let single_seconds = timer.elapsed().as_secs_f64();
    let timer = Instant::now();
    let batch = next
        .load_batch(repository, &cases)?
        .ok_or("missing real batch")?;
    let batch_seconds = timer.elapsed().as_secs_f64();
    if batch != singles {
        return Err("real batch changed historical observations".into());
    }
    for result in &batch {
        report.push(labelled("batch", result));
    }

    let origin = singles[0].samples[0]
        .shared_from
        .as_deref()
        .ok_or("missing origin")?;
    let fingerprint = origin.split(':').nth(1).ok_or("missing origin identity")?;
    let directory = repository
        .join("target/resource-diagnostics/v2")
        .join(fingerprint);
    let path = directory.join(format!("{}.json", cases[0].label));
    let original = fs::read(&path).map_err(|e| e.to_string())?;
    let alias_case = ResourceModule::Images.cases()[2];
    let alias = next
        .load(repository, alias_case.into())?
        .ok_or("missing real equivalent case")?;
    if alias.samples.len() != RESOURCE_REPETITIONS
        || alias
            .samples
            .iter()
            .zip(&fresh[0].samples)
            .any(|(cached, actual)| {
                cached.cohort != alias_case.label
                    || cached.run != actual.run
                    || cached.measurements != actual.measurements
                    || cached.shared_from.as_deref() != Some(origin)
            })
        || fs::read(&path).map_err(|e| e.to_string())? != original
        || directory
            .join(format!("{}.json", alias_case.label))
            .exists()
    {
        return Err("real alias changed raw observations, origin, or record lifetime".into());
    }
    report.push(labelled("alias", &alias));
    let timing = EvidenceResult {
        id: "live diagnostic cache comparison".into(), status: EvidenceStatus::Passed,
        detail: Some("Two real five-sample cohorts; single reads use four identity captures, batch uses two; DIAGNOSTIC_ONLY, not full resource qualification".into()),
        measurements: [("fresh_case_execution", fresh_seconds), ("single_loads", single_seconds), ("batch_load", batch_seconds)]
            .into_iter().map(|(name, value)| EvidenceMeasurement { name: name.into(), unit: "seconds".into(), value }).collect(),
        gates: Vec::new(), samples: Vec::new(),
    };
    eprintln!(
        "LIVE_CACHE_REVIEW fresh_seconds={fresh_seconds:.6} single_seconds={single_seconds:.6} batch_seconds={batch_seconds:.6} fresh_samples=10 alias_samples=5; DIAGNOSTIC_ONLY"
    );
    report.push(timing);
    Ok(())
}

fn labelled(prefix: &str, result: &EvidenceResult) -> EvidenceResult {
    let mut result = result.clone();
    result.id = format!("{prefix}/{}", result.id);
    result
}
