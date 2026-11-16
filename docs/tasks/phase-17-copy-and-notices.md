# Phase 17 复制语义、复制侧条、提示条与文末留白

## Completion State

In Progress。2026-11-16 开工，分支 `feat/phase-17-copy-notices`。M1–M4 均未实现，
[验收矩阵](../acceptance-cases/phase-17.md)全部保持 `NOT TESTED`。

## Goal

按 USER 2026-11-14 批准的契约实现四项能力，按 M1→M4 的顺序落地：

- M1：提示条；
- M2：Preview 文末留白；
- M3：Preview 复制语义第一期；
- M4：复制侧条。

## Inputs

- **契约：**
  - [`02` #copy-what-you-see](../plan/02_positioning_and_scope.md#copy-what-you-see)；
  - [`04` #notice-state](../plan/04_runtime_state_model.md#notice-state)，以及 `SaveState` 的 `unresolved_failure`；
  - [`06` #preview-copy-semantics](../plan/06_markdown_math_rendering.md#preview-copy-semantics) 与
    [#preview-copy-rail](../plan/06_markdown_math_rendering.md#preview-copy-rail)；
  - `07` 复制反馈；
  - [`09` #notices](../plan/09_windows_shell.md#notices) 与 [#vertical-scrollbars](../plan/09_windows_shell.md#vertical-scrollbars)；
  - [`11` #phase-17-copy-and-notices](../plan/11_testing_and_release.md#phase-17-copy-and-notices)。
- **依据：** [提议报告](../report/2026-11-14-copy-semantics-rail-notices.md)。报告与 plan 不一致时以 plan 为准，例如：
  - 留白取 `R = 视口高 / 2`（plan 09），不取报告决议中的 `max(…)`；
  - 保存故障用 `unresolved_failure` 与 `FaultDisplay`（plan 04），不用报告中的“持续故障”层。
- **改动前基线：** `main` @ `0841524`。它的产品代码与 `b2b9aea` 相同，macOS 程序为 `2906076…`。
  每个里程碑的性能与内存比较都以它为“改动前”。

## Scope

| 里程碑 | 交付 | 验收行 |
| --- | --- | --- |
| M1 | 提示状态（定时栈与派生层）、保存故障、发出点归类、Shared Desktop 提示条与工具栏标记 | P17-N01..N20 |
| M2 | Preview 文末留白：绘制、滚轮、滑块、视图切换与 Split 同步共用一个末端函数 | P17-E01..E04 |
| M3 | Preview 复制映射与复制语义规则 1–6，帧持有源码快照 | P17-C01..C17 |
| M4 | 复制侧条：可见目标、命中、悬停、点击复制 | P17-R01..R06 |

每个里程碑完成时，同步改写 `phase-17.md`“落地时更新”中属于它的条款、验收行与测试。

### 实施决定（契约内的实现选择）

**M1 提示条**

1. **位置。** 提示状态放在 `stickymd-app` 的新模块，由 `AppSession` 持有。
   Shell 只读取一个提示视图（文字、类别、按钮、标记）来绘制，并把到期与点击交回。
2. **类别由 App 决定。** 每个发出点是一个带类型的提示，类别由 App 内一个穷尽的 `match` 决定。
   - Shared Desktop 与宿主只报告发生了什么，不决定类别（plan 04：Shell 只读）。
   - 删除四条字符串通道：`NoteEffect::Diagnostic`、`AssetStep::Diagnostic`、`ShellEffect::Diagnostic`、`PreviewFollowUp::Report`，
     以及 `DesktopShell.diagnostic`。这样由编译器列出全部发出点，P17-N09 不靠检索。
3. **`SaveState` 的落点。** plan 04 的 `SaveState` 是概念类型，代码由 `PersistenceCoordinator` 承担。
   - `unresolved_failure` 加在它上面。
   - 故障种类取 `FailureKind`（`Failed`、`Unavailable`、`Unsupported`），外加 `WorkerUnavailable`。
   - 回执 generation 不一致与无效回执没有 `FailureKind`，按 `Failed` 处理。
4. **手动保存。** 持久化工作线程合并 note 请求，回执后又以 `Debounce` 重新提交，所以触发方式不能随回执带回。
   - `PersistenceCoordinator` 另记一个待完成的手动保存（目标 generation），只在请求真正提交时设置；
   - 第一次覆盖到该 generation 的保存成功，显示“已保存”并清除；
   - 保存失败也清除它，失败由故障展示呈现。
5. **到期。** `AppSession::next_deadline` 纳入栈顶提示的期限。到期时按栈顶的 id 交回到期回执，状态机按 id 校验。
6. **持久化工作线程停止。**
   - 设置 `unresolved_failure`（`WorkerUnavailable`）。
   - 清除“保留本地”进行中、恢复的 `operation` 与待完成的手动保存。
   - 同时清除 `export_in_flight`。plan 04 只列了前三项；导出走同一个工作线程，不清除的话“正在导出”会一直显示。这是实施补充。
   - 之后的保存请求不再标记进行中，hide 与 quit 屏障立即按失败结算。
     其中用户主动选择的请求（Ctrl+S，以及 F6/F7 中需要持久化的选项）立即得到重要提醒“持久化不可用”。
7. **呈现。**
   - Shared Desktop 在 Search 浮层之后、工具栏之前，在窗口底部画 34 窗口 DIP 的提示条；文字用 Source projection 的 UI 文字绘制，与 Search 相同。
   - 删除 Source 的诊断条绘制与它在绘制键中的位置。
   - 提示变化只请求整帧重绘：重新 blit 已缓存的窗格像素，不触发 Source 绘制，也不提交 Preview 任务。
   - 光标的局部重绘区域与提示条相交时，改走整帧重绘。
8. **点击。**
   - 按钮命中后派发与 F6/F7 相同的决定；提示条其余区域的点击穿透到下面的窗格，与现状一致。
   - 工具栏标记的命中区为标记矩形向外扩展，点击开始一段新的故障展示。
9. **启动警告。** 宿主把全部启动警告交给 App，每条一个重要提醒；现状只保留最后一条。

M2–M4 的实施决定在各自开工时补入本节。

## Deliverables

- 代码与单元测试：`stickymd-app`、`stickymd-desktop`、`stickymd-render`，以及两个宿主的启动接线。
- `tools/smoke/phase-17.ps1`，由 `stickymd-smoke phase 17` 承载：
  - 测试任务：render、app、desktop 三个 crate；
  - 性能任务：Source、Phase 5 Preview、Phase 6 数学与 Phase 8 窗口的 Release 基线。
- 验收矩阵逐行填写证据；人工行在视觉验收前保持 `NOT TESTED`。
- “落地时更新”所列的 plan、features、验收行与测试改写。

## Verification

- 每个里程碑：
  - `cargo fmt --all --check`；
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`；
  - 相关 crate 的测试与 `cargo test -p stickymd-smoke`；
  - phase 00 与 phase 17。
- 性能：Preview 帧时间、Source 输入延迟与内存基准，与改动前比较，不得劣于它（plan 10）。
- Windows 宿主：`stickymd-win` 不能在 macOS 上构建。在 Windows CI 或 Windows 设备构建并测试之前，记为未验证。
- 视觉验收：macOS 在 `mac-test` 虚拟机中做，Windows 在宿主上做（P17-N12、P17-R06）。
- 审查：每个里程碑完成后做一次独立的跨模型审查（`codex-reviewer`），逐条核实其发现。

## Out of Scope

- 复制第二期：用结构自己的定界符只包住选中部分，另行批准。
- Source 文末留白（plan 09 决定不加）。
- 现有提示文字的改写：本阶段只改类别与呈现，文字保持原样（待决定提示去掉文字中的 `[F6 …]`、`[F7 …]`，由按钮承载）；新增的提示只有“已复制”“已替换 N 处”“已转换 N 处”“持久化不可用”。
- 单独选中 alt 为空的图片（需要新的选区表示）。
