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
| P00-A10 | plan 11 portable verification tooling and unsupported GUI evidence boundary | Automated | strict smoke CLI Clippy on Windows/Linux; Linux planner job runs lint/tests for full or smoke scope; [`compiled CLI regression`](../../tools/stickymd-smoke/tests/cli_exit.rs) checks unsupported GUI requests | AUTOMATED PASS |
| P00-M01 | USER constitution semantic fidelity review | Manual | Current-commit section-by-section review receipt required | NOT TESTED |
| P00-M02 | architecture contract judgment review | Manual | Current-commit architecture checklist receipt required | NOT TESTED |

The automated rows are re-evaluated by the checked-in runner; manual judgment remains separate.

P00-A08: Given clean Git base/HEAD and changed inputs, plan without executing checks. Expect
the affected modules and reverse dependencies; shared/unknown inputs, missing baseline,
dirty worktree or Cargo registry drift must select the original full entry. Rename/delete
cases retain the old owner. An omitted affected module or a plan claiming PASS is a failure.

P00-A09: Given a declared CI scope and completed job statuses, aggregate the result. Expect
success only when requested jobs succeeded and intentional skips match the plan. Failure,
cancellation, missing/unknown status or unexpected skip must return nonzero. Manual, scheduled
and release lanes must retain full checks. These local adapter checks do not prove remote execution.
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
