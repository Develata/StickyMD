# Phase 7 Managed Images and Export Report

## Executive Result

| Capability | Result | Evidence |
| --- | --- | --- |
| Managed Asset Identity | **PASS** | strict 20/32/64 lowercase grammar, canonical extension and full SHA-256 prefix proof |
| Ownership Safety | **CONDITIONAL** | canonical ordinary-file + name + content proof and replaced-root automation pass; real symlink/junction receipt remains `NOT TESTED` |
| Clipboard File Images | **CONDITIONAL** | production CF_HDROP and bounded worker pipeline implemented; real Explorer sources `NOT TESTED` |
| Clipboard Screenshot | **CONDITIONAL** | DIB/V5/CF_BITMAP→PNG automated; Snipping Tool/Paint `NOT TESTED` |
| Managed Paste | **PASS (automated)** | streamed asset-first publication, cumulative bound, one TextDelta, generation OCC and latest-ref failure convergence |
| Undo/Redo Asset Transaction | **PASS (automated)** | pure private effects, text-first semantics, reverse-order grouped undo and eventual I/O convergence |
| Startup Reconciliation | **PASS (automated)** | referenced restore plus durable-fingerprint/handle-gated safe boundary |
| GC | **PASS (automated)** | runtime logical trash; physical delete only from a proven safe boundary, otherwise deferred |
| Native Image Preview | **CONDITIONAL** | standalone/mixed/table native rasters automated; real visual matrix remains open |
| Lazy Decode | **PASS (automated)** | viewport +300 DIP and 100-image bounded-load fixture |
| Decoded Cache | **PASS** | <=16 MiB including deterministic metadata estimate, <=512 entries, hit/miss/eviction counters |
| Remote Zero-Network | **PASS** | remote source never called; no network dependency |
| Export | **PASS (automated), dialog conditional** | semantic nodes, staged local assets, source-range rewrites; native dialog manual row open |
| Memory | **PASS (automated)** | five-run copied-Release matrix stays below typical steady gates and proves same-process cache release; 4K transient peak is recorded separately |
| Idle CPU | **PASS (automated)** | selected 60-second intervals peak at 0.001302% normalized CPU |
| Visual | **NOT TESTED** | real clipboard, local-image/DPI and dialog receipts are not inferred from headless tests |

Phase 7 adds a managed image lifecycle and portable export, not a generic attachment system.
`DocumentState` remains canonical. All user-facing/manual conditions are retained in
[`phase-07.md`](../acceptance-cases/phase-07.md).

## Preconditions

- Starting commit: `44e3e08`.
- Phase 6 recommendation: `APPROVE Phase 7 WITH CONDITIONS`.
- USER supplied and authorized Phase 7.
- Inherited Microsoft Pinyin, WeChat IME, native Preview/math visual and strict same-process
  first-math memory rows remain `NOT TESTED`.
- Environment: Windows 11 Home Chinese build 26200; Intel Core i7-12700H; 16,962,281,472 bytes
  RAM; NTFS; 20 logical processors; Rust/Cargo 1.97.1.

## Managed Identity and Ownership

| Filename / state | Hash match | Location | Classification | Auto-delete allowed? |
| --- | --- | --- | --- | --- |
| `stickymd-<20hex>.png` | yes | canonical `images/` or `.trash/` | proven managed | yes, lifecycle rules only |
| `stickymd-<32hex>.jpg` | yes | canonical managed directory | collision-expanded managed | yes |
| `stickymd-<64hex>.webp` | yes | canonical managed directory | full-hash managed | yes |
| managed-looking basename | no | canonical managed directory | untrusted/user evidence | **no** |
| any user basename | n/a | `images/` or `.trash/` | user asset | **no** |
| valid managed file | yes | outside canonical roots | external/local image | **no** |
| reparse file/directory | unknown | any managed-looking path | ownership unproven | **no** |

Stable PNG/JPEG/WebP/GIF bytes are preserved exactly. BMP, ICO, DIB and raw RGBA normalize to PNG.
Content collisions extend 20→32→64 hexadecimal characters; if all names contain different bytes,
the transaction fails without overwriting them.

