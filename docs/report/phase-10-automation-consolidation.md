# Phase 10 Automation Consolidation

## Result

**PASS for architecture and headless coverage.** `tools/stickymd-smoke` is the single checked-in
authority for phase task selection, de-duplication, local gate evaluation, copied-Release process
control and evidence projection. PowerShell remains a thin Windows invocation boundary. Manual
acceptance remains outside automated PASS logic.

## Before and After

| Concern | Before Phase 10 | Phase 10 result |
| --- | --- | --- |
| Phase selection | Rust CLI through Phase 09 | Rust CLI through Phase 10 |
| task de-duplication | Rust task graph | retained; Phase 10 regression tests prove each task appears once |
| machine evidence | human output only | schema-v1 JSON with commit, dirty-tree flag, verified artifact hash, result status, detail and structured measurements |
| exit status | process failure propagated | explicit `0 = passed`, nonzero = failed/blocked/NOT_TESTED |
| startup statistics | Rust calculation, terminal text | Rust calculation plus JSON cold/warm count and p50/p95/max |
| resource plan | phase-specific subsets | Phase 10 explicitly composes base, math, image, window and zoom matrices once each |
| CI | direct Cargo steps and smoke | direct fmt/clippy/build plus one `all --ci --json` headless graph |
| PowerShell | phase launchers | argument forwarding and exit-code propagation only |

## Rust-Owned Logic

- CLI parsing and typed Phase 00..10 selection.
- task planning, ordering and de-duplication;
- governance/acceptance contract checks;
- Release benchmark invocation;
- copied-Release runtime, resource and startup process control;
- unique ready-event ownership, graceful process exit and nearest-rank statistics;
- performance/resource gate evaluation;
- package/SBOM/verification orchestration;
- schema-v1 JSON escaping, status, measurements and verified package hash projection.

The JSON schema intentionally stays small. The envelope reports `commit` and `worktree_dirty`, so
an uncommitted run cannot masquerade as exact-commit evidence. Each result contains an ID, status,
optional detail and a list of `{name, unit, value}` measurements. Artifact SHA-256 is emitted only
after the package verification task has passed; it is never inferred from an unverified file.

## PowerShell-Owned Logic

[`tools/smoke/phase-10.ps1`](../../tools/smoke/phase-10.ps1) and
[`tools/smoke/all.ps1`](../../tools/smoke/all.ps1) resolve the repository, translate switches into
the Rust CLI command and propagate its exit code. They contain no product gate values, percentile
algorithm, acceptance status mutation or package allowlist.

The existing package scripts remain responsible for Windows shell-level archive production and
SBOM command invocation. Their results are planned and judged by the Rust automation authority;
Phase 10 did not duplicate mature archive construction code merely to rename the implementation.

## CI

The Windows workflow calls:

```text
cargo run -p stickymd-smoke --locked -- all --ci --json
```

The CI graph includes all deterministic headless tests and Release benchmarks exactly once. GUI,
machine-resource, startup, IME, visual, tray and physical-display work is deliberately excluded
from hosted CI. `all --ci` judges only that requested headless graph; it still validates acceptance
syntax but does not reinterpret an unrequested runtime/manual row as a CI failure. Local Phase
readiness, runtime and package modes retain the full automated-row readiness gate. Missing runtime
capability is `NOT_TESTED`, never PASS.

## Verification

- 30 CLI unit tests and two process exit/JSON integration tests pass.
- strict `clippy -D warnings` passes for the tools crate.
- task-graph tests cover CI, Phase 10 performance/runtime/resources and package modes.
- JSON schema tests cover escaping, measurements, NOT_TESTED and verified artifact hash admission.

## Dependency Delta

No production dependency was added. The automation crate remains std-only.
