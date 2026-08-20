# Phase 5 Markdown Native Preview Report

## Executive Result

| Capability | Result | Evidence |
| --- | --- | --- |
| Comrak dialect | **PASS** | exact 0.54.0, six enabled extensions, default features disabled |
| Minimal dependency configuration | **PASS** | CLI/syntect/onig absent; Comrak only in render |
| Arena → Owned AST | **PASS** | transient Arena facade, stable golden owned-tree, source ranges |
| Source range mapping | **PASS** | ASCII/CJK/emoji/mixed UTF-8 roundtrips and malformed fail-safe |
| RenderTree separation | **PASS** | project-owned semantic-to-render projection tests |
| Native layout/paint | **PASS (automated)** | cosmic-text + tiny-skia, viewport culling, 100/150/200% geometry |
| Source mode | **PASS (automated)** | source projection remains authoritative editor projection |
| Preview mode | **PASS (automated)** | immediate refresh, read-only native surface |
| Split mode | **PASS (automated)** | fixed 50/50, 1000 ms debounce, independent scroll state |
| Preview selection/copy | **PASS (automated)** | row-bounded wrapped ranges, grapheme-safe hit test, headings/lists/code/table/HTML/math/image copy |
| Link safety | **PASS (automated)** | typed intent/effect plus independent allowlist validation inside the Windows adapter |
| Raw HTML safety | **PASS** | literal nodes only; no DOM/script/style execution path |
| Remote image no-network | **PASS** | placeholder/link projection; no HTTP client dependency |
| Math delimiter semantics | **PASS** | four Comrak delimiters; formula rendering intentionally deferred |
| Preview scheduling | **PASS** | immediate Preview, fixed 1000 ms Split debounce, stale drop |
| Worker bounds | **PASS** | one lazy worker; one in-flight + one latest pending; typed completion |
| Resource/robustness | **PASS** | 5 MiB/depth/node limits and 10,000 deterministic malformed cases |
| Preview performance | **PASS** | 20 KiB/100 KiB/1 MiB p95 below 100/400/2000 ms hard gates |
| Memory / idle CPU | **PASS (automated local)** | five-run Source/Preview/Split Windows receipt plus 60-second CPU interval recorded below |
| Visual/OS interaction | **CONDITIONAL — NOT TESTED** | formal current-commit manual matrix is still required |

Phase 5 establishes a native Preview foundation, not completed v1 rendering. Formula nodes are
source-preserving placeholders until the RaTeX phase; local images are placeholders until the
asset/image phase. Neither limitation changes Markdown semantics or canonical source.

## Preconditions and Environment

- Starting commit: `dfc893322c83b8f658e2ed1ad3b5907b482b2232`
- Phase 4: automated/portable slice accepted by USER; Phase 3 real IME and Phase 4 manual conditions
  remain open and unchanged.
- OS: Windows 11 Home Chinese, build 26200.
- CPU: Intel Core i7-12700H; RAM: 16,962,281,472 bytes; workspace filesystem: NTFS.
- Rust/Cargo: 1.97.1; Release profile: fat LTO, one codegen unit, abort panic, stripped symbols.

## Comrak and Cargo-Tree Evidence

| Item | Frozen value |
| --- | --- |
| Version | exact `0.54.0` |
| License | BSD-2-Clause |
| Declared Rust requirement | 1.85 |
| Default features | disabled |
| Enabled extensions | strikethrough, table, autolink, tasklist, math_dollars, math_latex |
| Heavy/unused features | CLI absent; syntect/onig/onig_sys absent |

The direct dependency is confined to `stickymd-render`. The locked Comrak subtree and its licenses,
including `finl_unicode`'s `Unicode-DFS-2016` data/software license, are recorded in
[`phase-05-dependency-delta.md`](phase-05-dependency-delta.md) and accepted explicitly by
`deny.toml`. `cargo deny check` passes; its existing duplicate-version notices remain warnings.

## Semantic Coverage

| Markdown form | Owned AST | RenderTree | Native Preview |
| --- | --- | --- | --- |
| Paragraph / heading | yes | yes | native text |
| Emphasis / strong / strike | yes | yes | styled native text |
| Quote | yes | yes | native block decoration |
| Ordered / unordered / nested list | yes | yes | native list text |
| Task item | yes | yes | read-only marker |
| Inline code / fenced code | yes | yes | monospace native text |
| Link | yes | typed action | native text + allowlisted activation |
| Image | yes | typed placeholder | placeholder; decode deferred |
| Table | yes | rows/cells/alignment | bounded native table layout |
| Thematic / soft / hard break | yes | yes | native separator/break |
| Raw HTML | literal | literal | literal text; never executed |
| Inline / display math | yes | placeholder | source-preserving placeholder |