## Clipboard Evidence

The platform order is CF_HDROP → registered encoded PNG/JPEG/WebP → DIB/V5 → CF_BITMAP → text.
HGLOBAL data is bounded at 64 MiB, file lists at 256 entries, dimensions before pixel copy, and
OpenClipboard retries at 0/10/30/80 ms only. Full adapter details are in
[`phase-07-windows-clipboard-formats.md`](phase-07-windows-clipboard-formats.md).
An all-image file drop becomes one image transaction. A mixed image/non-image drop becomes CRLF
path text through the established text-paste path; StickyMD never creates a half-image,
half-attachment transaction. A corrupt file bearing an approved image extension instead fails the
image transaction without changing `DocumentState`.

| Source | Available Formats | Selected Path | Stored Format | Result |
| --- | --- | --- | --- | --- |
| Explorer PNG | NOT TESTED | NOT TESTED | expected PNG preservation | NOT TESTED |
| Explorer JPEG | NOT TESTED | NOT TESTED | expected JPEG preservation | NOT TESTED |
| Snipping Tool | NOT TESTED | NOT TESTED | expected PNG normalization | NOT TESTED |
| Paint | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |
| Browser | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |

## Paste Transaction

```text
clipboard descriptor/bytes
  → bounded worker read and image preparation
  → content-addressed files persisted/reused/restored
  → one generation-checked Markdown TextDelta
  → pure asset effects retained by private UndoEntry
```

Images are read, prepared and stored one at a time under a cumulative 128 MiB transaction cap.
Only after the whole batch succeeds is one Markdown edit attempted. Failure or a stale generation
returns a publication ledger to the main coordinator, which derives convergence effects from the
latest canonical reference set; it never blindly replays an obsolete rollback. Text clipboard
behavior remains the established synchronous edit intent.

## Undo and Reference Evidence

| Transition | Canonical action | Asset effect |
| --- | --- | --- |
| paste | one Markdown insertion | referenced files remain/enter `images/` |
| undo paste | text removed first | final 1→0 moves to `.trash/` |
| redo paste | text restored first | 0→1 restores to `images/` |
| normal last-ref delete | text edit succeeds | 1→0 moves to `.trash/` |
| undo normal delete | text restored | 0→1 restores |

Multiple occurrences are counted; deleting one of two references produces no trash effect. The
scanner deliberately counts managed-looking literals in code/raw text, preferring false retention
over false deletion. The 1 MiB full scan remains available for load/reconciliation; ordinary edits
use a UTF-8-safe bounded local window.

## Reconciliation and GC Evidence

| Startup/final state | Automated result |
| --- | --- |
| referenced proven asset in `images/` | keep |
| referenced proven asset only in `.trash/` | restore |
| unreferenced proven asset in `images/` | move to `.trash/` |
| unreferenced proven asset in `.trash/` | delete only while guarded durable note hash + handle + durable/runtime reference union remain valid; otherwise defer |
| user file in either directory | preserve |
| managed-looking wrong-hash file | preserve |
| equal proven active+trash duplicate | remove duplicate trash copy |
| same managed name but unequal full active/trash digests | preserve ambiguous evidence and report collision |

Startup completes safe reconciliation before accepting editor mutation. Normal exit waits pending
paste/input, final note save and latest asset transitions. Each request has a unique identity, so an
old same-generation completion cannot satisfy a newer quit request. The physical-delete boundary
holds a stable `note.md` read handle, verifies the expected durable hash and unions durable/runtime
references; uncertainty becomes non-destructive deferred cleanup. Real forced-crash timing remains
`NOT TESTED`.

## Native Image Preview

- Actual PNG/JPEG/WebP/GIF/BMP/ICO format is inspected; GIF renders first frame only.
- Encoded bytes <=64 MiB, sides <=16,384 and pixels <=40 MP; checked RGBA byte math.
- Local relative paths resolve from `note/`; absolute/user paths are read-only.
- Standalone, mixed-text and table-cell local images preserve aspect ratio and paint native raster;
  standalone images do not upscale.
