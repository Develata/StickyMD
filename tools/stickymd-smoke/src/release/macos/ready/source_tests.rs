//! The receipt of the source tests on the release commit R (plan 11 #macos-release-gate, the
//! "R 上的源码测试" row; format #macos-readiness-check): what the run recorded, and how the gate
//! judges it (`verdict`). Each test binary carries every test its own `--list` names, with the
//! outcome the run printed for it; the explicit runs of ignored tests (P18-L08's two volumes,
//! P16-S06's exFAT volume) carry the same, by test name (USER 2026-11-24, decision (a)).
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

mod format;
mod ignored;
pub(super) mod output;
#[cfg(target_os = "macos")]
mod read;
#[cfg(target_os = "macos")]
pub(super) mod run;
#[cfg(target_os = "macos")]
mod steps;
#[cfg(test)]
mod tests;
mod verdict;

pub(super) use format::{parse, render};
#[cfg(target_os = "macos")]
pub(super) use read::read_receipt;
#[cfg(target_os = "macos")]
pub(super) use verdict::Verdicts;
pub(super) use verdict::{Outcome, driver_of, judge};

/// Where the receipt lives; the runner replaces it, the gate reads it.
pub(crate) const RECEIPT: &str = crate::qualification::reserved_paths::MACOS_SOURCE_TEST_RECEIPT;

/// Held exclusively by a run from before it retires the old receipt until it has published the
/// new one, and shared by the gate while it reads the receipt: two runs never overlap, and the
/// gate never reads a receipt a run is replacing. The operating system releases it when the
/// holder exits, killed or not.
pub(crate) const LOCK: &str = crate::qualification::reserved_paths::MACOS_SOURCE_TEST_LOCK;

/// The identity of the last run that started, written atomically under the lock before the old
/// receipt is retired; a receipt counts only when it names this run (review round 4, BLOCKER 2):
/// a run interrupted before it published leaves its own identity here, so an older receipt,
/// restored or left behind, no longer matches.
pub(crate) const RUN: &str = crate::qualification::reserved_paths::MACOS_SOURCE_TEST_RUN;

/// The run's lock, created if need be, or `None` while another holder keeps it. A lock that does
/// not exist yet is created only after the run record is gone (and its removal synced): a record
/// and receipt left without a lock never counted, and a new lock must not make them count, even
/// if this run stops before it records itself (fix round, BLOCKER 2).
#[cfg(target_os = "macos")]
pub(super) fn lock_for_run(root: &std::path::Path) -> Result<Option<std::fs::File>, String> {
    let path = root.join(LOCK);
    let marker = root.join(RUN);
    if !refuse_link(&path)? && refuse_link(&marker)? {
        std::fs::remove_file(&marker)
            .map_err(|error| format!("cannot remove {}: {error}", marker.display()))?;
        if let Some(parent) = marker.parent() {
            std::fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("cannot sync {}: {error}", parent.display()))?;
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => {
            Err(format!("cannot lock {}: {error}", path.display()))
        }
    }
}

/// What a reader finds at the run's lock.
#[cfg(target_os = "macos")]
pub(super) enum Reading {
    /// No run holds it; the shared lock (if the file exists) keeps one from starting meanwhile.
    Free {
        /// Whether the lock file exists (and is now held shared).
        held: bool,
        /// Held to keep a run from starting meanwhile; never read.
        _lock: Option<std::fs::File>,
    },
    /// A run is in progress: it has retired the old receipt and not published its own.
    Busy,
}

/// The run's lock for a reader: opened read-only, never created (the gate writes nothing), and
/// absent only when no run ever started.
#[cfg(target_os = "macos")]
pub(super) fn lock_for_reading(root: &std::path::Path) -> Result<Reading, String> {
    let path = root.join(LOCK);
    refuse_link(&path)?;
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Reading::Free {
                held: false,
                _lock: None,
            });
        }
        Err(error) => return Err(format!("cannot open {}: {error}", path.display())),
    };
    match file.try_lock_shared() {
        Ok(()) => Ok(Reading::Free {
            held: true,
            _lock: Some(file),
        }),
        Err(std::fs::TryLockError::WouldBlock) => Ok(Reading::Busy),
        Err(std::fs::TryLockError::Error(error)) => {
            Err(format!("cannot lock {}: {error}", path.display()))
        }
    }
}

