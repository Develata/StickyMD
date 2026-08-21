# StickyMD Phase 9 — Pre-Release Convergence, Manual Acceptance, Performance Hardening & Release Readiness

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–8 已完成主体工程实现。

当前 Phase 8 commit：

```text
318037fe9be4ddbb41785eb723e6ebea9b40c390
```

Phase 8 Recommendation：

```text
APPROVE Phase 9 WITH CONDITIONS
```

USER 已批准进入 Phase 9。

本阶段名称：

> **Phase 9 — Pre-Release Convergence, Manual Acceptance, Performance Hardening & Release Readiness**

---

# 0. Phase 9 本质

Phase 9 **不是产品开发阶段**。

Phase 9 是：

```text
Implementation Complete
        │
        ▼
Close Release Blockers
        │
        ▼
Manual Acceptance
        │
        ▼
Performance Hardening
        │
        ▼
Reliability / Failure Injection
        │
        ▼
Supply-chain / License Audit
        │
        ▼
Portable RC Packaging
        │
        ▼
Release Workflow
        │
        ▼
Release Readiness Decision
```

---

# 1. Phase 9 绝对禁止增加产品能力

禁止：

```text
New
Open
Recent Files
multiple notes
tabs
file tree
search
tags
settings page
global hotkeys
auto startup
plugins
AI
sync
network
auto updater
installer
MSI
MSIX
Store
syntax highlighting
PDF export
HTML export
```

---

# 2. 本阶段允许修改什么

只允许：

```text
bug fixes
correctness fixes
performance optimization
memory optimization
accessibility/basic UX fixes
visual defect fixes
test infrastructure
release infrastructure
packaging
documentation
license / SBOM / provenance
```

所有修改必须服务于：

> 已批准 v1 行为的正确实现和发布准备。

---

# 3. Feature Freeze

从 Phase 9 第一行代码开始：

```text
FEATURE FREEZE = ACTIVE
```

任何发现的“新功能想法”：

不得实现。

记录：

```text
docs/report/post-v1-ideas.md
```

或现有 future backlog。

---

# 4. Release Blocker 定义

建立四级：

```text
P0 — data loss / corruption / security / destructive file safety
P1 — frozen v1 capability broken / unusable / major crash
P2 — significant UX / performance defect but usable
P3 — cosmetic / minor
```

---

# 5. Release Rule

Stable release 前：

```text
P0 = 0
P1 = 0
```

P2：

可以存在，但必须：

- documented；
- USER aware；
- 不违反明确 hard gate。

---

# 6. USER Waiver

只有 USER 可以豁免：

```text
hard performance gate
mandatory manual acceptance
known P1-like environment limitation
```

Agent 无权：

```text
“看起来差不多，所以 PASS”
```

---

# 7. 当前已知硬问题

Phase 8：

```text
Cold Startup hard gate:
target ≤300 ms
observed median ≈393.911 ms
max ≈1429.905 ms
result = CONDITIONAL
```

这是 Phase 9 第一 release blocker。

---

# 8. 当前继承的 NOT TESTED

至少：

```text
Microsoft Pinyin
WeChat Input Method

Preview visual
RaTeX visual
image visual

Light/Dark/System real visual
whole-window opacity real visual

Explorer tray
Left/Right/Top Dock physical behavior
sensor hover/no-focus

125/150/200% real DPI

dual monitor
mixed DPI
monitor disconnect
sleep/resume
RDP if available

real clipboard sources
native export dialog

hard-kill recovery
real junction / symlink safety
```

必须从各 Phase report 重新汇总。

---

# 9. 不允许漏掉 inherited condition

创建统一表：

```text
docs/report/phase-09-inherited-conditions.md
```

列：

```text
origin phase
test
current status
release importance
environment needed
final result
USER waiver if any
```

---

# 10. 状态 vocabulary

Phase 9 必须统一只使用：

```text
AUTOMATED PASS
AUTOMATED VISUAL PASS
MANUAL PASS
NOT TESTED
FAIL
USER WAIVED
```

---

# 11. AUTOMATED VISUAL PASS

仅当：

- 实际运行 GUI；
- 自动化截图；
- 自动/AI视觉检查；

可以写：

```text
AUTOMATED VISUAL PASS
```

不能写：

```text
MANUAL PASS
```

---

# 12. MANUAL PASS

只有实际：

> 人或有真实 GUI 交互/观察能力的执行者，对运行中的 Windows 应用进行了明确观察。

才能写。

---

# 13. NOT TESTED

不是失败。

但：

> release-blocking manual case 仍为 NOT TESTED 时，不得宣称 stable-release-ready。

除非 USER WAIVED。

---

# 14. 开始前必须读取

严格执行：

```text
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md
```

以及全部：

```text
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

---

# 15. 读取所有 Phase Final Reports

至少：

```text
phase-01
phase-02
phase-03
phase-04
phase-05
phase-06
phase-07
phase-08
```

不要只读 Phase 8。

---

# 16. 读取所有 Risk Reports

至少搜索：

```bash
find docs/report -type f | sort
```

或 PowerShell 等价。

特别：

```text
RISK-source-font-startup.md
```

必须完整阅读。

---

# 17. Repository Preflight

执行：

```bash
git status --short
git branch --show-current
git log -15 --oneline

cargo metadata --no-deps

cargo tree -p stickymd-core
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```text
starting commit
branch
clean/dirty
```

不得 reset / clean / rebase USER 工作。

---

# 18. Phase 9 分段

必须按顺序：

```text
9A — Release Blocker Inventory
9B — Cold Startup Hardening
9C — Manual Acceptance Closure
9D — Reliability / Failure Convergence
9E — Final Performance & Memory Budget
9F — Dependency / License / Security Audit
9G — Portable RC Packaging
9H — GitHub Release Infrastructure
9I — Clean-environment RC Validation
9J — Final Release Readiness Review
```

不要顺序颠倒。

---

# 19. Phase 9A — Release Blocker Inventory

创建：

```text
docs/report/phase-09-release-blockers.md
```

---

# 20. 汇总所有未关闭项

来源：

```text
Phase reports
Risk reports
acceptance matrices
TODO/FIXME
ignored tests
cargo deny warnings
smoke exclusions
```

---

# 21. 搜索

至少：

```bash
rg "NOT TESTED|CONDITIONAL|FAIL|TODO|FIXME|HACK|TEMP|PENDING" \
  docs apps crates tools .github
```

---

# 22. 不把所有 TODO 都当 blocker

分类：

```text
v1 blocker
post-v1
test infra
comment only
```

---

# 23. ignored Rust tests

执行：

```bash
cargo test --workspace -- --list
```

确定所有：

```text
ignored
```

为什么被 ignored。

---

# 24. ignored performance tests

合理。

但必须在 Phase 9：

```text
显式运行对应 Release performance suite
```

---

# 25. 不允许 ignored correctness test

如果有：

```text
correctness test ignored
```

必须修复或报告 blocker。

---

# 26. Release Blocker表

至少：

| ID | Severity | Source | Description | Status | Gate |
|---|---|---|---|---|---|

---

# 27. Phase 9B — Cold Startup Hardening

这是本阶段最高优先工程任务。

---

# 28. 不允许先重构字体系统

先建立 measurement。

---

# 29. Startup 定义

必须明确：

> “startup complete / editable” 的时间点是什么。

冻结为：

```text
process startup
→ main runtime init
→ canonical note loaded/recovered
→ window visible
→ source editor projection ready
→ input/IME enabled
```

最后一个时点：

```text
EDITOR_READY
```

---

# 30. 不能用：

```text
process alive
window handle exists
tray created
```

冒充 startup complete。

---

# 31. Startup Instrumentation

为 Debug/benchmark build建立 milestone：

```text
main_enter
program_dir_ready
single_instance_ready
persistence_ready
document_ready
window_created
monitor_ready
font_system_begin
font_system_end
source_projection_ready
tray_ready
window_visible
editor_ready
```

---

# 32. 不记录用户文本

---

# 33. Release instrumentation

正常用户 Release build：

不得永久输出冗余 startup log。

使用：

```text
diagnostic env flag
```

或现有 smoke diagnostics。

---

# 34. 推荐 readiness signal

例如测试环境变量：

```text
STICKYMD_DIAGNOSTIC_READY_EVENT
```

存在时：

