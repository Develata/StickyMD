# Phase 4 Portable Persistence Report

## Executive Result

| Capability | Result | Evidence boundary |
| --- | --- | --- |
| Program Directory Identity | **PASS** | canonical path, native UTF-16 key, dot/case/Unicode and 100-directory tests |
| Single Instance | **PASS** | named mutex/event integration tests and copied Release EXE smoke |
| Writable Validation | **PASS** | create/write/flush/delete probe; no fallback |
| UTF-8 / Line Ending | **PASS** | BOM, invalid UTF-8, mixed newline and isolated CR tests |
| Atomic Persistence | **PASS** | fixed same-directory temp, `FlushFileBuffers`, disposition-bound publish, failure stages and OCC race tests |
| Autosave | **PASS** | deterministic 650 ms tests, 1000 edits → one action, one in-flight + one latest pending |
| Recovery | **CONDITIONAL** | classification, quarantine, guarded restore and UI path pass; deterministic kill during the replace interval is not tested |
| External Clean Reload | **PASS** | pure decision tests, native watcher test, portable UI/file injection receipt |
| External Conflict | **PASS** | guarded conflict, Load External, Keep Local, invalid UTF-8 and deletion receipts |
| Config | **PASS** | v1 defaults/unknown/corrupt/newer/atomic roundtrip tests and overwrite suppression |
| Persistence Performance | **PASS** | 1 MiB end-to-end save p95 9.446 ms on the recorded NVMe environment |

## Repository Baseline

- Starting branch: `main`
- Starting commit: `bba362984be3a62e60aa03a659c332b6125d5137`
- Starting worktree: clean
- Phase 3 recommendation: `STOP — architecture review required` because real IME acceptance was
  not run. USER explicitly authorized the independent Phase 4 slice; this did not close Phase 3.

## Inherited Conditions

- Microsoft Pinyin: **NOT TESTED**.
- WeChat Input Method: **NOT TESTED**.
- Candidate positioning at 100/150/200% DPI: **NOT TESTED**.

## Environment

| Item | Value |
| --- | --- |
| OS | Windows 11 Home Chinese, 10.0.26200 build 26200 |
| CPU | Intel Core i7-12700H, 20 logical processors |
| RAM | 15.8 GiB |
| GPU | NVIDIA RTX 3060 Laptop + virtual display adapter |
| Display | Lenovo DisplayHDR 2560×1440; DPI scale not independently recorded |
| Rust / Cargo | 1.97.1 / 1.97.1, `x86_64-pc-windows-msvc` |
| Disk | ZHITAI TiPlus7100 1TB NVMe, NTFS |
| Defender | disabled according to `Get-MpComputerStatus` during measurement |
| Source commit | starting commit above plus this uncommitted Phase 4 worktree at measurement time |

## Final Module Map

- `stickymd-core::persistence`: durable byte decode/hash, external facts, recovery facts; no I/O.
- `stickymd-core::DocumentState`: sole canonical text owner and stale-safe persistence acknowledgement.
- `stickymd-win::flow::{save,persistence,reconciliation,recovery}`: deterministic coordination state.
- `stickymd-win::persistence::{storage,worker}`: guarded full-note I/O and bounded serialized execution.
- `stickymd-win::startup`: ordered bootstrap and recovery inspection.
- `stickymd-win::config`: config v1 DTO and independent atomic storage.
- `stickymd-win::app::*_runtime`: thin typed-intent/result presentation bridges.
- `stickymd-win::platform::windows`: canonical program directory, file identity, atomic publish,
  watcher, mutex/event and startup message box adapters.

## Runtime Paths

```text
program_dir = canonical parent of current_exe()
note_dir    = <program-dir>/note/
note_file   = <program-dir>/note/note.md
note_tmp    = <program-dir>/note/note.md.tmp
config      = <program-dir>/note/config.toml
config_tmp  = <program-dir>/note/config.toml.tmp
```

`images/` and `.trash/` are created as frozen ownership boundaries, but no asset behavior was added.

## Startup Pipeline

1. Resolve and canonicalize Program Directory.
2. Acquire same-directory named mutex or signal the existing instance and exit.
3. Create/write/flush/delete the program-directory probe.
4. Ensure the frozen `note/`, `images/`, `.trash/` layout.
5. Load/preserve config v1.
6. Inspect canonical and temporary note bytes with the 16 MiB engineering guard.
7. Quarantine unusable fixed-temp evidence or enter explicit recovery.
8. Safely create/load `note.md` and construct `DocumentState`.
9. Start the source shell and then the non-recursive watcher; enable autosave only after recovery.

