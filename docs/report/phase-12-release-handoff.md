# Phase 12 Release Handoff

## Current Stop Point

Phase 12 stays active. The allowed local preparation/qualification lane does not authorize push, tag,
draft release, or publish.

## Exact Evidence Locations

| Evidence | Path | Expected producer |
| --- | --- | --- |
| Candidate identity | `dist/evidence/release-candidate.json` | `stickymd-smoke qualification candidate` |
| Automated gates | `dist/evidence/automated-qualification.json` | Phase 12 `-Release` wrapper / Rust runner |
| USER decisions | `dist/evidence/release-decisions.json` | candidate projection; `qualification decision` only after explicit USER instruction |
| Manual matrix | `dist/evidence/manual-acceptance.json` | interactive `stickymd-smoke acceptance manual` |
| Remote workflow | `dist/evidence/remote-workflow.json` | `qualification remote` after push authorization |
| Downloaded artifact | `dist/evidence/downloaded-artifact-smoke.json` | `qualification downloaded` after remote artifact download |
| Readiness | `dist/evidence/release-readiness.json` | `qualification readiness --explain` |

## USER Gates Still Required

1. Approve release version or reject it.
2. Execute manual matrix, or approve only explicitly enumerated waivers.
3. Approve/reject unsigned v0.1.0 policy.
4. Only after local readiness: authorize push exact SHA.
5. Separately authorize tag, draft release, and publish at their respective stop points.

## Invalidation Rule

Any committed change to source, workspace manifests/lock, runtime assets, or release tooling invalidates
candidate, manual, remote, downloaded-artifact and readiness receipts. Rebuild from the new exact SHA.
