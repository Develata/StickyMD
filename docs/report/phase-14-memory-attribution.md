# Phase 14 Release Memory Attribution

## Status

Engineering attribution and bounded optimization complete. Final exact-candidate source-preview
qualification remains a distinct release gate.

## Scope and method

- Copied standalone `target/release/stickymd-win.exe` only; Debug builds are not evidence.
- Windows Private Working Set and Private Bytes, five independent processes per case.
- Each process reaches the expected projection-ready fact, parks the physical cursor outside the
  paper, then remains idle for 30 seconds before sampling.
- Source, Preview and Split are separate processes. `source-after-preview` first builds Preview,
  switches to Source, waits another five seconds, then samples projection-release residency.
- Targeted JSON receipts are written below `target/` and are development evidence, not frozen
  exact-candidate evidence.

## Steady-view baseline

| View | Private Working Set median | max | Private Bytes median | Idle CPU p95 |
| --- | ---: | ---: | ---: | ---: |
| Source | 12.98 MiB | 13.03 MiB | 15.01 MiB | 0.0027% |
| Preview | 15.50 MiB | 15.56 MiB | 16.76 MiB | 0% |
| Split | 20.89 MiB | 23.58 MiB | 23.47 MiB | 0% |

These Release measurements do not reproduce the approximately 55 MiB Task Manager observation.
That observation was not bound to an executable hash or memory counter and is consistent with a
Debug process or a different Working Set column; it is not used as a Release baseline.

## Finding and bounded optimization decision

Source after a previously built Preview retained the semantic `RenderTree` while releasing laid-out
rasters. A trial implementation also dropped the tree on every switch to Source. It reduced the
fixed fixture by less than 0.6 MiB, but forced a full parse/layout and a skeleton interval whenever
the user immediately returned to Preview. That trade was rejected: it spends visible latency to
solve a Release memory problem that the measurements did not reproduce.

The accepted lifecycle keeps two ordered worker-owned release levels:

1. switching to Source clears pending work and releases laid-out rasters/decoded images while
   retaining the semantic tree for fast Preview return;
2. the existing tray-hidden cache boundary drops `RenderTree`, layout, decoded images and math
   rasters while retaining the reusable font database and bounded math layout cache;
3. document release dominates a queued raster-only release;
4. a completion arriving after tray hide is rejected and followed by an idempotent document
   release;
5. reopening after tray release requires a generation-tagged full Build.

No shared mutable font system, allocator tuning, unsafe code, new dependency or cache-budget
relaxation was introduced.

| Rejected always-drop-on-Source experiment | Before | Trial | Delta |
| --- | ---: | ---: | ---: |
| Private Working Set median | 16.17 MiB | 15.78 MiB | -0.39 MiB |
| Private Working Set max | 16.32 MiB | 15.92 MiB | -0.40 MiB |
| Private Bytes median | 18.97 MiB | 18.43 MiB | -0.54 MiB |
| Private Bytes max | 19.79 MiB | 18.69 MiB | -1.10 MiB |
| Idle CPU p95 | 0.02% | 0% | -0.02 percentage points |

These numbers explain why the trial was not retained; they are not measurements of the final
lifecycle. The final optimization targets tray-hidden residency, where rebuilding later has no
immediate visible interaction cost. More aggressive sharing or allocator tuning would couple UI and
Preview worker ownership and is not justified by the measured 12.98--23.58 MiB steady-view range.

## Search-path allocation audit

The first case-insensitive search draft built a lowercase copy of the whole document plus two
document-length offset tables, and Replace All retained every match range. Both were rejected.
The final algorithm uses:

- streaming Unicode lowercase tokens plus KMP, O(n+m) time and O(m) auxiliary memory;
- a query-length source-boundary ring that rejects lowercase-expansion midpoint matches;
- compact bounded navigation ranges only while the search panel is open;
- one forward Replace All output build without retaining every replacement range;
- no whole-document clone on each query keystroke.

Release smoke on a 1 MiB mixed CJK/Latin/emoji/combining-mark document, 30 measured
case-insensitive scans after warm-up, reported median `4.9862 ms`, p95 `5.9297 ms`, and maximum
`6.0965 ms`; the p95 gate is `< 50 ms`.

## Evidence identity

| Receipt | Release EXE SHA-256 |
| --- | --- |
| `target/phase14-memory-before-source.json` | `bc1ab99ad35e7c81097d963fe81a9b16a4360298ef6dbe70f3ba97c8bf5a28c7` |
| `target/phase14-memory-before-preview.json` | `bc1ab99ad35e7c81097d963fe81a9b16a4360298ef6dbe70f3ba97c8bf5a28c7` |
| `target/phase14-memory-before-split.json` | `d62059db65000f543d431212c7e799a476c967718d62d71a7d4b2774deb68685` |
| `target/phase14-memory-before-source-after-preview.json` | `d62059db65000f543d431212c7e799a476c967718d62d71a7d4b2774deb68685` |
| `target/phase14-memory-after-source-after-preview.json` (rejected trial) | `2c74c1853e21dbf49e244a251e7a943a75a494e234b11cf5d565f14810977c54` |

The first table is engineering attribution assembled during implementation and spans two
pre-optimization executable hashes. Final qualification must rerun the complete source-preview
module against one exact candidate; this report must not be used to bypass that gate.

## Result

`PASS` for the Phase 14 memory-attribution task. This result does not make an unfrozen worktree a
release candidate and does not replace exact-candidate Resources qualification.
