# Phase 15 C/D 设计裁决与合成方案

基线：`ae09b67`，分支 `feat/phase-15-c-shared-desktop`。由调研、两个独立方案和对照代码的裁决合成，产出时没有运行 cargo；状态：USER 批准（2026-11-07）。
标记：[V] 本次读到代码或 registry 源码；[I] 推断；[U] 未验证。

## USER 决定（2026-11-07）

- D1 批准拆分。
- D2 C 与 D 同一出口，plan 11 已修订。
- D3 `Keymap` 类型与默认表放 `stickymd-desktop`。
- D4 与 Codex 讨论后定（2026-11-07）：删除 `toolbar_mode_at_cursor` 及其调用，`ControlLayout` 做唯一几何来源，绘制、悬停、按下共用一份缓存和一个 `hit_test`。Windows 整数鼠标坐标下切换视图的分支不可达（算术模型），删除不改当前行为，C3 用路由测试确认。性能收益很小，价值在消除漂移。三个可见小问题单独提交：按钮背景侵入缩放区、工具栏空白的手形光标、125% 下同步按钮绘制与命中不一致。C3 的特征测试扩到 DPI 1.0、1.25、1.5、2.0，并覆盖最终路由命中图。
- D5 回归两次：抽完一次宽回归，回执做完一次窄回归；回执在 Phase 15 之内。

## 0. 回答

要抽的没有抽完。`stickymd-desktop` 不存在（[V] 根 `Cargo.toml:3-9` members 只有 core、render、app、win、smoke；`crates/` 只有三个目录）。`docs/tasks/phase-15-shared-core-migration.md:24-26` 写明 C、D 未开始。`apps/stickymd-win/src/app.rs:105-170` 的 `StickyApp` 仍有 56 个字段（[V] 计数），`app.rs`+`app/` 共 13119 行含测试。

建议顺序：先做完下面 P0 到 D 的全部抽取与局部修复，再让 Develata 跑**一次**桌面回归；显示、置顶、定位回执是行为变更，不属于"抽"，放在回归之后另开一批，配一次窄回归（理由见 §4 与 §8）。

## 1. 抽查结果（共 22 条，列出错的与要紧的）

### 1.1 有误或需要修正的（10 条）

| # | 出处 | 声称 | 实际 | 影响 |
|---|---|---|---|---|
| W1 | 方案 B 批 4 | 约 59 个回执测试（36+17+6）可改在 `stickymd-app` 下运行 | [V] 它们走 `HeadlessApp` fixture（`app/headless_tests.rs:11-41`）：`win_io_ports`、`RuntimePaths`、`SingleInstanceGuard`、`winit::platform::windows::EventLoopBuilderExtWindows`，还有真实 worker 与磁盘。只要 fixture 用 Windows 存储 adapter，就不能进 `stickymd-app`、也不能在 Linux 跑 | 这些测试留在 `apps/stickymd-win`，改成驱动 `DesktopShell<WinHost>` 的公开面；方案 A 的"留 host"是对的 |
| W2 | 方案 A C2b | 把 `preview_input.rs:99-114` 的字面量 7.0/38.0/34.0 命中改用 `ControlLayout`，"全像素扫描命中一致" | [V] 字面量是偏移 7、步距 38（DIP）；`ControlLayout` 是 `EDGE_DIP=5`、`CONTROL_DIP=28`、`GAP=4`，步距 32（`controls.rs:11-13,106-110`）。两者不等价，等价测试必然失败。该函数只被 `input.rs:441` 的左键按下调用 | 不能当零差异替换。先写特征测试查清哪些按下会落到这条路径（工具栏按钮已被更早的 handler 吃掉时它是死代码，否则是第二个、与绘制不一致的布局权威）。结论是 USER 决定项 |
| W3 | 方案 A、B | 治理硬编码标记是 `.with_decorations(false)`、`CornerPreference::RoundSmall`、`ControlFlow::Wait` | [V] `governance.rs:670-677` 有四个标记，还有 `.with_undecorated_shadow(true)` | 拆分检查时四个都要分配：`with_decorations(false)`、`ControlFlow::Wait` 去 desktop 文件；`with_undecorated_shadow(true)`、`CornerPreference::RoundSmall` 去 host 文件 |
| W4 | 方案 A、B | Release 的 LTO/codegen-units 未知，跨 crate 内联是风险 | [V] 根 `Cargo.toml:65-70`：`lto="fat"`、`codegen-units=1` | release 下跨 crate 内联风险基本消除；不需要 `#[inline]` 补丁或专门对比 |
| W5 | 方案 A §8.6、B §9 | `age ≥ 2` 的整体呈现比基线多拷贝，要请 USER 确认 plan 10 豁免 | [V] Windows softbuffer `age()` 只返回 0 或 1（`softbuffer-0.4.8/src/backends/win32.rs:308-313`），CoreGraphics 恒 0（`cg.rs:294-296`）。Windows 序列不变，Windows 硬门不受影响；plan 06 `:336-339` 与 plan 10 已规定其他平台另行建基线 | 不需要豁免，也不需要 USER 决定。只在任务文件记一句 |
| W6 | 方案 A §1.3、B §3 | `mutation_input_allowed` 属 app，`handle_ime` 问 `session.mutation_allowed()` | [V] 谓词本身是四个 bool（`input.rs:690-697`），但参数里 `preview_focused` 是 desktop 视图状态，`!window_accepts_editor_mutation()` 读 `shell_input_enabled`（desktop）再叠加 `window_flow`（`window_geometry_runtime.rs:74-80`） | 无参的 `mutation_allowed()` 会得出错误答案，等于改 IME 门控。签名必须是 `AppSession::mutation_allowed(&self, preview_focused: bool, shell_input_enabled: bool) -> bool` |
| W7 | 方案 A、B | `ExitProcess` 改为 `exit` 标志即可 | [V] `window_runtime.rs:370-379`：先 `self.tray = None` 再 `event_loop.exit()`；没有 event loop 时写诊断并重新打开输入。两个方案都没提这个回退 | 标志必须在同一个 `ApplicationHandler` 回调末尾消费，且仍先销毁托盘；无事件循环的回退分支随之消失，要在批记录里写明 |
| W8 | 方案 A §1.3 | 放置夹取用 `config::MIN_WINDOW_*_DIP` 复用 | [V] 最小值是 `pub const`，但 `MAX_WINDOW_DIP` 是私有（`config/runtime.rs:8-10`） | 需把 `MAX_WINDOW_DIP` 设为 `pub`，数值 220/120/16_384 逐字不变 |
| W9 | 方案 A C1 | desktop 先不带 winit，把 `winit::dpi` 换成 `f64` 对，并建立中立 `Key/Chord/PointerPos` 词汇 | plan 03 desktop 行（`03_system_architecture.md:80`）明写可使用 winit/softbuffer；Debian 宿主也是 winit | 多出一层翻译和一批改动，无共享收益，还给 IME 按键路径（`Key::Process` 类事件）增加回归面。不采用；A 的"晚暴露 Linux 风险"也不成立，因为 C3 的 softbuffer 本来就要先上 Linux |
| W10 | 方案 A §4 | "age 判断要放在裁剪之前，现有写法在裁剪为空时直接返回" | [V] 现有代码先判 `age()==0`（`surface.rs:138`）再裁剪（`:143`）。真正的缺陷只是条件应为 `age != 1` | 描述有误，结论无害：新计划函数必须先看 age，再裁剪 |

