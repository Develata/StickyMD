# Phase 1 Portable Persistence Revalidation

- `Status`: Automated logic/failure-injection tests and Windows Release smoke completed
- `Date`: 2026-08-20

## Corrected contracts

- Recovery receives bytes **and mtime**. Only a valid UTF-8, newer, different temp is offered.
- Invalid/stale temp is classified; it is not silently promoted to document authority.
- Temp creation uses `create_new`, so an unresolved recovery temp is never overwritten.
- Failure before temp creation keeps the original and leaves no temp.
- Failure after temp flush/before replace keeps the original and leaves a recoverable temp.
- Existing targets use `ReplaceFileW`; first creation uses non-replacing `MoveFileExW` with
  `WRITE_THROUGH`. Unknown `ReplaceFileW` errors are returned—there is no unconditional fallback.
- Named mutex/event remains isolated in the Windows adapter.

## Commands

```powershell
cargo test --manifest-path experiments/phase-01/persistence/Cargo.toml --locked
cargo run --release --manifest-path experiments/phase-01/persistence/Cargo.toml --locked
```

## Still NOT TESTED

- Real junction/reparse-point identity equivalence.
- Non-ASCII Windows ordinal case equivalence; automated identity tests cover only ASCII case and separator variants.
- ACL/read-only-volume rejection.
- Kill-mid-save and hardware power-loss behavior.
- Microsoft Pinyin and WeChat IME (not a persistence test).

These items remain explicit gates; they are not inferred from unit tests.

## Fresh results

- `cargo test --locked`: 9 passed, 0 failed.
- Release smoke: canonical directory resolved; atomic first create/replace PASS; second-process named
  mutex detection and event wake PASS.