/// `phase 18`'s check of the ignored-test table on a Mac (plan 11 #macos-readiness-check, the
/// Phase 16 row classification; whole-gate review A1): the eight packages' test targets and their
/// `--list --ignored`, found and listed with the runner's own commands (in Cargo's default build
/// directory, which this never empties), must name exactly the table's tests, so a renamed or new
/// ignored test is caught before it is merged rather than on R. A target that cannot be listed
/// fails the check, since its ignored tests are unknown.
#[cfg(target_os = "macos")]
pub(super) fn ignored_table(root: &std::path::Path) -> Result<(), String> {
    let steps = steps::Steps::in_default_directory(root);
    let (executables, doc) = steps::discover(&steps)?;
    let (listed, mut problems) = steps::listings(&steps, &executables, &doc);
    problems.extend(ignored::differences(&listed));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the ignored-test table ({}) does not match the targets' own listings: {}",
            "tools/stickymd-smoke/src/release/macos/ready/source_tests/ignored.rs",
            problems.join("; ")
        ))
    }
}

/// Whether `path` exists, refusing a link (it would be locked or replaced as the link) and any
/// error but absence (review round 4).
#[cfg(target_os = "macos")]
pub(super) fn refuse_link(path: &std::path::Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(format!("{} is a symbolic link", path.display()))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect {}: {error}", path.display())),
    }
}

/// The eight packages the row names, in the order of `release-macos`'s self-check.
pub(super) const PACKAGES: [&str; 8] = [
    "stickymd-core",
    "stickymd-render",
    "stickymd-app",
    "stickymd-desktop",
    "stickymd-posix",
    "stickymd-mac",
    "stickymd-package",
    "stickymd-smoke",
];

/// P18-L08's two tests: the default volume (in the default set) and the case-sensitive one
/// (`#[ignore]`, it mounts a disk image), both run explicitly (plan 11 #macos-release-gate).
pub(super) const SAME_KEY_TESTS: [&str; 2] = [
    "note_root::tests::one_key_is_exclusive_and_another_key_runs_beside_it",
    "note_root::tests::on_a_case_sensitive_volume_case_makes_another_key",
];

/// P16-S06's test (`#[ignore]`, it mounts an exFAT disk image).
pub(super) const S06_TESTS: [&str; 1] =
    ["paths::tests::an_exfat_volume_is_refused_and_keeps_no_probe_file"];

/// The explicit runs the receipt carries, by gate item, in this order (decision (a)).
pub(super) const EXPLICIT: [(&str, &[&str]); 2] =
    [("same-key", &SAME_KEY_TESTS), ("P16-S06", &S06_TESTS)];

/// The binary the explicit runs select.
pub(super) fn explicit_target() -> Target {
    Target {
        package: "stickymd-mac".to_owned(),
        kind: "bin".to_owned(),
        name: "stickymd-mac".to_owned(),
    }
}

/// The one command the row allows: `cargo test --locked`, the eight packages, no filter.
pub(super) fn command() -> Vec<String> {
    let mut argv = vec!["cargo".to_owned(), "test".to_owned(), "--locked".to_owned()];
    for package in PACKAGES {
        argv.push("-p".to_owned());
        argv.push(package.to_owned());
    }
    argv
}

/// Where the runner builds the `mac-gui` driver: its own directory, emptied before every build, so
/// no cache another commit left can stand in for R's build (fix round, BLOCKER 1). The path is
/// fixed because the bytes depend on it (the linker's UUID does): a rerun on the same checkout
/// builds the same driver, and the VM runs use that file.
pub(super) const DRIVER_TARGET: &str = "target/macos-driver";