### 1.2 核实为正确的（12 条）

1. `StickyApp` 56 个字段，含 3 个 `cfg(test)`（`app.rs:105-170`）。
2. `surface.rs:138` 只对 `age()==0` 走全量，`age≥2` 误入局部路径；与 plan 06 `:336-339` 冲突。
3. `governance.rs:670/686` 读 `app/lifecycle.rs` 与 `app/window_geometry_runtime.rs`，文件迁出会直接报错。
4. `Module::ALL` 长度 7，顺序 Core、Render、App、Windows…（`headless.rs:18-26`）；`deny.toml:3` 只检查 `x86_64-pc-windows-msvc`。
5. `fingerprint.rs:201` `is_global_input` 与 `:309` `PRODUCT_ROOTS` 存在，`:471` 有"product sources outside PRODUCT_ROOTS"拒绝。
6. `TOOLBAR_HEIGHT_DIP` 两份：`controls.rs:10`（f64）与 `preview_runtime.rs:23`（f32）。
7. `winit_resize_direction` 重复：`window_interaction.rs:443`、`window_runtime.rs:547`。
8. winit 0.30.13 在 Linux 未启 `x11`/`wayland` 时 `compile_error!`（`platform_impl/linux/mod.rs:3-4`），方案 B 对，方案 A 只凭记忆。
9. `commit_window_placement` 硬编码 `clamp(220.0,16_384.0)`/`clamp(120.0,16_384.0)`（`window_runtime.rs:517-544`）。
10. `lifecycle.rs:24` 恢复期间不启动看门狗；`:65-70` 失焦且 dirty 发 `SaveNow(FocusLoss)`；`:78-94` `settle_event_pass` 含 zoom 去抖。
11. `hide_native` 已读回 `is_visible()`（`window_runtime.rs:444-467`）；`apply_effective_topmost` 只写诊断（`:114-121`）。
12. `ClipboardPort` 对象安全（无泛型、无 `Self` 返回，`flow/clipboard.rs:36-43`），但 `EditorCoordinator<C: ClipboardPort>`（`flow/editor.rs:62,69`）需要一个 `impl ClipboardPort for Box<dyn ClipboardPort>` 转发。`scrollbar.rs:8`、`toolbar_paint.rs:8`、`caret_runtime.rs:9` 都从 `preview_runtime` 取 `PaneRect/ViewGeometry`，所以几何必须第一个抽出。

## 2. 评分（5 分制）

| 维度 | 方案 A（视图先行） | 方案 B（壳对象先行） | 说明 |
|---|---|---|---|
| 对 plan 03/04/06/10/11 的正确性 | 3.5 | 4 | A 的中立输入词汇无依据（W9）；两者都漏掉 W3、W6、W7。B 用 `winit::platform::` 禁令保持 desktop 可在 Linux 编译，是好点子。plan 03 第 9 条（快捷键属宿主配置）两者都按 plan 走 |
| IME 与热路径风险 | 3.5 | 4 | A 的 IME 手测与门控表很细，但自己引入的 Key 翻译层增加风险；B 把 IME 区域更新顺序与门控说清楚，且不增加翻译层。两者都漏了 W6 |
| Debian 宿主实际复用 | 4 | 4.5 | 终态都是 `DesktopShell<H>`。A 的 `WindowHost` 含 minimize、drag 共 11 个方法，把 winit 可移植调用也塞进端口，违背 plan 03 规则 3；B 只留需要读回或原生行为的 8 个方法 |
| 批次独立性 | 4 | 3.5 | A 的 C1（只有 geometry）和 C2a（纯叶子）很小，每批可独立绿；但 C5-C8 在跨 crate 状态下反复改 `pub` 可见性。B"先原位解耦再机械搬家"更稳，但批 6 是一次性大搬家，还同时重写 fixture |
| 改动量 | 3 | 3.5 | A 多出中立输入层和 `dpi` 替换；B 的 `StickyApp<H>` 原位泛型化多一轮，但没有额外抽象 |
| 度量与登记 | 4.5 | 4 | A 的 M0-M6 和登记清单更全；B 补了 `FrameTarget` 的无窗口计数思路（本文改为纯函数计划+转换，不引入 dyn） |

取舍：采用 B 的主干（原位解耦，最后机械搬家；desktop 可用 winit；窄端口），吸收 A 的纯叶子早搬、特征测试先行、登记清单与度量表。

## 3. 合成方案

### 3.1 终态布局

依赖方向（plan 03:86）：`Platform Host → stickymd-desktop → stickymd-app → stickymd-core`，desktop 与 app 均可依赖 render。

```text
crates/stickymd-app/src/
  ports/window.rs        WindowHost, Visibility, TopmostFact, FrameFact           (新)
  ports/monitor.rs       MonitorSource, MonitorSnapshot                            (新)
  ports/shell.rs         TrayHost, TrayView, TrayCommand, ShellLauncher,
                         ExportTargetPicker, FatalNotice                           (新)
  session/               AppSession, ShellEffect, ShellEffects, MonotonicClock     (新)
  interaction/rules.rs   纯规则: search_on_view_change, search_open_action,
                         replace_current_action, replacement_end, replacement_allowed,
                         scope_to_selection, theme_rotation, clamp_selection,
                         clamp_placement, mutation_allowed, guard_snapshot
  flow/window/           已有；R 批补 ShowPending/TopmostPending/PlacementPending

crates/stickymd-desktop/src/
  lib.rs                 #![forbid(unsafe_code)]；每个业务 module 带 plan_ref
  host.rs                Host, EventSink, DesktopEvent, HostSignal, Keymap, PresentFacts
  geometry.rs            PaneRect, ViewGeometry, geometry(), TOOLBAR_HEIGHT_DIP(唯一一份)
  title.rs               标题投影, QuitCancelNotice
  frame/                 plan.rs(PresentPlan, plan_present)  convert.rs(copy_rgba_full/damage)
                         surface.rs(SoftwareSurface)  ledger.rs(FrameLedger)
                         compose.rs(render: fill+blit 顺序)  caret.rs(局部损伤, 闪烁)
  controls/              layout.rs  state.rs  toolbar_paint.rs
  scroll/                scrollbar.rs  runtime.rs  application.rs
  preview/               view.rs  input.rs  horizontal.rs
  search/                layout_paint.rs  events.rs
  input/                 ime.rs  keyboard.rs  pointer.rs  wheel.rs  escape.rs  keymap.rs
  shell/                 mod.rs(DesktopShell<H>)  view_state.rs  effects.rs(解释 ShellEffect)
                         handler.rs(ApplicationHandler)  guards.rs  testing.rs(FakeHost, cfg(feature="testing"))

apps/stickymd-win/src/   main.rs(单实例, 消息钩子, 构造 EventSink/AppSession/WinHost, 调 run)
                         host.rs(WinHost: 实现 Host 与全部窄 trait)  platform/windows/*  各存储 adapter
```