app 在 `EDITOR_READY` 时 signal测试对象。

不改变普通用户行为。

---

# 35. 不新增用户 CLI surface

避免：

```text
--benchmark-mode
```

变成公开长期 API。

环境变量/测试feature更合适。

---

# 36. 外部测量

应尽量：

```text
CreateProcess
→ EDITOR_READY signal
```

而非只测 `Instant::now()` from main。

---

# 37. Internal + External都记录

方便知道：

```text
process creation overhead
app initialization overhead
```

---

# 38. Cold Process Method

若现有 plan未定义更严格方式：

采用：

```text
standalone Release EXE
no existing StickyMD process
same writable portable directory
10+ seconds idle before run
no debugger
20 launches minimum
```

不要人为清 Windows filesystem cache。

---

# 39. Warm Process Method

```text
同一环境
第一轮之后
20 launches
短间隔
```

---

# 40. First-ever / Post-reboot

单独记录：

```text
FIRST_RUN
POST_REBOOT
```

如果环境可测试。

不要混入 normal cold p95。

---

# 41. Cold Gate

冻结：

```text
p95 <= 300 ms
```

---

# 42. Warm Gate

冻结：

```text
p95 <= 180 ms
```

---

# 43. Sample Count

至少：

```text
20 cold
20 warm
```

建议：

```text
30+
```

---

# 44. 不允许用 5 samples 宣布 p95 PASS

---

# 45. Phase8首样本 1429ms

必须分析：

```text
first-run
Defender
font cache
filesystem cache
other
```

不能直接删掉当 outlier。

---

# 46. FontSystem 是优先怀疑对象

当前 cosmic-text 官方说明：

```text
FontSystem::new()
```

即使 Release 都可能耗时接近：

```text
1 second
```

所以必须实际测量：

```text
font_system_begin → font_system_end
```

---

# 47. 首先检查调用次数

整个 app lifetime：

```text
FontSystem construction count
```

应尽可能：

```text
1
```

或已有明确分离且有理由。

---

# 48. Source + Preview重复 FontSystem

如果存在：

测：

```text
startup duplicates?
preview lazy?
```

---

# 49. Preview FontSystem

如果 Phase5已经 lazy：

Source-only startup不得初始化 Preview-only FontSystem。

---

# 50. Math Font

Phase6已经 lazy。

继续验证。

---

# 51. Image decoder

不得 startup init。

---

# 52. Tray

测初始化时间。

不要凭猜测把 tray移出startup。

---

# 53. DisplayTopology

测。

---

# 54. Persistence

测。

---

# 55. Cold Startup优化顺序

严格：

### Step 1

消除重复工作。

### Step 2

延后非 editor-ready 必需工作。

### Step 3

减少字体初始化成本。

### Step 4

最后才考虑更深平台优化。

---

# 56. 禁止为了 gate

删除：

```text
CJK fallback
emoji fallback
Unicode correctness
IME correctness
Times New Roman requirement
仿宋 preference
```

---

# 57. 禁止 bundle

```text
Times New Roman
仿宋_GB2312
Microsoft proprietary fonts
```

---

# 58. Font optimization调查

检查当前：

```text
cosmic_text::FontSystem::new()
```

是否直接：

```text
fontdb.load_system_fonts()
```

扫描：

```text
Windows Fonts
user Fonts
```

---

# 59. 可研究但不得盲做

```text
FontSystem::new_with_locale_and_db(...)
FontSystem::new_with_locale_and_db_and_fallback(...)
```

---

# 60. Narrow DB 方案

只有在能证明：

```text
preferred fonts
CJK fallback
Latin fallback
emoji
other-script fallback
```

行为不退化时才允许。

---

# 61. 不允许：

```text
只加载 Times New Roman + FangSong
```

然后声称 universal fallback仍支持。

---

# 62. Lazy system font discovery

可以研究：

```text
initial minimum font set
+
background system fallback discovery
```

但如果采用：

必须回答：

```text
已有note含unsupported script怎么办？
首次绘制是否tofu？
输入fallback完成前怎么办？
IME候选是否受影响？
线程安全？
memory？
```

---

# 63. 任何 temporary rendering degradation

属于行为变化。

必须：

```text
Risk Report
```

并交 USER 批准，除非可以证明用户看不到 degradation。

---

# 64. 不引入第二 text renderer

禁止仅为了startup：

```text
GDI temporary editor
DirectWrite temporary editor
RichEdit temporary editor
```

---

# 65. 不引入新大型 font framework

例如：

```text
font-kit
DirectWrite wrapper stack
```

除非 measurement证明现有路径无法满足且 USER批准。

---

# 66. Existing fontdb optimization

优先研究：

```text
feature flags
font directory scanning
duplicate scans
cached face construction
unneeded monospace collection
```

---

# 67. cosmic-text feature audit

检查实际 enabled features：

```bash
cargo tree -e features -p stickymd-win
```

---

# 68. 不需要功能不要启用

尤其检查：

```text
syntect
vi
```

不得存在。

---

# 69. fontconfig feature

在 Windows 是否造成无用成本：

以实际 dependency/features分析。

不要盲关影响 fallback。

---

# 70. Startup task reorder

只有不影响 correctness时可以：

```text
first editable frame
→ delayed noncritical work
```

---

# 71. 可能延后的对象

逐项测后决定：

```text
non-visible preview caches
nonessential diagnostics
release-only metadata checks
```

---

# 72. Tray不能造成 hide-to-nowhere

如果 tray延后：

在 tray READY 前：

```text
Close-to-Tray
```

必须安全。

最简单：

> tray仍在 show window 前 ready。

除非 tray明显是瓶颈。

---

# 73. Monitor placement不能延后到窗口显示之后

避免闪屏。

---

# 74. Config不能延后

---

# 75. note load不能延后

---

# 76. Startup optimization必须每步有

```text
before
patch
after
regression tests
```

---

# 77. 一次只改一个主要因素

避免无法归因。

---

# 78. Startup Report

创建：

```text
docs/report/phase-09-startup-hardening.md
```

---

# 79. Report表

| Milestone | Before p50 | Before p95 | After p50 | After p95 |
|---|---:|---:|---:|---:|

---

# 80. Gate Result

只能：

```text
PASS
FAIL
USER WAIVED
```

不能：

```text
close enough
```

---

# 81. 如果无法达到 300ms

在进行合理优化后：

停止进一步架构污染。

创建：

```text
docs/report/phase-09-startup-gate-review.md
```

---

# 82. Gate Review必须回答

```text
remaining bottleneck
best achievable value
cost of further optimization
correctness risks
memory effects
dependency effects
maintenance effects
recommended USER disposition
```

---

# 83. Agent无权修改目标到400ms

---

# 84. Phase 9C — Manual Acceptance Closure

建立：

```text
docs/acceptance-cases/phase-09.md
docs/report/phase-09-manual-acceptance.md
```

---

# 85. Manual test environment

每次记录：

```text
Windows edition
Windows build
CPU
RAM
GPU
monitor setup
DPI
input method/version
commit SHA
Release EXE SHA-256
```

---

# 86. 测试必须使用

```text
Release standalone portable EXE
```

不是：

```text
cargo run
```

---

# 87. 每次 manual matrix用同一 RC artifact

避免边测边改。

如果代码修复：

生成新的 RC：

```text
RC iteration N+1
```

重新执行受影响项目。

---

# 88. Manual IME — Microsoft Pinyin

Stable release blocker。

必须真实测试：

```text
Floating Source
Docked Source
Split Source
Opacity 70
Opacity 96
125/150/200 DPI if available
focus loss/regain
hover reveal then click
undo commit
selection replacement
```

---

# 89. 关键检查

```text
candidate position
no duplicate characters
no phantom commit
composition underline
Esc behavior
collapse guard
Ctrl+Z atomic commit
```

---

# 90. Microsoft Pinyin结果

必须：

```text
MANUAL PASS
FAIL
NOT TESTED
```

---

# 91. WeChat Input Method

同等 release requirement。

不能因为 Pinyin PASS跳过。

---

# 92. 若测试环境没有 WeChat

写：

```text
NOT TESTED
```

Stable release仍 blocked，除非 USER WAIVED。

---

# 93. Preview Visual

至少：

```text
paragraph
heading
bold/italic
CJK/Latin mixed
list
quote
code
table
raw HTML literal
image
math
```

---