## Single Instance Evidence

- Same directory: second copied Release EXE exited `0`, first remained alive, and neither
  `note.md` nor `config.toml` mtime changed.
- Different directories: two copied Release EXEs remained alive with independent runtime layouts.
- Wake path: the second instance signals the primary named event; the primary forwards only
  `ShowRequested` to the event loop.
- 100 distinct canonical directories produced 100 distinct SHA-256 instance identities.

## Encoding

- Load accepts UTF-8 with or without BOM; save emits UTF-8 without BOM.
- Runtime text uses `\n`; CRLF/LF majority is detected, ties and empty files use CRLF.
- Isolated `\r` is preserved.
- Invalid UTF-8 and over-limit external content never enter canonical text and are never
  automatically overwritten.
- Durable fingerprint hashes exact durable bytes, including BOM/newline differences.

## Atomic Persistence

1. Encode snapshot in the worker.
2. Create/truncate fixed same-directory temp only.
3. Write all bytes, Rust flush, then `FlushFileBuffers(temp handle)`.
4. Perform the final expected Durable Fingerprint check.
5. Bind the observed publish disposition:
   - existing target → `ReplaceFileW(..., flags=0)`;
   - missing target → `MoveFileExW(..., MOVEFILE_WRITE_THROUGH)` without replace.
6. Never blanket-fallback after `ReplaceFileW` failure.

`REPLACEFILE_WRITE_THROUGH` is not used. Errors 1175, 1176 and 1177 are separately classified;
filesystem evidence is preserved and no second mutation is attempted. This substantially prevents
half-written canonical files but does not claim absolute power-loss transactions on every device.

## Atomic Failure Tests

| Case | Result | Canonical outcome |
| --- | --- | --- |
| Before temp create | PASS | old complete |
| After temp write | PASS | old complete; temp may contain new complete |
| After `FlushFileBuffers` | PASS | old complete; recoverable new temp |
| Before replace | PASS | old complete; recoverable new temp |
| Create-new target appears after guard | PASS | external target untouched; local temp preserved; conflict |
| Guarded base mismatch | PASS | external target untouched; conflict |
| ForceOverwrite | PASS | used only by explicit Keep Local |
| 1175 / 1176 / 1177 classification | PASS | distinct fail-closed kinds |
| Real 100-cycle replacement | PASS | only complete new bytes observed |
| Rare live 1175/1176/1177 induction | NOT TESTED | deterministic unit classification only |

## Autosave and Save Queue

- Debounce: 650 ms; another edit resets the deadline.
- Snapshot: only when the deadline or immediate intent fires, never per keypress.
- Immediate triggers: Ctrl+S, focus loss and shutdown.
- Bound: one executing note request plus one replaceable latest pending request.
- Completion barrier: a following save cannot run against the pre-ack base fingerprint. Pending stale
  requests are dropped and the coordinator resubmits the latest snapshot after acknowledging receipt.
- 1000 continuous simulated edits produced one autosave action; 1000 direct mailbox submissions
  retained one latest pending job and counted 999 coalesces.
- Worker shutdown is joined, not detached.

## Optimistic Concurrency

Normal save carries the current base Durable Fingerprint. The worker re-reads/hash-checks the target
after temp flush and immediately before publish. A mismatch returns conflict without writing. A
missing target uses a create-only publish disposition, so a target appearing after the check also
fails closed. `ForceOverwrite` exists only behind explicit USER Keep Local resolution.

The final check-to-publish TOCTOU interval cannot be fully eliminated without a long-lived exclusive
lock, which would contradict supported external editing; this boundary is documented rather than hidden.

## Recovery Evidence

- equal canonical/temp hash → canonical load, then redundant temp cleanup;
- different valid temp → explicit F6 restore / F7 canonical choice;
- missing canonical + valid temp → explicit recovery candidate;
- invalid/oversized temp → uniquely quarantined before the fixed transaction temp can be reused;
- invalid/oversized canonical + valid temp → preserve canonical before guarded recovery publish;
- restore text remains dirty until a real successful publish acknowledgement;
- canonical mutation while the choice UI is waiting causes guarded conflict, never recovery overwrite;
- recovery-pending IME preedit/commit and ordinary editing are rejected;
- force-kill/relaunch recovery path was exercised with manually staged complete temp evidence.

