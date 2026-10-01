# StickyMD Phase Smoke CLI

`stickymd-smoke` is a development-only, std-only Rust CLI. It is the reusable
execution engine behind the stable PowerShell entry points in `tools/smoke/`.
It is not linked into `StickyMD.exe` and is not included in the portable
release package.

## Stable entry points

```powershell
./tools/smoke/phase-00.ps1
./tools/smoke/phase-01.ps1 -Performance
./tools/smoke/phase-02.ps1 -Performance
./tools/smoke/phase-03.ps1 -Performance -Runtime
./tools/smoke/phase-04.ps1 -Performance -Runtime
./tools/smoke/phase-05.ps1 -Performance -Runtime
./tools/smoke/phase-05.ps1 -Resources
./tools/smoke/phase-06.ps1 -Performance -Runtime
./tools/smoke/phase-06.ps1 -Resources
./tools/smoke/phase-07.ps1 -Performance -Runtime
./tools/smoke/phase-07.ps1 -Resources
./tools/smoke/phase-14.ps1 -G3
./tools/smoke/phase-14.ps1 -G3 -G3Case G3-05
./tools/smoke/phase-14.ps1 -G4
./tools/smoke/phase-14.ps1 -G4 -G4Case G4-02
./tools/smoke/all.ps1 -Ci
```

All phases (00–14, including 11-b) and `all.ps1` retain their PowerShell parameter
names, types, order and per-entry availability. Their shared
`invoke-phase.ps1` adapter forwards bound values to `phase-entry`; Rust owns action
exclusivity, required companions, mappings and window-stress defaults. Semantic
values reuse the existing canonical parsers. Common PowerShell parameters stay in
the shell; explicit false switches and zero cycle counts remain distinguishable
from absent parameters. CWD and console encoding are restored on success/failure.
The legacy Phase 12/13 `-Candidate` flag still reaches the unsupported legacy command;
it is not silently reinterpreted as Source Freeze.
`all` shard selection and Phase 10/14 resource-module values use the same Rust
parsers as direct CLI calls. Unsupported combinations still fail before execution;
the shared adapter does not make newer phase flags available to older entries.

For offline route inspection without qualification, GUI work or receipts:

```powershell
cargo run --quiet -p stickymd-smoke --locked -- phase-entry-plan 14 --WindowStress=true --TrayCycles=0
cargo run --quiet -p stickymd-smoke --locked -- phase-entry-plan all --Ci=true --CiShard=tests
```

`phase-entry-plan` validates through the same router and canonical CLI, then emits
one JSON document with `status: NOT_RUN` and the mapped arguments. `phase-entry`
executes the mapped existing command, preserving its output and exit code.

For a local diagnosis of one opt-in resource case, set
`STICKYMD_SMOKE_RESOURCE_CASE` to the exact case label before invoking the owning phase script.
This development filter is never set by CI or by the durable full-matrix receipts.

`-Ci` runs every headless check, including the Release performance entry
points. Stable hard thresholds may fail CI; machine-specific measurements are
diagnostic only. `-Performance` reruns the same measurements explicitly on a
local machine. `-Runtime` creates native windows and remains local-only. The
CLI rejects combining either explicit local mode with `-Ci`.

Phase 14 performance includes both Preview selection geometry and the Windows
1 MiB Unicode case-insensitive Source search benchmark. The latter also runs once
in `all --ci`'s performance shard and `modules run windows --mode=performance`;
test-only and render-only plans do not select it. Both use locked Release builds
and serial test execution, preserving their separate Cargo feature contexts.

`phase-14.ps1 -G3` is the serial exact-candidate Windows desktop lane for
clipboard, native export, process-kill recovery, and asset-safety checks. Rust
owns isolation, assertions, and the exact receipt. The checked-in UI Automation
helper only selects a native export path or invokes tray Exit. Its headless
parser/receipt tests are included in `all --ci`; GitHub-hosted CI never starts
the interactive G3 lane.

`-G3Case G3-01..G3-05` runs one module for fast diagnosis and writes a
case-suffixed receipt. A targeted receipt is intentionally insufficient for
release readiness; only the default full five-case receipt can close G3.
Because Explorer tray elements do not expose the owning application PID, G3
fails closed when any StickyMD process already exists and rechecks sole-process
ownership before tray Exit. It never terminates an unrelated user instance.

`phase-14.ps1 -G4` reuses the same exact-candidate lifecycle for five serial,
isolated groups: tray lifecycle, primary-monitor three-edge docking/timing,
legacy clipboard shortcuts, toolbar math conversion, and junction identity.
`-G4Case G4-01..G4-06` is diagnostic only. G3 and G4 must run sequentially on
an exclusive interactive desktop; their receipts are independently required.
Mixed-DPI Left/Right sensor behavior remains a guided human observation.

`-Resources` is the long-running Windows resource measurement. It launches copied standalone
Release executables, waits 30 seconds, records private working set/private bytes over five runs,
and measures 60-second idle CPU for Source, Preview and Split. It is never part of headless CI.

The complete Phase 14 resource entry is:

```powershell
cargo run --quiet -p stickymd-smoke --locked -- phase 14 --resources --json --evidence-file=dist/evidence/resources-qualification.json
cargo run --quiet -p stickymd-smoke --locked -- qualification modules
```

With a clean Source Freeze and valid Promoted Candidate, the full entry selects incompatible
groups from `source-preview`, `math`, `images`, `window`, and `zoom`. Each complete group
saves its own last-success receipt; a later failure preserves earlier successes. Readiness
requires all five compatible receipts. The aggregate JSON is a progress/source summary;
legacy aggregate success is not automatically imported. Reused results retain their original
source/EXE/ZIP identity and evidence hash. `qualification modules` reports missing, changed
or invalid receipts without launching desktop tests.

Each group archive contains its complete raw observations and existing hard gates. Shared
cohorts copy those observations with a `shared_from` origin; they remain the same five samples.
Recording/readiness validates run coverage, units, summary consistency and hard-gate results.
Count markers or `PASSED` alone cannot establish completeness.

