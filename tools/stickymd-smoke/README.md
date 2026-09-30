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

For a local diagnosis of one opt-in resource case, set
`STICKYMD_SMOKE_RESOURCE_CASE` to the exact case label before invoking the owning phase script.
This development filter is never set by CI or by the durable full-matrix receipts.

`-Ci` runs every headless check, including the Release performance entry
points. Stable hard thresholds may fail CI; machine-specific measurements are
diagnostic only. `-Performance` reruns the same measurements explicitly on a
local machine. `-Runtime` creates native windows and remains local-only. The
CLI rejects combining either explicit local mode with `-Ci`.

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

Equivalent scenarios share a complete cohort within one command: the 19 Source/Preview,
math and image names require 15 executions. The fixed waiting budget decreases by
1,500 seconds (25 minutes); this is a calculated budget, not a measured speedup.
Sampling counts, warmups, CPU intervals and thresholds are unchanged. Operation history
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

`--plan` prints one JSON document marked `NOT_RUN` and does not execute tests.
Execution stops on failure and returns a nonzero exit code. A module run reports
only its requested scope and writes no qualification or last-success receipt.
The existing `all --ci` entry remains the complete headless check. Automatic CI
selection is a separate planner, described below.

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
workflows preserve their existing artifact gates. Shared Cargo caches retain
downloads/build outputs, never candidate ledgers or note data. Daily CI superseded
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
./tools/release/verify-promoted-artifact.ps1 -ArtifactDirectory <directory> -SourceSha <full-sha> -ExpectedZipSha256 <sha256> -ExpectedSbomSha256 <sha256> -ReleaseTag v0.1.0
./tools/release/generate-third-party-notices.ps1 -DestinationPath <new-file>
./tools/release/generate-sbom.ps1 -PackageDirectory <directory> [-ZipPath <zip>] [-OutputPath <sbom>] [-SyftPath <syft>]
```

Their reusable commands are in the existing std-only CLI:

```powershell
cargo run --quiet -p stickymd-smoke --locked -- release package-inputs --allow-dirty-validation
cargo run --quiet -p stickymd-smoke --locked -- release prepare-package --exe <exe> --staging-directory <new-directory> [--allow-dirty-validation]
cargo run --quiet -p stickymd-smoke --locked -- release workspace-version
cargo run --quiet -p stickymd-smoke --locked -- release verify-package --package-directory <directory> [--zip <zip>] [--checksums <manifest>] [--runtime]
cargo run --quiet -p stickymd-smoke --locked -- release verify-promoted --artifact-directory <directory> --source-sha <full-sha> --expected-zip-sha256 <sha256> --expected-sbom-sha256 <sha256> --release-tag v0.1.0
cargo run --quiet -p stickymd-smoke --locked -- release verify-workflow --source-sha <full-sha> --workflow-json <utf8-file-or-dash>
cargo run --quiet -p stickymd-smoke --locked -- release notices --destination <new-file>
cargo run --quiet -p stickymd-smoke --locked -- release checksums --zip <zip> [--sbom <sbom>] --output <manifest>
cargo run --quiet -p stickymd-smoke --locked -- release publish-sbom --input <staged-sbom> --output <sbom> --zip <zip> --checksums <manifest>
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
`syft-download.ps1` handles network I/O, retry waits and partial-file cleanup;
it delegates all checksum/cache publication decisions to Rust. Offline dual-host
tests cover corrupted downloads, interrupted transport, retry bounds, cache
preservation and cleanup without contacting GitHub.

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