Source-range tests cover ASCII, CJK, emoji and mixed multiline input using UTF-8 byte ranges.
Invalid positions and non-boundaries fail closed instead of being rounded into a different source
location.

## Architecture and Authority

```text
DocumentState (sole canonical authority)
  → explicit DocumentSnapshot
  → one lazy Preview worker
  → transient Comrak Arena
  → OwnedDocumentTree
  → RenderTree
  → LaidOutDocument + PreviewTextIndex
  → viewport PreviewFrame
```

| Question | Answer |
| --- | --- |
| Canonical text owner | `DocumentState` only |
| Parse input | immutable `DocumentSnapshot` |
| Comrak AST lifetime | one synchronous worker parse; Arena dropped before return |
| Owned AST owner/status | worker-owned immutable generation projection, parser-type free |
| RenderTree authority | non-authoritative rendering projection |
| Preview authority | read-only presentation projection |
| Can Preview write source? | no |
| Can raw HTML execute? | no |
| Can remote images fetch? | no |

- Arena, Comrak nodes and cosmic-text buffers never enter `AppState` or `DocumentState`;
  `SourceMap` and dialect `Options` are private to the parser adapter and expose no Comrak type in
  the render crate's public preview contract.
- Resize relayouts the retained RenderTree without parsing; scroll/selection only repaint.
- Preview selection has its own immutable clipboard projection and cannot emit `Edit`.
- Link activation is `PreviewIntent → PreviewCoordinator validation → PreviewEffect → Shell adapter`.
- `javascript:`, `data:` and all unapproved schemes remain visible semantic targets but never reach
  `ShellExecuteW`.
- Remote images are classified and displayed as alt/link placeholders; no HTTP client exists.

## Module Map

| Area | Responsibility |
| --- | --- |
| `stickymd-render/preview/parser.rs` | frozen dialect, transient Arena conversion, source limits |
| `model.rs` / `source_map.rs` | project-owned semantic nodes and UTF-8 ranges |
| `render_tree.rs` | rendering projection, placeholders and typed link actions |
| `layout.rs` / `table_layout.rs` | paragraph/block and table layout |
| `selection.rs` | clipboard text, row-indexed hit test and glyph rectangles |
| `paint.rs` / `pipeline.rs` | viewport-culling paint and generation-tagged pipeline |
| `stickymd-win/preview/worker.rs` | lazy bounded worker and typed completion |
| `flow/preview.rs` | debounce, stale admission and link/view intents |
| `app/{preview_runtime,preview_input}.rs` | shell presentation and read-only interaction |
| `platform/windows/shell.rs` | final allowlisted Windows Shell capability |

The former linear document-wide hover scan was replaced by immutable visual-row indexing:
hit-test cost is `O(log rows + spans in the selected row)` instead of `O(all spans)`.
Wrapped spans are clipped to each visual row's actual cosmic-text byte range, so a wrapped row no
longer claims the entire source span. Rendered copy inserts one `\n` between blocks.

## Semantic and Safety Evidence

- Raw HTML execution: none. JavaScript runtime: none. DOM/HTML renderer: none.
- Remote image HTTP fetch: none. Custom URI execution: none.
- Golden and targeted semantic fixtures cover heading, inline styles, ordered/unordered/nested/task
  lists, quote, breaks, thematic rule, GFM table, code info string, inline/display math, image, link
  and raw HTML.
- Four delimiters are emitted only by Comrak; code spans keep dollar text literal.
- Raw HTML source is preserved as `HtmlLiteral`; no DOM, CSS, script or HTML renderer dependency is
  present.
- Image placeholders show alt plus path; selection copies only alt text.
- Phase 5 table layout uses fixed bounded columns and cell soft-wrap, so tables cannot expand the
  whole preview. A dedicated nested horizontal table scroller remains a later v1 layout refinement.
- 10,000 fixed-seed malformed/mixed Unicode inputs parse without panic.
- Adversarial tests cover a 10,000-character code block, 100×20 table, 2,000 math nodes,
  excessive depth, more than 200,000 nodes and source over 5 MiB.

## Scheduling and Lifecycle