`stickymd-desktop` Cargo.toml：直接依赖 core、render、app（`ci/registry.rs` 比对 `cargo tree --depth 1`）、tiny-skia、winit、softbuffer、raw-window-handle。不得依赖 windows、arboard、tray-icon、notify。winit/softbuffer 的 feature 按 target 分表（`target_os` 只能出现在 Cargo.toml 的 target 表里）：

- 全部 target：`winit = { workspace = true }`（`default-features=false, features=["rwh_06"]`）、`softbuffer = { workspace = true }`。
- `cfg(all(unix, not(target_os = "macos")))`：winit 加 `x11`、`wayland`、`wayland-dlopen`；softbuffer 加 `x11`、`x11-dlopen`、`wayland`、`wayland-dlopen`。不开 `kms`（拉 drm）与 `wayland-csd-adwaita`。[I] feature 名来自方案 B 读 registry 的结果，M1 实测确认。
- `Cargo.lock` 在同一提交更新，`--locked` 通过；`FORBIDDEN_PACKAGES` 含 `gtk`（`governance.rs:97`），必须确认不被拉入。

### 3.2 宿主能力面（精确签名）

端口放 `stickymd-app/src/ports/`，签名只用中立类型。复用现有类型：`PhysicalRect`、`MonitorGeometry`、`MonitorIdentity`（`flow::window::geometry`）、`FailureKind`（`ports/failure.rs`，Failed/Unavailable/Unsupported）。主线程限定 trait 不带 `Send`/`Sync`（plan 03 规则 6）。窗口创建、IME、光标、标题、主题、重绘、`drag_window`、`drag_resize_window`、`set_minimized` 等 winit 可移植调用由 desktop 直接调 `Arc<Window>`，不进端口（规则 3）。

```rust
// stickymd-app::ports::window
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visibility { pub visible: Option<bool>, pub minimized: Option<bool> } // None = Unsupported
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopmostFact { pub topmost: Option<bool> }
#[derive(Clone, Debug, PartialEq)]
pub struct FrameFact { pub outer: PhysicalRect, pub display: Option<MonitorIdentity>, pub scale: f64 }

pub trait WindowHost {
    /// set_visible(true) + 取消最小化 + 重设工具窗口身份/任务栏回退，随后读回。
    fn show(&mut self) -> Result<Visibility, FailureKind>;
    /// 现有 hide_native 语义逐字保留：Some(false)=Ok, Some(true)=Failed, None=Unsupported。
    fn hide(&mut self) -> Result<(), FailureKind>;
    /// 取消最小化 + 请求焦点 + 读回可见/最小化。焦点本身只由真实 Focused(true) 事件证明。
    fn focus(&mut self) -> Result<Visibility, FailureKind>;
    fn visibility(&self) -> Result<Visibility, FailureKind>;
    /// 同步 SetWindowPos(NOACTIVATE)。R2 批新增读回 WS_EX_TOPMOST；此前 topmost 为 None 之外的值也可返回，调用方暂不使用。
    fn set_topmost(&mut self, on: bool) -> Result<TopmostFact, FailureKind>;
    fn set_opacity(&mut self, percent: u8) -> Result<(), FailureKind>;       // 默认实现 Unsupported
    fn apply_frame(&mut self, target: PhysicalRect) -> Result<(), FailureKind>; // 只发起，不证明
    fn observe_frame(&self) -> Result<FrameFact, FailureKind>;               // outer_position/size + 当前显示器
}

// stickymd-app::ports::monitor
pub struct MonitorSnapshot { pub displays: Vec<MonitorGeometry>, pub current: Option<MonitorIdentity> }
pub trait MonitorSource { fn snapshot(&self) -> Result<MonitorSnapshot, FailureKind>; }
// Windows 实现吸收 monitor_snapshot / identity_for_native / hex_identity；十六进制 id 格式不变（config 字节兼容）。

// stickymd-app::ports::shell
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrayView { pub window_visible: bool, pub always_on_top: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand { ToggleVisibility, ToggleAlwaysOnTop, Quit } // 与 TrayPlatformEvent 三个变体一一对应
pub trait TrayHost {
    fn create(&mut self, view: TrayView, sink: EventSink) -> Result<(), FailureKind>; // 默认 Unsupported
    fn update(&mut self, view: TrayView);
    fn destroy(&mut self);                       // 必须在主线程；ExitProcess 前调用
}
pub trait ShellLauncher { fn open_target(&mut self, target: &str) -> Result<(), FailureKind>; } // 签名对照 platform/windows/shell.rs:22 校正
pub trait ExportTargetPicker {                    // 主线程、模态、阻塞，本阶段不改
    fn choose_markdown_export(&mut self, suggested_name: &str) -> Result<Option<std::path::PathBuf>, FailureKind>;
}
pub trait FatalNotice { fn show_error(&self, title: &str, body: &str); } // 窗口创建前后都可用
```

`EventSink` 定义在 desktop（要包 `IoCompletion`/`PreviewCompletion`），端口 `TrayHost::create` 接受它，所以 `TrayHost` 实际放 desktop 的 `host.rs` 而非 app；`TrayView/TrayCommand` 可留 app。其余端口放 app。

```rust
// stickymd-desktop::host
#[derive(Clone)]
pub struct EventSink(std::sync::Arc<dyn Fn(DesktopEvent) + Send + Sync>); // 生产里包 EventLoopProxy；测试里是普通闭包
impl EventSink { pub fn post(&self, e: DesktopEvent) { (self.0)(e) } }
pub enum DesktopEvent { Io(IoCompletion), Preview(PreviewCompletion), Host(HostSignal) }
pub enum HostSignal {
    Tray(TrayCommand), ShowRequested, DisplayTopologyChanged, NoteDirectoryChanged, WatchFailed(String),
}

pub struct Keymap {   // 数据，由 Host 提供；Windows 默认值=现有表逐字
    pub clipboard_alias: fn(/* 现有签名 */) -> /* 现有返回 */,
    pub window_key: fn(/* … */) -> /* … */,
    pub view_key_mode: fn(/* … */) -> /* … */,
    pub zoom_key_command: fn(/* … */) -> /* … */,
    pub global_persistence_shortcut: fn(/* … */) -> /* … */,
} // 五个函数现居 input.rs:606-669，签名原样搬，不做类型重设计 [I]

pub trait CaretOverlay {   // 可选原生 XOR 叠加；缺省 None，回退帧缓冲绘制（presentation.rs:258-262）
    fn toggle(&mut self, rect: PhysicalRect) -> Result<(), FailureKind>;
    fn release(&mut self);
}

pub struct PresentFacts { pub width: u32, pub height: u32, pub editor_ready: bool }
pub trait Host:
    WindowHost + MonitorSource + TrayHost + ShellLauncher + ExportTargetPicker + FatalNotice + 'static
{
    fn window_attributes(&self, base: winit::window::WindowAttributes) -> winit::window::WindowAttributes;
    // 追加 with_undecorated_shadow / with_corner_preference；desktop 提供 base（含 with_decorations(false)）
    fn window_created(&mut self, window: &std::sync::Arc<winit::window::Window>, sink: &EventSink)
        -> Result<(), FailureKind>;  // 托盘、工具窗口身份、初始不透明度、探针启动
    fn keymap(&self) -> &Keymap;
    fn start_watch(&mut self, dir: &std::path::Path, sink: &EventSink) -> Result<(), FailureKind>;
    fn stop_watch(&mut self);
    fn caret_overlay(&mut self) -> Option<&mut dyn CaretOverlay> { None }
    fn after_present(&mut self, _facts: &PresentFacts) {}   // 验证探针 editor_ready/exit_requested（plan 03:81）
}
```