# 94. Math Visual

至少：

```text
inline baseline
sup/sub
fraction
nested fraction
sqrt
sum/integral
large delimiter
matrix
cases
CJK mixed
display center
malformed error
```

---

# 95. Light/Dark

上述代表页面分别：

```text
Light
Dark
```

---

# 96. System Theme

真实切 Windows theme：

```text
Light → Dark → Light
```

确认 StickyMD跟随。

---

# 97. Whole-window Opacity

真实：

```text
70
85
96
100
```

检查：

```text
text
math
image
controls
IME
```

全部一起透明。

---

# 98. 100%

确认无奇怪 layered rendering回归。

---

# 99. Dock

真实：

```text
Left
Right
Top
```

---

# 100. 每 edge

至少：

```text
snap
expanded
focus-loss collapse
manual collapse
Esc
sensor
hover delay
hover no-focus
click-focus
detach
resize
restart
```

---

# 101. Foreground no-steal

测试：

```text
Notepad active
StickyMD collapsed
hover sensor
```

Notepad必须仍为 foreground。

---

# 102. Tray

真实 Explorer notification area：

```text
icon visible
menu exactly 3 logical items
显示/隐藏
置顶
退出
```

---

# 103. Close-to-Tray

```text
close button
Alt+F4
taskbar Close
```

不得正常退出process。

---

# 104. Quit

Tray退出：

必须真正退出且安全保存。

---

# 105. Native Export Dialog

真实调用。

检查：

```text
Chinese path
space path
overwrite confirmation
cancel
```

---

# 106. Clipboard Sources

至少真实：

```text
File Explorer PNG
File Explorer JPEG
Snipping Tool screenshot
Paint
browser copied image
```

---

# 107. Clipboard 每项验证

```text
selected clipboard format
final asset format
Markdown insertion
Preview
Undo
Redo
```

---

# 108. 用户文件安全

手工创建：

```text
note/images/USER-DO-NOT-DELETE.png
```

经过：

```text
restart
edit
undo
redo
GC
export
quit
```

必须仍存在。

---

# 109. Managed-looking fake file

创建名字像：

```text
stickymd-<hashlike>.png
```

但 content hash不匹配。

必须仍存在。

---

# 110. Junction/Symlink

真实环境可用时：

验证 destructive asset path：

```text
不追随到note目录外
```

---

# 111. Crash Kill

真实：

```powershell
Stop-Process -Id <pid> -Force
```

在不同时间点：

```text
typing before autosave
asset paste
note temp
```

重开验证：

```text
canonical intact
recovery available
asset reconcile safe
```

---

# 112. Multi-monitor

Stable release核心能力。

至少真实：

```text
2 monitors
```

---

# 113. Real multi-monitor matrix

```text
same DPI
mixed DPI
secondary right
secondary left if possible
secondary above if possible
dock on secondary
restart
```

---

# 114. Monitor Disconnect

运行中：

```text
disconnect secondary
```

窗口必须恢复 primary。

---

# 115. Mixed DPI

至少一个真实：

```text
100% + 150%
```

或其它不同 scale。

---

# 116. Candidate rect

拖到另一DPI显示器后测试 IME。

---

# 117. 125/150/200%

如果物理/虚拟环境支持：

真实测试。

---

# 118. Sleep Resume

如果环境允许：

```text
sleep
resume
```

---

# 119. RDP

如果环境允许：

```text
connect/reconnect
```

---

# 120. 不可用环境

保持：

```text
NOT TESTED
```

不能模拟成 MANUAL PASS。

---

# 121. Clean VM

Stable release hard acceptance之一。

必须测试一个：

```text
clean Windows 11 VM
```

---

# 122. Clean VM不得预装开发环境

不应依赖：

```text
Rust
Visual Studio
Git
WebView runtime特定版本
额外字体包
```

除 Windows系统本身。

---

# 123. Clean VM 检查

```text
EXE launches
note created
source typing
preview
math
image paste
tray
dock
quit
```

---

# 124. Portable原则

无需管理员权限。

---

# 125. Clean VM不可用

```text
NOT TESTED
```

Stable release blocked，除非 USER WAIVED。

---

# 126. Phase 9D — Reliability / Failure Convergence

重复执行关键数据安全流程。

---

# 127. Atomic Save

覆盖：

```text
temp create fail
write fail
flush fail
replace fail
recovery
```

---

# 128. OCC conflict

确保：

```text
external write between autosave scheduling and actual write
```

不会被 silent overwrite。

---

# 129. Watcher unavailable

模拟：

```text
watcher init failure
```

guarded save仍防 silent overwrite。

---

# 130. Config corruption

再验证。

---

# 131. note.md invalid UTF-8

再验证。

---

# 132. note.md read-only

编辑内容不能丢。

---

# 133. Program directory read-only

启动拒绝且消息正确。

---

# 134. note/ deleted during runtime

恢复行为符合 contract。

---

# 135. note/ replaced by file

fail safe。

---

# 136. Disk full

通过 fault injection。

---

# 137. Save failure + Quit

必须取消quit / restore UI。

---

# 138. Save failure + HideToTray

必须保持可见。

---

# 139. Conflict + Tray Quit

必须按明确 lifecycle。

---

# 140. Recovery + Assets

startup顺序正确。

---

# 141. Asset GC

final note save失败：

绝不 destructive GC。

---

# 142. Hardlink Export protection

Phase7已实现。

重新测试：

```text
canonical note.md hardlink alias
```

不能 export overwrite。

---

# 143. Export snapshot

用户导出中继续编辑：

export对应 captured generation。

---

# 144. Remote Image

全程无网络。

---

# 145. Raw HTML

全程无执行。

---

# 146. Custom URI

不能执行。

---

# 147. Malformed Math

不影响Document/Preview整体。

---

# 148. Malicious Image

至少：

```text
corrupt
huge dimensions header
pixel overflow
```

安全失败。

---

# 149. 4K BMP known peak

Phase7：

```text
~93.93 MiB transient
```

Phase9必须做一次有数据的优化评审。

---

# 150. 不要求自写 BMP decoder

先分析：

```text
encoded copy
decoder allocation
RGBA staging
resize staging
cache insertion
```

---

# 151. 可以优化

例如：

```text
earlier release of source buffer
avoid duplicate RGBA clone
resize ownership move
decode buffer reuse
```

仅在成熟 API 支持时。

---

# 152. 不为了 peak

引入：

```text
unsafe decoder
custom bitmap codec
GPU image pipeline
```

---

# 153. 4K peak 结果

如果无法合理降低：

记录：

```text
Known transient memory characteristic
```

只要：

- stable memory仍通过；
- safety guards有效；
- 不导致 OOM/crash。

不自动 blocker。

---

# 154. Phase 9E — Final Performance Budget

必须在最终 RC commit 上完整测量。

---

# 155. 不能引用 Phase 6/7/8旧数据作为最终结果

可以比较，但必须重新测。

---

# 156. 环境记录

```text
Windows build
CPU
RAM
GPU
monitor/DPI
filesystem
Defender state
commit
EXE hash
```

---

# 157. Startup

按 Phase9B最终方法。

---

# 158. Source Memory

20KiB：

```text
target <=28 MiB
hard <=40 MiB
```

---

# 159. Preview Memory

20KiB +20 formulas：

```text
target <=40 MiB
hard <=52 MiB
```

---

# 160. Split

```text
target <=48 MiB
hard <=64 MiB
```

---

# 161. Hidden

cache purge：

```text
target <=24 MiB
hard <=36 MiB
```

---

# 162. Idle CPU

60秒平均：

```text
target <=0.05%
hard <=0.1%
```

---

# 163. Startup

```text
cold p95 <=300ms
warm p95 <=180ms
```

---

# 164. Input latency

```text
20KiB p95 <=16ms
100KiB p95 <=25ms
1MiB p95 <=50ms
```

---

# 165. Preview

```text
20KiB <=100ms hard
100KiB <=400ms hard
1MiB <=2s hard background
```

---

# 166. Binary

EXE current约：

```text
7.89 MiB
```

portable ZIP：

```text
hard <=30 MiB
```

---

# 167. Typical test fixtures必须冻结

保存到：

```text
tests/fixtures/performance/
```

或现有 fixture位置。

---

# 168. 防止 benchmark漂移

报告fixture SHA/size。

---

# 169. Memory至少 5 runs

