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

## Acceptance status

The persistent result for each phase lives in
`docs/acceptance-cases/phase-XX.md`. Automated checks may be marked
`AUTOMATED PASS` only when their checked-in runner passes. Manual checks stay
`NOT TESTED` until a durable receipt is checked in; terminal output from a
one-off run is not such a receipt.