选择理由：`Host` 只是窄 trait 的 supertrait 捆绑，没有自己的业务方法；调用点按需写窄约束。`H: Host` 用泛型而非 `Box<dyn Host>`：宿主按 target 在编译期固定，热路径可单态化。唯一的 `dyn` 是 `EventSink`（保存完成/预览完成频率，不是逐键）与 `Box<dyn ClipboardPort>`（用户手势频率）。`Arc<Window>` 由 desktop 与 `WinHost` 各持一份，均在主线程。

### 3.3 状态拆分（`StickyApp` 的 56 个字段）

| 去向 | 字段 |
|---|---|
| `AppSession`（`stickymd-app`） | `coordinator`（改为 `EditorCoordinator<Box<dyn ClipboardPort>>`）、`persistence`、`config`、`config_persistence_allowed`、`recovery`、`worker`、`assets`、`preview_flow`、`preview_worker`、`window_flow`、`session`（内部改名 `editor`）、`search`、`parked_search`、`export_in_flight`、`zoom_config_deadline`、`started`（换为 `Box<dyn MonotonicClock>`，默认实现行为不变） |
| `ViewState`（desktop） | `projection`、`source_frame`、`source_paint_key`、`pending_redraw`、`next_blink`、`shown_title`、`preview_frame`、`preview_frame_viewport`、`preview_selection`、`preview_horizontal`、`preview_scroll_y`、`preview_focused`、`preview_dragging`、`preview_drag_moved`、`preview_press_position`、`preview_press_action`、`scrollbars`、`modifiers`、`cursor_position`、`pointer_inside_window`、`zoom_wheel`、`controls`、`system_theme`、`shell_input_enabled`、`move_resize_active`、`quit_notice`、`diagnostic`、`surface`、`window`、以及由 `native_caret_overlay/native_caret_failed` 合成的 `CaretOverlayState`（回退决策归 desktop） |
| `WinHost`（host） | `paths`、`watcher`、`_instance`、`tray`、`startup_diagnostics`；`proxy` 变为 `EventSink` |
| 删除 | `applied_window_effects`、`headless_view`、`headless_hide` 三个 `cfg(test)` 字段，由 `FakeHost` 的调用记录和可设置读回取代 |

`diagnostic` 被 12 个文件、三类所有者写：`AppSession` 通过 `ShellEffect::Diagnostic(String)` 上报，host 端口失败由 desktop 的 `effects.rs` 翻成诊断，desktop 自己的直接写。三者汇到 `ViewState.diagnostic` 一个出口。

### 3.4 `AppSession` 与输出缓冲

选择：**调用方持有的有序输出缓冲**，不用回调 trait。理由：app 侧无需知道 desktop 类型；Linux 上无需假 sink 就能断言顺序；`apply_effect` 现有扇出顺序（persistence → preview → session → search → projection → title → assets，`app.rs:288-346`）在缓冲里按发出顺序保持，desktop 按序应用，与现状等价（projection 增量只依赖当时的文档快照，resync 需要的 `coordinator.snapshot()` 经只读 `AppSession::snapshot()` 取得，plan 03:79 不暴露可变 `DocumentState`）。

```rust
// stickymd-app::session
pub enum ShellEffect {
    Window(WindowEffect),                        // Show/Hide/Focus/ApplyFrame/SetTemporarySensorTopmost/WakeAt 等，由 desktop 解释并调 WindowHost
    Projection(ProjectionChange),                // Delta{generation, delta} | FullResync
    TitleDirty, RedrawNeeded, Diagnostic(String),
    ImeAllowed(bool), CancelPreedit, EditorInputEnabled(bool), ReleaseHiddenCaches,
    Exit,                                        // 取代 Option<&ActiveEventLoop> 的层层传递
}
pub struct ShellEffects(Vec<ShellEffect>);       // 调用方复用，每个事件通过后 clear()，不逐键分配

impl AppSession {
    pub fn new(ports: AppPorts, config: ConfigCoordinator, clock: Box<dyn MonotonicClock>) -> Self;
    pub fn now_ms(&self) -> u64;
    pub fn handle_io(&mut self, c: IoCompletion, out: &mut ShellEffects);
    pub fn apply_app_effect(&mut self, e: AppEffect, out: &mut ShellEffects);
    pub fn dispatch_window_intent(&mut self, i: WindowIntent, out: &mut ShellEffects);
    pub fn on_focus_changed(&mut self, focused: bool, out: &mut ShellEffects); // lifecycle.rs:65-70 的 SaveNow(FocusLoss) 归此
    pub fn settle(&mut self, now_ms: u64, out: &mut ShellEffects);             // settle_event_pass + zoom 去抖
    pub fn mutation_allowed(&self, preview_focused: bool, shell_input_enabled: bool) -> bool; // W6
    pub fn guards(&self, view: ViewFacts) -> GuardSnapshot;
    pub fn next_deadline(&self) -> Option<u64>;
    pub fn snapshot(&self) -> DocumentSnapshot;                                // 只读
}
```

`AppSession` 在内部消化 `RequestNoteSave`、`RequestSafeAssetGc`、`RequestConfigFlush`、`CommitPlacement`（写 config，夹取常量用 `MIN_WINDOW_*_DIP` 与改为 `pub` 的 `MAX_WINDOW_DIP`）、配置确认与 `settles_quit`、`WorkerStopped` 屏障扇出，不外发。desktop 的 `effects.rs` 只解释外发的那一部分。`Exit` 在 `handler.rs` 每个 `ApplicationHandler` 回调末尾消费：先 `host.destroy()` 托盘，再 `event_loop.exit()`（W7）。

Esc 顺序（plan 04:248-250：透明度弹层、IME、Search、Preview 选区）是视图状态消费，留 desktop 的 `input/escape.rs`；未被消费才变成收起 intent 交给 `AppSession`。

### 3.5 批次

每批通用出口（plan 11:227-235）：

- E1 治理随代码移动：同一提交更新硬编码路径检查；新增业务 module 带 `plan_ref`；旧检查不空转。
- E2 测试选择与指纹：旧规则同提交删除或改指；纯搬迁不继承旧证据（旧收据保留历史身份）。
- E3 数据字节兼容：数据副本无操作启动、保存、恢复，比对 note/config，再用前一检查点重开。触及保存、退出、放置、配置的批（C4、C5、C6、R）必做，其余写静态证据。
- E4 性能（plan 10:70-87）：只要帧路径、输入路径、所有权或编译条件之一变化，就不能静态豁免，必须先有计数、再比较。C1-C6 都改变编译条件，全部要比。
- 每个提交单独过 fmt、clippy、定向测试、governance、指纹选择；搬迁提交检查 `git ls-files --eol`；不得与变异测试同时进行（MEMORY：mutation-run-safety）。