median/max。

---

# 170. Performance至少：

```text
median
p95
max
```

---

# 171. Leak stress

最终：

```text
1000 dock cycles
100 tray cycles
100 theme
100 opacity
100 autosaves
100 external reload
100 conflicts
100 image decode cycles
```

根据现有 smoke能力。

---

# 172. Private Bytes不能线性增长

---

# 173. Handles/GDI/USER objects

如果工具支持：

记录 before/after。

---

# 174. Phase9 Performance Report

创建：

```text
docs/report/phase-09-performance-final.md
```

---

# 175. Phase 9F — Dependency / Security / License Audit

从：

```text
Cargo.lock
```

冻结 release dependency graph。

---

# 176. 执行

```bash
cargo tree --workspace
cargo tree -d
cargo deny check
```

---

# 177. Duplicate Versions

现有 cargo deny duplicate warnings：

逐项分类：

```text
unavoidable upstream
can converge safely
should not touch before release
```

---

# 178. 不为了“零 duplicate”盲目升级依赖

release前稳定优先。

---

# 179. Cargo update

不得：

```bash
cargo update
```

无目的更新整个 lockfile。

---

# 180. Security Advisory

不得有已知 unresolved：

```text
security advisory
```

除非 USER明确接受且有非常充分理由。

---

# 181. Licenses

验证：

```text
MIT
BSD-2-Clause Comrak
RaTeX MIT
KaTeX fonts OFL-1.1
all transitive dependencies
```

---

# 182. THIRD_PARTY_NOTICES

必须与真实release dependency一致。

---

# 183. 不包含未使用依赖 notice

可以保留必要 attribution。

---

# 184. 不遗漏字体 license

特别：

```text
KaTeX fonts != MIT
```

---

# 185. Proprietary system fonts

不得打包：

```text
Times New Roman
仿宋_GB2312
FangSong
Consolas
```

---

# 186. Release ZIP检查

不得包含任何 Microsoft font file。

---

# 187. SBOM

正式产出：

```text
SBOM.spdx.json
```

---

# 188. Cargo 1.97 注意

不要依赖：

```text
cargo -Z sbom
```

作为 stable release requirement。

它仍是不稳定功能。

---

# 189. SBOM Tool

优先成熟独立工具：

```text
Syft
```

或已有经过审核等价工具。

---

# 190. Syft 使用规则

必须：

```text
pin exact version
verify upstream checksum/signature
document license
```

---

# 191. 当前Prompt基线

当前 Syft稳定线约：

```text
1.50.x
```

但 Agent必须重新查询实现时最新安全版本。

---

# 192. 不使用：

```bash
curl ... | sh
```

release workflow。

---

# 193. 下载工具

必须：

```text
known release URL
expected checksum
verify
execute
```

---

# 194. SBOM应覆盖

```text
final application
Cargo dependencies
packaged third-party assets/fonts
```

---

# 195. 如果 Syft 扫 binary不能完整识别 Rust deps

允许：

```text
scan repository/Cargo.lock context
+
final staging
```

并在 report说明生成方法。

---

# 196. 不自己手写 SPDX generator

---

# 197. Security Report

创建：

```text
docs/report/phase-09-supply-chain.md
```

---

# 198. No Network Runtime

再次审计：

```bash
cargo tree | rg "reqwest|hyper|ureq|curl"
```

---

# 199. Static search

```text
TcpStream
UdpSocket
WinHTTP
WinINet
```

所有命中审查。

---

# 200. Runtime network smoke

如果有能力：

运行 StickyMD：

```text
normal note
remote image
http link not clicked
```

观察无 outbound connection。

---

# 201. 点击用户 http link

当然可打开系统浏览器。

这是 explicit user action，不是 StickyMD network client。

---

# 202. Defender

如果 Windows Defender可用：

扫描：

```text
StickyMD.exe
portable ZIP
```

记录。

---

# 203. SmartScreen

无 code signing时可能出现 reputation warning。

不得写：

```text
false positive
```

除非有证据。

README可说明：

> unsigned open-source build may trigger reputation warnings.

---

# 204. Code Signing

Phase9不创建自签名证书。

---

# 205. 如果 USER没有真实代码签名证书

Release：

```text
unsigned
```

这是允许的。

---

# 206. 不阻塞 v1

除非 USER明确要求 signed release。

---

# 207. Phase 9G — Portable RC Packaging

建立正式：

```text
tools/release/package.ps1
tools/release/verify-package.ps1
tools/release/generate-sbom.ps1
```

或现有 tools结构的等价实现。

---

# 208. package.ps1 输入

应基于：

```text
exact built release EXE
workspace version
commit SHA
```

---

# 209. 不自动 cargo build inside package script unless明确

推荐：

```text
build step
→ verify
→ package step
```

职责分开。

---

# 210. Stable Version

不要自行把版本改：

```text
1.0.0
```

除非 USER已经批准。

---

# 211. Workspace version

读取真实：

```text
Cargo.toml
```

---

# 212. Phase9 local RC filename

在没有public release tag时：

```text
StickyMD-<version>-local-rc-<shortsha>-windows-x64-portable.zip
```

---

# 213. 不能使用假stable filename

例如：

```text
StickyMD-v1.0.0.zip
```

如果 USER尚未批准 v1.0.0 tag。

---

# 214. Final tagged workflow

将来 tag：

```text
vX.Y.Z
```

必须与 Cargo version完全匹配。

---

# 215. Staging structure

严格：

```text
StickyMD/
├─ StickyMD.exe
├─ README.txt
├─ LICENSE.txt
├─ THIRD_PARTY_NOTICES.txt
└─ licenses/
   ├─ SIL-OFL-1.1.txt
   └─ KaTeX-fonts-NOTICE.txt
```

可有其它真正需要的 license notice。

---

# 216. 不能包含

```text
note/
config.toml
images/
.trash/
user data
Cargo.lock
source files
PDB
test files
```

在主 portable ZIP。

---

# 217. README.txt

简洁说明：

```text
what StickyMD is
Windows 11 x64
portable usage
data location
Close-to-Tray
real Quit
Markdown/math profile
remote images not downloaded
license
project URL
```

---

# 218. First run

用户解压后：

```text
StickyMD.exe
```

自行创建 note。

---

# 219. ZIP layout

解压后不应该：

```text
StickyMD-vX/StickyMD/StickyMD.exe
```

多一层意外嵌套。

---

# 220. 建议用户解压目录本身就是 app目录

---

# 221. ZIP reproducibility

至少保证：

```text
stable file ordering
known included file list
```

---

# 222. 如果容易

规范化 ZIP entry timestamp。

---

# 223. 不为 bit-reproducible ZIP引入巨大工具链

---

# 224. Build reproducibility audit

从同一 commit：

```text
clean target A
clean target B
```

构建两次。

比较：

```text
StickyMD.exe SHA-256
```

---

# 225. 结果

```text
IDENTICAL
NON-DETERMINISTIC
```

---

# 226. NON-DETERMINISTIC

不是自动 release blocker。

但 report：

```text
likely reason
scope
```

---

# 227. 不伪造 reproducible claim

---

# 228. SHA256SUMS

正式生成：

```text
StickyMD-<version>-SHA256SUMS.txt
```

至少包含：

```text
portable zip
SBOM
symbols archive if published
```

---

# 229. EXE hash

也可以包含。

---

# 230. Checksum format

标准：

```text
<64hex>  filename
```

---

# 231. Symbols

早期规格希望：

```text
symbols.zip
```

但 Phase9必须验证它是否真的对应 released EXE。

---

# 232. 不发布错误 PDB

如果无法生成与最终 EXE精确对应的 symbol artifact：

宁可：

```text
symbols artifact omitted
```

并 report。

---

# 233. 不创建另一个 codegen build的PDB冒充release symbols

---

# 234. 如果可生成 exact matching PDB

发布：

```text
StickyMD-<version>-symbols.zip
```

不放主 portable ZIP。

---

# 235. PE Resources

最终 EXE应检查：

```text
application icon
manifest
PerMonitorV2
asInvoker
product name
file description
version
copyright
```

---

# 236. Version resource

应与 Cargo version一致。

---

# 237. 不引入 heavyweight runtime dependency

build-time resource embedding可以。

---

# 238. 如果已有 build.rs/resource pipeline

优先复用。

---

