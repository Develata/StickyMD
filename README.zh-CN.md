# StickyMD 中文说明

StickyMD 是一张原生、便携、常驻 Windows 11 桌面的 Markdown 草稿纸：打开即写，自动保存，公式可靠，贴边即隐，需要时迅速出现。它不是通用 Markdown 编辑器，也不是知识管理系统。

> 预发布状态：Phase 14 发布政策校准与 exact-candidate 收口正在进行。旧候选已失效；任何新 exact candidate 在完整 evidence 关闭前仍不具备 ready 状态。USER 已批准版本 0.1.0、unsigned portable 分发和 v0.1.0 cold/warm startup 550 ms hard boundary（180 ms 为 preferred，400 ms 为诊断性 engineering target）。真实输入法、视觉、任务栏/Alt+Tab、托盘、显示器、恢复、Clean VM 与远端 artifact 门仍开放；当前不宣称 RC-ready 或 stable-ready。

## 使用方式

1. 把 portable ZIP 解压到当前用户可写目录。
2. 直接运行 `StickyMD.exe`，无需管理员权限。
3. 程序只编辑 `<program-dir>/note/note.md`；移动便签时应连同整个目录一起移动。

一个程序目录就是一张便签的身份。同一 canonical 目录的第二个进程只会唤醒已有实例；不同目录中的副本彼此独立。不要把程序放进 `Program Files`。

v0.1.0 明确采用未做 Authenticode 签名的 portable ZIP，Windows 可能显示 SmartScreen/信誉提示。运行前请核对 `SHA256SUMS.txt`；高级用户还可以使用 `gh attestation verify` 验证 GitHub artifact attestation。StickyMD 不建议关闭 Defender 或 SmartScreen。

## 架构边界

- Rust 原生 UI，不使用 WebView、Electron、Tauri 或 JavaScript runtime。
- 不使用数据库、遥测、自动更新器或 runtime 网络 client。
- `DocumentState` 是运行时 canonical 文本的唯一权威；磁盘文件、编辑器排版与 Preview 都只是 durable fact 或 projection。
- 保存采用带 durable fingerprint 的 guarded atomic replace，避免 watcher 失效时静默覆盖外部编辑。
- Markdown 语义由 Comrak 定义，数学语义和布局由 RaTeX 定义。

## 编辑体验

- Split 默认按 generation-bound Markdown source range 双向对齐滚动，可通过分隔线上的同步按钮关闭并保留两边独立位置。
- Source 支持 `Ctrl+F` 纯文本查找与 `Ctrl+H` 查找替换，大小写敏感开关默认关闭，不支持正则。
- 顶部 `$` 按钮把 Comrak 确认的 `\(...\)` / `\[...\]` 数学分隔符批量转换为 dollar 形式。

## 从源码构建

需要 Windows 11 x64、MSVC/Windows SDK 构建工具，以及 `rust-toolchain.toml` 固定的 Rust 工具链。

```powershell
cargo build --workspace --release --locked
./tools/smoke/all.ps1 -Ci
```

真实 GUI、输入法、显示器拓扑、资源和视觉检查必须使用对应 Phase 的人工验收矩阵，未执行的项目保持 `NOT TESTED`。

## 文档

- [工程合同](docs/plan/)
- [v1 验收合同](docs/acceptance-cases/00_v1_acceptance.md)
- [Phase 14 验收矩阵](docs/acceptance-cases/phase-14.md)
- [发布检查清单](docs/release-checklist.md)
- [第三方声明](THIRD_PARTY_NOTICES.md)
- [安全策略](SECURITY.md)
- [贡献指南](CONTRIBUTING.md)

## 许可证

StickyMD 使用 MIT 许可证。嵌入的 KaTeX-compatible 字体保留 SIL Open Font License 1.1，完整声明随发布包分发。
