# StickyMD Phase 8 — Windows Desktop Shell, Docking, Tray, Theme, Opacity & Multi-Monitor Finalization

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–7 已完成。

当前系统已经具备：

```text
DocumentState
Source Editor + Unicode/IME architecture
Portable Persistence
Autosave / Recovery / Conflict
Native Markdown Preview
RaTeX Native Math
Managed Images
Asset GC / Undo Asset Effects
Portable Export
```

Phase 7 commit：

```text
a7af3a40fa357edd36cd1ae231e1c936f1b763bd
```

USER 已明确批准进入 Phase 8。

本阶段名称：

> **Phase 8 — Windows Desktop Shell, Docking, Tray, Theme, Opacity & Multi-Monitor Finalization**

---

# 0. 本阶段的核心目标

Phase 8 将已有可靠的编辑、持久化、Preview、Math 和 Assets 核心包装成最终 Windows 11 桌面便签行为。

本阶段正式完成：

```text
Paper Window
    │
    ├─ Custom undecorated shell
    ├─ Fixed shadow / rounded corners
    ├─ Source / Split / Preview controls
    ├─ Always-on-top
    ├─ Light / System / Dark
    ├─ Whole-window opacity 70–100%
    ├─ Manual collapse
    └─ Close → Hide to Tray

Window Placement
    │
    ├─ Floating
    ├─ Left Dock
    ├─ Right Dock
    ├─ Top Dock
    ├─ Expanded
    └─ Collapsed → 3 DIP sensor strip

Auto Hide
    │
    ├─ hover sensor 100 ms
    ├─ focus loss 700 ms
    ├─ hover leave 500 ms
    ├─ Esc/manual immediate
    └─ 140 ms slide animation

Multi Monitor
    │
    ├─ stable monitor identity
    ├─ work-area docking
    ├─ mixed DPI
    ├─ negative coordinates
    ├─ monitor disconnect
    └─ primary-monitor recovery

System Tray
    │
    ├─ 显示/隐藏
    ├─ 置顶
    └─ 退出
```

---

# 1. Phase 8 是 Desktop Shell 阶段，不是新功能阶段

本阶段禁止扩展 StickyMD 本体。

不得新增：

```text
New/Open
multiple notes
tabs
file tree
settings page
plugin system
global hotkey
auto launch
auto update
cloud
network
sync
search
Markdown syntax highlighting
image editor
PDF/HTML export
```

---

# 2. 本阶段明确不做

Phase 8 禁止：

```text
MSI
MSIX
Microsoft Store
installer
auto updater
release signing
release website
crash telemetry
analytics
startup registry
scheduled tasks
Windows service
global keyboard shortcuts
shell extension
Explorer context menu
```

这些不是 Desktop Shell 必需能力。

---

# 3. Phase 7 遗留人工条件必须继续保留

Phase 7 Recommendation：

```text
APPROVE Phase 8 WITH CONDITIONS
```

以下当前仍可能为：

```text
NOT TESTED
```

至少包括：

```text
Microsoft Pinyin real verification
WeChat Input Method real verification
real clipboard application tests
real Windows visual validation
mixed-DPI visual validation
native export dialog validation
junction/symlink real validation
crash-kill recovery
RaTeX visual matrix
Preview visual matrix
```

Phase 8 可以借真正 Desktop Shell 环境关闭其中一部分。

但：

> 自动化 PASS 不能被重新命名成真实人工 PASS。

无法执行的继续写：

```text
NOT TESTED
```

---

# 4. Phase 8 成功后的最终用户行为

运行 portable：

```text
D:\Notes\Math\
StickyMD.exe
```

用户应看到一张极简纸片窗口。

窗口可以：

- 编辑 Markdown。
- Preview。
- Split。
- 显示公式和图片。
- 自动保存。
- 导出。
- 置顶。
- 调透明度。
- 切主题。
- 拖动。
- resize。
- 拖到左/右/顶部吸附。
- 失焦后自动缩入屏幕边缘。
- 鼠标触碰 3 DIP sensor 后展开。
- 关闭按钮隐藏到 Tray。
- 从 Tray 唤回。
- 从 Tray 真正退出。

这是 v1 Desktop behavior 的最终形态。

---

# 5. 开始前必须读取

严格执行最近适用的 `AGENTS.md`。

至少读取：

```text
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md

docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/05_document_persistence.md
docs/plan/06_markdown_math_rendering.md
docs/plan/07_editor_and_ime.md
docs/plan/08_assets_and_export.md
docs/plan/09_windows_shell.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md

docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md

docs/report/phase-03-source-editor-ime.md
docs/report/phase-04-portable-persistence.md
docs/report/phase-05-markdown-native-preview.md
docs/report/phase-06-ratex-native-math.md
docs/report/phase-07-managed-images-export.md

docs/tasks/phase-07-managed-images-export.md
```

---

# 6. Phase 7 Gate

确认：

```text
Recommendation:
APPROVE Phase 8 WITH CONDITIONS
```

以及 USER 已批准。

如果仓库实际 report 中存在：

```text
STOP — architecture review required
```

立即停止。

---

# 7. Repository Preflight

执行：

```bash
git status --short
git branch --show-current
git log -12 --oneline

cargo metadata --no-deps

cargo tree -p stickymd-core
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```text
branch
starting commit
clean / dirty
```

不得：

```text
git reset
git clean
git rebase
覆盖 USER 未提交修改
```

---

# 8. Phase 8 架构原则

本阶段必须继续维持：

```text
DocumentState
=
Markdown runtime authority
```

Desktop Shell 不得获得 Markdown authority。

窗口壳只处理：

```text
presentation
window placement
window visibility
focus
timers
tray
theme
opacity
platform window state
```

---

# 9. Window Placement 也必须有 Single Source of Truth

运行时：

```text
WindowShellState
```

是窗口逻辑状态 authority。

Windows 实际：

```text
HWND rect
focus
monitor
```

属于 platform fact。

`config.toml`：

```text
durable projection
```

---

# 10. 禁止三重窗口 Authority

不得让：

```text
ConfigState coordinates
WindowShellState coordinates
HWND current coordinates
```

三者并列作为“最终真相”。

正确关系：

```text
WindowShellState
        │
        ├─ project → HWND
        └─ persist → ConfigState/config.toml

HWND external movement
        │
        ▼
reconcile
        │
        ▼
WindowShellState
```

---

# 11. 四层调用映射

Phase 8 正式路径：

```text
Interaction Shell
      │
      ▼
Window/Tray Intent
      │
      ▼
WindowShellCoordinator
      │
      ▼
Platform Capability
      │
      ▼
winit / Windows Adapter
```

---

# 12. Interaction Shell

负责：

```text
draw controls
hit testing
pointer capture
keyboard shell commands
window drag gesture
window resize gesture
opacity slider interaction
theme selector interaction
tray event translation
```

---

# 13. Interaction Shell 不得直接调用

```text
SetLayeredWindowAttributes
QueryDisplayConfig
SetWindowPos
tray menu mutation
config storage
```

---

# 14. Instruction Interface

至少建立/整理：

```text
SetViewMode(...)
SetTheme(...)
PreviewOpacity(...)
CommitOpacity(...)
SetAlwaysOnTop(...)
RequestWindowDrag
RequestWindowResize(...)
RequestCollapse
RequestExpand
HideToTray
ShowFromTray
RequestQuit
TrayToggleVisibility
TrayToggleTopmost
```

名称按已有 architecture 调整。

---

# 15. Pure presentation event 不需要 Intent

例如：

```text
button hover
cursor move
caret blink
control fade progress
```

仍是 shell presentation。

---

# 16. Flow Coordinator

正式建立/收敛：

```text
WindowShellCoordinator
TrayCoordinator
DisplayTopologyCoordinator
```

不要创建 generic UI coordinator framework。

---

# 17. Window state 建议

不要使用十几个互相矛盾 bool。

建议概念：

```rust
enum WindowVisibility {
    Visible,
    HiddenToTray,
}

enum PlacementMode {
    Floating(FloatingPlacement),
    Docked(DockedPlacement),
}

enum DockRevealState {
    Expanded,
    Collapsed,
    Animating(DockAnimation),
}
```

---

# 18. FloatingPlacement

至少：

```rust
struct FloatingPlacement {
    monitor: MonitorIdentity,
    width_dip: f32,
    height_dip: f32,
    x_ratio: f32,
    y_ratio: f32,
}
```

---

# 19. DockedPlacement

至少：

```rust
struct DockedPlacement {
    monitor: MonitorIdentity,
    edge: DockEdge,
    width_dip: f32,
    height_dip: f32,
    offset_ratio: f32,
}
```

---

# 20. DockEdge

严格只有：

```rust
enum DockEdge {
    Left,
    Right,
    Top,
}
```

禁止：

```text
Bottom
```

---

# 21. 为什么无 Bottom

避免：

```text
taskbar
auto-hide taskbar
system tray
```

冲突。

这是冻结产品决定。

---

# 22. WindowShellState

可以：

```rust
struct WindowShellState {
    visibility: WindowVisibility,
    placement: PlacementMode,
    dock_reveal: Option<DockRevealState>,

    focused: bool,
    pointer_inside: bool,
    hover_revealed: bool,

    interaction: ShellInteractionState,