#### P0 文档与基线（不改产品代码）

- 本文提交后请 USER 批准（AGENTS 流程第 8 条，骨架级）。
- M0a 帧复制基线：见 §5。
- M1 Linux 后端实验：WSL Debian 临时分支，给 winit/softbuffer 选 §3.1 的 feature，`cargo build --locked`，记录 `Cargo.lock` 增量、是否需要系统 `-dev` 包、是否拉入 gtk、`cargo deny` 的 Linux target 结果。
- 任务文件与 `docs/acceptance-cases/phase-15.md` 追加 C 行，保持 `NOT TESTED`。
- 出口：M0a、M1 数据落盘；USER 的 §7 决定。

#### C1 原位：帧呈现接缝与 `age` 修正（仍在 `apps/stickymd-win`）

- 搬什么：不搬。把 `surface.rs` 拆成纯函数 `plan_present(age: u8, buffer_len: usize, pixmap_len: usize, damage: DamageRect, w: u32, h: u32) -> Result<PresentPlan, SurfaceError>`（`PresentPlan::{Full, Damage(DamageRect)}`，`age != 1` 一律 `Full`，先看 age 再裁剪）与纯转换 `convert(plan, rgba, native: &mut [u32], w, h) -> u64`（返回写入字节数）；`present_damage` 退化为 buffer_mut → plan → convert → present/present_with_damage 的胶水。加 `FrameLedger`（§5）。
- 测试：`age = 0/1/2/3` 四个用例，用 `Vec<u32>`：age 1 只写 damage 且 damage 外哨兵不变并选 `Damage`；age 0/2/3 全覆盖无残留并选 `Full`；age≥2 且裁剪后为空仍选 `Full`；长度不匹配三种 age 都报错。保留 `phase9_caret_damage_conversion_does_not_touch_other_pixels`。
- 登记：`surface.rs` 的指纹规则不变（文件仍在原处）。
- 出口：Windows 实际序列（首帧 age 0，之后 age 1）的字节数与 M0a 逐项相同；E4 比较。

#### C2 建立 crate 与全部登记，搬入 geometry、title、frame

- 搬什么：新建 `crates/stickymd-desktop`；`PaneRect`、`ViewGeometry`、`geometry()` 及唯一的 `TOOLBAR_HEIGHT_DIP`（消除 f32/f64 两份，保持 `preview_runtime.rs:380` 与 `controls.rs:71` 现有取整路径不变）；`title.rs`（`title.rs` 前 91 行，无依赖）；`surface.rs`、`frame/plan.rs`、`frame/convert.rs`、`frame/ledger.rs`。`SoftwareSurface` 仍持 `Arc<winit::window::Window>`。
- 测试迁移：`surface.rs` 的 1 个测试、`title` 的 8 个测试、C1 的 age 测试。首次在 Linux 上运行。
- 登记（同一提交，A 批 `fe20bdd` 为样板）：
  - `tools/stickymd-smoke/src/headless.rs`：`Module::Desktop` 插在 `App` 与 `Windows` 之间（`Ord` 枚举序即选择序，A 批 tests 里有写死序列）；`ALL` 7→8；`name()="desktop"`；`package()=Some("stickymd-desktop")`；`root()="crates/stickymd-desktop/"`；`dependencies()`：`Desktop=>[Core,Render,App]`，`Windows` 追加 `Desktop`。
  - `ci.rs:86-89` `Checks.portable` 加 `Desktop`；`ci/projection.rs:55` 的 `[Core,Render,App]` 加 `Desktop` 并改同文件对 `portable_args` 的断言；`.github/workflows/ci.yml:196-197` job 名；`ci/workflow_tests.rs:21,34` 断言。
  - 根 `Cargo.toml` members；`governance/manifest_harness.rs:524-530` 与 `qualification/module_ledger/reuse_tests.rs:227-232` 两处硬编码清单。
  - 指纹 `qualification/module_ledger/fingerprint.rs`：`is_global_input`（`:201`）加 `crates/stickymd-desktop/Cargo.toml`；`PRODUCT_ROOTS`（`:309`）加 `crates/stickymd-desktop/src/`；每个新文件在 `EVERY_SESSION`（`:330`）或 `FEATURE_SOURCES`（`:400`）有显式规则；已迁出文件的旧规则同提交删除；`fingerprint/tests.rs:687` 对 `app/controls.rs` 的引用在 C3 改。
  - `governance.rs:651-655` 的 `forbid(unsafe_code)` 列表加 `crates/stickymd-desktop/src/lib.rs`；新增 `verify_shared_desktop_boundary`（仿 `:1342`）：源码禁 `windows::`、`HWND`、`HMONITOR`、`cfg(windows)`、`target_os`、`crate::platform`、`winit::platform::`；允许其余 `winit::`、`softbuffer::`（现有 app 版本禁了这两个，不能直接复用）；清单禁 `windows`、`arboard`、`tray-icon`、`notify`；源码树为空时报错。
  - 测试所有者 `qualification/manual_observation/coverage/table.rs:46-52` 仿 `app()` 加 `const fn desktop(path)`（`OwnerTarget::Lib`）；迁走的 `win("app::…")` 条目改 `desktop(...)`，前缀变短。
  - 名字过滤任务（`runner.rs`、`runner/performance.rs`、`runner/filtered_tasks.rs`）：`phase5_`、`phase6_`、`phase8_`、`phase10_`、`phase15_`、`render_win_tests`、`core_win_tests` 的 `-p` 与常量（如 `APP_WINDOWS`、`RENDER_APP_WINDOWS`）加 `stickymd-desktop`；`filtered_tasks.rs:213` 的 `phase15.unselected` 断言复核；名字过滤静态核对保持开启（Cargo 对不匹配过滤器也返回 0，测试搬走会静默变空）。
  - `deny.toml` Linux target 与 SBOM 见 §7。
  - 文档：`AGENTS.md` 目录图、`docs/coverage-matrix.md:53`、`docs/overview/architecture.md:46-50`、`CHANGELOG.md`、`CONTRIBUTING.md:304-307`、plan 11 `:117-118`、任务文件与 `phase-15.md`。
- Linux 特征：此批把 §3.1 的 feature 写进 manifest，更新 `Cargo.lock`。
- 出口：Windows 与 Linux CI 真实构建 `stickymd-desktop`；`verify_workspace_listing`（`runner/headless.rs:87-99`）无拒绝；E1-E4。

#### C3 纯叶子搬迁（不含 `impl StickyApp` 的部分）

