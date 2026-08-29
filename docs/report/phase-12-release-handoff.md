# Phase 12 Release Handoff

## Current Stop Point

Phase 12 stays active. The allowed local preparation/qualification lane does not authorize push, tag,
draft release, or publish.

Current blocker is the absence of a workflow-built Promoted Candidate for the new Option B Source Freeze.
Old local-candidate runtime/performance/resources/G3/G4/G5/manual receipts belong to invalidated bytes and
must not be promoted or copied. Source-only evidence may be reused only if its Source Freeze identity remains exact.

## Exact Evidence Locations

| Evidence | Path | Expected producer |
| --- | --- | --- |
| Source Freeze | `dist/evidence/release-source-freeze.json` | `stickymd-smoke qualification source-freeze` |
| Candidate identity | `dist/evidence/release-candidate.json` | `qualification downloaded` promotion after successful workflow artifact download |
| Local Release/package preflight | `dist/evidence/automated-qualification.json` | Phase 12 `-Release` wrapper / Rust runner；不计作 final artifact evidence |
| Headless CI gates | `dist/evidence/headless-ci-qualification.json` | `stickymd-smoke all --ci --json` with evidence output |
| Performance gates | `dist/evidence/performance-qualification.json` | Phase 12 `-Performance` wrapper |
| Runtime gates | `dist/evidence/runtime-qualification.json` | Phase 12 `-Runtime` wrapper |
| Resource gates | `dist/evidence/resources-qualification.json` | Phase 12 `-Resources` wrapper |
| USER decisions | `dist/evidence/release-decisions.json` | Source Freeze projection; `qualification decision` only after explicit USER instruction |
| Manual matrix | `dist/evidence/manual-acceptance.json` | interactive `stickymd-smoke acceptance manual` |
| Remote workflow | `dist/evidence/remote-workflow.json` | `qualification remote` after push authorization |
| Downloaded artifact | `dist/evidence/downloaded-artifact-smoke.json` | `qualification downloaded` after remote artifact download |
| Readiness | `dist/evidence/release-readiness.json` | `qualification readiness --explain` |

## USER Gates Still Required

1. Release version `0.1.0` and unsigned portable distribution are already USER approved.
2. Authorize push of the new exact Source Freeze SHA; this does not authorize tag or release actions.
3. Promote the successful workflow artifact and execute only the artifact-bound minimal exact requalification.
4. Execute remaining manual matrix items, or approve only explicitly enumerated waivers.
5. After readiness is `READY`, separately authorize tag, draft release, and publish at their respective stop points.

## Invalidation Rule

Any committed change to source, workspace manifests/lock, runtime assets, or release tooling invalidates
Source Freeze and all downstream receipts. Promoting a different workflow artifact invalidates every
artifact-bound receipt, while source-only CI/headless evidence may be reused only when Source Freeze remains exact.