The baseline 20 KiB Source/Preview/Split cohorts enforce private-working-set
maxima of 40/52/64 MiB. Their exact math aliases, per-case diagnostic cache and
group receipts consume the same Rust limits; an archive lacking the memory gate
is rejected even when its samples are below the limit. Other image/cache stress
observations do not silently gain a newly invented threshold.

Zoom measures Source, Preview and Split separately at 50/100/300%, with five
fresh-process observations per combination and a 30-second warmup. Source and
Preview cohorts never visit another view before sampling. The existing Split
toolbar checks and 100-cycle growth check remain. Coverage, wait budgeting and
execution use `resource_plan/zoom.rs`; old three-cohort or five-second-warmup
receipts cannot claim the current matrix. Every observation also measures the
full 60-second idle CPU interval. The fixed wait budget is 4,050 seconds,
excluding launch/probe/stress costs; memory-only archives fail current coverage.

The Window group additionally compares Source/Preview/Split at default 520×680
and compact 220×120 DIP, plus a default-size normal-style control for each view.
`resource_plan/window_comparison.rs` owns these nine cohorts. Each has five
fresh processes, 30-second warmup and 60-second CPU sampling. Matched arms use
identical note bytes, 100% zoom and the same DPI; their order rotates across
repetitions. The control changes only the disposable child's APPWINDOW/TOOLWINDOW
bits after normal startup, so it measures steady-state style differences, not
startup in an alternative product mode. Geometry/style are checked during every
CPU bucket; before/after DPI, size, handle/GDI/USER counts are retained. Invalid
facts or partial cohorts cannot complete the Window group. Comparisons add
4,050 seconds of fixed wait. Tool tests do not close P14-A46 or manual UX gates.

Equivalent scenarios share a complete cohort within one command: the 19 Source/Preview,
math and image names require 15 executions. The fixed waiting budget decreases by
1,500 seconds (25 minutes); this is a calculated budget, not a measured speedup.
Deduplication preserves each registered case's counts, warmup, CPU interval and gates. Operation history
is part of scenario identity, so Preview-to-Source cache release is a separate execution.
The command reports its waiting budget, per-case/group elapsed time and shared sample origins.

Runtime, Performance and Resources requests that consume a Promoted Candidate validate its
source and staged EXE/ZIP/SBOM instead of building an unused local Release EXE. The plan records
this as candidate verification, not a build. A missing or invalid candidate fails closed;
local preflight, packaging, release builds and headless CI keep their existing build steps.

Resource any/max gates stop after the first complete over-limit observation. Idle CPU still
uses the entire 60-second average, never an individual 10-second bucket. A failure preserves
raw memory/CPU observations, actual counts, thresholds and the error in JSON, leaves successful
ledgers intact, and stops later work. Successful cohorts still collect all five samples;
startup p95 sampling is unchanged. When the first CPU sample fails, skipping four remaining
30+60-second trials saves 360 seconds of fixed waiting in that cohort (a budget, not a benchmark).

`--resource-module=<group>` and `STICKYMD_SMOKE_RESOURCE_CASE` remain diagnostics.
Use a separate output such as `dist/evidence/window-diagnostic.json`; partial requests
are rejected when pointed at formal receipts. Diagnostics never replace last-success.
Filesystem aliases, including Windows verbatim/short paths and junctions, receive the same
protection before execution and writing; this also covers output files not yet created.
Ordinary Windows paths with trailing dots/spaces are checked before creation as well;
verbatim paths retain their literal identity. Ordinary paths with dots/spaces at the end
of an intermediate directory are rejected because ancestor lookup can change their meaning.
The entire `dist/evidence/module-success/`
ledger/archive directory is internal, including for G3/G4/G5 diagnostic output.
Resource reuse rechecks the current ledger, archived bytes and complete coverage at the
point of use; planning only decides which groups need measurement. If evidence disappears
or becomes invalid during the campaign, reuse fails without overwriting previous success.
An interrupted window stress group reruns the whole group. All native measurements still
require an exclusive interactive Windows desktop; the new harness has not yet received
its full desktop acceptance (P14-A46).

## Shared Runtime / Performance prerequisite

Full Phase 14 Runtime and Performance requests using their canonical evidence paths
share a successful `cargo test --workspace --locked` prerequisite when its inputs match:

```powershell
./tools/smoke/phase-14.ps1 -Runtime -EvidenceFile dist/evidence/runtime-qualification.json
./tools/smoke/phase-14.ps1 -Performance -EvidenceFile dist/evidence/performance-qualification.json
```

The source-only receipt at `dist/evidence/source-success/workspace-tests.json` binds the
clean Source Freeze, repository bytes, actual smoke executable, Rust/Cargo versions,
host/work directory and execution settings. Identity is checked again after execution
or before reuse. Failures and input changes preserve the previous success; a successful
prerequisite remains reusable even when a later desktop task fails. Partial requests,
ordinary diagnostics and CI cannot populate this receipt.

Unknown Rust/Cargo overrides, StickyMD test filters and unfamiliar Cargo configuration
disable sharing and run the full test command normally. Supported configuration is empty
or the repository's static MSVC CRT setting. Cargo's ordinary Rustup launch metadata is
included in the identity. Receipts contain hashes, never raw environment/configuration
values. A malformed, incomplete or mismatched receipt causes a fresh full test run.
Relative `CARGO_HOME` resolves from Cargo's repository working directory, including when
the smoke command itself starts in a repository subdirectory.
Empty `CARGO_HOME` uses the user-home fallback. Windows drive-relative values such as
`C:cache` disable sharing and run the complete test command normally.