    theme_mode: ThemeMode,
    effective_theme: EffectiveTheme,
    opacity: u8,
    always_on_top: bool,
}
```

具体结构按 cohesion 调整。

---

# 23. InteractionState

避免 bool soup。

例如：

```rust
enum ShellInteractionState {
    Idle,
    Dragging,
    Resizing(ResizeDirection),
    OpacityPopup,
    OpacityEditing,
    ThemeControl,
}
```

---

# 24. Editor Session 仍独立

不要把：

```text
source selection
IME state
preview scroll
```

搬入 WindowShellState。

---

# 25. Window default

冻结：

```text
width  = 520 DIP
height = 680 DIP
```

---

# 26. Minimum window size

冻结：

```text
360 × 240 DIP
```

但 Split 必须保证基本 pane usability。

---

# 27. Split geometry

固定：

```text
50 / 50
```

---

# 28. Split divider

```text
1 DIP
```

不可拖。

---

# 29. Split minimum pane

目标：

```text
≈240 DIP
```

如果窗口宽度太小：

可以提高当前 effective minimum width。

不得创建 user-configurable split size。

---

# 30. 自定义 Window Chrome

Phase 8 正式转为：

```text
undecorated window
```

---

# 31. 优先使用 winit

当前 winit 支持：

```text
decorations false
drag_window
drag_resize_window
WindowLevel
Windows corner preference
undecorated shadow
```

优先这些。

---

# 32. 不自己重写 Windows 窗口管理

只在 winit 无法表达：

```text
whole-window opacity
stable monitor identity
work area
no-activate dock animation
display topology event
```

时使用 Win32。

---

# 33. Window Creation

正式 production window 建议：

```text
decorations = false
resizable = true
visible = false initially
```

先配置完成再显示。

---

# 34. 为什么 initially invisible

避免 startup flash：

```text
wrong theme
wrong opacity
wrong position
wrong monitor
```

---

# 35. Startup Shell 顺序

建议：

```text
core startup
→ note recovery
→ asset reconciliation
→ create hidden main window
→ resolve monitor placement
→ apply size/position
→ apply theme
→ apply opacity
→ apply topmost
→ apply shadow/corners
→ create tray
→ show window
→ focus editor
```

---

# 36. Tray 创建失败

Tray 是 v1 必需能力。

如果正常 startup 时：

```text
tray initialization fails
```

不要进入一个：

```text
Close → Hidden forever
```

的危险状态。

推荐：

```text
show explicit shell initialization error
do not enter normal app lifecycle
exit safely
```

因为此时尚未有用户新编辑内容。

---

# 37. Window title

即使 undecorated：

```text
StickyMD
```

仍设置为 OS window title。

供：

```text
taskbar
Alt+Tab
accessibility
diagnostics
```

使用。

---

# 38. Taskbar 行为

除非已有 plan 明确冻结：

> Phase 8 不因为引入 Tray 就擅自设置 `skip_taskbar=true`。

主窗口 visible 时保留当前正常 taskbar 行为。

HideToTray 后窗口隐藏，自然不显示窗口。

如果现有 plan 已明确无 taskbar：

遵循 plan。

---

# 39. Window Buttons

不绘制：

```text
minimize
maximize
```

只保留：

```text
Close → Hide to Tray
```

---

# 40. OS CloseRequested

包括：

```text
Alt+F4
taskbar Close
system CloseRequested
```

正常用户关闭必须映射：

```text
HideToTray
```

而不是退出。

---

# 41. OS shutdown 是例外

Windows session shutdown / process termination不是用户“关闭”。

它可以走：

```text
best-effort lifecycle shutdown
```

不受“只有 Tray 退出”字面限制。

不要阻止 Windows 正常关机。

---

# 42. Top Control Bar

固定高度约：

```text
34 DIP
```

---

# 43. 推荐布局

左侧：

```text
Source
Split
Preview
```

右侧：

```text
Pin
Theme
Opacity
Collapse
Close
```

---

# 44. Drag Region

Top bar 中：

```text
不是按钮
不是 popup
不是 resize handle
```

的区域都是 drag region。

---

# 45. Drag

左键按下 drag region：

```text
window.drag_window()
```

优先 winit。

---

# 46. 自定义 resize

窗口边缘使用 invisible hit zones。

建议：

```text
6 DIP
```

---

# 47. Resize Directions

支持：

```text
Left
Right
Top
Bottom
TopLeft
TopRight
BottomLeft
BottomRight
```

Floating 状态。

---

# 48. Resize 实现

优先：

```text
Window::drag_resize_window(...)
```

---

# 49. Docked Resize

Docked Expanded 时只允许不会破坏 dock anchor 的 resize。

例如：

### Left dock

主要允许：

```text
Right
Top
Bottom
TopRight
BottomRight
```

### Right dock

```text
Left
Top
Bottom
TopLeft
BottomLeft
```

### Top dock

```text
Bottom
Left
Right
BottomLeft
BottomRight
```

---

# 50. Docked edge 本身不允许 outward resize

例如 Left dock 的 Left edge。

---

# 51. Collapsed 不允许 resize

3 DIP sensor strip：

```text
no resize cursor
```

---

# 52. Resize cursor

使用 winit cursor：

```text
EwResize
NsResize
NwseResize
NeswResize
```

---

# 53. OS Aero Snap 冲突

使用 native `drag_window()` 时必须真实验证 Windows 11：

```text
drag to left
drag to right
drag to top
```

是否触发系统 Snap/Maximize。

---

# 54. StickyMD Dock authority

最终落点必须由：

```text
StickyMD DockState
```

决定。

不能让 Windows Aero Snap 偷偷成为第四种 placement。

---

# 55. 如果 Windows native Snap干扰

优先尝试：

```text
minimal window style / winit behavior correction
post-drag restore
```

不要第一反应实现完整手写 mouse-drag window manager。

---

# 56. 如果必须修改实现策略

记录：

```text
docs/report/phase-08-native-drag-snap-risk.md
```

说明：

```text
observed behavior
attempted native solution
chosen correction
```

---

# 57. Double-click title region

不得最大化。

StickyMD 没有 Maximized 产品状态。

---

# 58. Maximized state

如果 Windows 因外部行为将窗口 maximize：

Coordinator 必须恢复到合法 Floating/Docked state。

不得持久化：

```text
maximized
```

---

# 59. Fixed visual shell

使用：

```text
paper-like background
small radius
subtle border/shadow
minimal controls
```

---

# 60. 禁止

```text
Mica
Acrylic
blur
glass
dynamic wallpaper
background image
```

---

# 61. Rounded Corners

优先：

```text
winit Windows CornerPreference
```

建议：

```text
RoundSmall
```

因为更接近纸片而非大圆卡片。

---

# 62. 如果 plan 已冻结 Round

按 plan。

---

# 63. Shadow

优先：

```text
with_undecorated_shadow(true)
```

或 runtime equivalent。

---

# 64. 不自绘窗口外部 shadow bitmap

因为：

```text
window bounds
clipping
DPI
opacity
```

复杂度没有必要。

---

# 65. Collapsed Shadow

Collapsed 最终状态建议：

```text
disable undecorated shadow
```

避免 sensor strip周围残留额外可见阴影。

---

# 66. Reveal 开始前

恢复 shadow。

---

# 67. Collapsed corners

为了让 3 DIP sensor 成为完整直线：

允许 collapsed 时临时：

```text
DoNotRound
```

展开时恢复：

```text
RoundSmall
```

若实际系统切换产生明显视觉闪烁：

可以只关闭 shadow，不改 corner。

记录真实结果。

---

# 68. Controls Icon

不要引入大型 icon pack。

优先：

```text
tiny-skia programmatic vector icons
```

---

# 69. 至少需要

```text
source
split
preview
pin
theme
opacity
collapse
close
sun
monitor
moon
```

---

# 70. 不用 Emoji 当核心 icon

避免：

```text
platform emoji appearance
font fallback
color glyph
```

差异。

---

# 71. App/Tray Icon

如果还没有 final icon：

生成一个简单、原创、项目内几何图标。

例如：

```text
rounded paper
small markdown-like mark
```

---

# 72. 不下载第三方 icon

---

# 73. 可以运行时生成 RGBA Icon

利用已有：

```text
tiny-skia
```

不必增加图片资源依赖。

---

# 74. Control visibility

当：

```text
window focused
pointer inside top bar
control popup active
```

时：

```text
full visibility
```

---

# 75. Idle controls

可以降低 alpha。

例如：

```text
35–45%
```

但仍可发现。

---

# 76. Control fade

允许短：

```text
~120 ms
```

---

# 77. Fade 不使用 permanent frame loop

只在 transition 期间 redraw。

---

# 78. Theme

正式三态：

```rust
enum ThemeMode {
    Light,
    System,
    Dark,
}
```

---

# 79. Default

首次：

```text
Light
```

不是 System。

---

# 80. Effective Theme

运行时另有：

```text
Light
Dark
```

---

# 81. Theme authority

```text
ThemeMode
```

来自 ConfigState。

```text
EffectiveTheme
```

是 derived state。

---

# 82. System Theme

当：

```text
ThemeMode::System
```

时：

优先使用 winit：

```text
window.set_theme(None)
WindowEvent::ThemeChanged
window.theme()
```

---

# 83. Light/Dark explicit

可以：

```text
window.set_theme(Some(Light))
window.set_theme(Some(Dark))
```

同时内部 Preview/Source style采用对应主题。

---

# 84. ThemeChanged

只有：

```text
ThemeMode::System
```

才改变 effective theme。

---

# 85. System Theme变化不写 config

因为 config仍然只是：

```text
theme = "system"
```

---

# 86. System Theme查询失败

Windows v1 fallback：

```text
Light
```

并记录 Debug diagnostic。

---

# 87. 不先写 Registry Theme Reader

只有真实验证证明 winit ThemeChanged在目标 Windows 11不可用时：

才允许极薄 Windows fallback。

---

# 88. 如果需要 fallback

写：

```text
docs/report/phase-08-system-theme-fallback.md
```

---

# 89. Theme control UI

三态：

```text
sun | monitor | moon
```

---

# 90. 交互

点击 icon：

直接选中对应 mode。

slider indicator平滑移动。

---

# 91. Theme动画

短：

```text
~120–160 ms
```

只影响 indicator/control。

---

# 92. Theme改变

不得：

```text
change Document generation
trigger autosave note
reparse Markdown
```

---

# 93. Theme改变应

```text
repaint Source
repaint Preview
update selection colors
update controls
invalidate math color raster if required
```

---

# 94. Image cache

Theme变化不需要重解码图片。

---

# 95. Raw HTML / code / errors

必须各有 Light/Dark 合理颜色。

---

# 96. Opacity

正式：

```text
70–100
integer
default 96
```

---

# 97. Opacity 是整个 HWND alpha

必须同时影响：

```text
paper
source text
preview
math
images
controls
shadow as system applies
```

---

# 98. 不实现只透明背景

---

# 99. Opacity UI

点击 opacity button：

显示极小 popup：

```text
slider
numeric input
```

---

# 100. Slider

```text
min 70
max 100
step 1
```

---

# 101. Numeric

仅整数。

---

# 102. 输入 `<70`

commit：

```text
70
```

---

# 103. 输入 `>100`

commit：

```text
100
```

---

# 104. 非数字

不提交。

---

# 105. Slider拖动

每一步：

```text
PreviewOpacity(value)
```

立即改变 HWND alpha。

---

# 106. 但拖动过程中

不得每步写：

```text
config.toml
```

---

# 107. Config commit只在

```text
mouse release
Enter
numeric input focus loss
```

---

# 108. Popup关闭

若有合法临时值：

commit。

---

# 109. Opacity不会改变Document generation

---

# 110. Whole Window Opacity Win32

winit没有完整整窗constant-alpha abstraction时：

允许 Windows adapter。

---

# 111. `<100%`

实现：

```text
ensure WS_EX_LAYERED
SetLayeredWindowAttributes(..., alpha, LWA_ALPHA)
```

---

# 112. Alpha conversion

建议：

```text
alpha = round(percent * 255 / 100)
```

必须 clamp。

---

# 113. 不使用 color key

```text
LWA_ALPHA only
```

---

# 114. 100%

建议：

```text
remove WS_EX_LAYERED
invalidate/redraw
```

而不是永远保留 layered alpha 255。

---

# 115. 修改 extended style

必须：

```text
GetWindowLongPtrW
SetWindowLongPtrW
```

保留其它 bit。

不得重写完整 exstyle常量。

---

# 116. 不使用 UpdateLayeredWindow

现有 softbuffer pipeline继续。

---

# 117. Opacity adapter边界

只：

```text
platform/windows/window_opacity.rs
```

或等价。

---

# 118. unsafe必须 SAFETY

---

# 119. 70/85/96/100

真实测试。

---

# 120. 100% layered state

测试：

```text
WS_EX_LAYERED absent
```

---

# 121. <100%

测试：

```text
WS_EX_LAYERED present
GetLayeredWindowAttributes alpha matches
```

---

# 122. Opacity反复切换

```text
70→100→80→100
```

不得失去：

```text
softbuffer rendering
mouse input
IME
shadow
corner
```

---

# 123. Always On Top

配置：

```text
always_on_top = true/false
```

---

# 124. 优先 winit

```text
WindowLevel::AlwaysOnTop
WindowLevel::Normal
```

---

# 125. 不直接 Win32 TOPMOST

除非后面 collapsed sensor temporary topmost需要。

---

# 126. Pin button

顶部 pin：

```text
toggle configured always_on_top
```

---

# 127. Tray item

`置顶`：

同一 state。

---

# 128. Tray check 与 Pin button

必须同步。

---

# 129. 不允许两个 topmost truth sources

---

# 130. Tray

正式加入/确认成熟：

```text
tray-icon
```

当前技术基线：

```text
0.24.x
```

但实现前重新核实 exact stable/version compatibility。

---

# 131. 不自动升级其它 dependencies

只为 Tray新增必要dependency。

---

# 132. tray-icon Dependency Audit

记录：

```text
exact version
license
muda version
transitive deps
binary delta
memory delta
```

---

# 133. tray-icon 来自 tauri-apps不意味着引入 Tauri runtime

但是必须用：

```bash
cargo tree
```

证明：

```text
tauri runtime absent
```

---

# 134. Tray event处理

禁止：

```text
每16ms try_recv()
每frame poll receiver
```

---

# 135. 正确方向

使用 tray library event handler：

```text
→ EventLoopProxy / existing app user-event path
```

---

# 136. Tray只有三个 selectable items

严格：

```text
显示/隐藏
置顶
退出
```

---

# 137. 不加入

```text
设置
关于
导出
新建
打开
更新
```

---

# 138. 显示/隐藏 label

可以动态变：

```text
窗口 visible → 隐藏
窗口 hidden  → 显示
```

但它仍是同一逻辑菜单项。

---

# 139. 置顶

使用 check item。

---

# 140. 退出

唯一正常用户 Quit入口。

---

# 141. Tray separator

不是必要。

为了“只保留三个项目”：

建议完全不加入 separator。

---

# 142. Left click Tray

默认可以显示 menu。

不要额外定义：

```text
left click直接toggle
double click打开
```

除非 tray crate默认必须处理。

产品行为只依赖 menu。

---

# 143. Tray icon lifetime

从 normal shell startup：

```text
until actual process exit
```

---

# 144. HideToTray

主窗口：

```text
Close button
OS CloseRequested
Tray 隐藏
```

都进入：

```text
HideToTray intent
```

---

# 145. HideToTray 保存语义

如果 document dirty：

推荐严格流程：

```text
freeze new document mutation for hide transition
→ request immediate latest save
→ success
→ hide window
→ unfreeze runtime while hidden
```

---

# 146. 为什么先保存

用户点击关闭按钮通常预期当前内容已安全。

---

# 147. Hide save失败

不得隐藏。

保持窗口 visible。

显示现有：

```text
Save failure banner
```

---

# 148. 如果 document clean

直接隐藏。

---

# 149. Paste/asset transaction active

HideToTray无需等待所有非关键 Preview decode。

但必须避免：

```text
正在提交图片paste
→ window隐藏时状态半提交
```

复用 Phase7 paste transaction lifecycle。

---

# 150. Export进行中

HideToTray可以允许 export继续。

但 popup/dialog lifecycle必须正确。

如果 native export dialog正在显示：

不要隐藏 owner窗口直到 dialog结束。

---

# 151. HiddenToTray

调用：

```text
window.set_visible(false)
```

或现有 abstraction。

---

# 152. Hidden 不等于 quit

event loop继续。

Autosave/persistence继续。

---

# 153. Hidden cache purge

进入 HiddenToTray 后：

必须调用已有 cache purge：

```text
decoded image cache → clear
math raster cache → clear
```

---

# 154. Preview semantic tree

可以保留。

---

# 155. Framebuffer

如果实测可以安全释放/重建：

建议 Hidden时释放大型 framebuffer/surface backing resources。

但：

不要为了理论优化破坏 show可靠性。

---

# 156. 是否释放 softbuffer surface

必须 benchmark后决定。

记录：

```text
memory saved
show latency
```

---

# 157. Hidden target

总规格：

```text
PWS <=36 MiB
```

当前项目大概率远低于此。

---

# 158. Tray Show

```text
show window
restore stored placement
ensure monitor valid
expand if docked
focus
enable editor input
```

---

# 159. Tray Show永远 expanded

即使之前：

```text
DockedCollapsed
```

Tray Show应显示完整窗口。

---

# 160. Dock placement仍保留

展开在原 edge。

---

# 161. Hidden状态不持久化

重启程序：

正常 visible。

---

# 162. Collapsed状态也不持久化

配置只持久化：

```text
dock edge
monitor identity
offset
size
floating placement
```

---

# 163. Startup saved dock

程序启动：

```text
DockedExpanded
```

不是直接隐藏sensor。

---

# 164. Actual Quit

Tray：

```text
退出
```

触发：

```text
RequestQuit
```

---

# 165. Quit Flow

复用并最终冻结：

```text
freeze keyboard/IME/document mutations
→ resolve/finish in-flight paste safely
→ stop accepting new export
→ latest note save
→ safe asset reconcile/GC
→ config save
→ stop workers
→ remove Tray
→ close main window
→ exit event loop
```

---

# 166. Save失败

取消 Quit。

---

# 167. Quit失败时窗口 hidden

如果 Quit从Tray、窗口当前hidden：

```text
show window
```

显示 save error。

恢复用户控制。

---

# 168. Asset GC失败

按Phase7：

不阻止退出。

多留trash是safe。

---

# 169. Config save失败

Note 已安全时：

可以提示，但通常不阻止退出。

遵循现有 plan。

---

# 170. Dock Snap

Frozen threshold：

```text
12 DIP
```

---

# 171. Drag release target monitor

优先：

> mouse release point 所在 monitor work area。

---

# 172. 如果 release pointer无法可靠获得

fallback：

```text
window center nearest monitor
```

---

# 173. Windows pointer

允许极薄：

```text
GetCursorPos
```

仅在 drag-end placement resolution需要。

---

# 174. Work Area

Dock必须使用：

```text
monitor work area
```

不是 full monitor bounds。

---

# 175. 为什么

尊重：

```text
taskbar
reserved desktop areas
```

---

# 176. Work area Windows adapter

允许：

```text
GetMonitorInfoW
rcWork
```

---

# 177. Snap Left

如果窗口左边/释放point距 work.left：

```text
<=12 DIP
```

则：

```text
DockEdge::Left
```

---

# 178. Snap Right

同理。

---

# 179. Snap Top

同理：

```text
work.top
```

---

# 180. Corner 优先级

当同时接近：

```text
Top + Left
```

需要 deterministic。

推荐：

比较实际 DIP距离：

```text
distance最小者
```

平局：

```text
Left/Right优先于Top
```

或按 plan已有规则。

必须测试。

---

# 181. Dock Expanded Rect — Left

```text
x = work.left
y = offset position
```

---

# 182. Right

```text
x = work.right - width
```

---

# 183. Top

```text
y = work.top
```

---

# 184. Dock Offset Ratio

Left/Right：

表示：

```text
vertical position ratio
```

Top：

表示：

```text
horizontal position ratio
```

---

# 185. Ratio范围

```text
0.0–1.0
```

---

# 186. ratio计算

假设：

```text
available = work_length - window_length
```

如果：

```text
available > 0
```

则：

```text
ratio = offset / available
```

clamp。

---

# 187. available <=0

```text
ratio = 0
```

窗口clamp到work area。

---

# 188. Collapsed sensor

冻结：

```text
3 DIP
```

---

# 189. physical sensor

```text
max(1 px, round(3 * scale_factor))
```

---

# 190. Left collapsed

```text
x = work.left - window_width + sensor
```

---

# 191. Right collapsed

```text
x = work.right - sensor
```

---

# 192. Top collapsed

```text
y = work.top - window_height + sensor
```

---

# 193. 其它轴

保持 expanded dock offset。

---

# 194. 不创建一个小 capsule/tab

Collapsed状态就是原窗口移出work area，仅留sensor strip。

---

# 195. Primary Sensor Architecture

优先：

```text
single main window
```

实现 sensor。

---

# 196. 为什么

更少：

```text
HWND
state
focus bug
z-order bug
memory
```

---

# 197. 只有真实证据证明 single-window sensor不可靠

才考虑：

```text
separate 3-DIP sensor helper window
```

---

# 198. Separate Sensor fallback

必须先记录：

```text
docs/report/phase-08-sensor-window-fallback.md
```

说明：

```text
single-window failure
why helper needed
focus/z-order semantics
memory
lifecycle
```

它仍只是 platform implementation detail。

---

# 199. Sensor Pointer Enter

Collapsed窗口：

```text
CursorEntered
```

开始：

```text
100 ms reveal timer
```

---

# 200. Pointer在100ms前离开

取消。

---

# 201. 100ms到

仍pointer inside：

立即开始：

```text
140ms expand animation
```

---

# 202. Hover Reveal不能抢焦点

hard invariant。

---

# 203. Hover Reveal不能 steal foreground app

必须真实测试。

---

# 204. 展开时

```text
focused = false
hover_revealed = true
```

---

# 205. Pointer停在展开窗口内

保持展开。

---

# 206. 不应因为无focus立即触发700ms collapse

hover-reveal模式有独立规则。

---

# 207. Hover Reveal后鼠标离开

开始：

```text
500 ms collapse timer
```

---

# 208. 500ms前重新进入

取消。

---

# 209. Hover Reveal后点击窗口

获得focus。

切换：

```text
hover_revealed = false
focused = true
```

自动隐藏禁用。

---

# 210. Configured AlwaysOnTop false 时 Sensor可访问性

这是必须解决的边界。

如果 main window collapsed 后仍是 normal z-order：

一个 maximized app可能盖住 3 DIP sensor。

---

# 211. 推荐行为

Collapsed / hover-revealed期间使用：

```text
temporary sensor topmost
```

仅保证 sensor/reveal可访问。

---

# 212. 这不是用户“置顶”设置

Configured：

```text
always_on_top = false
```

仍保持 false。

Tray check也保持 false。

---

# 213. Effective Topmost

逻辑：

```text
configured_topmost
||
collapsed_sensor_accessibility
||
hover_reveal_accessibility
```

---

# 214. 点击并获得focus

如果configured topmost false：

恢复：

```text
WindowLevel::Normal
```

---

# 215. hover collapse完成

sensor需要继续 temporary topmost。

---

# 216. HiddenToTray

无temporary topmost。

---

# 217. Temporary Topmost不能 activate

---

# 218. 如果 winit WindowLevel切换无法满足 no-activate

允许 Windows adapter：

```text
SetWindowPos
HWND_TOPMOST / HWND_NOTOPMOST
SWP_NOACTIVATE
```

---

# 219. Win32 Z-order必须隔离

---

# 220. User topmost=true

始终：

```text
AlwaysOnTop
```

无temporary distinction。

---

# 221. Focus Auto-hide

冻结：

```text
focus lost
→ 700 ms
→ collapse
```

---

# 222. 但只适用于

```text
DockedExpanded
```

Floating不auto-hide。

---

# 223. Focused

任何时候：

```text
auto collapse disabled
```

---

# 224. IME composing

同样：

```text
auto collapse disabled
```

---

# 225. Drag/Resize

同样禁止auto collapse。

---

# 226. Internal popup

Theme/Opacity popup open：

禁止auto collapse。

---

# 227. Pointer重新进入

在focus-loss timer期间：

建议取消auto-collapse，避免窗口在用户鼠标位于其上时突然消失。

---

# 228. Pointer再次离开

若仍unfocused：

重新：

```text
700 ms
```

或根据状态机使用对应 reason。

保持确定性。

---

# 229. Manual Collapse

点击：

```text
collapse button
```

---

# 230. Floating状态

按钮：

```text
disabled
```

或不显示 active affordance。

---

# 231. Docked状态

manual collapse：

```text
no delay
→ start 140ms collapse
```

---

# 232. Esc

Floating：

> 不执行 edge collapse。

保留 Editor/IME existing semantics。

---

# 233. Esc in Docked

如果 app 收到可处理的 Escape：

```text
RequestCollapse
```

立即开始 collapse动画。

---

# 234. IME Esc

不得破坏 Phase3 composition correctness。

必须真实验证：

```text
Docked + Pinyin preedit + Esc
```

---

# 235. Frozen user intent

Docked状态下：

manual Esc最终必须可以收起。

但不能：

```text
commit phantom IME text
```

---

# 236. Manual Collapse from focused editor

collapse后必须确保：

```text
hidden-offscreen editor不会继续接受canonical typing
```

---

# 237. Editor Input Guard

当：

```text
DockedCollapsed
HiddenToTray
hover_revealed but not focused
```

Source editor不得接受普通 typing/IME commit。

---

# 238. On collapse

建议：

```text
disable IME participation
clear/cancel preedit safely
editor_input_enabled = false
```

---

# 239. 不需要 DocumentState变化

---

# 240. Hover expand未点击

仍：

```text
editor_input_enabled = false
```

---

# 241. 点击并focus

重新：

```text
editor_input_enabled = true
IME allowed
```

---

# 242. Tray Show

直接：

```text
enable input
focus
```

---

# 243. Animation

冻结：

```text
140 ms
```

---

# 244. Easing

建议固定：

```text
cubic ease-out
```

例如：

```text
1 - (1-t)^3
```

---

# 245. 不暴露 animation setting

---

# 246. Animation only position

不要在dock hide同时：

```text
resize
scale
fade content
```

---

# 247. Whole opacity保持用户设定

---

# 248. Animation Tick

约：

```text
16 ms
```

只在140ms animation active期间。

---

# 249. No permanent 60 FPS

hard invariant。

---

# 250. 计时与既有 scheduler统一

Phase 4–7已有：

```text
autosave deadline
preview debounce
caret blink
```

不要再创建：

```text
独立 busy loop
独立 animation thread
```

---

# 251. Main Event Loop

维护：

```text
nearest deadline
```

使用：

```text
Wait / WaitUntil
```

或已有等价 scheduler。

---

# 252. Dock timers

至少：

```text
HoverReveal
FocusLossCollapse
HoverLeaveCollapse
AnimationTick
```

---

# 253. 使用 virtual clock测试

状态机 unit tests不真的sleep。

---

# 254. Programmatic Move vs User Move

这是 hard invariant。

动画、monitor recovery、startup placement都会产生：

```text
Moved
```

事件。

---

# 255. Programmatic Moved不能被当user drag

否则会：

```text
覆盖 floating ratio
清 dock state
把 collapsed offscreen坐标写config
```

---

# 256. 必须有 move origin

例如：

```rust
enum MoveOrigin {
    User,
    DockAnimation,
    StartupRestore,
    DisplayRecovery,
    DpiReflow,
}
```

或等价。

---

# 257. Config persistence

只有 stable logical placement写 config。

---

# 258. 绝不能持久化 collapsed physical rect

hard invariant。

---

# 259. 绝不能每 animation frame写 config

---

# 260. User Drag End

只在 drag finished 后：

```text
resolve dock/floating
update WindowShellState
commit ConfigState
```

---

# 261. 如何检测 drag end

优先检查现有 winit event behavior。

---

# 262. 如果 winit Mouse Released足够可靠

使用。

---

# 263. 如果 native drag loop吞/延迟 release

允许 Windows-specific：

```text
WM_ENTERSIZEMOVE
WM_EXITSIZEMOVE
```

观察。

---

# 264. 不 subclass WndProc作为第一选择

winit已有 Windows message hook。

---

# 265. EventLoopBuilderExtWindows::with_msg_hook

如果使用：

只观察少数明确 message：

```text
WM_ENTERSIZEMOVE
WM_EXITSIZEMOVE
WM_DISPLAYCHANGE
```

必要时：

```text
WM_SETTINGCHANGE
```

---

# 266. msg hook不得消费正常message

通常返回：

```text
false
```

---

# 267. msg hook不得执行业务

只产生：

```text
platform hint
```

---

# 268. unsafe读取 MSG

必须集中在：

```text
platform/windows/message_hook.rs
```

---

# 269. Display topology

多显示器是 v1一级要求。

---

# 270. 不持久化 HMONITOR

`HMONITOR` 只是一段 runtime handle。

---

# 271. MonitorIdentity

正式定义稳定值。

推荐：

```rust
struct MonitorIdentity {
    stable_hash: ...,
}
```

---

# 272. Preferred Identity Source

使用 Windows CCD：

```text
QueryDisplayConfig
DisplayConfigGetDeviceInfo
monitor target device path
```

---

# 273. Persist

推荐：

```text
SHA-256(device path normalized)
```

使用已有 hash能力。

可以存：

```text
ccd:<32hex>
```

---

# 274. 为什么hash

config更短。

避免暴露长 PnP device path。

---

# 275. Fallback identity

如果 CCD mapping失败：

允许：

```text
GDI device name + runtime geometry
```

构造 fallback identity。

---

# 276. Fallback必须标记

例如：

```text
gdi:<hash>
```

---

# 277. 不假装 fallback同样稳定

报告。

---

# 278. MonitorTopology

runtime缓存：

```text
identity
work_area_physical
full_bounds
scale_factor
primary
```

---

# 279. CCD Query

优先：

```text
QDC_ONLY_ACTIVE_PATHS
```

不要：

```text
QDC_ALL_PATHS
```

无必要昂贵调用。

---

# 280. QueryDisplayConfig topology race

显示器插拔时：

buffer size可能变化。

必须：

```text
bounded retry
```

例如3次。

---

# 281. 不无限 retry

---

# 282. Query topology时机

只：

```text
startup
display change
resume
placement recovery
```

---

# 283. 不每 frame query

---

# 284. Work Area

仍使用当前 monitor：

```text
GetMonitorInfoW.rcWork
```

---

# 285. Monitor Scale

优先：

```text
winit MonitorHandle::scale_factor()
```

对应。

---

# 286. Geometry Unit

Config：

```text
DIP
normalized ratio
```

Windows placement：

```text
physical pixels
```

---

# 287. 所有 unit类型必须清晰

不要混：

```text
f32 x
```

不知道是DIP还是px。

---

# 288. 建议 thin newtypes

例如：

```rust
struct Dip(f32);
struct PhysicalPx(i32);
```

如果不造成过度复杂。

至少函数名标明。

---

# 289. Snap threshold

```text
12 DIP
```

转 target monitor physical。

---

# 290. Detach threshold

Docked窗口用户往内拖离：

```text
16 DIP
```

清 Docked state。

---

# 291. 用户从Docked拖动

推荐：

```text
drag starts
→ logical state becomes Floating
→ native drag
→ on release re-evaluate snap
```

---

# 292. Dock edge follow work area

Taskbar改变位置/尺寸：

下次 display/workarea reconcile重新计算。

---

# 293. Multi-monitor Floating persistence

保存：

```text
monitor identity
width_dip
height_dip
x_ratio
y_ratio
```

---

# 294. Floating x/y ratio

相对于：

```text
work_area - window_size
```

的 available position。

---

# 295. Negative coordinates

必须完整支持。

不 clamp到：

```text
x >= 0
y >= 0
```

---

# 296. Monitor left of primary

例如：

```text
[-1920, 0] → [0,0]
```

必须工作。

---

# 297. Monitor above primary

负y。

---

# 298. Saved monitor missing at startup

fallback：

```text
primary monitor
```

---

# 299. Floating recovery

使用原：

```text
x_ratio
y_ratio
```

在primary work area重建位置。

---

# 300. Docked recovery

保留：

```text
same DockEdge
same offset_ratio
```

在primary。

---

# 301. Size

保留DIP尺寸。

如果超过新work area：

clamp。

---

# 302. 窗口必须至少有可操作区域

恢复后：

```text
fully inside work area
```

expanded状态。

---

# 303. Runtime monitor disconnect

如果当前visible窗口所在monitor消失：

```text
recover to primary
```

---

# 304. Docked monitor disconnect

保留same edge。

---

# 305. 推荐 visible recovery

```text
expanded
```

即使之前collapsed。

原因：

让用户明显知道窗口被恢复。

---

# 306. HiddenToTray monitor disconnect

不必show。

更新/标记placement。

下一次Tray Show：

primary expanded。

---

# 307. Floating disconnect

move primary。

---

# 308. Cancel animation on topology change

显示器插拔期间：

```text
cancel current dock animation
resolve new stable topology
```

---

# 309. WM_DISPLAYCHANGE

如果 winit没有足够事件：

通过 message hook产生：

```text
DisplayTopologyChanged
```

---

# 310. Sleep / Resume

EventLoop resume：

重新query topology。

---

# 311. Remote Desktop

如果可能：

requery topology on resume/display change。

---

# 312. DPI Change

winit：

```text
ScaleFactorChanged
```

---

# 313. DPI Change不得改变Document generation

---

# 314. DPI Change应更新

```text
source layout/raster
preview layout/raster
math raster cache
image display raster if needed
control geometry
resize hit zones
snap threshold physical px
sensor strip px
window position
IME cursor rect
```

---

# 315. Docked DPI change

保持：

```text
logical DIP size
edge
offset_ratio
```

重建 physical rect。

---

# 316. Floating DPI change

保持逻辑尺寸与relative position。

---

# 317. Avoid double scale

手工/自动测试。

---

# 318. Mixed DPI

例如：

```text
Monitor A 100%
Monitor B 150%
```

拖过去：

窗口视觉逻辑大小不应突然变成1.5倍或2/3。

---

# 319. IME candidate

Phase3已有。

移动DPI monitor后：

candidate仍靠caret。

---

# 320. Sensor physical width

100%：

约3px。

150%：

约5px。

200%：

约6px。

使用round/max1。

---

# 321. Top sensor

高度 3 DIP。

宽度：

```text
原window width
```

---

# 322. Left/right sensor

宽度3 DIP。

高度：

```text
原window height
```

---

# 323. Window resize while docked

更新 expanded size和offset ratio。

Collapsed sensor仍按新size。

---

# 324. Window resize persistence

只在 resize end写 config。

---

# 325. Window move persistence

只在 drag end写。

---

# 326. Programmatic display recovery

写新的 recovered monitor/placement config once。

---

# 327. Config write coalescing

不得每 Moved event写disk。

---

# 328. Theme click

更新ConfigState一次。

---

# 329. Topmost click

一次。

---

# 330. Opacity drag

只commit一次。

---

# 331. ViewMode 已按Phase5

继续。

---

# 332. Config version

先检查Phase4 schema。

如果字段：

```text
theme
opacity
always_on_top
view_mode
window monitor/dock/ratio
```

已经存在：

不需要version bump。

---

# 333. 如果新增 durable field

必须遵守 config migration责任。

---

# 334. 不因为项目未release就忽略版本责任

工程宪法仍要求明确。

---

# 335. Timers

Phase8 timers必须 deterministic。

---

# 336. Timer cancellation

任何：

```text
focus gain
pointer enter
pointer leave
manual collapse
tray hide
display change
drag begin
resize begin
popup open
```

都需要明确取消/重排相关timer。

---

# 337. Stale timer token

使用：

```text
generation/token/deadline identity
```

避免旧timer后来错误触发。

---

# 338. Example

focus loss：

```text
timer #10
```

300ms后focus gain。

timer #10到点：

必须：

```text
no-op
```

---

# 339. Animation token

同理。

---

# 340. Reversing Animation

用户在collapse动画中hover/interaction：

允许：

```text
reverse from current position
```

或者：

```text
cancel and expand
```

不得jump。

---

# 341. 简化推荐

维护：

```text
current interpolated rect
```

新animation从当前rect开始。

---

# 342. 不是从旧logical endpoint开始

避免跳变。

---

# 343. Animation duration reversal

可以按剩余distance比例缩短。

或固定140ms。

固定更简单。

---

# 344. Manual Collapse during Expand

取消expand。

从当前rect collapse。

---

# 345. Tray Hide during animation

取消animation。

直接：

```text
HiddenToTray
```

不持久化intermediate rect。

---

# 346. Display change during animation

取消animation。

reconcile monitor。

---

# 347. Pointer Leave during expand

可以等expand完成后500ms。

---

# 348. Focus during expand

完成expand，进入focused。

---

# 349. Window activation

Hover reveal不activate。

---

# 350. Foreground preservation test

启动：

```text
Notepad focused
StickyMD collapsed
```

hover sensor。

检测：

```text
foreground window remains Notepad
```

直到点击StickyMD。

---

# 351. 可以使用 Windows GetForegroundWindow 测试

仅smoke工具。

---

# 352. Runtime app不需要轮询 foreground

---

# 353. Auto-hide Focus Rule

只要：

```text
focused
```

绝不自动collapse。

---

# 354. “正在打字”由 focus+editor active自然满足

IME preedit另作guard。

---

# 355. Window focus丢失但pointer仍在

建议：

```text
cancel/cap collapse while pointer inside
```

用户鼠标离开后再计时。

---

# 356. 这是为了避免窗口在指针下消失

记录实现行为。

---

# 357. Focus loss typical

pointer已在其它窗口：

直接700ms。

---

# 358. Popup

Theme/opacity popup：

保持窗口展开。

---

# 359. Popup outside click

关闭popup。

如果此时unfocused/docked：

重新评估collapse timer。

---

# 360. Opacity numeric input

它属于 shell control focus。

不要把数字键输入到Markdown。

---

# 361. Control focus routing

Phase3 Input dispatcher必须区分：

```text
Editor
PreviewSelection
ShellControl
```

---

# 362. Opacity input Backspace

只改数字。

---

# 363. Enter

commit。

---

# 364. Escape

如果 Docked：

遵守 manual collapse。

同时关闭 popup。

---

# 365. Floating Escape

关闭popup，不collapse。

---

# 366. Close button hit

不得让底层source selection改变。

---

# 367. Manual Collapse button

Docked时active。

Floating时disabled。

---

# 368. Collapse icon

可以根据edge方向变化：

```text
left arrow
right arrow
up arrow
```

但不是必要。

---

# 369. No Settings Page

Theme/Opacity/Pin都直接在纸张小控件。

---

# 370. Tray Context Interaction

Menu action进入同一 typed Intent。

---

# 371. Tray toggle topmost

必须更新：

```text
WindowShellState
ConfigState
top bar Pin
tray check item
platform level
```

---

# 372. Tray Show from hidden

必须：

```text
restore monitor
restore expanded placement
show
focus
```

---

# 373. Tray Hide from visible

走safe HideToTray。

---

# 374. Tray Visibility label sync

准确。

---

# 375. Multiple Tray events

必须幂等。

---

# 376. Show when already visible

可以：

```text
focus/bring to front
```

---

# 377. Hide when already hidden

no-op。

---

# 378. Quit double event

只执行一个 quit flow。

---

# 379. Tray Menu event handler

不能导致 reentrant mutation。

转到主 event loop处理。

---

# 380. tray-icon latest version

实现前确认 exact current stable及 lock。

---

# 381. 不为了tray升级winit

除非兼容性硬要求。

---

# 382. Dependency delta report

创建：

```text
docs/report/phase-08-dependency-delta.md
```

---

# 383. 至少记录

```text
tray-icon
muda
any Windows support crates
binary size delta
memory delta
```

---

# 384. No Tauri runtime

执行：

```bash
cargo tree | rg "tauri"
```

注意：

repo/crate组织名不等于依赖。

实际 normal dependency tree不得有Tauri runtime。

---

# 385. Windows API Audit

预计可能新增/正式使用：

```text
GetWindowLongPtrW
SetWindowLongPtrW
SetLayeredWindowAttributes
GetLayeredWindowAttributes