- Only the viewport plus 300 DIP prefetch band decodes; missing/corrupt/oversized input stays a
  local placeholder and does not fail the document.
- Remote images never reach the local source and stay clickable/visible placeholders.
- Image selection copies alt text, never bitmap data.
- Real visual/DPI acceptance remains `NOT TESTED`.

## Cache Evidence

| Property | Evidence |
| --- | --- |
| max bytes | 16 MiB, including RGBA plus deterministic metadata and rasters leased by current layout chunks |
| max entries | 512, independently bounding allocator/hash-table metadata |
| hits/misses | explicit cumulative counters; repeated identical projection test produces a hit after the initial miss |
| evictions | explicit counter; only unleased rasters can be evicted, so an external layout `Arc` cannot escape accounting |
| lifecycle | Source transition clears pixels/entries; counters remain diagnostic only |

The cache key is source SHA-256 plus target dimensions. User/local mutable images are reread and
rehash on preview rebuild; no filesystem stat or hash runs on each paint.

## Export Evidence

| Property | Automated result |
| --- | --- |
| snapshot generation | immutable current runtime snapshot; worker never owns `DocumentState` |
| assets directory | `stem-assets`, then `stem-assets-2...`; existing user directories untouched |
| rewritten paths | real local image nodes only, reverse source-range application |
| dedup | same source and different paths with equal full SHA-256 copy once |
| remote preservation | URL unchanged, zero request |
| reference-style image | occurrence locally normalized to inline image without changing shared normal links |
| missing local file | fail before final Markdown/assets publication |
| no-image Markdown | byte-exact UTF-8 source projection, no assets directory |
| failure cleanup | only unique staging owned by this export is removed |
| Markdown temp | exclusive `create_new`; a pre-existing predictable file is never truncated or removed |
| working-note alias | canonical path and Windows file identity reject the note itself and hard-link aliases |
| output path safety | asset directory component is percent-encoded and roundtrips through the parser/resolver |
| hash-prefix collision | 20→32→64 expansion preserves both distinct full digests |

Assets publish before the atomic Markdown file. If Markdown publication fails after assets publish,
the uniquely named asset directory is retained; this fail-safe may leave recoverable clutter but
cannot delete an existing user directory or publish known-broken Markdown. Export never changes the
working path, generation, dirty state or Undo/Redo history.

## Performance

All values are current local Release measurements over 30 samples unless noted. p50/p95/max are
reported; clipboard capture from real producer applications is not represented by file fixtures.

| Pipeline | Input | Median | p95 | Max |
| --- | --- | ---: | ---: | ---: |
| full conservative scan | 1 MiB Markdown | 392 µs | 468 µs | 546 µs |
| incremental scan | 1 MiB, local edit window | 1 µs | 4 µs | 5 µs |
| managed paste end-to-end worker preparation | ten tiny PNG files | 84.629 ms | 112.992 ms | 124.668 ms |
| scaled decode+resize | 1024×768→800×600 PNG | 21.885 ms | 29.662 ms | 35.468 ms |
| export | 20 refs, one 1 MiB shared asset | 14.751 ms | 19.985 ms | 20.357 ms |

Paste stage table (independent Release stage observations; real clipboard producer capture remains
manual):

| Input | Capture | Inspect | Hash/Encode | Persist | Document Insert |
| --- | ---: | ---: | ---: | ---: | ---: |
| ten tiny PNG files | 1.061 / 3.185 / 4.300 ms | 17 / 33 / 34 µs | 30 / 53 / 54 µs | 83.222 / 107.709 / 110.542 ms | 12 / 22 / 22 µs |

Decode table (`decode+resize` is intentionally one production API so a full-resolution decoded
copy is not retained between stages):

| Format/Size | Inspect | Decode + Resize | Cache Insert |
| --- | ---: | ---: | ---: |
| PNG 1024×768→800×600 | 31 / 56 / 171 µs | 21.885 / 29.662 / 35.468 ms | 2 / 3 / 67 µs |

