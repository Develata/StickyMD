# Phase 14 Global Module Review

Date: 2026-08-25  
Starting commit: `125f868763620158000750bf04e75e9069c4b4d2`  
Scope: current Phase 14 implementation only; no Phase 15, tag, publish, or architecture-contract rewrite.

## Executive Result

The repository keeps the intended authority boundaries and remains suitable for continued v0.1.0
qualification. This review found and corrected seven correctness/data-ownership defects, one
projection-documentation drift, and two avoidable formula-rendering allocations. No database,
network client, async runtime, browser engine, cross-layer filesystem authority, or platform
dependency entered the portable crates.

The current worktree is **not an exact candidate**. Automated evidence below binds to the dirty
worktree executable hash emitted by the smoke CLI, not to the starting commit and not to any older
candidate. Real IME, visual, tray, physical multi-monitor, recovery-choice, Clean VM, and guided
manual rows remain `NOT TESTED` until a new exact candidate is frozen.

## Findings Corrected

| Severity | Area | Root cause | Correction / regression evidence |
| --- | --- | --- | --- |
| High | Recovery/config evidence | Windows rename paths could overwrite an occupied timestamp-derived evidence name or race a preceding existence check | Added same-directory `MoveFileExW` no-replace publication; canonical, temporary and corrupt-config preservation now suffix only after `AlreadyExists` and never overwrite older evidence |
| High | Conflict resolution | An older guarded-save completion releases the one-note acknowledgement barrier and deliberately discards a pending request derived from the old durable fact; this could discard the user's pending `Keep Local` force write | Note completions now carry `PersistMode`; only a `ForceOverwrite` receipt resolves Keep Local, while any superseded guarded receipt resubmits the latest canonical snapshot and keeps quit/hide barriers pending |
| High | Export cleanup | Cleanup remembered only a staging pathname; an external process replacing that pathname before cleanup could cause StickyMD to delete a different object | Staging ownership now records the open-file observation; cleanup reopens with delete authority, verifies the complete observation, and deletes the proved handle-bound object only |
| Medium | Unicode literal search | A lowercase-expansion match rejected at a source boundary reset KMP state and could skip a later valid suffix match | Invalid expanded matches retain the KMP prefix suffix; accepted matches remain non-overlapping; a dotted-I/combining-mark regression covers the skipped-match case |
| Medium | Durable line endings | `LineEnding::apply` treated an adjacent `\r\n` in canonical runtime text as already-normalized CRLF and consumed the user-owned isolated `\r` byte | Save-boundary conversion now replaces canonical `\n` tokens only and preserves any preceding isolated carriage return |
| Medium | Preview selection | Partial selection geometry divided a visual run by UTF-8 byte count, making CJK/emoji highlight widths incorrect | Geometry now clips on grapheme counts; logical-line and wrapped-row ranges remain disjoint and viewport indexed |
| Medium | Math rendering | Raster misses cloned the entire RaTeX `DisplayList`; RenderTree and formula-cache keys also copied the same formula source | Foreground substitution is borrowed while painting, explicit TeX colors remain intact, and RenderTree/layout/raster keys share one `Arc<str>` source allocation |
| Low | Config cohesion | Runtime config value semantics and durable TOML/atomic-file operations occupied one module | Split pure `config::runtime` from adapter-facing `config::storage` without changing the public config contract |
| Low | Documentation | Architecture/Phase 8 projections still described the obsolete 70% opacity floor | Corrected current projections to the contracted 40–100 range; historical reports/prompts were not rewritten |

## Module Review

