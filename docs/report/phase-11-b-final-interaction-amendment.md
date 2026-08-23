# Phase 11-B Final Interaction Amendment Report

## Executive Result

The semantic delimiter conversion and Pin/auto-hide orthogonality contracts are implemented without a
new parser, dependency, authority, thread or persistence path. Automated headless and copied-Release
runtime checks pass. The five real interaction rows remain `NOT TESTED`.

## Math Delimiter Conversion

| Contract | Result |
| --- | --- |
| inline `\\(...\\)` to `$...$` | PASS |
| display `\\[...\\]` to `$$...$$` | PASS |
| Source/Split non-empty selection | only fully-contained semantic math nodes |
| Preview or empty selection | whole current snapshot |
| code/literal/dollar/malformed safety | unchanged by owned-AST source ranges |
| body preservation | exact original body bytes |
| transaction | one `EditRequest`, one generation, one Undo/Redo entry |
| no matches | no generation/dirty/history change |

Detection is a fresh Comrak parse of the current `DocumentSnapshot`; stale Preview AST is never reused.
The coordinator alone applies the batch through `DocumentState`, after which the ordinary document-changed
effects schedule autosave and Preview refresh.

## Toolbar

The compact toolbar now has nine controls. The math-conversion control is a short vector glyph, preserves
the Close control and view-mode hit targets, and passes the 220 DIP geometry regression. It does not add a
menu or direct mutation path.

## Pin / Auto-hide Orthogonality

Pin controls configured/effective topmost only. The reducer contains no configured/effective topmost input.
The Pin ON/OFF transition-equivalence regression covers focus/IME/drag/popup guards, 700 ms focus-loss
collapse, manual and Esc collapse, 100 ms sensor reveal, 500 ms hover-leave collapse, floating exclusion
and temporary sensor topmost.

## Performance

| Workload | median | p95 | max | Gate |
| --- | ---: | ---: | ---: | --- |
| 1 MiB / 1000 semantic math nodes | 25.218 ms | 29.030 ms | 31.758 ms | PASS <50 ms |
| equal-geometry zoom relayout 50% | 1.756 ms | 2.479 ms | 2.851 ms | PASS <=50 ms |
| Preview 1 MiB | 153.957 ms | 208.042 ms | 332.852 ms | PASS <=2 s |
| math document 1 MiB / 500 formulas | 146.531 ms | 190.651 ms | 195.145 ms | PASS <=2 s |

The first broad short-run aggregation regressed 50% zoom p95 to 56–62 ms and was rejected. The retained
layout policy keeps typical-note attributed runs fine-grained, batches large-document mixed runs by the
available line width, and uses a document-scoped second-occurrence shaping cache capped at 1024 entries
and 1024 text bytes per key. Repeated equal geometry now repaints without relayout; changing image-source
availability still forces a correct rebuild. No cache survives the current layout or becomes an authority.

## Authority and Boundaries

- delimiter detection authority: Comrak owned semantic projection;
- mutation authority: `DocumentState`;
- toolbar: typed intent only;
- topmost authority: config + `WindowShellState` projection;
- auto-hide dependency on topmost: none;
- new runtime dependencies: 0;
- core/render unsafe: 0.

## Automation

- `tools/smoke/phase-11-b.ps1` is the stable thin PowerShell entry;
- Rust CLI owns Phase 11-B headless, performance and copied-Release runtime scenarios;
- `docs/acceptance-cases/phase-11-b.md` traces P11B-A01..A06, P11B-D001..D046 and
  P11B-M01..M05;
- final CI-safe evidence: `docs/report/evidence/phase-11-all-ci-final.json`;
- copied-Release interaction evidence: `docs/report/evidence/phase-11-b-runtime-final.json`;
- five manual rows remain `NOT TESTED`.

## Artifact and Readiness

The pre-amendment artifact is superseded. Candidate `23d2a410a256` has an exact package/hash/SBOM receipt
in `phase-11-release-final.json`; package SHA-256 is `e5110550...60899`. Phase 11 warm-startup and manual
gates remain independent blockers, so this amendment does not make the product RC ready.

## Architecture Drift

None identified. No push, tag or release is authorized.