SetWindowPos

GetMonitorInfoW
GetCursorPos

GetDisplayConfigBufferSizes
QueryDisplayConfig
DisplayConfigGetDeviceInfo

GetForegroundWindow        # smoke/test preferred
```

以及必要message constants。

---

# 386. 不是所有都必须用

优先winit。

---

# 387. Windows APIs列表必须实际最小化

---

# 388. QueryDisplayConfig不属于core

---

# 389. Opacity不属于render

---

# 390. Monitor identity不属于Document core

---

# 391. `stickymd-core`

必须继续：

```text
unsafe = 0
Windows dependency = 0
winit dependency = 0
tray dependency = 0
```

---

# 392. `stickymd-render`

必须继续：

```text
unsafe = 0
Windows dependency = 0
tray dependency = 0
```

---

# 393. Window shell module建议

根据现有结构调整：

```text
apps/stickymd-win/src/
├─ shell/
│  ├─ mod.rs
│  ├─ state.rs
│  ├─ coordinator.rs
│  ├─ geometry.rs
│  ├─ docking.rs
│  ├─ animation.rs
│  ├─ controls.rs
│  ├─ theme.rs
│  ├─ opacity.rs
│  └─ tray.rs
│
└─ platform/windows/
   ├─ monitor.rs
   ├─ display_config.rs
   ├─ window_opacity.rs
   ├─ window_position.rs
   ├─ message_hook.rs
   └─ tray_adapter.rs