| Module group | Cohesion / authority result | Complexity and resource result |
| --- | --- | --- |
| `stickymd-core` document/text/edit/generation/undo | PASS. `DocumentState` remains the only mutable canonical text authority; mutations stay typed and failure-atomic; disk/editor/preview cannot obtain mutable text | `String` edits are O(n), history is bounded by 256 entries and 4 MiB, snapshots copy only at explicit worker boundaries. Release 1 MiB common edits remain well below the 50 ms hard gate |
| `stickymd-core` persistence/assets | PASS after isolated-CR correction. Durable facts and managed names are pure value objects; no Windows or filesystem adapter enters core | Hashing/scanning are linear and bounded; no persistent undo, WAL, database, or global mutable state |
| Source projection/editor/IME | PASS. The cosmic-text buffer remains a disposable projection and cannot save independently; preedit remains non-authoritative | Ordinary deltas rebuild affected logical lines; suffix line offsets are O(lines), consistent with the current measured String model. 1 MiB common edit p95 is 1.1–3.3 ms. Full projection rebuild p95 is about 74.7 ms and is limited to explicit startup/reload/recovery/conversion boundaries |
| Markdown owned AST / RenderTree / preview layout | PASS. Comrak Arena is transient; owned tree, RenderTree and layout are generation-tagged worker projections | Full parse/layout remains off the UI thread. Warm p95 is about 6.6 ms (20 KiB), 22.3 ms (100 KiB), and 261.7 ms (1 MiB), inside the 100/400/2000 ms hard gates. Visible-row and visible-block indices avoid full-document paint scans |
| Preview selection / links | PASS after grapheme correction. Selection is a read-only clipboard projection and links are typed/scheme-gated | Row binary search plus viewport clipping avoids per-frame whole-document scans. Exact real mouse/clipboard behavior remains manual |
| RaTeX math / painter / caches | PASS after clone removal/source sharing. RaTeX retains parse/layout authority; StickyMD only paints its `DisplayList` | 512-entry layout, 8 MiB raster and 4 MiB outline bounds remain unchanged. Cold-first representative formula measured about 1.15 ms; warm representative hits were about 0.1–0.5 us |
| Images / managed assets | PASS. Managed-name proof, root identity, reparse rejection, hash proof, handle-bound mutation and safe-boundary reconciliation remain layered | Encoded/decode limits and strict live-lease cache accounting remain bounded. No remote fetch exists |
| Persistence worker / reconciliation / recovery | PASS after evidence and Keep Local fixes. Worker executes immutable requests only; the UI coordination thread alone commits `DocumentState` transitions | Mailbox remains at most one in-flight plus one latest pending note. OCC hashes immediately before guarded publish; watcher is an acceleration path, not the correctness gate |
| Config | PASS after module split. Config revision/state is separate from document generation and note durability | Latest-only coalescing; atomic TOML publication; corrupt/newer evidence is preserved without blocking note editing |
| Export | PASS after cleanup ownership fix. Export consumes a snapshot and never mutates the working note | Assets are streamed and hashed, identical assets deduplicate, staging/final publish is no-replace, cleanup is handle-bound. 20 references to one 1 MiB asset measured 11.17 ms median / 13.70 ms p95 |
| Window shell / dock / tray / theme / opacity | PASS by code and reducer automation. Always-on-top and auto-hide remain orthogonal; shell effects are applied only by Windows adapters | Reducers use constant-time transitions; idle redraw is event driven. Real shell, physical display and IME interaction remain manual |
| Windows platform adapters | PASS. All runtime unsafe remains under `platform/windows`, with adjacent `SAFETY` explanations; portable crates contain zero runtime unsafe | Win32 feature surface remains narrow. Atomic replacement still uses flushed temp + `ReplaceFileW(flags=0)` and classified partial failures; no blanket fallback |
| Smoke CLI / CI / acceptance projections | PASS for checked-in automation structure. Rust CLI owns automated cases and PowerShell remains a thin entry; manual rows remain `NOT TESTED` | Modular resource groups avoid forcing a full GUI campaign after every focused change. This review ran only `source-preview` resources plus targeted Release benchmarks before the final headless shard |

## Architecture Boundaries

- Canonical text owner: `DocumentState`.
- Durable representation: `<program-dir>/note/note.md`; it is an external fact, not peer authority.
- Save source: immutable `DocumentSnapshot`, never cosmic-text or Preview.
- External entry: watcher hint -> bounded read/hash -> reconciliation coordinator -> typed document transition.
- UI: emits typed intents and paints projections; it performs no direct durable file I/O.
- Workers: own parsing, rendering and I/O execution, but cannot mutate `DocumentState`.
- Windows: unsafe handles and platform side effects stay in the approved adapter directory.

No architecture drift requiring a plan change was found.

## Algorithms and Long-Term Trade-offs

- Literal search is streaming KMP: O(n + m) time and O(m) matcher memory. Replace All streams
  output and does not retain all ranges; UI highlight retention remains bounded.
- Source line-offset adjustment is O(number of following logical lines). Replacing it with a rope,
  Fenwick tree or piece table is not justified while measured 1 MiB editing is far below the gate;
  the public document boundary still permits a future evidence-driven replacement.
- Markdown deliberately uses background full parse/layout with debounce instead of an incremental
  parser. This keeps authority and failure behavior simple while measured 1 MiB p95 remains about
  262 ms against the 1 s target.
