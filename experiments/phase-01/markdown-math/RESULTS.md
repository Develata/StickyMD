# Phase 1 Markdown / Math Revalidation

- `Status`: Automated contract tests and Release measurements completed on the current worktree
- `Date`: 2026-08-20

## Proven by automated tests

- Comrak 0.54 parses CommonMark/GFM plus all four approved math delimiter forms.
- Code spans do not become math nodes.
- Raw HTML remains literal AST data; this spike has no HTML execution path.
- Arena data is copied into an owned diagnostic tree and can be dropped.
- RaTeX 0.1.14 parses/layouts/renders the required baseline formula set without WebView.
- Malformed math returns an error.

The PNG renderer is spike-only. It is not approval for a production PNG encode/decode hot path.

## Commands

```powershell
cargo test --manifest-path experiments/phase-01/markdown-math/Cargo.toml --locked
cargo run --release --manifest-path experiments/phase-01/markdown-math/Cargo.toml --locked
```

## Fresh Release measurements

20 samples after three warm-ups, Windows 11 x64, Rust 1.97.1:

| Pipeline | median | p95 | max |
| --- | ---: | ---: | ---: |
| 20 KiB Comrak + owned projection | 3.665 ms | 3.944 ms | 4.147 ms |
| 100 KiB Comrak + owned projection | 16.980 ms | 17.727 ms | 17.744 ms |
| 1 MiB Comrak + owned projection | 175.445 ms | 184.886 ms | 184.977 ms |
| Representative RaTeX parse/layout/PNG | 0.558 ms | 0.920 ms | 1.824 ms |

These numbers are also summarized in `docs/report/phase-01-performance-baseline.md`; they are local
engineering evidence, not public performance claims.
