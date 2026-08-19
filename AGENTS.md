# AGENTS.md — StickyMD Agent 总入口

> 你正在实现 StickyMD：一个 Windows 11 x64、以 Rust 为主体、无 WebView 的桌面 Markdown 草稿纸。
> 它不是 Obsidian，不是 Typora，不是知识管理工具，也不是通用 Markdown 编辑器。
> 它只做一件事：打开即写、自动保存、公式可靠、贴边即隐、需要时迅速出现。
>
> 你的首要任务不是增加功能，而是在数据安全、输入法正确性、输入延迟、内存占用与
> Markdown/数学渲染正确性之间，保持最小且可验证的实现。

---

## Purpose

StickyMD 是一个极致轻量、常驻 Windows 11 桌面的便携式 Markdown 临时草稿纸。

- 一个程序目录就是一张便签的身份边界。
- 首版永远只编辑一个 canonical working note：`<program-dir>/note/note.md`。
- 纯 portable：解压即用，无安装器，不写 AppData / Registry。
- 核心本体对象只有：Note、Document Text、Preview、Managed Image Asset、Runtime Config、Window Placement、Editor Session。

当前仓库状态：**治理与架构文档初始化阶段**。尚无任何运行时功能实现。
任何未写入 `docs/plan` 契约的产品功能，默认禁止实现。

---

## Authority Order

本仓库的架构真相源按以下优先级排列，高优先级约束低优先级：

```text
Engineering Constitution（docs/plan/00_engineering_constitution.md）
    ↓
docs/plan/（authoritative engineering contract）
    ↓
docs/features/、docs/acceptance-cases/、docs/overview/（projection）
    ↓
code
```

规则：

1. `docs/plan/` 是工程骨架与工程合同的唯一权威文档树。
2. `docs/features/` 是用户可见行为的投影，不得重新定义架构。
3. `docs/acceptance-cases/` 是验证合同的投影，不得发明产品需求。
4. `docs/adr/` 记录决策历史（只解释 why），不凌驾于当前 `docs/plan/`。
5. `docs/report/` 是有时间属性的分析证据，不是长期权威。
6. `docs/reference/` 是外部技术参考，永远不得覆盖 plan。
7. 代码不得反向成为架构真相源。实现必须遵循：

```text
docs/plan → projection docs → code
```

不允许存在 `docs/plan/` 之外的“更权威”文档。

---

## Mandatory Agent Workflow

未来任何 implementation、bug fix、architecture review、dependency change、
code generation、refactor，必须依次执行：

1. 阅读最近适用的 `AGENTS.md`（目录越窄越优先）。
2. 阅读 `docs/plan/00_engineering_constitution.md`。
3. 阅读 `docs/plan/01_terminology.md`。
4. 找到对应的 plan chapter（`docs/plan/02..11`）。
5. 确认该契约的：boundary、authority、state transition、failure path、verification。
6. 阅读对应的 `docs/features/` 与 `docs/acceptance-cases/` 投影。
7. 判断是否需要修改 plan。
8. 若是骨架级改变：**停止实施**，先提交 `docs/report/` 分析报告并请求 USER 批准。
9. 只有 contract 清晰后才开始实现。
10. 实现后运行 targeted tests。
11. review boundary drift（是否越层、是否引入平级权威）。
12. 最后运行适用 baseline（fmt / clippy / tests / 相关 benchmark）。
13. 不得 push remote，除非 USER 明确要求。

---

## Architecture Change Rule

> 已有代码与 plan 冲突时，默认判定为 **implementation drift**，
> 而不是修改 plan 迁就代码。

但是：

> 如果 plan 被事实证伪（例如依赖能力不存在、平台行为与契约假设不符），
> 必须创建分析报告（`docs/report/RISK-<topic>.md` 或
> `docs/report/<phase>-architecture-question.md`）并请求 USER 批准骨架修改。
> 未经批准，不得擅自修改主骨架、核心边界、核心对象关系、主能力轴或关键接口结构。

---

## File Cohesion

文件体量参考线（不是机械拆文件规则）：

```text
~250 手写行   = soft architecture warning（提示审视职责是否开始混杂）
~500 手写行   = hard review threshold（必须 review 是否应拆分）
```

测试文件可以合理超出。拆分的依据是职责边界，不是行数本身。

---

## plan_ref

未来正式 Rust 业务 module 必须在 module 文档注释中声明契约来源：

```rust
//! plan_ref: docs/plan/<chapter>.md#<stable-anchor>
```

规则：

- target 只能是 `docs/plan/` 下的章节及其 stable anchor。
- ADR 不作为 `plan_ref` target。
- 缺少 `plan_ref` 的业务 module 视为无契约实现，应在 review 中拒绝。

---

## Forbidden Architecture

以下方向在 v1 被明确禁止。引入任何一项都需要新 ADR + USER 明确批准：

```text
WebView / WebView2 / CEF / 任何浏览器引擎
Electron
Tauri
HTML/CSS 作为 UI 或预览渲染层
JavaScript runtime / Node.js
Tokio / async-std / 任何通用 async runtime
通用 GPU UI framework（iced、egui、slint、qt、gtk、sdl 等作为主框架）
数据库
runtime 网络 client（含远程图片下载、遥测、自动更新）
插件系统
跨层 filesystem 调用（绕过 Execution Domain adapter 直接操作磁盘）
业务逻辑写在 Interaction Shell 里
```

其他硬性禁令：

- 不自行实现 Markdown parser（语义由 Comrak 定义）。
- 不自行实现 TeX parser/layout（数学语义由 RaTeX 定义）。
- 平台无关 crate 禁止 `unsafe`；Win32 调用只能存在于经批准的平台 adapter 目录。
- 每个 `unsafe` block 必须有 `SAFETY` 说明。
- 所有文件写入必须原子替换，禁止 truncate + in-place write。
- 不静默吞掉保存错误。
- 不自动删除用户文件（只能删除可证明由 StickyMD 自己管理的 managed asset）。
- 后台任务结果必须携带 generation，过期结果直接丢弃。

---

## Agent Stop Conditions

遇到以下情况，停止扩展实现并记录 `docs/report/RISK-<topic>.md`，而不是绕过规格：

- 数学引擎无法覆盖关键公式。
- 窗口/文本库无法稳定处理指定输入法。
- 软件渲染 framebuffer 与 layered window 存在不可接受冲突。
- 原子保存不能满足数据安全要求。
- 新依赖许可证不兼容。
- 内存超过 hard gate 且无法定位原因。
- 实现需要引入 WebView 或 JS。
- 产品需求与 Non-Goals 冲突。

报告内容至少包括：重现步骤、根因、已尝试方案、数据、可选路径、对规格的影响。

---

## Directory Map

```text
AGENTS.md                  ← 你在这里
docs/AGENTS.md             ← 文档树职责说明
docs/plan/                 ← 工程合同（唯一架构权威）
docs/features/             ← 用户可见行为投影
docs/acceptance-cases/     ← 验证合同投影
docs/overview/             ← 可读架构投影
docs/adr/                  ← 决策历史（非权威）
docs/report/               ← 分析证据（有时间属性）
docs/tasks/                ← 阶段实施计划
docs/reference/            ← 外部技术参考
docs/coverage-matrix.md    ← plan ↔ feature ↔ acceptance ↔ code 对照
```
