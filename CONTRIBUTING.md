# 参与 StickyMD

感谢你愿意帮助 StickyMD。

我们尤其欢迎准确、可复现的问题报告。Pull Request 也欢迎，但 StickyMD 的产品边界有意保持
狭窄：**代码修改请先建立 Issue，与维护者确认问题、范围和方案，再开始实现。**

文档错字、失效链接和不改变产品行为的明显测试补充可以直接提交；其余代码、依赖、架构、
用户行为和验证工具修改都应先讨论。

## 先了解产品边界

StickyMD 是一张 Markdown 草稿，不是通用 Markdown 编辑器或知识管理系统。我们不会因为某项
功能“很常见”就默认接受它。

首版明确不包含：

- 多文档、文件树、多标签页、Workspace 或 Vault。
- 双向链接、Graph、标签系统或知识库组织。
- WYSIWYG、插件系统、LSP、命令面板或代码执行。
- 云同步、账户、AI、遥测、远程图片下载或自动更新。
- WebView、Electron、Tauri、JavaScript runtime、数据库或通用 async runtime。

提交功能建议前，请先阅读：

1. [`AGENTS.md`](AGENTS.md)
2. [`docs/plan/00_engineering_constitution.md`](docs/plan/00_engineering_constitution.md)
3. [`docs/plan/01_terminology.md`](docs/plan/01_terminology.md)
4. [`docs/plan/02_positioning_and_scope.md`](docs/plan/02_positioning_and_scope.md)
5. 与改动相关的 `docs/plan/03..11`
6. [`docs/features/00_v1_product_behavior.md`](docs/features/00_v1_product_behavior.md)
7. [`docs/acceptance-cases/00_v1_acceptance.md`](docs/acceptance-cases/00_v1_acceptance.md)

长期工程合同只存在于 `docs/plan/`。features、acceptance、overview、代码和报告都不能反向
重新定义它。

## 报告问题

### Bug 报告

提交 Issue 前请先搜索是否已有相同问题。报告中尽量包含：

- StickyMD 版本或完整 commit SHA。
- Release ZIP 或 `StickyMD.exe` 的 SHA-256（如果使用发布包）。
- Windows 11 build、显示缩放、显示器数量。
- 使用的输入法及版本（涉及输入时）。
- 最小复现步骤、预期行为和实际行为。
- 是否涉及保存失败、数据覆盖、崩溃、窗口不可达或持续资源增长。
- 可以公开的合成 Markdown、截图或短视频。

请不要公开上传真实 `note.md`、剪贴板内容、用户名、完整个人路径、恢复文件、crash dump、
令牌或其他敏感信息。

### 功能建议

请回答：

1. 它是否直接服务于“一张 Markdown 草稿”？
2. 现有能力为什么不能解决这个场景？
3. 它会不会引入新的产品本体、设置面板、长期依赖或维护轴？
4. 能否用更小、更统一的交互解决？
5. 如何验证失败路径、性能与内存边界？

功能建议获得讨论支持，不等于实现方案已经获批。涉及主骨架、核心 authority、对象关系、
持久格式或关键接口时，必须先提交分析报告并获得维护者明确批准。

### 安全问题

不要在公开 Issue 中发布漏洞利用、私人草稿或敏感路径。请遵循
[`SECURITY.md`](SECURITY.md) 的私密报告流程。

## 开发环境

完整 Windows 应用开发需要：

- Windows 11 x64。
- Git 与 PowerShell。
- `rust-toolchain.toml` 固定的 Rust 工具链。
- Visual Studio Build Tools 中的 MSVC C++ 工具与 Windows SDK。

发布包的普通用户不需要这些工具；它们只用于从源码编译。

克隆后先确认基础环境：

```powershell
rustc --version
cargo --version
cargo metadata --no-deps
```

构建开发版本：

```powershell
cargo build --workspace --locked
```

构建 Release：

```powershell
cargo build --workspace --release --locked
```

不要在仓库根目录把 `cargo run` 当作普通便签使用；开发运行的 Program Directory 通常位于
`target/debug`，可能在那里创建运行时 `note/`。Portable 行为应使用复制到独立临时目录的
Release EXE 验证。

## 仓库结构

```text
crates/stickymd-core/       平台无关的 canonical document 与纯状态
crates/stickymd-render/     Markdown、数学、文本布局与原生 Preview
apps/stickymd-win/          Windows 应用、流程协调与平台 adapter
tools/stickymd-smoke/       不进入产品包的 Rust 验收 CLI
tools/smoke/                各 Phase 的 PowerShell 薄入口
docs/plan/                  唯一工程合同
docs/features/              用户行为投影
docs/acceptance-cases/      验证合同投影
docs/report/                有时间属性的分析与证据
```

## 开始修改前

1. 在 Issue 中确认问题、期望行为和范围。
2. 阅读最近适用的 `AGENTS.md` 和对应 plan/feature/acceptance。
3. 检查工作树，避免覆盖无关修改。
4. 找到唯一 authority、输入、输出、状态变化和失败路径。
5. 判断是否需要先更新工程合同。
6. 选择最小、可独立验证的改动范围。

如果现有实现与 `docs/plan/` 冲突，默认修复实现；不要修改 plan 来迁就代码。

如果事实证明 plan 的核心判断不成立，请先在 `docs/report/` 写明：

