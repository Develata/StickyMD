# Phase 7 Windows Clipboard Format Audit

## Selection Contract

StickyMD queries Windows clipboard content in this order:

1. `CF_HDROP` file descriptors.
2. Registered encoded image formats: `PNG`, `image/png`, `JFIF`, `JPEG`, `image/jpeg`, `WebP`,
   `image/webp`.
3. `CF_DIBV5`, then `CF_DIB`.
4. `CF_BITMAP` converted to RGBA through the bounded arboard fallback.
5. Unicode text through the established text clipboard adapter.

This ordering prefers user-selected source files and original encoded bytes before any bitmap
conversion. File content is read and decoded on the I/O worker, not inside the clipboard callback.

## Safety Boundaries

- Clipboard open attempts occur at 0/10/30/80 ms and then fail visibly.
- File lists are capped at 256 entries.
- HGLOBAL payloads are capped at 64 MiB before copy.
- DIB/V5 and CF_BITMAP headers/dimensions are validated before allocation/copy.
- Pixel byte multiplication is checked and then revalidated by the render decoder.
- `OpenClipboard` is paired by an RAII `CloseClipboard` guard.
- Every Win32 pointer/handle block has a local `SAFETY` invariant.

## Automated Adapter Evidence

Unit tests cover encoded-format preservation, DIB and raw-RGBA conversion, alpha premultiplication,
corrupt input, side/pixel/byte limits and image-paste OCC. The implementation order and enabled
format constants are checked by source/governance review.

## Real Windows Source Matrix

Real producers are environment-dependent. No simulated test is promoted to a real-source PASS.

| Source | Available Formats | Selected Path | Stored Format | Result |
| --- | --- | --- | --- | --- |
| Explorer PNG | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |
| Explorer JPEG | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |
| Snipping Tool | NOT TESTED | NOT TESTED | expected bitmap→PNG | NOT TESTED |
| Paint | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |
| Browser image copy | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED |

These rows can only change after a durable current-commit manual receipt records the formats exposed
by each producer, the selected branch, the resulting managed filename and visible paste result.