- Source mode does not construct a Preview worker.
- The worker uses the specified 512 KiB stack and owns its long-lived FontSystem/Swash cache.
- Preview entry and edits while Preview-only are immediate.
- Split edits reset a deterministic 1000 ms deadline; no snapshot is taken per keystroke.
- A deterministic 100-rapid-edit Split test emits zero early builds and exactly one build for the
  latest generation at the final deadline.
- The mailbox is bounded to one running job and one coalesced latest pending job.
- All completions keep their generation and typed `PreviewPipelineError`; stale success and failure
  cannot replace the current frame.
- Worker shutdown joins the single thread; no pool or async runtime exists.
- Instrumented pipeline evidence: one initial build followed by 100 resizes ends at
  `parses=1`, `render_tree_builds=1`, `layouts=101`; 1000 subsequent scroll paints leave those
  semantic counters unchanged and end at `paints=1101`.

## Release Performance

Each size has one cold end-to-end construction plus 20 warm repetitions after one shared
font/cache initialization. Warm values are median / p95 / max in milliseconds; total includes
parse, owned conversion, RenderTree, layout and viewport paint. The first warm layout may still
show a glyph-cache outlier in `max`; gates use warm p95.

| Size | Cold end-to-end |
| --- | ---: |
| 20 KiB | 283.678 ms |
| 100 KiB | 272.118 ms |
| 1 MiB | 1311.275 ms |

| Size | Comrak | Owned | RenderTree | Layout | Paint | Total |
| --- | --- | --- | --- | --- | --- | --- |
| 20 KiB | 0.216 / 0.252 / 0.269 | 0.032 / 0.056 / 0.068 | 0.009 / 0.010 / 0.028 | 9.495 / 10.621 / 199.749 | 2.251 / 2.442 / 4.061 | **12.136 / 13.240 / 204.206** |
| 100 KiB | 0.781 / 1.198 / 1.512 | 0.095 / 0.156 / 0.324 | 0.048 / 0.105 / 0.207 | 51.008 / 83.746 / 200.568 | 2.167 / 2.887 / 4.950 | **54.173 / 87.570 / 206.648** |
| 1 MiB | 10.955 / 13.624 / 14.859 | 1.057 / 1.344 / 1.947 | 0.635 / 1.344 / 2.078 | 686.269 / 804.857 / 1128.801 | 2.656 / 4.284 / 4.977 | **704.910 / 824.282 / 1148.332** |

All p95 values pass the 100/400/2000 ms hard gates. Measurements are local evidence, not a
cross-machine performance promise.

## Binary and Runtime Resource Evidence

| Measurement | Phase 4 | Phase 5 | Delta / status |
| --- | --- | --- | --- |
| Stripped Release EXE | 2,963,968 bytes | 3,495,424 bytes | +531,456 bytes (+0.507 MiB) |
| Source Private Working Set / Commit | prior evidence not current-commit comparable | 7,692,288 B / 8,572,928 B median | automated local receipt below |
| Preview 20 KiB Private Working Set / Commit | not comparable | 17,145,856 B / 18,382,848 B median | automated local receipt below |
| Split 20 KiB Private Working Set / Commit | not comparable | 18,120,704 B / 19,316,736 B median | automated local receipt below |
| Preview 100 KiB Private Working Set / Commit | not comparable | NOT TESTED | formal five-run receipt open |
| Preview 1 MiB Private Working Set / Commit | not comparable | NOT TESTED | formal five-run receipt open |

| 60-second idle CPU mode | Status |
| --- | --- |
| Source | PASS — 0.000000% |
| Preview | PASS — 0.000000% |
| Split | PASS — 0.000000% |

The copied-EXE lifecycle smoke does not close visual acceptance. The separate resource runner below
owns memory and idle-CPU acceptance as repeatable automated local evidence.

## Phase 6 Preflight Runtime Baseline

Measured during Phase 6 preflight, before any RaTeX production dependency or product-code change.
The checked-in Rust smoke entry copied the stripped Release executable into a distinct portable
directory for every run, seeded the same 20 KiB document with exactly 20 Comrak math nodes, waited
30 seconds without a debugger, and sampled each mode five times. Idle CPU is process kernel+user
time over 60 seconds divided by wall time and 20 logical processors. `Private Bytes` is the process
commit charge and `Private Working Set` comes from `PROCESS_MEMORY_COUNTERS_EX2`.

- Product commit under measurement: `c527c4a2e20cde29a33cb8dfcb0eabf0e7c58c68`.
- Windows 11 Home Chinese build 26200; Intel Core i7-12700H; 20 logical processors;
  15.8 GiB visible RAM; 2560×1440 at 96 DPI; Defender real-time protection was not running.
