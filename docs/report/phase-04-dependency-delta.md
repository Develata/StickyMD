# Phase 4 Dependency Delta

## Runtime additions

| Crate | Requested / resolved | License | Purpose | Runtime and boundary impact |
| --- | --- | --- | --- | --- |
| `sha2` | `0.10.9` / `0.10.9` | MIT OR Apache-2.0 | Stable SHA-256 durable fingerprint and instance key | Pure Rust; core value/hash capability only; no I/O |
| `serde` | `1.0.219` / `1.0.229` | MIT OR Apache-2.0 | Config DTO derive | Windows app only; `DocumentState` is not serialized |
| `toml` | `0.9.5` / `0.9.12+spec-1.1.0` | MIT OR Apache-2.0 | `config.toml` v1 parsing/writing | Windows app config adapter only |
| `notify` | `8.2.0` / `8.2.0` | CC0-1.0 | Non-recursive `note/` change hints | Windows target only, `default-features = false`; backend thread only emits hints |
| `windows` | `0.62.0` / `0.62.2` | MIT OR Apache-2.0 | Required Win32 atomic replace, named objects, message box | Windows target only; explicit namespaces, no broad `Win32` feature |

## Windows features

```text
std
Win32_Foundation
Win32_Security
Win32_Storage_FileSystem
Win32_System_Threading
Win32_UI_WindowsAndMessaging
```

## Audit notes

- `notify 8.2.0` declares Rust 1.77 and uses the Windows native backend; no polling loop is selected.
- Resolved versions are recorded from `Cargo.lock`; requested lower bounds remain intentionally narrow.
- No `tokio`, async runtime, network client, database, WebView, GPU UI framework, or plugin system
  was introduced.
- `stickymd-core` gained only `sha2`; it remains platform-independent and contains no filesystem API.
