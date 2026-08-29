# 01_terminology.md - StickyMD 术语表

## Metadata

- `Layer`: Foundation
- `Status`: Governing Rule
- `Version`: 0.1.0
- `Last Review`: 2026-08-29
- `Scope`: 固定 StickyMD 核心术语的定义、权威来源、等价性边界与生命周期；全仓库文档与代码命名必须使用本表术语

每个术语包含四个字段：

- **Definition**：定义。
- **Authority**：该概念的唯一权威来源（谁说了算）。
- **Not equivalent to**：明确不等于什么（防止概念漂移）。
- **Lifetime**：生命周期（何时产生、何时消亡）。

---

## 产品与文件身份

### StickyMD

- **Definition**：一个 Windows 11 x64、以 Rust 为主体、无 WebView 的便携式 Markdown 桌面草稿纸程序。一个程序目录即一张便签。
- **Authority**：`docs/plan/02_positioning_and_scope.md` 的本体定义。
- **Not equivalent to**：Markdown IDE、通用 Markdown 编辑器、知识管理工具、文档管理器。
- **Lifetime**：进程启动至退出；其数据身份持久于程序目录。

### Program Directory

- **Definition**：包含 `StickyMD.exe` 的目录（canonical 化后的真实路径）。它是便签的身份边界与单实例判定单位。
- **Authority**：文件系统本身（canonical 路径哈希用于实例互斥）。
- **Not equivalent to**：进程名、EXE 文件名、当前工作目录（CWD）。
- **Lifetime**：持久。复制整个目录即产生一张新便签。

### Note Directory

- **Definition**：`<program-dir>/note/`，包含 `note.md`、`config.toml`、`images/`、`.trash/` 的运行时数据目录。
- **Authority**：文件系统；程序启动时若不存在则创建。
- **Not equivalent to**：Program Directory；也不是任何用户自定义的任意文件夹。
- **Lifetime**：首次运行时创建，随目录持久存在。

### Canonical Note

- **Definition**：`<program-dir>/note/note.md`，唯一的持久工作文档。
- **Authority**：程序未运行时，磁盘上的 `note.md` 是 durable canonical representation；程序运行时，DocumentState 是唯一权威工作状态，`note.md` 是其 durable projection。
- **Not equivalent to**：Preview、导出副本、临时文件 `note.md.tmp`。
- **Lifetime**：首次保存/创建后持久存在；外部删除时可由内存内容原子恢复。

---

## 文档状态

### DocumentState

- **Definition**：程序运行时的规范文档状态：文本内容、generation、saved generation、脏标记、换行风格、undo 管理、managed 资产引用计数。
- **Authority**：运行时唯一权威工作状态。UI 文本、Preview、磁盘文件都从它派生。
- **Not equivalent to**：UI 编辑控件里的文本、Preview 文本、磁盘文本。它们不得与 DocumentState 并列成为权威。
- **Lifetime**：进程生命周期内存在；重启后从磁盘（或恢复候选）重建。

### Document Snapshot

- **Definition**：DocumentState 在某一时刻的只读快照（如 `Arc<str>` 文本快照），供后台任务使用。
- **Authority**：派生物，无权威；仅携带产生它的 generation 标记。
- **Not equivalent to**：DocumentState 本身；快照不得被写回。
- **Lifetime**：从创建到后台任务完成/丢弃。

### Generation

- **Definition**：DocumentState 的单调递增版本号。每次 canonical mutation（edit、undo、redo、external reload、recovery replacement）递增；caret/selection/preedit 不递增。所有后台任务结果必须携带来源 generation。
- **Authority**：DocumentState。
- **Not equivalent to**：时间戳；generation 只表达文档版本顺序。
- **Lifetime**：进程生命周期；重启后重新计数（与磁盘 hash 联合识别状态）。

### Dirty

- **Definition**：`generation != saved_generation` 的保守状态，表示当前 canonical generation 尚未收到落盘确认；即使 undo 后文本字节碰巧与磁盘相同，也仍可保持 dirty，直到该 generation 被保存确认。
- **Authority**：DocumentState（generation 与 saved_generation 的比较）。
- **Not equivalent to**：Preview dirty（预览刷新标志是独立概念）。
- **Lifetime**：从一次修改开始，到该 generation 成功落盘结束。

### Saved Generation

- **Definition**：已成功原子落盘的最新 generation。
- **Authority**：保存流程完成回执；只有实际落盘的 generation 才能更新它。
- **Not equivalent to**：最近一次提交的保存请求。
- **Lifetime**：随每次成功保存推进。