先写特征测试（搬之前，在原位）：toolbar、scrollbar、Search 在 DPI 1.0 与 1.5 的像素哈希；`ControlLayout` 在两种 DPI 下的矩形。
- C3a `controls`（`ControlLayout`、`ControlState`、不透明度输入）+ `toolbar_paint`。`winit::dpi` 保留，不换。
- C3b `scrollbar`（几何、绘制、交互状态）。
- C3c `search_runtime`（布局与绘制）。
- C3d `escape` 决策表、`Keymap` 类型与 `Keymap::ctrl_standard()`（逐字等于现有表）；`input.rs:606-669` 的五个函数迁入 `input/keymap.rs`，由 host 的 `keymap()` 返回它（USER 决定 D3）。
- C3e `preview_input` 中与 `StickyApp` 无关的几何命中。`toolbar_mode_at_cursor` 的去留见 §7 D4，未决定前原样搬，不改行为。
- 各文件里仍含 `impl StickyApp` 的粘合代码留在 win，作为不保存状态的转发（plan 03:114-115 允许）。
- 测试迁移：约 71 个纯函数测试中，`controls` 12、`scrollbar` 11、`escape` 5、`search_runtime` 4、`toolbar_paint` 2 随文件搬；`caret_runtime.rs:240` 用 `CaretOverlayRect` 的断言留 win。所有者表同步。
- 治理：这些文件的 `plan_ref` 指向 `09_windows_shell.md#…` 的既有锚点（`window-geometry`、`vertical-scrollbars`、`theme-opacity-and-content-zoom` 等），[V] plan 09 `:26-28` 已声明共享 desktop 实现控件与绘制，锚点不必重指，不改 plan。
- 出口：像素哈希不变；E1-E4。

#### C4 `AppSession`（原位建立，最后一步才进 `stickymd-app`）

- C4a 纯规则下沉：`interaction/rules.rs` 的函数（§3.1）。每条带 Linux 可跑的等价性测试，覆盖迁前行为的边界：失焦 dirty 才发 `SaveNow(FocusLoss)`、恢复期间不启动看门狗、`mutation_allowed` 对（恢复、Preview 焦点、退出等待、资产对账、`shell_input_enabled`）的真值表、`clamp_placement` 的 220/120/16_384、`replacement_*`、`scope_to_selection`、主题轮转顺序。搬迁前先确认失焦保存有直接测试，没有就在原位补一个。
- C4b `AppSession` 结构：按簇移字段（note/persistence/recovery/worker → assets/export → preview → config/window_flow/search），`StickyApp` 持 `app: AppSession`；零行为变更。测试只改字段路径（`app.coordinator` 引用约 81 处），不改断言。
- C4c 输出缓冲：`ShellEffect`/`ShellEffects` 取代 `Option<&ActiveEventLoop>` 的传递（`assets_runtime.rs`、`note_effects.rs`、`persistence_runtime.rs`、`window_runtime.rs` 共 86 处提到 `event_loop`）；`Exit` 在回调末尾消费（W7）。
- C4d `Box<dyn ClipboardPort>` 与转发 impl。
- 文件移到 `crates/stickymd-app/src/session/` 与 `interaction/rules.rs` 时：app 的指纹规则、`plan_ref`、`verify_shared_application_boundary` 继续生效（不得出现 winit）。
- 测试：回执测试 36+17+6 与 `hot_path_tests` 每个提交都要绿；新增"屏障未结算时 `Exit` 不出现"和"`Exit` 先于托盘销毁"两个反向测试。W1：这些测试留在 win。
- 出口：E3 必做；E4 用 `hot_path_tests` 的每键快照与每次编辑构建计数。

#### C5 `Host` 与窄端口（原位，`StickyApp<H>` 泛型化）

子提交按风险递增，每个单独绿：①`FatalNotice`、`ShellLauncher`、`ExportTargetPicker`；②`TrayHost` + `TrayCommand`（`TrayPlatformEvent` 中立化）；③`MonitorSource` + `HostSignal::DisplayTopologyChanged`（消息钩子 `native_message.rs` 只产生该信号，非客户区拖动修复留在 host 内部）；④`WindowHost`（语义保持"发起即成功"，`hide` 仍带回执，`show/focus/set_topmost/apply_frame` 的新读回值本批忽略）；⑤`CaretOverlay` + `after_present`（热路径：无逐帧分配）；⑥`EventSink`、`start_watch`/`stop_watch`、唤醒监听（`ShowRequested`）。fixture 换成 `FakeHost`，删三个 `cfg(test)` 字段；`HeadlessApp` 不再需要 `with_any_thread` 事件循环。
- 治理：`governance.rs:670` 的四个标记按 W3 分配；`:686` 的 `enumerate_active_displays()` 恰好一次检查改指 `MonitorSource` 的 Windows 实现文件，其禁用词检查（`DocumentState`、`std::fs`、`File::`）随之迁移。
- 出口：E3 必做（`ExitProcess`、唤醒、单实例所有权、drop 顺序：`SingleInstanceGuard` 晚于 worker 停止）；E4。

#### C6 机械搬家：`app.rs`、`app/` → `stickymd-desktop::shell` 等

- 只做 `git mv` 与 import 修改，可用 `git diff -M` 审阅；`StickyApp<H>` 改名 `DesktopShell<H>`，`impl ApplicationHandler<DesktopEvent>` 进 `shell/handler.rs`；win 的 `main.rs` 只装配并调用 `stickymd_desktop::run`：

```rust
pub fn run<H: Host>(el: winit::event_loop::EventLoop<DesktopEvent>, host: H, session: AppSession)
    -> Result<(), RunError>;
```

- 测试：只依赖 `FakeHost` 的 fixture 测试可迁入 desktop（不是出口条件）；用 `win_io_ports`/真实磁盘的回执测试留 win，经 `DesktopShell` 的 `testing` feature 访问器驱动（W1）。
- 治理：`verify_shared_desktop_boundary` 对完整源码树生效；E2 指纹旧规则（`apps/stickymd-win/src/app/` 整目录、`surface.rs`）同提交删除。
- 出口：Windows 与 Linux CI 真实构建；`age` 测试；M0a 逐项相同；`hot_path_tests` 绿；E3 必做。

#### D 收口

删除所有"只为迁移保留"的转发；审查：`rg "winit|softbuffer" crates/stickymd-app` 为空，`rg "windows::|platform::windows|cfg\(windows\)" crates/stickymd-desktop` 为空；适用 baseline（fmt、clippy、tests、governance、相关 benchmark）；文档同步；Debian/macOS 保持 `NOT TESTED`。

**此处 Develata 跑桌面回归 #1**（预期零行为差异）。范围：IME（微软拼音：组字、候选窗位置于 DPI 1.0 与 1.5、提交、Search 框内组字、恢复期间输入被拒、托盘唤醒后立即输入）、托盘、透明度、置顶、贴边、多屏、剪贴板、导出。局部 bug 在回执批之前修完。

#### R 回执批（行为变更，回归 #1 之后）

R1 显示、R2 置顶、R3 定位，设计见 §4。结束后只跑窄回归 #2：B decisions 报告 `:27-35` 的五个场景。

## 4. 显示、置顶、定位回执

