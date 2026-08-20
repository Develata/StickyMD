# Windows file replacement reference

> 本文件记录可变的外部 API 事实，不是架构权威。长期合同见
> `docs/plan/05_document_persistence.md#atomic-save`。

## Verified sources

- Microsoft `ReplaceFileW`:
  <https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew>
- Microsoft `MoveFileExW`:
  <https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-movefileexw>
- Microsoft `FlushFileBuffers`:
  <https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers>

Verified on 2026-08-20.

## Facts used by StickyMD

1. `REPLACEFILE_WRITE_THROUGH` is documented as unsupported. StickyMD therefore flushes the
   replacement temp handle itself and passes `flags=0` to `ReplaceFileW`.
2. `ERROR_UNABLE_TO_REMOVE_REPLACED` (1175), `ERROR_UNABLE_TO_MOVE_REPLACEMENT` (1176), and
   `ERROR_UNABLE_TO_MOVE_REPLACEMENT_2` (1177) describe different possible filesystem states.
   StickyMD classifies them and preserves evidence instead of attempting a blanket second move.
3. New-target publication uses same-directory `MoveFileExW(MOVEFILE_WRITE_THROUGH)` without
   `MOVEFILE_REPLACE_EXISTING`; an unexpectedly appearing target is a conflict.
4. `FlushFileBuffers` is called on the temp file handle after the Rust buffer flush and before
   publish. This reduces process-crash/half-write risk but is not represented as an absolute
   power-loss transaction guarantee for every storage stack.