### Durable Fingerprint

- **Definition**：`note.md` 某次已观察 durable bytes（含实际 BOM/换行字节）的 SHA-256。
- **Authority**：Execution Domain 对真实磁盘 bytes 的读取，或成功 atomic publish 的确切输出 bytes。
- **Not equivalent to**：规范化后的 DocumentState 文本 hash、mtime、watcher event 或 generation。
- **Lifetime**：启动 load、成功 save 或 external reconciliation 时更新；冲突未解决时不更新。

### External File Fact

- **Definition**：程序自身保存之外，`note.md` 在磁盘上发生的变化（外部编辑器修改、删除等）。
- **Authority**：重新读取的文件系统 bytes 与 Durable Fingerprint 比对；watcher 只提供检查 hint。
- **Not equivalent to**：程序自己的原子替换事件（必须被识别并忽略）。
- **Lifetime**：从被观测到，到经 reconcile 流程进入 DocumentState 或被判定为自身写入而忽略。

### Conflict

- **Definition**：外部文件变化发生时 DocumentState 为 dirty 的冲突状态。Autosave 暂停，等待用户选择“载入外部”或“保留本地”。
- **Authority**：ConflictCoordinator（Flow Coordination 层）。
- **Not equivalent to**：保存失败（Failed）；冲突是等待用户决策，失败是执行错误。
- **Lifetime**：从检测到外部脏变化，到用户做出选择并完成相应动作。

### Recovery Candidate

- **Definition**：启动时发现的、合法 UTF-8 且比 `note.md` 更新的 `note.md.tmp` 内容，供用户选择是否恢复。
- **Authority**：用户选择。在用户选择前不得覆盖任何文件。
- **Not equivalent to**：已恢复的 DocumentState；候选只是候选。
- **Lifetime**：启动检测时产生，用户选择后消亡。

---

## 图片资产

### Managed Asset

- **Definition**：位于 `note/images/` 或 `note/.trash/`、文件名严格匹配
  `stickymd-<20|32|64-lowercase-hex>.<png|jpg|webp|gif>`，且文件实际
  SHA-256 与文件名 hash 前缀一致的普通图片文件。
- **Authority**：受控 canonical 目录 + 严格文件名语法 + 内容 hash 前缀一致 +
  非 reparse 普通文件共同构成所有权证明；仅有名称或位置绝不授权自动移动/删除。
  “是否需要存在”的真相来自 DocumentState 中的保守引用计数。
- **Not equivalent to**：User Asset；文件系统里存在 ≠ 应该存在。
- **Lifetime**：粘贴写入时创建；引用归零进入 trash；确认无引用后被物理删除。

### User Asset

- **Definition**：用户手工放入 `note/images/` 的文件，以及任何虽然外观像 managed
  名称、但无法通过完整所有权证明的文件。
- **Authority**：用户。程序只能显示与导出复制，永不自动删除、移动或重命名。
- **Not equivalent to**：Managed Asset；即使扩展名相同。
- **Lifetime**：由用户决定。

### Trash Asset

- **Definition**：位于 `note/.trash/` 的 managed 文件，处于逻辑删除状态，等待安全物理删除。
- **Authority**：AssetCoordinator 的事务状态；启动时以最新 DocumentState 引用为准决定去留。
- **Not equivalent to**：已物理删除；trash 中的文件可被 undo 恢复。
- **Lifetime**：从 move-to-trash 到确认无引用后的物理删除，或被恢复到 `images/`。

### Ownership Proof

- **Definition**：允许 StickyMD 自动移动/删除某个文件前必须同时成立的 canonical
  managed 目录、严格名称、非 reparse 普通文件和实际 SHA-256 前缀一致证明。
- **Authority**：Execution Domain 的 managed storage adapter；证明失败即视为用户/不可信文件。
- **Not equivalent to**：文件名匹配、路径字符串匹配或 Preview 成功解码。
- **Lifetime**：每次 destructive operation 前重新建立，不作为永久授权缓存。

### Asset Reference

- **Definition**：canonical DocumentState 文本中出现的严格 managed basename 字面量及其计数。
- **Authority**：DocumentState 的保守 reference tracker；宁可 false positive 保留，禁止 false negative 删除。
- **Not equivalent to**：Preview AST Image node；code/raw 中的字面量也会保守计数。
- **Lifetime**：随每个 canonical edit、Undo/Redo 或外部 reload 同步更新。

### Asset Reconciliation

- **Definition**：从最新 canonical reference set 推导并收敛 managed 文件在 `images/`、
  `.trash/` 或物理删除状态的过程。
