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

The prior `d02f8a6` local RC is superseded. The replacement exact local RC is:

- source commit: `eb687b2441a5816111c116ce30a01bb5b0fba8c6`;
- file: `StickyMD-0.1.0-local-rc-eb687b2441a5-windows-x64-portable.zip`;
- size: 3,878,842 bytes (3.699 MiB);
- ZIP SHA-256: `ef3b503d580fbd587239f9585eeb6195734703cd3abda59c6657f422766b05f9`;
- packaged EXE size: 8,287,744 bytes (7.904 MiB);
- packaged EXE SHA-256: `84057a4322c965dbf48646274f2686464f060059a70aeebe1e72264d260c7831`;
- SBOM SHA-256: `757163513bb80f89ee9c30437ca35f4dd3db1de294f64b80a0b50b1daf5343ce`.

The package was generated twice from the same clean commit and built EXE and produced the same ZIP
digest. This proves deterministic archive construction for one EXE input; it does not claim two
independent Rust/linker builds are bit-for-bit reproducible.

## Automated verification

`verify-package.ps1` rejects duplicate, absolute, drive-qualified, backslash or traversal ZIP paths
and any entry outside the allowlist. It requires exactly the ZIP and SPDX SBOM in `SHA256SUMS.txt`,
regenerates the frozen runtime notices for a byte comparison, and verifies the 30 MiB ZIP gate,
PE/x86_64/PE32+/GUI subsystem, PerMonitorV2/asInvoker manifest, complete matching version resource
and an extractable application icon.

The replacement exact local RC passed copied-package runtime tests in ASCII, space-containing and
Chinese paths, same-directory wake/exit and different-directory isolation. No archive was
published, tagged or pushed.

## Symbols

The default release profile strips symbols and the checked-in package does not create a second differently-built EXE or PDB. No symbols archive is claimed for Phase 9. A future symbols artifact must be produced from the exact same linker invocation and verified against that EXE before publication.
