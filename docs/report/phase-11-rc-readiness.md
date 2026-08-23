# Phase 11 RC Readiness

## Decision

**NOT RC READY.**

## Passing Evidence

- Phase 1–11-B CI-safe matrix: automated regression and Release performance tasks pass.
- Known P0 correctness/security defects after the reported caret/image fixes: 0.
- Core/render unsafe: 0; no WebView, Tokio, database or runtime network dependency.
- Preview, math, delimiter conversion and zoom Release gates pass.
- Phase 11 / 11-B smoke scripts and acceptance matrices are checked in; manual rows remain explicit.
- Complete resource matrix passes; observed idle CPU max is 0.005% and the highest stressed private
  working set is 31.77 MiB.
- Exact local package, checksum, SPDX SBOM and runtime verification pass for candidate `23d2a410a256`.

## Blocking Evidence

1. Warm startup p95 remains above the authoritative 180 ms gate. The Agent recommends reassessment but
   has no USER approval to change it.
2. Microsoft Pinyin, WeChat Input Method, Tool Window, tray, docking, physical multi-display/DPI,
   visual rendering, native clipboard/export and real crash-recovery acceptance remain `NOT TESTED` on
   the current candidate.
3. No current-candidate human receipt closes the release-critical manual matrix.

## Release Actions

- push: not authorized;
- tag: not authorized;
- GitHub Release: not authorized;
- previous local artifacts: superseded by
  `StickyMD-0.1.0-local-rc-23d2a410a256-windows-x64-portable.zip`.

The first exact packaging attempt exposed a non-idempotent local automation defect: an older ZIP made
SBOM selection ambiguous. The retained fix selects the package matching the current version/commit and
reuses an existing same-candidate ZIP only when a newly generated temporary ZIP has the identical SHA-256;
different bytes still fail closed. Two consecutive validation packages produced the same hash, and both
final Release and Package smoke modes pass with multiple superseded artifacts still present.

## Candidate Identity

- source candidate: `23d2a410a2564554e47b7ea056e77b9a68f35010`;
- EXE SHA-256: `6dbc31fb34a21b687316dbcb40719c123623598f8f7d10477af30b2bb41f7c5d`;
- ZIP SHA-256: `e51105507bd5fc4db8ba212dbec8ad5ba9c78d4ae7bdc2c205e10c1fe0960899`;
- SBOM SHA-256: `06adcbfc2abb943049344225b651f789d46dcf5ed8abea7237cc090592971c3d`.

## Recommendation

`STOP — release gates remain open.` Implementation may be committed locally, but RC tagging/release must
wait for USER startup-gate disposition and current-candidate manual receipts.
