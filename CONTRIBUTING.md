# Contributing

StickyMD is contract-first. Before changing code, read `AGENTS.md`, the engineering constitution, terminology, the applicable `docs/plan` chapter, and the matching acceptance projection.

Core rules:

- Preserve `DocumentState` as the only canonical runtime text authority.
- Keep platform-neutral crates free of Windows APIs and `unsafe`.
- Do not add WebView, Electron, Tauri, a JavaScript runtime, a database, a general async runtime, telemetry, updater, or runtime network client.
- Keep filesystem writes behind the approved storage adapters and use atomic replacement.
- Prefer cohesive, measured implementations over speculative frameworks or dependency growth.
- Add or update the owning Phase Rust smoke and acceptance matrix. Real IME/GUI/visual/display checks must remain `NOT TESTED` until actually performed.

Before proposing a change, run the narrowest relevant tests, then:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
./tools/smoke/all.ps1 -Ci
git diff --check
```

Dependency changes require license, advisory, transitive-tree, runtime-cost and feature-scope review. Architecture changes require a risk report and explicit maintainer approval before implementation.
