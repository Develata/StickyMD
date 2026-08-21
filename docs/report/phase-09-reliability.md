# Phase 9 Reliability and Failure Convergence

## Status

Automated convergence was rerun on 2026-08-21 against the measured Phase 9 convergence tree. The
exact clean package commit is recorded only after the implementation tree is committed and rebuilt.
Real permission-policy, hard-kill and GUI observations remain in the manual acceptance report and
are not upgraded by this report.

## Data Authority

- Canonical runtime text remains `DocumentState`.
- Persistence accepts an immutable generation-tagged snapshot and cannot mutate the document.
- Watch notifications remain hints; the final guarded fingerprint check is immediately before the
  selected create-new or replace primitive.
- Asset physical deletion remains behind a matching durable-note fingerprint and a restrictive open
  note handle. Any uncertainty degrades to non-destructive reconciliation.

## Failure Matrix

| Case | Result | Automated evidence |
| --- | --- | --- |
| temp create/write/flush/before-replace failure | PASS | `failure_stages_never_truncate_the_canonical_note` plus `phase9_disk_full_injection_before_write_preserves_canonical_note` |
| `ReplaceFileW` 1175/1176/1177 classification | PASS | `rare_replace_failures_are_not_collapsed`; no blanket fallback exists |
| recovery evidence | PASS | bootstrap invalid/same/newer/missing/oversize temporary tests |
| OCC external write race | PASS | `guarded_save_detects_external_change_and_never_writes`, `first_publish_race_after_guard_is_reported_as_conflict_without_overwrite` |
| watcher unavailable | PARTIAL | guarded storage tests prove the correctness fallback; watcher-factory/runtime degraded-mode coordination is not injected end to end |
| corrupt/newer config | PASS | config preservation/default/persistence-eligibility tests |
| invalid UTF-8 note | PASS | core durable decode plus startup canonical rejection tests |
| read-only note | PASS | `phase9_read_only_note_save_failure_preserves_dirty_document_authority` |
| unwritable program directory | PASS (fault seam) | `phase9_writable_probe_failure_is_typed_and_preserves_existing_file`; real ACL case remains manual |
| `note/` deleted during runtime | PARTIAL | storage repair is covered; the full watcher hint to coordinator to durable acknowledgement chain is not injected end to end |
| `note/` replaced by file | PASS | `phase9_note_directory_replaced_by_file_fails_closed` |
| disk full | PASS (fault injection) | Win32 error 112 is injected before temporary write; canonical bytes remain unchanged |
| save failure + Quit | PASS | `note_save_failure_cancels_quit_and_reenables_input` |
| save failure + HideToTray | PASS | `dirty_close_save_failure_keeps_window_visible_and_reenables_input` |
| conflict + Tray Quit | PASS | `phase9_conflict_blocks_tray_quit_before_any_destructive_barrier` |
| recovery + assets | PASS | recovery input freeze, recovery cleanup, safe-boundary and durable/runtime reference-union tests |
| final note save failure before GC | PASS | quit reducer cannot advance to asset GC after failed note barrier |
| canonical hard-link export | PASS | `export_rejects_the_working_note_and_hard_link_alias_without_modifying_either` |
| captured export snapshot | PASS | `phase7_export_snapshot_cannot_mutate_document_authority_or_history` |
| remote image | PASS | remote images never call the image source loader; no runtime HTTP client dependency |
| raw HTML | PASS | Comrak raw HTML is projected as `HtmlLiteral`; copy/selection tests preserve literal source |
| custom URI | PASS | parser, flow and Windows adapter independently reject blocked schemes |
| malformed math | PASS | malformed/oversized math and 10,000-input deterministic tests isolate errors |
| corrupt/huge/overflow image | PASS | encoded-size, dimensions, decoded-length and corrupt decode tests fail before unsafe allocation |

## Runtime Directory Repair

`persist_note` now owns one narrow policy helper: it may recreate a missing plain `note/` parent,
because external deletion must not clear the in-memory note. The helper does not create asset roots,
does not accept a reparse point, and does not turn the generic atomic publisher into a directory
bootstrap framework. A file or reparse point at `note/` fails closed and preserves the external
object.

## 4K BMP Transient Memory Review

The previous preview path held the complete encoded BMP allocation while the decoder allocated its
full decoded image and the scaled output. The cache-miss path now transfers ownership of the encoded
bytes into `decode_scaled_image_owned`, drops that buffer immediately after decode, and only then
allocates the resized raster. It does not add unsafe code, a custom codec, a GPU path or another
cache.

Targeted copied-Release `preview-4k-image` resource smoke, five runs:

| Metric | Phase 7 maximum | Phase 9 maximum | Delta |
| --- | ---: | ---: | ---: |
| Peak working set | 93.93 MiB | 83.46 MiB | -10.47 MiB |
| Peak private bytes | 79.93 MiB | 65.33 MiB | -14.60 MiB |
| Steady working set | 16.78 MiB | 19.35 MiB | +2.57 MiB |
| Steady private bytes | 17.84 MiB | 20.92 MiB | +3.08 MiB |

The change removes a large overlapping allocation while leaving steady state and cache ownership
unchanged. The stable increase is retained as measured evidence; it remains far below the normal
Preview hard gate and the transient is bounded.

## Fresh Verification

Latest full Phase 9 performance invocation included `cargo test --workspace --locked`:

- 357 passed at the final review tree;
- 0 failed;
- 12 ignored Release-only performance tests;
- ignored tests are executed by the explicit performance smoke route.

Fresh final baseline results are recorded after the review fixes; the exact package remains blocked
until those commands are rerun.

## Remaining Manual Evidence

- real ACL-denied program directory;
- real process hard-kill timing around save;
- copied-RC recovery UI and tray error presentation;
- Clean Windows 11 VM.

These remain `NOT TESTED`; their absence is a release-readiness blocker unless the USER supplies
evidence or explicitly waives the applicable gate.
