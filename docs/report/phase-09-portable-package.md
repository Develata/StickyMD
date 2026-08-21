# Phase 9 Portable Package Report

## Contract

The package script accepts an already-built EXE; it never invokes Cargo. Local RC names include workspace version and a 12-character source commit. A dirty tree is refused unless the caller explicitly requests a `local-validation-...-dirty` artifact, which cannot be mistaken for an RC. Tagged names are accepted only when `vX.Y.Z` exactly matches the workspace version.

The ZIP is produced in deterministic lexical entry order with a fixed ZIP timestamp and this exact allowlist:

```text
StickyMD/StickyMD.exe
StickyMD/README.txt
StickyMD/LICENSE.txt
StickyMD/THIRD_PARTY_NOTICES.txt
StickyMD/licenses/SIL-OFL-1.1.txt
StickyMD/licenses/KaTeX-fonts-NOTICE.txt
```

No `note/`, config, images, trash, PDB, user data or proprietary font can enter the archive.

## Exact local RC

The prior `d02f8a6` local RC is superseded: it predates generated notices for the complete locked
Windows runtime graph and the stricter checksum/PE verifier. The replacement source commit, file,
size and ZIP/EXE/SBOM SHA-256 values remain pending until the review fixes are committed and built
from a clean tree. Superseded digests are intentionally not presented as current evidence.

## Automated verification

`verify-package.ps1` rejects duplicate, absolute, drive-qualified, backslash or traversal ZIP paths
and any entry outside the allowlist. It requires exactly the ZIP and SPDX SBOM in `SHA256SUMS.txt`,
regenerates the frozen runtime notices for a byte comparison, and verifies the 30 MiB ZIP gate,
PE/x86_64/PE32+/GUI subsystem, PerMonitorV2/asInvoker manifest, complete matching version resource
and an extractable application icon.

The replacement exact local RC must rerun copied-package runtime tests in ASCII, space-containing
and Chinese paths, same-directory wake/exit and different-directory isolation. No archive was
published, tagged or pushed.

## Symbols

The default release profile strips symbols and the checked-in package does not create a second differently-built EXE or PDB. No symbols archive is claimed for Phase 9. A future symbols artifact must be produced from the exact same linker invocation and verified against that EXE before publication.