Deterministically killing the process specifically between temp flush and replace remains **NOT TESTED**.

## External Change

### Clean Reload

**PASS.** Fresh validated bytes replace canonical state through the coordinator, clear Undo/Redo,
refresh the source projection and update the Durable Fingerprint.

### Dirty Conflict

**PASS.** Conflict is a first-class state and pauses normal autosave. Later observations replace the
external fact; current local editing remains allowed.

### Load External

**PASS.** Loads the latest valid external fact, clears history, resyncs projection and releases conflict.

### Keep Local

**PASS.** Captures the latest canonical generation and explicitly force-publishes; failure keeps conflict.

### External Delete

**PASS.** Never clears memory; it records a required guarded recreate. Quit cannot bypass that write.

### Invalid UTF-8 / Too Large

**PASS.** Neither is loaded. Even a clean document enters a protected conflict/error state, so close
cannot discard the last valid in-memory text. Only explicit Keep Local may overwrite it.

Portable UI/file-injection receipts exercised clean reload, dirty conflict, Load External, Keep Local,
invalid UTF-8 preservation and deletion recreation. The same flow was not repeated through the actual
Notepad UI and remains a manual condition.

## File Watch

- crate: `notify 8.2.0`, Windows native recommended backend;
- scope: non-recursive `note/`, relevant only to `note.md` / `note.md.tmp`;
- callback: one atomic pending gate before EventLoopProxy, preventing raw-event queue storms;
- flow debounce: 150 ms, then a fresh bounded read/hash;
- self writes: ignored by exact Durable Fingerprint, never by a blind time window;
- read failures: bounded 50/150/300 ms retries plus final attempt;
- degraded mode: visible warning; guarded OCC remains the correctness gate.

## Config

- schema version 1, frozen fields and defaults;
- missing fields default; unknown fields ignored; invalid enum/value is corrupt;
- corrupt config is renamed to `config.invalid-<timestamp>.toml` before defaults are publishable;
- unreadable, unpreserved-corrupt, or newer-version config suppresses automatic writes, including close;
- config uses an independent atomic transaction and cannot block note persistence.

## Acceptance

| Case | Result | Evidence |
| --- | --- | --- |
| AC-001 Portable First Launch | PASS | copied Release EXE created independent frozen layouts |
| AC-005 Autosave | PASS | deterministic scheduler + portable typing receipt |
| AC-006 Manual Save | PASS | Ctrl+S portable receipt |
| AC-007 External Clean Reload | PASS | watcher integration + portable file-injection receipt |
| AC-008 External Dirty Conflict | PASS | OCC tests + F6/F7 portable receipt |
| AC-026 Same Directory Single Instance | PASS | integration + copied Release EXE, mtime unchanged |
| AC-027 Different Directory Multi Instance | PASS | integration + two copied Release EXEs |
| AC-030 Crash Recovery | CONDITIONAL | classification/restore and force-kill relaunch pass; kill-during-publish not tested |

## Performance

Values are median / p95 / max from 40 Release iterations, microseconds, on the environment above.

| Size | Snapshot | Encode | Hash | Write + flush | Replace | End-to-end |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 2 / 16 / 22 | 26 / 35 / 35 | 14 / 16 / 16 | 3159 / 3410 / 6783 | 1938 / 2985 / 3577 | 5125 / 6873 / 8719 |
| 100 KiB | 40 / 77 / 80 | 158 / 202 / 259 | 70 / 92 / 156 | 3166 / 3350 / 3355 | 1943 / 2290 / 2438 | 5443 / 5909 / 6131 |
| 1 MiB | 337 / 388 / 435 | 1529 / 1892 / 2001 | 698 / 822 / 933 | 4074 / 4482 / 4531 | 2053 / 2547 / 2562 | 8740 / 9446 / 9603 |

The UI thread only updates deadlines, constructs an explicit snapshot when due and sends a bounded
request. Encoding/hash/file operations execute on the I/O worker.

## Memory and Idle CPU

| Measurement | Phase 3 | Phase 4 copied Release | Delta |
| --- | ---: | ---: | ---: |
| Source shell Working Set | 31.57 MiB median | 32.691 MiB observed | +1.121 MiB |
| Phase 4 Private Bytes | not recorded in the same Phase 3 run | 12.363 MiB | n/a |
| Portable EXE | n/a | 2,963,968 bytes | n/a |