- 当前契约及被证伪事实。
- 根因和可复现证据。
- 备选方案及其正确性、复杂度、性能和内存影响。
- 迁移、兼容、回退和验证方式。

未经维护者明确批准，不要修改主骨架、核心边界、核心对象关系、主能力轴或关键接口结构。

## 实现要求

### Authority 与边界

- `DocumentState` 是运行时文本的唯一 canonical authority。
- Source、Preview、磁盘文件和 worker snapshot 都是投影或外部事实，不是平级 authority。
- UI 只捕获动作和呈现结果；业务判断必须经过 typed intent 与 coordinator。
- 文件系统、Win32 和剪贴板只能从 approved adapter 进入。
- worker 不得直接修改 `DocumentState` 或窗口。

### Rust 与 unsafe

- `stickymd-core`、`stickymd-render` 必须继续 `#![forbid(unsafe_code)]`。
- 必要的 Win32 `unsafe` 只能位于批准的平台 adapter，并紧邻准确的 `// SAFETY:` 说明。
- 不要暴露可绕过 canonical mutation gateway 的可变文本引用。
- 生产路径中的非法输入应返回 typed error，而不是 `unwrap()`、`expect()` 或 panic。

### 性能与内存

- 优先使用清晰、复杂度可证明的标准库或轻量算法。
- 不在每次按键复制全文，不建立无界队列、历史或缓存。
- 长任务离开 UI thread；后台结果必须携带 generation，并丢弃过期结果。
- 优化必须包含真实瓶颈、before/after 测量和回归测试。
- 不为了少量内存或微基准收益破坏 IME、数据安全、authority 或可维护性。

### 依赖

新增或升级依赖前必须说明：

- 为什么标准库和现有依赖不能完成。
- 直接与传递依赖、启用的 features、运行时线程/内存/体积影响。
- 许可证、维护状态、RustSec/advisory 和工具链兼容性。
- 是否进入最终 `StickyMD.exe` 或只属于 dev tooling。

禁止架构清单见 [`AGENTS.md`](AGENTS.md)。引入禁止项需要 ADR、风险分析和维护者明确批准。

### 文档与测试

- 正式业务 module 必须有指向 `docs/plan/` stable anchor 的 `plan_ref`。
- 用户行为变化按 `docs/plan → features → acceptance → code` 顺序更新。
- 每个 Phase 必须维护 `tools/smoke/phase-XX.ps1` 和
  `docs/acceptance-cases/phase-XX.md`。
- 自动化 verdict 由 Rust CLI 持有；PowerShell 只做稳定薄入口或必要的 Windows 适配。
- 没有真正执行的人工项目必须保持 `NOT TESTED`，不能用一次性观察冒充 PASS。

## 验证改动

先运行最小相关测试，不要每改一行就启动完整 Release qualification。

常见定向入口：

```powershell
cargo test -p stickymd-core --locked
cargo test -p stickymd-render --locked
cargo test -p stickymd-win --locked
cargo test -p stickymd-smoke --locked
./tools/smoke/phase-05.ps1
./tools/smoke/phase-14.ps1 -Ci
```

将 Phase 编号换成实际拥有该改动的阶段。各 wrapper 的参数并不完全相同：Phase 0–11 不普遍
接受 `-Ci`，只有声明该参数的 wrapper 才能使用；不确定时先查看对应脚本顶部的 `param(...)`。

提交前至少执行：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
git diff --check
```

影响依赖、发布、跨 crate 边界或大量公共路径时，还应执行：

```powershell
cargo deny check
cargo build --workspace --release --locked
./tools/smoke/all.ps1 -Ci
```

GUI、真实输入法、托盘、物理鼠标、显示器拓扑、绝对性能和资源测试不应在同一交互桌面并发。
它们只在相关模块变化、候选冻结或维护者明确要求时运行，并使用对应验收矩阵记录证据。

## 提交与 Pull Request

### Commit

- 一个 commit 只表达一个完整意图。
- 不混入无关格式化、重命名、依赖升级或生成文件。
- 建议使用清楚的前缀，例如 `fix(core):`、`feat(render):`、`test(qualification):`、`docs:`。
- 不重写或清理与当前改动无关的历史和工作树。

### Pull Request 必须包含

- 关联 Issue 和已确认范围。
- 问题与根因，而不只是“改了什么”。
- 方案、关键不变量、复杂度及重要权衡。
- 修改的 authority、模块和用户可见行为。
- 实际运行的命令与 PASS/FAIL 结果。
- 尚未执行的人工验证和原因。
- 依赖、性能、内存、持久格式、平台行为与安全影响。
- 适用的截图、短视频、JSON receipt 或 benchmark，但不包含私人草稿。

维护者可能拒绝功能本身，即使代码质量良好；这通常意味着它不符合 StickyMD 的产品边界，
并不表示实现没有价值。

## Review 标准

Review 依次关注：

1. 正确性、数据安全和失败原子性。
2. 中文输入法与交互行为。
3. authority 是否唯一，层与模块是否高内聚低耦合。
4. 算法复杂度、分配、复制、缓存上限与 UI thread 工作量。
5. 依赖与平台边界。
6. 自动化和人工证据是否足以支持结论。
7. 文档是否与实际行为一致。

## 许可证

提交代码即表示你同意按仓库的 [MIT License](LICENSE) 提供该贡献，并确认你有权提交相关内容。
不要加入许可证不兼容的代码、字体、图片、测试 fixture 或其他资产。
