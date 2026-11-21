# StickyMD Packaging CLI

`stickymd-package` builds the release bytes the repository's own tools produce: the Windows and
macOS package directories and ZIPs, the package README and third-party notices, the SBOMs and
checksum files, the pinned Syft cache, and the macOS product input fingerprint that
`apps/stickymd-mac/packaging/build-app.sh` writes into the bundle. It is std-only, declares no
dependency and never calls `stickymd-smoke`; `stickymd-smoke` depends on it to verify what it
builds. Plan 11 #platform-artifact-reuse therefore counts this crate, and not the verification
tool, as a build input of both platforms: a change here rebuilds both platforms, a change to the
verification tool alone does not. The governance check of `stickymd-smoke phase 00` covers the
edges the compiler cannot see as a closed world (plan 11 "边界的证明"): no dependency or build
script; every file rustc reads compiling this crate, as its dep-info lists them, is a regular
tracked file inside this directory; the byte-producing scripts, build scripts and release
`build-package` jobs are fixed lists or pinned copies; and none of these files, comments
included, names the verification tool. Keep the tool's name out of this crate's sources.

The release workflows, `tools/release/package.ps1`, `generate-sbom.ps1`,
`generate-third-party-notices.ps1` (through `invoke-package.ps1`) and `build-app.sh` are its
callers. Every command writes where its caller says, never into `dist/evidence/`, the resource
diagnostics cache or the clone-wide qualification ledger.

```powershell
cargo run --quiet -p stickymd-package --locked -- package-inputs --allow-dirty-validation
cargo run --quiet -p stickymd-package --locked -- prepare-package --exe <exe> --staging-directory <new-directory> [--allow-dirty-validation]
cargo run --quiet -p stickymd-package --locked -- build-package [--exe <exe>] [--output-directory <directory>] [--allow-dirty-validation]
cargo run --quiet -p stickymd-package --locked -- generate-sbom [--package-directory <directory>] [--zip <zip>] [--output <sbom>] [--syft-path <tool>]
cargo run --quiet -p stickymd-package --locked -- notices --destination <new-file>
cargo run --quiet -p stickymd-package --locked -- checksums --zip <zip> [--sbom <sbom>] --output <manifest>
cargo run --quiet -p stickymd-package --locked -- publish-sbom --input <staged-sbom> --output <sbom> --zip <zip> --checksums <manifest>
cargo run --quiet -p stickymd-package --locked -- publish-package --input <completed-zip> --output <zip> --checksums <manifest>
cargo run --quiet -p stickymd-package --locked -- prepare-sbom --package-directory <directory> --staging-directory <new-directory> [--zip <zip>] [--syft-path <provided-tool>]
cargo run --quiet -p stickymd-package --locked -- syft-plan [--syft-path <provided-tool>]
cargo run --quiet -p stickymd-package --locked -- syft-publish --kind <archive|checksums> --input <downloaded-file>
cargo run --quiet -p stickymd-package --locked -- syft-verify --archive <archive> --checksums <upstream-manifest> --staging-directory <new-directory>
cargo run --quiet -p stickymd-package --locked -- macos-package --app <StickyMD.app> --output-directory <directory> [--allow-dirty-validation]
cargo run --quiet -p stickymd-package --locked -- macos-sbom --zip <zip>
cargo run --quiet -p stickymd-package --locked -- app-inputs [--home <dir>]
```

`notices` takes `--platform <windows-x64|macos-arm64>` (Windows by default). `app-inputs` prints
the product input fingerprint of this checkout and toolchain (plan 11 #phase-16-macos-host);
`--home` is the caller's home directory, given by the caller so that a Cargo configuration cannot
redirect where Cargo's home is looked for. `build-app.sh` runs it in a clean environment.

`package-inputs` also accepts `--version`, `--commit-sha`, `--release-tag` and
`--exact-candidate`, preserving `package.ps1`'s override and dirty-tree policy.
It returns one JSON plan marked `NOT_RUN`. Package selection and naming use the
same Rust implementation. `workspace-version` reads the scalar from
`[workspace.package]`, accepts assignment whitespace and comments, and refuses
missing or duplicate versions instead of selecting a dependency/metadata version.
Verification/notices retain the scripts' `KEY=value` stdout and 0/nonzero exit
semantics. The wrappers resolve relative paths using the caller's PowerShell
location, restore that location and console encoding even on failure, and use
locked Cargo invocations.

`build-package` and `generate-sbom` coordinate these same typed Rust rules directly.
The runner calls them in-process and preserves its task IDs/order and JSON stdout
boundary. The stable PowerShell scripts each make one locked Cargo invocation;
Cargo still checks source freshness. The two commands accept `--powershell <host>`
for their platform adapter; wrappers supply their own 5.1/7 host, while direct CLI
and runner calls default to `pwsh`. No application build occurs inside packaging.
ZIP compression keeps the original member order, timestamp and compression API in
`package-archive.ps1`. Rust owns private staging, publication and cleanup. Adapter
or validation failure returns nonzero without creating success qualification evidence.

`prepare-package` accepts the same identity overrides and flags as `package-inputs`.
It creates only a new staging directory (the parent must exist), writes complete
files atomically, and removes its owned staging tree on failure. Existing files or
directories are never overwritten or cleaned. Its JSON contains the unchanged
`inputs` plan, ordered `members`, and `diagnostics` for the wrapper to forward.
`inputs.status: NOT_RUN` still means package verification/qualification has not run.
`src/package_content.rs` owns the six members, staging content sources and the
four SBOM-required flags. ZIP validation and SBOM coverage consume that inventory;
PowerShell compresses the returned ordinal member list without enumerating or
selecting content. README titles come from the validated source state; the text
template remains UTF-8 without BOM with CRLF output. Packaged licenses reuse the
strict notices decoder, normalize line endings to LF and omit BOMs.