共同原则（plan 04:253-266；`docs/report/2026-11-07-phase-15-b-decisions.md`）：回执只证明明确列出的后置条件；提交移到成功回执之后；新 id 使旧 id 失效，迟到回执不提交；失败或无法确认时不提交成功状态及其依赖效果。三层分工：状态机在 `stickymd-app/flow/window`，执行与读回在 `WindowHost` 的 Windows 实现，解释在 desktop `shell/effects.rs`。状态机与 `FakeHost` 测试都在 Linux 可跑，只有 Windows 读回是新原生代码。动作 id 沿用 `last_window_action` 的 `u64` 计数。

### R1 显示

- 现状 [V]：`show()` 在发 `ApplyFrame`/`ShowWindow`/`FocusWindow` 前先写 `Presented(restore)`，同一次派发还发 `SetImeAllowed(true)`、`SetEditorInputEnabled(true)`；`show_native`（`window_runtime.rs:469-478`）不读回，无条件翻托盘文字；已可见但收起或最小化时只发 `FocusWindow`（无 id）。
- 改法：`WindowEffect::ShowWindow { id }`、`WindowEffect::FocusWindow { id }`；状态 `VisibilityState::ShowPending { id, restore }`。desktop 调 `WindowHost::show()/focus()`，把 `Visibility` 映射成 `WindowIntent::ActionCompleted { id, outcome, now_ms }`：`visible==Some(true) && minimized==Some(false)` 为 `Ok(())`；`Some(true)` 的最小化或可见为 `Some(false)` 为 `Err(Failed)`；任一为 `None` 为 `Err(Unsupported)`。已有 `complete_window_action` 扩展到接受 `ShowPending`。
- 成功回执才提交：`Presented(restore)`、`SetImeAllowed(true)`、`SetEditorInputEnabled(true)`、Preview 恢复与 `configure_viewports`（现在在 `ShowWindow` 效果里，`window_runtime.rs:311-321`）、托盘文字。
- 拒绝、`Unsupported` 或过期：保持原可见性，不启用输入，不恢复 Preview，发 `ShowRejected(FailureKind)` 诊断；同目标可再次申请（等待能结束）。
- 不声称焦点、未遮挡、定位：焦点只由真实 `Focused(true)` 证明。
- 风险：回执被拒时输入法一直关闭，必须有"失败后重试能结束等待"的测试。

### R2 置顶

- 现状 [V]：用户路径先 `config.update`（`preferences.rs:44`）再 `ApplyAlwaysOnTop`，失败只写诊断；感应置顶在 reducer 里先改 `temporary_sensor_topmost` 再发效果（`reducer.rs:442-449` [I] 来自调研）；`window_topmost.rs:44-54` 只检查 `Result`，没有读回。
- 改法：请求 `TopmostRequest { id, desired_effective: bool, source: TopmostSource::{User, Sensor} }`；状态按来源分存 `user_confirmed/user_pending(id,bool)` 与 `sensor_confirmed/sensor_pending(id,bool)`，有效值 = `effective_topmost(user, sensor)`。`WindowHost::set_topmost(on)` 同步 `SetWindowPos(NOACTIVATE)` 后读回 `GetWindowLongPtrW(GWL_EXSTYLE) & WS_EX_TOPMOST`（**新代码**），`topmost == Some(on)` 为成功。
- 用户值只在成功回执后 `config.update(always_on_top)` + `submit_config_if_needed` + 托盘勾选；失败不写 config。临时感应值与用户值分离，`HidePending` 保存/恢复的 `sensor_topmost`（`lifecycle.rs:174-178` [I]）改为读 confirmed 值。同来源的新 id 取代旧 id。
- 只证明 topmost 位；不抢焦点和与别的窗口的层级仍是桌面观察项。winit `set_window_level` 无返回值、异步（`SWP_ASYNCWINDOWPOS`），不能作为回执路径，保留原生 adapter。

### R3 定位

- 现状 [V]：`apply_frame`（`window_geometry_runtime.rs:83-86`）是发后不管，`request_inner_size` 结果被丢弃；`CommitPlacement` 与 `ApplyFrame` 在同一次派发发出（拖动结束、拓扑恢复）；动画靠计时结束；`WindowEvent::Moved` 被忽略（`lifecycle.rs:317`）。
- 改法：`WindowEffect::ApplyFrame { id, target: PhysicalRect, last: bool }`。动画中间帧 `last=false`，只是呈现请求，无回执（plan 04:265）。非动画定位与动画最终目标 `last=true`，各有且仅有一次终结裁决：目标确认、执行失败、或确认超时三选一。
- desktop 在 `apply_frame` 之后，于 `Moved`、`Resized`、`ScaleFactorChanged` 事件与注入时钟的 `WakeAt` 到期时调用 `WindowHost::observe_frame()`，送入 `WindowIntent::PlacementObserved { id, observed: FrameFact, now_ms }`。reducer 在容差内相等则确认，并由**已接受的观察几何**派生 `CommitPlacement`（plan 04:266）；超时则 `Failed`，不提交、写诊断；更新请求使旧 id 失效，迟到观察不提交。
- Wayland：`set_outer_position` 不受支持（winit `window.rs:731`），回执为 `Unsupported`；贴边体验需 plan 09:30 另行批准，本阶段不处理。
- 容差：混合 DPI 下观察矩形与目标矩形的比较容差未定（[U]）。M6 实测前不预设数值；以 `PLACEMENT_TOLERANCE_PX` 常量暴露，并在 plan 04 窗口回执节补一句（USER 决定 D6）。

## 5. 帧复制计数（先于 C1 建立）

plan 10:76-78：帧路径相关因素有一项改变就不能静态豁免，必须在第一次改变前建立计数。`render()` 在 `surface` 为 `None` 时直接返回（`presentation.rs:246-248`），而 `SoftwareSurface::new` 需要真实窗口，所以无窗口测试跑不到。

**M0a（P0，在基线 SHA 上，临时补丁，不提交）**：开工时 `git rev-parse HEAD` 确认基线（快照为 `ae09b67`）。在下列站点加原子计数，记录"写入目的缓冲的字节数"：

| 计数器 | 站点 | 字节 |
|---|---|---|
| `fill_bytes` | `presentation.rs:249` 窗口 pixmap 清屏 | W×H×4 |
| `pane_blit_source_bytes` | `presentation.rs:256`，`blit_pixmap` 定义 `:329-343` | pane 宽×高×4（按窗口裁剪） |
| `pane_blit_preview_bytes` | `presentation.rs:277`，`render/preview/paint.rs:114-126` 的 `blit_to` | 同上 |
| `convert_full_bytes` | `surface.rs:110-118,167-171` `copy_rgba_full` | W×H×4 |
| `convert_damage_bytes` | `surface.rs:130-164` 局部分支 | damage 宽×高×4 |
| `damage_blit_bytes` | `caret_runtime.rs:57,161-175` | damage 宽×高×4 |
| `full_presents`、`damage_presents`、`pixmap_allocs` | `present`、`present_damage`、`SoftwareSurface::resize`（`surface.rs:93-107`） | 次数；`pixmap_allocs` 为诊断，不阻断 |