# 239. Icon ownership

只使用项目原创/已有项目icon。

---

# 240. 不下载第三方 icon。

---

# 241. Verify Package Script

必须检查：

```text
exact file allowlist
no note/
no temp files
no PDB in main ZIP
no proprietary font
manifest
EXE architecture x64
version
hashes
license files
```

---

# 242. ZIP path traversal

verify entry names：

不得有：

```text
../
absolute path
```

---

# 243. ZIP解压 smoke

解压到：

```text
path with spaces
Chinese path
```

运行。

---

# 244. Example

```text
C:\Temp\我的 Markdown 便签\
```

---

# 245. 读写

首次创建 note。

---

# 246. Program Files test

复制到无写权限位置/模拟。

必须正确拒绝。

---

# 247. Phase 9H — GitHub Release Infrastructure

创建：

```text
.github/workflows/release.yml
```

---

# 248. 只创建 workflow

Phase9 本地任务：

```text
不得 push tag
不得发布 GitHub Release
```

---

# 249. Release Workflow trigger

推荐：

```yaml
on:
  push:
    tags:
      - "v*"
```

可加：

```text
workflow_dispatch
```

仅用于 dry-run/package验证时必须防止意外正式 release。

---

# 250. 最安全方案

正式 release creation只对：

```text
tag push
```

发生。

---

# 251. workflow_dispatch

若提供：

只能：

```text
build/package validation
```

不得自动发布正式Release。

---

# 252. Version Validation

Tag：

```text
vX.Y.Z
```

必须：

```text
X.Y.Z == workspace package version
```

否则 fail。

---

# 253. Pre-release tag

如果未来：

```text
v1.0.0-rc.1
```

Cargo version也必须对应。

---

# 254. Release commit

构建 exact tagged commit。

---

# 255. Branch provenance

Stable release tag应能证明来自：

```text
main
```

或 USER批准 release branch。

---

# 256. Release workflow permissions

默认最小：

```yaml
contents: read
```

---

# 257. 需要创建 draft release时：

对应 job：

```yaml
contents: write
```

---

# 258. Artifact attestation

需要：

```yaml
attestations: write
id-token: write
```

---

# 259. 不给：

```text
packages: write
actions: write
security-events: write
```

如果不用。

---

# 260. Pull Request CI永远不拥有 release write permissions

---

# 261. 不使用 pull_request_target 构建不可信代码并写Release

hard security rule。

---

# 262. GitHub Actions version pinning

所有 Actions：

```text
pin full immutable commit SHA
```

并注释：

```text
# vX.Y.Z
```

---

# 263. 不只：

```yaml
uses: actions/checkout@v7
```

正式 release workflow。

---

# 264. 实现前重新确认当前 release

当前 Prompt 时：

```text
actions/checkout latest major = 7
actions/upload-artifact latest major = 7
actions/attest current major = 4
```

但 Agent必须重新核实。

---

# 265. 不盲复制 Prompt版本

---

# 266. Checkout安全

使用当前安全release。

不要：

```text
persist-credentials: true
```

如果后续不需要git push。

---

# 267. Release job需要 gh auth

GitHub CLI可以使用：

```text
GH_TOKEN = github.token
```

---

# 268. 不需要checkout Git credentials留下

---

# 269. Rust Toolchain

使用：

```text
rust-toolchain.toml
```

作为真相源。

---

# 270. Workflow不得：

```text
cargo update
```

---

# 271. 构建：

```bash
cargo build --workspace --release --locked
```

---

# 272. 在 release 前仍运行：

```text
fmt
clippy
tests
cargo deny
full smoke CI-safe subset
```

---

# 273. GUI manual tests当然不能在 GitHub runner自动冒充。

---

# 274. Package

workflow使用同一：

```text
tools/release/package.ps1
```

---

# 275. 不在 YAML复制另一套 packaging规则

Single Source of Truth。

---

# 276. Verify

同一：

```text
verify-package.ps1
```

---

# 277. SBOM

workflow使用：

```text
generate-sbom.ps1
```

---

# 278. Syft

如果使用：

workflow下载 exact pinned version。

---

# 279. 必须verify checksum

---

# 280. 不：

```text
Invoke-WebRequest install.ps1 | iex
```

---

# 281. Attestation

当前新 workflow优先：

```text
actions/attest
```

而不是旧：

```text
attest-build-provenance
```

wrapper。

---

# 282. Attest对象

至少：

```text
portable ZIP
```

---

# 283. 可以同时attest：

```text
SBOM
checksum manifest
```

根据 action当前 API。

---

# 284. SBOM Attestation

如果 actions/attest支持当前 SBOM input：

生成关联 attestation。

---

# 285. 不硬猜 action schema

实现时阅读 current official docs。

---

# 286. Public Repo provenance

GitHub公开仓库可使用 Sigstore transparency/provenance。

---

# 287. Release Artifact

必须先构建/验证，再attest。

---

# 288. Draft Release

workflow可以创建：

```text
draft GitHub Release
```

---

# 289. 不自动 publish stable

USER最终查看：

```text
manual acceptance
release report
artifact
```

后再Publish。

---

# 290. Release creation

优先使用：

```text
gh release create --draft
gh release upload
```

避免不必要 third-party release action。

---

# 291. Release notes

来源：

```text
CHANGELOG.md
```

或 dedicated release notes。

---

# 292. 不自动生成夸大功能的 AI release notes

---

# 293. Release Asset

至少：

```text
portable ZIP
SHA256SUMS
SBOM.spdx.json
symbols.zip if valid
```

---

# 294. GitHub workflow artifacts

用于 job diagnostics可以：

```text
actions/upload-artifact
```

---

# 295. 如果使用

pin exact full SHA。

---

# 296. Actions cache

可使用 cargo cache，但：

> build必须在无cache时也能成功。

---

# 297. 不把 cached binary直接作为 release artifact

release EXE必须由当前 tag job build。

---

# 298. Artifact Provenance Report

创建：

```text
docs/report/phase-09-release-workflow.md
```

---

# 299. Scheduled Workflow

检查现有：

```text
.github/workflows/scheduled.yml
```

---

# 300. 如果不存在，Phase9可创建

每周：

```text
cargo deny advisories
cargo audit if current project policy uses it
dependency drift report
deterministic fuzz/stress smoke
```

---

# 301. 不自动升级依赖

---

# 302. 不自动创建update PR，除非仓库已有该治理策略

---

# 303. cargo audit

如果新增：

应是 CI/dev tool。

不进入 runtime dependency。

---

# 304. Phase 9I — Local RC Build

在全部代码冻结后：

创建：

```text
RC candidate
```

---

# 305. RC Build Commit

必须：

```text
clean worktree
```

---

# 306. RC build前执行 full baseline

---

# 307. Local RC artifact

输出：

```text
dist/
```

---

# 308. `dist/`

应：

```gitignore
/dist/
```

如果未跟踪。

---

# 309. dist 不commit

---

# 310. Package Hash

记录在 Phase9 report。

---

# 311. Local RC 不是 GitHub release

---

# 312. RC Smoke

解压到至少：

```text
ASCII path
space path
Chinese path
```

---

# 313. 各运行一次

---

# 314. RC Data Isolation

三个目录：

各自独立：

```text
note/
```

---

# 315. Same-dir single instance

RC包实际测试。

---

# 316. Different dirs

实际测试。

---

# 317. Main ZIP 不携带 note

---

# 318. Clean VM

尽量使用相同 RC ZIP。

---

# 319. Windows Defender

相同 RC。

---

# 320. Manual acceptance

尽量使用相同 RC。

---

# 321. 如果修了 bug

旧RC作废。

生成新RC。

---

# 322. RC ID

建议：

```text
RC-01
RC-02
...
```

内部报告标识。

---

# 323. 不需要改变 SemVer每次 local RC

---

# 324. Phase 9J — Full Acceptance Matrix

合并：

```text
AC-001 .. AC-030
```

---

# 325. 每一个 AC

最终状态必须存在。

不能只有 Phase9新增。

---

# 326. 创建：

```text
docs/acceptance-cases/phase-09.md
```

作为 release projection。

---

# 327. Matrix

至少：

| AC | Automated | Manual | Final | Evidence |
|---|---|---|---|---|

---

# 328. Final status规则

例如：

```text
AUTOMATED PASS + MANUAL PASS → PASS
```

