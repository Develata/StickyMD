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

## Automated verification

`verify-package.ps1` rejects duplicate, absolute, drive-qualified, backslash or traversal ZIP paths and any entry outside the allowlist. It verifies the 30 MiB ZIP gate, every SHA256SUMS entry, PE/x86_64/PE32+, embedded PerMonitorV2/asInvoker manifest, StickyMD product/file version and an extractable application icon.

The development-tree validation package and SPDX checksum passed. Its filename intentionally contains `local-validation` and `dirty`; it is not release evidence. ASCII/space/Chinese-path runtime and instance lifecycle evidence must be rerun from the final exact-commit local RC before those acceptance rows advance.

## Symbols

The default release profile strips symbols and the checked-in package does not create a second differently-built EXE or PDB. No symbols archive is claimed for Phase 9. A future symbols artifact must be produced from the exact same linker invocation and verified against that EXE before publication.
