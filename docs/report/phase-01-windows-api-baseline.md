# Phase 1 Windows API Baseline — Rebuilt

- `Date`: 2026-08-20
- `Status`: Current for retained experimental adapters

## APIs Actually Used

| API | Purpose | Owner | Unsafe invariant |
| --- | --- | --- | --- |
| `ReplaceFileW` | replace an existing flushed note | persistence Windows adapter | two live NUL-terminated same-directory paths; temp closed/flushed |
| `MoveFileExW` + `WRITE_THROUGH` | first creation only, without replace flag | persistence Windows adapter | target confirmed absent; unknown race returns error |
| `CreateMutexW` | per-directory first-instance ownership | persistence Windows adapter | handle wrapped or immediately closed |
| `CreateEventW` / `OpenEventW` / `SetEvent` | second-instance wake signal | persistence Windows adapter | named event handle owned by RAII wrapper |
| `WaitForSingleObject` | bounded wake observation | persistence Windows adapter | valid owned event handle |
| `CloseHandle` | handle cleanup | RAII wrapper | exactly-once ownership |

`std::fs::File::sync_all` provides the flushed-temp durability call in this spike; no duplicate raw
`FlushFileBuffers` wrapper is needed merely to restate the standard library implementation.

## Conservative Replacement Policy

The old spike called `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` after **every** `ReplaceFileW`
error. That implementation was removed. The rebuilt adapter:

- existing target → `ReplaceFileW`; unknown error is returned and original remains;
- absent target → non-replacing `MoveFileExW(WRITE_THROUGH)`;
- target race/permission/share failure → typed I/O error, no silent overwrite.

## Not Yet Implemented or Validated

Opacity (`SetLayeredWindowAttributes`), DWM rounded corners, monitor identity, CF_HDROP, theme events,
session shutdown and watcher behavior are future production adapters. Old compile/smoke claims for
those APIs are not carried forward as current evidence.