/// How the runner builds the `mac-gui` driver on R: the README's build, into `DRIVER_TARGET`; it
/// records the SHA-256 of what Cargo built (review round 4, BLOCKER 1). The first entry is the
/// environment the build is given.
pub(super) fn driver_command() -> Vec<String> {
    [
        "CARGO_PROFILE_RELEASE_PANIC=unwind",
        "cargo",
        "build",
        "--locked",
        "--release",
        "-p",
        "stickymd-smoke",
        "--target-dir",
        DRIVER_TARGET,
        "--message-format=json",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The explicit run of `tests`, ignored ones included, by exact name.
pub(super) fn explicit_command(tests: &[&str]) -> Vec<String> {
    let mut argv: Vec<String> = [
        "cargo",
        "test",
        "--locked",
        "-p",
        "stickymd-mac",
        "--bin",
        "stickymd-mac",
        "--",
        "--exact",
        "--include-ignored",
    ]
    .map(str::to_owned)
    .to_vec();
    argv.extend(tests.iter().map(|test| (*test).to_owned()));
    argv
}

/// One libtest run: a test binary or a library's doc-tests, named without paths.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Target {
    pub package: String,
    /// Cargo's target kinds joined by `,` (`lib`, `bin`, `test`, …), or `doctest`.
    pub kind: String,
    pub name: String,
}

/// libtest's summary of one run: `running <ran> tests` and its `test result:` line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Counts {
    pub ran: u64,
    pub passed: u64,
    pub failed: u64,
    pub ignored: u64,
    pub measured: u64,
    pub filtered_out: u64,
}

impl Counts {
    /// libtest's own arithmetic: every test it ran ended in exactly one of these. Summed without
    /// wrapping, so no recorded count can overflow into agreement.
    fn adds_up(&self) -> bool {
        [self.failed, self.ignored, self.measured]
            .into_iter()
            .try_fold(self.passed, u64::checked_add)
            == Some(self.ran)
    }
}

/// What the run printed for one test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CaseOutcome {
    Ok,
    Failed,
    Ignored,
    /// Listed by the binary but not printed by the run.
    NotRun,
}

impl CaseOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Ignored => "ignored",
            Self::NotRun => "not-run",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "ok" => Self::Ok,
            "failed" => Self::Failed,
            "ignored" => Self::Ignored,
            "not-run" => Self::NotRun,
            _ => return None,
        })
    }
}

/// One test: its full libtest name, whether its binary lists it as ignored, and its outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Case {
    pub name: String,
    pub ignored: bool,
    pub outcome: CaseOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Run {
    pub target: Target,
    /// libtest's summary; `None` when the output stopped before it (a failure printed before
    /// that is still kept in `cases`, fix round, BLOCKER 3).
    pub counts: Option<Counts>,
    pub cases: Vec<Case>,
}

/// How a process ended: its exit code, a signal, never started, or not known (it was started but
/// could not be followed to its end). Only the first two are results (review round 4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Exit {
    Code(i32),
    Signal,
    NotStarted,
    Unknown,
}

/// The `mac-gui` driver the run built on R: the command, and the SHA-256 of what Cargo built or
/// why there is none.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Driver {
    pub command: Vec<String>,
    pub sha256: Result<String, String>,
}

/// The source identity at one end of the run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Side {
    pub head: String,
    pub clean: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Host {
    pub product_name: String,
    pub product_version: String,
    pub build_version: String,
    pub arch: String,
    /// `kern.hv_vmm_present`: `1` in a virtual machine.
    pub virtual_machine: String,
    pub cargo: String,
    pub rustc: String,
}

