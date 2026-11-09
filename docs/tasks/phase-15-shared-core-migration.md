# Phase 15 共享实现迁移

## Completion State

In Progress。A 的代码迁移与本机验证已完成；B 的笔记、预览、资产/导出/配置链已迁入 `stickymd-app`，隐藏已按请求、回执、提交执行，显示（R1）与置顶（R2）同样，见 R 记录；C 的 C1–C6 已完成（`stickymd-desktop` 已建立，纯叶子已迁入，`AppSession` 在 `stickymd-app`，壳经窄端口使用宿主并对宿主泛型，壳已是 `stickymd-desktop` 的 `DesktopShell<H>`，测试在 Linux 也运行）；D 的代码收口与跨模型审查修正已本机验证（见 D 记录）。显示、置顶、定位的回执见 R 与 R3 记录；单屏的本地桌面预检已全部通过（见“最终桌面回归”），回执的人工检查见“回执的人工检查”，共享裁决的移动见“里程碑 E”；失败路径的真实运行、多屏与混合 DPI 未做（见 Open Items）。

本文件与 phase-15 验收表引用的短提交号都是 squash 之前的：A、B 在 `origin/feat/phase-15-a-shared-app`（末端 `ae09b67`）；C、D、R 的末端是 `edbfdb9`，只保留在本地分支 `archive/phase-15-c-pre-squash`。

## Goal

把可共享的应用逻辑与界面逻辑集中到 `stickymd-app`、`stickymd-desktop`，Windows 只保留宿主与原生 adapter，为后续
Debian、macOS 宿主留出真实可用的共享层。Windows 产品行为与数据格式不变。

## Inputs

