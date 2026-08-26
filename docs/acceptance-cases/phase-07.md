# Phase 07 Acceptance Matrix

> Verification projection for managed-image ownership, clipboard image transactions, native local
> image preview, conservative lifecycle reconciliation and source-preserving Markdown export.
> Headless rows are owned by the checked-in Rust smoke graph. Real clipboard applications, visual
> quality and crash timing remain `NOT TESTED` until repeatable receipts are checked in. Process
> memory/idle CPU is a separate opt-in Rust CLI matrix; it never promotes visual/manual rows.

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P07-A01 | 08 managed ownership; AC-011 | Automated | strict 20/32/64 lowercase names, replaced-root identity rejection, `symlink_metadata` reparse fail-closed branch, full-content hash proof and proof-to-mutation write/delete exclusion tests | AUTOMATED PASS |
| P07-A02 | 08 collision safety | Automated | 20-hex hostile collision falls back to 32 hex without modifying the untrusted file | AUTOMATED PASS |
| P07-A03 | 08 conservative references | Automated | literals in image syntax, code and raw text; CJK edit-boundary and multi-reference transition tests | AUTOMATED PASS |
| P07-A04 | 08 clipboard preparation; AC-010 | Automated | PNG/JPEG/WebP/GIF preservation, real EXIF-oriented JPEG metadata+raster, BMP/ICO/DIB/raw RGBA normalization and corrupt/size guard tests | AUTOMATED PASS |
| P07-A05 | 08 clipboard ordering and bounded capture | Automated | adapter-level CF_HDROP → registered encoded formats → DIB/V5 → CF_BITMAP → text implementation, dimension guards and bounded busy retry; real sources remain manual | AUTOMATED PASS |
| P07-A06 | 08 paste transaction; AC-010 | Automated | worker-deferred image paste, 128 MiB cumulative batch bound, streaming store, generation OCC rejection, two-image atomic commit and failure-ledger convergence against latest refs | AUTOMATED PASS |
| P07-A07 | 08 undo/redo asset effects; AC-011 | Automated | private UndoEntry carries pure effects; text-first undo/redo, multi-reference and grouped reverse-order transaction tests prove 0↔1 transitions | AUTOMATED PASS |
| P07-A08 | 08 asset lifecycle; AC-011 | Automated | handle-bound ownership, root replacement/reparse rejection, full-digest duplicate/collision proof, runtime non-destructive reconcile and durable-fingerprint safe-boundary GC tests | AUTOMATED PASS |
| P07-A09 | 08 native image preview; AC-017 | Automated | standalone, mixed-text and table-cell local image rasters, aspect/no-upscale, alpha and selection-alt tests | AUTOMATED PASS |
| P07-A10 | 08 lazy/bounded preview | Automated | 100-image viewport-band test, live-layout raster accounting, 16 MiB plus 512-entry bounds, corrupt/missing isolation and Source cache release tests | AUTOMATED PASS |
| P07-A11 | 08 remote zero-network; AC-017 | Automated | remote image source is never invoked and dependency denylist contains no network client | AUTOMATED PASS |
| P07-A12 | 08 semantic export; AC-012 | Automated | only real Comrak image nodes are collected; code/raw/normal reference links and remote images are preserved | AUTOMATED PASS |
| P07-A13 | 08 export resources; AC-012 | Automated | relative/absolute/user local copies, byte dedup, 20→32 hash collision expansion, percent-safe paths, missing-resource fail-before-publish and existing-directory suffix tests | AUTOMATED PASS |
| P07-A14 | 08 source-preserving export; AC-012 | Automated | no-image byte identity, localized image rewrite, reference-style normalization and title/alt tests | AUTOMATED PASS |
| P07-A15 | 08 export isolation | Automated | immutable snapshot, exclusive owned temp, no-replace staging publication, handle-observed cleanup and canonical/hardlink working-note rejection prove export never mutates document authority or deletes a path replaced after ownership proof | AUTOMATED PASS |
| P07-A16 | 10 Phase 7 performance | Automated | 1 MiB full/incremental scan, 10-file paste, scaled decode and 20-reference export Release baselines via [`phase-07.ps1 -Performance`](../../tools/smoke/phase-07.ps1) | AUTOMATED PASS |
| P07-A17 | 11 runtime local-image smoke | Automated | copied Release local-image Preview process/source/user-file survival via [`phase-07.ps1 -Runtime`](../../tools/smoke/phase-07.ps1) | AUTOMATED PASS |
| P07-A18 | 11 governance/CI | Automated | Rust smoke Phase 07 route, all-phase CI, plan-ref/unsafe/dependency checks and locked workspace tests | AUTOMATED PASS |
| P07-A19 | 10 image process resources | Automated | copied Release no-image/lazy/1/12/4K/Preview+Split saturated-cache plus same-process Preview→Source release, five runs each and selected 60-second idle CPU via [`phase-07.ps1 -Resources`](../../tools/smoke/phase-07.ps1) | AUTOMATED PASS |
| P07-A20 | 08 lifecycle races | Automated | unique request identity, newer-request downgrade, quit-time input freeze, paste→latest-save→safe-GC decision sequence, already-running old-sync→latest-reference convergence, durable+runtime reference union, typed external-reload→projection+asset-reconcile effect and deferred-delete fallbacks | AUTOMATED PASS |
| P07-M01 | AC-010 real clipboard sources | Manual | Snipping Tool, Paint, Explorer PNG/JPEG and browser-copy format receipts required | NOT TESTED |
| P07-M02 | AC-010/011 real paste and history | Manual | Windowed paste → undo → redo → restart matrix with visible diagnostics required | NOT TESTED |
| P07-M03 | AC-017 local-image visual quality | Manual | alpha, wide/tall, missing/corrupt, 100/150/200% DPI and Split screenshots required | NOT TESTED |
| P07-M04 | AC-012 native export dialog | Manual | Ctrl+Shift+S dialog, dirty/conflict export and Explorer-opened output receipt required | NOT TESTED |
| P07-M05 | 10 image visual/cache behavior | Manual | visible long-document scrolling, placeholder-to-raster transitions and cache-pressure visual quality receipt required; process metrics are automated separately | NOT TESTED |
| P07-M06 | 08 crash reconciliation | Manual | real forced termination after asset write and after trash move, followed by restart receipt required | NOT TESTED |
| P07-M07 | inherited Phase 3 IME gate | Manual | Microsoft Pinyin and WeChat IME checklist remains open | NOT TESTED |
| P07-M08 | inherited Phase 5/6 visual gate | Manual | native Preview and RaTeX Light/Dark/DPI matrices remain open | NOT TESTED |
| P07-M09 | 08 real reparse boundary | Manual | actual Windows file symlink and directory-junction escape attempts require a suitable Developer Mode/admin test environment | NOT TESTED |