/// One explicit run of ignored tests for a gate item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Explicit {
    pub item: String,
    pub command: Vec<String>,
    pub tests: Vec<String>,
    /// The binary the run's one section belongs to, when it could be named.
    pub target: Option<Target>,
    pub counts: Option<Counts>,
    pub cases: Vec<Case>,
    /// Of `tests`, those stickymd-mac's binary names in its own listing, taken before any run, so
    /// the check does not need the main run to have reached that binary (review round 4).
    pub listed: Vec<String>,
    pub exit: Exit,
    /// P18-L08's two volumes as its tests measured them on the directories their assertions use
    /// (`case-insensitive` or `case-sensitive`), reported through `STICKYMD_VOLUME_REPORT`;
    /// `None` when not reported, and for P16-S06.
    pub temporary_volume: Option<String>,
    pub image_volume: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Receipt {
    pub release_source: String,
    /// The identity of this run, as `RUN` holds it while this receipt stands.
    pub run: String,
    pub host: Host,
    pub start: Side,
    pub end: Side,
    pub command: Vec<String>,
    pub exit: Exit,
    pub expected: Vec<Target>,
    pub binaries: Vec<Run>,
    pub explicit: Vec<Explicit>,
    pub driver: Driver,
    /// What the runner could not establish (discovery or output it could not read).
    pub problems: Vec<String>,
}

/// A receipt that judges PASS on every verdict, for `expected` (tests only): each binary holds
/// three passing tests; stickymd-mac's binary also lists the explicit runs' tests, the ignored
/// ones as ignored, and the explicit runs ran exactly them.
#[cfg(test)]
pub(super) fn fixture(release: &str, expected: Vec<Target>) -> Receipt {
    let case = |name: &str, ignored: bool| Case {
        name: name.to_owned(),
        ignored,
        outcome: if ignored {
            CaseOutcome::Ignored
        } else {
            CaseOutcome::Ok
        },
    };
    let binaries = expected
        .iter()
        .map(|target| {
            let mut cases: Vec<Case> = (0..3)
                .map(|i| case(&format!("{}::tests::t{i}", target.name), false))
                .collect();
            if *target == explicit_target() {
                cases.push(case(SAME_KEY_TESTS[0], false));
                cases.push(case(SAME_KEY_TESTS[1], true));
                cases.push(case(S06_TESTS[0], true));
            }
            let ignored = cases.iter().filter(|c| c.ignored).count() as u64;
            Run {
                target: target.clone(),
                counts: Some(Counts {
                    ran: cases.len() as u64,
                    passed: cases.len() as u64 - ignored,
                    ignored,
                    ..Counts::default()
                }),
                cases,
            }
        })
        .collect();
    let explicit = EXPLICIT
        .iter()
        .map(|(item, tests)| Explicit {
            item: (*item).to_owned(),
            command: explicit_command(tests),
            tests: tests.iter().map(|t| (*t).to_owned()).collect(),
            target: Some(explicit_target()),
            counts: Some(Counts {
                ran: tests.len() as u64,
                passed: tests.len() as u64,
                filtered_out: 10,
                ..Counts::default()
            }),
            cases: tests
                .iter()
                .map(|t| Case {
                    name: (*t).to_owned(),
                    ignored: *t != SAME_KEY_TESTS[0],
                    outcome: CaseOutcome::Ok,
                })
                .collect(),
            listed: tests.iter().map(|t| (*t).to_owned()).collect(),
            exit: Exit::Code(0),
            temporary_volume: (*item == "same-key").then(|| "case-insensitive".to_owned()),
            image_volume: (*item == "same-key").then(|| "case-sensitive".to_owned()),
        })
        .collect();
    Receipt {
        release_source: release.to_owned(),
        run: "1764000000000000000-4242".to_owned(),
        host: Host {
            product_name: "macOS".into(),
            product_version: "27.2".into(),
            build_version: "27C55".into(),
            arch: "arm64".into(),
            virtual_machine: "0".into(),
            cargo: "cargo 1.97.1".into(),
            rustc: "rustc 1.97.1".into(),
        },
        start: Side {
            head: release.into(),
            clean: true,
        },
        end: Side {
            head: release.into(),
            clean: true,
        },
        command: command(),
        exit: Exit::Code(0),
        expected,
        binaries,
        explicit,
        driver: Driver {
            command: driver_command(),
            sha256: Ok("1".repeat(64)),
        },
        problems: Vec::new(),
    }
}
