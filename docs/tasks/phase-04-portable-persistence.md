# Phase 4 — Portable Persistence, Autosave, Recovery & External Reconciliation

## Completion State

In Progress

## Goal

Connect the canonical in-memory document to its fixed portable durable representation without
introducing a second text authority, silent overwrite, unbounded work, or Windows leakage into core.

## Prerequisites

- Phase 0 governance and architecture contracts are present.
- Phase 1 risk spikes and Phase 2 canonical `DocumentState` exist.
- USER explicitly authorized Phase 4 despite Phase 3's outstanding manual IME acceptance gate.

## Inputs

- Governing Phase 0 plan chapters and v1 acceptance projections.
- Phase 2 `DocumentState`, generation, snapshot and acknowledgement contracts.
- Phase 3 source-shell typed intent/effect pipeline.
- USER's Phase 4 implementation prompt and explicit authorization.

## Inherited Conditions

- Microsoft Pinyin: NOT TESTED.
- WeChat Input Method: NOT TESTED.
- Candidate positioning at 100/150/200% DPI: NOT TESTED.
- These conditions remain unchanged; persistence must not mask or relabel them.

## Scope

- Canonical Program Directory identity and same-directory single instance.
- Portable runtime paths, writable validation, note/config bootstrap.
- UTF-8/BOM/newline load and encode boundaries.
- Same-directory atomic note/config publication.
- One bounded I/O worker and deterministic 650 ms autosave scheduling.
- Guarded save OCC, recovery, watcher hints, external reload/conflict/delete handling.
- Minimal safety UI for recovery, conflict, save failure, and degraded watcher state.

## Out of Scope

Preview/Split, Comrak/RaTeX production rendering, image/asset transactions, export, tray/dock,
multi-monitor behavior, final theme/opacity/topmost UI, and RichEdit fallback.

## Authority Model

`DocumentState` is the only runtime canonical text owner. Disk bytes are durable facts;
`SourceProjection` is disposable. Worker/watcher callbacks return typed results to the main
coordination thread and cannot mutate either authority or projection.

## Startup Lifecycle

Canonical program directory → single instance → writable probe → directory layout → config load →
recovery inspection → note load/create → editor → watcher → autosave. Secondary instances exit
before durable bootstrap.

## Single Instance

Canonical identity bytes are normalized separately from the real I/O path, hashed with SHA-256,
and used for named mutex/event objects. The primary listener forwards only `ShowRequested`.

## Persistence Pipeline

Document Snapshot → worker newline encoding → exact-byte SHA-256 → same-directory temp write →
Rust flush → `FlushFileBuffers` → final expected-fingerprint check → atomic publish → typed completion.

## Autosave

650 ms virtual-time-testable debounce. An edit updates only deadline/generation; snapshot occurs
when due. Worker capacity is one in-flight note plus one replaceable latest pending note.

## Recovery

`note.md.tmp` is never deleted before canonical load or a successful restore publish. Equal content
is redundant; different valid content is a user choice; invalid/oversized evidence is preserved.

## External Reconciliation

Non-recursive watcher hints coalesce for 150 ms. A fresh read/hash decides known self-write,
clean reload, missing-file recreation, or conflict. Watcher failure degrades UX only; guarded OCC
still prevents silent overwrite.

## Conflict

Autosave pauses. The latest external fact can be loaded (clearing undo) or the latest local
generation can be force-written only after explicit Keep Local intent.

## Config

TOML schema v1 is independent of DocumentState. Missing fields default; unknown fields are ignored;
corrupt/newer files are preserved and defaults used. Config writes reuse the atomic publisher.

## Failure Paths

Typed paths cover writable/bootstrap failures, invalid/large note content, temp write/flush/publish
failure, ReplaceFileW 1175/1176/1177, guarded conflict, watcher degradation, config preservation,
worker loss, and save failure without canonical rollback.

## Performance Gates

- Release save stages measured for 20 KiB, 100 KiB, and 1 MiB.
- No unbounded queues, per-keypress snapshots, polling watcher, or persistent saved-text cache.
- Portable Release idle memory/CPU and executable size are measured separately from tests.

## Acceptance

AC-001, AC-005, AC-006, AC-007, AC-008, AC-026, AC-027, and AC-030 are mapped. Automated
component evidence is not relabeled as complete manual end-to-end acceptance.

## Deliverables

- Pure persistence/recovery/conflict facts in core.
- Portable paths, writable validation, single instance and narrow Win32 adapters.
- Guarded atomic note storage, independent config storage and one bounded worker.
- Deterministic autosave/reconciliation/recovery coordinators and minimal safety UI.
- Dependency delta, reference note, coverage projection and Phase 4 result report.

## Verification

- 稳定入口：`tools/smoke/phase-04.ps1`；`-Performance` 与 `-Runtime` 分别运行
  persistence baseline 和 copied-Release portable lifecycle。
- 当前自动/人工状态：`docs/acceptance-cases/phase-04.md`。
- Workspace fmt/clippy/test/release build and package-specific Release tests.
- Forbidden-dependency, unsafe-boundary, plan-anchor and diff checks.
- Release persistence stage benchmark and copied standalone EXE portable smoke.
- Manual gaps remain listed in the report and are not counted as PASS.

## Risks

- `ReplaceFileW` cannot provide an absolute all-filesystem power-loss guarantee.
- OCC narrows but cannot eliminate the final hash-check-to-replace TOCTOU interval.
- Real Notepad conflict/recovery UI, ACL/read-only, and kill-near-save cases require portable smoke
  evidence or remain explicitly NOT TESTED.

## Result

The production persistence slice is implemented with guarded whole-file publication, bounded
coordination, recoverable temporary evidence, external reconciliation, and portable single-instance
identity. Automated and portable Release evidence passes, but the task remains In Progress under the
repository completion rule until the listed manual conditions are closed. The inherited real-IME gate, an actual
Notepad receipt, read-only `note.md`, long-path behavior, and a deterministic kill during the narrow
publish transaction remain explicit manual conditions; see the Phase 4 report.
