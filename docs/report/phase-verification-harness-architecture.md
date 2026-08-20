# Phase Verification Harness Architecture Decision

- `Date`: 2026-08-20
- `Decision`: USER approved
- `Authority after adoption`: `docs/plan/11_testing_and_release.md#phase-verification-harness`

## Current problem

Phase 0–4 accumulated substantial unit/integration coverage and a global AC map, but did not use one
uniform, checked-in verification entry per Phase. Phase 4 portable EXE evidence was produced by a
one-off command rather than a repository runner; Phase 1 experiment crates and Release-only
benchmarks were also outside CI. This made reproduction depend on narrative reports.

## Approved skeleton

```text
tools/smoke/phase-XX.ps1        stable human/CI entrypoint (thin)
        ↓
tools/stickymd-smoke            std-only Rust task planner/executor
        ↓
deduplicated cargo/internal/runtime tasks
        ↓
typed exit status + console receipt

docs/acceptance-cases/phase-XX.md
        ↑
plan + global AC verification projection
```

The verification CLI may read repository contracts and start test/runtime processes. It cannot
become a product runtime dependency, mutate product authority, or enter the portable release.

## Benefits

- Phase evidence is discoverable and repeatable from a stable path.
- Rust owns path/process/status logic; PowerShell remains portable Windows glue.
- `all --ci` deduplicates shared workspace tests, executes every headless Release performance
  entry and never starts native windows. Machine-specific measurements remain diagnostic unless a
  stable threshold is explicitly encoded.
- Manual gates remain visible and cannot be upgraded by a one-off automation receipt.

## Costs and risks

- The tooling crate adds a small workspace build target and its own tests.
- A Rust orchestrator cannot materially reduce the compilation work performed by Cargo; its speed
  benefit comes from task deduplication and lower scripting overhead.
- Runtime GUI smoke remains opt-in because it can create windows and is unsuitable for headless CI.
- Matrix claims can still become stale after later edits; CI and Phase completion review must execute
  the current runner rather than trusting prose.

## Compatibility, rollback and release impact

No product API, durable data, runtime authority, package format or user file changes. Rollback is
limited to removing the tooling member, wrappers, matrices and CI invocation. The production crates
remain independently buildable. `stickymd-smoke` is not packaged.

## Verification

- CLI unit tests for argument parsing, task deduplication and matrix validation.
- Phase 0 governance smoke validates required files, AC numbering, phase matrix status vocabulary,
  PowerShell routing, plan anchors, local links and forbidden root dependencies.
- Windows CI invokes the merged headless task graph.
- Local opt-in runtime smoke uses copied Release executables in a unique temporary directory and
  cleans only that directory.

## Resolution

USER explicitly required one `phase-XX.ps1` and one `phase-XX.md` per Phase, CI execution of all
headless work, strict `NOT TESTED` treatment for unfinished manual gates, and Rust-based automation.
The governing contract was updated accordingly.
