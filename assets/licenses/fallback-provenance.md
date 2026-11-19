# macOS 依赖的许可证回退文本：来源记录

`stickymd-smoke release notices --platform macos-arm64` 为下列没有随包许可证文件的 crate 使用 `assets/licenses/` 中的回退文本，
映射按 (crate, 版本) 写在 `tools/stickymd-smoke/src/release/notices/licenses.rs`，未列出的版本会让生成失败。每个 crate 包里的
`.cargo_vcs_info.json` 记着发布时的上游提交；下表的“上游”即该提交，`.crate` 的 SHA-256 取自 `Cargo.lock`。2026-11-20 核对。

许可证选择：`objc2`、`objc-sys`、`block2`、`objc2-encode`、`objc2-foundation` 与 0.2.2 的框架 crate 只声明 MIT；0.3.2 的框架 crate 与
`dispatch2` 声明 Zlib OR Apache-2.0 OR MIT，取 MIT；`dispatch` 声明 MIT。

| crate | 版本 | `.crate` SHA-256 | 上游 | 包内路径 | 使用的文本 | 文本来源 |
| --- | --- | --- | --- | --- | --- | --- |
| `block2` | 0.5.1 | `2c132eebf10f5cad…` | `madsmtm/objc2@4fc083f1c6d6` | `crates/block2` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `block2` | 0.6.2 | `cdeb9d8705160014…` | `madsmtm/objc2@b4167b582b2f` | `crates/block2` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `dispatch2` | 0.3.1 | `1e0e367e4e7da845…` | `madsmtm/objc2@8852b424193c` | `crates/dispatch2` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `dispatch` | 0.2.0 | `bd0c93bb4b0c6d9b…` | `SSheldon/rust-dispatch@82d6c7a5b75d` | `/` | `dispatch-MIT.txt` | 重建：该修订没有许可证文件；按 Cargo.toml 的 `license = "MIT"` 与作者 Steven Sheldon |
| `objc-sys` | 0.3.5 | `cdb91bdd390c7ce1…` | `madsmtm/objc2@4fc083f1c6d6` | `crates/objc-sys` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-app-kit` | 0.2.2 | `e4e89ad9e3d7d297…` | `madsmtm/objc2@e282618be4c3` | `framework-crates/objc2-app-kit` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-app-kit` | 0.3.2 | `d49e936b501e5c5b…` | `madsmtm/objc2@7b1abfd750a2` | `framework-crates/objc2-app-kit` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2-core-data` | 0.2.2 | `617fbf49e071c178…` | `madsmtm/objc2@e282618be4c3` | `framework-crates/objc2-core-data` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-core-foundation` | 0.3.2 | `2a180dd8642fa45c…` | `madsmtm/objc2@7b1abfd750a2` | `framework-crates/objc2-core-foundation` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2-core-graphics` | 0.3.2 | `e022c9d066895efa…` | `madsmtm/objc2@7b1abfd750a2` | `framework-crates/objc2-core-graphics` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2-core-image` | 0.2.2 | `55260963a527c99f…` | `madsmtm/objc2@e282618be4c3` | `framework-crates/objc2-core-image` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-encode` | 4.1.0 | `ef25abbcd74fb260…` | `madsmtm/objc2@8d214f547736` | `crates/objc2-encode` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2-foundation` | 0.2.2 | `0ee638a5da379932…` | `madsmtm/objc2@e282618be4c3` | `framework-crates/objc2-foundation` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-foundation` | 0.3.2 | `e3e0adef53c21f88…` | `madsmtm/objc2@7b1abfd750a2` | `framework-crates/objc2-foundation` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2-io-surface` | 0.3.2 | `180788110936d59b…` | `madsmtm/objc2@7b1abfd750a2` | `framework-crates/objc2-io-surface` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2-metal` | 0.2.2 | `dd0cba1276f60239…` | `madsmtm/objc2@e282618be4c3` | `framework-crates/objc2-metal` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-quartz-core` | 0.2.2 | `e42bee7bff906b14…` | `madsmtm/objc2@e282618be4c3` | `framework-crates/objc2-quartz-core` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2-quartz-core` | 0.3.2 | `96c1358452b371bf…` | `madsmtm/objc2@7b1abfd750a2` | `framework-crates/objc2-quartz-core` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |
| `objc2` | 0.5.2 | `46a785d4eeff09c1…` | `madsmtm/objc2@4fc083f1c6d6` | `crates/objc2` | `objc2-MIT-4fc083f.txt` | 原文：该修订根目录 `LICENSE.txt`（MIT，Steven Sheldon），逐字节照录 |
| `objc2` | 0.6.4 | `3a12a8ed07aefc76…` | `madsmtm/objc2@8852b424193c` | `crates/objc2` | `objc2-LICENSE-8852b42.md`、`objc2-MIT.txt` | 原文 + 重建：该修订根目录只有 `LICENSE.md`（逐字节照录，说明哪些 crate 只用 MIT、哪些三选一）；没有 MIT 全文，`objc2-MIT.txt` 为重建 |

## 文件

| 文件 | 来源 | SHA-256 |
| --- | --- | --- |
| `objc2-MIT-4fc083f.txt` | `madsmtm/objc2` 在 `4fc083f1c6d6` 与 `e282618be4c3` 的根目录 `LICENSE.txt`，两处逐字节相同 | `e353f37b12aefbb9…` |
| `objc2-LICENSE-8852b42.md` | `madsmtm/objc2` 在 `8852b424193c`、`b4167b582b2f`、`8d214f547736`、`7b1abfd750a2` 的根目录 `LICENSE.md`，四处逐字节相同 | `7f976f7e9cb2d87d…` |
| `objc2-MIT.txt` | 重建：MIT 全文，版权行取 `4fc083f1` 的 Steven Sheldon 与 `madsmtm/objc2` 主线 `8a5abac` 的 `LICENSE-MIT.txt` 的 Mads Marquart | — |
| `dispatch-MIT.txt` | 重建：MIT 全文，版权行取 `rust-dispatch` 0.2.0 Cargo.toml 的作者 | — |

## 未决

- 两份重建文本的署名是否完整，上游无法确认（`madsmtm/objc2` #826）。`dispatch2` 0.3.1 的 Cargo.toml 还列有作者 Mary（其前身项目
  `marysaka/dispatch2` 的 MIT 文本没有版权行），证据不足以自行加入版权人。首个公开 macOS 包之前由 USER 决定是否接受。