如果该项不需要manual：

```text
AUTOMATED PASS → PASS
```

---

# 329. Manual-required且NOT TESTED

Final：

```text
BLOCKED
```

---

# 330. USER WAIVED

Final：

```text
WAIVED
```

不能写 PASS。

---

# 331. 关键手工项

至少：

```text
AC-003 Microsoft Pinyin
AC-004 WeChat IME
AC-010 real image paste
AC-013 Preview visual
AC-014 math visual
AC-018 native export
AC-019 Left Dock
AC-020 Right Dock
AC-021 Top Dock
AC-022 Input focus guard
AC-023 Tray
AC-024 Opacity
AC-025 Theme
AC-028 Monitor disconnect
AC-029 Mixed DPI
AC-030 crash recovery
```

---

# 332. User File Safety

即使现有AC-012：

必须有 final PASS。

这是 P0级。

---

# 333. Security

Raw HTML和network safety final PASS。

---

# 334. Phase9 Tests不可因为“以前PASS”而跳过所有回归

关键能力重跑。

---

# 335. Final Regression Order

```text
core tests
render tests
Windows tests
smoke
performance
manual
package
package smoke
clean VM
```

---

# 336. Final Clippy/Fmt

在最终 RC commit再次运行。

---

# 337. Final cargo deny

再次。

---

# 338. Final Dependency tree

保存摘要到 report。

---

# 339. Final Binary Hash

记录。

---

# 340. Final Package Hash

记录。

---

# 341. Release Readiness Report

创建：

```text
docs/report/phase-09-release-readiness.md
```

---

# 342. Report结构

# Phase 9 Release Readiness

## Executive Decision

只能：

```text
RC READY
RC READY WITH USER WAIVERS
NOT RC READY
```

不要直接写：

```text
RELEASED
```

---

# 343. Release Blockers

表：

```text
ID
severity
result
```

---

# 344. Cold Startup

完整数据。

---

# 345. Manual Acceptance

完整统计：

```text
MANUAL PASS
AUTOMATED PASS
NOT TESTED
USER WAIVED
FAIL
```

---

# 346. Performance

最终。

---

# 347. Memory

最终。

---

# 348. Security

最终。

---

# 349. Reliability

最终。

---

# 350. Packaging

最终。

---

# 351. Supply Chain

最终。

---

# 352. Known Issues

列真正剩余。

---

# 353. No Known Data-loss Issues

只有经过测试才能写。

---

# 354. Release Artifact

```text
name
size
SHA-256
```

---

# 355. SBOM

```text
file
tool/version
SHA-256
```

---

# 356. Build Toolchain

```text
rustc
cargo
MSVC target
Windows SDK if known
```

---

# 357. Dependency Lock

```text
Cargo.lock hash
```

---

# 358. Git

```text
commit SHA
```

---

# 359. Stable Release Recommendation

报告最后可以推荐：

```text
RECOMMEND USER CREATE RELEASE TAG
```

或：

```text
DO NOT TAG — BLOCKERS REMAIN
```

---

# 360. Agent不得创建stable tag

除非 USER在后续单独明确要求。

---

# 361. Agent不得push

Phase9仍：

```text
push = no
```

---

# 362. Versioning

检查当前 workspace version。

---

# 363. 如果还是开发版本

不要擅自改到：

```text
1.0.0
```

---

# 364. Version bump属于 release decision

提交给 USER。

---

# 365. Changelog

更新：

```text
CHANGELOG.md
```

但使用：

```text
Unreleased
```

直到 USER确定版本。

---

# 366. Changelog必须只写已实现行为

---

# 367. README Finalization

至少：

```text
product description
Windows 11 x64
portable install/use
data path
view modes
math syntax
images
export
tray
dock
theme
opacity
privacy/network
build from source
license
```

---

# 368. README 不应长成产品手册

保持克制。

---

# 369. 中文 README

如果 repo已有：

```text
README.zh.md
```

同步。

---

# 370. 没有的话

建议创建简洁中文版本。

---

# 371. README 状态

RC阶段：

```text
Release candidate validation
```

---

# 372. 不能写 stable

直到USER发布。

---

# 373. SECURITY.md

必须完善：

```text
supported version
reporting vulnerabilities
data safety
no telemetry
no automatic network access
```

---

# 374. 不公开私人联系方式

除非USER已有。

可使用：

```text
GitHub Security Advisories
```

推荐渠道。

---

# 375. CONTRIBUTING.md

至少：

```text
read AGENTS
architecture-first
fmt/clippy/test
no feature creep
plan_ref
license
```

---

# 376. Release Checklist

创建：

```text
docs/release-checklist.md
```

---

# 377. Checklist区分

```text
Automated
Manual
USER decision
GitHub release
```

---

# 378. USER decision项

至少：

```text
version
cold startup waiver if needed
remaining NOT TESTED waivers
code signing decision
publish draft release
```

---

# 379. GitHub release checklist

未来：

```text
commit main
push
tag
workflow PASS
inspect artifacts
verify checksum
verify attestation
download RC from GitHub
clean machine smoke
publish draft
```

---

# 380. `gh attestation verify`

README/release checklist可以给验证命令。

---

# 381. 不需要普通用户必须安装gh

只是 advanced verification。

---

# 382. SBOM verification

说明 file存在。

---

# 383. Source Code Build

README build指令使用：

```text
rust-toolchain.toml
cargo build --release --locked
```

---

# 384. Windows target

```text
x86_64-pc-windows-msvc
```

---

# 385. 不写不支持平台的 build承诺

---

# 386. License

MIT。

---

# 387. Third-party

单独 notices。

---

# 388. Release workflow本身 security review

必须检查：

1. untrusted PR不能拿 write permission。
2. tag release只构建tag代码。
3. Action pinned SHA。
4. tool downloads checksum。
5. no curl|sh。
6. no mutable latest URLs without verification。
7. GITHUB_TOKEN minimum permissions。
8. artifacts generated current job。
9. package verification before upload。
10. attestation after final artifact。

---

# 389. GitHub Action Major Tags

即使 official：

release workflow也使用SHA。

---

# 390. Action SHA update

未来 Dependabot可更新。

Phase9不必添加Dependabot，如果repo没有治理。

---

# 391. CI Pinning

如果现有 CI workflow仍用 floating tags：

Phase9应审计。

---

# 392. 是否全仓Action pin SHA

推荐：

```text
yes
```

因为 release-readiness。

---

# 393. 但不要在一次大diff中无脑升级 action major

先验证 current behavior。

---

# 394. Current upstream baseline

当前 Prompt 时：

```text
actions/checkout 7.x
actions/upload-artifact 7.x
actions/attest 4.x
```

实现时重新确认。

---

# 395. actions/attest

新 release workflow优先使用。

---

# 396. 不新用 deprecated provenance wrapper

除非 current官方文档发生变化。

---

# 397. SBOM Tool供应链

如果 Syft：

pin：

```text
exact version
download checksum
```

---

# 398. Syft自身不进portable package

---

# 399. Release Workflow Dry Validation

Phase9不能真的tag。

可以：

```text
workflow syntax validation
local package script
local equivalent release build
```

---

# 400. 如果有 `act`

不要求。

Windows workflow与act差异大。

---

# 401. 不为本地模拟引入Docker

---

# 402. GitHub workflow实际验证

需要未来 USER push后由GitHub运行。

Phase9 report应：

```text
WORKFLOW NOT EXECUTED REMOTELY
```

如果尚未push。

---

# 403. 不能写 GitHub CI Release PASS

除非真的运行。

---

# 404. Local equivalent PASS

单独写。

---

# 405. Release Workflow状态

例如：

```text
STATIC REVIEW PASS
LOCAL BUILD/PACKAGE PASS
REMOTE GITHUB RUN NOT TESTED
```

---

# 406. Phase9 Smoke

创建：

```text
tools/smoke/phase-09.ps1
```

---

# 407. 支持

建议：

```powershell
tools/smoke/phase-09.ps1
tools/smoke/phase-09.ps1 -Performance
tools/smoke/phase-09.ps1 -Release
tools/smoke/phase-09.ps1 -Package
```

按现有 conventions。

---

# 408. `all.ps1 -Ci`

继续必须 PASS。

---

# 409. Release Mode

可以执行：

```text
all automated release gates
```

但不执行manual。

---

# 410. Smoke Output