```

不要机械创建所有文件。

以 cohesion 为准。

---

# 394. `tray.rs` 职责

如果 tray-icon本身已足够平台抽象：

可以直接位于 Windows app shell adapter。

不要把它塞入 render/core。

---

# 395. plan_ref

所有正式新module：

```rust
//! plan_ref: docs/plan/09_windows_shell.md#...
```

必要时：

```text
03_system_architecture
04_runtime_state_model
10_performance_reliability
```

---

# 396. Geometry 模块应尽量 pure

核心 docking geometry可以作为：

```text
pure Rust
```

测试。

---

# 397. Geometry 不需要 Win32

输入：

```text
WorkArea
Scale
WindowDipSize
DockEdge
OffsetRatio
```

输出：

```text
PhysicalRect
```

---

# 398. 这样可以自动测试

```text
negative monitor coords
100/125/150/200%
small monitor
large window
all edges
```

---

# 399. WorkArea type

避免使用裸 tuple。

---

# 400. Geometry unit tests — Floating

至少：

```text
primary positive coords
left negative monitor
upper negative monitor
window smaller than work
window larger than work
ratio 0
ratio 0.5
ratio 1
```

---

# 401. Geometry tests — Docked

Left/Right/Top ×：

```text
100%
125%
150%
200%
```

---

# 402. Sensor tests

确认：

```text
visible dimension >=1 physical px
logical≈3DIP
```

---

# 403. Clamp tests

window完全可见expanded。

---

# 404. Offset tests

ratio roundtrip。

允许1px rounding error。

---

# 405. State Machine Unit Tests

必须不依赖真实timer sleep。

---

# 406. Minimum cases

```text
Floating → drag left → DockedExpandedLeft
Floating → drag right → DockedExpandedRight
Floating → drag top → DockedExpandedTop

