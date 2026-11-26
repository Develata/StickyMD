//! `mac-gui`'s evidence: the identity of the bundle under test and the checkpoint file written
//! after every row, marked finished only after the last row of a run that was not cut short.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
#[cfg(test)]
use super::outcome::{Failure, settle};
use crate::mac_common::fixture::Expected;
#[cfg(test)]
use crate::mac_common::fixture::PreLaunch;
use crate::mac_gui::{
    model::{Row, quote},
    staging::{Bytes, Candidate},
};
use std::path::Path;

pub(super) struct AppHash {
    pub(super) executable: String,
    pub(super) bundle: String,
    /// The product inputs the bundle was built from, when `build-app.sh` recorded them.
    pub(super) inputs: Option<String>,
    /// This driver's own executable, so the ledger can tell which driver produced the evidence.
    pub(super) driver: String,
    /// The candidate ZIP R03 opens (`--zip`), checked to hold this bundle before any row runs.
    pub(super) zip: Option<Candidate>,
    /// R03's write watch calibration (`--r03-write-restore`): its evidence cannot be recorded.
    pub(super) write_restore: bool,
}

impl AppHash {
    pub(super) fn expected(&self) -> Expected<'_> {
        Expected {
            executable: &self.executable,
            bundle: &self.bundle,
            inputs: self.inputs.as_deref(),
        }
    }

    pub(super) fn bytes(&self) -> Bytes {
        Bytes {
            executable: self.executable.clone(),
            bundle: self.bundle.clone(),
        }
    }
}

/// `path` made absolute with its existing part canonical; `..` is refused. W12's logout cases use
/// it too.
/// Writes the evidence; `finished` is true only on the write after the last row, so a checkpoint
/// of an interrupted run is never taken for a finished one (mac-ledger records only finished runs).
/// Schema 2 is the first whose `finished` only this driver's rules set, and the first that names
/// the batch the run was started for; mac-ledger records no older schema.
/// Whether `name` (lowercased) is one `mac-gui` may write into --out: the evidence, each row's
/// `<id>-stderr.log`, R03's attachments and F01's screenshot.
pub(crate) fn written_here(name: &str) -> bool {
    name == "mac-gui.json"
        || name == "f01.ppm"
        || name.ends_with("-stderr.log")
        || name.starts_with("r03-")
}

/// `first`: the run's first checkpoint, created exclusively; later ones replace only it.
pub(super) fn save(
    out: &Path,
    environment: &str,
    hash: &AppHash,
    rows: &[Row],
    finished: bool,
    batch: Option<&str>,
    first: bool,
) -> Result<(), String> {
    let json = evidence_json(environment, hash, rows, finished, batch);
    let path = out.join("mac-gui.json");
    if first {
        crate::atomic_evidence::write_new(&path, json.as_bytes())
    } else {
        crate::atomic_evidence::write(&path, json.as_bytes())
    }
}

/// The evidence as written. A calibration run (`--r03-write-restore`) is never marked finished,
/// so a `mac-ledger` that does not know the `calibration` member refuses it too (review fix
/// round, B1): its R03 FAIL is the driver's bad sample, not the product's.
pub(super) fn evidence_json(
    environment: &str,
    hash: &AppHash,
    rows: &[Row],
    finished: bool,
    batch: Option<&str>,
) -> String {
    let finished = finished && !hash.write_restore;
    let batch = batch.map_or_else(String::new, |b| format!(",\"batch\":{}", quote(b)));
    // The candidate ZIP R03 opened; mac-ledger records it with R03 alone.
    let zip = hash.zip.as_ref().map_or_else(String::new, |zip| {
        format!(",\"zip_sha256\":{}", quote(&zip.sha256))
    });
    // A calibration run's evidence says so, and mac-ledger refuses it.
    let zip = if hash.write_restore {
        format!("{zip},\"calibration\":\"r03-write-restore\"")
    } else {
        zip
    };
    format!(
        "{{\"schema_version\":2,\"acceptance_status\":\"INCOMPLETE\",\"finished\":{finished}{batch},\"environment\":{environment}{},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{}{zip},\"scenarios\":[{}]}}\n",
        hash.inputs
            .as_ref()
            .map_or_else(String::new, |inputs| format!(
                ",\"app_inputs\":{}",
                quote(inputs)
            )),
        quote(&hash.executable),
        quote(&hash.bundle),
        quote(&hash.driver),
        rows.iter().map(Row::json).collect::<Vec<_>>().join(",")
    )
}

/// The evidence of an F01 run whose only row failed before (`pre_launch`) or after the product
/// was spawned, as this driver writes it, for mac-ledger's tests (gate review D2).
#[cfg(test)]
pub(crate) fn failed_f01_evidence(pre_launch: bool) -> String {
    let error = "ditto failed: exit status: 1".to_owned();
    let failure = if pre_launch {
        Failure::from(PreLaunch(error))
    } else {
        Failure::Run(error)
    };
    let mut row = Row::new("F01");
    let mut stopped = None;
    settle("F01", Ok(Err(failure)), &mut row, &mut stopped);
    let hash = AppHash {
        executable: "a".repeat(64),
        bundle: "b".repeat(64),
        inputs: Some("e".repeat(64)),
        driver: "d".repeat(64),
        zip: None,
        write_restore: false,
    };
    let environment = r#"{"os":"ProductName:\tmacOS\nProductVersion:\t27.2\nBuildVersion:\t26B123\n","hv_vmm_present":"1","allow_host":false,"unix_seconds":42}"#;
    // `execute` marks a run finished only when nothing stopped it.
    evidence_json(environment, &hash, &[row], stopped.is_none(), Some("b1"))
}

#[cfg(test)]
mod tests {
    use super::{AppHash, Row, evidence_json};

    /// Review fix round, B1: a calibration run's evidence is never marked finished, so a reader
    /// that ignores `calibration` (the mac-ledger of 916a0f1) refuses it as unfinished too.
    #[test]
    fn calibration_evidence_is_never_finished() {
        let hash = |write_restore| AppHash {
            executable: "a".repeat(64),
            bundle: "b".repeat(64),
            inputs: None,
            driver: "d".repeat(64),
            zip: None,
            write_restore,
        };
        let rows = [Row::new("R03")];
        let ordinary = evidence_json("{}", &hash(false), &rows, true, Some("rc1"));
        assert!(ordinary.contains("\"finished\":true"), "{ordinary}");
        assert!(!ordinary.contains("calibration"));
        let calibration = evidence_json("{}", &hash(true), &rows, true, Some("rc1"));
        assert!(calibration.contains("\"finished\":false"), "{calibration}");
        assert!(calibration.contains("\"calibration\":\"r03-write-restore\""));
    }
}