- **Authority**：AssetCoordinator 决策 + 单 I/O worker 执行 + 每次操作 Ownership Proof。
- **Not equivalent to**：目录清空或通用 GC；用户/不可信文件永不参与 destructive path。
- **Lifetime**：运行时只做逻辑 move；startup 与成功 normal exit 只有在 durable note
  指纹匹配、稳定 note 句柄仍有效且 durable/runtime 引用并集确认无引用的 safe boundary
  才允许 proof-gated 删除。不确定状态一律延后物理删除。

### Export Snapshot

- **Definition**：用户触发导出时从当前 DocumentState 捕获的 immutable text/generation/line-ending projection。
- **Authority**：只定义该次导出的输入，不成为新的工作文档 authority。
- **Not equivalent to**：最近落盘 note、Preview tree 或“另存为”后的 active document。
- **Lifetime**：一次 export job，完成或失败后释放。

---

## 视图

### Preview

- **Definition**：DocumentState 快照经 Markdown/数学渲染后的只读投影。
- **Authority**：无。Preview 永远只是派生 projection，不得反写文档。
- **Not equivalent to**：Document；Preview 文本不得成为编辑或保存依据。
- **Lifetime**：一次渲染结果，被新 generation 结果原子替换或随隐藏清理。

### Source View

- **Definition**：直接显示并编辑原始 Markdown 的视图。
- **Authority**：呈现层；其编辑结果必须通过 EditText intent 进入 DocumentState。
- **Not equivalent to**：DocumentState；视图是呈现，状态是权威。
- **Lifetime**：视图模式切换期间。

### Preview View

- **Definition**：只读渲染视图。支持选择、复制、滚动、点击允许的链接。
- **Authority**：同 Preview。
- **Not equivalent to**：可编辑视图；Preview View 不接受文本输入。
- **Lifetime**：视图模式切换期间。

### Split View

- **Definition**：源码与预览固定 50/50 并排的视图。分隔线不可拖动；两侧各自保存滚动位置，并提供默认开启、可关闭的语义滚动同步。
- **Authority**：同 Source/Preview；两个面板呈现的是同一 DocumentState。
- **Not equivalent to**：两个独立文档。
- **Lifetime**：视图模式切换期间。

### Split Scroll Sync

- **Definition**：Split 中由当前滚动手势所属面板单向驱动的 source-range 语义对齐；不使用两侧原始滚动百分比。
- **Authority**：Runtime Config 中的 `split_scroll_sync` 只决定是否启用；实际锚点来自当前 Document generation 的 Source/Preview projection。
- **Not equivalent to**：Document selection、双向反馈循环、两个面板共享同一个 scroll offset。
- **Lifetime**：配置跨进程持久化；一次同步锚点只在对应 generation/layout 有效。

---

## 窗口与显示器

### Docked

- **Definition**：窗口吸附到屏幕左/右/上边缘的状态。收起时仅保留 3 DIP 感应条。
- **Authority**：WindowState（DockState），durable projection 为 config.toml 的 dock 字段。
- **Not equivalent to**：被最小化；dock 窗口仍然存在并可被 hover 展开。
- **Lifetime**：从吸附到被拖离边缘超过阈值。

### Collapsed

- **Definition**：dock 窗口的收起形态：主体缩出屏幕，保留 3 DIP 感应条。
- **Authority**：WindowState。
- **Not equivalent to**：Hidden to tray（完全不可见、仅托盘可达）。
- **Lifetime**：失焦超时/Esc/手动收起后开始，hover 展开或取消 dock 时结束。

### Floating

- **Definition**：未吸附的自由窗口状态。
- **Authority**：WindowState；durable projection 为 config 中的 relative position。
- **Not equivalent to**：Docked；拖离边缘超过阈值才成为 Floating。
- **Lifetime**：窗口存在期间与 Docked 互斥切换。

### Monitor Identity

- **Definition**：显示器的稳定身份标识，优先使用 Windows 显示配置设备路径的稳定哈希。
- **Authority**：平台 adapter 的显示器枚举结果。
- **Not equivalent to**：显示器绝对坐标、枚举顺序、设备名（都可能变化）。
- **Lifetime**：持久；显示器不存在时窗口恢复到主显示器。

---

## 配置

### Runtime Config

- **Definition**：运行时的配置状态（ConfigState）：theme、opacity、content zoom、split scroll sync、always on top、view mode、窗口布局等。
- **Authority**：运行时 ConfigState 是当前配置权威。
- **Not equivalent to**：config.toml 文件本身。
- **Lifetime**：进程生命周期；启动时从 durable config 载入（损坏则默认值）。