Docked focus lost → 699ms expanded
Docked focus lost → 700ms collapse begins

focus lost timer → focus gain → no collapse

Collapsed hover 99ms → no expand
Collapsed hover 100ms → expand

hover reveal → pointer stay → remains
hover reveal → pointer leave 499ms → remains
hover reveal → pointer leave 500ms → collapse

manual collapse → immediate animation
Esc docked → immediate animation
Esc floating → no dock collapse
```

---

# 407. Interaction Guard tests

```text
focused → no auto collapse
IME preedit → no auto collapse
dragging → no auto collapse
resizing → no auto collapse
opacity popup → no auto collapse
theme control active → no auto collapse
```

---

# 408. Input suppression tests

```text
collapsed + keyboard text → no Document mutation
hidden + keyboard text → no Document mutation
hover-revealed-unfocused + keyboard text → no mutation
```

---

# 409. Focus click

hover reveal后click：

```text
focus
enable editor input
```

---

# 410. Temporary topmost tests

Configured false：

```text
Floating Expanded → Normal
Collapsed → EffectiveTopmost
HoverReveal → EffectiveTopmost
Focused after click → Normal
```

---

# 411. Configured true

全状态：

```text
Topmost
```

---

# 412. Config Write Tests

100 animation frames：

```text
config writes = 0
```

---

# 413. 100 opacity slider previews：

```text
config writes = 0
```

release：

```text
=1
```

---

# 414. User drag many Moved events

最终：

```text
placement config commit once/coalesced
```

---

# 415. Programmatic collapsed Moved events

不得改变logical placement。

---

# 416. ViewMode config仍正确

---

# 417. Monitor Identity Tests

使用 fake topology。

至少：

```text
saved monitor exists
saved monitor missing
same physical device path hash
fallback GDI
primary recovery
```

---

# 418. HMONITOR不进入config

测试 serializer。

---

# 419. Monitor Disconnect State Tests

### Floating visible

```text
missing monitor
→ primary floating
→ expanded
```

---

# 420. Docked visible

```text
missing monitor
→ primary same edge
→ expanded
```

---

# 421. Docked collapsed

```text
disconnect
→ primary same edge
→ expanded
```

---

# 422. Hidden

```text
disconnect
→ state recover
→ stay hidden
```

show：

```text
primary expanded
```

---

# 423. Scale Tests

Mixed scale moving.

---

# 424. DPI change should not modify

```text
Document generation
undo
dirty
saved generation
```

---

# 425. Theme Tests

```text
default Light
Light → Dark
Dark → System
System ThemeChanged Light
System ThemeChanged Dark
explicit Light ignores system ThemeChanged
explicit Dark ignores system ThemeChanged
```

---

# 426. Theme change counters

```text
Markdown parse delta = 0
```

---

# 427. Math

theme change may:

```text
raster invalidation
```

but：

```text
RaTeX parse/layout preferably 0
```

取决于 Phase6 color design。

记录真实。

---

# 428. Image decode

theme：

```text
0
```

---

# 429. Opacity tests

```text
70
85
96
100
```

---

# 430. Clamp

```text
0→70
69→70
101→100
255 string invalid/no commit
```

---

# 431. Numeric edit

```text
"7" temporary
"70" commit
```

UI不会在第一字符7时直接clamp，避免难输入。

---

# 432. Opacity commit only when结束

---

# 433. Opacity does not trigger

```text
preview parse
math raster
image decode
Document generation
```

OS alpha即可。

---

# 434. Always-On-Top Tests

toggle：

```text
Normal ↔ AlwaysOnTop
```

config+tray+button同步。

---

# 435. Tray unit tests

将 tray event转 typed intent。

---

# 436. Tray no polling test

静态/architecture review确认。

---

# 437. Tray integration smoke

真实 Windows：

```text
tray icon created
3 items
```

---

# 438. 如果无法视觉验证

菜单项目视觉：

```text
NOT TESTED
```

但内部 menu construction可自动验证3 items。

---

# 439. HideToTray tests

Clean：

立即hide。

Dirty：

```text
save success → hide
save fail → stay visible
```

---

# 440. Tray Show tests

visible false→true。

---

# 441. Quit tests

```text
clean
dirty save success
dirty save failure
asset GC failure
config save failure
hidden save failure
```

---

# 442. CloseRequested test

```text
no process exit
```

---

# 443. Second Instance interaction

Phase4 second instance：

```text
wake first
```

现在 first如果：

```text
HiddenToTray
```

必须：

```text
ShowFromTray
```

---

# 444. Second instance wake if collapsed

必须：

```text
expand
focus
```

---

# 445. Second instance wake if visible

focus/attention。

---

# 446. Same-dir single instance不得回归

---

# 447. Dock while Source editing

拖 edge。

window保持Document。

---

# 448. Dock while Preview

工作。

---

# 449. Dock while Split

工作。

---

# 450. Dock resize and Split

不得破坏 fixed 50/50。

---

# 451. Auto-hide and Preview worker

collapse不取消重要Preview state。

---

# 452. Auto-hide不触发parse

---

# 453. Hover reveal不触发parse

---

# 454. Window moving不触发parse

除非width变化。

---

# 455. Collapse/expand只position

width不变。

所以：

```text
Preview layout delta = 0
```

---

# 456. Resize才需要 layout

仍不parse Markdown。

---

# 457. Opacity Slider interaction and autosave

不得误触Document edit。

---

# 458. Theme controls and autosave

只保存 config。

---

# 459. Close/Hiding and note save

必须协调。

---

# 460. Performance Baseline

Release standalone portable。

---

# 461. 测试状态

至少：

```text
Source expanded
Preview expanded
Split expanded
Docked expanded
Docked collapsed
HiddenToTray
Opacity 70
Opacity 96
Opacity 100
AlwaysOnTop
```

---

# 462. Memory hard gates

继续：

```text
Source typical <=40 MiB
Preview typical <=52 MiB
Split typical <=64 MiB
Hidden <=36 MiB
```

---

# 463. Phase7 已约26MiB Split saturated

如果 Phase8大幅上涨：

分析 Tray/window shell dependency。

---

# 464. Tray memory delta

专门测：

```text
Phase7 baseline
Phase8 with tray
```

---

# 465. Dependency review trigger

若 Tray/Shell导致：

```text
+5 MiB stable Private Working Set
```

必须分析。

---

# 466. Binary size

记录：

```text
Phase7 exe
Phase8 exe
delta
```

---

# 467. tray-icon binary review

如果：

```text
+2 MiB以上
```

分析 dependency tree。

不是自动失败，但需说明。

---

# 468. Startup performance

新增 Tray / Monitor Query后重新测。

---

# 469. Cold startup target

```text
p95 <=300ms
```

---

# 470. Warm

```text
p95 <=180ms
```

理想更低。

---

# 471. DisplayConfig startup

不得成为明显慢点。

记录。

---

# 472. Idle CPU

全部稳定状态：

```text
<0.1%
```

---

# 473. Hidden idle

理想：

```text
≈0
```

---

# 474. Tray必须事件驱动

这是 idle CPU gate。

---

# 475. Collapsed sensor idle

不轮询鼠标。

依靠：

```text
window mouse events
```

---

# 476. 禁止：

```text
GetCursorPos polling loop
```

---

# 477. GetCursorPos只允许

例如：

```text
drag-end target monitor resolution
```

一次性。

---

# 478. Monitor change不poll

使用：

```text
WM_DISPLAYCHANGE
resume
```

---

# 479. Animation CPU

140ms期间短时CPU允许。

---

# 480. Animation结束

必须回到 event wait。

---

# 481. Timer leak test

1000 collapse/reveal cycles。

最终：

```text
no active animation timer
no timer list growth
```

---

# 482. Position message flood

100 animations：

不得导致config写 flood。

---

# 483. Shell allocation

Animation tick不得每帧：

```text
clone AppState
clone Document
rebuild Preview
```

---

# 484. Framebuffer

只是window move：

无需repaint整个content，除非 OS要求。

---

# 485. Control fade可以redraw topbar。

---

# 486. Hidden cache memory

测：

```text
Preview math/images
→ HideToTray
→ wait 5 sec
```

cache purge结果。

---

# 487. Show latency

从Tray Show到可交互：

目标：

```text
<100ms warm
```

cache重建可以lazy。

---

# 488. Hover reveal latency

真实：

```text
100ms delay + 140ms animation
```

完成≈240ms。

---

# 489. Manual collapse

无delay：

```text
≈140ms
```

---

# 490. Focus loss

```text
700ms + 140ms
```

---

# 491. Timings不需要硬实时

自动state timer应精确。

OS scheduling允许少量偏差。

---

# 492. Windows Manual Visual Matrix

必须建立。

---

# 493. SHELL-VIS-001 Paper

检查：

```text
no native titlebar
rounded
subtle shadow
controls
```

---

# 494. SHELL-VIS-002 Light

---

# 495. SHELL-VIS-003 Dark

---

# 496. SHELL-VIS-004 System Light

---

# 497. SHELL-VIS-005 System Dark

---

# 498. SHELL-VIS-006 Opacity 70

---

# 499. SHELL-VIS-007 Opacity 85

---

# 500. SHELL-VIS-008 Opacity 96

---

# 501. SHELL-VIS-009 Opacity 100

---

# 502. SHELL-VIS-010 Source

---

# 503. SHELL-VIS-011 Preview

---

# 504. SHELL-VIS-012 Split

---

# 505. SHELL-VIS-013 Math

---

# 506. SHELL-VIS-014 Images

---

# 507. SHELL-VIS-015 Error Banner

---

# 508. SHELL-VIS-016 Conflict Banner

---

# 509. SHELL-VIS-017 Recovery UI

---

# 510. SHELL-VIS-018 Tray

---

# 511. SHELL-VIS-019 Dock sensor

---

# 512. SHELL-VIS-020 hover reveal

---

# 513. 无视觉能力

必须：

```text
NOT TESTED
```

---

# 514. Dock Manual Matrix

至少：

```text
Left
Right
Top
```

每edge：

```text
snap
collapse
sensor
hover
focus
manual collapse
Esc
detach
resize
restart restore
```

---

# 515. Left/Right/Top不只测试一个

---

# 516. Auto-hide Manual Matrix

### Focused

等待>2sec：

```text
must remain expanded
```

---

# 517. Focus Lost

约700ms后开始collapse。

---

# 518. Re-focus 500ms

collapse取消。

---

# 519. Sensor Hover <100ms

不展开。

---

# 520. Sensor Hover >100ms

展开。

---

# 521. Hover reveal pointer stay

保持。

---

# 522. pointer leave

500ms后collapse。

---

# 523. hover reveal click

focus。

之后不collapse。

---

# 524. Manual button

立即。

---

# 525. Esc

Docked immediate。

---

# 526. Hidden typing safety

手动collapse后：

键盘输入不得改变note。

---

# 527. Tray Manual Matrix

```text
Close button → hide
Alt+F4 → hide
Tray Show → visible/focused
Tray Hide → hidden
Tray Topmost → sync
Tray Quit → process exit
```

---

# 528. Close不退出

Task Manager process仍存在。

---

# 529. Quit必须退出

---

# 530. Multi-Monitor Matrix

最低：

```text
single monitor
dual same DPI
dual mixed DPI
```

---

# 531. 位置：

```text
second right
second left
second above
```

如果硬件/环境可配置。

---

# 532. DPI：

```text
100→150
150→100
125
200
```

---

# 533. Dock on secondary

必须在secondary work area。

---

# 534. Restart

仍secondary same edge。

---

# 535. Disconnect

窗口恢复primary。

---

# 536. Reconnect

不要求自动跳回旧monitor。

当前runtime保持primary。

---

# 537. 为什么不跳回

避免用户工作时窗口突然移动。

---

# 538. 下一次启动

如果 config已更新primary：

继续primary。

---

# 539. Hidden monitor disconnect

show后primary。

---

# 540. Taskbar Work Area

如果有能力：

测试 taskbar：

```text
bottom
top
left
right
```

至少自动 geometry模拟全部。

---

# 541. Top taskbar

Top dock应对齐：

```text
work.top
```

而不是physical monitor top。

---

# 542. Negative coordinates

自动和真实尽量测试。

---

# 543. Sleep/Resume

如果可：

```text
sleep
resume
```

保持窗口可见/placement合法。

---

# 544. RDP

如果可：

Remote Desktop reconnect。

否则：

```text
NOT TESTED
```

---

# 545. IME Final Shell Matrix

这是关闭Phase3遗留条件的机会。

---

# 546. Microsoft Pinyin

真实测试：

```text
Floating
Left dock expanded
Hover-revealed then click
Opacity 70
Opacity 96
Mixed DPI if possible
Split mode
```

---

# 547. WeChat Input Method

同样。

---

# 548. 必须检查

```text
candidate position
composition
commit
undo
focus
collapse guard
```

---

# 549. IME composing while focus

不得auto collapse。

---

# 550. Manual collapse while composing

不得phantom commit。

---

# 551. 如果无法安装/访问 WeChat IME

继续：

```text
NOT TESTED
```

---

# 552. 不能用synthetic IME冒充

---

# 553. Math Visual Final Matrix

Phase6遗留：

至少：

```text
baseline
fraction
sqrt
matrix
cases
inline mixed CJK
display center
malformed
100/125/150/200 DPI
Light/Dark
opacity70
```

---

# 554. Image Visual Final Matrix

Phase7遗留。

---

# 555. Clipboard Real Matrix

也可以在真实shell完成。

---

# 556. Export dialog

真实 native dialog。

---

# 557. 若Phase8不涉及实现变更

仍可关闭人工验收。

---

# 558. Acceptance Cases

Phase8重点正式覆盖：

```text
AC-019 Left Dock
AC-020 Right Dock
AC-021 Top Dock
AC-022 Input Focus Guard
AC-023 Tray Lifecycle
AC-024 Opacity
AC-025 Theme
AC-028 Monitor Disconnect
AC-029 Mixed DPI
```

---

# 559. 还要回归

```text
AC-026 Same Directory Single Instance
AC-027 Different Directory Multi Instance
```

尤其：

```text
second instance wakes hidden/collapsed first
```

---

# 560. Always On Top acceptance

如果现有AC没有编号：

Phase8 acceptance matrix建立：

```text
PH8-TOPMOST
```

或在现有体系分配ID。

不要随意重编号已有AC。

---

# 561. Real Acceptance 不得混淆

表中用：

```text
AUTOMATED PASS
MANUAL PASS
NOT TESTED
FAIL
```

---

# 562. 不使用单一 PASS掩盖来源

---

# 563. Automated Dock Harness

建立 pure geometry/state smoke。

---

# 564. `tools/smoke/phase-08.ps1`

遵循现有工具结构。

---

# 565. 建议支持

```powershell
tools/smoke/phase-08.ps1
tools/smoke/phase-08.ps1 -Performance
tools/smoke/phase-08.ps1 -Runtime
tools/smoke/phase-08.ps1 -StateMachine
```

按已有参数习惯调整。

---

# 566. Runtime Smoke

可以创建 standalone portable copy。

---

# 567. Opacity runtime readback

通过 Windows API验证。

---

# 568. Window Level runtime

可以验证：

```text
effective topmost
```

但不替代视觉。

---

# 569. Hidden runtime

检查 process仍运行。

---

# 570. Tray creation runtime

检查 Tray object成功。

---

# 571. Foreground no-steal

使用：

```text
GetForegroundWindow
```

测试 hover reveal。

---

# 572. DisplayConfig smoke

列 monitor：

```text
identity
work area
scale
primary
```

不要在最终日志输出完整device path。

---

# 573. Hash only

---

# 574. Shell Performance Report

测：

```text
cold startup
warm startup
tray initialization
display topology query
show from tray
hide to tray
animation CPU
```

---

# 575. Startup repeat

至少：

```text
5
```

最好：

```text
10–20
```

---

# 576. Memory repeat

5次 median/max。

---

# 577. Hidden Memory

必须测。

---

# 578. Collapsed Memory

必须测。

---

# 579. No leak cycles

执行：

```text
1000 expand/collapse
100 show/hide tray
100 theme changes
100 opacity commits
100 topmost toggles
```

---

# 580. 返回idle

Private Bytes不得线性增长。

---

# 581. Tray lifecycle leak

重复 show/hide不会创建多个TrayIcon。

---

# 582. Monitor topology cache

display event不会无限增长identity列表。

---

# 583. Timer storage

bounded。

---

# 584. System theme changes

不增加旧style/cache history。

---

# 585. Opacity style bits

反复70/100不会累加其它exstyle错误。

---

# 586. Window handle/resource

检查：

```text
USER objects
GDI objects
handles
```

如果 smoke工具已有资源计数。

---

# 587. 不强求复杂 profiler

至少观察无明显线性增长。

---

# 588. Source typing latency

Phase8 shell加入后重新回归。

尤其：

```text
opacity96
topmost
docked
```

---

# 589. 输入目标仍

```text
20KiB <=16ms p95
100KiB <=25ms
1MiB <=50ms
```

---

# 590. Dock animation不能影响Document input

Focused时本来不auto animation。

Hover reveal不接受editor input。

所以冲突应少。

---

# 591. Theme change瞬间

短时间重绘可以。

---

# 592. Theme change不freeze几百ms

目标：

```text
<50ms UI-blocking
```

---

# 593. Opacity slider

应即时。

---

# 594. Config writes

监测。

---

# 595. Dependency scan

最终：

```bash
cargo tree | rg \
"tauri|wry|webview|cef|chromium|tokio|async-std|wgpu|reqwest|hyper"
```

不得新增。

---

# 596. tray-icon正常命中“tauri-apps”仓库名不算依赖

检查实际 crate名。

---

# 597. Network

Phase8不需要。

---

# 598. Database

不需要。

---

# 599. Windows features

新增 `windows` features必须精确最小化。

---

# 600. 预计 namespaces

根据实际：

```text
Win32_Foundation
Win32_UI_WindowsAndMessaging
Win32_Graphics_Gdi
Win32_Devices_Display
```

或 CCD实际 namespace。

不要全开。

---

# 601. Unsafe

Windows adapter可有。

---

# 602. core unsafe

必须：

```text
0
```

---

# 603. render unsafe

必须：

```text
0
```

---

# 604. Shell unsafe

只围绕：

```text
HWND style
display config
message hook
```

必要 API。

---

# 605. `with_msg_hook`

若使用：

记录 why。

---

# 606. Message Hook Safety

callback收到：

```text
*const c_void
```

转换 MSG指针前：

明确 winit contract。

---

# 607. callback内

不：

```text
free
retain raw pointer
block
call business code
```

---

# 608. 只读取message id等必要字段

---

# 609. 不consume无关消息

---

# 610. Monitor API Error

Query失败：

不要：

```text
panic
```

---

# 611. Fallback

至少保留 winit monitor info，使窗口仍可恢复到nearest/primary。

---

# 612. Stable identity failure

运行可继续。

但：

```text
cross-restart secondary monitor persistence
```

可能 degraded。

报告：

```text
CONDITIONAL
```

---

# 613. Opacity failure

如果 Windows alpha API失败：

不要 silently claim opacity changed。

恢复旧ConfigState/old value。

显示小错误。

---

# 614. Topmost apply failure

winit API无Result。

实际状态尽力。

---

# 615. Tray failure runtime

如果 tray object unexpectedly invalid：

强制 main window visible。

不得允许用户hide后找不回来。

---

# 616. HideToTray在 tray unavailable

拒绝 hide。

---

# 617. Theme error

fallback Light。

---

# 618. Dock placement API failure

保持当前 visible rect。

不更新 config为虚假state。

---

# 619. State commit order

Platform move成功后：

commit stable placement。

不要先写config再发现move失败。

---

# 620. Programmatic animation move individual failure

取消animation。

reconcile actual HWND rect。

保持visible。

---

# 621. No offscreen fail

任何 error路径最后：

至少让窗口回到：

```text
primary monitor
expanded
```

如果可以。

---

# 622. Shell Failure principle

宁可：

```text
显示普通可见窗口
```

也不要：

```text
不可访问的3px/屏外window
```

---

# 623. Config corruption现有逻辑继续

不允许shell bug把 note影响。

---

# 624. Window config invalid

例如：

```text
NaN ratio
negative size
unknown edge
```

必须 default/clamp。

---

# 625. TOML不产生NaN通常，但仍防御。

---

# 626. opacity invalid config

clamp70–100。

---

# 627. unknown theme

Config parser按Phase4 corruption策略。

---

# 628. Unknown monitor

primary fallback。

---

# 629. Dock offset invalid

clamp0–1。

---

# 630. Window size invalid

default or minimum clamp。

---

# 631. Configuration migration tests

Phase4旧config加载。

---

# 632. 不要求用户删除config

---

# 633. Phase8 Task

创建：

```text
docs/tasks/phase-08-windows-desktop-shell.md
```

---

# 634. Task 至少包含

```text
Status
Prerequisites
Inherited Conditions
Scope
Out of Scope