`REUSED_PASS` reports its source and fingerprint. `workspace.origin_run_seconds` is
historical; `workspace.run_seconds` exists only for a new run. Task timing is emitted as
`TASK_TIMING` and, when the task has a result, `task.execution_seconds` in JSON. Identity
checks have separate measurements. Resource planning enumerates paths once and streams
each shared input once into buffered, independent v1 fingerprints. `RESOURCE_FINGERPRINT_BATCH`
reports batch time and actual source files/bytes read; `RESOURCE_COMPATIBILITY` reports
each group's receipt checks. Scratch streams are flushed before hashing and cleaned on
success, error or unwind. Reuse and promotion still recompute current inputs. Product dependency scopes
remain conservative; this optimization does not establish native acceptance or measured
end-to-end savings. See [plan 11](../../docs/plan/11_testing_and_release.md#shared-headless-prerequisite).

The optional local comparison reads the checkout without starting StickyMD or writing
qualification evidence; it is ignored by normal test runs:

```powershell
cargo test -p stickymd-smoke --bin stickymd-smoke --locked resource_planning_profile -- --ignored --nocapture --test-threads=1
```

## Optional startup shell details

Startup observations also retain cumulative process CPU time when the observer
receives EDITOR_READY, and the lag between observing ready and completing the CPU
query. The wall timer stops before that query. Process CPU sums all threads, can
exceed elapsed time, and may include work just after ready; subtracting it from
wall time does not establish scheduler or I/O wait. This adds no gate or milestone.
If later startup sampling fails, completed raw samples and their actual counts
are retained without percentiles for the incomplete cohort; the result stays failed.

For targeted native investigation, the existing Rust test binary provides three
explicit opt-in diagnostics. Set `STICKYMD_SMOKE_PROBE_REPOSITORY` to a clean
checkout with a valid local Release build or matching promoted candidate, then run
one command at a time on an exclusive desktop:

```powershell
cargo test -p stickymd-smoke --locked --bin stickymd-smoke native_startup_cpu_diagnostic -- --ignored --nocapture --test-threads=1
cargo test -p stickymd-smoke --locked --bin stickymd-smoke native_zoom_cpu_diagnostic -- --ignored --nocapture --test-threads=1
cargo test -p stickymd-smoke --locked --bin stickymd-smoke native_window_comparison_diagnostic -- --ignored --nocapture --test-threads=1
```

These reuse the real startup/resource executors and candidate resolver. Frozen
checkouts cannot fall back to an unrelated local EXE. Results, including failures,
are written to unique `tmp/native-diagnostics/` directories in the probe checkout;
startup traces are archived before fixture cleanup. Source/EXE/harness identities
are checked before and after. They do not write success ledgers, and targeted
window comparisons cannot stand in for the full Window stress/hidden matrix.

For an isolated copied Release diagnostic, set both
`STICKYMD_DIAGNOSTIC_STARTUP_TRACE` (the legacy v2 output path) and
`STICKYMD_DIAGNOSTIC_STARTUP_DETAILS` (a separate, new sidecar path) on the child
process. With no legacy trace path, details stay disabled. Existing files are never
overwritten; use fresh paths for every run. `STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY=1`
exits after the first ready frame even when capture fails: success returns 0;
ready-event/trace/sidecar failure is reported on stderr and returns 1 after normal
app cleanup. Without that explicit exit request, a diagnostic failure leaves the
interactive app running. Collect GUI diagnostics serially in an isolated
portable directory; do not reuse the user's running note or candidate receipts.

```powershell
cargo run --quiet -p stickymd-smoke --locked -- startup-details --trace 'tmp/启动 trace.txt' --details 'tmp/启动 details.txt' --json
```

The analysis command also runs outside a repository using the compiled CLI. It
reads UTF-8 regular files of at most 8 KiB and never starts a process or writes a
receipt. Output is `OBSERVATION_ONLY`, in microseconds: Split-mode application,
showing the window, reasserting tool-window identity, and the remaining time within
`tray_ready..window_visible`. Missing, duplicate, out-of-order, nonmonotonic or
out-of-interval timestamps, unsupported versions and mismatched trace pairs fail
with exit 1 and no partial stdout.
The `legacy_trace_begin` marker must occupy its own complete LF/CRLF-terminated
line; a marker attached to the preceding value is invalid.

The optional sidecar stores six bounded timestamps, PID and the exact v2 trace
text. The reader requires byte-for-byte equality with the supplied v2 file; it
does not treat PID or diagnostic text as release identity. Legacy v2 remains the
same 26 milestones. Both files are written after the ready event using the existing
atomic no-replace adapter. The formal startup harness explicitly disables details,
preserving its cohort, readiness definition and gates. This adds attribution detail,
not evidence of faster startup or a new successful qualification.

## Explicit headless modules

```powershell
cargo run --quiet -p stickymd-smoke --locked -- modules list
cargo run --quiet -p stickymd-smoke --locked -- modules run core
cargo run --quiet -p stickymd-smoke --locked -- modules run render,windows --mode=performance
cargo run --quiet -p stickymd-smoke --locked -- modules run all --mode=all --plan
```

The six module names are `core`, `render`, `windows`, `smoke`,
`phase1-markdown-math`, and `phase1-persistence`. The default mode is `tests`;
`performance` selects the existing headless Release baselines, and `all`
selects both. Cargo still builds dependencies as needed. Explicit selection
does not automatically add dependent modules: callers choose the validation scope.
The `windows` module requires Windows to execute; plans can be inspected on any host.

The module adapter narrows the existing full plan's Cargo package selectors.
It keeps the same filters, profiles, thresholds and test-harness arguments;
selecting every module preserves the original plan exactly. Shared commands
are run once per request. Governance runs for every request. Workspace membership
comes from Cargo; an unregistered member or task target fails closed.

Release test recipes share one Rust implementation in `runner/performance.rs`.
Phase 6/7 select the three product packages, keeping unit and integration targets
eligible while avoiding the smoke tool's empty Release test programs. Standalone
render-library recipes retain their original feature context and `--lib` scope;
they are not widened to the Windows graph. Every measurement still uses the locked
Release profile and runs serially. Cargo owns artifact freshness and compilation
parallelism; no executable path cache bypasses its source checks.

`--plan` prints one JSON document marked `NOT_RUN` and does not execute tests.
Execution stops on failure and returns a nonzero exit code. A module run reports
only its requested scope and writes no qualification or last-success receipt.
The existing `all --ci` entry remains the complete headless check. Automatic CI
selection is a separate planner, described below.

## Local change-based checks

```powershell
cargo run --quiet -p stickymd-smoke --locked -- dev-check --plan
cargo run --quiet -p stickymd-smoke --locked -- dev-check
cargo run --quiet -p stickymd-smoke --locked -- dev-check --mode=all --plan
```

`dev-check` observes staged, unstaged and nonignored untracked inputs relative to
HEAD. Deletions and moves retain both original and destination owners. It reuses
the CI path classifier, Cargo module registry and reverse dependency closure;
the existing commit-only CI still falls back on dirty worktrees. Unknown inputs,
conflicts, invalid Git facts and registry drift select conservative full checks.
The `--plan` JSON is `NOT_RUN`, with observed paths, module/check reasons and exact
commands. It does not build, execute checks or write acceptance receipts.

The default mode is `tests`; `performance` and `all` include the existing explicit
headless Release baselines. Shared governance and formatting always run; applicable
Clippy, dependency policy and Windows Release/native checks come from the same CI
policy. Cargo invocations use locked dependencies. Tasks run in order, Cargo
manages compilation and independent libtest cases can overlap. Performance
measurements remain serial. A Windows selection requires Windows to execute.
The local native-runtime gate verifies the executable reported by this run's
successful Cargo build, including custom target directories; it does not resolve
a previously qualified candidate as a substitute for that output.
Local success only covers the requested checks; the command has no qualification
or last-success writer. Commit changes are outside its comparison scope: after
committing, use the existing CI base comparison or explicit module/full entry.

## Timing observations

```powershell
cargo run --quiet -p stickymd-smoke --locked -- timings --input target/diagnostics/math.json --json
cargo run --quiet -p stickymd-smoke --locked -- timings --input target/check.log --input target/diagnostics/math.progress.json
```

`timings` reads existing UTF-8 JSON/log files from any working directory. It
accepts evidence schema 2, resource progress/diagnostic-plan and shared-workspace
schema 1, existing `TASK_TIMING`/resource telemetry and Cargo's explicit compile/
test elapsed records. ANSI SGR color codes are ignored in logs, including CI's
forced-color output. Each input is an independent scope. The report keeps current
tasks, nested/group/identity observations, historical origins and fixed-wait
budgets separate. Missing timings stay missing. Inputs are read before stdout is
emitted; malformed values, ambiguous duplicates or a later invalid file return
nonzero without a partial report. Input files are preserved.

The summary is `OBSERVATION_ONLY`: it does not verify source/receipt identity or
grant acceptance. Overlapping task/subtask measurements and multiple files are
not summed. Wall-clock and agent-work totals remain unknown. A compile-profile
elapsed record includes Cargo work; it cannot distinguish lock/queue waits or
active compiler time. Fixed-wait budgets are predictions, and historical elapsed
values describe their origin run. No speedup is inferred from a summary.

For a development batch, inspect `dev-check --plan`, run the selected checks, then
summarize the captured log. PowerShell 5.1/7 can both write an accepted UTF-8 log:

```powershell
cargo run --quiet -p stickymd-smoke --locked -- dev-check 2>&1 |
    ForEach-Object { $_.ToString() } |
    Set-Content -LiteralPath target/dev-check.log -Encoding UTF8
$checkExitCode = $LASTEXITCODE
cargo run --quiet -p stickymd-smoke --locked -- timings --input target/dev-check.log --json
if ($checkExitCode -ne 0) { throw "Local checks failed with exit code $checkExitCode" }
```

For an affected resource case, use the existing Phase 14 module selector; inspect
`-ResourceResume -ResourceFailureFirst -ResourcePlan` before explicitly running
the diagnostic without `-ResourcePlan`. Resume needs an ignored `target/` evidence
path and the complete existing identity. Failures run fresh; unchanged successful
cases may reuse validated history. These diagnostics preserve the sampling
protocol and do not produce formal resource success or manual acceptance.

## Change-based CI

```powershell
cargo run --quiet -p stickymd-smoke --locked -- ci plan --base=<full-commit-sha>
cargo run --quiet -p stickymd-smoke --locked -- ci plan --full
```

The planner compares the supplied base with the actual checkout's HEAD. On PRs
the base is the target branch commit and HEAD is GitHub's checked-out merge commit;
on pushes the base is the event's `before` commit. It selects changed modules and
their reverse dependencies: core selects core/render/windows, render selects
render/windows, and smoke-only changes select smoke.

Shared contracts/build inputs, CI/planner changes, unknown paths, missing or zero
SHAs, a dirty worktree, failed Git comparisons and Cargo registry drift select
the original full CI shards. Renames retain both old and new owners. The shared
rendering-stress fixture also selects full CI because the smoke harness embeds it.
Known documentation-only changes retain fmt and governance. Planner regressions
run with the smoke module; changes to the planner itself select complete CI.

The Rust plan selects a separate Linux smoke job for strict CLI Clippy and tests
when it selects `smoke` or full coverage. After planning, that job runs alongside
the other isolated CI jobs and remains required by the aggregate result gate.
Windows desktop executors are compiled only for Windows; their pure
classification/repetition rules retain Linux unit coverage.
Shared evidence status names remain stable. A GUI qualification request on Linux
reports `UNSUPPORTED` / `NOT_TESTED` and returns a nonzero exit code.

Rust also projects the applicable lint, Linux portability, dependency-policy and
Windows Release checks. The GitHub adapter forwards those arguments and runs
selected modules on isolated runners. The stable `CI result` job uses `ci verify`
to reject failures, cancellations, missing results and unexpected skipped jobs.
`NOT_RUN` plans and partial job success are not qualification receipts.

Manual full runs, scheduled maintenance and release preparation retain complete
checks. Scheduled maintenance reuses the CI workflow; release and promotion
workflows preserve their existing artifact gates. Cargo download caches share an
OS/dependency identity across lanes; build caches additionally bind the lane and
toolchain. Both rotate by commit and restore only their matching prefix. A first
writer may have downloaded only part of the graph; Cargo fetches missing entries
normally and a cache hit never skips checks. Incremental compilation stays enabled
where Cargo enables it by default. Caches retain downloads/build outputs, never
candidate ledgers or note data. Daily CI superseded
by a newer commit can be cancelled. Actual savings require remote workflow timing.

The governing contract is [plan 11](../../docs/plan/11_testing_and_release.md#modular-headless-ci).

## Package path adapter

`tools/release/package-path.ps1` retains `Resolve-StickyMdPackagePath` and
delegates selection to `package-path --directory <path>`. Rust owns candidate
enumeration and clean/dirty local archive naming. A lone matching ZIP is returned
as before; selection does not prove its checksum, provenance or release readiness.
The PowerShell wrapper resolves relative paths against the caller's PowerShell
location, preserves Unicode paths and restores the caller's working
directory and console encoding on success and failure. ZIP processing and native
UI Automation remain platform adapters.

## Release tooling

The existing PowerShell parameter interfaces remain available:

```powershell
./tools/release/package.ps1 -OutputDirectory <directory> -AllowDirtyValidation
./tools/release/verify-package.ps1 -PackageDirectory <directory> [-ZipPath <zip>] [-ChecksumPath <manifest>] [-Runtime]
./tools/release/verify-promoted-artifact.ps1 -ArtifactDirectory <directory> -SourceSha <full-sha> -ExpectedZipSha256 <sha256> -ExpectedSbomSha256 <sha256> -ReleaseTag <release-tag>
./tools/release/generate-third-party-notices.ps1 -DestinationPath <new-file>
./tools/release/generate-sbom.ps1 -PackageDirectory <directory> [-ZipPath <zip>] [-OutputPath <sbom>] [-SyftPath <syft>]
```

Their reusable commands are in the existing std-only CLI:

Replace `<release-tag>` with the tag for the approved source and artifact version.
Verification binds it to the checkout's full source SHA and workspace version; an old
release tag is not a reusable default for a later checkout or candidate.

Artifact, cache and evidence SHA-256 share `integrity`. On Windows its CNG adapter
streams each newly opened input through a bounded 64 KiB buffer, including empty
files, and returns failure for read/CNG errors. It neither implements cryptography
nor caches digests. Other platforms retain their existing `sha256sum` adapter.
Known vectors, interrupted/short/error reads, locked/missing inputs and fresh reads
after file changes are covered by tests; tool timings are not product benchmarks.
Measurements, wrapper overhead and verification limits are recorded in the
[routing/workflow/CNG report](../../docs/report/2026-09-30-phase-routing-release-workflows-cng.md).

```powershell
cargo run --quiet -p stickymd-smoke --locked -- release package-inputs --allow-dirty-validation
cargo run --quiet -p stickymd-smoke --locked -- release prepare-package --exe <exe> --staging-directory <new-directory> [--allow-dirty-validation]
cargo run --quiet -p stickymd-smoke --locked -- release build-package [--exe <exe>] [--output-directory <directory>] [--allow-dirty-validation]
cargo run --quiet -p stickymd-smoke --locked -- release generate-sbom [--package-directory <directory>] [--zip <zip>] [--output <sbom>] [--syft-path <tool>]
cargo run --quiet -p stickymd-smoke --locked -- release workspace-version
cargo run --quiet -p stickymd-smoke --locked -- release verify-package --package-directory <directory> [--zip <zip>] [--checksums <manifest>] [--runtime]
cargo run --quiet -p stickymd-smoke --locked -- release verify-promoted --artifact-directory <directory> --source-sha <full-sha> --expected-zip-sha256 <sha256> --expected-sbom-sha256 <sha256> --release-tag <release-tag>
cargo run --quiet -p stickymd-smoke --locked -- release verify-workflow --source-sha <full-sha> --workflow-json <utf8-file-or-dash>
cargo run --quiet -p stickymd-smoke --locked -- release verify-remote-state --kind <tag|draft> --source-sha <full-sha> --release-tag <tag> --query-exit <code> --http-response <utf8-file-or-dash> [--allow-missing]
cargo run --quiet -p stickymd-smoke --locked -- release notices --destination <new-file>
cargo run --quiet -p stickymd-smoke --locked -- release checksums --zip <zip> [--sbom <sbom>] --output <manifest>
cargo run --quiet -p stickymd-smoke --locked -- release publish-sbom --input <staged-sbom> --output <sbom> --zip <zip> --checksums <manifest>
cargo run --quiet -p stickymd-smoke --locked -- release publish-package --input <completed-zip> --output <zip> --checksums <manifest>
cargo run --quiet -p stickymd-smoke --locked -- release prepare-sbom --package-directory <directory> --staging-directory <new-directory> [--zip <zip>] [--syft-path <provided-tool>]
cargo run --quiet -p stickymd-smoke --locked -- release syft-plan [--syft-path <provided-tool>]
cargo run --quiet -p stickymd-smoke --locked -- release syft-publish --kind <archive|checksums> --input <downloaded-file>
cargo run --quiet -p stickymd-smoke --locked -- release syft-verify --archive <archive> --checksums <upstream-manifest> --staging-directory <new-directory>
```

`package-inputs` also accepts `--version`, `--commit-sha`, `--release-tag` and
`--exact-candidate`, preserving `package.ps1`'s override and dirty-tree policy.
It returns one JSON plan marked `NOT_RUN`. Package selection and naming use the
same Rust implementation. `workspace-version` reads the scalar from
`[workspace.package]`, accepts assignment whitespace and comments, and refuses
missing or duplicate versions instead of selecting a dependency/metadata version.
Verification/notices retain the scripts' `KEY=value` stdout and 0/nonzero exit
semantics. The wrappers resolve relative paths using the caller's PowerShell
location, restore that location and console encoding even on failure, and use
locked Cargo invocations.

`build-package` and `generate-sbom` coordinate these same typed Rust rules directly.
The runner calls them in-process and preserves its task IDs/order and JSON stdout
boundary. The stable PowerShell scripts each make one locked Cargo invocation;
Cargo still checks source freshness. The two commands accept `--powershell <host>`
for their platform adapter; wrappers supply their own 5.1/7 host, while direct CLI
and runner calls default to `pwsh`. No application build occurs inside packaging.
ZIP compression keeps the original member order, timestamp and compression API in
`package-archive.ps1`. Rust owns private staging, publication and cleanup. Adapter
or validation failure returns nonzero without creating success qualification evidence.

`prepare-package` accepts the same identity overrides and flags as `package-inputs`.
It creates only a new staging directory (the parent must exist), writes complete
files atomically, and removes its owned staging tree on failure. Existing files or
directories are never overwritten or cleaned. Its JSON contains the unchanged
`inputs` plan, ordered `members`, and `diagnostics` for the wrapper to forward.
`inputs.status: NOT_RUN` still means package verification/qualification has not run.
`release/package_content.rs` owns the six members, staging content sources and the
four SBOM-required flags. ZIP validation and SBOM coverage consume that inventory;
PowerShell compresses the returned ordinal member list without enumerating or
selecting content. README titles come from the validated source state; the text
template remains UTF-8 without BOM with CRLF output. Packaged licenses reuse the
strict notices decoder, normalize line endings to LF and omit BOMs.

The release workflow uses `workspace-version` too. `verify-workflow` reads one
GitHub Actions run API observation from a UTF-8 JSON file, or stdin when
`--workflow-json -` is supplied. It requires a full matching `head_sha`,
`conclusion: "success"` and `name: "release"`, rejects missing/duplicate/mistyped
fields and emits only `WORKFLOW_IDENTITY=PASS` on success. `release/workflow.rs`
also supplies these predicates to qualification's remote-run recording path.
The CLI performs no network request and writes no receipt. The publish workflow
fetches the observation once, propagates GitHub/validation failures, and retains
its separate draft, tag and artifact checks. It must check out a Source Freeze
that includes this command; older released sources do not gain new tooling.

`verify-remote-state` consumes `gh api --include` observations. Tags use the REST
ref response; draft lookup uses GraphQL `repository.release(tagName:)`, because
drafts can have pending tags. The tool requires a full matching tag SHA, matching
ref/tag name, an unpublished draft, successful transport and no GraphQL errors.
The requested release tag must match the workspace version. It returns only
`{"exists":true}` for a matching object. With explicit `--allow-missing`, a tag
HTTP 404 with gh exit 1 or successful GraphQL release null returns `{"exists":false}`; missing
repository/data, 403, 5xx and transport failures are errors. This observation is
not authorization or a qualification result.
`github-observation.ps1` performs the read and preserves HTTP status separately
from native stderr. The workflow retains every GitHub write, permission and
tag/draft/publish boundary. Offline tests execute the actual step bodies with
simulated GitHub I/O and the compiled Rust validator, including refusal before
mutation. PowerShell 5.1/7 cases use an aliased Unicode/spaced caller directory and
compare its actual location before/after each step, while checking encoding separately.
They do not establish remote workflow acceptance or an atomic remote
transaction; state can still change after observation.

Phase package tasks and downloaded-artifact verification call the same typed
Rust verifier as `release verify-package`. They no longer launch the outer
PowerShell entry, Cargo and a second smoke process. ZIP/resource adapters and
locked dependency metadata remain in use. Diagnostics go through an explicit
writer: human CLI output is unchanged, while JSON phase runs keep them on stderr
and reserve stdout for their existing receipt format. Package verification alone
does not qualify a candidate or write a successful qualification receipt.

Rust-launched Windows release adapters start with the selected host's default
module search path. They do not inherit `PSModulePath` from a different PowerShell
edition through Cargo; this affects only the child environment. Wrapper tests
exercise a conflicting module path as well as preserving the caller's environment.

`integrity.rs` shares hash validation and strict two-member checksum manifests
between package checks, promotion input checks and existing candidate receipts.
Hash adapters parse the digest field, never hex-looking filename words; empty
files have their actual SHA-256 on Windows as well as Linux.
`release/identity.rs` binds the full source SHA and release tag/version. Duplicate
checksum members, unsafe names and ambiguous README source declarations fail
closed. The ZIP and SBOM checksum roles must have distinct artifact names;
one file cannot satisfy both roles. An explicit ZIP must be the file covered by the package directory's
manifest. Archive and SBOM checks operate on private snapshots of the supplied files.

`release/checksums.rs` uses the same manifest name/hash rules for generation.
`checksums` writes UTF-8 without BOM, lowercase SHA-256 and LF, then returns one
JSON object containing `zip_sha256` and optional `sbom_sha256`. Hashing a file
does not establish package or SBOM validity.

`publish-package` snapshots the completed ZIP and uses those same hash/path/
manifest rules. Existing identical bytes are accepted; different bytes are never
replaced. Atomic no-replace publication handles concurrent producers: an identical
winner is accepted, a different winner is refused. The input is preserved for the
caller's cleanup. UTF-8/LF checksums are written last and success retains the
checksum JSON shape. A manifest write failure leaves the completed ZIP and returns
failure, without a qualification receipt; retry is safe. ZIP and manifest are not
a multi-file transaction. `package.ps1` retains its success markers and exit behavior;
refusal text is emitted by Rust stderr through the existing release-tool wrapper.
Windows destination spelling is validated before normalization can erase an unsafe
trailing dot or space.

`release/sbom.rs` owns the existing SPDX 2.x version, nonempty package list and
four required packaged-file coverage checks. Both publication and package
verification use these checks; a correct checksum cannot make malformed SBOM
JSON valid. This is the repository's structural/coverage gate, not a complete
SPDX schema or license audit.

Syft writes into its private temporary directory, outside the scanned context.
`publish-sbom` snapshots and validates that output before replacing the final
SBOM and checksum manifest. Both destinations require existing parents and must
be distinct from each other and the inputs. Each replacement is atomic; the
manifest is written last. Generation/validation failures preserve both existing
outputs. This is not a multi-file transaction: if manifest replacement fails
after SBOM replacement, the command fails and verification rejects mismatched
bytes. No successful result or qualification receipt is written on that path.

`release/package_rules.rs` checks the shared inventory, Windows path safety,
30 MiB limit and version/icon facts. PE checks reuse `pe_dependencies`.
`release/package_runtime.rs` owns bounded bootstrap, same-directory secondary
exit and unchanged durable-file assertions, and independent ASCII/space/Chinese
directories. Children use the existing RAII owner; cleanup targets only the
processes and temporary directories created by this invocation. This check does
not send keyboard, clipboard, tray or mouse input. Run it serially on an interactive
Windows desktop.
Performance/resource preflight also recognizes leftover `stickymd-verify-*`
package-test children and blocks measurement without terminating those processes.

`release/notices/` invokes locked Windows-filtered Cargo metadata, follows normal
dependency edges through local packages, excludes build/dev-only edges, sorts
ordinally, and selects license files or the existing reviewed fallback list.
Missing graph/classification/license facts and unsupported runtime sources fail.
Output remains UTF-8 without BOM, with LF, and includes the Cargo.lock hash.
The destination parent must exist; an existing output is never overwritten.
Occupied destinations and missing parents are rejected before invoking Cargo metadata
or reading license files. This preflight does not reserve the path: final atomic
publication still rejects an output created concurrently during generation.
Publication uses a same-directory temporary file and a no-replace move on Windows
(an atomic hard link on Linux); unsupported filesystem operations return failure.

PowerShell keeps ZIP compression/extraction, native resource fact collection,
Syft download transport/invocation and existing UIA/COM adapters. `package.ps1`
delegates staging and README generation to Rust. No product dependency was added.
The general metadata JSON reader is confined to release tooling; qualification
receipt schemas and release permissions are unchanged.

`release/syft/` owns the unchanged Syft 1.50.0 archive/manifest pins, filenames,
download URLs and cache decisions. `syft-plan` is read-only JSON: a matching version
directory alone is insufficient; only matching file hashes are cache hits. Missing
or corrupt files become `downloads` entries, with the existing three-attempt limit.
`syft-publish` validates a private snapshot against the selected fixed pin and
atomically replaces only that role's cache file under `target/release-tools/syft/`.
Failed validation/publication preserves existing cache bytes. There is no CLI pin
override. `syft-verify` creates a new snapshot directory, validates both hashes and
the unique upstream archive checksum entry, and returns the verified archive path
for extraction. Later cache changes cannot change that private extraction input.
The upstream text-mode checksum format uses the existing integrity name/digest/
duplicate rules after separator normalization.

`-SyftPath` retains its existing caller-provided-tool bypass; `syft-plan --syft-path`
checks that file exists and returns `external: true` without accessing the cache.
It does not attest the provided executable's version or hash. The existing
`SYFT_VERSION` output names the configured pin, not verified external-tool identity.
`syft-fetch.ps1` handles one network transfer; `syft-execute.ps1` extracts archives
and invokes Syft with scoped environment settings. Rust owns the three-attempt
retry policy, 1/2-second backoffs, partial cleanup and verified cache publication.
Offline tests cover corrupted/interrupted downloads, early success, retry bounds,
cache preservation and cleanup without contacting GitHub.

`prepare-sbom` composes workspace version, existing package-path selection and the
Syft plan. On a pinned cache hit it also creates the existing verified private
snapshot. Its JSON contains `workspace_version`, `zip_path`, `syft` (the unchanged
plan) and `verified_archive` (path or null). No ZIP validation or qualification is
implied. Cache misses retain the download/publish/verify flow; external Syft keeps
its bypass and creates no pinned snapshot. The staging directory must be new when
a snapshot is created. `generate-sbom.ps1` now calls the composed `generate-sbom`
command once, reusing preparation and publication internally. The earlier two-call
step and its timing remain historical evidence in the
[follow-up report](../../docs/report/2026-09-30-release-cli-finalization.md).

These are distinct scopes: selecting a path, verifying a package, verifying
supplied promotion inputs, and qualifying a Promoted Candidate. None of the new
commands creates a candidate, updates a qualification ledger, authorizes a remote
action or advances manual acceptance. Tests run with `cargo test -p stickymd-smoke
--locked`; `release_wrappers` exercises PowerShell 5.1 and PowerShell 7 when available,
including actual packaged license bytes, failed Syft output, invalid SBOMs,
checksum contents and preservation of the caller's environment.
CLI, package-path and release-wrapper cases share the `headless` integration target,
so the default Rust test harness can overlap their independent work. They read the
same prebuilt CLI and repository; each owns distinct
temporary fixture/output directories, child environment and console state. The
PowerShell fixtures dispatch to the prebuilt CLI instead of invoking Cargo builds.
Git queries disable optional index writes; dependency metadata/cache access remains
with Cargo.
Cases within an edition retain their order because they share mutable fixture state.
`--test-threads=1` still serializes the integration cases when requested. This concurrency
does not apply to GUI, startup or resource qualification.

Focused integration checks use the module filter within that target:

```powershell
cargo test -p stickymd-smoke --locked --test headless cli_exit::
cargo test -p stickymd-smoke --locked --test headless package_path_wrapper::
cargo test -p stickymd-smoke --locked --test headless release_wrappers::
cargo test -p stickymd-smoke --locked --test headless release_workflow::
```

The workflow tests execute the actual promotion step with an offline GitHub
observation and the compiled CLI under PowerShell 5.1/7. They check a single query,
failure propagation and Unicode inputs, not remote authorization or publication.
Two ignored `release::package::diagnostics` tests can compare direct/wrapper
dispatch or exercise serial runtime verification. Set
`STICKYMD_PACKAGE_DIAGNOSTIC_DIRECTORY` to a freshly built isolated local package,
including its SBOM and manifest, and select one exact test with `--ignored
--test-threads=1 --nocapture`. The timing test alternates the two paths, excludes
one warmup pair and checks equivalent output. Run it without concurrent builds;
it measures local package verification, not product startup or remote CI latency.
The runtime test opens only its private package copies and checks cleanup; run it
on an interactive desktop with no other GUI verification campaign.

Automatic standalone integration-target discovery is disabled for this crate; register
new integration modules in `tests/headless.rs`. Existing module/phase/workspace commands,
unit tests and Cargo's documentation-test handling are unchanged.

## Acceptance status

Each measured resource group first runs a disposable desktop interaction probe.
It verifies real toolbar routing and persisted Source/Preview/Split transitions before
the long sampling windows. Probe processes never contribute resource samples.
Occlusion errors include the observed PID, executable basename and window class at
failure time, without window titles or full process paths. Other windows are not dismissed.
The probe cannot guarantee that the desktop remains undisturbed later in the run.

With `-Resources -Json -EvidenceFile <path>`, each completed matrix case saves an
`INCOMPLETE` group checkpoint to the evidence file. Window runs retain diagnostic
snapshots between complete repetitions; zoom cohorts checkpoint after five samples.
The adjacent `<stem>.progress.json` reports the group, case, repetition, phase start,
phase duration and remaining fixed-wait budget. It is always diagnostic progress,
not a success receipt. File updates happen outside sampling windows; the budget
excludes process startup, fixture preparation, probes and stress work. No continuous
background timer consumes CPU during the 60-second measurements.

For **local diagnostics only**, opt into complete-case resume:

```powershell
./tools/smoke/phase-14.ps1 -Resources -ResourceModule math -ResourceResume -EvidenceFile target/diagnostics/math.json
```

The Rust flag is `--resource-resume`; it requires Phase 14 Resources and an ignored
JSON output under `target/`. Canonical qualification paths and aliases reject it.
Only complete, successful five-sample Source/Preview, Math and Images cases, or whole
Window/Zoom groups, enter `target/resource-diagnostics/v2/`. Whole groups retain every
fixture, stress and hard-gate check and require successful cleanup. Partial or failed units
run again from the beginning; old diagnostic JSON and v1 caches
is not imported. No qualification ledger or readiness result consumes this cache.

Reuse binds all tracked and nonignored untracked input bytes, source commit, actual
Release EXE and harness hashes, host/boot/logon, single-monitor geometry/DPI/work area,
power state and execution settings. Unknown identities disable resume; changes detected
during execution abort the diagnostic. Every record expires after 24 hours and must
pass checksum, raw-observation, sampling-protocol, statistic and hard-gate validation.
Missing, corrupt, expired or incompatible records cause a fresh complete case.
Instantaneous system load is not guaranteed to match historical observations.

With the same identity, complete equivalent Source/Math/Images cases can be reused
across commands. The requested case is checked first, followed by registered cases
with identical inputs, view, operation history and sampling protocol. Each candidate
passes all checks independently. Reused observations retain the original unit and
timestamp; relabeling does not write another record or renew its lifetime. Window
and Zoom remain whole groups. Planning uses the same lookup rules as execution.

When all remaining cases in a group are cached, resume reads them as one batch after
the desktop probe, with full fresh identity checks before and after. Each record is
still validated independently. A missing or invalid case discards the entire batch
and falls back to per-case execution. No batch survives a fresh measurement. The
`cache_batch.execution_seconds` metric separates shared validation cost from case
processing; same-command aliases do not receive a second budget credit.

Reused cases carry `DIAGNOSTIC_REUSED` and `diagnostic-cache:` sample origins, with
`origin_execution_seconds` separate from this invocation's `execution_seconds`.
Progress removes only the fixed waits actually avoided. Identity checks and cache
writes happen between cases, outside measurement windows.

Resume prints a per-unit plan before the desktop probe, with `RUN`, `REUSE_IF_VALID`
or `SHARE_IN_COMMAND`, specific miss reasons, and expected fixed waits. It is advisory:
execution revalidates identity and records, and live progress credits only actual reuse.
The internal `latest/` pointer explains changes without authorizing another identity.
Private host/user/environment values are never printed.

To inspect without building or starting StickyMD, add `-ResourcePlan` (Rust:
`--resource-plan`) to the same resume command. It emits `DIAGNOSTIC_PLAN` / `NOT_RUN`
JSON on stdout and leaves `-EvidenceFile` untouched; that path still validates scope.
A missing program or unknown identity is shown as `RESUME_DISABLED`, never as a pass.
`RESOURCE_IDENTITY` reports Git, artifact hashing, environment and input hashing times
outside measurement windows.

To prioritize the last failed unit, add `-ResourceFailureFirst` (Rust:
`--resource-failure-first`) to a diagnostic resume command. Its group and case run
first after the usual prerequisites, within the selected module/case scope. The
selected unit runs fresh even if an older complete cache exists; Window/Zoom remain
whole groups. Sampling counts, durations and gates are unchanged. Combine with
`-ResourcePlan` to preview without starting the product or modifying evidence/hints.
Every diagnostic resume can record an advisory `last-failure.json` under the internal
cache directory. It contains a registered unit, timestamp and revision, with a
checksum and 24-hour lifetime. Missing, corrupt, expired or out-of-scope hints fall
back to the fixed order. Hints can survive source changes because they only guide
ordering; success reuse still requires the full current identity. Historical hits
do not clear hints. Fresh complete success clears only the still-matching revision;
this advisory comparison is not a cross-process scheduling lock. Formal evidence
paths and mixed qualification wrapper actions reject the option. If the hint expires
or becomes unreadable during measurement, clearing is skipped with a diagnostic;
the fresh result remains valid. Actual atomic clear-write failures still propagate.
The wrapper also rejects standalone `-ResourceResume` mixed with qualification actions.

An opt-in Windows test exercises alias and batch reuse with real observations:

```powershell
$env:STICKYMD_SMOKE_PROBE_REPOSITORY = 'E:\gitclone\StickyMD-resource-probe-0d89ca5'
cargo test -p stickymd-smoke --locked native_resource_alias_and_batch_reuse -- --ignored --nocapture --test-threads=1
```

Use a dedicated checkout with a matching Release executable and no existing
`preview-no-images` cache for its current identity. Leave `STICKYMD_SMOKE_RESOURCE_CASE`
unset. The test takes the desktop for about five minutes: two registered cases each
retain all five 30-second warmups. It saves real complete records, reopens the Store,
compares individual and batch reads, and checks alias observations and unchanged
origin bytes. It writes `target/acceptance-profiling/resource-native-reuse-review.json`
under the probe checkout. Do not edit probe inputs or rebuild during the run. This
tests native observations and Store reuse; it does not run the full resource matrix
or establish formal qualification.

The persistent result for each phase lives in
`docs/acceptance-cases/phase-XX.md`. Automated checks may be marked
`AUTOMATED PASS` only when their checked-in runner passes. Manual checks stay
`NOT TESTED` until a durable receipt is checked in; terminal output from a
one-off run is not such a receipt.
