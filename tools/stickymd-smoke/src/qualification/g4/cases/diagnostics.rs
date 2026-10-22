//! Optional local-preflight visual diagnostics; never a case verdict or manual acceptance.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

pub(super) fn step(
    repository: &Path,
    program: &Path,
    process_id: u32,
    stage: &str,
) -> Result<(), String> {
    if std::env::var("STICKYMD_LOCAL_PREFLIGHT_DIAGNOSTICS").as_deref() != Ok("1") {
        return Ok(());
    }
    let image = program.join(format!("{stage}.png"));
    super::super::super::exact_desktop::invoke_uia(
        repository,
        "capture-window",
        process_id,
        Some(&image),
    )?;
    println!("LOCAL_PREFLIGHT_DIAGNOSTIC_IMAGE={}", image.display());
    Ok(())
}