### Content Zoom

- **Definition**：Source、Preview 与 Split 内容区共享的整数缩放偏好，范围 50–300%，默认 100%。
- **Authority**：Runtime Config 中的 `content_zoom_percent`；UI、source projection、Preview、数学与图片只消费该值。
- **Not equivalent to**：Windows DPI、窗口尺寸、Document generation、Markdown 语义或 Shell 控件缩放。
- **Lifetime**：进程内由 ConfigCoordinator 提交更新，持久投影到 Durable Config；重启后恢复。

### Source Search Session

- **Definition**：当前 Source 文档的纯文本查找/替换会话，持有 query、replacement、大小写选项、generation-bound match ranges 与 active match。
- **Authority**：无；它只读取 DocumentState projection，并通过 typed edit intent 请求替换。
- **Not equivalent to**：另一份文档文本、正则表达式引擎、跨文件搜索索引。
- **Lifetime**：进程内的 Editor Session；关闭查找控件后可丢弃，Document generation 改变时旧 match ranges 立即失效。

### Durable Config

- **Definition**：`note/config.toml`，Runtime Config 的 durable projection。仅在明确提交点（slider 释放 / Enter / 失焦等）原子写入。
- **Authority**：启动时作为初始来源；运行期间不是权威。
- **Not equivalent to**：实时配置镜像；拖动 slider 过程中的中间值不写盘。
- **Lifetime**：持久；损坏时被改名保留并以默认值启动。

---

## 发布资格化

### Source Freeze

- **Definition**：从 clean worktree 建立的发布源身份，至少绑定 source commit、workspace version、
  `Cargo.lock` SHA-256、目标平台与资格化 harness。它允许 source-only CI、依赖治理和本地构建
  preflight 在最终发布字节产生前执行。
- **Authority**：clean Git HEAD、受控 manifest/lock 与 Rust smoke CLI 生成的 ignored receipt。
- **Not equivalent to**：Release Exact Artifact、Promoted Candidate、本地 `target/release` 输出或发布授权。
- **Lifetime**：从 clean HEAD freeze 到 source/manifest/lock/release tooling 发生变化；变化后旧 Source Freeze
  与其动态决策投影全部 stale。

### Local Preflight Build

- **Definition**：由当前 Source Freeze 在本机生成的 Release/ZIP/SBOM，用于尽早验证源码、构建、包结构、
  native runtime import 与工具合同。
- **Authority**：仅对该次本地 preflight 结果成立；它不拥有最终发布 artifact 身份。
- **Not equivalent to**：Release Exact Artifact；相同 source commit 的独立 Windows linker build 也不得据此
  假定逐字节相同。
- **Lifetime**：一次 preflight；Source Freeze 变化或产物被替换后失效。

### Release Exact Artifact

- **Definition**：由批准 Source Freeze 的 GitHub `release` workflow 构建、校验并上传的单一 Windows x64
  artifact 集合，包含 portable ZIP、`SHA256SUMS.txt` 与 `SBOM.spdx.json`。它是拟发布字节的唯一来源。
- **Authority**：成功 workflow run/attempt 的 artifact id，加上下载后对 checksum、SBOM、包结构、portable
  runtime 与成员 hash 的重新验证。
- **Not equivalent to**：同 commit 的本地 build、source equivalence、可复现构建证明、tag/draft/publish。
- **Lifetime**：workflow artifact 产生后存在；只有经 Promote 后才能成为本地 exact qualification 的输入。

### Promoted Candidate

- **Definition**：已下载并通过完整 artifact 自校验后，被原子复制到 ignored canonical staging
  `dist/exact-candidate/` 的 Release Exact Artifact。candidate receipt 绑定 source SHA、workflow run/attempt、
  artifact id/name、ZIP/EXE/SBOM SHA-256，且不记录机器绝对路径。
- **Authority**：Rust qualification promotion transaction 与 canonical staging 中重新校验的字节。
- **Not equivalent to**：Local Preflight Build、任意 downloaded ZIP、remote workflow 仅成功、source-only receipt。
- **Lifetime**：从成功 Promote 到任何 source/manifest/lock/release tooling 变化或下一次 Promote；替换 candidate
  会使所有旧 artifact-bound receipt stale。

### Source-bound Evidence

