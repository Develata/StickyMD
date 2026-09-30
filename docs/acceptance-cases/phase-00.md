# Phase 00 Acceptance Matrix

> Verification projection for repository governance. Product behavior remains defined by
> [`00_v1_acceptance.md`](00_v1_acceptance.md); status rules come from
> [`plan 11`](../plan/11_testing_and_release.md#phase-verification-harness).

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P00-A01 | plan authority tree and required governance files | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): required-file check | AUTOMATED PASS |
| P00-A02 | AC-001..AC-030 stable verification case structure | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): AC sequence validator | AUTOMATED PASS |
| P00-A03 | plan_ref targets and stable anchors | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): production plan-ref validator | AUTOMATED PASS |
| P00-A04 | local Markdown links and forbidden root dependencies | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): governance validators | AUTOMATED PASS |
| P00-A05 | one smoke entry and one matrix for every retained Phase | Automated | [`phase-00.ps1`](../../tools/smoke/phase-00.ps1): phase-artifact validator | AUTOMATED PASS |
| P00-A06 | explicit module selection from the complete headless task graph | Automated | `cargo test -p stickymd-smoke --locked`: [`module plan tests`](../../tools/stickymd-smoke/src/runner/headless/tests.rs), full-plan equality, disjoint coverage, shared-command deduplication, unchanged Release arguments, unknown-target/workspace-drift rejection | AUTOMATED PASS |
| P00-A07 | Rust package selection with a thin Windows adapter | Automated | `cargo test -p stickymd-smoke --locked`: [`package selection tests`](../../tools/stickymd-smoke/src/package_path/tests.rs), [`PowerShell compatibility regression`](../../tools/stickymd-smoke/tests/package_path_wrapper.rs), ambiguity/missing inputs, Unicode paths and relative paths from the caller's actual location (including 8.3 TEMP aliases), with state restoration | AUTOMATED PASS |
| P00-A08 | plan 11 modular headless CI selection and conservative fallback | Automated | `cargo test -p stickymd-smoke --locked ci::`: [`Git input tests`](../../tools/stickymd-smoke/src/ci/git/tests.rs), [`classification tests`](../../tools/stickymd-smoke/src/ci/selection/tests.rs), Cargo registry verification | AUTOMATED PASS |
| P00-A09 | plan 11 job aggregation and full manual/scheduled/release boundaries | Automated | [`workflow adapter tests`](../../tools/stickymd-smoke/src/ci/workflow_tests.rs), [`result aggregation`](../../tools/stickymd-smoke/src/ci/results.rs), actionlint for CI/scheduled workflows | AUTOMATED PASS |
| P00-A10 | plan 11 portable verification tooling and unsupported GUI evidence boundary | Automated | strict smoke CLI Clippy on Windows/Linux; independent Linux smoke job runs lint/tests for full or smoke scope; [`compiled CLI regression`](../../tools/stickymd-smoke/tests/cli_exit.rs) checks unsupported GUI requests | AUTOMATED PASS |
| P00-A11 | plan 11 shared phase entry routing, parameter availability and caller-state preservation | Automated | `phase_entry` rules + actual 00–14/11-b/all wrappers on PowerShell 5.1/7; old/new same-input mapping comparison | AUTOMATED PASS |
| P00-A12 | plan 11 local affected-module checks using the existing CI classification and reverse dependencies | Automated | `development` Git/selection tests, shared `runner/headless/local` task plan and compiled `dev-check --plan` | AUTOMATED PASS |
| P00-A13 | plan 11 read-only timing observations with current/history/budget scopes preserved | Automated | `timing_summary` receipt/log fixtures and compiled Unicode-path/outside-repository/error tests | AUTOMATED PASS |
| P00-M01 | USER constitution semantic fidelity review | Manual | Current-commit section-by-section review receipt required | NOT TESTED |
| P00-M02 | architecture contract judgment review | Manual | Current-commit architecture checklist receipt required | NOT TESTED |

The automated rows are re-evaluated by the checked-in runner; manual judgment remains separate.

P00-A12: Given staged, unstaged, new, deleted or renamed local inputs, inspect the affected
headless checks before execution. Expect the same path classification and reverse dependency
closure as CI, both owners of a move, per-check reasons, and `NOT_RUN` for the plan. Unknown
inputs, conflicts, invalid Git transport or Cargo registry drift must select conservative full
checks in the requested mode. Execution uses the existing task graph with locked Cargo;
local checks do not write qualification ledgers. Missing checks, a plan claiming PASS,
loss of either rename owner, or changed commit-only CI behavior are failures.

P00-A13: Given existing task/resource receipts or timing logs, inspect elapsed observations.
Expect current task times, historical origins, nested measurements and fixed-wait budgets
to remain distinct. Missing timings stay missing; overlapping scopes and multiple files
are not added into a fabricated wall clock or agent-work total. Malformed/negative/nonfinite
durations, ambiguous duplicates and invalid later inputs must fail without partial stdout.
Unicode/spaced paths work outside the repository; input bytes and acceptance state stay intact.

P00-A11: Given each retained PowerShell entry and supported parameters, request a
read-only route through the compiled CLI. Expect the original canonical arguments,
`NOT_RUN`, unchanged caller CWD/encoding, and distinct false/absent values. Unsupported
entry parameters, invalid shards/resource modules and conflicting modes must fail
without dispatch or receipt writes. Source-keyword presence is not evidence of routing.

P00-A08: Given clean Git base/HEAD and changed inputs, plan without executing checks. Expect
the affected modules and reverse dependencies; shared/unknown inputs, missing baseline,
dirty worktree or Cargo registry drift must select the original full entry. Rename/delete
cases retain the old owner. An omitted affected module or a plan claiming PASS is a failure.

P00-A09: Given a declared CI scope and completed job statuses, aggregate the result. Expect
success only when requested jobs succeeded and intentional skips match the plan. Failure,
cancellation, missing/unknown status or unexpected skip must return nonzero. Manual, scheduled
and release lanes must retain full checks. These local adapter checks do not prove remote execution.
Linux smoke lint/tests must run as a separate job after planning so other lanes can start
concurrently. Rust selects it for full/smoke scope; its failure, cancellation, missing result
or unexpected skip must fail the aggregate gate, while other scopes must omit it as planned.
The workflow retains the Rust plan and per-module stdout/stderr as separate CI artifacts;
logging must preserve a failing native exit code. Successful captured performance tasks keep their
measurements on stderr while CLI stdout remains JSON. Module performance plans include the Source
scrollbar baseline, keep measurements serial within a runner, and do not link integration binaries for
library-only baselines. The module union must remain equal to the complete deduplicated task graph.

P00-A10: Given a Linux host, build/lint the smoke CLI with locked dependencies and
`--all-targets -- -D warnings`, then request GUI qualification environment inspection.
Expect a nonzero exit code with `UNSUPPORTED` / `NOT_TESTED` evidence, never `PASSED`.
Windows-only execution paths must not disable portable CLI parsing or pure rule tests.
Any lint failure, successful GUI qualification on Linux, or success evidence is a failure.

P00-A07/A10 integration concurrency: Given the existing CLI and Windows-wrapper cases,
run the Cargo `headless` integration target. Expect the same case inventory with independent
temporary outputs, child environments and console state; a case failure must fail the target.
The Windows cases remain platform-gated, and the Linux unsupported-GUI case stays included.
Missing or duplicated cases, shared mutable fixtures, or starting concurrent Cargo builds
are failure signals. Module and phase commands keep their existing coverage.