## Memory and Idle CPU

The deterministic cache hard invariant is **PASS**. The copied-Release OS resource matrix runs five
fresh processes per mode after 30-second warmup and measures selected 60-second idle intervals.
The last row first saturates Preview with 420 distinct 128×128 BMP rasters, clicks the real Source
toolbar control in the same process, waits for durable mode acknowledgement, then measures the
released state. Values are MiB (2^20 bytes) from the stable
`tools/smoke/phase-07.ps1 -Resources` receipt.

| State | Private Working Set median / max | Private Bytes median / max | Peak WS / Peak Private max | Idle CPU |
| --- | ---: | ---: | ---: | ---: |
| Source, no images | 7.49 / 7.53 | 8.36 / 8.40 | 22.77 / 23.23 | not measured |
| Source, 12 local refs lazy | 7.38 / 7.39 | 8.22 / 8.24 | 22.56 / 23.07 | 0.001302% |
| Preview, no images | 16.30 / 16.32 | 17.32 / 17.36 | 32.95 / 21.96 | not measured |
| Preview, 1 image | 16.22 / 16.25 | 17.49 / 17.54 | 33.12 / 21.93 | not measured |
| Preview, 12 images | 15.78 / 15.90 | 16.82 / 17.14 | 32.73 / 21.83 | 0.000000% |
| Split, 12 images | 16.72 / 16.89 | 17.96 / 18.14 | 32.84 / 22.45 | 0.000000% |
| Preview, one 3840×2160 BMP downscaled | 16.43 / 16.78 | 17.50 / 17.84 | 93.93 / 79.93 | not measured |
| Preview, cache saturated | 24.12 / 24.25 | 25.91 / 25.92 | 41.07 / 27.26 | 0.001302% |
| Split, cache saturated | 26.05 / 26.10 | 27.85 / 27.93 | 42.06 / 27.93 | 0.000000% |
| Source after saturated Preview, same process | 11.32 / 11.59 | 14.87 / 15.02 | 41.02 / 27.30 | 0.000000% |

Every steady sample is below the original typical Preview 52 MiB / Split 64 MiB hard gates. The
4K BMP transient peak is intentionally much higher: the local source bytes (~31.6 MiB), decoded
full-resolution buffer and scaled raster overlap briefly. The scaled raster is the only copy
admitted to the cache, and steady memory returns to at most 16.8/17.9 MiB. Reducing this peak would require
a reader/decoder boundary capable of streaming or format-specific downsampling; it is recorded as
a measured optimization opportunity, not hidden behind the steady result.

## Binary Size

| Artifact | Bytes |
| --- | ---: |
| Phase 6 stripped Release EXE | 6,930,944 |
| Phase 7 stripped Release EXE | 8,072,192 |
| Delta | +1,141,248 (+1.088 MiB, +16.47%) |

The delta is below the Phase 7 +5 MiB review trigger. Dependency details are in
[`phase-07-dependency-delta.md`](phase-07-dependency-delta.md).

## Windows API and Unsafe

Phase 7 adds clipboard access (`OpenClipboard`, `CloseClipboard`, `GetClipboardData`,
`IsClipboardFormatAvailable`, `RegisterClipboardFormatW`, `GlobalLock/Size/Unlock`,
`DragQueryFileW`, `GetObjectW`) and COM save-dialog calls (`CoInitializeEx`, `CoCreateInstance`,
`IFileSaveDialog`, `CoTaskMemFree`). Managed-file mutation adds `SetFileInformationByHandle`
(`FileRenameInfo` without replacement and `FileDispositionInfo`) over the exact source handle;
managed-file, directory and note handles deny write/delete sharing for the complete proof-to-mutation
boundary. Failed new-asset publication cleans up through the retained create-new handle, never by
re-resolving a replaceable pathname. Each unsafe block is
contained in `platform/windows/{clipboard,export_dialog,managed_file}.rs` and documents
pointer/handle/ownership/thread invariants.

- `stickymd-core` runtime unsafe: **0**.
- `stickymd-render` runtime unsafe: **0**.
- Asset storage/export/decoder logic: safe Rust.

