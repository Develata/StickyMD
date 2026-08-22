# StickyMD Phase 10 — Post-Freeze UX Corrections, Automation Consolidation & RC Requalification

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–9 已完成。

当前状态：

```
Phase 9 Result:
NOT RC READY / DO NOT TAG
```

Phase 9 implementation commit：

```
3aedb24d66254860ba1583a4f8d4e6b94f105f51
```

Phase 9 source candidate commit：

```
eb687b2441a5816111c116ce30a01bb5b0fba8c6
```

USER 现在明确批准一次 **post-freeze v1 UX correction**。

这不是恢复自由功能开发。

这是一组经过 USER 明确批准、必须进入 v1 的交互合同修订。

本阶段名称：

> **Phase 10 — Post-Freeze UX Corrections, Automation Consolidation & RC Requalification**

---

# 0. Phase 10 的本质

本阶段包含三个职责：

```
A. Apply USER-approved v1 UX corrections
B. Consolidate automated verification around Rust CLI authority
C. Re-run full RC qualification, including Phase 9 startup gates
```

Phase 10 完成后：

```
Implementation
    ↓
Updated product contract
    ↓
Updated acceptance
    ↓
Full regression
    ↓
RC readiness re-evaluation
```

---

# 1. Phase 10 不是重新开放 Feature Development

严格 Feature Freeze 仍然有效。

只允许实现 USER 已明确批准的以下十项。

任何其它想法：

```
DO NOT IMPLEMENT
```

记录到：

```
docs/report/post-v1-ideas.md
```

或现有 future backlog。

---

# 2. USER 已批准的十项正式合同

以下全部已经 USER 明确批准。

不得再次询问。

---

## UX-10-01 — Windows Traditional Clipboard Shortcuts

除现有：

```
Ctrl+C
Ctrl+X
Ctrl+V
```

外，正式支持：

```
Ctrl+Insert  = Copy
Shift+Delete = Cut
Shift+Insert = Paste
```

所有这些必须映射到：

```
同一个既有 typed Intent
```

不得建立第二套 clipboard pipeline。

---

## UX-10-02 — Global Content Zoom

增加单一全局内容缩放：

```
content_zoom_percent
```

范围：

```
50% – 300%
```

默认：

```
100%
```

持久化到：

```
config.toml
```

---

## UX-10-03 — Zoom Input

正式支持：

```
Ctrl + +
Ctrl + =
Ctrl + Numpad +
Ctrl + -
Ctrl + Numpad -
Ctrl + 0
Ctrl + Mouse Wheel
```

规则：

```
Ctrl + / -       → ±10%
Ctrl + Wheel     → ±5% per notch
Ctrl + 0         → 100%
```

结果 clamp：

```
50–300
```

---

## UX-10-04 — Minimum Window Size

原：

```
360 × 240 DIP
```

改为：

```
220 × 120 DIP
```

这是正式 v1 contract。

---

## UX-10-05 — Tool Window Identity

StickyMD 主窗口：

```
不显示在 Windows taskbar
不显示在 Alt+Tab switcher
```

用户访问已有实例的正式入口：

```
1. 当前可见窗口本身
2. edge sensor
3. system tray
4. 再次启动同一 program directory 的 StickyMD.exe
```

不得加入：

```
WS_EX_NOACTIVATE
```

点击 StickyMD 后仍必须：

```
获得 keyboard focus
支持 IME
正常编辑
```

---

## UX-10-06 — Dock Capture Threshold

原：

```
12 DIP
```

改为：

```
24 DIP
```

---

## UX-10-07 — Dock Edge Selection

只允许：

```
Top
Left
Right
```

无 Bottom。

令：

```
d_top
d_left
d_right
```

表示 drag release point / candidate geometry 与目标 monitor **work area** 相应边缘的距离，单位 DIP。

计算：

```
d_min = min(d_top, d_left, d_right)
```

仅当：

```
d_min <= 24 DIP
```

才发生 Dock。

否则：

```
Floating
```

---

## UX-10-08 — Dock Tie Rule

正常情况下：

> 最近边获胜。

只有两条或多条候选边距离差：

```
<= 1 DIP
```

才视为 tie。

Tie priority：

```
Top > Left > Right
```

注意：

> Top 并不是无条件优先。

---

## UX-10-09 — Dock Release Behavior

用户 drag release 在捕获区：

```
→ DockedExpanded
```

不立即 collapse。

只要窗口：

```
focused
```

就保持 expanded。

之后只有：

```
focus loss
manual collapse
Esc
```

才进入既有 collapse 行为。

---

## UX-10-10 — Opacity

原：

```
70–100
```

改为：

```
40–100
```

默认仍：

```
96
```

仍然：

```
whole-window constant alpha
```

不是：

```
background-only opacity
```

也不是：

```
click-through
```

---

# 3. Automation Architecture 修订

USER 明确批准：

> 能 Rust CLI 化的 smoke / performance / runtime / package / acceptance evidence，优先 Rust CLI 化。

本阶段必须把：

```
Rust CLI
```

提升为：

> automated verification 的主要 authority。

---

# 4. 但这不意味着“所有东西都塞进 Rust CLI”

以下保持原位：

```
cargo test
cargo clippy
cargo fmt
cargo deny
GitHub Actions YAML
```

不要为了形式统一包一层无意义 CLI。

---

# 5. Manual Acceptance 不能 CLI 化

以下永远不得被 Rust CLI 冒充：

```
真实 Microsoft Pinyin
真实 WeChat IME
真实视觉
真实 Tray UI
真实显示器拔插
真实 mixed DPI 视觉
真实 Alt+Tab / foreground behavior
真实 export dialog视觉
```

CLI 可以：

```
启动
驱动
采集
记录
辅助
```

但最终状态仍：

```
MANUAL PASS
NOT TESTED
FAIL
```

---

# 6. Phase 10 开始前必须修改 Authority 文档

因为本阶段修改了冻结产品行为：

必须先：

```
plan update
→ review
→ implementation
```

不能先改代码再让 plan 追代码。

---

# 7. 开始前读取

严格遵守：

```
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md
```

必须完整读取：

```
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
```

同时读取：

```
docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md
```

以及：

```
所有 Phase 8 reports
所有 Phase 9 reports
Phase 9 release readiness
Phase 9 performance
Phase 9 manual acceptance
RISK-source-font-startup
```

---

# 8. Repository Preflight

执行：