- Small deterministic LRU caches currently find the oldest of at most 512 entries in O(512) on
  insertion. A linked-map dependency would add more complexity than the bounded scan saves.
- `app/input.rs` is about 639 production lines and remains the main cohesion watch item. It still
  owns one stable responsibility—native input translation—and splitting keyboard/pointer paths now
  would duplicate access to the same selection/IME/search state without measured benefit.
- The 512-entry formula layout contract is entry-bounded rather than byte-bounded. Current resource
  measurements are far below hard gates; adversarial near-64-KiB unique-formula residency should be
  remeasured before changing this plan-owned eviction contract.

## Current Worktree Performance and Resources

All timing numbers are Release measurements on the current host. They are engineering evidence,
not cross-machine guarantees.

| Path | Current result |
| --- | --- |
| 1 MiB Source common edits | typing p95 about 1.06 ms; backspace/delete/selection/newline/undo/redo p95 about 1.39–3.24 ms |
| 1 MiB Source full resync + viewport paint | p95 about 74.71 ms; explicit full-boundary path, not per-key input |
| Preview warm build p95 | 20 KiB 6.57 ms; 100 KiB 22.33 ms; 1 MiB 261.71 ms |
| Preview cold pipeline + first build | about 202.68 / 233.00 / 369.20 ms; pipeline/font initialization occurs on the dedicated preview worker |
| Formula engine | cold-first 1.15 ms; warm representative formulas 0.1–0.5 us |
| 1 MiB Unicode case-insensitive search | median 5.52 ms; p95 6.24 ms; max 6.42 ms |
| Atomic persistence end-to-end p95 | 20 KiB 7.28 ms; 100 KiB 5.92 ms; 1 MiB 10.23 ms |
| Export 20 refs / shared 1 MiB asset | median 11.17 ms; p95 13.70 ms; max 17.81 ms |

The copied Release `source-preview` resource module passed on the dirty worktree executable
`762a667c9f72dc29955b879fe535a16daf0e8e0a9f26698359fcc49a33a0a608`:

| State | PWS median / max | Private Bytes median / max | Idle CPU p95 |
| --- | --- | --- | --- |
| Source | 12.98 / 13.01 MiB | 15.00 / 15.06 MiB | 0.002604% |
| Preview | 15.41 / 15.42 MiB | 16.64 / 16.68 MiB | 0.001302% |
| Split | 20.96 / 20.98 MiB | 23.51 / 23.53 MiB | 0.005208% |

These results are well below the 28/40/48 MiB preferred PWS gates and 40/52/64 MiB hard gates.
The user's earlier approximately 55 MiB Task Manager observation is therefore not representative of
this copied Release executable and may have been a Debug process, an older build, or a different
Task Manager memory column. No large allocator-level optimization is justified by current Release
evidence.

## Verification Status

Completed on the final review worktree:

- targeted correctness tests for every corrected invariant;
- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- `cargo test --workspace --locked`;
- `cargo build --workspace --release --locked`;
- Release tests for `stickymd-core`, `stickymd-render`, and `stickymd-win`;
- `cargo run -p stickymd-smoke --locked -- all --ci --ci-shard=tests --json`;
- targeted Release Source, Preview, math, search, persistence and export baselines;
- copied Release `source-preview` resource module;
- forbidden-architecture, unsafe-boundary, and direct-filesystem-access audits;
- `git diff --check`.

All automated commands above passed. The smoke receipt deliberately reports
`worktree_dirty: true`: it binds the tested executable hash to this review worktree and is not an
exact-candidate qualification receipt.

## Remaining Qualification Gaps

- No new exact candidate has been frozen from this worktree.
- Guided manual G1/G2/G3 receipts for the future exact candidate are not carried forward.
- Real Microsoft Pinyin and WeChat IME, actual mouse selection/copy, tray/taskbar/Alt+Tab,
  left/top/right docking, physical multi-monitor/DPI, recovery choice, Clean VM and remote artifact
  checks remain exactly as marked in their acceptance matrices.
- Full all-module resource qualification was intentionally not repeated; the current focused
  source/preview module passed, while math/image/window/zoom modules retain prior evidence only and
  require a new exact-candidate campaign before release.

## Recommendation

Continue Phase 14 qualification after committing these corrections. Generate a new exact candidate
from the resulting clean commit; do not tag or publish from the review worktree or carry previous
manual receipts onto the new candidate.
