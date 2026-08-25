# Phase 14 Pre-Release Module Audit

## Scope and baseline

- Starting commit: `085152cb45f9cd3c46ab44c9f3d3109763396c53`
- Branch: `main`
- Starting worktree: clean
- Scope: `stickymd-core`、`stickymd-render`、`stickymd-win`、smoke/CI/release contract。
- Authority: `DocumentState` remains the only runtime canonical text owner；disk、source projection、preview tree、worker snapshot and watcher facts remain projections or external facts。
- This audit changes source code and therefore invalidates the previous exact candidate and its manual receipts。

## Executive result

| Area | Result | Evidence |
| --- | --- | --- |
| Core authority / UTF-8 / Undo / persistence acknowledgement | PASS | Debug + Release tests；no mutable text bypass；core `unsafe = 0` |
| Preview generation / layout / selection / paint | PASS after fixes | stale-result、hard-break、viewport-selection、lazy-image regressions |
| Math engine and caches | PASS after fix | semantic layout cache is theme-independent；explicit TeX color preserved |
| Persistence / recovery / worker ordering | PASS after fixes | missing-canonical recovery receipt、note acknowledgement barrier、OCC tests |
| Managed files / assets / export | PASS after fixes | hard-link temp rejection、streamed ownership hash、staging collision tests |
| Windows shell state | PASS for automated contracts | reducer geometry、pin/auto-hide orthogonality、tray/tool-window tests；fresh exact-candidate manual still pending |
| CI / release portability | PASS | tests shard、`cargo deny`、native-runtime PE gate；no developer runtime imports |

## Findings fixed

### Data reliability and ordering

1. Fixed a missing-canonical recovery path that could mark the empty runtime note clean and delete the only valid `note.md.tmp` evidence before an empty `note.md` had actually been published。The replacement now remains dirty until a real persist acknowledgement；temporary evidence is removed only after success。
2. Fixed the persistence mailbox selecting a second note write while the previous completion still awaited coordinator acknowledgement whenever unrelated work was also queued。Job selection now enforces one note in-flight plus one latest pending at the actual dequeue boundary。
3. Fixed fixed-name atomic temporaries accepting an existing hard link and truncating the linked user file。Existing temporaries are opened without write/delete sharing, inspected through the open handle, and accepted only as ordinary non-reparse single-link files。
4. Fixed stale preview relayout/paint requests mutating current layout/cache state before generation validation；stale work now fails without changing the current projection or counters。
5. Fixed preview-worker coalescing so an older generation can never replace a newer pending generation, regardless of job kind。

### Rendering correctness

1. Hard Markdown breaks now remain semantic layout boundaries when adjacent to math or images；they no longer behave as zero-width text on the preceding visual line。
2. Math semantic layout cache no longer includes theme foreground。Theme changes rerasterize only；explicit TeX colors remain explicit rather than being overwritten by the preview foreground。
3. Viewport/image-source relayout validates generation before releasing the previous layout, preserving the last good preview on stale requests。

### Complexity, allocations, and memory

1. Preview selection painting changed from a whole-document scan/allocation to row-index lookup plus visible-row traversal: `O(log rows + visible boxes)` rather than `O(all boxes)` per frame。
2. Preview framebuffer completion now consumes the pixmap allocation directly instead of copying the full RGBA buffer。
3. Equal visual/copy span text shares one `Arc<str>` allocation；only image placeholders retain distinct visual and clipboard strings。
4. Stable PNG/JPEG/WebP/GIF paste consumes the worker-owned byte vector, eliminating an additional encoded-image clone of up to 64 MiB while validation decode is live。
5. Managed-asset ownership proof hashes through a fixed 64 KiB buffer: `O(n)` time and `O(1)` auxiliary memory instead of allocating up to 64 MiB。
6. Lazy-image admission keeps at least one viewport of margin, avoiding repeated full layout rebuilds on ordinary nearby scroll while retaining bounded image-cache ownership。
7. Corrupt-config evidence、export staging and writable probes now use bounded collision-resistant suffix search and never overwrite stale evidence。Export name exhaustion has its own typed error。