机器可读：

```text
PASS
FAIL
NOT TESTED
```

---

# 411. 不把 manual mock成 smoke。

---

# 412. Test Count

记录最终：

```text
unit
integration
ignored perf
manual
```

---

# 413. Core unsafe

必须：

```text
0
```

---

# 414. Render unsafe

必须：

```text
0
```

---

# 415. Windows unsafe

全部：

```text
adapter boundary
SAFETY docs
```

---

# 416. `unwrap()/expect()` final audit

runtime public/user paths：

审查。

---

# 417. 测试 `expect`合理。

---

# 418. Panic audit

任何用户输入：

```text
Markdown
math
image
config
clipboard
file path
```

不得可触发已知 panic。

---

# 419. Deterministic stress

继续运行：

```text
Markdown randomized
math randomized
editor randomized
assets
window state
```

---

# 420. No telemetry

final scan。

---

# 421. No updater

final scan。

---

# 422. No hidden network

final scan。

---

# 423. Runtime Logs

release默认不得：

```text
log user text
clipboard
math source
image bytes
full private paths
```

---

# 424. Release logs

如果无显式 log file：

更好。

---

# 425. Crash

不自动上传。

---

# 426. Phase9 Dependency Freeze

当 RC candidate形成后：

```text
Cargo.lock frozen
```

---

# 427. 之后不接受非 blocker dependency update

直到 release。

---

# 428. Rust toolchain freeze

```text
rust-toolchain.toml
```

保持 exact。

---

# 429. GitHub Runner changes

Release workflow依赖 hosted runner仍可能变化。

记录：

```text
windows-2025/windows-latest actual image
```

如果当前 GitHub提供具体 Windows 11 runner label且项目已使用，则按 current docs。

不要猜。

---

# 430. Release workflow使用符合 x64 MSVC 的 runner。

---

# 431. Binary Architecture Verify

正式脚本验证：

```text
PE x86-64
```

---

# 432. Manifest Verify

正式。

---

# 433. Subsystem

GUI app不应弹 console window。

验证：

```text
Windows GUI subsystem
```

如果当前如此。

---

# 434. Console smoke helper不进入package。

---

# 435. Debug markers

检查 release EXE不包含明显：

```text
Phase 8 Dev Build
NOT PERSISTED
debug placeholder
```

---

# 436. Placeholder scan

搜索：

```bash
rg "Phase [0-9].*Dev|NOT PERSISTED|TODO USER|placeholder" apps crates
```

人工判断。

---

# 437. Preview placeholder

Remote image placeholder当然是产品行为。

不要误删。

---

# 438. Math placeholder应已不存在正常公式路径。

---

# 439. Missing image fallback是产品行为。

---

# 440. Window controls

最终无 debug labels。

---

# 441. Version info

PE属性中不能仍：

```text
0.0.0
dev
```

除非实际 workspace version如此且这就是RC internal。

---

# 442. Public version USER未批准时

不要假装 stable。

---

# 443. Phase9 Risk Conditions

以下任一必须 STOP 或 USER decision：

### R1

cold startup hard gate无法达到。

### R2

Microsoft Pinyin真实FAIL。

### R3

WeChat IME真实FAIL。

### R4

user asset safety真实FAIL。

### R5

atomic save/crash recoveryFAIL。

### R6

monitor disconnect可丢窗口。

### R7

clean VM无法启动。

### R8

known high-severity security advisory。

### R9

portable package缺license。

### R10

release package携带user data/proprietary font。

---

# 444. 如果只是环境不可测

不是技术FAIL。

但 final：

```text
NOT TESTED
```

Release readiness仍由USER决定。

---

# 445. Review Subagents

如果支持，最多3个。

### Reviewer 1

```text
Data safety
Persistence
Assets
Crash/failure
```

### Reviewer 2

```text
Performance
Startup
Memory
UI responsiveness
```

### Reviewer 3

```text
Release packaging
Supply chain
Licenses
GitHub Actions
Acceptance completeness
```

---

# 446. 最终 main Agent architecture review

必须亲自检查：

1. 有没有Phase9 feature creep？
2. 有没有为startup牺牲fallback？
3. 有没有为内存牺牲正确性？
4. 有没有 ignored blocker？
5. 有没有 manual item被自动化冒充？
6. 有没有用户文件破坏路径？
7. 有没有 release workflow supply-chain弱点？
8. 有没有未固定 tool/action version？
9. 有没有主包漏 license？
10. 有没有 user data进入ZIP？
11. 有没有 WebView/Tokio/network重新进入？
12. 有没有未批准版本号变化？
13. 有没有自动 tag/push/release？
14. 有没有 plan被实现反向削弱？
15. 有没有隐藏风险未进report？

---

# 447. Final Automated Commands

至少：

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

# 448. Smoke

```powershell
tools/smoke/all.ps1 -Ci
tools/smoke/phase-09.ps1
tools/smoke/phase-09.ps1 -Performance
tools/smoke/phase-09.ps1 -Release
tools/smoke/phase-09.ps1 -Package
```

按实际接口。

---

# 449. Forbidden Dependencies

```bash
cargo tree | rg \
"tauri|wry|webview|cef|chromium|tokio|async-std|wgpu|reqwest|hyper|rusqlite"
```

---

# 450. Network API scan

Windows侧也检查：

```text
WinHTTP
WinINet
```

---

# 451. Runtime Unsafe Scan

```bash
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
rg "\bunsafe\b" apps/stickymd-win/src
```

---

# 452. Destructive IO Audit

```bash
rg \
"remove_file|remove_dir|remove_dir_all|DeleteFile|MoveFile|rename" \
apps crates
```

逐个证明 boundary/ownership。

---

# 453. Release Package Verify

必须：

```text
PASS
```

否则不能 RC READY。

---

# 454. Final Git Diff

```bash
git diff --stat <starting-commit>
git diff <starting-commit>
git diff --check
```

---

# 455. Git History

建议 Phase9 commits按职责：

```text
perf(startup): harden cold source initialization

test(rc): close Windows acceptance gaps

fix(rc): resolve release-blocking regressions

build(release): add portable packaging and SBOM

ci(release): add pinned release provenance workflow

docs: finalize release readiness documentation
```

不必机械使用。

---

# 456. 不 squash USER历史

---

# 457. 不 Push

```text
push = no
```

---

# 458. 不 Tag

```text
tag = no
```

---

# 459. 不 Create GitHub Release

```text
release = no
```

---

# 460. Phase9 Task

创建：

```text
docs/tasks/phase-09-pre-release-convergence.md
```

---

# 461. Task内容

至少：

```text
Status
Prerequisites
Inherited Conditions
Feature Freeze
Release Blockers

Cold Startup
Manual Acceptance
Reliability
Performance
Security
Licenses
Packaging
SBOM
Release Workflow
Clean VM
Final RC

Risks
Result
```

---

# 462. 如果实现完成但 manual blockers未关闭

状态：

```text
Implementation Complete — release validation incomplete
```

---

# 463. 如果全部 release gates关闭

```text
Completed — RC ready for USER review
```

---

# 464. Phase9 Final Report

必须：

```text
docs/report/phase-09-release-readiness.md
```

---

# 465. 最终回复格式

严格：

# Phase 9 Result

## Preconditions

```text
Phase 8 recommendation
USER approval
starting commit
inherited conditions count
```

## Repository State Before Work

```text
branch
clean/dirty
```

## Feature Freeze

确认：

```text
No new product capability added.
```

## Release Blockers

表：

```text
ID
severity
before
after
status
```

## Cold Startup

完整：

```text
method
samples
p50
p95
max
milestones
before
after
```

明确：

```text
300ms hard gate = PASS / FAIL / USER WAIVED
```

## Warm Startup

同上。

## Font Initialization

```text
before
after
strategy
fallback regressions?
dependencies changed?
```

## Manual Acceptance Summary

表：

```text
MANUAL PASS
AUTOMATED VISUAL PASS
NOT TESTED
FAIL
USER WAIVED
```

## IME

### Microsoft Pinyin

### WeChat Input Method

必须真实状态。

## Visual Acceptance

```text
Source
Preview
Math
Images
Light
Dark
System
Opacity
```

## Desktop Shell

```text
Tray
Left Dock
Right Dock
Top Dock
sensor
no-focus hover
```

## DPI / Displays