- Stable repeatable entry: `tools/smoke/phase-05.ps1 -Resources`.

| Mode | Private Working Set median | Private Working Set max | Private Bytes median | Private Bytes max | 60 s idle CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Source | 7,692,288 B (7.336 MiB) | 26,009,600 B (24.805 MiB) | 8,572,928 B (8.176 MiB) | 28,581,888 B (27.258 MiB) | 0.000000% |
| Preview | 17,145,856 B (16.352 MiB) | 17,240,064 B (16.441 MiB) | 18,382,848 B (17.531 MiB) | 18,460,672 B (17.605 MiB) | 0.000000% |
| Split | 18,120,704 B (17.281 MiB) | 18,161,664 B (17.320 MiB) | 19,316,736 B (18.422 MiB) | 19,333,120 B (18.438 MiB) | 0.000000% |

The Source samples had substantial OS working-set variability, so both median and maximum are kept
instead of selecting a favorable run. Preview and Split were stable. All maxima remain below their
exploratory hard limits; no CPU-time increase, persistent redraw, or monotonic memory growth was
observed. The Phase 6 preflight gate therefore passes. These are local-machine measurements, not
cross-machine product claims and not a substitute for visual or IME acceptance.

## Dependencies and Unsafe

- New direct dependency: exact `comrak 0.54.0`, BSD-2-Clause, default features disabled. Full audit:
  [`phase-05-dependency-delta.md`](phase-05-dependency-delta.md).
- No `syntect`, `onig`, network client, browser/DOM, Tokio, GPU framework, database or RaTeX runtime
  dependency was added.
- `stickymd-core` unsafe: 0.
- `stickymd-render` unsafe: 0.
- One new Windows adapter unsafe block calls `ShellExecuteW`; NUL-terminated UTF-16 lifetime and
  non-owned return status are documented adjacent to the block. The adapter independently
  reclassifies the destination, so a forged allowed `LinkKind` cannot execute a blocked scheme.

## Windows API Added

| API | Purpose | Unsafe |
| --- | --- | --- |
| `ShellExecuteW` | hand an independently allowlisted preview link to Windows Shell | yes; isolated in `platform/windows/shell.rs` with adjacent `SAFETY` invariant |

## Automated Verification Surface

- Stable entry: `tools/smoke/phase-05.ps1`.
- Rust owner: `stickymd-smoke phase 05`; CI calls the deduplicated `all --ci` graph.
- Headless tests: render semantics/layout/robustness + Windows flow/worker/input tests.
- Release gate: `phase5_preview_release_baseline` (20 repeats per size).
- Opt-in runtime: copied Release instances seeded separately with Preview and Split fixtures.
- Acceptance state: [`docs/acceptance-cases/phase-05.md`](../acceptance-cases/phase-05.md).

## Acceptance Status

| Acceptance case | Phase 5 status |
| --- | --- |
| AC-013 Markdown Preview | automated foundation PASS; visual/mouse/Shell matrix NOT TESTED |
| AC-014 Math Delimiters | delimiter semantics PASS; RaTeX formula rendering PENDING Phase 6 |
| AC-016 Raw HTML Safety | automated PASS |
| AC-017 Remote Image No Network | automated dependency/runtime-path safety PASS; live network observation NOT TESTED |

## Verification Receipt

| Command / stable entry | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `cargo build --workspace --release --locked` | PASS |
| core/render/Windows release tests | PASS |
| `cargo deny check` | PASS; duplicate-version notices only |
| `tools/smoke/phase-05.ps1 -Performance -Runtime` | PASS |
| `tools/smoke/all.ps1 -Ci` | PASS |
| `git diff --check` | PASS |

The final commands above are rerun after report synchronization before the Phase 5 commit.

## Architecture Drift

None. The initial direct Shell call from preview mouse handling was rejected during self-review and
replaced by a typed intent/effect boundary before this report.

## Open Conditions

- Windows 11 visual fidelity, mouse selection/copy, actual Shell activation, Light/Dark and DPI
  screenshots remain `NOT TESTED`.
- Phase 3 Microsoft Pinyin/WeChat IME and Phase 4 manual platform conditions remain inherited.
- RaTeX and decoded local images are intentionally outside Phase 5 and are not reported as defects
  in this foundation.

## Recommendation

**APPROVE Phase 6 WITH CONDITIONS**: retain every open manual item as `NOT TESTED`; Phase 6 may add
RaTeX rendering without weakening the owned-tree, worker, safety or performance contracts.