- **Definition**：只依赖 Source Freeze 的 fmt、Clippy、headless tests、portable-core、dependency-policy 等证据。
- **Authority**：与 Source Freeze identity 匹配的可重复命令或 CI receipt。
- **Not equivalent to**：最终 EXE/ZIP 的运行、性能、资源、桌面或人工验收证据。
- **Lifetime**：在 Source Freeze 不变且相应 harness contract 未改变时可复用。

### Artifact-bound Evidence

- **Definition**：必须针对 Promoted Candidate 的确切 EXE/ZIP 产生的 package/runtime、Performance、Resources、
  G3/G4/G5 与人工验收收据。
- **Authority**：统一 candidate resolver 返回的 canonical staged ZIP/EXE 与包含其 hash 的 receipt。
- **Not equivalent to**：Local Preflight Build 结果、旧 candidate receipt 或 source-only CI。
- **Lifetime**：只绑定一个 Promoted Candidate；任何 candidate identity 变化立即 stale。

---

## 架构层

### Interaction Shell

- **Definition**：第一层。窗口、视图呈现、输入捕获、托盘、控件、动画。唯一职责是转译 + 呈现。
- **Authority**：无业务权威；只表达用户动作与呈现状态。
- **Not equivalent to**：业务逻辑所在地；不得做保存决策、GC 判断、冲突决策。
- **Lifetime**：进程生命周期。

### Instruction Interface

- **Definition**：第二层。将 UI 动作转为 typed intent（EditText、Undo、SaveNow、Export、SetViewMode 等），校验合法性并映射为状态变化请求。
- **Authority**：intent 的合法性校验。
- **Not equivalent to**：执行者；它不直接改状态、不碰文件系统。
- **Lifetime**：随每个用户动作。

### Flow Coordination

- **Definition**：第三层。Save / Preview / Asset / Conflict / Recovery / WindowDock / Lifecycle 各 coordinator：负责顺序、依赖、失败路径与状态推进。
- **Authority**：流程编排决策；但不直接绕过 Execution Domain 操作资源。
- **Not equivalent to**：Execution Domain；coordinator 不接触磁盘/解析器等具体能力。
- **Lifetime**：进程生命周期。

### Execution Domain

- **Definition**：第四层。具体执行能力：Markdown 解析、数学排版、文本整形、光栅化、文件 I/O、原子替换、资产移动、剪贴板、文件监听、显示器查询、平台窗口适配、Shell 启动、配置序列化。
- **Authority**：执行结果的正确性；环境依赖一律通过 adapter 进入。
- **Not equivalent to**：决策者；它执行被请求的能力，不决定业务流程。
- **Lifetime**：按调用。

### Object Plane

- **Definition**：对象层（不是第五调用层）。系统实际操作的最小数据元对象的集合：`doc::text`、`preview::owned_ast`、`math::display_list`、`asset::managed_image`、`config::runtime`、`window::placement`、`file::note_md` 等。
- **Authority**：各对象的权威在 `docs/plan/04_runtime_state_model.md` 中逐一指定。
- **Not equivalent to**：一个调用层级；对象层被第四层操作、被第三层间接推进。
- **Lifetime**：按对象各自定义。

### Adapter

- **Definition**：外部环境依赖进入系统的兼容层/适配层（Win32、剪贴板、文件系统原子替换、显示器枚举等）。所有平台细节只能存在于 adapter 之后。
- **Authority**：平台行为的封装；核心逻辑不直接绑定环境细节。
- **Not equivalent to**：业务层；adapter 不含产品决策。
- **Lifetime**：进程生命周期。

---

## 治理

### Architecture Change

- **Definition**：对主骨架、核心边界、核心对象关系、主能力轴或关键接口结构的修改。必须先提交分析报告并获 USER 批准。
- **Authority**：USER 批准 + `docs/plan` 落盘。
- **Not equivalent to**：模块内实现迭代（允许）；接口签名小改（视影响判定）。
- **Lifetime**：审批通过后成为新契约。

### Implementation Drift

- **Definition**：代码与 plan 契约不一致的状态。默认判定为代码的问题，而非修改 plan 迁就代码。
- **Authority**：`docs/plan` 为准。
- **Not equivalent to**：plan 被证伪（后者需要报告 + USER 批准纠偏）。
- **Lifetime**：发现后应立即在下一次变更中收敛。

---

## 重点防漂移声明

以下等式**永不成立**，任何文档与代码都不得隐含它们：

```text
Preview != Document
磁盘文件 != 运行时工作状态（运行期间 DocumentState 才是权威）
images/ 目录内容 != 资产权威（权威是 DocumentState 的引用状态 + 可证明的所有权）
UI 状态 != 业务权威（UI 只是呈现与转译）
```
