//! Shared bounded probes for G5 exact-candidate cases.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::super::exact_desktop::{
    ArtifactEvidence, ChildGuard, invoke_uia, wait_for_config,
};
use super::super::super::receipt;

const PROJECTION_TIMEOUT: Duration = Duration::from_secs(12);

pub(super) fn start_ready(
    program: &Path,
) -> Result<(ChildGuard, crate::window_control::WindowHandle), String> {
    let child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    let window = crate::window_control::visible_window(child.id())?;
    Ok((child, window))
}

pub(super) fn capture(
    repository: &Path,
    process_id: u32,
    case_id: &str,
    label: &str,
    artifacts: &mut Vec<ArtifactEvidence>,
) -> Result<(), String> {
    artifacts.push(capture_artifact(repository, process_id, case_id, label)?);
    Ok(())
}

/// Captures the same window until its pixels converge, replacing a blind
/// post-action sleep with an observable bounded acknowledgement.
pub(super) fn capture_when_stable(
    repository: &Path,
    process_id: u32,
    case_id: &str,
    label: &str,
    must_differ_from: Option<&str>,
    artifacts: &mut Vec<ArtifactEvidence>,
) -> Result<(), String> {
    const REQUIRED_EQUAL_SAMPLES: usize = 3;
    let deadline = Instant::now() + PROJECTION_TIMEOUT;
    let mut previous = None::<String>;
    let mut equal_samples = 0usize;
    while Instant::now() < deadline {
        let artifact = capture_artifact(repository, process_id, case_id, label)?;
        let differs = must_differ_from.is_none_or(|hash| hash != artifact.sha256);
        if differs && previous.as_deref() == Some(artifact.sha256.as_str()) {
            equal_samples += 1;
            if equal_samples >= REQUIRED_EQUAL_SAMPLES {
                artifacts.push(artifact);
                return Ok(());
            }
        } else {
            equal_samples = 0;
        }
        previous = Some(artifact.sha256);
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "window pixels did not stabilize for {case_id}/{label} before {:?}",
        PROJECTION_TIMEOUT
    ))
}

fn capture_artifact(
    repository: &Path,
    process_id: u32,
    case_id: &str,
    label: &str,
) -> Result<ArtifactEvidence, String> {
    let relative = PathBuf::from("dist/evidence/g5-artifacts")
        .join(format!("run-{}-{process_id}", artifact_run_id()?))
        .join(case_id.to_ascii_lowercase())
        .join(format!("{label}.png"));
    let output = repository.join(&relative);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create screenshot directory: {error}"))?;
    }
    invoke_uia(repository, "capture-window", process_id, Some(&output))?;
    let metadata = fs::metadata(&output)
        .map_err(|error| format!("cannot inspect screenshot {}: {error}", output.display()))?;
    if metadata.len() < 128 {
        return Err(format!(
            "screenshot {} is unexpectedly small ({} bytes)",
            output.display(),
            metadata.len()
        ));
    }
    Ok(ArtifactEvidence {
        path: relative.to_string_lossy().replace('\\', "/"),
        sha256: receipt::sha256(&output)?,
    })
}

fn artifact_run_id() -> Result<u128, String> {
    static RUN_ID: OnceLock<u128> = OnceLock::new();
    if let Some(value) = RUN_ID.get() {
        return Ok(*value);
    }
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    Ok(*RUN_ID.get_or_init(|| value))
}

pub(super) fn assert_source_projection(
    window: crate::window_control::WindowHandle,
    expected: &str,
) -> Result<(), String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::clear_clipboard()?;
    let deadline = Instant::now() + PROJECTION_TIMEOUT;
    let mut observed = None;
    while Instant::now() < deadline {
        crate::window_control::press_select_all(window)?;
        crate::window_control::press_copy(window)?;
        observed = crate::window_control::clipboard_text()?;
        if observed
            .as_deref()
            .is_some_and(|text| normalize_newlines(text) == expected)
        {
            crate::window_control::press_document_end(window)?;
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "source projection did not match canonical text; expected_bytes={} observed_bytes={}",
        expected.len(),
        observed.as_ref().map_or(0, String::len)
    ))
}

pub(super) fn assert_preview_projection(
    window: crate::window_control::WindowHandle,
    fragments: &[&str],
) -> Result<(), String> {
    crate::window_control::focus_split_preview(window)?;
    crate::window_control::clear_clipboard()?;
    let deadline = Instant::now() + PROJECTION_TIMEOUT;
    let mut observed = None;
    while Instant::now() < deadline {
        crate::window_control::press_select_all(window)?;
        crate::window_control::press_copy(window)?;
        observed = crate::window_control::clipboard_text()?;
        if observed
            .as_deref()
            .is_some_and(|text| fragments.iter().all(|fragment| text.contains(fragment)))
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "preview projection is incomplete; required={fragments:?} observed_bytes={}",
        observed.as_ref().map_or(0, String::len)
    ))
}

pub(super) fn switch_source(process_id: u32, program: &Path) -> Result<(), String> {
    crate::window_control::switch_to_source(process_id)?;
    wait_for_config(program, "view_mode = \"source\"")
}

pub(super) fn switch_preview(
    window: crate::window_control::WindowHandle,
    program: &Path,
) -> Result<(), String> {
    crate::window_control::switch_to_preview(window)?;
    wait_for_config(program, "view_mode = \"preview\"")
}

pub(super) fn switch_split(
    window: crate::window_control::WindowHandle,
    program: &Path,
) -> Result<(), String> {
    crate::window_control::switch_to_split(window)?;
    wait_for_config(program, "view_mode = \"split\"")
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}