## Architecture Authority

| Question | Answer |
| --- | --- |
| Who owns Markdown text? | `DocumentState` only |
| Who determines managed reference? | conservative tracker derived from canonical text |
| Who proves asset ownership? | Windows app storage adapter from stable canonical roots + exact ordinary-file handle + strict name + full content hash |
| Who performs file movement? | the existing single I/O worker through `AssetStorage` |
| Can Preview delete assets? | no |
| Can ImageDecoder mutate Document? | no |
| Can GC use Preview AST as authority? | no |
| Can Export switch active document? | no |

No generic attachment abstraction, worker thread pool, filesystem authority in core, or second
document state was introduced. Preview image caches and export projections are disposable. A
`request_id` orders asset jobs; document generation remains a stale-content token and is not
misused as request identity.

The native Preview layout was cohesion-reviewed after image integration. Standalone image sizing
and lazy decode now live in `preview/image_layout.rs`; attributed text shaping and selection-box
projection live in `preview/text_layout.rs`; `preview/layout.rs` remains the block orchestrator.
The pre-existing source projection is 521 production lines, but its incremental line mutation,
buffer mirror and generation checks share one `SourceProjection` invariant; it was reviewed and
kept cohesive rather than split into helpers that would couple across its private mutable fields.

## Acceptance and Manual Conditions

The durable matrix is [`docs/acceptance-cases/phase-07.md`](../acceptance-cases/phase-07.md), driven
by [`tools/smoke/phase-07.ps1`](../../tools/smoke/phase-07.ps1) and the Rust CLI. Headless CI calls
the deduplicated all-phase graph. These remain `NOT TESTED`:

- Explorer/Snipping Tool/Paint/browser clipboard formats and visible paste lifecycle;
- local-image alpha/wide/tall/missing/corrupt rendering at real 100/150/200% DPI;
- native Ctrl+Shift+S dialog and Explorer-opened export;
- forced-crash asset reconciliation timing;
- actual Windows file-symlink and directory-junction escape attempts in an enabled test environment;
- inherited Microsoft Pinyin, WeChat IME and Preview/RaTeX visual matrices.

## Architecture Drift

None observed after review remediation. Initial review found stale destructive reconciliation,
path-based ownership, rollback, grouped-undo, mixed-inline and export-alias/temp gaps. The final
implementation closes them with quiescent/durable safe boundaries, exclusive proof-to-mutation
handles, handle-bound failure cleanup, latest-reference convergence (including an already-running
older sync), reverse-order effects, native inline/table rasters and exclusive export publication.
The quit path has deterministic paste→latest-save→safe-GC and mutation-freeze tests. Clean reload
and Load External share one typed Flow effect whose sole Shell route performs both full projection
resync and runtime asset convergence. No plan boundary was weakened to accommodate the earlier
implementation.

## Verification

Final command receipts are recorded after the last patch. Phase-specific results already obtained:

| Entry | Result |
| --- | --- |
| `tools/smoke/phase-07.ps1` | PASS |
| `tools/smoke/phase-07.ps1 -Performance` | PASS; scanner/paste/decode/export stage instrumentation recorded |
| `tools/smoke/phase-07.ps1 -Runtime` | PASS; copied Release local-image lifecycle |
| `tools/smoke/phase-07.ps1 -Resources` | PASS; final full ten-case copied-Release receipt, including same-process Preview→Source release |
| `tools/smoke/all.ps1 -Ci` | PASS; CI's full headless Phase 0–7 graph and Release performance tasks |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `cargo build --workspace --release --locked` | PASS |
| core/render/win Release tests | PASS |
| `cargo deny check` | PASS with pre-existing duplicate-version warnings only |
| `git diff --check` | PASS |

## Recommendation

**APPROVE Phase 8 WITH CONDITIONS**: retain all manual rows above as `NOT TESTED`; do not weaken
managed ownership proof, content-addressed immutability, generation OCC, bounded decode/cache,
zero-network behavior or source-preserving export.