- [批准的提案](../report/2026-10-23-shared-core-thin-platform-shell-architecture.md)（USER 批准 2026-10-31）。
- [`03` 共享实现节](../plan/03_system_architecture.md#shared-crates)、
  [`11` Phase 15 节](../plan/11_testing_and_release.md#phase-15-shared-core-migration)、
  [`10` 迁移性能判定](../plan/10_performance_reliability.md#phase-15-migration-gates)。

## Scope

| 里程碑 | 状态 | 内容 |
| --- | --- | --- |
| A | 本机验证完成 | `stickymd-app`；`instruction/`、`flow/`（含 `flow/window/`）、配置模型、`interaction/`、`source_search.rs` |
| B | 本机验证完成：笔记、预览、资产/导出/配置链与隐藏回执；显示（R1）、置顶（R2）、定位（R3）的回执见 R 与 R3 记录，真实桌面观察未做（见 Open Items） | 执行接口与应用运行时；窗口动作的请求、回执与提交；入口测试见 plan 11 |
| C | 本机验证完成：C1 帧呈现接缝、C2 crate 与登记、C3 控件、滚动、Search、按键表与纯命中、C4 `AppSession`、C5 宿主窄端口与泛型壳、C6 壳迁入 `stickymd-desktop/src/shell/`（`DesktopShell<H>`）；远程 CI 与桌面回归未做 | `stickymd-desktop`；通用控件、绘制、命中、事件翻译；各 target 的 backend features |
| E | 代码与自动化已完成，桌面回归见“里程碑 E” | 配置文本、启动恢复裁决、粘贴批次、导出规划与百分号解码在 `stickymd-app`/`stickymd-render`；存储事务仍在宿主 |
| D | 本机验证完成：转发删除、剩余裁决迁入 `AppSession`、`AppSession` 字段在产品构建中收窄、跨模型审查修正与补测；桌面回归未做 | 迁移剩余业务分支，删除仅为迁移保留的转发，最终审查 |

A 中 `config/storage.rs` 留在 `apps/stickymd-win`：它依赖 Windows 原子发布与文件系统，存储能力在 B 进入 Capability Port。
`apps/stickymd-win/src/config/mod.rs` 曾无状态转发 `stickymd_app::config`，D 已删除（`cc41df0`），宿主直接导入共享模型。B 后写入经 `ConfigStorage` 端口，读取与
迁移告警仍在启动路径。

## Deliverables

- `crates/stickymd-app`（`forbid(unsafe_code)`，不依赖 winit、softbuffer 或平台 API）：含 `interaction/rules.rs` 与 `session/`。
- `crates/stickymd-desktop`（`forbid(unsafe_code)`，可用 winit、softbuffer，不依赖原生 API）：`geometry.rs`、`title.rs`、
  `frame/`、`controls/`、`scroll/`、`search/`、`input/`、`preview/`、`host.rs`（`Host`、`EventSink`）、`shell/`（`DesktopShell<H>`，`handler.rs` 是 `ApplicationHandler`；`storage_conformance.rs` 是给宿主存储的回执行），feature `testing` 下的 `testing.rs`（`FakeHost`、`MemoryClipboard`、`PortableDiskPorts`、`storage_conformance!`）。CLI 模块 `desktop`，`phase 15` 的 `-p stickymd-desktop` 任务，边界检查 `verify_shared_desktop_boundary`。
- CLI 模块 `app`、Linux portable 选择、资格化指纹归属、`phase 15` 入口、[验收矩阵](../acceptance-cases/phase-15.md)。
- 治理：共享应用的平台类型、平台条件与平台依赖禁令；未归入产品根的源码拒绝；按名字过滤的测试任务静态核对。
- 迁移前基线：配置字节 golden、无操作启动与保存、热路径计数（基线提交 `b846e83`）。

## Verification

见 [phase-15 验收矩阵](../acceptance-cases/phase-15.md)。每批出口按 plan 11：治理随代码、测试选择与指纹、数据字节兼容与
回退、性能。

### 性能对比

基线提交 `b846e83` 与迁移后在同一台机器上、无其他编译任务时运行（Release，p95，单位 ms；数值受机器波动影响，只用于确认
没有退步）。硬门的计数项见 [P15-A07](../acceptance-cases/phase-15.md)，计数与基线相同。

| 基线项 | 基线 | A 后 | B 后 |
| --- | --- | --- | --- |
| 1 MiB 查找 | 14.94 | 12.16 | 15.6–21.0 |
| 窗口 reducer 100k | 4.87 | 3.81 | 3.77 |
| Source 1 MiB 末尾输入 | 1.73 | 1.73 | 1.31 |
| Source 1 MiB full resync | 87.0 | 58.8 | 90.4 |

B 后的查找偏高不是回退：`source_search.rs` 自 A 起未改；同一时段与迁移前提交 `03ba733` 交替运行 4 轮，迁移前 p95 为
18.4–27.0，迁移后为 15.6–21.0。硬门计数未变（`phase 15 --performance` PASS）。

### 每帧复制字节

真实窗口，900×600，DPI 1.0，Release，首个可用帧后退出；Preview 与 Split 多一帧等预览结果。基线 `ae09b67` 用临时补丁计数，此后由
`FrameLedger` 经 `STICKYMD_DIAGNOSTIC_FRAME_LEDGER` 输出。单位字节。C1（`3dcb345`）与 C2（`5cf03e8`）逐项与基线相同。

| 模式 | 清屏 | Source 拷贝 | Preview 拷贝 | 全量转换 | 全量呈现次数 |
| --- | --- | --- | --- | --- | --- |
| Source | 2,160,000 | 1,992,320 | 0 | 2,160,000 | 1 |
| Preview | 4,320,000 | 0 | 1,992,320 | 4,320,000 | 2 |
| Split | 4,320,000 | 1,942,512 | 973,520 | 4,320,000 | 2 |

C3 结束时（`6caf3d3`）这台机器的系统缩放是 150%，同一配置得到 1350×900 的窗口，所以 DPI 1.0 这张表没有在 C3 上重测。对照用的是同一时刻、同一 DPI 下重新构建并运行的
C3 之前的提交 `02d93a6`，下表每项两者相同，也都等于窗口与窗格尺寸算出的字节数。DPI 1.0 留到桌面回归复测。

| 模式（DPI 1.5） | 清屏 | Source 拷贝 | Preview 拷贝 | 全量转换 | 全量呈现次数 |
| --- | --- | --- | --- | --- | --- |
| Source | 4,860,000 | 4,482,720 | 0 | 4,860,000 | 1 |
| Preview | 9,720,000 | 0 | 4,482,720 | 9,720,000 | 2 |
| Split | 9,720,000 | 4,374,048 | 2,187,024 | 9,720,000 | 2 |

### 每次编辑的构建计数

同一 fixture（200 键、900×600 视口），`b846e83` 上用临时补丁加同样的视口与 worker 计数实测，迁移后由
`shell::hot_path_tests` 固定（C6 前在 `app::hot_path_tests`）。下表计的是键入提交给 worker 的任务；完成回执触发的任务与 IME、Search、交错 tick 见 D 记录的计数表。

| 模式 | 输入时快照 | 收敛时快照 | 保存提交 | Preview 提交 |
| --- | --- | --- | --- | --- |
| Source | 0 | 1 | 1 | 0 |
| Split | 0 | 2 | 1 | 1 |
| Preview | 200 | 1 | 1 | 200 |

### C2 记录

- `TOOLBAR_HEIGHT_DIP` 只剩一份（`f32`）：`geometry()` 仍是 `f32` 乘法与取整，`ControlLayout::new` 先无损转 `f64` 再乘。
  `GUTTER_DIP` 与 `geometry()` 一起进入 crate（设计未列，`geometry()` 依赖它），宿主的 `scrollbar.rs` 从 crate 取。
- `title.rs` 的 8 个测试里 6 个随类型迁入 crate。`the_title_has_one_native_writer` 与
  `the_notice_settles_at_the_end_of_each_event_pass` 读宿主源码，留在 `app/title.rs`（只在测试构建编译）；前者同时扫描 crate 源码。
- `#[cfg(test)]` 不跨 crate：宿主的测试要读 `QuitCancelNotice::settle_count` 并用 `every_effect()`，所以 crate 增加 `testing`
  feature，宿主只在 `dev-dependencies` 打开，产品构建不含。
- `age ≥ 2` 的整体呈现不影响 Windows：softbuffer 0.4.8 的 Windows 后端 `age()` 只返回 0 或 1（设计 W5），Windows 序列不变；
  其他平台另行建基线（plan 06、plan 10）。
- Linux 后端按设计 §3.1 写在 crate 的 target 表里：winit `x11`、`wayland`、`wayland-dlopen`，softbuffer `x11`、`x11-dlopen`、
  `wayland`、`wayland-dlopen`。`Cargo.lock` 新增 26 个第三方包（336 → 363，含 `stickymd-desktop`），未引入 gtk；这些包不进入
  Windows 构建。两个后端都在运行时加载系统库：WSL Debian 上 `pkg-config` 不可用时重新构建并测试通过，测试程序只链接 libc 与 libgcc_s，
  所以构建不需要 `-dev` 包；远程 ubuntu runner 未验证。`deny.toml` 的 Linux target 留待 D7。

### C3 记录

- 特征测试 `app::shell_pins_tests` 在搬迁前写成，留在宿主，整批不动，只随搬迁改导入；期望行在 `app/shell_pins/`，
  `STICKYMD_SHELL_PINS=bless` 重写，给目录则写出哈希背后的完整路由图，用于两个提交之间比较。
- 真实路径的接缝：`shell_view()` 取窗口尺寸与 DPI，测试里由 `headless_view` 提供；`headless_window_gesture` 决定原生拖动是否成功；
  `press_probe!` 只在测试构建存在，把按下停在它动手之前，所以路由图不产生效果。原先无窗口时壳处理函数直接返回，按下会落到它在真实窗口里到不了的分支。
- C3a：`ControlLayout`、`ControlState`、透明度输入与 `toolbar_paint` 进入 `controls/`（`layout.rs`、`state.rs`、`toolbar_paint.rs`、`tests.rs`），
  `winit::dpi` 保留。`opacity_input_selected` 是 `pub(super)`：只有同目录的测试读它。
- C3b：`scrollbar.rs` 整个是纯代码，原样进入 `scroll/`；窗口接线 `scroll_runtime.rs`、`scroll_application.rs` 留在宿主。
- C3c：`search_runtime.rs` 的布局与绘制进入 `search/layout_paint.rs`。设计说这个文件的 4 个测试随文件走，但其中一个要求机器上至少装有一个
  UI 候选字体，Linux 上没有，也不能在 crate 里写平台条件：它拆成两半，几何部分
  （`every_view_leaves_readable_fields_and_reachable_buttons`）随 crate，标签宽度
  （`every_button_label_fits_in_each_installed_ui_font`）留在宿主只在测试构建编译的 `app/search_runtime.rs`，`cosmic-text` 不进 crate。
- C3d：`escape_action` 与 `is_escape` 进入 `input/escape.rs`，读状态、应用结果的 `handle_escape` 留在宿主的 `app/escape.rs`。
  `input.rs` 的五个函数连同 `ClipboardAlias`、`ZoomCommand`、`WindowKey` 进入 `input/keymap.rs`，签名与表不变；`Keymap` 字段是函数指针，
  `Host` 还不存在（C5），宿主现在用 `const KEYMAP: Keymap = Keymap::ctrl_standard()` 选择。`window_key` 直接调用另两个函数，换表时要一起换。
  `phase10_zoom_keys_and_high_resolution_wheel_are_deterministic` 拆成两半：按键随表走，`ZoomWheelAccumulator` 留在宿主，它的一半改名
  `phase10_high_resolution_wheel_is_deterministic`。
- C3e：`preview/input.rs` 收下不需要窗口的命中：`pane_local`（`preview_at_cursor` 与 `source_local_position` 原来各写一份）、
  `view_cursor`（`cursor_icon_for_position` 的判断）、`meaningful_preview_drag`。Preview 帧的“当前”判断仍惰性，只在 Preview 窗格上询问。
  `preview_command` 是按键而不是几何，留到事件翻译；`toolbar_mode_at_cursor` 留到下一步删除。
- 命中清理（D4）：删除 `toolbar_mode_at_cursor` 及其调用；`ControlLayout::new(mode, size, dpi)` 带上 `split_sync` 与缩放带，唯一的 `hit_test` 返回 `ControlHit`，
  悬停与按下共用。`resize_direction` 逐字进入 `controls/layout.rs`，没有采用设计示意里的 `client.contains` 前置判断：它会改变窗口外一列的路由（路由图钉着 `x=-1` 与 `x=W`）。
  `is_drag_region`、每次移动构造布局、`formula_error_under_pointer` 的第二次构造、绘制时的逐 ID 查找都已去掉。
- 缓存：`StickyApp::control_layout` 只有一份，`sync_control_layout()` 在窗口创建、`resize()`（DPI 变化也走这里）与 `apply_view_mode()` 三处重建，同在壳一层，没有失效标志，
  所以没有退回按需计算。`surface` 缩放失败时不重建：屏幕上还是旧帧，布局留在旧尺寸，按下落在看到的东西上；这是这个失败路径的行为变化，无窗口测试到不了，留给桌面回归。
- 分数坐标（设计的 125% 反例）：删除前 `(100,42.75)` 走旧分支切到 Split；删除后这一带 `y∈[42.5,43)` 没有任何东西接收按下，什么也不发生。路由图变化的只有这些行：
  125% 的 `press-centres`（`y=42.5` 一行，`x∈[9,153)` 从 `OldMode` 变为 `Nothing`）与 `press-edges`（24 个点，全是 `OldMode(Preview)` 变为 `Nothing`）。Windows 的鼠标位置是整数，真实按下到不了这里。
- Search 的像素只含无字体的标记路径：带文字的路径经机器字体，哈希会钉住机器。
- 路由图结论：整数坐标上旧的模式命中分支（设计 W2、D4）一次也没有被走到，含弹层与 Search 打开时；它只在 125% 的分数点 `y∈[42.5,43)` 可达，
  此时控件布局的工具栏到 42.5、视图的到 43，这一带既不是工具栏也不是窗格。

### C4 记录

- 特征测试 `app::phase15_session_tests` 在搬迁前写成，留在宿主：失焦保存、恢复时的监视、编辑门控的 32 行真值表（IME 与键盘各一张）、事件通过与缩放去抖、放置夹取、无事件循环的退出回退。
  设计说要确认失焦保存是否已有直接测试：没有，此处补上。
- C4a：`interaction/rules.rs` 收下设计列的纯规则，`MAX_WINDOW_DIP` 改为 `pub`，数值不变。`mutation_allowed` 有五个参数，不是设计里的四个 bool 加隐含状态：`preview_focused`、`shell_input_enabled`（W6）之外还要窗口协调器的 `accepts_editor_mutation`，没有协调器时不允许编辑。
  `a_view_without_a_source_pane_parks_search_and_one_with_it_resumes_it` 的决策表随规则走，几何核对留在宿主，改名 `only_preview_has_no_source_pane`。`recovery_pending_rejects_ime_commits_and_preedit` 等改成新签名，判断的事不变。`app/guards.rs` 只留读视图事实的 `window_guards()`。

- C4c（W7）：`Option<&ActiveEventLoop>` 不再穿过处理函数。托盘仍在退出效果处销毁，`event_loop.exit()` 在同一个 `ApplicationHandler` 回调末尾执行（`finish_callback`）。
  **无事件循环的回退分支随之消失**：它写“退出屏障完成，但当前事件没有退出权限。”并重新打开输入。产品里没有路径到得了它：`ExitProcess` 只来自配置屏障，配置屏障只来自配置回执、`WorkerStopped` 或资产屏障之后的配置刷写，资产屏障只来自同步回执与 `WorkerStopped`，这些都在带事件循环的 `user_event` 里；不带事件循环的调用方（按键、拖动结束、控件、守卫刷新）只发起保存或同步，回执之后才继续。所以 Windows 行为不变，只有无窗口 fixture 走过它。
  因此 `a_quit_sync_receipt_completes_the_asset_barrier` 的诊断断言换成退出已请求，这是本批唯一改动的回执行；`app::phase15_session_tests` 里钉住回退的特征测试同时删除，换成 `exit_comes_after_the_tray_is_destroyed` 与 `exit_does_not_appear_while_a_barrier_is_unsettled`。
- C4b：`app/session/` 原位建立，按簇搬字段（笔记、持久化、恢复、worker；资产与导出；Preview；配置、窗口流、Search），`session` 改名 `editor`，测试只改字段路径。`started` 换成 `Box<dyn MonotonicClock>`：
  宿主另存同一个 `Instant`（`clock_origin`），因为 `about_to_wait` 要把毫秒截止时间换回原生唤醒时刻，经 `now_ms()` 再加差值会迟到不足 1 ms。设计的 `AppSession::new(ports, config, clock)` 与代码不符：worker 在宿主用端口建好，这里收 `SessionParts`。
  `app` 字段放在原 `coordinator` 的位置，所以 worker 仍先于 `_instance` 释放。
- C4c 的顺序（设计的缺口）：设计说 `AppSession` 在内部消化 `RequestNoteSave` 等并把外发效果写进缓冲，壳随后按序应用。这样做会改行为：一个效果列表里，后面的会话步骤要看前面的壳效果做了什么。例如退出列表 `SetEditorInputEnabled(false)` 提交透明度输入，随后的 `RequestConfigFlush` 才读得到；`NoteBarriers` 的第二次派发要读第一次派发之后的守卫。
  所以缓冲不止放外发效果：会话把自己后续的步骤也按序放进去（`ShellEffect::Resume(SessionTurn)`），壳不看内容，到点交回 `AppSession::resume`，并带上此刻的 `ViewFacts`。设计的 `handle_io(c, out)` 等因此多一个 `view` 参数。
  嵌套的步骤先于外层剩余效果完成，与原来的同步调用一样：`ShellEffects` 是帧栈（`open`、`seal`、`next`），`with_session` 每次开一帧并排空。`ShellEffect` 也不像设计那样把 `ImeAllowed`、`CancelPreedit` 等拆成单独变体：窗口效果原样走 `Window(WindowEffect)`，拆开没有收益。
  `NoteEffects` 与 `AssetStep` 逐项进缓冲，`app/note_effects.rs`、`app/assets_runtime.rs` 已删除，宿主的解释层只剩 `app/shell_effects.rs`。`PreviewFollowUp` 没有并入：它借用 `preview_frame` 等视图状态，并入不减少层次。Preview 的提交留在宿主（视口是视图事实，worker 启动要 `EventLoopProxy`，C5 换成 `EventSink`），`ShellEffect::SubmitPreview` 把到期的动作交给它。
  `applied_window_effects` 只记到达壳的效果，被会话消化的不再记录；读它的行都不含这些效果。
  `the_notice_settles_at_the_end_of_each_event_pass` 读宿主源码，窗口 tick 进入 `AppSession::settle` 后改为检查 `app.settle(now_ms, out)`，判断的事不变。
- C4d：转发 impl 三个方法都转发，默认的 `read_paste` 会把宿主的图片粘贴变成文本。
- `AppSession` 的字段是 `pub`：宿主的壳代码与留在宿主的回执测试（W1）都直接读写它们。D 收口时可以再收窄。
- 帧计数：C4 结束时这台机器回到 100%，真实窗口 900×600，三种模式的逐项字节与上面 DPI 1.0 的基线表相同，C3 留下的 DPI 1.0 复测也由此补上；DPI 1.5 没有在 C4 上重测。
  `app::hot_path_tests` 的快照与构建计数未改，`session::tests::typing_does_not_grow_the_buffer` 钉住键入不增长缓冲。数据字节：`phase15_` 的字节测试与真实磁盘的回执行通过；回退重开同一副本（P15-B13）仍未做。
- 中间提交的绿色有两处缺口，没有改写历史：`ed3f7d1` 的 `clippy -D warnings` 有一个未使用的导入，下一个提交去掉；C4a 改了 `phase-08.md` 却没有更新 `coverage/sources.rs` 的审阅摘要，`53bf0a7` 到 `80bf22e` 的 `cargo test -p stickymd-smoke` 有两个 `coverage::sources` 测试失败，`45a9205` 补上。最终提交全部通过。

### C5 记录

- 八个提交：①`FatalNotice`、`ShellLauncher`、`ExportTargetPicker`；②`TrayHost` 与事件；③`MonitorSource`；④`WindowHost`；⑤`CaretOverlay`、`after_present`；⑥监视、按键表、单实例与会话装配；⑦`StickyApp<H: Host>`；⑧`FakeHost`。前六步壳持具体的 `WinHost` 经 trait 调用，原生调用清完才泛型化，这样 ⑦ 是纯机械改动。
- 顺序的一处调整：`TrayHost::create` 要 `EventSink`，所以 `EventSink`、`DesktopEvent`、`HostSignal` 在 ② 建立，`AppEvent` 同时退场，fixture 也从那时起不再需要 `with_any_thread` 的事件循环；唤醒监听也在 ② 改用 `EventSink`；⑥ 只剩监视、按键表、单实例 guard 与会话装配。
- 端口的签名与设计 §3.2 不同的地方，都按代码校正：
  - 错误类型。`Result<_, FailureKind>` 装不下今天诊断里的原生文本（托盘、置顶、透明度、工具窗口身份、链接、导出、监视、光标叠加），文本一变就是行为变化。这些方法返回 `HostFailure`（`StorageFailure` 的别名，同样是类别加宿主原文）。`hide`、`show`、`focus`、`visibility`、`apply_frame`、`observe_frame`、`snapshot` 仍是 `FailureKind`。
  - `ShellLauncher::open_target(target, kind)`：Windows adapter 要 `LinkKind` 复核；`note_dir` 在宿主。`ExportTargetPicker::choose_markdown_export()` 没有 `suggested_name`：对话框写死 `note.md`，没有调用方给别的名字。
  - `Host::start_watch(sink)` 没有 `dir`：`paths` 归 `WinHost`（§3.3），壳不知道目录。设计漏了确认：`notify` 的提示靠壳消费后放行下一个，所以多一个 `Host::acknowledge_watch`。
  - `Host::window_created(window)` 只交窗口句柄，不带 `sink`，不返回 `Result`。设计让它做托盘、工具窗口身份和初始不透明度，做不到：托盘要等显示器检查之后（否则“无可用显示器”时托盘闪现），不透明度与置顶要在 `set_visible(true)` 之后（winit 可能重写扩展样式）。这些仍按原顺序由 `initialize_window_shell` 逐个调用端口。
  - `WindowHost` 多两个带默认实现的方法：`reassert_identity`、`restore_ordinary_entry`。设计把重设身份放进 `show`、`hide`、`focus` 内部，但身份失败要写诊断，而 `hide` 的回执与它无关，塞进 `Result` 会把成功的隐藏报成失败。所以壳在每次 `show`、`hide`、`focus` 之后调用，失败时恢复任务栏并写诊断，顺序与原来相同；启动路径把失败当致命。
  - `after_present` 返回 `bool`：探针决定退出时壳调用 `event_loop.exit()`。`PresentFacts` 是 `width`、`height`、`view_complete`、`ledger`，帧计数要 ledger，`editor_ready` 由探针自己记。
  - 设计没有的两个窄 trait：`StartupProbe`（`milestone`、`detail`，壳走过的启动里程碑，名字就是稳定的 trace；`StartupDiagnostics` 归宿主后壳不能再直接写）与 `PreviewImages`（Preview worker 的图片来源，壳启动 worker，来源要 `note_dir`）。`Host` 自己的方法是设计列的七个加 `acknowledge_watch`。`StartupDetail` 枚举进 `host.rs`，下标与名字表不变。
  - `CaretOverlay::release` 有默认空实现：Windows 的叠加不持资源，壳在每次新像素覆盖之后调用。没有叠加的宿主（`caret_overlay()` 为 `None`）从构造起就走帧缓冲绘制，不写诊断；`WinHost` 总是 `Some`，窗口缺失时 `toggle` 报原来的“window is unavailable”。
- 留在壳里直接调用窗口的：IME（`set_ime_allowed`、`set_ime_cursor_area`）、光标、标题、主题、重绘、`drag_window`、`drag_resize_window`、最小化按钮、`inner_size`、`scale_factor`、`outer_position`、`outer_size`，以及启动路径的 `set_visible(true)` 与 `focus_window()`（`WindowHost::show`、`focus` 多一步取消最小化，启动不做）。
- `observe_frame`、`visibility` 与 `show`、`focus`、`set_topmost` 的读回本批没有调用方，留给 R 批。`complete_window_drag` 没有改用 `observe_frame`：它的 `display` 要枚举一次显示器，拖动结束就会比现在多查一次。
- 托盘：`TrayHost::update` 收整份 `TrayView`，壳记 `tray_view`，`WinHost` 只写变了的一项，值未变时不再调用 setter。现有调用方到这里时值都变了（`ApplyAlwaysOnTop` 只在配置改变时发出，隐藏与显示翻转可见性），所以实际的原生调用序列相同。工具窗口身份失败的启动路径原来靠局部变量在返回时销毁托盘，现在托盘归宿主，这条路径显式 `destroy()`，仍在 `event_loop.exit()` 之前。
- 释放顺序：`StickyApp` 里 `app` 声明在 `host` 之前，worker 先停，然后才轮到 `WinHost` 的监视、单实例 guard、托盘；`phase15_the_worker_stops_before_the_host_drops` 钉住这一点。`WinHost` 多持一份 `Arc<Window>`，窗口对象比原来晚释放（原来先于 worker）；winit 的 `Window::drop` 只是向已结束的事件循环投递销毁消息，进程退出时窗口才真正消失，两种顺序看不出差别。这一点读 winit 源码得出，没有桌面观察。
- 测试字段：`FakeHost` 只替换了 `headless_hide`。`applied_window_effects` 留下：它记的 `CancelImePreedit`、`SetImeAllowed`、`SetEditorInputEnabled`、`ReleaseHiddenCaches`、`RequestRedraw` 不经过宿主，顺序表要这些行；只有它的 `TrayDestroyed` 被 `HostCall` 取代。`headless_view` 留下：`inner_size`、`scale_factor` 是直接窗口调用。`headless_window_gesture` 留下：`drag_window` 是直接窗口调用。`press_probe` 留下：它把按下停在动作之前，与宿主无关。
- fixture：`HeadlessApp` 是 `StickyApp<FakeHost>`，没有事件循环，不再取单实例 guard；worker 与 `win_io_ports`、真实磁盘留在 Windows crate（W1），`paths` 放在 `HeadlessApp` 上，读它的行不变。回执表的改动只有三处设置（`headless_hide` 换成 `host.hide_result`）和一处路径（`app.watcher` 换成 `app.host.watching`），期望行未动；`app/shell_pins/` 没有改动。`exit_comes_after_the_tray_is_destroyed` 读 `HostCall::TrayDestroyed`，判断的事不变，托盘不再是 `None`。
- 新增 `app::phase15_host_tests`（18 个）：壳这一侧每个端口的调用顺序与每种答复的结果。`testing::tests`、`host::tests` 在 Linux 也运行。这些都只证明接口，不证明 `WinHost` 的原生实现；后者只有桌面回归能证明。
- 治理：`.with_decorations(false)`、`ControlFlow::Wait` 在 `app/lifecycle.rs`，`.with_undecorated_shadow(true)`、`CornerPreference::RoundSmall` 在 `host.rs`；`enumerate_active_displays()` 恰好一次与禁用词检查改指 `host/monitor_source.rs`。壳的非测试源码已无 `crate::platform`、`crate::startup`、`crate::preview`、`winit::platform::`，C6 可以机械搬家；`crate::config` 的转发留到搬家时改导入。
- 帧计数：本机 DPI 1.0，Release 的真实窗口 900×600（`95eddd4`），三种模式的逐项字节与上面的基线表相同；同一批运行的启动 trace 里程碑序列、`note.md` 与 `config.toml` 字节也与 C4 的运行相同。回退重开同一副本（P15-B13）、第二实例唤醒与真实退出路径仍未在桌面上跑。
- 回退重开（读路径）：`ae09b67` 加帧计数补丁的 Release 与 C4 的 Release 分别打开 `16028f2` 运行过的三份数据副本，退出码都是 0，27 行里程碑名序列相同，`note.md`、`config.toml` 字节不变。这些运行都没有写入，只证明读兼容；保存后再回退（P15-B13）留给桌面回归。trace 的时间与 cargo 构建重叠，不作证据，桌面回归前在空闲机器上再量一次启动。

### C6 记录

- 提交：a fixture 不再需要 Windows（`PortableDiskPorts`、`MemoryClipboard`、存储一致性行）；b 四个裁决原位移入 `AppSession`（视图切换、偏好写入、启动显示器与初次放置、Search 打开）；c 纯搬迁（`git mv`，`git diff -M` 下 44 个改名文件（壳 43 个与字体检查）共 169 行增、162 行删，都是导入、路径与 feature 门）；
  c 之后补一个提交把 `.gitattributes` 的 pins LF 规则跟到新路径（c 漏了，`core.autocrlf=true` 的检出会给 pins 加 CR）；d 改名 `DesktopShell`；e `shell/handler.rs`；f 按职责拆分；g 文档。
- 设计的两处更正（USER 决定 4、5）：没有 `run(el, host, session)`，它返回不了退出码，也少了 `sink`、`clock_origin`、`diagnostic`，还会改变失败路径；`main.rs` 仍是 `DesktopShell::new(...)`、`event_loop.run_app(&mut shell)`、`shell.host().startup_diagnostic_exit_code()`。`impl DesktopShell<H>` 留在 `shell/`，没有拆进 `scroll/runtime.rs`、`preview/view.rs` 等领域目录：那样私有字段都要变成 `pub(crate)`，纯叶子已在领域目录里。`main.rs` 的缩减（去掉 `mod app`、从 desktop 取壳）只能在 c 做，否则 c 编译不过，e 没有可再删的。
- W1 的更正（USER 决定 1、3）：回执表不留在 Windows crate。全部壳测试随代码进入 `shell/`；`rg "fs::|app\.paths"` 找到的 5 行（`restoring_publishes_the_temporary_and_then_removes_the_evidence`、`restoring_over_an_invalid_canonical_moves_it_aside_first`、`using_the_canonical_removes_the_temporary_without_a_save`、`using_a_missing_canonical_publishes_it_before_removing_the_temporary`、`keeping_local_forces_the_save_and_refuses_a_second_choice`）是 `shell/storage_conformance.rs` 的公开函数，只有 fixture 一行换成 `receipt_fixture_on(storage, ...)`，断言不变；`stickymd_desktop::testing::storage_conformance!` 每行展开一个 `#[test]`，名字不变。桌面在 `shell::phase15_receipt_tests` 用 `PortableDiskPorts` 展开，Windows 在 `persistence::phase15_storage_conformance` 用 `win_io_ports` 展开（模块名带 `phase15_`，所以 `phase15_` 名字过滤仍选中它）。`phase15_host_tests` 的导出行只把 `note_dir` 当目标路径，不读文件，不是一致性行。
- fixture：`AppSession` 由 `SessionParts` 直接构造，配置为默认且不写，诊断为 `None`（原来的 `BootstrapOutcome` 在没有告警时也是 `None`）。剪贴板换成 `MemoryClipboard`：原来是真实的 `ArboardClipboard`，查过没有测试复制、剪切或粘贴（没有 Ctrl+C/X/V、`CopySelection`、`PasteClipboard`、Search 的 Ctrl+V），图片粘贴的行直接给 `ClipboardPaste`。`PortableDiskPorts` 不声称原子性；守护保存用共享的 `guarded_publish_verdict`，被拒时留下临时文件，无效的正文移成 `note.invalid-<秒>.md`，与 Windows adapter 一致；它不限大小、不存粘贴图片、导出不复制资产、不编码配置（fixture 不写配置）。fixture 在宿主的测试构建里由 feature `testing` 编译，只给本 crate 测试用的部分留在 `cfg(test)`。
- 裁决（b）：
  - `apply_view_mode`：`begin_view_change` 判同一视图与 Search 组字拒绝，壳结束滚动条拖动后 `commit_view_change` 取消组字、发 `SplitModeChanged`、写配置、Source 时释放 Preview 投影与光栅缓存、决定 Preview 的提交、停放或恢复 Search、提交配置；视图部分由 `ShellEffect::ViewModeChanged`、`SubmitPreview`、`ImeAreaChanged`、`KeyboardOwnerChanged` 驱动，顺序与原来相同。拖动的结束必须在会话动手之前（它刷新的守卫读的是旧视图与组字），所以分两步而不是一个效果。
  - `dispatch_window_preference_intent`：配置何时写入（主题、置顶、Split 同步、提交的透明度立即写；预览的透明度不写；缩放等 250 ms）归 `AppSession::dispatch_window_preference`，壳经 `ShellEffect::Preference` 只做显示。缩放的截止时间仍在壳完成重排之后由 `defer_zoom_config` 取当前时钟，与原来同一时刻。唯一的顺序差别：配置任务现在在壳的显示步骤之前交给 worker，原来在之后；两者在不同线程，界面与文件内容都看不出差别。
  - `initialize_window_shell`：启动显示器的选择（配置的、主显示器、任一个；纯规则 `interaction::rules::window_display`，拖动结束也用它）与按配置的尺寸、位置、贴边建立窗口流、返回第一帧归 `AppSession::startup_display` 与 `start_window_flow`；托盘、身份、透明度、置顶与启动里程碑的宿主调用顺序不变，留在壳。
  - `open_search`：组字拒绝、开关与展开替换行、从 Preview 先切 Source、停放的 Search 回来后不重开、用选中文本打开都归 `AppSession::open_search` 与 `open_search_in_source_view`；壳只按 `SearchOpenStep` 关闭或先切视图。
  - `close_search` 没有裁决（无条件关闭，接着是固定的视图更新），留在壳，`SearchOpenStep::Close` 交给它。
  - `ShellEffect::Resume` 没有新增步骤。12 个新测试在 `session::decision_tests`，Linux 也运行。
- 拆分（f）：`input.rs` 分成键盘与 IME（`input.rs`）和指针与滚轮（`pointer.rs`）；`window_interaction.rs` 分出透明度输入框（`opacity_field.rs`，按键、IME、聚焦、提交与取消，连同它的两个纯判定测试）；`presentation.rs` 分出帧的组成（`render.rs`）。`window_interaction/runtime_tests.rs` 留在原处：它要读 `ime_allowed_for` 等私有方法，挪走就要放宽可见性。`mod.rs` 的 861 行里 610 行是内联测试，没有拆。
- 留在 Windows crate 的壳测试只有 `every_button_label_fits_in_each_installed_ui_font`（`apps/stickymd-win/src/search_label_fonts.rs`，原 `app/search_runtime.rs`）：它量的是这台机器装的 UI 字体，断言至少有一个候选字体（C3 记录），`cosmic-text` 仍是 Windows 的 dev 依赖。
- Linux：壳第一次在 WSL Debian 上编译并运行，`stickymd-desktop` 213 通过、2 个 ignored，与 Windows 相同；`shell_pins_tests`、`hot_path_tests` 与回执表都通过。没有测试只在 Linux 失败，也没有用 `cfg` 绕开，所以没有要找的字体根因；`shell_pins` 的 Search 像素本来只取无字体的标记路径（C3 记录）。
- 测试数（含 ignored，`--list`）：C6 前 app 187、desktop 72、win 272，共 531；C6 后 app 199（+12 裁决测试）、desktop 215（72 + 141 个迁入 + 2 个磁盘替身测试）、win 136（272 − 142 + 1 个字体检查 + 5 个一致性行），共 550。迁走的 142 个 `app::` 测试：141 个在 `shell::`（路径只把 `app::` 换成 `shell::`，f 之后 `input::wheel_tests` 在 `pointer::wheel_tests`，透明度输入框的两个纯判定在 `opacity_field::phase8_interaction_tests`，`lifecycle::tests` 的一个在 `handler::tests`，`presentation::tests` 的一个在 `render::tests`），1 个是 `search_label_fonts::every_button_label_fits_in_each_installed_ui_font`。受保护的五个表与 `8d3661d` 逐行比较（只有一致性行的 fixture 行与函数签名不同），pins 的 8 个文件哈希相同。
- 登记：两个 crate 的 `Cargo.toml`（desktop 加 dev 依赖 `unicode-segmentation`；win 去掉 `tiny-skia`、`softbuffer`、`unicode-segmentation`）与 `Cargo.lock`；工作区 crate 依赖图不变，`ci/registry.rs` 无需改；指纹的 `apps/stickymd-win/src/app.rs`、`app/` 两条规则删除，`shell/` 每个文件一条规则（新文件要有意识地归类），`search_label_fonts.rs` 与 `testing/` 各一条；测试所有者 `win("app::…")` 全部变成 `desktop("shell::…")`，`win` 构造函数随之删除，`OwnerTarget::Bin` 在非测试构建里不再被构造，加了带理由的 `allow(dead_code)`（核验仍支持二进制目标）；`phase3_source_pipeline_release_baseline` 的任务改选 `-p stickymd-desktop -p stickymd-win`；窗口标记 `.with_decorations(false)`、`ControlFlow::Wait` 指向 `shell/handler.rs`；两份报告里指向旧路径的链接目标跟到新文件。
- 释放顺序不变：`DesktopShell` 里 `app` 仍在 `host` 之前（`phase15_the_worker_stops_before_the_host_drops`），托盘在退出效果处销毁、`event_loop.exit()` 在同一回调末尾（`exit_comes_after_the_tray_is_destroyed`）。
- 未改的投影：phase-03、05、08、14 验收文件里带日期的补充段仍写当时的路径与命令（`app::…`、`-p stickymd-win`），C2–C5 搬迁时也没有改，它们是当时的记录；当前证据见本文件与 phase-15 矩阵。
- 帧计数（`c3fe217`，本机 DPI 1.0，Release 真实窗口 900×600）：三种模式的逐项字节与上面的基线表相同，也与 C5 的运行相同；27 行启动里程碑名序列、`note.md` 与 `config.toml` 字节与 C5 的运行相同。Release 大小 8,694,784 字节，C5 为 8,691,712（+3,072）。回退重开（P15-B13）、远程 CI 与桌面回归未做。

### 桌面回归的预期差异

C3 之后、D 之后的 Windows 回归（P15-C09）只允许下列可见差异。其余路由、光标、绘制都应与修复前相同。

**8a 选中背景不再画进缩放带。** 这些区域的按下本来就开始缩放，没有改变；变的只是选中背景（当前视图按钮、开启的置顶、跟随系统的主题、打开的透明度）。
图标位置不变，按钮区域其余部分不变。s 为 DPI，单位物理像素，区间半开：

| 布局 | DPI | 背景不再覆盖的上方行 | Source 按钮不再覆盖的左侧列 |
| --- | --- | --- | --- |
| 常规（宽度 ≥ 294 DIP） | 1.0 | `[3, 6)` | `[5, 6)` |
| 常规 | 1.25 | `[3.75, 7.5)` | `[6.25, 7.5)` |
| 常规 | 1.5 | `[4.5, 9)` | `[7.5, 9)` |
| 常规 | 2.0 | `[6, 12)` | `[10, 12)` |
| 紧凑（宽度 < 294 DIP，220 DIP 时） | 任一 | `[49s/9, 6s)`，约 0.56 DIP | 无（按钮从 6s 开始） |

看到的样子：选中按钮的背景比图标区矮一截，上沿离工具栏顶 6 DIP，不再贴近窗口上边。右侧没有差异：能被选中的按钮都离右边缘足够远。

**8b 工具栏空白显示普通箭头。** 工具栏里按钮之外的地方（按钮之间的间隙、中间的大片空白、按钮上下方的工具栏行）原来显示手形，现在显示箭头；按下仍是拖动窗口。
按钮上仍是手形，缩放带仍是缩放光标，弹层上仍是手形。

**8c 绘制不再用工具栏高度反推 DPI。** 只有 `34 × DPI` 不是整数的 DPI 有差异，100%、150%、200% 逐像素不变。125% 时工具栏高度取整为 43，反推出 1.2647，现在用真实的 1.25，三处亚像素变化：

| 125% 的绘制 | 修复前 | 修复后 |
| --- | --- | --- |
| Split 同步按钮（650 px 窗宽，分隔线 x=324） | `x=[310.088, 337.912)`，`y=[48.059, 75.882)`，边长 27.82 | `x=[310.25, 337.75)`，`y=[48, 75.5)`，边长 27.5，与命中矩形相同；内部图标按同一比例缩小 |
| Split 分隔线宽度 | 1.2647 px | 1.25 px |
| 右上角诊断条 | `x=W-12.65`，`y=8.85`，宽 5.06，高 25.29 | `x=W-12.5`，`y=8.75`，宽 5，高 25 |

命中与路由不变：`ControlLayout` 一直用真实 DPI。分隔线、诊断条的变化是这项修复的连带结果，因为它们读同一个比例。

### D 记录

- 提交：`cc41df0` 删除 config 转发；`08c6c91` 键盘缩放立即写入归会话、停放的 Search 改为私有；`e1082bf` 并行测试的临时目录加序号（`development::git::tests` 在 CI 分片上因同一时钟值撞名失败过一次，`create_dir` 报 OS 183；`tests/cli_exit.rs` 的同类辅助函数一并修）；`b57c677`、`28ef42e` 补计数；`8b81932` 包装错误的分类表；`eba939b` 只读 DPI；`aed2780` 启动资产边界；D2 的七个提交 `9a2387e`..`161834c`；`b7b5980` 注释。
- 跨模型审查（Codex，WSL Debian，`max`，只读）分四路，都无阻断：PR #10 的 `a96bdc1` 与 `ae09b67`；C+D 的性能与内存；C+D 的共享应用层；C+D 的宿主边界与测试替身。`ae09b67..28ef42e` 整段的架构审查两轮四次都只回一句确认：Codex 把约 1.2 MB 的 diff 读进上下文后被压缩（stderr 有 `context compacted`），任务随之丢失；按路径拆成两路、禁止整段输出 diff 后完成。下列发现都对照代码核实过。
- 性能证据（plan 10 的硬门没有授权任何排除）：
  - 完成回执触发的 Preview 任务原来不在 P15-A07 的计数里。`b846e83` 到 `43b1294^` 之间这条路径只改了导入，`43b1294` 把它改写为 `PreviewIo::on_result`，所以不能静态豁免。每键之后依次交给真实的完成处理函数三个结果（旧文本、当前文本换视口、当前文本同视口），基线与迁移后都是 600 个任务（P15-D02）。
  - IME、Search、交错 tick 原来没有计数，三个场景在基线与迁移后相同（P15-D03）。
  - 设计 §5 要求的 Source CaretOnly 从 M0a 起没有测过。两端各打临时补丁：原生光标叠加视为失败、首次软件局部呈现后输出计数并退出。基线 `ae09b67` 与 `28ef42e` 相同（P15-D04）。
  - 只需要 DPI 的五处（滚动条三处含每次指针移动、滚动条槽命中、Search 布局）在 C3 起经 `shell_view()` 多查一次窗口尺寸（winit 的 Windows 实现每次 `inner_size()` 调 `GetClientRect`），`eba939b` 改为只读 DPI；没有计数器，帧复制字节不受影响。
- 其余发现与处理：
  - 包装错误的分类测试漏了 `AssetStorageError::Directory`，改为三行表，每行断言分类与完整外层消息（`8b81932`）。
  - 显示回执须同时证明“可见且未最小化”只写在 B 的决定报告里，plan 04 与验收矩阵没有；显示回执在 R1 才实现，届时一并写入（Open Items）。
  - `main.rs` 决定启动时何时做破坏性资产整理并组织告警文本：移入 `stickymd_app::flow::reconcile_at_startup`，经 `ManagedAssetStore::sync` 的安全边界请求执行（Windows 适配器调用的仍是同一个 `reconcile_safe_boundary`），调用位置不变，`main.rs` 只装配（`aed2780`）。
  - 壳里剩下的裁决移入 `AppSession`：Search 的替换当前项与全部替换事务、缩放何时写配置（壳只报键盘或滚轮）、托盘显示/隐藏命令选哪个窗口意图、Preview 的调度、释放与监视提示（D2 1、2、3、5）。托盘的两个意图没有合并：`TrayToggleRequested` 还会覆盖 guards，合并会改变行为。
  - 视图切换的两步可以被伪造（`commit_view_change` 接受任意 Search 决定，含 `Refuse`）：`begin_view_change` 返回不可复制的 `PreparedViewChange`，`commit_view_change` 消费它（D2 4）。
  - 共享层的平台字样：“Windows 未提供可用显示器”改为“没有可用的显示器”。今天这句从不显示：窗口创建时不可见，启动在 `set_visible(true)` 之前返回并退出，诊断不输出。光标叠加矩形的 `i32` 换算错误不再写 Win32；`stickymd-app` 的两处注释改为宿主与桌面壳（D2 6）。`stickymd-desktop` 注释里描述真实平台行为的（softbuffer 的 Win32 后端、Windows IME 的按键投递、按键表）保留。
  - 视图切换的顺序（C6 记录未写）：会话内部的 Preview 显示判断、Search 停放与恢复、配置提交现在早于壳的重排（`ShellEffect::ViewModeChanged` → `lay_out_view`），原来在重排之后。重排只读视图几何、配置的视图、组字状态与投影；停放只在没有组字时发生，所以最终状态相同。配置任务提前交给 worker，与 C6 记录的偏好写入同类。Source 切换时 worker 先释放光栅缓存、壳随后清 Preview 帧，两者互不读取。
  - Replace Current 多一个会话步骤 `Turn::MatchReplaced { end }`：原来壳分三帧（改动的效果，其中 `DocumentAccepted` 重扫 Search；`activate_from(end)`；选中下一命中），在会话里直接调用会把 `activate_from` 提前到重扫之前，所以它排在改动的效果之后。C6 记录的“`ShellEffect::Resume` 没有新增步骤”指 C6 当时。
- `AppSession` 字段收窄（D2 7）：`editor`、`search` 是壳本来就编辑的交互状态，保持公开；其余字段只在 `cfg(any(test, feature = "testing"))` 下 `pub`，每个字段按原顺序声明两次（相反的 `cfg`），释放顺序不变。`stickymd-app` 新增 feature `testing`，`stickymd-desktop` 的 `testing` 转发它，只在测试依赖里打开。产品构建（`cargo check --locked -p stickymd-desktop`、`-p stickymd-win`，feature 关闭）里壳只能经 `session/access.rs` 的只读视图与会话方法；在生产文件里读 `self.app.persistence` 得到 `E0616: field persistence of struct AppSession is private`。要同时借用正文与可变状态的几处（Search 的 Ctrl+V、Preview 结果、Search 编辑与两处光标移动）由 `read_clipboard_text()`、`on_preview_result(...)`、`search_over_text()`、`editor_over_text()` 拆开借用，返回的 `&mut` 只指向两个公开字段。受保护的五个表、`storage_conformance.rs` 与 `shell/shell_pins/` 在 D 各提交逐字不变。
- 原有问题（基线 `ae09b67` 已如此，不在 D 修改，见 Open Items）：启动失败（无显示器、身份、透明度、置顶失败）以成功退出码结束，无显示器等原因不显示；这几条路径与诊断探针的退出不在 `event_loop.exit()` 之前销毁托盘，托盘随宿主析构；`FakeHost` 不记录显示器查询、窗口交接与启动里程碑，启动失败路径没有无窗口测试；启动失败的继续或终止仍由壳决定。

D 的计数（基线与迁移后同一 fixture；基线一侧在临时 worktree 里加临时补丁，不提交，用后删除）：

| 场景 | 基线 | 迁移后 | 结果 |
| --- | --- | --- | --- |
| Preview：200 键，每键后三个结果 | `b846e83`：600 个任务 | 600 | 相同 |
| Split：200 次 IME 组字并提交 | `b846e83`：快照 2、保存 1、Preview 1 | 2、1、1 | 相同 |
| Split：Search 打开时输入 50 键 | `b846e83`：0、0、0 | 0、0、0 | 相同 |
| Split：50 键，每键后截止时间都到期 | `b846e83`：100、50、50 | 100、50、50 | 相同 |
| Source CaretOnly，DPI 1.0（单进程 `__COMPAT_LAYER=DPIUNAWARE`），900×600 | `ae09b67`：整帧 1 次，局部拷贝 100 B、局部转换 100 B、局部呈现 1 次 | `28ef42e` 相同 | 每端 3 次，相同 |
| Source CaretOnly，原生 DPI 1.5，1350×900 | `ae09b67`：整帧 1 次，局部 304 B、304 B、1 次 | `28ef42e` 相同 | 每端 3 次，相同 |

CaretOnly 一行的整帧部分与 Source 整帧表相同。这台机器此时只接笔记本面板（150%），DPI 1.0 经系统按 DPI-unaware 拉伸，不等于真实的 100% 显示器；整帧对照在兼容层下与 DPI 1.0 基线逐项相同。
- 收尾（`b7b5980`）：测试数（含 ignored，`--list`）D 前（`75e50ed`）app 199、desktop 215、win 136，D 后 app 209、desktop 217、win 136；`all --ci --ci-shard=tests`、`phase 15 --performance` 通过；WSL Debian 的 `linux-smoke.sh`、`linux-app.sh`、`linux-desktop.sh` 退出码 0（desktop 215 通过、2 个 ignored，app 206、3，smoke 483 与 19，gtk 0）。`cargo check --locked -p stickymd-desktop`、`-p stickymd-win` 通过。帧计数：原生 DPI 1.5 的三种模式与上面的 DPI 1.5 表逐项相同；单进程兼容层下的 DPI 1.0 与基线表逐项相同；两组运行的 27 行启动里程碑名序列、`note.md` 与 `config.toml` 字节与 C5 的运行相同。Release 大小 8,689,664 字节（C6 为 8,694,784，C5 为 8,691,712）。
- 空闲机器上的启动（`b7b5980` 的 Release，新的便携目录，`STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY`，3 次）：`persistence_ready` 7.8–9.2 ms，`window_created` 36–44 ms，`editor_ready` 217–292 ms。trace 的单位是微秒，这就是 C5 记录留待空闲机器复测的那次测量，启动没有延迟问题。

### R 记录（R1 显示、R2 置顶）

- 提交：`7494f9b` plan 04 写明显示回执的后置条件，验收矩阵加 `NOT TESTED` 的桌面观察行；`0f27b27` R1；`da3eca5` R2。
- R1 显示。`show` 只发 `ApplyFrame` 与 `ShowWindow { id }`，状态进入 `ShowPending { id, restore }`；`restore` 是隐藏前记录的值，拒绝时原样回到 `HiddenToTray`。壳调用 `show()`，以 `Visibility::show_outcome` 映射成 `ActionCompleted`：`(Some(true), Some(false))` 成功；已知错误的读回（可见为假或最小化为真）是 `Failed`，即使另一项读不到；其余缺值是 `Unsupported`。成功提交 `Presented`（收起的恢复为展开）、托盘文字、`configure_viewports`、Preview 恢复（`WindowEffect::WindowShown`）、`FocusWindow { id }`、IME 与输入；拒绝只发 `ShowRejected(kind)`。焦点请求带 id，成功不提交任何东西（`focus_pending` 区分当前与过期回执），失败上报同样的 `ShowRejected`。真实壳下回执在同一次派发内同步到达，所以 `ShowPending` 只活在一次效果序列里；下表的裁决覆盖状态机的全部入口。
- R2 置顶（请求按来源分存的最初形态，已由 M2 改为单一顺序结算，见“审查修正”）。`flow/window/topmost.rs` 的 `TopmostState` 按来源分存确认值与待决值，`ApplyTopmost(TopmostRequest { id, desired_effective, source })` 带两来源的合成值；回执只提交自己来源的值（同一 `last_window_action` 计数，不同来源的 id 不混）。用户值的路径改为 `UserTopmostRequested` → 成功回执 → `CommitAlwaysOnTop` → `Turn` → `coordinate_window_preference`（写配置、提交写盘、托盘勾选）；失败不写，`TopmostRejected` 上报。感应值走同一请求形状，没有配置效果，`HidePending` 存的是确认值。`WinHost::set_topmost` 在 `SetWindowPos(NOACTIVATE)` 后读 `GetWindowLongPtrW(GWL_EXSTYLE) & WS_EX_TOPMOST`（沿用 `tool_window` 的 SAFETY 处理）；启动路径照旧调用 `set_topmost`，失败语义不变，读回值不看。
- 裁决（`flow::window::show_receipt_tests`、`flow::window::topmost::tests`）：

| 状态 / 动作 | 显示等待（`ShowPending`） | 置顶（按来源） |
| --- | --- | --- |
| 成功回执 | 提交全部依赖效果；已在退出时只记录可见性与 `WindowShown`，不请求焦点、不开输入 | 只提交该来源；用户值才进入配置与托盘 |
| `Failed`/`Unavailable`/`Unsupported` | 回到 `HiddenToTray { restore }`，不开输入，不恢复 Preview，托盘不变，`ShowRejected` | 确认值不变，待决清除，`TopmostRejected`（宿主错误原文保留在状态行） |
| 过期回执 | 无效果 | 无效果 |
| 同目标再次请求（重试） | 被取代，新 id；旧回执不提交，等待可由新请求结束 | 待决清除后可再次申请；同目标待决中不重复发 |
| 第二次显示 / 托盘切换 | 取代在途显示（新 id）；托盘文字仍是“显示”，所以托盘也是再次显示 | — |
| 隐藏请求 | 忽略，等回执 | 无关 |
| 退出请求 | 照常进入退出屏障；后续回执见上两行；显示不再接受 | 照常 |
| 守卫刷新 | 记录守卫；不武装计时（`may_animate` 为假） | 获得焦点撤销感应值（走请求） |
| 自动收起截止 | 无计时；`tick` 无动作；提交时不武装，与此前的显示相同，由焦点事件决定 | 无关 |
| 拖动结束 | 忽略，不打断等待 | — |
| 拓扑变化 | 等待保持，请求的帧随新显示器更新 | — |
| 已可见（Floating、贴边、收起、动画中） | 不进入等待，只有 `FocusWindow { id }`；收起的同时开始展开 | — |

- 与设计或任务书的差异：
  1. `WindowEffect::WindowShown` 是新加的提交信号，把托盘文字、`configure_viewports`、Preview 恢复放在回执之后；设计写在“提交”里但没有命名。效果序列多一项（受保护表一行，见下）。
  2. 隐藏等待保存（`AwaitingHideSave`）期间的唤醒不进 `ShowPending`：窗口没有离开屏幕，没有原生显示动作要确认，唤醒结束冻结（`Presented`、输入恢复、焦点请求带回执）。这是“已可见”路径的一种，等待中的保存结果此后不再有动作可做，与此前一致。
  3. 显示盖过在途隐藏（`HidePending`）被拒绝时落回 `HiddenToTray`：隐藏的 id 已被取代，回到它的等待永远结束不了，所以不恢复原值；这条路径在真实壳里不可达。
  4. 窗口流程未启动时 `SetAlwaysOnTop` 直接写配置：没有窗口可确认，启动时应用配置的值。真实路径窗口流程在任何可达输入之前已启动。
  5. `FakeHost::topmost_result` 改为 `Option`，`None` 回显请求值，默认是“成功的桌面”；`set_topmost` 的 `TopmostFact { None }` 已代表不支持，原来的默认值不再能表示成功。
- 受保护表：`shell::phase15_receipt_tests::closing_and_showing_reach_the_window_in_order` 的 show 一段，改为 `ShowWindow { id: 2 }`、`WindowShown`、`FocusWindow { id: 3 }`，原因见差异 1 与 R1 的 id。其余受保护文件未改（`git diff 074c6dc`）；`shell_pins_tests` 的置顶行不变，经默认 `FakeHost` 的回显通过。
- 测试：`flow::window::show_receipt_tests`（18 个）、`flow::window::topmost::tests`（13 个）、`ports::window::tests`（2 个）、`session::decision_tests`、`shell::phase15_host_tests`、`testing::tests`。计数（含 ignored，`--list`）：R 前 `074c6dc` app 209、desktop 217、win 136、smoke 672；R 后 app 245、desktop 224、win 137、smoke 672。
- 未验证：真实窗口上的显示、焦点读回与置顶读回（P15-R01..R06、P15-R09），`GetWindowLongPtrW` 读回从未在真实窗口上运行；由主会话的桌面回归决定。`WinHost` 的 `is_visible`/`is_minimized` 同步性依据 winit 0.30.13 源码（事件循环线程上 `set_visible`、`set_minimized` 直接执行，读回为 `IsWindowVisible`/`IsIconic`），未在真实窗口上观察。

### R3 记录（定位）

- 提交：`ce7c335` plan 04 写明定位比较与容差，验收矩阵加 `NOT TESTED` 的桌面观察行；`a6a7c02` R3；拖动中的定位不进入等待见下（`MoveResizeEnded`）。
- 改了什么：`WindowEffect::ApplyFrame { id, target, last }`。动画的中间帧 `last = false`、`id = 0`，只是呈现请求，没有回执；非动画定位与动画最后一帧 `last = true`，由 `flow/window/placement.rs` 的 `PendingPlacement` 等待恰好一次裁决。壳在 `apply_frame` 后立刻 `observe_frame()`，并在 `Moved`、`Resized`、`ScaleFactorChanged` 时、仅当有待决定位才再读，送 `WindowIntent::PlacementObserved { id, observed, now_ms }`。到 `PLACEMENT_CONFIRM_TIMEOUT_MS`（500 ms：单屏实测的 20 个目标里 19 个在 100 ms 内确认，测量窗口 250 ms）后 `Tick` 发 `ObservePlacement { id }` 再读一次；其答复还不符，或者再下一个 tick 都没有答复，就以 `Failed` 结束，等待不会悬空。`PLACEMENT_TOLERANCE_PX = 0`（USER 决定 D6），比较 `abs_diff <= PLACEMENT_TOLERANCE_PX` 逐边进行。拖动结束与拓扑恢复的 `CommitPlacement` 不再和 `ApplyFrame` 同一次派发，只在确认后发出。
- 观察次数：一次贴边动画的中间帧不读窗口。最后一帧读 1 次；若没有确认（winit 传了 `SWP_ASYNCWINDOWPOS`，同线程上第一次读是否已是新值未观察），此后每个 `Moved`、`Resized` 读 1 次，直到确认；超时路径多读 1 次。`ScaleFactorChanged` 在拓扑恢复的那次读之外还会再读 1 次。没有待决定位时 `Moved` 与 `Resized` 不读，也就不枚举显示器。
- 裁决（`flow::window::placement::tests`）：

| 情况 | 定位等待（`PendingPlacement`） |
| --- | --- |
| 观察在容差内 | 确认；拖动结束写下观察值派生的放置，拓扑恢复写请求的放置，贴边动画与显示不写 |
| 执行失败（`apply_frame` 报错） | 以该 kind 结束，不写，`PlacementFailed`（诊断） |
| 观察 `Unsupported` | 立即以 `Unsupported` 结束（这个宿主永远读不到） |
| 观察不符或 `Unavailable` | 继续等；到期后的观察仍不符即 `Failed` |
| 超时 | `Failed`，不写，诊断，等待结束 |
| 过期 id、被取代后的迟到观察 | 无效果 |
| 新定位 | 取代旧定位，旧定位没有裁决也不写 |
| 新动画（反向、中断） | 取代等待中的定位；只判最新的最后一帧 |
| 原生拖动开始 | 放弃等待（`MoveResizeStarted`）；手动拖动中壳不读窗口，所以不会把拖动当确认 |
| 拖动中产生的定位（如拓扑恢复、动画最后一帧） | 只作呈现帧（`last = false`），不进入等待，没有裁决、诊断或写入；松手的 `DragEnded` 才被裁决（`native_move_active` 由 `MoveResizeStarted`、`MoveResizeEnded` 维持，后者在没有 `DragEnded` 时也解除） |
| `DragEnded` | 新定位（带写入），确认后才写 |
| 拓扑变化 | 窗口在屏上（含显示等待）：新定位，确认后写；隐藏中或隐藏等待保存：不移动窗口，只记录所属显示器，当场写，无观察可做 |
| 隐藏请求 | 不取消，裁决照常 |
| 显示 | 自己的帧是新定位，取代等待中的定位 |
| 退出 | 不等待也不取消；确认仍可写入，退出屏障不依赖它 |
| 失败后 | 等待已清，下一次定位照常裁决；动画按时间结束，不受裁决影响 |

- 与设计或任务书的差异：
  1. 拓扑恢复的写入用请求的放置，只在确认后写（协调者裁决接受）。容差为 0 时观察帧等于目标，派生不带来新信息；而把被小显示器钳制的帧写入配置，会覆盖用户的尺寸，回到大显示器时窗口就变小了。拖动结束按设计从观察值派生，副作用是贴边时写入的 `floating_x_ratio`、`floating_y_ratio` 取吸附后的矩形（原来取吸附前的），`width_dip`、`height_dip` 经 `dip_to_px` 往返，对贴边窗口的表现没有影响，`config.toml` 的字节可能不同。
  2. 隐藏（`HiddenToTray`、`HidePending`、`AwaitingHideSave`）时的拓扑恢复照旧当场写，那里没有 `ApplyFrame`，只记录窗口所属的显示器。
  3. 被取代的定位不会把写入转交给后来的定位：拖动结束 500 ms 内又发生隐藏再显示，那次拖动不写。下一次写入的放置仍是当时的 `state.placement`。
  4. 启动时第一次 `apply_frame` 仍不经回执，在 reducer 之外（`initialize_window_shell`）。
  5. 待确认期间若动画开始（如未确认时按 Esc），这次拖动的 `CommitPlacement` 被取代，贴边边缘要等下一次写入的定位才进配置；此时退出写的是旧的窗口段。
  6. `FakeHost::observed_frame` 改为 `Option`，`None` 回显最后一次 `apply_frame`；新增 `observe_answers` 按次答复，`HostCall::ObserveFrame` 记录每次读窗口。
- 受保护表：`shell::phase15_receipt_tests::closing_and_showing_reach_the_window_in_order` 的 `FocusWindow { id: 3 }` 改为 `{ id: 4 }`，因为显示的定位现在占用一个动作 id。其余受保护文件未改（`git diff 73f5d0d`）。
- 混合 DPI 下用户看到的：系统跨显示器改变窗口尺寸时观察与目标不符，500 ms 后状态行显示“窗口未能到达目标位置（Failed）。”，窗口留在系统放下的位置，这次定位不写入配置，下次启动回到上次保存的放置；进程内的 `state.placement` 仍是拖动时的请求值，下一次贴边或拖动照常进行。不会挂起。
- 测试：`flow::window::placement::tests`（21 个）、`session::decision_tests::a_drag_is_written_only_after_its_frame_was_observed`、`shell::phase15_host_tests` 的 6 个定位用例。计数（含 ignored，`--list`）：R3 前 `73f5d0d` app 245、desktop 224、win 137、smoke 672；R3（`a6a7c02`）后 app 267、desktop 231、win 137、smoke 672；拖动修正（`bfe535f`）后 app 269、desktop 232、win 137。
- 未验证：真实窗口上的定位确认（P15-R11..R15），容差与超时只来自协调者的单屏测量；混合 DPI 下的结果是由比较逻辑推出的，未观察。观察的次数由测试与代码得出（`phase15_an_animation_observes_the_window_only_at_its_last_frame`），真实事件序列未观察。

### 审查修正（两轮独立跨模型审查，`074c6dc..bfe535f`）

USER 决定全部修复，包括在当前同步端口下不可达的项。提交：`55e4621` 壳与原生（S1–S5）、`e65b0fe` 状态机（M1–M7），文档在本记录的提交中。

| 项 | 修复前 | 修复后 |
| --- | --- | --- |
| S1（阻断） | `ScaleFactorChanged` 在 `WM_DPICHANGED` 回调内恢复拓扑并读窗口，winit 随后还有自己的 `SetWindowPos`，可能确认一个马上被移走的矩形 | 回调内只 `resize` 并置 `display_recovery_due`；恢复与读窗口在下一次 `about_to_wait` 执行，`settle_event_pass` 之前 |
| S2 | 迟到确认与超时的测试没有经过事件入口 | `geometry_event` 是共享的几何事件入口（不是完整的原生事件循环），`Moved`、`Resized` 用真实 `WindowEvent` 经此入口驱动，失配后匹配的答复都显式给出（`strict_observations` 下，未计划的观察使测试失败）；受控时钟下断言到期正好多一次 `ObserveFrame`，并检查等待、写入与诊断的顺序 |
| S3 | `ObservePlacement` 不核对就读窗口 | 先比较 `pending_placement_id() == Some(id)`，同一次 tick 里定位被自动收起取代时不读 |
| S4 | `read_extended_style` 失败会变成 `set_topmost` 的错误，启动路径多了一条退出路径 | 读回失败映射为 `TopmostFact { topmost: None }`（`SetWindowPos` 失败仍是错误）；启动路径忽略读回，运行时为 `Unsupported`（`apply_startup_topmost` 与测试） |
| S5 | `observe_frame` 枚举全部显示器只为填没人读的 `display`、`scale` | `FrameFact` 只有 `outer`，一次观察就是两次矩形读取 |
| M1 | 回执不来时等待不结束 | 显示、隐藏、焦点、置顶都有 `WINDOW_ACTION_RECEIPT_TIMEOUT_MS`（2 000 ms，理由见注释）；到期按失败处理，id 作废，输入在失败的显示后仍关闭；同目标可再次申请，包括 `user_target() == on` 的抑制；现有宿主同步答复，这条路径为不同步答复的宿主而设（端口注释与 plan 04） |
| M2 | 置顶按来源分别结算，一个来源成功、另一个失败或被取代后，确认值可能与最后证明的原生位不一致 | 只有最新一次原生请求可被结算，请求带两来源的目标，成功同时确认两者；更新请求取代旧请求（因为新请求已含全部目标）；失败后按确认值再请求一次（`Reconcile`），该请求失败只上报。测试含审查序列：user=true，ManualCollapse 的感应请求 S 待决，user 关闭 U，U 成功，S 失败 |
| M3 | 隐藏时拓扑恢复不作废等待中的定位，写入的放置可能含未确认拖动的尺寸、比例或贴边 | 作废等待（迟到观察不提交），只把显示器归属写到 `confirmed_placement`/`confirmed_dock_edge`；可见窗口的拓扑恢复按此前决定，确认后写请求的放置 |
| M4 | 退出取消无条件开输入 | 按当前可见性与生命周期重新投影输入和 IME，隐藏中保持关闭，退出期间落地的显示恢复；退出屏障因冲突被阻止的路径同样处理 |
| M5 | `finish_drag` 能覆盖隐藏中的状态 | 只在屏上状态（`may_animate`）改变可见性 |
| M6 | 动作 id 自增 | `checked_add`，耗尽时不发请求、各按自己的失败路径结束，不复用 id 或 0 |
| M7 | Pin 正交测试从未打开 Pin | 明确设置 ON 与 OFF，比较可见性、贴边与定时器，允许置顶效果与字段不同 |

- 受保护行：本轮没有改动任何受保护行（`git diff bfe535f`）。
- 新增与改写的测试：`flow::window::show_receipt_tests`（25 个）、`flow::window::topmost::tests`（16 个）、`flow::window::placement::tests`（24 个）、`shell::phase15_host_tests` 的 S 项用例。计数（含 ignored，`--list`）：本轮前 `bfe535f` app 269、desktop 232、win 137、smoke 672；本轮后 app 280、desktop 236、win 138、smoke 672。
- 未验证：S1 的时序只由 winit 0.30.13 源码（`WM_DPICHANGED` 处理在回调返回后调用 `SetWindowPos`）和无窗口测试支持，未在混合 DPI 的真实窗口上观察（P15-D10）；M1 的超时路径在现有同步宿主上不可达，只由 reducer 测试证明；真实窗口上的读回与确认仍属 P15-R01..R06、P15-R09、P15-R11..R15。

### 第二轮审查修正（`bfe535f..848498a`）

USER 决定全部修复。提交：`b1ffd55` 代码，文档在本记录的提交中。

| 项 | 修复前 | 修复后 |
| --- | --- | --- |
| F1 | 恢复推迟期间 `Moved`、`Resized` 仍能确认旧显示器上的定位 | `display_recovery_due` 期间不读窗口，推迟的恢复取代等待中的定位，或在没有新定位时事后再读 |
| F2 | 请求失败且重新请求也失败后，同值的重试被目标相等的短路抑制，原生位可能是错的 | `native_bit_unknown` 记录“最后的结算失败、原生位未知”，未知时同值请求照常到达宿主；已经在途或已知相符时才抑制。自动重试仍止于一次 reconcile（状态转换引起的感应请求是事件驱动的，不是重试循环）。失败的请求带着未确认的用户值时报告 `User`，否则报告请求自己的来源（第三轮更正了原来“总是发起者”的说法） |
| F3 | id 耗尽时定位仍发 `ApplyFrame { last: false }`，旧的等待留着，匹配的观察会确认较新的 `state.placement` | 不发任何帧，结束旧等待，只报 `PlacementFailed`；已核对显示、焦点、隐藏、置顶：都在耗尽分支提前失败，不发原生请求，也不改动在途动作（在途的 receipt 仍能结算自己的值） |
| F4 | 迟到的成功没有明确约定 | 端口文档写明：晚答复的宿主必须保证取消或顺序，动作在 tick 处理期限时到期，此前到达的回执接受、之后的忽略，恢复用新 id（第三轮更正）；Open Items 加异步宿主；reducer 测试覆盖过期后的成功与重试 |
| F5 | S2 的第三次观察靠 `FakeHost` 的回显 | 匹配的结果显式给出，新增 `strict_observations`；记录更正：覆盖的是共享的几何事件入口，不是完整的原生事件循环 |

- 受保护行：本轮没有改动（`git diff 848498a`）。
- 计数（含 ignored，`--list`）：本轮前 `848498a` app 280、desktop 236、win 138、smoke 672；本轮后 app 284、desktop 238、win 138、smoke 672。
- 未验证：真实窗口上的行为与上一节相同；F1 的时序由 winit 源码与无窗口测试支持，混合 DPI 下未观察（P15-D10）；F4 的约定只由 reducer 测试证明，没有真实的异步宿主。

### 第三轮审查修正（`848498a..8d8858a`）

USER 决定全部修复。提交：`db4a3e9` 代码，文档在本记录的提交中。

| 项 | 修复前 | 修复后 |
| --- | --- | --- |
| 1 | `show()` 取到最后一个 id 进入 `ShowPending`，定位拿不到 id 不发帧，`ShowWindow` 仍发出；隐藏时收起的窗口可能被提交为贴边展开却还在收起矩形 | 进入 `ShowPending` 前先确认显示与定位都有 id，否则保持原状态并 `ShowRejected`；`AwaitingHideSave` 的唤醒同样要定位的 id（同一形状） |
| 2 | 合同写“过期 id 永不接受”，而 `complete_window_action` 不比较时间 | 合同更正（端口文档、plan 04、本记录）：动作在 tick 处理期限时到期，此前到达的回执接受，之后的忽略。不在回执时比较时间，因为同步调用可能本身就超过期限，判失败会留下可见窗口却记录隐藏 |
| 3 | 记录写“拒绝的 source 取发起者” | 实际规则：失败请求带未确认用户值时报 `User`，否则报请求来源；新增感应请求带未确认用户值失败的测试 |
| 4 | `strict_observations` 没有反向测试 | `#[should_panic]` 钉住未计划的观察 panic 及其消息 |

- 受保护行：本轮没有改动（`git diff 8d8858a`）。
- 计数（含 ignored，`--list`）：本轮前 `8d8858a` app 284、desktop 238、win 138、smoke 672；本轮后 app 288、desktop 239、win 138、smoke 672。
- 未验证：同前两轮。

### 桌面回归（脚本部分）

D 的 Release（`b7b5980` 的产品代码）用仓库自带的本地预检跑真实窗口与真实输入：`STICKYMD_PREFLIGHT_EXE` 指定 exe，干净 worktree，逐个 `--ignored --exact --test-threads=1`。这是本地诊断，不是 exact-candidate 证据。

| 用例 | 结果 |
| --- | --- |
| G4-01 托盘 | 通过，19 项 |
| 托盘失败恢复 | 通过，1 项 |
| G4-02 贴边与计时 | 通过，54 项 |
| G4-03 Search 与剪贴板快捷键 | 通过，15 项 |
| G4-04 公式转换 | 通过，4 项 |
| G4-05 单实例身份 | 通过，23 项 |
| 双副本、空笔记双副本 | 通过，各 1 项 |
| G4-06 微软拼音与微信输入法 | 通过，72 项 |
| G3-01 图片粘贴 | 通过，13 项 |
| G3-03 崩溃恢复 | 通过，17 项 |
| G3-04 用户资产安全（含一次真实导出） | 通过，11 项 |
| G3-05 受管资产安全 | 通过，4 项 |
| G5-01..04 截图 | 通过，24 张截图待人工看 |
| G3-02 导出 | 未通过，见下 |

- 工具修正：`PhysicalCursorKind::DragRegion` 仍等手形（`3022ff1`）。C3 起工具栏空白是箭头（预期差异 8b），这些用例默认 ignored，没有人跑到，第一轮 5 个用例因此停在第一步。修正后都通过，8b 也由此得到真实光标的证实。
- 新增的本地入口（`36edcf2`）：G3-01、G3-02、G3-04、G3-05、G4-04 原来只能用候选包跑。
- G3-02：`press_export` 是启动后的第一个动作，它的激活点击在 500 ms 内读不到左键按下，之后前台停在 `GameInputServiceWindow`。同样的步骤在基线 `ae09b67` 的二进制上失败在同一处，所以不是 Phase 15 的回归；根因未查清。导出的发布在 G3-04 通过，资产复制与改写由 `export` 模块的单元测试覆盖，这一项列入人工清单。
- 环境：桌面上没有可接手的普通窗口时，前台会落到 `GameInputSvc` 的 `GameInputServiceWindow`，Windows 随后拒绝所有注入的光标移动。这与 StickyMD 无关；几个用例因此被挡住，换到有可用前台时重跑通过。
- 回退重开（P15-B13）：D 编辑并保存后的副本由 C5、C4、基线 `ae09b67` 的二进制打开，退出码都是 0，27 行里程碑，`note.md`、`config.toml` 字节不变。这证明读兼容；基线的二进制带帧计数补丁，其余与 `ae09b67` 相同。
- 帧计数补充：这台机器接回 100% 显示器后，D 的 Release 在原生 DPI 1.0 下三种模式与基线表逐项相同，启动里程碑与数据字节与 C5 相同。

人工清单（脚本判断不了或没跑通）：

1. 带本地图片的笔记导出（Ctrl+Shift+S，选路径）：生成 `.md` 与 `-assets` 目录，图片引用改写，原笔记不变（G3-02）。
2. 输入法组字时候选窗贴近光标（72 项检查只验证组字、提交与撤销的结果）。
3. 预期差异 8a：选中按钮的背景上沿离工具栏顶 6 DIP，图标位置不变。
4. G5 的 24 张截图，看控件、缩放、透明度、主题、紧凑尺寸与公式渲染。
5. 多屏与混合 DPI：当前只有一块可用显示器，未测。

结果：USER 2026-11-08 查完 1–4 项，都没有问题；第 5 项没有测（P15-D10）。

### 最终桌面回归（R 之后）

| 用例 | 结果 | 提交 |
| --- | --- | --- |
| G4-01 托盘 | 通过，19 项 | `9c19544` |
| 托盘失败恢复 | 通过，1 项 | `9c19544` |
| G4-05 单实例身份 | 通过，23 项 | `9c19544` |
| G4-02 贴边与计时 | 通过，54 项 | `9c19544` |
| G4-03 Search 与剪贴板快捷键 | 通过，15 项 | `9c19544` |
| G4-04 公式转换 | 通过，4 项 | `9c19544` |
| 双副本、空笔记双副本 | 通过，各 1 项 | `9c19544` |
| G4-06 微软拼音与微信输入法 | 通过，72 项 | `9c19544` |
| G3-01 图片粘贴 | 通过，13 项 | `9c19544` |
| G3-03 崩溃恢复 | 通过，17 项 | `9c19544` |
| G3-04 用户资产安全 | 通过，11 项 | `9c19544` |
| G3-05 受管资产安全 | 通过，4 项 | `9c19544` |
| G5-01..04 截图 | 通过，证据是截图产物，不输出检查项；R 未重新人工看图 | `9c19544` |
| G3-02 导出（含取消与关闭后的前台断言） | 通过，4 项 | `b6e2be1` |

- `9c19544` 的 Release sha256 为 `30dc312e43428b55523b84c0c898c416690c366897005431dd2df787a2650d14`，`b6e2be1` 的为 `8d5fd9275a7e3c44507fc2b84d6e9c4d47f45742a587164676e288a3b39977cf`。两者只差导出对话框（owner 与 COM 释放顺序），其他 17 项没有在 `b6e2be1` 上重跑。这是本地诊断，不是 exact-candidate 证据。
- G3-02 的根因（修正 D 记录里“根因未查清”与“卡在启动后的第一个动作”）：产品的 `IFileSaveDialog::Show(None)` 没有 owner。采样前台窗口看到导出其实已经完成：对话框按下“导出”关闭后约 16 ms，前台从对话框落到 `GameInputServiceWindow`，StickyMD 没有拿回前台；用例下一次点击时 Windows 拒绝注入，报 500 ms 读不到左键，或光标移动不到位。修复后（`0492160`）同一位置前台回到 StickyMD。基线同样失败，这是 Phase 15 之前就有的问题。
- 有了 owner，UI Automation 把对话框列在便签窗口之下，`windows-uia.ps1` 改为在这里找，找到对话框同时证明了归属。
- 环境：前台落到 `GameInputSvc` 后，本地编排脚本在后台运行时多次拿不回前台（第一轮 9 个用例被拦在启动前，空闲 80 分钟也没恢复），而从前台工具调用启动的窗口能拿回。原因未查清，只影响测试脚本的编排。
- 独立跨模型审查（`0492160`）：没有阻断项。COM apartment 先于对话框结束（原有问题）、UIA 查询没有限定对话框的进程、SAFETY 注释断言了文档没有保证的事，都已在 `b6e2be1` 修正。它还要求验证取消与前台恢复，G3-02 因此先取消一次，并在两次关闭后断言便签取得前台；这一步单独断言，是因为用例的下一次点击能从任何普通窗口激活便签，会掩盖丢失。
- 模态期间：同步阻塞不等于没有消息，但审查读 winit 0.30 的 Windows runner，回调期间的窗口事件与用户事件被缓存，折叠、隐藏、退出在 `Show` 返回后才处理；owner 没有改变这一点。这是读代码的结论，没有桌面运行。
- 出口：`phase 15 --performance` 在 `0492160` 通过；同一提交原生 100% 的帧字节三种模式与基线表逐项相同；27 行启动里程碑。
- 数据：`config.toml` 的字段集没有变。G4-02 由 R 二进制写下的配置，以及人工改成贴边吸附极值的变体（`dock_edge = "right"`、`floating_x_ratio = 1.0`、`floating_y_ratio = 0.0`），由基线 `ae09b67`、D、R 的二进制打开，退出码都是 0，27 行里程碑，`note.md`、`config.toml` 字节不变。这只证明读兼容；G4-02 最后的配置不在吸附状态，吸附后真实写下的比值没有采到，极值是构造的。
- 这一轮的脚本没有运行（人工结果见下节）：置顶时对话框的层级；对话框打开期间的贴边折叠、托盘隐藏或退出、第二实例唤醒、外部最小化（owner 被最小化时对话框随之隐藏）；P15-R01..R06、R09、R11..R15 中脚本断言不到的部分（见验收表）。

### 回执的人工检查（2026-11-09）

USER 用这条分支构建的版本检查了 5 项，都没有问题：置顶时的导出对话框、从隐藏/最小化/贴边收起的托盘唤醒、贴边动画反向、拖动松手与重启、置顶按钮。程序路径与 SHA-256 没有记录。逐项对照见[收据](../report/2026-11-09-phase-15-window-receipt-manual-check.md)：P15-R03、R04、R11、R12 为 `MANUAL PASS`；P15-R01、R02、R09 各有一条没有观察到，保持 `NOT TESTED`。

远程 CI：Develata/StickyMD#11 在 `f9c1813` 上 9 项全部通过。

### 里程碑 E（USER 批准 2026-11-09）

设计、审查结论与修订见[报告](../report/2026-11-09-phase-15-e-shared-startup-logic.md)。最初的“统一文件原语端口”被清点否定，第一版 `StartupStorage` trait 被独立审查否定。现在的切分：共享层只做决定，观察、隔离、删除、首次创建、暂存、改名、发布都留在宿主，顺序不变。

- 移动：`config.toml` 的文本规则与警告，`plan_note_startup`，粘贴批次，`ExportPlan`，百分号解码；D8 的笔记文件过滤与 D9 的链接分类。
- 留在宿主：存储事务、失败分类（输入是各宿主自己的错误类型）、路径语法校验、`preview/image_source.rs`。
- 已有问题，这批不改：`remove_temporary`、`quarantine_temporary` 与导出暂存文件的改名都按路径执行，观察之后被其他进程替换的窗口今天就存在。
- macOS 的具体保证按 `05` 在建立宿主时单独批准。
- 桌面（`35d3c25` 的 Release，sha256 `d1e9b1fb1ddbd689874709a3923f5aadd280d6d07f7fe681ebde5eacc18434c9`，单屏）：全部 18 个本地预检通过，见 P15-E08。真实启动退出码 0，trace 27 行。之后的提交只改测试脚本。这是本地诊断，不是 exact-candidate 证据。
- 回归中修了一处脚本竞态：恢复替换 `note.md` 时轮询读到瞬时的文件不存在，脚本立即判失败；现在和 `wait_note` 一样重试到期限。
- 同一轮把 P15-R01、R02、R09 没观察到的条款改成脚本断言：托盘 Show 取得焦点、第二实例唤醒后矩形不变、托盘文字正确。
- D7 增加了 `aarch64-apple-darwin` 的依赖策略目标，D8 的笔记文件过滤与 D9 的链接分类已共享；D10 保持现状。

### 窗口启动失败（USER 批准方案 A，2026-11-09）

合同是 `09` 的 Failure Paths，分析见[报告](../report/2026-11-09-startup-failure-paths.md)。

- `stickymd-app` 的 `flow::startup` 用两个类型表达继续或终止：`StartupStop`（窗口、绘制表面、无显示器、工具窗口身份）与 `StartupDegraded`（托盘、透明度、置顶），壳不能把后者当作前者处理。
- 壳只有 `abort_startup` 一条终止路径：移除托盘、提示、记录失败、结束事件循环；`main.rs` 据此返回失败退出码，事件循环的两处失败改走 `fatal_startup`。
- `initialize_window_shell` 接收显示与聚焦两个闭包，整个编排都能在 `FakeHost` 上测试；里程碑与原生调用顺序不变。
- 行为变化：透明度、置顶失败不再结束启动；启动失败的退出码由 0 变为失败；原来没有提示的路径现在有提示；诊断探针退出前移除托盘。
- winit 在 `exit` 之后仍派发缓冲的事件（`runner.rs` 的 `call_new_events` 与 `dispatch_buffered_events`），提示框又没有 owner，所以显示后身份失败时，提示期间的输入会在回调恢复后进入编辑器（读代码的结论，没有实际运行）。`abort_startup` 先记录失败，`window_event`、`about_to_wait`、`user_event` 第一步就返回。
- `run_app` 出错时先结束壳（托盘、worker、单实例守卫），再提示并退出，不再跳过清理。
- 幸存的失败在状态行上用“；”连接，换行压成空格；恢复提示待处理时改用对话框，因为它的按钮按固定比例布局，追加文字会盖住按钮，不写又会丢失原因。
- 独立审查：前两轮 Codex（`0b77793`）没有产出结论（后端报错，compaction 后丢失任务），第一轮的中途文字不算结论，其中的问题对照 winit 源码与本仓库代码核实后修在 `219c343`。第三轮（`219c343`，限定读取范围）完成，结论是 1 个阻断项：winit 隐藏事件窗口的创建失败不上报。事实成立，这是依赖的原有缺口，这一批没有修改，见 Open Items。其余发现已修正：恢复提示期间失败原因丢失，几个测试抓不住坏实现，验收行声明过宽。第四轮（`219c343..b9f7875`）没有阻断项，指出的断言与声明差距也已修正。测试能否失败用 8 个注入缺陷验证过（`b9f7875`），都被抓住。
- 桌面（`ce77223` 的 Release，sha256 `6eb83b4a6f5380519add1182a3b0778f00cfa48f6ef06bdd50545fc88cb5d121`，单屏）：真实启动退出码 0，trace 27 行（表头加 26 个里程碑），`focus_ready` 在 `topmost_ready` 之后；本地预检 G4-01 托盘 19 项、托盘失败恢复 1 项、G4-05 单实例身份 23 项、G4-02 贴边与计时 54 项都通过。这是本地诊断，不是 exact-candidate 证据，其余预检没有在这个提交上重跑。

## Open Items

- 人工观察的剩余项：P15-R05、R06、R15 的失败路径（需要能制造失败的环境），R13、R14 的多屏。
- 保存时被强杀的窗口：临时文件已创建、还没写入时被杀，会留下空的 `note.md.tmp`。它与正文不同，所以下次启动按 `05` 恢复第 3 条提示恢复，选 F6 会得到空笔记。这是已有行为，保存代码这批没有改；G3-03 在 15 ms 偏移上约六分之一命中。是否把空临时文件当作没有内容，需要改 `05`，待 USER 决定。
- 不同步答复的宿主（M1、F4）：回执期限与“tick 处理期限后的回执被忽略”只在 reducer 测试中运行过；这样的宿主必须保证取消或顺序（端口文档），恢复一律用新 id。
- 回执期限（M1）只在 reducer 测试中运行过：现有宿主同步答复，这条路径要等到不同步答复的宿主才有真实运行。
- 定位的容差是单屏实测（原生 100%）：混合 DPI 未测（P15-D10、P15-R14），系统跨屏改变窗口尺寸时这次定位会以诊断结束且不写入，取值可能变更。
- 启动失败路径的真实进程观察未做（P15-S06）；无窗口测试覆盖见 P15-S01..S05。
- winit 0.30.13 的隐藏事件窗口创建失败不上报（`event_loop.rs` 的 `create_event_target_window` 不检查 `CreateWindowExW`，`EventLoop::new` 仍返回 `Ok`）：此时 `EventLoopProxy` 投递到调用线程，worker 回执到不了界面。只在隐藏窗口失败而主窗口成功时出现，读源码的结论，没有故障注入。考虑过的修复是改 winit 或依赖它的私有窗口类名；USER 2026-11-09 决定记为已知缺口，不修改。
- 多屏与混合 DPI 未测（P15-D10）。

## Out of Scope

- 非 Windows 宿主与其产品决定（`02` 平台节）。
- 任何产品行为变化；A 只移动代码、调整可见性与导入路径。
- 把 Phase 15 加入 Phase 14 的正式资格化门；它不是资格化 suite。