```
git status --short
git branch --show-current
git log -15 --oneline

cargo metadata --no-deps

cargo tree -p stickymd-core
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```
starting commit
branch
clean / dirty
```

不得：

```
reset
clean
rebase
force
```

---

# 9. Phase 10 分段

严格顺序：

```
10A — Contract Amendment
10B — Automation CLI Consolidation
10C — Clipboard Compatibility Shortcuts
10D — Content Zoom
10E — Compact Window Geometry
10F — Tool Window / Taskbar / Alt+Tab Identity
10G — Dock Capture Semantics
10H — Opacity Range Revision
10I — Warm Startup Benchmark Audit
10J — Full Regression & RC Requalification
```

不要边做 UX 边顺手重构无关模块。

---

# 10. Phase 10A — Contract Amendment

创建：

```
docs/phases/2026-08-22-phase-10-user-approved-ux-corrections.md
```

或当前项目 phase archive naming convention。

必须明确：

```
USER-approved post-freeze correction
```

不是 Agent 自行扩 scope。

---

# 11. 更新 plan

至少：

```
docs/plan/07_editor_and_ime.md
docs/plan/09_windows_shell.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md
```

---

# 12. Editor Plan 更新

加入：

```
Ctrl+Insert
Shift+Insert
Shift+Delete
content zoom
zoom keyboard/mouse wheel behavior
```

---

# 13. Windows Shell Plan 更新

正式修改：

```
minimum window = 220×120 DIP
opacity = 40–100
taskbar = hidden
Alt+Tab = hidden
dock capture = 24 DIP
tie epsilon = 1 DIP
priority = Top > Left > Right
Bottom unsupported
release in capture zone = DockedExpanded
```

---

# 14. Features 文档同步

这是 USER-visible product behavior。

---

# 15. Acceptance 更新

不得重编号已有：

```
AC-001..AC-030
```

可以：

### A

扩展已有对应 AC。

### B

增加：

```
AC-031 ...
```

如果项目治理允许追加。

建议新增 Phase10专门验收：

```
AC-031 Traditional Clipboard Shortcuts
AC-032 Content Zoom
AC-033 Compact Window
AC-034 Tool Window Identity
AC-035 Dock Capture Semantics
AC-036 Extended Opacity
```

如果当前治理不允许新增编号：

使用：

```
PH10-...
```

但不要破坏 frozen numbering。

---

# 16. Coverage Matrix

在代码实现前先更新：

```
planned code areas
```

实现后再换成 actual paths。

---

# 17. Architecture Change Classification

本阶段不属于 skeleton change。

正式记录：

```
Change class:
USER-approved product behavior correction
+
platform shell implementation refinement
```

---

# 18. Tool Window identity

尤其记录：

> 不是新增 WindowState variation。

它是 StickyMD Windows app 永久 window identity。

不要创建：

```
show_in_taskbar: bool
show_in_alt_tab: bool
```

用户设置。

---

# 19. Phase 10B — Rust Automation CLI Consolidation

先盘点当前：

```
tools/smoke/*.ps1
Rust smoke binaries
package scripts
performance tools
runtime tools
```

---

# 20. 目标

形成一个稳定：

```
Rust automation command
```

推荐形式：

```
cargo xtask ...
```

或：

```
tools/stickymd-dev/
```

---

# 21. 如果现有已有 Rust smoke CLI

优先演进现有。

不要另起：

```
第二套 automation CLI
```

---

# 22. 不为名字“xtask”重写全部

架构目标：

```
one Rust automation authority
```

而不是名字。

---

# 23. 推荐 command surface

例如：

```
stickymd-dev ci
stickymd-dev smoke --phase 10
stickymd-dev performance
stickymd-dev runtime
stickymd-dev package
stickymd-dev verify-package
stickymd-dev acceptance automated
stickymd-dev readiness
```

具体按已有CLI设计。

---

# 24. 不制造几十个 flags

保持：

```
subcommand
typed options
```

---

# 25. Rust CLI 输出

必须同时支持：

```
human-readable
machine-readable JSON
```

推荐：

```
--json
```

---

# 26. Evidence Schema

例如：

```
{
  "schema_version": 1,
  "commit": "...",
  "artifact_sha256": "...",
  "suite": "phase-10",
  "results": []
}
```

---

# 27. 不把 JSON schema过度复杂化

只保证：

```
stable enough for scripts/CI
```

---

# 28. PowerShell role

PowerShell 只保留：

```
Windows environment setup
thin invocation wrapper
GUI/manual helper
GitHub workflow Windows shell integration
```

---

# 29. PowerShell不得重复业务判断

例如不得同时存在：

```
Rust says gate <=300ms
PowerShell says gate <=400ms
```

---

# 30. Gate数值 authority

应集中在：

```
Rust constants/config
```

或 plan-projected single source。

---

# 31. Smoke thin wrapper

原：

```
tools/smoke/phase-10.ps1
```

可以只：

```
resolve exe
invoke Rust CLI
propagate exit code
```

---

# 32. 不必 Rust 化

```
cargo fmt
cargo clippy
cargo test
cargo deny
```

CI直接执行。

---

# 33. Performance采集

Rust CLI 应优先负责：

```
process launching
ready event
Win32 process metrics
timing
statistics
JSON output
gate evaluation
```

---

# 34. Package verification

Rust CLI 可负责：

```
ZIP entry allowlist
hash
PE verification
manifest/resource verification
path traversal
proprietary font scan
```

如果当前 PowerShell逻辑很成熟：

可以分阶段迁移。

---

# 35. 不为 Rust 化增加 production dependency

automation crates：

```
dev/tool workspace only
```

不得进入：

```
stickymd-win normal dependency
```

---

# 36. Automation CLI性能

不是因为 CLI自己必须“超快”。

价值排序：

```
correctness
single authority
typed errors
portable logic
deterministic evidence
performance
```

---

# 37. Phase 10 不强制删掉所有 PowerShell

只要求：

> 不再让 PowerShell 成为 duplicated automated test logic authority。

---

# 38. Automation Tests

CLI自身必须有：

```
unit tests
schema tests
exit-code tests
```

---

# 39. Exit Code

统一：

```
0 = gates passed
nonzero = failed/blocked
```

---

# 40. NOT TESTED 自动项

如果 automated environment缺 capability：

机器输出应明确：

```
NOT_TESTED
```

不是 PASS。

---

# 41. Manual acceptance不会由 readiness CLI自动变 PASS

---

# 42. Phase 10C — Clipboard Compatibility

找到当前：

```
keyboard translation
shortcut handling
```

正式实现：

```
Ctrl+Insert → CopySelection
Shift+Delete → CutSelection
Shift+Insert → PasteClipboard
```

---

# 43. 必须复用 Intent

例如：

```
Ctrl+C
Ctrl+Insert
```

最终产生：

```
同一个 CopySelection
```

---

# 44. Paste

```
Ctrl+V
Shift+Insert
```

必须走完全相同：

```
text / file image / encoded image / bitmap
```

优先级。

---

# 45. 不允许 Shift+Insert text-only

---

# 46. Cut failure safety

Shift+Delete必须和 Ctrl+X同样：

```
clipboard write fail
→ Document unchanged
```

---

# 47. Preview shortcuts

Preview：

```
Ctrl+Insert = Copy
```

合理。

---

# 48. Preview Shift+Delete

因为 Preview readonly：

```
no Cut
```

不得改变 Document。

---

# 49. Preview Shift+Insert

不得 Paste。

---

# 50. Shell control focus

如果 opacity numeric input active：

这些传统快捷键按照普通 text field context。

不要误操作 Document。

---

# 51. IME composing

传统快捷键行为遵循现有 IME guard。

不得 phantom commit。

---

# 52. Keyboard layout

判断：

```
Insert/Delete key
modifiers
```

不要通过字符文本路径。

---

# 53. Tests

自动覆盖：

```
Ctrl+C == Ctrl+Insert
Ctrl+X == Shift+Delete
Ctrl+V == Shift+Insert
```

---

# 54. Image Paste Test

mock image clipboard：

```
Shift+Insert
```

必须生成同样 managed asset transaction。

---

# 55. Phase 10D — Content Zoom

正式加入：

```
ContentZoom
```

或等价 constrained value。

---

# 56. 不要用裸 f32 持久化

推荐：

```
struct ContentZoomPercent(u16);
```

或：

```
u16
```

确保：

```
50..=300
```

---

# 57. Config

增加：

```
content_zoom_percent = 100
```

---

# 58. Config Schema

Phase4已有：

```
version = 1
```

如果新增 optional default field不要求 breaking migration：

可以仍 version 1，前提当前 policy允许：

```
missing field → default
```

如果项目 plan规定 schema change必须version bump：

按 plan。

不要自行决定。

---

# 59. Invalid config zoom

```
<50 → clamp 50
>300 → clamp 300
```

或 invalid whole config policy按当前 Config model。

优先使 validated DTO明确。

---

# 60. Zoom authority

```
ConfigCoordinator committed zoom
```

是 preference authority。

运行时：

```
Window/App presentation state
```

使用 effective zoom。

---

# 61. Zoom 不属于 DocumentState

不得：

```
generation += 1
dirty = true
autosave note
```

---

# 62. Zoom作用域

统一：

```
Source
Preview
Split Source
Split Preview
```

同一倍率。

---

# 63. Shell不缩放

这些保持不变：

```
top control bar
buttons
window resize handles
dock threshold
sensor
tray
opacity popup shell dimensions
```

---

# 64. Source Zoom

影响：

```
font size
line height
caret
selection
hit testing
IME cursor area
scroll geometry
```

---

# 65. Preview Zoom

影响：

```
body text
heading
code
table text
math effective size
block spacing if em-relative
```

---

# 66. Images

图片仍：

```
max width = pane content width
preserve aspect ratio
no forced enlargement beyond intended policy
```

---

# 67. Zoom 与图片

默认：

> 内容 zoom 改变可用于图片的 layout尺度/上下文，但不允许图片把 pane 撑宽。

如果当前 preview image逻辑按 intrinsic DIP：

zoom可以使它的目标 display size乘zoom，但仍 clamp pane。

---

# 68. 不生成超大 raster

如果 zoom 300%：

仍受：

```
image cache
raster safety
viewport
```

约束。

---

# 69. Math

effective math font size：

```
base_preview_size × zoom
```

---

# 70. Math Cache

Zoom变化：

```
RaTeX semantic parse/cache can remain
raster cache invalidates for effective size
```

不要重新parse Math source。

---

# 71. Markdown

Zoom：

```
Markdown parse count delta = 0
RenderTree semantic rebuild = 0
layout rebuild = yes
```

---

# 72. Source

Zoom会 relayout cosmic text。

---

# 73. Preview scroll preservation

Zoom时推荐保持：

```
当前 viewport logical anchor
```

最简单可以保持：

```
scroll ratio
```

但应避免用户直接跳到文档完全不同位置。

---

# 74. 更推荐 anchor

若已有 source/preview mapping：

```
top visible block/source position
```

尽量保持。

---

# 75. 不为 zoom建复杂 scroll-sync系统

如果实现成本高：

可以只 clamp existing scroll_y。

记录行为。

---

# 76. Ctrl + +

注意主键盘：

通常：

```
+ = Shift + =
```

因此 shortcut translator必须正确接受：

```
Ctrl + =
Ctrl + Shift + =
```

---

# 77. Numpad

同时支持：

```
NumpadAdd
NumpadSubtract
```

---

# 78. Ctrl+0

```
100%
```

---

# 79. Ctrl+Wheel

只有：

```
Ctrl held
```

时处理 zoom。

---

# 80. 非 Ctrl wheel

保持：

```
Source scroll
Preview scroll
```

---

# 81. Wheel direction

```
up → +5
down → -5
```

---

# 82. Trackpad high-resolution wheel

不能每个微小 delta都 ±5。

必须量化“notch”。

---

# 83. 建立 wheel accumulator

例如：

```
accumulate line delta
```

达到一个标准 step再：

```
±5%
```

---

# 84. PixelDelta

高分辨率触控板：

换算到合理阈值。

不要一帧滚动放大几十级。

---

# 85. Zoom Config Write

键盘每次 ±10：

可以立即提交 config。

---

# 86. Ctrl+Wheel连续缩放

不要每个 wheel event写 config。

使用类似 opacity：

```
live preview
short debounce / gesture end commit
```

---

# 87. 没有明确 wheel gesture-end event

建议：

```
250 ms inactivity
→ commit config
```

---

# 88. 这只是 config preference debounce

不是 Document autosave。

---

# 89. 不创建 generic Debounce Framework

已有 scheduler即可。

---

# 90. Zoom bounds

严格：

```
50
60
...
300
```

键盘 step可能最后：

```
295 +10 →300
```

---

# 91. Mouse wheel可以产生：

```
50,55,60...
```

---

# 92. Ctrl+0

立即100并commit。

---

# 93. Mode persistence

Zoom在：

```
Source→Preview→Split
```

保持。

---

# 94. Restart

恢复。

---

# 95. Zoom tests

至少：

```
50
100
300
Ctrl+
Ctrl=
Numpad+
Ctrl-
Numpad-
Ctrl0
wheel
clamp
restart
```

---

# 96. Zoom no-authority tests

确保：

```
Document generation unchanged
saved_generation unchanged
dirty unchanged
undo unchanged
```

---

# 97. Preview instrumentation

100 zoom操作：

```
Markdown parse delta = 0
```

---

# 98. Math instrumentation

```
RaTeX source parse preferably = 0
raster rebuild expected
```

---

# 99. Image instrumentation

不得重新decode相同足够分辨率图片，除非更高输出resolution确实需要。

---

# 100. Phase 10E — Compact Window Geometry

原 minimum：

```
360×240 DIP
```

废止。

正式：

```
220×120 DIP
```

---

# 101. 不要改变默认窗口

仍：

```
520×680 DIP
```

---

# 102. Minimum只是允许更小

---

# 103. Source在220×120必须仍可基本操作

至少：

```
top bar
small editor viewport
caret
typing
scroll
```

---

# 104. Preview

同样至少：

```
能显示
能scroll
```

---

# 105. Split

用户明确要求：

> 任何 ViewMode 不因为窗口变小自动改变。

所以：

```
Split remains Split
```

---

# 106. 废除：

```
each split pane >=240 DIP
```

硬限制。

---

# 107. Split小窗口

在220px宽附近：

```
50/50
```

两边会很窄。

这是用户选择。

不得自动：

```
switch Source
hide Preview
stack vertically
```

---

# 108. Divider仍1 DIP

---

# 109. 控制栏 Compact Behavior

220 DIP宽时，现有所有按钮可能放不下。

必须解决。

---

# 110. 不新增 hamburger settings menu

这会改变交互。

---

# 111. 建议 compact priority

保留：

```
Source/Split/Preview
Pin
Theme
Opacity
Collapse
Close
```

但：

- 缩小 horizontal gaps；
    
- 减少button hit visual width；
    
- 保持最小可点击区域合理。
    

---

# 112. 如果 220 DIP真的放不下

允许：

```
controls visual overlap/secondary compact state
```

但不要自动隐藏关键 Close。

---

# 113. 优先保留顺序

```
Close
ViewMode
Collapse when docked
```

其次：

```
Pin
Theme
Opacity
```

---

# 114. 如果需要条件隐藏次要控制

只允许在极窄窗口：

例如：

```
width < X
```

隐藏：

```
Pin
Theme
Opacity
```

用户仍可通过：

```
Tray topmost
restart config
```

但 Theme/Opacity若没有其它入口会不可达。

因此更推荐：

> compact layout，而非隐藏。

---

# 115. 不引入“...”菜单

除非实测220宽绝对无法满足。

若需要：

这属于新增interaction，必须提交 USER review。

默认不能做。

---

# 116. Button hit target

在极小窗口也不要小于约：

```
24×24 DIP
```

如果当前设计要求更大，可根据实际。

---

# 117. Topbar

约：

```
30–34 DIP
```

可以在 compact width时略降：

```
28–30 DIP
```

但不能缩到难点击。

---

# 118. Minimum Height 120

扣除 topbar 后仍有：

```
~90 DIP
```

内容。

---

# 119. Resize Handle

6 DIP可以保持。

---

# 120. Dock sensor

仍3 DIP，不随min size改变。

---

# 121. Clamp

Monitor recovery如果保存窗口220×120：

必须允许。

不能恢复成360×240。

---

# 122. Config migration

旧size：

不变。

新用户可缩更小。

---

# 123. Tests

geometry：

```
220×120
221×121
below minimum clamp
Split at minimum
Dock left minimum
Dock right minimum
Dock top minimum
DPI100/125/150/200
```

---

# 124. Visual极小窗口

必须在manual matrix新增：

```
220×120 Source
220×120 Preview
220×120 Split
```

---

# 125. Phase 10F — Taskbar / Alt+Tab

这是 Windows shell identity修正。

---

# 126. 不需要修改 WindowShellState

不得新增：

```
show_in_taskbar
show_in_alt_tab
```

它是固定 app invariant。

---

# 127. 实现优先级

### Step 1

使用当前 winit：

```
with_skip_taskbar(true)
```

在 main window creation时配置。

---

# 128. 窗口创建仍 hidden-first

理想：

```
create hidden
→ apply tool identity
→ tray ready
→ placement/theme/opacity
→ show
```

---

# 129. 必须先真实验证

```
Taskbar
Alt+Tab
```

是否都消失。

---

# 130. 如果 winit skip-taskbar 已同时满足

停止。

不要加 Win32代码。

---

# 131. 如果 Taskbar消失但 Alt+Tab仍存在

增加薄：

```
Windows ToolWindowIdentity adapter
```

---

# 132. Win32 fallback

正式要求：

```
add WS_EX_TOOLWINDOW
remove WS_EX_APPWINDOW
preserve all unrelated extended style bits
```

---

# 133. 禁止：

```
WS_EX_NOACTIVATE
```

---

# 134. ToolWindow adapter

只允许：

```
platform/windows/
```

---

# 135. 修改时机

优先 window show之前。

---

# 136. 如果必须 runtime 修改

遵循：

```
hide
→ modify extended style
→ show
```

不得在 visible状态粗暴改后不通知Shell。

---

# 137. Tray readiness invariant

在进入：

```
Taskbar hidden
Alt+Tab hidden
```

的正常生命周期前：

```
Tray must be initialized
```

---

# 138. Tray失败

不得进入 tool-window-only状态。

---

# 139. 恢复入口

必须同时回归：

```
sensor
tray
second instance wake
visible click
```

---

# 140. Alt+Tab行为

Expected：

```
StickyMD not present
```

---

# 141. Alt+Tab from StickyMD

用户当前focused StickyMD后按：

```
Alt+Tab
```

必须能切到别的正常窗口。

这是当前用户观察到的bug之一。

---

# 142. 不能发生

```
Alt+Tab ineffective
StickyMD traps switching
```

---

# 143. Focus

点击 StickyMD：

仍能focused。

---

# 144. IME

仍能正常。

---

# 145. Taskbar

不应出现按钮。

---

# 146. Alt+Tab

不应出现 StickyMD thumbnail/item。

---

# 147. Win+D

不做特殊保证。

正常Windows行为。

---

# 148. Tray Show

必须正常。

---

# 149. Second instance wake

### hidden

show/focus。

### collapsed

expand/focus。

### visible

focus。

---

# 150. Foreground restrictions

Windows可能限制：

```
SetForegroundWindow
```

继续使用现有 fallback：

```
show
raise
flash attention
```

按 Phase4/8已验证逻辑。

---

# 151. ToolWindow与Dock

不得有关系。

Dock不应把taskbar item重新出现。

---

# 152. Opacity 40

也不应改变toolwindow identity。

---

# 153. Tests

自动 Win32 style readback。

---

# 154. 但 taskbar/Alt+Tab视觉

必须：

```
MANUAL PASS / NOT TESTED
```

或 GUI automation evidence另标。

---

# 155. 不用进程是否存活冒充 Alt+Tab验证

---

# 156. Phase 10G — Dock Capture Revision

修改 pure geometry/state logic。

---

# 157. SNAP_THRESHOLD

正式：

```
24 DIP
```

---

# 158. TIE_EPSILON

正式：

```
1 DIP
```

---

# 159. Eligible Edges

只有：

```
Top
Left
Right
```

---

# 160. 无 Bottom

不得加入：

```
Bottom
```

---

# 161. Candidate distance

必须相对于：

```
target monitor rcWork
```

---

# 162. Release point

优先：

```
actual pointer location at user drag release
```

---

# 163. 如果现有实现按window rect edge

可以保留 candidate geometry，只要 contract与用户感受一致。

但必须在 report说明：

```
what distance means
```

---

# 164. 推荐定义

最符合USER语言：

```
pointer release point to work-area edge
```

---

# 165. Candidate Monitor

使用：

```
release point containing monitor
```

---

# 166. 如果 pointer恰在无monitor gap

nearest monitor。

---

# 167. Distances

在 target monitor coordinate：

```
d_top   = abs(y - work.top)
d_left  = abs(x - work.left)
d_right = abs(work.right - x)
```

转换为 DIP。

---

# 168. 如果 pointer在work area外一点

abs仍可用。

但：

```
<=24
```

才capture。

---

# 169. 如果离Bottom最近

Bottom不eligible。

例如：

```
d_bottom = 2
d_right = 18
```

因为bottom unsupported：

right仍可capture，只要：

```
d_right<=24
```

---

# 170. 是否应因为bottom更近而阻止right？

USER批准的算法只比较：

```
Top / Left / Right
```

因此：

```
ignore bottom
```

---

# 171. Nearest eligible edge wins

---

# 172. Tie

例如：

```
d_top=10
d_left=10.7
```

差：

```
0.7 <=1
```

→ tie：

```
Top
```

---

# 173. 非tie

```
d_top=10
d_left=11.2
```

→ Top本来就更近。

---

# 174. 关键反例

```
d_top=14
d_left=10
```

必须：

```
Left
```

不能因为priority选Top。

---

# 175. 另一反例

```
d_left=9
d_right=8
```

→ Right。

---

# 176. Tie Left/Right

```
Left
```

---

# 177. Three-way tie

```
Top
```

---

# 178. Release在capture

state：

```
DockedExpanded(edge)
focused=true
```

---

# 179. 不立即auto-collapse

---

# 180. 既有focus loss

之后：

```
700ms
```

---

# 181. 手动/Esc

既有立即collapse。

---

# 182. Hover sensor timings

不改。

---

# 183. Detach

仍：

```
16 DIP
```

---

# 184. Detach threshold vs snap threshold

允许：

```
detach 16
snap 24
```

形成 hysteresis。

这是好事。

---

# 185. 为什么

避免：

```
刚拖离12px
又立刻重新吸附
```

---

# 186. Geometry tests

必须至少：

```
distance 23.9 → dock
distance 24.0 → dock
distance 24.1 → float
```

---

# 187. Tie tests

```
Top vs Left <1
Top vs Left =1
Top vs Left >1
Left vs Right tie
three-way
```

---

# 188. Bottom proximity tests

---

# 189. Negative monitor coords

同样。

---

# 190. Mixed DPI

distance必须在DIP比较。

---

# 191. 不用physical 24px

150% monitor：

```
24 DIP = 36px
```

---

# 192. Phase 10H — Opacity 40–100

修改：

```
MIN_OPACITY = 40
```

---

# 193. Default仍：

```
96
```

---

# 194. Slider

```
40–100
step1
```

---

# 195. Numeric

同样。

---

# 196. Config Migration

旧：

```
70–100
```

都仍合法。

---

# 197. 如果 config 旧值意外 <70但>=40

现在合法。

---

# 198. <40

clamp40。

---

# 199. Whole-window alpha

Win32流程不变。

---

# 200. 40%

alpha约：

```
102
```

根据：

```
round(40*255/100)
```

---

# 201. 不 click-through

不得加：

```
WS_EX_TRANSPARENT
```

---

# 202. 40%仍：

```
mouse hit
focus
IME
tray
dock
```

正常。

---

# 203. 100%

继续移除不必要 layered style。

---

# 204. Tests

```
40
50
70
96
100
39→40
101→100
```

---

# 205. Manual

40%必须真实检查：

```
readability
math
image
controls
IME
```

---

# 206. Phase 10I — Warm Startup Audit

Phase9当前：

```
Cold p95 = 277.205 ms PASS
Warm p95 = 342.891 ms FAIL
```

Warm比Cold更差：

```
suspicious / non-intuitive
```

必须先审 benchmark methodology。

---

# 207. 不能一上来优化字体

先验证：

```
measurement correctness
```

---

# 208. 重新读取 Phase9 startup report

理解：

```
cold definition
warm definition
ready signal
process cleanup
sample order
```

---

# 209. 检查 readiness signal

必须仍：

```
EDITOR_READY
```

而不是：

```
process started
window created
```

---

# 210. Warm process lifecycle

每样本前必须确认：

```
previous StickyMD fully exited
named mutex released
tray destroyed
worker threads joined
process handle signaled
```

---

# 211. 检查 same-directory interaction

Warm连续run若：

```
previous instance still terminating
```

可能导致：

```
second-instance wake path
```

污染数据。

必须排除。

---

# 212. 每个 sample

等待：

```
process exit confirmed
```

不是固定 sleep猜测。

---

# 213. Ready signal唯一

避免：

```
stale named event
```

被下一次进程误读。

---

# 214. 每次 readiness object

必须 unique：

```
PID / nonce
```

---

# 215. Startup test directory

避免 watcher/asset recovery残留影响。

---

# 216. 但不要每个 warm sample用全新directory

否则不再是warm steady-state。

---

# 217. 推荐 Warm定义

同一 clean portable dir：

```
note/config already bootstrapped
```

每次：

```
launch
EDITOR_READY
quit gracefully
wait process exit
```

连续20+次。

---

# 218. Cold定义

可以：

```
fresh process but existing app data
```

与 OS cache语义按 Phase9已有定义。

必须在 report清楚。

---

# 219. 如果 Phase9 Cold/Warm命名错误

允许：

```
修正 benchmark terminology
```

这是测量事实纠错，不是放宽gate。

---

# 220. 但 gate仍：

```
Cold p95 <=300
Warm p95 <=180
```

---

# 221. 重新采样

至少：

```
30 cold
30 warm
```

---

# 222. 随机化/交错

为了降低时间漂移：

建议：

```
C W W C W C ...
```

或分组但记录系统load。

---

# 223. 记录

```
Defender
CPU load
background process
```

---

# 224. Outlier

不得任意删。

---

# 225. 可以单独标记

例如：

```
Defender scan
```

但统计仍保留。

---

# 226. 如果修正 methodology 后 Warm PASS

更新：

```
Phase9 performance final
Phase10 report
```

说明：

```
measurement bug fixed
```

---

# 227. 如果仍FAIL

再profiling。

---

# 228. Startup milestones

继续：

```
font
tray
display
persistence
window
source projection
```

---

# 229. 新 UX影响startup

Tool-window identity、zoom config不会显著增加。

必须确认。

---

# 230. 不为了warm gate

牺牲：

```
font fallback
IME
tray readiness
monitor placement
```

---

# 231. 如果 Warm无法180

沿 Phase9 gate review流程：

```
STOP / USER decision
```

Agent不能改门槛。

---

# 232. Phase 10J — Full Regression

实现所有改动后：

生成：

```
RC requalification candidate
```

---

# 233. 必须重新跑所有 Phase9 automated release gates

不能只跑 Phase10 tests。

---

# 234. 重点回归

```
Document
IME architecture
Persistence
OCC
Preview
Math
Images
Assets
Export
Dock
Tray
Config
Release package
```

---

# 235. 新 Clipboard shortcuts

加入 final acceptance。

---

# 236. Zoom

加入 final acceptance。

---

# 237. Compact Window

加入。

---

# 238. Tool Window

加入。

---

# 239. Dock Capture

加入。

---

# 240. Opacity 40

加入。

---

# 241. AC-001..AC-030

仍全部重新映射最终状态。

---

# 242. Manual遗留

继续保持。

---

# 243. Phase10 manual-required新项目

至少：

```
Traditional shortcuts real keyboard
Zoom real Source/Preview/Split
Ctrl+wheel real
220×120 visual
Taskbar hidden
Alt+Tab absent
Alt+Tab from focused StickyMD switches away
Tray restore
Sensor restore
second-instance restore
24DIP dock feel
nearest-edge behavior
opacity40 visual
```

---

# 244. ToolWindow Manual Test

明确：

### T1

StickyMD visible：

```
Taskbar item absent
```

### T2

Alt+Tab列表：

```
StickyMD absent
```

### T3

StickyMD focused：

```
Alt+Tab once
```

切到previous/next app。

### T4

click StickyMD：

可focus和type。

### T5

Pinyin仍工作。

### T6

Tray show。

### T7

sensor show。

### T8

second instance wake。

---

# 245. Windows style readback自动化不能代替 T1/T2

---

# 246. Dock Feel Test

真实拖动：

```
5 DIP
12 DIP
20 DIP
24 DIP
30 DIP
```

观察capture。

---

# 247. Corner Test

靠近：

```
Top-Left
Top-Right
```

验证 nearest/tie rule。

---

# 248. 不是只测单元函数

---

# 249. Zoom visual matrix

至少：

```
50
75
100
150
200
300
```

---

# 250. Source

检查：

```
caret
selection
IME
scroll
```

---

# 251. Preview

检查：

```
paragraph
table
math
image
```

---

# 252. Split

两个pane同倍率。

---

# 253. Window shell

不缩放。

---

# 254. Tiny window

220×120：

```
Source
Preview
Split
```

---

# 255. Resize

比minimum再小：

系统clamp。

---

# 256. Opacity40

真实。

---

# 257. Traditional shortcut manual

至少：

```
Ctrl+Insert copy text
Shift+Delete cut text
Shift+Insert paste text
Shift+Insert paste screenshot/image
```

---

# 258. Preview

```
Ctrl+Insert copy
Shift+Delete no mutation
Shift+Insert no mutation
```

---

# 259. Zoom与shortcut冲突

`Ctrl+Insert`不能触发其它行为。

---

# 260. Ctrl+0

不能输入字符0。

---

# 261. Ctrl+=

不能插入 `=`。

---

# 262. Ctrl+Wheel

不能同时scroll。

---

# 263. Zoom config persistence

restart。

---

# 264. Warm Startup在最终UX commit重新测

---

# 265. Final performance

重新测：

```
startup
memory
idle CPU
typing
preview
image
```

至少完整 Phase9 hard gates。

---

# 266. Zoom Extremes Performance

特别：

```
300% Preview
300% Split
```

检查：

```
memory
layout time
no huge raster explosion
```

---

# 267. 50%

检查：

```
font legibility不作为hard
geometry correctness
```

---

# 268. 300% Math

raster safety仍有效。

---

# 269. 300% Image

不能超过cache/alloc guard。

---

# 270. Minimum window performance

无特别负担。

---

# 271. ToolWindow memory

不应明显变化。

---

# 272. Automation CLI回归

CI必须优先调用新的 Rust authority。

---

# 273. `all.ps1`

可以继续作为 thin orchestration。

但其内部：

```
Rust automation CLI
```

完成自动化逻辑。

---

# 274. 不需要一次性重写 GitHub Actions所有command

只需避免duplicated rules。

---

# 275. Release scripts

如果已有 PowerShell package script：

Phase10可保留 shell-level ZIP操作。

但 package verification rules最好 Rust 化。

---

# 276. Phase10 Automation Report

创建：

```
docs/report/phase-10-automation-consolidation.md
```

列：

```
before
after
Rust-owned logic
PowerShell-owned logic
duplicated logic removed
JSON schema
CI usage
```

---

# 277. Phase10 UX Report

创建：

```
docs/report/phase-10-ux-corrections.md
```

---

# 278. Phase10 Startup Report

创建/更新：

```
docs/report/phase-10-startup-requalification.md
```

---

# 279. Phase10 Acceptance

创建：

```
docs/acceptance-cases/phase-10.md
```

---

# 280. Phase10 Task

创建：

```
docs/tasks/phase-10-ux-corrections-rc-requalification.md
```

---

# 281. Task Status

开始：

```
In Progress
```

---

# 282. 如果 implementation完成但 manual仍未测

```
Implementation Complete — RC validation incomplete
```

---

# 283. 只有达到 readiness

```
Completed — awaiting USER RC decision
```

---

# 284. Window Shell Tests

新增 pure tests：

```
min size
snap24
tie1
priority
no bottom
release expanded
```

---

# 285. Shortcut Tests

新增。

---

# 286. Zoom Tests

新增。

---

# 287. Config Tests

旧config缺 zoom：

```
default100
```

---

# 288. Old opacity70

继续70。

---

# 289. 新opacity40

roundtrip。

---

# 290. ToolWindow Tests

如果使用 Win32 fallback：

```
WS_EX_TOOLWINDOW present
WS_EX_APPWINDOW absent
WS_EX_NOACTIVATE absent
```

---

# 291. 不要求 Core知道 style bits

---

# 292. Automated Performance

Rust CLI负责：

```
statistics
gate
JSON evidence
```

---

# 293. Startup readiness object

确保无 stale-object bug。

---

# 294. Manual status仍从人工报告读取，不由 CLI写 PASS

---

# 295. Architecture Boundaries

不得因为 Zoom把：

```
WindowShellState
```

与：

```
DocumentState
```

混在一起。

---

# 296. Zoom状态可以放

```
Config/App presentation state
```

不是 window placement核心。

---

# 297. Keyboard translation

仍 Interaction Shell。

---

# 298. Dock geometry

仍 pure。

---

# 299. Tool identity

仍 platform invariant。

---

# 300. Automation CLI

是 tools workspace，不进入 runtime。

---

# 301. Dependency Discipline

Phase10理想：

```
production dependencies added = 0
```

---

# 302. 如果 automation CLI需要 dev deps

可以。

---

# 303. 不为了 CLI加入production serde_json

如果 tooling已有可使用。

---

# 304. ToolWindow不应需要新crate

用 winit/windows已有能力。

---

# 305. Zoom不需要新crate

---

# 306. Shortcut不需要新crate

---

# 307. Dock不需要新crate

---

# 308. Dependency delta如果无

报告：

```
No new runtime dependencies.
```

---

# 309. Binary Size

Phase10 UX逻辑不应大幅增加。

记录：

```
Phase9 EXE
Phase10 EXE
delta
```

---

# 310. Trigger

如果：

```
+1 MiB runtime binary
```

仅这些改动导致：

分析。

---

# 311. Memory

ToolWindow/zoom state不应增加明显stable PWS。

---

# 312. Leak Tests

继续：

```
zoom 1000
dock 1000
tray
opacity
```

---

# 313. Zoom Cache Leak

50↔300重复：

math/image raster cache必须有界。

---

# 314. Config Write Count

Ctrl+wheel 100 events：

不能100次disk write。

---

# 315. Keyboard zoom 10次

10个独立 user command可以10次config commit。

这是可接受。

---

# 316. Wheel debounce

test。

---

# 317. Startup config

读取 zoom不能显著影响startup。

---

# 318. 220×120 Startup

保存小尺寸后restart：

恢复。

---

# 319. Taskbar Identity Startup

不闪现taskbar icon。

这是重要 visual manual test。

---

# 320. Hidden-first

tool identity必须在 first show前ready。

---

# 321. Tray before inaccessible identity

hard invariant。

---

# 322. If tray init fail

main window必须仍可达或 startup fail visibly。

不能：

```
no taskbar
no AltTab
no tray
```

---

# 323. ToolWindow Recovery Safety

如果 tray runtime later fails：

window如果visible仍可用。

如果hidden且tray failure：

尽可能：

```
show main window
```

沿Phase8 policy。

---

# 324. Alt+F4

仍：

```
HideToTray
```

---

# 325. ToolWindow不改变 close semantics

---

# 326. Win+Tab

Windows Task View是否显示：

建议真实观察。

产品contract主要：

```
Taskbar + AltTab
```

如果 Win+Tab仍显示：

记录。

不自动block，除非 USER后续要求。

---

# 327. App Switcher Identity

不要继续堆特殊 Shell APIs试图隐藏所有系统UI。

范围：

```
taskbar
Alt+Tab
```

---

# 328. Phase 9 Release Artifacts

Phase10代码变更后：

旧：

```
ZIP hash
EXE hash
```

全部作废。

---

# 329. 不再引用 Phase9 candidate作为最终RC。

---

# 330. Phase10必须重新生成

```
local RC ZIP
EXE hash
SBOM
SHA256SUMS
```

---

# 331. Release Workflow

如果 Phase9已有：

不因 Phase10大改。

---

# 332. 但必须验证：

```
new Rust CLI paths
```

没有让 workflow失效。

---

# 333. package script

仍Single Source。

---

# 334. SBOM重新生成

因为artifact hash改变。

---

# 335. Attestation remote仍：

```
NOT TESTED
```

如果未push/tag。

---

# 336. Clean VM

如果原本Phase9仍NOT TESTED：

Phase10仍需要。

---

# 337. Manual Conditions

所有 Phase9未测项继续。

---

# 338. 不因为Phase10增加新manual项而遗漏旧项

---

# 339. Final Acceptance Matrix

最终必须覆盖：

```
AC-001..AC-030
+
Phase10追加 contract
```

---

# 340. Final RC Readiness Logic

只有：

```
all P0/P1 closed
all hard gates pass or USER waived
all mandatory manual acceptance pass or USER waived
```

才：

```
RC READY
```

---

# 341. Warm Startup

仍FAIL：

```
NOT RC READY
```

除非USER waiver。

---

# 342. Manual 32+项仍NOT TESTED

如果属于 mandatory：

仍：

```
NOT RC READY
```

除非USER waiver。

---

# 343. Agent不得因为产品现在“看起来完整”

写：

```
RC READY
```

---

# 344. Automated Commands

至少：

```
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

# 345. Rust Automation

运行实际CLI：

```
automated CI suite
Phase10 smoke
performance
runtime
package verification
readiness
```

---

# 346. PowerShell thin wrapper

例如：

```
tools/smoke/all.ps1 -Ci
```

仍可以运行。

必须证明：

```
does not duplicate gate logic
```

---

# 347. Forbidden Runtime Dependencies

```
cargo tree | rg \
"tauri|wry|webview|cef|chromium|tokio|async-std|wgpu|reqwest|hyper|rusqlite"
```

---

# 348. Unsafe

```
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
rg "\bunsafe\b" apps/stickymd-win/src
```

---

# 349. Core

仍：

```
unsafe = 0
Windows deps = 0
```

---

# 350. Render

仍：

```
unsafe = 0
Windows deps = 0
```

---

# 351. ToolWindow adapter unsafe

如果需要：

逐block SAFETY。

---

# 352. Plan Ref

所有新增production module遵守。

---

# 353. UX Manual Matrix

至少创建：

```
UX10-01 Ctrl+Insert
UX10-02 Shift+Delete
UX10-03 Shift+Insert text
UX10-04 Shift+Insert image
UX10-05 Zoom50
UX10-06 Zoom100
UX10-07 Zoom300
UX10-08 Ctrl+wheel
UX10-09 220x120 Source
UX10-10 220x120 Preview
UX10-11 220x120 Split
UX10-12 no taskbar
UX10-13 no Alt+Tab
UX10-14 Alt+Tab away from focused StickyMD
UX10-15 tray restore
UX10-16 sensor restore
UX10-17 second-instance restore
UX10-18 snap24
UX10-19 nearest edge
UX10-20 tie Top
UX10-21 tie Left over Right
UX10-22 no Bottom
UX10-23 opacity40
```

---

# 354. Manual Source/IME with ToolWindow

特别：

```
click StickyMD
→ focus
→ Pinyin
→ Alt+Tab away
```

必须工作。

---

# 355. Zoom+IME

150/300%：

candidate位置仍跟caret。

---

# 356. Zoom+DPI

不要混淆：

```
content zoom
device scale factor
```

---

# 357. Effective text pixel size

概念：

```
base_DIP
× content_zoom
× DPI
```

---

# 358. 不把 DPI写入 zoom config

---

# 359. DPI move

zoom保持百分比。

---

# 360. Font raster

正确rebuild。

---

# 361. Dock threshold与zoom无关

仍24 DIP。

---

# 362. Sensor与zoom无关

仍3 DIP。

---

# 363. Window minimum与zoom无关

仍220×120。

---

# 364. Zoom 300 at min window

允许极少可见文本。

不得自动resize window。

---

# 365. Zoom 50

不得让shell controls一起变小。

---

# 366. Phase10 Performance Final

重新输出：

```
cold p50/p95/max
warm p50/p95/max

Source PWS
Preview PWS
Split PWS
Hidden PWS

Idle CPU

input latency
preview latency

Zoom 50/100/300
```

---

# 367. Warm blocker分析

报告必须明确：

```
methodology before
methodology after
whether Phase9 measurement was valid
```

---

# 368. 不能为了PASS篡改 definition

---

# 369. Phase10 Packaging

重新生成 local candidate。

---

# 370. Filename

仍：

```
local RC
```

如果版本尚未USER批准。

---

# 371. 不Tag

---

# 372. 不Push

---

# 373. 不Release

---

# 374. Git Commit建议

可以：

```
docs: approve Phase 10 v1 interaction corrections

refactor(tools): consolidate automated smoke authority in Rust

feat(editor): add traditional clipboard shortcuts and content zoom

feat(shell): support compact window and tool-window identity

feat(shell): refine edge capture and opacity range

test(rc): requalify Phase 10 interaction contracts

perf(startup): correct and revalidate warm startup measurement

docs: record Phase 10 RC requalification
```

不强制数量。

---

# 375. Architecture Review

完成后逐项回答：

1. USER批准的10项是否全部实现？
    
2. 有没有额外feature creep？
    
3. Clipboard alias是否走同一Intent？
    
4. Shift+Insert图片是否和Ctrl+V相同？
    
5. Zoom是否单一全局authority？
    
6. Zoom是否污染Document generation？
    
7. Zoom是否触发Markdown reparse？
    
8. Math是否只做必要raster invalidation？
    
9. Image cache是否仍bounded？
    
10. Shell是否不缩放？
    
11. min size是否真220×120？
    
12. Split是否没有自动mode switch？
    
13. tool-window identity是否不进入Config？
    
14. Taskbar是否隐藏？
    
15. Alt+Tab是否隐藏？
    
16. 是否错误加入WS_EX_NOACTIVATE？
    
17. Tray是否在tool-window lifecycle前可靠？
    
18. Alt+Tab away是否修复？
    
19. Dock是否使用24 DIP？
    
20. Bottom是否仍不存在？
    
21. nearest-edge是否真正优先？
    
22. priority是否仅tie使用？
    
23. tie epsilon是否1 DIP？
    
24. release dock是否保持expanded？
    
25. opacity是否40–100？
    
26. 40%是否仍可正常input？
    
27. Rust CLI是否成为automation authority？
    
28. PowerShell是否仍有duplicated gate？
    
29. Manual acceptance是否仍诚实？
    
30. warm benchmark是否方法正确？
    
31. Phase9 RC artifacts是否已标记obsolete？
    
32. 新candidate是否重新package/hash/SBOM？
    
33. core/render unsafe是否仍0？
    
34. no WebView/Tokio/network是否保持？
    

---

# 376. Phase10 Report

创建：

```
docs/report/phase-10-rc-requalification.md
```

---

# 377. Report Executive

只能：

```
RC READY
RC READY WITH USER WAIVERS
NOT RC READY
```

---

# 378. Final Result格式

必须严格：

# Phase 10 Result

## Preconditions

```
Phase 9 result
USER approval
starting commit
```

## Contract Amendments

列10项：

```
APPROVED
IMPLEMENTED
```

## Automation Consolidation

```
Rust CLI:
PowerShell:
JSON evidence:
CI:
```

## Clipboard Shortcuts

```
Ctrl+Insert
Shift+Delete
Shift+Insert text
Shift+Insert image
```

## Content Zoom

```
range
default
keyboard
wheel
persistence
Source
Preview
Split
math
images
```

## Compact Window

```
old minimum
new minimum
Source
Preview
Split
```

## Tool Window Identity

```
taskbar
Alt+Tab
implementation
winit only / Win32 fallback
WS_EX_TOOLWINDOW
WS_EX_APPWINDOW
WS_EX_NOACTIVATE
```

## Window Reachability

```
visible click
sensor
tray
second instance
```

## Dock Capture

```
threshold = 24 DIP
tie epsilon = 1 DIP
priority
no Bottom
release behavior
```

## Opacity

```
40
70
96
100
```

## Startup Benchmark Audit

完整说明：

```
Phase9 method
identified issues
Phase10 method
cold samples
cold p95
warm samples
warm p95
```

## Warm Gate

```
<=180 ms = PASS / FAIL / USER WAIVED
```

## Performance

完整最终数据。

## Memory

完整最终数据。

## Idle CPU

完整。

## Manual Acceptance

区分：

```
MANUAL PASS
AUTOMATED PASS
AUTOMATED VISUAL PASS
NOT TESTED
FAIL
USER WAIVED
```

## Final Acceptance Matrix

```
AC-001..AC-030
+
Phase10 UX contracts
```

## Package

```
new EXE hash
new ZIP hash
size
SBOM
verification
```

## Phase9 Artifact Status

明确：

```
obsolete / superseded by Phase10 candidate
```

## Dependencies

运行时新增：

```
None
```

如果确实无。

## Unsafe

```
core = 0
render = 0
Windows adapter = ...
```

## Architecture Authority

确认：

```
DocumentState
WindowShellState
ConfigCoordinator
asset authority
automation authority
```

## Architecture Drift

```
None
```

或 Risk Report。

## Verification

所有command。

## Git

```
commits
push = no
tag = no
release = no
```

## USER Decisions Required

列：

```
remaining manual waivers
warm startup waiver if still failed
release version
tag decision
```

## Recommendation

只能：

```
RC READY
RC READY WITH USER WAIVERS
NOT RC READY
```

最后：

> Awaiting USER release decision. Do not push, tag, or create a GitHub Release automatically.

---

# 379. Phase 10 Definition of Done

只有全部满足才结束：

- Phase10 USER-approved contract文档落盘。
    
- plan先于implementation更新。
    
- Ctrl+Insert正式支持。
    
- Shift+Delete正式支持。
    
- Shift+Insert text正式支持。
    
- Shift+Insert image正式支持。
    
- shortcuts复用原Intent。
    
- Preview traditional shortcut语义正确。
    
- content zoom实现。
    
- zoom 50–300。
    
- default100。
    
- Ctrl++。
    
- Ctrl+=。
    
- Ctrl+Numpad+。
    
- Ctrl+-。
    
- Ctrl+Numpad-。
    
- Ctrl+0。
    
- Ctrl+wheel。
    
- wheel high-resolution accumulator。
    
- wheel config debounce。
    
- zoom持久化。
    
- Source zoom。
    
- Preview zoom。
    
- Split共享zoom。
    
- shell不zoom。
    
- math zoom正确。
    
- image zoom bounded。
    
- zoom不改Document generation。
    
- zoom不触发Markdown parse。
    
- minimum window改220×120。
    
- default仍520×680。
    
- Split在最小窗口仍Split。
    
- no automatic mode switch。
    
- controls在220宽可操作。
    
- old 240-per-pane hard limit移除。
    
- taskbar隐藏。
    
- Alt+Tab隐藏。
    
- Alt+Tab从focused StickyMD可切走。
    
- 点击StickyMD仍可focus。
    
- IME仍可用。
    
- WS_EX_NOACTIVATE未加入。
    
- winit skip-taskbar优先验证。
    
- Win32 fallback只在必要时。
    
- WS_EX_TOOLWINDOW fallback正确。
    
- WS_EX_APPWINDOW不冲突。
    
- tool identity在first show前配置。
    
- tray readiness gate。
    
- tray restore。
    
- sensor restore。
    
- second-instance restore。
    
- Dock threshold=24DIP。
    
- Tie epsilon=1DIP。
    
- Top>Left>Right仅tie。
    
- nearest eligible edge正常。
    
- Bottom不存在。
    
- WorkArea作为edge基准。
    
- mixed DPI按DIP计算。
    
- release capture→DockedExpanded。
    
- focus保持expanded。
    
- focus-loss700ms不变。
    
- manual/Esc行为不变。
    
- detach16DIP不变。
    
- opacity范围40–100。
    
- default96。
    
- opacity40 whole-window。
    
- opacity40 non-click-through。
    
- opacity100 layered cleanup不回归。
    
- Rust automation CLI成为主要authority。
    
- CLI支持machine-readable evidence。
    
- PowerShell降为thin wrapper。
    
- cargo test/fmt/clippy/deny不被无意义包裹。
    
- automated manual distinction保持。
    
- duplicated gate logic被移除。
    
- Phase9 warm benchmark methodology审计。
    
- readiness signal仍EDITOR_READY。
    
- stale readiness event不存在。
    
- previous process exit确认。
    
- cold≥30 samples。
    
- warm≥30 samples。
    
- Cold p95≤300或USER waiver。
    
- Warm p95≤180或USER waiver。
    
- 不通过删fallback优化startup。
    
- Phase9 artifact标记obsolete。
    
- Phase10 artifact重新生成。
    
- EXE hash重新生成。
    
- ZIP hash重新生成。
    
- SBOM重新生成。
    
- package verification重新运行。
    
- AC-001..030 regression完成。
    
- Phase10 acceptance完成。
    
- manual items保持诚实。
    
- final memory重新测。
    
- final idle CPU重新测。
    
- final performance重新测。
    
- zoom extremes resource测试。
    
- no production dependency creep。
    
- core unsafe=0。
    
- render unsafe=0。
    
- no WebView。
    
- no Tauri runtime。
    
- no Tokio。
    
- no DB。
    
- no runtime network。
    
- docs更新。
    
- coverage matrix更新。
    
- Phase10 task完成。
    
- Phase10 reports完成。
    
- fmt PASS。
    
- clippy PASS。
    
- workspace tests PASS。
    
- Release build PASS。
    
- cargo deny PASS。
    
- all smoke PASS到readiness gate。
    
- git diff --check PASS。
    
- 未push。
    
- 未tag。
    
- 未创建GitHub Release。
    

完成后立即停止。