## Frozen Definition-of-Done Trace

Every checkbox in the frozen Phase 7 prompt is projected below. Broad acceptance rows above remain
the reusable behavior contracts; these rows prevent a broad label from hiding an individual frozen
obligation.

| ID | Frozen DoD obligation | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P07-D001 | USER approved Phase 7 | Automated | task precondition records the supplied Phase 7 prompt and starting commit | AUTOMATED PASS |
| P07-D002 | preserve Phase 6 inherited conditions | Automated | task/report and M07/M08 retain every inherited manual gate | AUTOMATED PASS |
| P07-D003 | bootstrap `images/` | Automated | A08/A17 runtime-path and copied-Release lifecycle coverage | AUTOMATED PASS |
| P07-D004 | bootstrap `.trash/` | Automated | A08/A17 runtime-path and copied-Release lifecycle coverage | AUTOMATED PASS |
| P07-D005 | implement managed asset identity | Automated | A01 strict `ManagedAssetName` value-object tests | AUTOMATED PASS |
| P07-D006 | hash final encoded bytes with SHA-256 | Automated | A01/A04 content-addressed storage and preserved/normalized codec fixtures | AUTOMATED PASS |
| P07-D007 | default to 20-hex filename | Automated | A01 content-addressed store tests | AUTOMATED PASS |
| P07-D008 | 32/64 collision fallback | Automated | A02/A13 hostile prefix-collision tests | AUTOMATED PASS |
| P07-D009 | canonical managed extension | Automated | A01 strict extension grammar | AUTOMATED PASS |
| P07-D010 | ownership proves path, name and content | Automated | A01/A08 root identity, exact-handle, grammar and digest tests | AUTOMATED PASS |
| P07-D011 | a basename alone cannot delete a user file | Automated | A01/A08 user and wrong-hash preservation tests | AUTOMATED PASS |
| P07-D012 | wrong-hash managed-looking file is untouched | Automated | A01/A08 wrong-hash reconcile fixture | AUTOMATED PASS |
| P07-D013 | real reparse/symlink boundary safety | Manual | implementation and replaced-root automation are A01; an actual Windows junction/symlink receipt is M09 | NOT TESTED |
| P07-D014 | preserve PNG encoding | Automated | A04 byte-preservation fixture | AUTOMATED PASS |
| P07-D015 | preserve JPEG encoding | Automated | A04 byte-preservation fixture | AUTOMATED PASS |
| P07-D016 | preserve WebP encoding | Automated | A04 byte-preservation fixture | AUTOMATED PASS |
| P07-D017 | preserve GIF encoding | Automated | A04 byte-preservation fixture | AUTOMATED PASS |
| P07-D018 | normalize screenshot pixels to PNG | Automated | A04 DIB/raw-RGBA PNG fixtures; real producers remain M01 | AUTOMATED PASS |
| P07-D019 | normalize other approved formats to PNG | Automated | A04 BMP/ICO fixtures | AUTOMATED PASS |
| P07-D020 | disable `image` default features | Automated | A18 dependency-manifest/graph checks | AUTOMATED PASS |
| P07-D021 | keep codec features minimal | Automated | A18 exact feature allowlist | AUTOMATED PASS |
| P07-D022 | exclude AVIF/EXR and unrelated heavy codecs | Automated | A18 dependency deny/feature checks | AUTOMATED PASS |
| P07-D023 | encoded-size guard | Automated | A04/A05 bounded encoded-input fixtures | AUTOMATED PASS |
| P07-D024 | dimension guard | Automated | A04/A05 metadata and bitmap pre-copy guards | AUTOMATED PASS |
| P07-D025 | pixel-count guard | Automated | A04 40 MP boundary fixtures | AUTOMATED PASS |
| P07-D026 | checked decoded-byte arithmetic | Automated | A04 overflow/bounds fixtures | AUTOMATED PASS |
| P07-D027 | Windows CF_HDROP adapter | Automated | A05 adapter code, count/classification tests and Windows compile; real Explorer remains M01 | AUTOMATED PASS |
| P07-D028 | native encoded clipboard path audit | Automated | A05 format-order implementation and dependency audit; real producers remain M01 | AUTOMATED PASS |
| P07-D029 | bitmap fallback | Automated | A04/A05 CF_BITMAP pre-guard plus RGBA preparation tests; real producers remain M01 | AUTOMATED PASS |
| P07-D030 | finite clipboard-busy retry | Automated | A05 fixed 0/10/30/80 ms schedule and bounded-attempt tests | AUTOMATED PASS |
| P07-D031 | file read/encode stays off UI thread | Automated | A06 descriptor-only capture and worker-owned preparation tests | AUTOMATED PASS |
| P07-D032 | paste generation OCC | Automated | A06 stale-generation commit rejection | AUTOMATED PASS |
| P07-D033 | persist assets before Markdown insertion | Automated | A06 worker completion then one coordinator delta | AUTOMATED PASS |
| P07-D034 | failed paste leaves Document unchanged | Automated | A06 failure and generation-race fixtures | AUTOMATED PASS |
| P07-D035 | multi-image paste is all-or-nothing | Automated | A06 two-image atomic commit plus second-file failure ledger | AUTOMATED PASS |
| P07-D036 | managed Markdown paths use `/` | Automated | A06 exact generated Markdown assertions | AUTOMATED PASS |
| P07-D037 | conservative reference scanner | Automated | A03 code/raw/malformed-literal fixtures | AUTOMATED PASS |
| P07-D038 | multi-reference count correctness | Automated | A03/A07 multi-reference transition tests | AUTOMATED PASS |
| P07-D039 | final `1→0` moves logically to trash | Automated | A07/A08 transition tests | AUTOMATED PASS |
| P07-D040 | `0→1` restores active asset | Automated | A07/A08 transition tests | AUTOMATED PASS |
| P07-D041 | UndoEntry carries pure asset effects | Automated | A07 private-history tests and core boundary scan | AUTOMATED PASS |
| P07-D042 | core performs no filesystem I/O | Automated | A18 forbidden-import/direct-I/O scan | AUTOMATED PASS |
| P07-D043 | undo restores asset state | Automated | A07 text-first reverse-order undo tests | AUTOMATED PASS |
| P07-D044 | redo trashes asset state | Automated | A07 text-first redo tests | AUTOMATED PASS |
| P07-D045 | asset failure never rolls back correct text | Automated | A06/A07 failure-ledger convergence tests | AUTOMATED PASS |
| P07-D046 | I/O requests remain bounded | Automated | A06/A20 single-slot coalescing/mailbox tests | AUTOMATED PASS |
| P07-D047 | GC cannot starve note save | Automated | A20 worker priority and save/quit ordering tests | AUTOMATED PASS |
| P07-D048 | image directory events do not become note conflicts | Automated | A20 watcher scope and external-note routing tests | AUTOMATED PASS |
| P07-D049 | native local-image preview | Automated | A09 standalone/mixed/table raster tests; visual quality remains M03 | AUTOMATED PASS |
| P07-D050 | remote image path remains zero-network | Automated | A11 source-dispatch and dependency-deny tests | AUTOMATED PASS |
| P07-D051 | inspect image metadata before decode | Automated | A04/A09 format and real EXIF-JPEG fixtures | AUTOMATED PASS |
| P07-D052 | decode lazily | Automated | A10 off-band 100-image fixture | AUTOMATED PASS |
| P07-D053 | viewport prefetch band | Automated | A10 viewport +300 DIP assertions | AUTOMATED PASS |
| P07-D054 | decoded cache stays at or below 16 MiB | Automated | A10 live-lease accounting and saturation tests | AUTOMATED PASS |
| P07-D055 | image cache has entry and byte bounds | Automated | A10 512-entry and 16 MiB tests | AUTOMATED PASS |
| P07-D056 | animation policy is first-frame-only | Automated | A04/A09 GIF metadata/decode contract | AUTOMATED PASS |
| P07-D057 | preserve transparent alpha | Automated | A09 alpha-pixel paint fixture | AUTOMATED PASS |
| P07-D058 | preserve aspect ratio | Automated | A09 layout dimension fixtures | AUTOMATED PASS |
| P07-D059 | default no-upscale behavior | Automated | A09 standalone-image layout fixture | AUTOMATED PASS |
| P07-D060 | missing image is a local fallback | Automated | A09/A10 missing-source isolation test | AUTOMATED PASS |
| P07-D061 | corrupt image is a local fallback | Automated | A09/A10 corrupt-source isolation test | AUTOMATED PASS |
| P07-D062 | image failure cannot fail whole Preview | Automated | A09/A10 mixed valid/corrupt layout test | AUTOMATED PASS |
| P07-D063 | recovery/load precedes asset reconcile | Automated | A08/A20 startup-state and mutation-gate tests | AUTOMATED PASS |
| P07-D064 | referenced trash is restored | Automated | A08 reconcile fixture | AUTOMATED PASS |
| P07-D065 | unreferenced active asset moves to trash | Automated | A08 reconcile fixture | AUTOMATED PASS |
| P07-D066 | unreferenced trash deletes only at safe boundary | Automated | A08 durable-hash/handle/union/defer tests | AUTOMATED PASS |
| P07-D067 | user active file is never deleted | Automated | A08 user-file fixture | AUTOMATED PASS |
| P07-D068 | user trash file is never deleted | Automated | A08 user-trash fixture | AUTOMATED PASS |
| P07-D069 | corrupt managed-looking file is never deleted | Automated | A08 wrong-hash fixture | AUTOMATED PASS |
| P07-D070 | failed final save forbids destructive exit GC | Automated | A20 exit-GC failure cancellation and required-write decision tests | AUTOMATED PASS |
| P07-D071 | normal exit performs guarded safe GC | Automated | A08/A20 paste-in-flight→latest-save→quiescent safe-GC sequence, input-freeze and request-identity tests | AUTOMATED PASS |
| P07-D072 | real crash is followed by startup reconcile | Manual | deterministic startup logic is A08; real forced-process timing receipt is M06 | NOT TESTED |
| P07-D073 | real Ctrl+Shift+S native export flow | Manual | production intent/dialog wiring is compiled; end-to-end native dialog receipt is M04 | NOT TESTED |
| P07-D074 | UI labels the action `导出` | Automated | native dialog title and confirmation-label compile path | AUTOMATED PASS |
| P07-D075 | Export never changes working path | Automated | A15 canonical/hardlink target rejection and authority tests | AUTOMATED PASS |
| P07-D076 | Export snapshots current runtime text | Automated | A15 immutable generation-tagged snapshot tests | AUTOMATED PASS |
| P07-D077 | dirty document can export | Automated | A15 snapshot export independent of dirty/save state | AUTOMATED PASS |
| P07-D078 | conflict exports local runtime text | Automated | A15 conflict-independent local snapshot route | AUTOMATED PASS |
| P07-D079 | Export never downloads remote images | Automated | A11/A12 remote preservation tests | AUTOMATED PASS |
| P07-D080 | only semantic image nodes export | Automated | A12 Comrak occurrence tests | AUTOMATED PASS |
| P07-D081 | copy referenced managed local image | Automated | A13 local managed fixture | AUTOMATED PASS |
| P07-D082 | copy referenced user local image | Automated | A13 user-image fixture | AUTOMATED PASS |
| P07-D083 | copy referenced absolute external image | Automated | A13 absolute-path fixture | AUTOMATED PASS |
| P07-D084 | deduplicate identical export bytes | Automated | A13 shared-content fixture | AUTOMATED PASS |
| P07-D085 | missing local resource fails before publish | Automated | A13 fail-before-Markdown fixture | AUTOMATED PASS |
| P07-D086 | asset directory never overwrites an existing directory | Automated | A13 create-only publication tests | AUTOMATED PASS |
| P07-D087 | automatically suffix occupied asset directory | Automated | A13 suffix selection fixture | AUTOMATED PASS |
| P07-D088 | source-preserving Markdown rewrite | Automated | A14 localized source-range rewrite fixtures | AUTOMATED PASS |
| P07-D089 | reference-style images export | Automated | A14 reference normalization fixture | AUTOMATED PASS |
| P07-D090 | normal links are not image-rewritten | Automated | A12/A14 normal-reference-link fixture | AUTOMATED PASS |
| P07-D091 | export uses staging | Automated | A13/A15 staged publication tests | AUTOMATED PASS |
| P07-D092 | failure cleanup touches only invocation-owned files | Automated | A15 exclusive temp and non-recursive cleanup fixtures | AUTOMATED PASS |
| P07-D093 | successful export does not change generation | Automated | A15 authority snapshot test | AUTOMATED PASS |
| P07-D094 | successful export does not change dirty state | Automated | A15 authority snapshot test | AUTOMATED PASS |
| P07-D095 | successful export does not change Undo | Automated | A15 authority snapshot test | AUTOMATED PASS |
| P07-D096 | 1 MiB scanner benchmark | Automated | A16 Release median/p95/max receipt | AUTOMATED PASS |
| P07-D097 | image-paste benchmark | Automated | A16 ten-file staged Release receipt | AUTOMATED PASS |
| P07-D098 | decode benchmark | Automated | A16 inspect/decode-resize/cache Release receipt | AUTOMATED PASS |
| P07-D099 | export benchmark | Automated | A16 20-reference Release receipt | AUTOMATED PASS |
| P07-D100 | image-cache process-memory matrix | Automated | A19 no-image/small/saturated/Source-after-Preview five-run matrix | AUTOMATED PASS |
| P07-D101 | 4K image steady and peak memory | Automated | A19 five-run 4K receipt | AUTOMATED PASS |
| P07-D102 | selected 60-second idle CPU | Automated | A19 Source/Preview/Split/saturated/after-release receipt | AUTOMATED PASS |
| P07-D103 | binary delta | Automated | dependency report and Release artifact byte count | AUTOMATED PASS |
| P07-D104 | advance AC-010 | Automated | A04-A06 automated transaction core; real producers remain M01/M02 | AUTOMATED PASS |
| P07-D105 | advance AC-011 | Automated | A03/A07/A08 lifecycle transaction core; real visible/crash rows remain M02/M06 | AUTOMATED PASS |
| P07-D106 | complete AC-012 end to end | Manual | export core is A12-A15; native-dialog/user-opened receipt remains M04 | NOT TESTED |
| P07-D107 | keep AC-017 passing | Automated | A09-A11 native/remote isolation tests; visual receipt remains M03 | AUTOMATED PASS |
| P07-D108 | advance AC-018 | Automated | A08/A20 startup/exit reconciliation core; real crash remains M06 | AUTOMATED PASS |
| P07-D109 | core unsafe remains zero | Automated | A18 unsafe scan and crate attribute | AUTOMATED PASS |
| P07-D110 | render unsafe remains zero | Automated | A18 unsafe scan and crate attribute | AUTOMATED PASS |
| P07-D111 | every Windows unsafe block has SAFETY text | Automated | A18 source-policy scan plus clippy | AUTOMATED PASS |
| P07-D112 | no network client | Automated | A11/A18 dependency denylist | AUTOMATED PASS |
| P07-D113 | no WebView/browser runtime | Automated | A18 dependency/source denylist | AUTOMATED PASS |
| P07-D114 | no generic attachment system | Automated | architecture/module and dependency audit | AUTOMATED PASS |
| P07-D115 | documentation updated | Automated | governance link/required-artifact checks | AUTOMATED PASS |
| P07-D116 | Phase 7 task completed | Automated | task status and result sections | AUTOMATED PASS |
| P07-D117 | Phase 7 report completed | Automated | report required-section and link checks | AUTOMATED PASS |
| P07-D118 | all headless smoke/CI graph passes | Automated | A18 `stickymd-smoke all --ci` receipt | AUTOMATED PASS |
| P07-D119 | manual items are completed or honestly open | Automated | M01-M09 are explicitly `NOT TESTED`; matrix policy rejects unsupported pass claims | AUTOMATED PASS |
| P07-D120 | Phase 8 was not started automatically | Automated | repository phase/task/working-tree scope audit | AUTOMATED PASS |

## Current Phase Gate

The Rust CLI owns all headless Phase 7 checks. Passing them does not promote real Windows
clipboard applications, visual quality, crash timing, native-dialog interaction or IME; those rows
deliberately remain `NOT TESTED`. The separate process-resource row is automated and reports only
memory/CPU counters, never visual correctness.
