# Phase 14 Global Module Re-review

Date: 2026-08-25  
Starting commit: `7dc258325a37251550ff9f478d546e171aed69fb`  
Scope: Phase 14 current implementation; no Phase 15, tag, publish, startup-policy change, or
architecture-contract rewrite.

## Executive Result

The second full module review found two related Source search lifecycle defects and corrected them
without changing the plan-owned architecture:

1. closing the search overlay left its bounded match projection allocated and every later canonical
   edit still rescanned the complete document; and
2. the open search overlay consumed `Ctrl+S` / `Ctrl+Shift+S`, so the globally contracted save and
   export shortcuts did not reach the persistence instruction boundary.

External/recovery full projection resynchronization now also refreshes an open search projection.
No additional correctness defect, unbounded queue/cache, authority duplication, forbidden
dependency, or skeleton-level architecture drift was found. The worktree is not an exact candidate;
old guided-manual receipts must not be carried forward.

## Findings Corrected

| Severity | Area | Root cause | Correction / regression |
| --- | --- | --- | --- |
| Medium | Search CPU and memory | `SearchSession::close` retained up to 262,144 ranges (about 2 MiB payload), while every later `DocumentChanged` still called an unconditional O(n) scan | Closing drops the derived `Vec`, generation and truncation state but retains the small query/replacement strings; a closed session ignores refresh requests. A regression opens 100,000 matches, proves the allocation is released, and proves later generations do not repopulate it |
| Medium | Search reconciliation | External/recovery full projection resync rebuilt Source and Preview but left an open search session bound to the old generation | The same immutable `DocumentSnapshot` now refreshes the search projection before Source resync; closed sessions remain O(1) no-ops |
| Medium | Global persistence shortcuts | Search input routing consumed all unhandled shortcuts before `handle_shortcut`, including save/export | `Ctrl+S` and `Ctrl+Shift+S` are translated to typed `PersistenceIntent` before search-field routing; document clipboard/undo shortcuts remain isolated from search input |

## Module Review

| Module group | Cohesion / coupling result | Algorithm and resource result |
| --- | --- | --- |
| `stickymd-core` document/edit/undo/generation | PASS. `DocumentState` remains the only mutable canonical text authority; typed mutation and persist acknowledgement remain failure-atomic | `String` edits and explicit snapshots are measured and bounded; history remains 256 entries / 4 MiB; no rope or shared mutable text is justified |
| Core persistence/assets value objects | PASS. Durable facts, conflicts, recovery and managed names contain no Win32/filesystem execution | Hashing and scans are linear; no database, WAL, runtime serialization of `DocumentState`, or global mutable state |
| Source projection/editor/IME | PASS. cosmic-text is a disposable projection and cannot save; preedit remains non-authoritative | Ordinary deltas update affected logical lines; explicit full resync remains off the per-key path; caret damage avoids full-frame conversion during blinking |
| Source literal search | PASS after correction. Query/replacement/ranges remain session projection only; save/export cross through typed persistence instructions | Case-sensitive standard search and Unicode lowercase KMP remain O(n+m+matches); Replace All streams one output; retained ranges are bounded and are now released immediately on close |
| Markdown/Preview/selection | PASS. Comrak Arena is transient and generation-tagged owned/layout results are derived facts | Background latest-only worker, viewport indices and bounded repaint avoid UI-thread full parse or full-document paint scans |
| RaTeX/math caches | PASS. RaTeX owns parse/layout semantics and StickyMD only paints display lists | Layout 512 entries, raster 8 MiB and outline 4 MiB remain bounded; source strings are shared rather than duplicated |
| Images/managed assets/export | PASS. Proof, identity and safe-boundary checks precede mutation; export never changes the working note | Decode/input limits, 16 MiB/512-entry image cache, streamed hashing and no-replace staging remain bounded |
| Persistence/recovery/config/watcher | PASS. Workers consume immutable requests; the coordination thread alone commits runtime transitions | Note mailbox remains one in-flight plus one latest pending; guarded OCC protects correctness even if watcher hints fail; config coalesces independently |
| Window/tray/dock/theme/opacity | PASS. Reducers own state transitions and Win32 adapters own effects; pin and auto-hide authorities remain orthogonal | Constant-time reducers, event-driven idle behavior and caret damage presentation avoid polling/full redraw loops |
| Windows adapters | PASS. Runtime unsafe remains confined to approved platform modules with adjacent `SAFETY` contracts | Narrow Win32 features, flushed atomic replacement and classified partial failures remain unchanged |
| Smoke CLI / CI / acceptance | PASS. Rust CLI owns the headless workspace test shard; PowerShell remains a thin phase entry; manual rows remain `NOT TESTED` | Test/performance/resource lanes stay separable; no full GUI/resource campaign is required for this search-only correction |

## Long-term Trade-offs

- The Source line-start suffix adjustment is O(number of later logical lines). Current 1 MiB edit
  measurements are far below the hard gate, so a rope/Fenwick/piece-table layer would add authority
  and synchronization complexity without evidence-based value.
- Markdown remains a debounced background full parse/layout. This deliberately trades incremental
  parser complexity for a simple generation contract and remains inside the measured gate.
- Small LRUs may scan at most 512 entries when evicting. Replacing that deterministic bounded scan
  with a linked-map dependency would not materially improve the current workload.
- Source/Preview keep separate shaping state because they live on different ownership/thread
  boundaries. The last copied Release resource receipt was substantially below the 40/52/64 MiB
  hard gates; merging that state would increase coupling for no measured memory need.

## Targeted Evidence

- all 248 `stickymd-win` unit/integration tests: 240 passed, 8 ignored performance receipts;
- closed-search projection allocation regression: PASS;
- global save/export shortcut translation regression: PASS;
- 1 MiB Unicode case-insensitive literal search Release result: median 5.54 ms, p95 6.83 ms,
  max 6.96 ms, below the 50 ms engineering gate.

Final workspace/baseline verification is recorded in the completion response and binds to this
review worktree, not to an exact candidate.

## Architecture Drift

None. The defects were implementation drift inside already-approved Source search and instruction
routing contracts; `docs/plan` did not require modification.

## Recommendation

Commit the reviewed corrections after final baseline verification, then generate a new exact
candidate before any guided manual qualification. Do not tag or publish this worktree.