Shell Authority
Window State
Window Creation
Custom Chrome
Tray
Theme
Opacity
Always-on-top

Dock Geometry
Auto-hide State Machine
Animation
Monitor Identity
Display Topology
DPI

Config Persistence
Lifecycle
Failure Paths

Performance
Manual Verification
Risks
Result
```

---

# 635. 开始

```text
Status: In Progress
```

---

# 636. 如果实现完成但人工验收没完成

写：

```text
Status: Implementation Complete — manual verification incomplete
```

---

# 637. 全部完成人工验证才

```text
Status: Completed — awaiting USER review
```

---

# 638. Phase8 Report

创建：

```text
docs/report/phase-08-windows-desktop-shell.md
```

---

# 639. Executive Result

必须：

```text
Undecorated Paper Window:
PASS / CONDITIONAL / FAIL

Custom Drag/Resize:
PASS / CONDITIONAL / FAIL

Rounded Corners / Shadow:
PASS / CONDITIONAL / FAIL / NOT TESTED

Always-on-top:
PASS / CONDITIONAL / FAIL

Theme:
PASS / CONDITIONAL / FAIL

Whole-window Opacity:
PASS / CONDITIONAL / FAIL

Tray:
PASS / CONDITIONAL / FAIL

Close → Hide:
PASS / CONDITIONAL / FAIL