场景：窗口 900×600（与 `hot_path_tests.rs` 一致），DPI 1.0 与 1.5；Source、Preview、Split 各一次 Full；Source 一次 CaretOnly。用 Release 构建的真实窗口运行，借 `exit_requested()` 探针（`presentation.rs:307-312`）在首帧后退出。结果写入 `docs/tasks/phase-15-shared-core-migration.md`。

**永久化（C1）**：`FrameLedger`（`frame/ledger.rs`）是 `SoftwareSurface` 持有的一组 `u64`，只在上述站点做整数加法，无分配，无原子；`convert()` 返回写入字节数供胶水累加。验收口径：同 fixture 的各项字节之和，每批不得高于 M0a。`hot_path_tests` 继续守每键全文快照与每次编辑构建次数。

不要在迁移中修的既有成本：`presentation.rs:201` 每帧 `diagnostic.clone()`、`presentation.rs:255-256` 的 Source 帧拷贝（要改就单独成批，否则计数比较失去意义）。

## 6. 其他度量

| 编号 | 内容 | 时间 |
|---|---|---|
| M1 | Linux 后端 feature 实验（§P0） | P0 |
| M2 | 每键全文快照、每次编辑构建数 | 每批，`hot_path_tests` |
| M3 | IME 手测脚本（见回归 #1 范围） | 回归 #1；有条件时先在基线 SHA 上跑一遍作对照 |
| M5 | 显示、置顶、定位桌面观察表 | R 批前，直接用 B decisions 报告 `:27-35` |
| M6 | 混合 DPI 目标矩形与读回矩形偏差 | R3 前，需要 Develata 的多显示器/混合 DPI 硬件 |

Windows softbuffer `age()` 的运行时序列：源码为证（`win32.rs:308-313`：已 present 过为 1，否则 0），运行时序列 [U]；C1 的 M0a 数字可间接确认（首帧全量，之后局部）。

## 7. 需要 USER 决定的事

必须在 C2 前决定（改变批次形态）：

- D1 批准本设计为骨架级变更：`StickyApp` 拆成 `DesktopShell<H>` + `AppSession` + `WinHost`；端口 trait 在 `stickymd-app`，`Host` 在 `stickymd-desktop`；`ApplicationHandler` 的 impl 归 desktop（plan 03:80 事件翻译），`main.rs` 构造与 `run` 调用归 host（03:81 启动接线）。
- D2 里程碑 C 与 D 是同一候选上的两个标签：因为 `ApplicationHandler` 依赖 `AppSession`，C 的出口回执与 D 的业务裁决迁出无法分先后。需要一句 plan 11 修订：C 与 D 在同一候选 SHA 收口，C1-C3 的 Linux 构建与 `age` 测试各自成立，桌面回归在 D 后执行一次。我不替你改 plan。
- D3 `Keymap` 归属：类型与 `Keymap::ctrl_standard()`（逐字等于现有表）放 desktop，Windows host 返回它（Debian 可直接复用）；还是只放 host。我建议前者，两者都符合 plan 03 第 9 条（宿主选择、Windows 默认值不变）。
- D4 `preview_input.rs:99-114` 的 `toolbar_mode_at_cursor`（W2）：先做特征测试；若实际不可达就删除，若可达则这是与绘制不一致的命中区，保留还是改用 `ControlLayout` 是行为变更，需要你决定。
- D5 回归节奏：默认两次（D 之后一次宽，R 之后一次窄）。合并为一次的代价是结构搬迁与回执变更混在一次观察里难以归因，且 R3 需要混合 DPI 硬件；我不建议合并，但听你的。同时请确认 R 批在 Phase 15 之内（plan 04 的回执合同目前是 implementation drift），而不是单独的后续。

可在对应批次前决定：

- D6 R3 容差（M6 之后）与 plan 04 补一句。
- D7 Linux 依赖面：`Cargo.lock` 增长是否可接受；`deny.toml:3` 只检查 Windows target，要不要加一个 Linux target；`release/notices/mod.rs:33-38` 用 `--filter-platform x86_64-pc-windows-msvc`，Linux 后端不进 `THIRD_PARTY_NOTICES`，对 Windows 发布是对的，SBOM 是否受影响 [U]；ubuntu runner 是否需要额外 `-dev` 包 [U]（M1 回答）。
- D8 文件监视：`note.md`/`note.md.tmp` 过滤（`file_watch.rs:66-70`）是产品逻辑，建议作为纯函数下沉到 `stickymd-app`；`NoteDirectoryWatcher` 封装是否也共享（允许 desktop 依赖 `notify`）留到 D 批决定，Debian 宿主需要时再议。默认：封装留 host。
- D9 链接 scheme 白名单（现在在 `shell.rs:22` 的 Windows adapter 内）是否提到 app 作为共享策略。对 Windows 行为中性，但属安全规则。
- D10 导出对话框阻塞主线程（`export_dialog.rs:42`）、拖动结束依赖左键释放（`lifecycle.rs:311`，Wayland 上可能不触发）：本阶段保持现状。

## 8. 为什么这样排序

- 回归次数：USER 的建议（抽完再跑）与 plan 11:224 的 C 出口一致，只需 D2 的一句修订；回执是行为变更，且依赖 `WindowHost` 已存在，所以放在回归 #1 之后。
- 先原位解耦再机械搬家：plan 03:114-115 要求每批只保留一个生效实现；搬家提交只含 `git mv` 与 import，可审阅；语义变化发生在所有测试仍可在 Windows 上跑的位置。
- 先建 `AppSession` 再搬 `ApplicationHandler`：`lifecycle.rs` 内含业务裁决（看门狗门控、失焦保存、`settle_event_pass`），plan 03:80 不允许 desktop 持有，否则搬过去的 desktop 违反边界。
- 帧路径先行：C1 在任何跨 crate 搬迁之前把 `age` 缺陷和计数固定下来，之后每批都有可比的数。

## 9. 已验证 / 推断 / 未验证

已验证：§1 全部 [V] 项；`WindowEffect` 变体表（`flow/window/state.rs:262-292`）；`ports/mod.rs` 现有端口（assets、config、export、failure、note、preview）；plan 03:76-115、plan 06:333-340、plan 11:215-245、plan 10:66-90、plan 04:244-266；winit/softbuffer 源码行为；`governance.rs:664-700`；`headless.rs` 的 `Module`；`deny.toml:3`；`controls.rs:11-13,60-135`。

推断：`Keymap` 五个函数的签名原样可搬；`MonitorGeometry` 已含工作区与缩放（`monitor_geometries(&window)`，未逐字段核对）；`reducer.rs:442-449` 与 `lifecycle.rs:174-178` 的置顶细节（来自调研，未重读）；回执测试留 win 的划分；各 feature 名。

未验证：任何编译、测试、桌面回归结果；Linux 上 `cargo build -p stickymd-desktop --locked`；帧复制字节、输入延迟、内存的实际数字；Windows softbuffer `age()` 运行时序列；混合 DPI 容差；SBOM 与 `-dev` 包；失焦保存是否已有直接测试；`input.rs:99-283` 中 IME 与快捷键判断顺序细节（C6 前要读）。
