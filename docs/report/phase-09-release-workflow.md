# Phase 9 Release Workflow Report

## Result

- Local workflow syntax/static audit: **PASS**.
- Remote GitHub workflow execution: **NOT EXECUTED**.
- Tag, push and GitHub Release creation during Phase 9: **NO**.

## Pinned components

| Component | Version | Immutable commit |
| --- | --- | --- |
| actions/checkout | 6.0.2 | `de0fac2e4500dabe0009e67214ff5f5447ce83dd` |
| actions/upload-artifact | 7.0.1 | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` |
| actions/download-artifact | 8.0.1 | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` |
| actions/attest | 4.2.2 | `1e69f48acb82d1966a394da916b4c1698aa569d6` |
| EmbarkStudios/cargo-deny-action | 2.1.1 | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` |

Versions and tag commits were rechecked against the official repositories on 2026-08-21. `actions/attest` 4.2.2 now requires `artifact-metadata: write` in addition to `id-token: write` and `attestations: write`; this current upstream requirement is granted only to the tag-only attestation/draft job. No package, actions or security-events write permission is granted.

## Security model

The global workflow permission is `contents: read`. `workflow_dispatch` can build, package, verify and upload a short-lived diagnostic artifact, but its tag-only job is skipped and it cannot attest or create a release. Only `push` of a `v*` tag can enter the write-capable job. The tag must equal the workspace version and the commit must be contained in `origin/main`.

Checkout credentials are not persisted. Actions use full immutable SHAs. Syft is downloaded at an exact release URL and verified by pinned upstream checksums. The workflow contains no `pull_request_target`, installer pipe, cache-derived release binary or automatic dependency update.

The Windows job runs the checked-in smoke and release scripts; YAML does not reproduce package rules. The tag job attests `SHA256SUMS.txt` subjects, attaches the SPDX SBOM to the portable ZIP, and creates or refreshes a **draft** release. It never publishes stable automatically.

The scheduled workflow has read-only permissions, runs policy checks, reports dependency drift with `cargo update --dry-run`, and executes deterministic platform-independent stress tests. It does not modify the lockfile, open a PR or merge an update.