Tray Quit:
PASS / CONDITIONAL / FAIL

Left Dock:
PASS / CONDITIONAL / FAIL

Right Dock:
PASS / CONDITIONAL / FAIL

Top Dock:
PASS / CONDITIONAL / FAIL

3-DIP Sensor:
PASS / CONDITIONAL / FAIL

Hover Reveal:
PASS / CONDITIONAL / FAIL

Focus Auto-hide:
PASS / CONDITIONAL / FAIL

Multi-monitor:
PASS / CONDITIONAL / FAIL / NOT TESTED

Mixed DPI:
PASS / CONDITIONAL / FAIL / NOT TESTED

Monitor Disconnect:
PASS / CONDITIONAL / FAIL / NOT TESTED

Performance:
PASS / CONDITIONAL / FAIL

Memory:
PASS / CONDITIONAL / FAIL

Idle CPU:
PASS / CONDITIONAL / FAIL

Manual Visual:
PASS / CONDITIONAL / FAIL / NOT TESTED

Final IME:
PASS / CONDITIONAL / FAIL / NOT TESTED
```

---

# 640. Shell Architecture Map

表：

| Layer | Implementation |
|---|---|
| Interaction Shell | |
| Instruction Interface | |
| Coordinator | |
| Platform Window | |
| Tray Adapter | |
| Monitor Adapter | |
| Durable Config | |

---

# 641. Window State Evidence

列：

```text
Floating
Docked Left
Docked Right
Docked Top
Expanded
Collapsed
HiddenToTray
Animating
```

实际实现。

---

# 642. Geometry Evidence

记录：

```text
snap threshold
sensor DIP
work area
offset ratio
recovery
```

---

# 643. Timer Evidence

```text
hover = 100ms
focus lost = 700ms
hover leave = 500ms
animation = 140ms
```

---

# 644. No-steal Evidence

如果真实测试：

```text
foreground before
foreground during hover reveal
```

---

# 645. Tray Evidence

```text
crate
version
menu item count
event delivery strategy
polling? no
```

---

# 646. Theme Evidence

```text
Light
System
Dark
runtime system changes
```

---

# 647. Opacity Evidence

表：

| Setting | Alpha | Layered Style | Result |
|---:|---:|---|---|
| 70 | | yes | |
| 85 | | yes | |
| 96 | | yes | |
| 100 | 255 | no | |

---

# 648. Topmost Evidence

```text
button
tray
config
temporary sensor topmost
```

必须区分 configured/effective。

---

# 649. Monitor Identity Evidence

说明：

```text
preferred CCD source
hashed form
fallback
```

---

# 650. Work Area Evidence

至少列 monitor：

```text
full bounds
work bounds
scale
```

不输出完整device path。

---

# 651. Disconnect Evidence

真实/模拟区分。

---

# 652. DPI Evidence

自动与真实区分。

---

# 653. Config Evidence

说明：

```text
no animation frame writes
opacity commit rule
window placement write rule
monitor persistence
```

---

# 654. Memory

至少：

| State | PWS median | PWS max | Private Bytes |
|---|---:|---:|---:|
| Source | | | |
| Preview | | | |
| Split saturated | | | |
| Docked Expanded | | | |
| Docked Collapsed | | | |
| Hidden to Tray | | | |
| Opacity 70 | | | |
| Opacity 100 | | | |

---

# 655. Phase7 baseline vs Phase8

明确delta。

---

# 656. Startup Table

```text
cold p50/p95/max
warm p50/p95/max
```

---

# 657. Tray init cost

单独。

---

# 658. Display topology init cost

单独。

---

# 659. Idle CPU

所有重要状态。

---

# 660. Animation performance

```text
duration
frame count
peak CPU
```

---

# 661. Binary size

```text
Phase7
Phase8
delta
```

---

# 662. Resource Leak

记录：

```text
1000 dock cycles
100 tray cycles
100 theme
100 opacity
```

---

# 663. Manual Acceptance Table

不得只写一段话。

每项：

```text
AUTOMATED PASS
MANUAL PASS
NOT TESTED
FAIL
```

---

# 664. Inherited Manual Conditions

逐项重新列。

不能遗漏。

---

# 665. Dependencies Added

表：

```text
crate
version
license
purpose
normal/dev
binary implication
runtime implication
```

---

# 666. Windows APIs Added

表：

```text
API
why winit insufficient
unsafe?
adapter
```

---

# 667. Unsafe Report

```text
core = 0
render = 0
windows shell = ...
```

---

# 668. Architecture Authority

报告必须回答：

```text
Who owns Markdown text?
Who owns logical window placement?
Who owns durable window config?
Who owns actual HWND geometry?
Who resolves external display topology?
Can HWND Moved event directly rewrite config?
Can animation physical rect become durable placement?
Can tray directly mutate Document?
Can opacity change Document generation?
```

---

# 669. 正确核心

```text
Markdown → DocumentState
logical shell → WindowShellState
durable shell → ConfigState/config.toml
physical HWND → platform fact
topology → Windows adapter external fact
```

---

# 670. Architecture Drift

如果没有：

```text
None.
```

---

# 671. Risk Reports

以下任一需要独立 report：

### R1

native drag与Windows Snap无法稳定共存。

### R2

single-window 3DIP sensor无法可靠接收hover。

### R3

whole-window alpha与softbuffer/IME发生结构冲突。

### R4

stable monitor identity无法可靠取得。

### R5

monitor hot-unplug可能把window留屏外。

### R6

tray dependency造成异常大资源成本。

### R7

undecorated window无法获得可靠shadow/resize。

---

# 672. 不遇到风险不要虚构report

---

# 673. Review Subagents

如果支持，最多3个。

### Reviewer 1

```text
Dock geometry + state machine + timer races + config persistence
```

### Reviewer 2

```text
Windows API + monitor/DPI + tray + opacity + focus/z-order
```

### Reviewer 3

```text
architecture authority + performance + manual acceptance coverage
```

---

# 674. Main Agent必须亲自review

不能把判断交给subagent。

---

# 675. Final Self Review

逐项回答：

1. 是否出现第二Document authority？
2. WindowShellState是否唯一逻辑placement source？
3. Collapsed rect是否可能写入config？
4. Animation frame是否写config？
5. tray是否轮询？
6. hover sensor是否轮询GetCursorPos？
7. hover reveal是否抢foreground？
8. focused是否可能auto collapse？
9. IME composing是否可能auto collapse？
10. manual collapse后hidden editor是否仍接受typing？
11. Hover-revealed未focus是否可输入？
12. Close是否错误退出？
13. Tray Quit是否安全保存？
14. tray失败是否可能导致window永久不可达？
15. AlwaysOnTop configured与temporary sensor topmost是否分离？
16. opacity100是否仍无必要保持layered？
17. opacity变化是否重建Markdown？
18. theme变化是否重parseMarkdown？
19. System Theme是否写config flood？
20. monitor identity是否错误使用HMONITOR？
21. Dock是否使用work area？
22. negative coordinates是否完整支持？
23. monitor disconnect是否recover primary？
24. mixed DPI是否使用DIP→physical正确？
25. sensor是否真的3DIP？
26. bottom dock是否意外存在？
27. Windows Snap是否污染PlacementMode？
28. resize是否能产生非法offscreen window？
29. Source/Preview/Split是否回归？
30. Autosave/Conflict/Export/Assets是否回归？
31. core/render unsafe是否仍0？
32. 是否引入Mica/Acrylic？
33. 是否加入settings page？
34. 是否实现了Phase9内容？

---

# 676. Automated Verification

至少运行：

```bash
cargo fmt --check

cargo clippy \
  --workspace \
  --all-targets \
  -- -D warnings

cargo test \
  --workspace \
  --locked

cargo build \
  --workspace \
  --release \
  --locked

cargo test \
  -p stickymd-core \
  --release \
  --locked

cargo test \
  -p stickymd-render \
  --release \
  --locked

cargo test \
  -p stickymd-win \
  --release \
  --locked

cargo deny check