## Cohesion and coupling review

- `stickymd-core` remains platform-free and owns document invariants only。
- `stickymd-render` consumes immutable snapshots and owns parser projection、layout、selection and raster caches；it cannot write files or mutate `DocumentState`。
- `stickymd-win` keeps filesystem and Win32 calls in execution/platform adapters；UI/flow modules do not directly perform note I/O。
- Preview and persistence workers have bounded mailboxes and return typed generation-tagged results to the coordinator；neither owns UI/window objects or mutable document authority。
- Files above the 500-line review threshold were inspected by responsibility。`app/input.rs`、source projection、parser、persistence worker and asset storage remain single-axis modules with large inline invariant tests；mechanical splitting would increase cross-module private state without creating a stable capability boundary, so no speculative split was made。
- No runtime dependency was added。`cargo deny` reports only approved transitive version duplication from RaTeX/Comrak/cosmic-text/winit/tray dependencies；there is no safe direct-version edit that removes it without changing an upstream baseline。

## Performance and resource evidence

### Headless Release receipts

- 1 MiB source common edit p95: approximately `1.1–3.3 ms`; full resync p95: `75.7 ms`。
- 1 MiB preview total p95: `314.8 ms` (background path, below the 2 s hard gate)。
- 1 MiB note persistence end-to-end p95: `12.2 ms`。
- 1024×768 image decode/resize p95: `36.7 ms`; cache-hit p95: `4 µs`。
- 1 MiB managed-reference full scan p95: `434 µs`; incremental edit window p95: `5 µs`。
- Source edits while the math worker is busy p95: `1.9 µs`。

### Copied Release Source / Preview / Split resource module

| Mode | Private Working Set median / max | Private Bytes median / max | Idle CPU p95 |
| --- | --- | --- | --- |
| Source | 12.8 / 12.9 MiB | 14.8 / 14.9 MiB | 0.003906% |
| Preview | 15.3 / 15.4 MiB | 16.6 / 16.7 MiB | 0.002604% |
| Split | 20.7 / 20.8 MiB | 23.3 / 24.2 MiB | 0.002604% |

All are below the v1 hard resource gates。This targeted resource module passed；the complete resource qualification campaign was intentionally not rerun because this was a modular audit, not an exact-candidate qualification。

## Deferred measured risks

1. Continuous typing undo grouping appends immutable payloads and can accumulate quadratic copying over one very long uninterrupted group。The group is bounded by the existing undo budget and typical edits are well below latency gates；changing grouping semantics or introducing a mutable builder without a dedicated benchmark would add more risk than value。Retain as a benchmark target, not an unmeasured rewrite。
2. Source line-start maintenance remains linear in following line count for a structural edit。Release evidence at 1 MiB is comfortably below the input gate；a tree/rope index is not justified before measurement disproves the String model。
3. The previous exact candidate and all candidate-bound manual evidence are stale after these fixes。A new exact candidate and fresh manual qualification are required before any RC/tag claim。

## Verification

- `cargo fmt --all -- --check`: PASS。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS。
- `cargo test --workspace --locked`: PASS。
- `cargo build --workspace --release --locked`: PASS。
- Release tests for core/render/win: PASS。
- All ignored headless Release performance receipts for core/render/win: PASS。
- `stickymd-smoke all --ci --ci-shard=tests --json`: PASS。
- Phase 14 targeted `source-preview` resource module: PASS。
- `cargo deny check`: PASS with approved duplicate-version warnings only。
- portable native-runtime dependency gate: PASS；developer runtime imports `none`。
- `git diff --check`: PASS。

## Recommendation

The architecture skeleton remains high-cohesion and authority-safe after the fixes。Do not tag yet；freeze a new exact candidate and repeat candidate-bound manual qualification。