After 20 seconds settle, one focused instance consumed 31.25 ms process CPU over a 10-second window;
on 20 logical processors this is approximately 0.016% machine-normalized. A second unfocused instance
recorded 0 ms in that interval. Working Set is allocator/OS-sensitive and is reported as local evidence,
not a universal product promise.

## Dependencies Added

| Crate | Resolved | License | Purpose | Boundary |
| --- | --- | --- | --- | --- |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | exact-byte SHA-256 | platform-independent core value capability |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | config DTO derive | Windows app only |
| `toml` | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 | config v1 | Windows app only |
| `notify` | 8.2.0 | CC0-1.0 | native directory hints | Windows app only, defaults disabled |
| `windows` | 0.62.2 | MIT OR Apache-2.0 | narrow Win32 adapters | Windows target only |

No Tokio/async runtime, network client, database, WebView, Tauri or GPU UI framework was introduced.

## Windows APIs Added

| API | Purpose | Unsafe |
| --- | --- | --- |
| `FlushFileBuffers` | durable temp flush before publish | yes, scoped adapter |
| `ReplaceFileW` | replace existing canonical/config target with flags=0 | yes, scoped adapter |
| `MoveFileExW` | create-only new target with WRITE_THROUGH | yes, scoped adapter |
| `GetFileInformationByHandle` | stable identity/size/write observation of open file | yes, scoped adapter |
| `CreateMutexW` / `CreateEventW` / `SetEvent` | per-directory instance and wake | yes, scoped adapter |
| `WaitForMultipleObjects` / `CloseHandle` | listener lifecycle and owned handles | yes, scoped adapter |
| `MessageBoxW` | pre-window fatal startup message | yes, scoped adapter |

Every unsafe block is in `platform/windows/` and has an adjacent pointer/handle/lifetime `SAFETY`
invariant. `stickymd-core` and `stickymd-render` unsafe runtime code: **0**.

## Architecture Authority

- Canonical text owner: `DocumentState` on the main coordination thread.
- Durable representation: exact `note.md` bytes plus Durable Fingerprint.
- External fact entry: watcher hint → worker bounded read/hash → typed main-thread reconciliation.
- Save source: immutable `DocumentSnapshot`, never cosmic-text/SourceProjection.
- Watcher authority: none; it is only a coalesced hint. Guarded OCC remains correct without it.
- Worker authority: none over runtime state; it executes immutable requests and returns typed receipts.

## Architecture Drift

None. Review-driven fixes removed shell-owned persistence decisions, stale-save completion races,
create-new overwrite races, recovery evidence truncation, conflict-exit bypass, config overwrite on
protected states and unbounded watcher hints. Production shell lifecycle, presentation and persistence
bridges are separate modules.

## Verification

| Command / check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --all-targets --locked` | PASS: 132 tests; 2 release-only ignored |
| `cargo test -p stickymd-core --release --locked` | PASS: 45 tests |
| `cargo test -p stickymd-render --release --locked` | PASS: 18 tests |
| `cargo test -p stickymd-win --release --locked` | PASS: 69 tests; 2 release-only ignored |
| `cargo build --workspace --release --locked` | PASS |
| `cargo deny check` | PASS; existing duplicate-version warnings remain non-fatal |
| forbidden dependency tree scan | PASS |
| core/render unsafe runtime scan | PASS: 0 |
| Windows unsafe location scan | PASS: adapter-only |
| 30 production `plan_ref` stable anchors | PASS |
| Markdown relative-link scan | PASS |
| `git diff --check` | PASS; Git reports only the repository's LF→CRLF checkout warning for `.gitignore` |

## Remaining Manual Conditions

1. Execute and record Microsoft Pinyin / WeChat IME / candidate-position matrix (inherited Phase 3).
2. Repeat external-editor acceptance through real Notepad (current receipt used file injection).
3. Validate read-only `note.md` UI behavior; read-only Program Directory fatal path was exercised,
   but the note-only automation receipt was inconclusive.
4. Run a deterministic process kill between temp flush and publish; current force-kill recovery used
   manually staged complete temp evidence.
5. Test >260-character Program Directory on a host whose long-path policy permits it.
6. Rare live `ReplaceFileW` 1175/1176/1177 states were not induced; typed classification is automated.

## Recommendation

**APPROVE Phase 5 WITH CONDITIONS**

The persistence architecture and automated correctness gates are suitable for the next independent
Markdown/Owned-AST phase. The conditions above remain release/acceptance debt and must not be relabeled
as PASS or silently inherited into a v1 readiness claim.
