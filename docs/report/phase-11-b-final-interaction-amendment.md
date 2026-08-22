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
| 1 MiB / 1000 semantic math nodes | 19.745 ms | 21.904 ms | 22.108 ms | PASS <50 ms |
| equal-geometry zoom relayout 50% | 1.358 ms | 1.596 ms | 1.691 ms | PASS <=50 ms |
| Preview 1 MiB | 234.258 ms | 253.253 ms | 266.010 ms | PASS <=2 s |
| math document 1 MiB / 500 formulas | 237.516 ms | 242.320 ms | 246.823 ms | PASS <=2 s |

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
- final CI-safe evidence: `docs/report/evidence/phase-11-b-ci-final.json`;
- five manual rows remain `NOT TESTED`.

## Artifact and Readiness

The pre-amendment artifact is superseded. A new exact candidate package/hash/SBOM receipt is required after
the implementation commit. Phase 11 startup and manual gates remain independent blockers, so this amendment
does not make the product RC ready.

## Architecture Drift

None identified. No push, tag or release is authorized.