git diff --check
```

---

# 677. Full Smoke

```powershell
tools/smoke/all.ps1 -Ci
```

必须仍PASS。

---

# 678. Phase8 Smoke

```powershell
tools/smoke/phase-08.ps1
tools/smoke/phase-08.ps1 -StateMachine
tools/smoke/phase-08.ps1 -Performance
tools/smoke/phase-08.ps1 -Runtime
```

按现有CLI规范调整。

---

# 679. Forbidden Dependency Scan

```bash
cargo tree | rg \
"tauri|wry|webview|cef|chromium|tokio|async-std|wgpu|reqwest|hyper|rusqlite"
```

不得有新增禁止runtime。

---

# 680. Tray dependency检查

允许：

```text
tray-icon
muda
```

但不能顺带出现：

```text
tauri
```

runtime。

---

# 681. Unsafe Scan

```bash
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
rg "\bunsafe\b" apps/stickymd-win/src
```

逐项review。

---

# 682. Direct Windows Call Audit

所有 Win32调用必须位于：

```text
platform/windows/
```

或现有批准的 Windows adapter。

---

# 683. UI module不得直接

```text
SetWindowPos
QueryDisplayConfig
SetLayeredWindowAttributes
```

---

# 684. Config write audit

搜索：

```text
config save/write
```

确保animation/move event没有直接写。

---

# 685. Timer audit

确保无：

```text
sleep loop
busy loop
poll loop
```

---

# 686. Runtime Manual Smoke — Basic

在独立 portable目录运行 Release EXE。

完成：

1. Source输入。
2. Preview。
3. Split。
4. Math。
5. Images。
6. Export。
7. Autosave。
8. External clean reload。
9. Conflict。
10. 置顶。
11. Theme。
12. Opacity。
13. Left dock。
14. Right dock。
15. Top dock。
16. sensor hover。
17. Close→Tray。
18. Show。
19. Quit。

---

# 687. Runtime Manual Smoke — Persistence

Shell行为不得破坏：

```text
note.md
config.toml
images/
.trash/
```

---

# 688. Runtime Manual Smoke — Restart

设置：

```text
Dark
opacity85
topmost true
Right dock
specific size
```

退出。

重启。

确认全部恢复。

---

# 689. Collapsed state不恢复

重启应：

```text
Right Dock Expanded
```

---

# 690. Hidden state不恢复

重启 visible。

---

# 691. Runtime Manual Smoke — Second Instance

主实例：

```text
hidden to tray
```

启动第二实例。

Expected：

```text
first show expanded + focus
second exit
```

---

# 692. Docked collapsed second instance

同理。

---

# 693. Real visual not available

不要写：

```text
PASS
```

写：

```text
NOT TESTED
```

---

# 694. README

Phase8后更新真实状态：

例如：

> StickyMD now implements the intended Windows desktop shell, native Markdown/math/image preview, portable persistence, tray lifecycle and edge docking. Release validation is still in progress.

---

# 695. 不宣称

```text
v1 stable
production ready
fully tested
```

除非后续 release phase完成。

---

# 696. Plan Updates

主要：

```text
docs/plan/09_windows_shell.md
```

补充实际验证：

```text
state model
geometry
monitor identity
opacity
tray
timers
no-activate behavior
```

---

# 697. Runtime State Plan

必要时更新：

```text
04_runtime_state_model
```

只增加真实Shell状态合同。

---

# 698. Performance plan

实际benchmark写：

```text
report
```

Plan只保留contract/targets。

---

# 699. Features

更新真实用户桌面行为。

---

# 700. Acceptance

建立：

```text
docs/acceptance-cases/phase-08.md
```

---

# 701. Coverage Matrix

映射：

```text
AC-019
AC-020
AC-021
AC-022
AC-023
AC-024
AC-025
AC-028
AC-029
```

和相关回归。

---

# 702. Overview Architecture

加入：

```text
Tray
WindowShellCoordinator
DockState
MonitorTopology
Windows Adapter
```

---

# 703. Dependency Report

完成。

---

# 704. Windows API Baseline

更新：

```text
docs/report/phase-08-windows-api-delta.md
```

---

# 705. Phase8 task完成状态

根据真实人工结果诚实。

---

# 706. Git commit建议

如果初始clean，可按cohesion：

```text
feat(shell): establish final Windows paper window

feat(shell): add tray theme opacity and topmost controls

feat(shell): implement edge docking and auto-hide state machine

feat(shell): add stable multi-monitor placement and DPI recovery

test(shell): verify docking geometry and lifecycle invariants

docs: record phase 8 Windows shell results
```

无需机械分这么多。

---

# 707. 不Push

```text
push = no
```

除非 USER明确要求。

---

# 708. Phase 8 Definition of Done

只有以下实现条件满足才算 implementation complete：

- [ ] USER批准Phase8。
- [ ] Phase7 inherited conditions完整记录。
- [ ] main window正式undecorated。
- [ ] paper shell完成。
- [ ] fixed rounded corner完成。
- [ ] fixed shadow完成。
- [ ] custom drag完成。
- [ ] custom resize完成。
- [ ] OS native Snap不会污染logical placement。
- [ ] Source/Split/Preview controls整合。
- [ ] no settings page。
- [ ] Always-on-top button。
- [ ] Always-on-top tray sync。
- [ ] Theme三态。
- [ ] default Light。
- [ ] System运行时响应theme change。
- [ ] Theme change不改Document generation。
- [ ] Theme change不reparseMarkdown。
- [ ] Opacity 70–100。
- [ ] whole-window opacity。
- [ ] slider实时preview。
- [ ] numeric integer input。
- [ ] clamp70–100。
- [ ] opacity只在release/Enter/focus loss写config。
- [ ] opacity100移除unnecessary layered style。
- [ ] opacity变化不改Document。
- [ ] Tray创建。
- [ ] Tray只有显示/隐藏、置顶、退出。
- [ ] Tray事件不poll。
- [ ] Close button→HideToTray。
- [ ] Alt+F4→HideToTray。
- [ ] Hide dirty先安全保存。
- [ ] Hide save失败保持visible。
- [ ] Hidden cache purge。
- [ ] Tray Show恢复expanded。
- [ ] Tray Quit唯一正常user exit。
- [ ] Tray Quit保存/GC/config正确。
- [ ] same-dir wake hidden first instance。
- [ ] Left dock。
- [ ] Right dock。
- [ ] Top dock。
- [ ] no Bottom dock。
- [ ] snap threshold 12DIP。
- [ ] detach threshold 16DIP。
- [ ] 3DIP sensor。
- [ ] top sensor保持window width。
- [ ] left/right sensor保持window height。
- [ ] primary architecture使用single window sensor。
- [ ] sensor hover 100ms。
- [ ] hover reveal不抢focus。
- [ ] hover reveal不抢foreground。
- [ ] hover leave 500ms。
- [ ] focus loss 700ms。
- [ ] focused永不auto collapse。
- [ ] IME composing永不auto collapse。
- [ ] dragging/resizing不auto collapse。
- [ ] popup不auto collapse。
- [ ] manual collapse无delay。
- [ ] Docked Esc collapse。
- [ ] Floating Esc不执行edge collapse。
- [ ] collapse/reveal animation 140ms。
- [ ] animation不permanent 60FPS。
- [ ] animation使用existing scheduler。
- [ ] timer stale token安全。
- [ ] programmatic move不会写placement config。
- [ ] collapsed physical rect不会持久化。
- [ ] temporary sensor topmost与configured topmost分离。
- [ ] collapsed sensor在configured topmost=false时仍可hover。
- [ ] collapsed/hidden/hover-unfocused不能编辑Document。
- [ ] hover click重新focus和enable editor。
- [ ] monitor identity不使用HMONITOR持久化。
- [ ] CCD stable identity实现。
- [ ] QueryDisplayConfig只在必要时运行。
- [ ] Work area使用rcWork。
- [ ] negative monitor coords支持。
- [ ] floating relative position支持。
- [ ] dock relative offset支持。
- [ ] size使用DIP持久化。
- [ ] 100% DPI。
- [ ] 125% DPI automatic geometry。
- [ ] 150% DPI automatic geometry。
- [ ] 200% DPI automatic geometry。
- [ ] monitor missing startup→primary。
- [ ] runtime monitor disconnect→primary recovery。
- [ ] dock recovery保持same edge。
- [ ] monitor disconnect visible恢复expanded。
- [ ] hidden monitor disconnect下次show正确。
- [ ] DPI change不改Document。
- [ ] DPI change更新IME rect。
- [ ] DPI change更新math/image raster必要部分。
- [ ] display topology event不是polling。
- [ ] config persistence无write flood。
- [ ] Shell memory被测量。
- [ ] Hidden memory被测量。
- [ ] Collapsed memory被测量。
- [ ] Tray memory delta被测量。
- [ ] startup timing被测量。
- [ ] idle CPU各状态被测量。
- [ ] animation CPU被测量。
- [ ] 1000 dock cycles无明显leak。
- [ ] 100 tray cycles无明显leak。
- [ ] dependency delta完成。
- [ ] Windows API delta完成。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] no WebView。
- [ ] no Tauri runtime。
- [ ] no Tokio。
- [ ] no network。
- [ ] no Mica/Acrylic。
- [ ] no auto startup。
- [ ] docs更新。
- [ ] coverage matrix更新。
- [ ] Phase8 task/report完成。
- [ ] CI/smoke通过。
- [ ] 未自动进入Phase9。

---

# 709. 人工验收完成定义

以下每项如果环境可用应真实执行：

- [ ] Microsoft Pinyin final shell。
- [ ] WeChat IME final shell。
- [ ] Light real visual。
- [ ] Dark real visual。
- [ ] System theme live change。
- [ ] whole-window opacity visual。
- [ ] math visual。
- [ ] image visual。
- [ ] Source/Preview/Split visual。
- [ ] Left Dock visual。
- [ ] Right Dock visual。
- [ ] Top Dock visual。
- [ ] 3DIP sensor visual。
- [ ] hover no-focus visual。
- [ ] Tray visual/menu。
- [ ] native Export dialog。
- [ ] real 125/150/200 DPI。
- [ ] real dual monitor。
- [ ] real mixed-DPI monitor。
- [ ] real monitor disconnect。
- [ ] real negative monitor geometry if environment permits。
- [ ] sleep/resume if environment permits。
- [ ] RDP reconnect if environment permits。

无法执行：

```text
NOT TESTED
```

不是 FAIL，也不是 PASS。

---

# 710. Final Recommendation

只有：

```text
APPROVE Phase 9
```

或：

```text
APPROVE Phase 9 WITH CONDITIONS
```

或：

```text
STOP — architecture review required
```

---

# 711. Phase 9 预定方向

如果 Phase 8 架构通过：

Phase 9 应是：

> **Pre-Release Convergence, Full Manual Acceptance, Performance Hardening, Packaging & GitHub Release Readiness**

集中完成：

```text
all remaining NOT TESTED
final Windows 11 acceptance
IME
visual polish regression
full performance budget
memory budget
crash/failure injection
clean VM
portable package
icons/resources
license audit
SBOM
GitHub Actions release workflow
checksums
README finalization
RC build
```

Phase 9 不应再出现新的产品能力。

---

# 712. 最终回复格式

必须严格：

# Phase 8 Result

## Preconditions

```text
Phase 7 recommendation
USER approval
starting commit
inherited conditions
```

## Repository State Before Work

```text
branch
clean / dirty
```

## Desktop Shell

```text
undecorated
drag
resize
shadow
corners
control bar
```

## Window State Model

列：

```text
Floating
Docked Left
Docked Right
Docked Top
Expanded
Collapsed
HiddenToTray
Animating
```

## Tray

```text
crate/version
menu
event delivery
Close behavior
Quit behavior
```

## Theme

```text
Light
System
Dark
live system response
```

## Opacity

完整表。

## Always On Top

说明：

```text
configured state
temporary sensor topmost
tray sync
```

## Dock Geometry

```text
threshold
sensor
offset ratio
work area
```

## Auto Hide

```text
hover
focus loss
hover leave
manual
Esc
animation
```

## Focus / Input Safety

明确：

```text
collapsed typing
hidden typing
hover-reveal typing
IME preedit
```

## Multi Monitor

```text
identity source
fallback
negative coords
work area
disconnect
primary recovery
```

## DPI

```text
100
125
150
200
mixed
```

区分 automated/manual。

## Persistence

说明：

```text
window size
monitor id
dock edge
offset
floating ratios
theme
opacity
topmost
```

## Config Write Behavior

说明无flood。

## Acceptance

表：

```text
AC-019
AC-020
AC-021
AC-022
AC-023
AC-024
AC-025
AC-028
AC-029
```

以及回归。

## Inherited Manual Verification

逐项：

```text
MANUAL PASS
NOT TESTED
FAIL
```

## Performance

完整表。

## Memory

完整表。

## Idle CPU

完整表。

## Startup

完整结果。

## Binary Size

```text
Phase7
Phase8
delta
```

## Dependencies Added

表。

## Windows APIs Added

表。

## Unsafe

```text
core = 0
render = 0
Windows shell = ...
```

## Architecture Authority

明确回答。

## Resource / Leak Testing

说明 cycles。

## Architecture Drift

```text
None
```

或 Risk Report。

## Verification

所有命令和 smoke。

## Documentation

```text
task
report
acceptance
coverage
overview
plan
dependency delta
Windows API delta
README
```

## Git

```text
commit(s)
push = no
```

## Recommendation

三选一。

最后：

> Awaiting USER review. Do not start Phase 9 automatically.

完成后立即停止。