```text
125
150
200
dual monitor
mixed DPI
disconnect
sleep/resume
RDP
```

## Clipboard

真实来源表。

## Export

native dialog result。

## Crash / Recovery

结果。

## User Asset Safety

结果。

## Reliability

完整关键 failure表。

## Final Performance

完整表。

## Final Memory

完整表。

## 4K Image Transient Peak

```text
before
after
decision
```

## Idle CPU

完整。

## Binary Size

```text
EXE
portable ZIP
```

## Dependency Audit

```text
cargo deny
duplicates
advisories
```

## License Audit

列：

```text
MIT
Comrak BSD-2-Clause
RaTeX MIT
KaTeX OFL
...
```

## Security

```text
raw HTML
network
remote images
custom URI
image decode
```

## SBOM

```text
tool
version
format
file
hash
```

## Portable Package

```text
filename
contents
size
SHA-256
verification
```

## Reproducibility Audit

```text
EXE build A hash
EXE build B hash
result
```

## PE Resources

```text
x64
manifest
PerMonitorV2
asInvoker
icon
version
```

## Clean VM

```text
MANUAL PASS / NOT TESTED / FAIL
```

## GitHub Release Workflow

```text
actions pinned by SHA?
package script reused?
SBOM?
checksums?
attestation?
draft release?
remote execution status?
```

必须区分：

```text
STATIC PASS
LOCAL PASS
REMOTE NOT TESTED
```

## Artifact Attestation

说明：

```text
planned/current actions/attest
permissions
artifact subject
```

## Unsafe

```text
core = 0
render = 0
Windows adapter = ...
```

## Architecture Authority

确认：

```text
DocumentState authority unchanged
WindowShellState authority unchanged
ConfigCoordinator unchanged
asset ownership unchanged
```

## Architecture Drift

```text
None
```

或报告。

## Acceptance Matrix

```text
AC-001 .. AC-030
```

最终统计。

## Known Issues

完整。

## USER Decisions Required

例如：

```text
release version
cold-start waiver if failed
manual-test waivers
code signing
stable tag
```

## Verification

所有命令。

## Documentation

列：

```text
task
blockers
startup report
manual acceptance
performance
supply chain
release readiness
release checklist
acceptance matrix
README
CHANGELOG
SECURITY
CONTRIBUTING
```

## Git

```text
commit(s)
push = no
tag = no
release = no
```

## Final Recommendation

只能：

```text
RC READY
```

或：

```text
RC READY WITH USER WAIVERS
```

或：

```text
NOT RC READY
```

最后：

> Awaiting USER release decision. Do not create a tag, push, or publish a GitHub Release automatically.

---

# 466. Phase 9 Definition of Done

只有下列工程项全部满足才可停止：

- [ ] Feature freeze遵守。
- [ ] Phase0–8 inherited conditions完整汇总。
- [ ] 所有 release blockers分类。
- [ ] Cold startup完整instrumentation。
- [ ] Cold startup ≥20 samples。
- [ ] Warm startup ≥20 samples。
- [ ] Cold startup p95 ≤300ms，或USER WAIVED。
- [ ] Warm startup p95 ≤180ms，或USER WAIVED。
- [ ] FontSystem瓶颈被实测。
- [ ] Startup优化没有牺牲CJK/Emoji fallback。
- [ ] 没有bundle proprietary fonts。
- [ ] Microsoft Pinyin真实测试或NOT TESTED。
- [ ] WeChat IME真实测试或NOT TESTED。
- [ ] Preview视觉测试或NOT TESTED。
- [ ] Math视觉测试或NOT TESTED。
- [ ] Image视觉测试或NOT TESTED。
- [ ] Light视觉测试。
- [ ] Dark视觉测试。
- [ ] System theme真实切换。
- [ ] Opacity真实测试。
- [ ] Tray真实测试或NOT TESTED。
- [ ] Left Dock真实测试或NOT TESTED。
- [ ] Right Dock真实测试或NOT TESTED。
- [ ] Top Dock真实测试或NOT TESTED。
- [ ] Hover no-focus真实测试或NOT TESTED。
- [ ] 125% DPI真实测试或NOT TESTED。
- [ ] 150% DPI真实测试或NOT TESTED。
- [ ] 200% DPI真实测试或NOT TESTED。
- [ ] dual monitor真实测试或NOT TESTED。
- [ ] mixed DPI真实测试或NOT TESTED。
- [ ] monitor disconnect真实测试或NOT TESTED。
- [ ] sleep/resume真实测试或NOT TESTED。
- [ ] RDP真实测试或NOT TESTED。
- [ ] Explorer PNG clipboard真实测试或NOT TESTED。
- [ ] Explorer JPEG clipboard真实测试或NOT TESTED。
- [ ] Snipping Tool真实测试或NOT TESTED。
- [ ] browser image clipboard真实测试或NOT TESTED。
- [ ] native Export dialog真实测试或NOT TESTED。
- [ ] hard-kill recovery真实测试或NOT TESTED。
- [ ] real junction/symlink测试或NOT TESTED。
- [ ] Clean Windows 11 VM测试或NOT TESTED。
- [ ] Atomic save failure matrix PASS。
- [ ] OCC external-race PASS。
- [ ] user asset safety PASS。
- [ ] managed-looking fake file safety PASS。
- [ ] raw HTML safety PASS。
- [ ] remote image zero-network PASS。
- [ ] 4K image transient memory评审。
- [ ] Final Source memory测量。
- [ ] Final Preview memory测量。
- [ ] Final Split memory测量。
- [ ] Final Hidden memory测量。
- [ ] Final Idle CPU测量。
- [ ] Final input latency测量。
- [ ] Final Preview latency测量。
- [ ] Final startup测量。
- [ ] Leak stress PASS。
- [ ] Cargo dependency freeze。
- [ ] cargo deny PASS。
- [ ] unresolved high-severity advisory = 0。
- [ ] third-party licenses完整。
- [ ] proprietary font package scan PASS。
- [ ] `SBOM.spdx.json`生成。
- [ ] SBOM tool/version固定。
- [ ] SBOM checksum。
- [ ] Portable staging allowlist。
- [ ] Portable ZIP生成。
- [ ] ZIP不含note/。
- [ ] ZIP不含user data。
- [ ] ZIP不含proprietary fonts。
- [ ] ZIP路径安全。
- [ ] SHA256SUMS生成。
- [ ] symbols策略经过验证。
- [ ] PE x64验证。
- [ ] PerMonitorV2验证。
- [ ] asInvoker验证。
- [ ] icon/version resource验证。
- [ ] package在ASCII path运行。
- [ ] package在space path运行。
- [ ] package在Chinese path运行。
- [ ] same-dir single instance package测试。
- [ ] different-dir instances package测试。
- [ ] README finalization。
- [ ] README.zh同步或创建。
- [ ] CHANGELOG更新为Unreleased。
- [ ] SECURITY.md完善。
- [ ] CONTRIBUTING.md完善。
- [ ] release checklist完成。
- [ ] `.github/workflows/release.yml`完成。
- [ ] release workflow actions pin full SHA。
- [ ] release workflow最小permissions。
- [ ] no pull_request_target release privilege。
- [ ] no curl|sh。
- [ ] package script是CI/local唯一规则。
- [ ] release workflow生成checksums。
- [ ] release workflow生成SBOM。
- [ ] release workflow配置actions/attest。
- [ ] release workflow只创建draft release。
- [ ] release workflow不自动stable publish。
- [ ] release workflow未在Phase9擅自运行远端。
- [ ] Phase9 smoke完成。
- [ ] all.ps1 -Ci PASS。
- [ ] fmt PASS。
- [ ] clippy PASS。
- [ ] workspace tests PASS。
- [ ] Release build PASS。
- [ ] cargo deny PASS。
- [ ] git diff --check PASS。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] no WebView。
- [ ] no Tauri runtime。
- [ ] no Tokio。
- [ ] no DB。
- [ ] no runtime network。
- [ ] no updater。
- [ ] no telemetry。
- [ ] AC-001..AC-030 final release matrix完成。
- [ ] Phase9 task完成。
- [ ] Phase9 reports完成。
- [ ] working tree clean或明确解释。
- [ ] 未push。
- [ ] 未tag。
- [ ] 未创建GitHub Release。
- [ ] 未自动开始任何新产品Phase。

完成后立即停止。
