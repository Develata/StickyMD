use super::*;
use std::{fs, process::ExitCode};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = crate::test_support::unique_temp_path("startup 失败路径");
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn diagnostic(&self, exit_after_ready: bool) -> StartupDiagnostics {
        let mut diagnostic = StartupDiagnostics {
            started: Instant::now(),
            ready_event: None,
            trace_path: Some(self.0.join("trace.txt")),
            exit_after_ready,
            milestones: vec![("process_start", 0)],
            details: Some(details::Details::new(self.0.join("details.txt"))),
            finished: false,
            failed: false,
        };
        for milestone in [
            StartupDetail::SplitModeBegin,
            StartupDetail::SplitModeEnd,
            StartupDetail::WindowShowBegin,
            StartupDetail::WindowShowEnd,
            StartupDetail::ToolIdentityBegin,
            StartupDetail::ToolIdentityEnd,
        ] {
            diagnostic.record_detail(milestone);
        }
        diagnostic
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Exclusive create_dir above establishes ownership of this fixture only.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn capture_failures_preserve_auto_exit_and_report_failure_after_completion() {
    for auto_exit in [false, true] {
        for occupied in ["trace.txt", "details.txt"] {
            let fixture = Fixture::new();
            let occupied = fixture.0.join(occupied);
            fs::write(&occupied, b"keep existing bytes").unwrap();
            let mut diagnostic = fixture.diagnostic(auto_exit);
            assert!(!diagnostic.exit_requested());
            assert!(diagnostic.editor_ready().is_err());
            assert_eq!(diagnostic.exit_requested(), auto_exit);
            assert_eq!(
                diagnostic.exit_code(),
                if auto_exit {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            );
            assert_eq!(fs::read(&occupied).unwrap(), b"keep existing bytes");
            // A later redraw neither retries the writes nor erases the failure.
            assert_eq!(diagnostic.editor_ready(), Ok(false));
            assert_eq!(diagnostic.exit_requested(), auto_exit);
            assert_eq!(
                diagnostic.exit_code(),
                if auto_exit {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            );
        }
    }
}

#[test]
fn missing_ready_event_also_finishes_the_explicit_diagnostic_with_failure() {
    let fixture = Fixture::new();
    let mut diagnostic = fixture.diagnostic(true);
    diagnostic.ready_event = Some(format!(
        "Local\\missing-{}",
        fixture.0.file_name().unwrap().to_string_lossy()
    ));
    assert!(
        diagnostic
            .editor_ready()
            .unwrap_err()
            .contains("ready event")
    );
    assert!(diagnostic.exit_requested());
    assert_eq!(diagnostic.exit_code(), ExitCode::FAILURE);
    assert!(!fixture.0.join("trace.txt").exists());
}

#[test]
fn successful_capture_retains_the_existing_exit_flag_and_zero_status() {
    for auto_exit in [false, true] {
        let fixture = Fixture::new();
        let mut diagnostic = fixture.diagnostic(auto_exit);
        assert_eq!(diagnostic.editor_ready(), Ok(auto_exit));
        assert_eq!(diagnostic.exit_requested(), auto_exit);
        assert_eq!(diagnostic.exit_code(), ExitCode::SUCCESS);
        let trace = fs::read_to_string(fixture.0.join("trace.txt")).unwrap();
        let details = fs::read_to_string(fixture.0.join("details.txt")).unwrap();
        assert!(details.ends_with(&trace));
    }
}

#[test]
fn disabled_diagnostics_do_not_accumulate_milestones() {
    // The regular test process does not set the private smoke variables.
    let mut diagnostics = StartupDiagnostics::from_environment();
    if std::env::var_os(READY_EVENT_ENV).is_none() && std::env::var_os(TRACE_PATH_ENV).is_none() {
        diagnostics.record("main_enter");
        assert!(diagnostics.milestones.is_empty());
        assert!(!diagnostics.exit_requested());
        assert_eq!(diagnostics.exit_code(), ExitCode::SUCCESS);
    }
}
