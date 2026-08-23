# StickyMD Phase 13 — Exact Candidate Qualification Campaign & Release Evidence Closure

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–12 已完成产品实现和 release qualification infrastructure。

当前 Phase 12 结果：

```text
Implementation architecture: complete
Local package: exact and valid
Release/package automation: PASS
Headless CI: PASS

Final status:
NOT RC READY
```

当前已知 Phase 12 candidate：

```text
source commit:
e35a08c5021c5c82233572033756df6995bc5f5c

version:
0.1.0

EXE SHA-256:
b65e86c596975a2c40743a1174248f817562e0de3f2c7ef078367a447d5e5fb6

ZIP SHA-256:
3eca8016726aa9203c096a5c89a53a5c1f65b37c64263bcb11298b446e95e36d

SBOM SHA-256:
6e1f8561d5f0399bb6e3681ff43e29bf3af3a05f82602614e32cf94ad90332f5

Cargo.lock SHA-256:
0c44aa6811f0ef0226a3cc41bddcdebc497a2de7ea13b032f43134f28fabfa25
```

当前 unresolved qualification evidence：

```text
Performance exact-candidate receipt
Runtime exact-candidate receipt
Resources exact-candidate receipt
44 manual acceptance rows
Remote workflow receipt
Downloaded artifact smoke receipt
```

USER 已明确批准：

```text
Warm startup preferred target:
p95 <=180 ms

v0.1.0 release hard boundary:
p95 <=400 ms

Disposition:
USER-APPROVED GATE RECALIBRATION
```

因此 **warm 180 ms 已不再是 release blocker**。

180 ms 继续作为 post-v0.1.0 preferred optimization target。

---

# 0. Phase 13 名称

> **Phase 13 — Exact Candidate Qualification Campaign & Release Evidence Closure**

---

# 1. Phase 13 的唯一目标

Phase 13 不开发产品。

只完成：

```text
Stable source candidate
        ↓
Valid qualification environment
        ↓
Performance receipt
        ↓
Runtime receipt
        ↓
Resources receipt
        ↓
Human acceptance receipts
        ↓
Exact package identity
        ↓
Local readiness
        ↓
USER push decision
        ↓
remote qualification later
```

---

# 2. Phase 13 是 Evidence Campaign，不是 Feature Phase

严格禁止新增：

```text
editor feature
Markdown feature
math feature
window feature
tray behavior
dock behavior
keyboard shortcut
configuration
theme
rendering feature
export feature
asset feature
```

---

# 3. 产品 Feature Freeze

状态：

```text
HARD FROZEN
```

只有发现：

```text
P0 correctness bug
P1 correctness bug
release-blocking platform defect
```

才允许改产品源码。

---

# 4. 不允许 Release Cleanup Refactor

禁止：

```text
final cleanup refactor
rename modules for aesthetics
dependency upgrade for freshness
large formatting rewrite
cache redesign
startup optimization
performance micro-optimization
file splitting just for line count
```

---

# 5. Phase 13 不再优化 warm startup

USER 已批准：

```text
release hard boundary <=400 ms
```

因此只：

```text
measure
validate
record
```

不继续追：

```text
180 ms
```

---

# 6. 只有最终候选实测 warm p95 >400 ms

才重新成为 blocker。

---

# 7. Cold release boundary

沿当前 authoritative Phase 11/12 contract。

当前预期：

```text
cold p95 <=400 ms
```

不要恢复旧300 ms gate。

---

# 8. Performance philosophy

继续遵守 Phase 11：

```text
Correctness
>
UX
>
Maintainability
>
Performance
```

不得为 performance receipt 制造新产品复杂度。

---

# 9. 开始前必须读取

严格执行最近的：

```text
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md
```

完整读取：

```text
docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/05_document_persistence.md
docs/plan/07_editor_and_ime.md
docs/plan/08_assets_and_export.md
docs/plan/09_windows_shell.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md
```

并完整读取：

```text
docs/report/phase-11-rc-readiness.md
docs/report/phase-11-performance-final.md
docs/report/phase-11-manual-acceptance.md

docs/report/phase-12-final-qualification.md
docs/report/phase-12-release-handoff.md
docs/report/phase-12-release-decisions.md

docs/acceptance-cases/phase-11.md
docs/acceptance-cases/phase-11-b.md
docs/acceptance-cases/phase-12.md

docs/release-checklist.md
```

---

# 10. Repository Preflight

首先：

```bash
git status --short
git branch --show-current
git log -15 --oneline

git rev-parse HEAD
git rev-parse origin/main
```

记录：

```text
HEAD
origin/main
ahead
behind
dirty
```

---

# 11. 不 reset

禁止：

```text
git reset
git clean
git rebase
force push
```

---

# 12. Phase 13 分为八部分

严格顺序：

```text
13A — Qualification Environment Gate
13B — Pre-Freeze Qualification Tool Fixes
13C — Final Exact Candidate Freeze
13D — Automated Evidence Campaign
13E — Manual Acceptance Campaign
13F — Local Readiness Consolidation
13G — Push/Remote Handoff
13H — Final Phase 13 Decision
```

---

# 13. Phase 13A — Qualification Environment Gate

Phase 12 最大问题之一：

> 长时间测试运行后才发现当前输入桌面由 `LockApp` 占据。

Phase 13 必须先解决这个 **test-environment fail-fast** 问题。

---

# 14. 注意：这是 Tooling 修正

只能进入：

```text
tools/stickymd-smoke
tools/smoke
```

不得进入 product runtime。

---

# 15. 创建 QualificationEnvironment

例如：

```rust
struct QualificationEnvironment {
    interactive_session: bool,
    input_desktop: InputDesktopState,
    workstation_locked: bool,
    foreground_available: bool,
    display_topology: ...,
}
```

具体按现有 tooling architecture。

---

# 16. 不过度抽象

只需要回答：

> 当前 Windows session 是否可以形成有效 GUI runtime/performance/resource evidence？

---

# 17. 至少检测

```text
interactive user session exists
input desktop usable
not obviously LockApp / locked session
main candidate can obtain expected interactive shell conditions
```

---

# 18. 不靠固定 sleep猜测

使用实际平台事实。

---

# 19. 如果已有工具能检查这些事实

复用。

不要再造第二套 platform inspection。

---

# 20. Windows 检测代码只能进入 smoke/tooling adapter

不得进入：

```text
stickymd-core
stickymd-render
product shell behavior
```

---

# 21. Environment结果

统一：

```text
VALID
ENVIRONMENT_BLOCKED
UNSUPPORTED
ERROR
```

---

# 22. 如果 session 被锁

必须在正式 suite 开始前：

```text
ENVIRONMENT_BLOCKED
```

并返回明确非零/blocked code。

---

# 23. 不运行54分钟以后才发现无效

这是 Phase 13 hard tooling requirement。

---

# 24. Performance suite preflight

顺序：

```text
environment preflight
→ valid
→ run performance
```

---

# 25. Runtime suite

同样。

---

# 26. Resources suite

同样。

---

# 27. Manual acceptance helper

也先显示：

```text
Environment:
VALID / BLOCKED
```

---

# 28. 机器可读 evidence

例如：

```json
{
  "qualification_environment": {
    "status": "VALID",
    "interactive": true,
    "desktop": "...",
    "locked": false
  }
}
```

不要输出隐私敏感内容。

---

# 29. 如果桌面名称等信息无需存

只保留：

```text
valid / locked / unavailable
```

即可。

---

# 30. Environment preflight必须快

目标：

```text
<1 second typical
```

不是新的长测试。

---

# 31. Environment Blocked 不等于 Product FAIL

必须记录：

```text
NOT TESTED — ENVIRONMENT BLOCKED
```

不能：

```text
FAIL
```

---

# 32. 但也不能算 PASS

---

# 33. Phase 13B — Pre-Freeze Qualification Tool Fixes

只有在 candidate freeze 前允许修改：

```text
tooling
current-state docs
qualification orchestration
```

---

# 34. 盘点当前 Phase12 qualification

确认是否仍存在：

```text
stale receipt selection
old SHA acceptance
receipt type substitution
LockApp late detection
output collision
```

---

# 35. Readiness五通道保持

Phase12修复后的：

```text
Release
Headless CI
Performance
Runtime
Resources
```

五项不可互换。

这是正确的。

不得削弱。

---

# 36. Readiness不能因为

```text
Release package PASS
```

就替代：

```text
Performance
Runtime
Resources
```

---

# 37. Exact SHA binding继续

所有 receipt：

```text
source_commit
exe_sha256
```

必须匹配。

---

# 38. Receipt selection

fail closed。

---

# 39. 不自动选“最新看起来差不多”的 receipt

---

# 40. Phase 13 manual receipt tooling

如果 Phase12还没有足够好的人类验收录入工具：

允许在 freeze前完成。

---

# 41. 推荐 CLI

例如：

```text
stickymd-smoke manual list
stickymd-smoke manual run <session>
stickymd-smoke manual record ...
stickymd-smoke manual status
```

按当前CLI风格调整。

---

# 42. 但不要造完整 TUI framework

简单 CLI即可。

---

# 43. Manual receipt不是自动判定

工具只：

```text
show steps
show expected
record human result
bind artifact
```

---

# 44. Human result只允许

```text
MANUAL_PASS
FAIL
NOT_TESTED
```

以及 USER决定后的：

```text
USER_WAIVED
```

---

# 45. Agent自己不能产生 USER_WAIVED

---

# 46. 避免人工44项逐条完全重复操作

允许把 acceptance cases组织成：

```text
manual sessions
```

---

# 47. 但 session只是执行优化

不能减少 requirements。

---

# 48. 推荐 Manual Sessions

## Session M1 — Editor / IME / Zoom / Math

覆盖：

```text
Microsoft Pinyin
WeChat IME
Source editing
traditional shortcuts
zoom
math conversion
Undo/Redo
Preview visual
math visual
```

---

## Session M2 — Shell / Tool Window / Dock

覆盖：

```text
taskbar absent
Alt+Tab absent
Alt+Tab away
tray
topmost
Pin orthogonality
Top Dock
Left Dock
Right Dock
No Bottom
24 DIP capture
nearest edge
sensor
220×120
opacity
theme
```

---

## Session M3 — Clipboard / Images / Export / Recovery

覆盖：

```text
Explorer PNG
Explorer JPEG
Snipping Tool
browser image
Shift+Insert image
image visual
native Export dialog
hard-kill recovery
user asset safety
```

---

## Session M4 — Multi-Monitor / DPI

覆盖：

```text
dual monitor
mixed DPI
125/150/200
secondary dock
monitor disconnect
negative geometry if available
```

---

## Session M5 — Environment / Platform Optional

覆盖：

```text
sleep/resume
RDP
junction/symlink real test
Clean Windows 11 VM
```

---

# 49. 一个 manual session PASS 不自动让全部rows PASS

每一个 case必须：

```text
explicit observation recorded
```

---

# 50. Session只是共享setup

---

# 51. Phase13 source-controlled deliverables

在 freeze 前创建：

```text
docs/tasks/phase-13-exact-candidate-qualification.md
docs/acceptance-cases/phase-13.md
docs/report/phase-13-qualification-plan.md
```

---

# 52. 不创建几十个新的 report

Phase13重点是 evidence。

---

# 53. 根 AGENTS/current summary

如果仍写 Phase12 In Progress：

改成：

```text
Phase 12 local qualification infrastructure complete.
Phase 13 exact-candidate evidence qualification in progress.
```

---

# 54. 不写 RC READY。

---

# 55. Phase 13C — Final Exact Candidate Freeze

完成所有需要进入source的：

```text
tool fixes
docs
receipt schema
manual helper
```

之后：

```text
STOP TRACKED CHANGES
```

---

# 56. 运行 pre-freeze gates

```bash
cargo fmt --check

cargo clippy --workspace --all-targets -- -D warnings

cargo test --workspace --locked

cargo build --workspace --release --locked

cargo deny check

git diff --check
```

---

# 57. 运行：

```text
all --ci --json
```

---

# 58. 全部通过后 commit

建立新的：

```text
PHASE13_RELEASE_SOURCE_COMMIT
```

---

# 59. Commit后：

```bash
git status --short
```

必须：

```text
clean
```

---

# 60. 这一 SHA 是 Phase13唯一 candidate source

---

# 61. 不再使用：

```text
e35a08c...
```

作为最终候选，如果 Phase13发生了任何 tracked change。

---

# 62. 如果 Phase13无需任何tracked变化

可以继续使用：

```text
e35a08c...
```

但必须明确证明：

```text
HEAD unchanged
```

---

# 63. Freeze之后

不得再 commit：

```text
docs
test reports
manual results
performance data
```

---

# 64. Evidence全部进入 gitignored：

```text
dist/evidence/
```

---

# 65. 这样不会再次造成

```text
candidate SHA != evidence HEAD
```

循环。

---

# 66. Freeze之后若必须修改产品/tooling

立即：

```text
candidate invalidated
```

然后：

```text
new commit
new package
new receipts
```

---

# 67. Phase 13D — Automated Evidence Campaign

顺序必须优化。

不要随机先跑最贵的。

---

# 68. 正确顺序

```text
Environment Preflight
        ↓
Release/Package
        ↓
Headless CI
        ↓
Runtime
        ↓
Performance
        ↓
Resources
        ↓
Readiness
```

---

# 69. 为什么 Runtime先于Resources

Phase12 Runtime旧receipt已经暴露：

```text
sensor hover failure
```

虽然已修。

先跑较短 Runtime可以提前发现 GUI状态问题。

不要先花一小时Resources。

---

# 70. Runtime必须 exact candidate

绑定：

```text
PHASE13_RELEASE_SOURCE_COMMIT
EXE SHA256
```

---

# 71. Runtime suite至少覆盖

```text
launch
EDITOR_READY
tray lifecycle
tool-window behavior that is automatable
sensor
dock reducer/runtime
opacity
zoom
second instance
save/restart
```

按现有 suite。

---

# 72. 如果 Runtime FAIL

停止：

```text
Performance
Resources
```

直到分类。

避免浪费。

---

# 73. 如果 Runtime被 environment blocked

停止昂贵 suites。

报告：

```text
ENVIRONMENT BLOCKED
```

---

# 74. Performance

环境有效后运行。

---

# 75. Startup最终 gate

正式：

```text
Cold p95 <=400 ms
Warm p95 <=400 ms
```

---

# 76. 同时报告 preferred targets

例如：

```text
Warm preferred <=180 ms
```

但 readiness只使用 approved release boundary。

---

# 77. 不要再次把180作为 hard blocker

这是已批准决策。

---

# 78. Performance样本

遵循 Phase11成熟方法：

```text
Cold >=30
Warm >=50
unique ready objects
previous process fully exited
EDITOR_READY unchanged
```

---

# 79. 如果系统环境方差很大

仍不能删outlier。

---

# 80. 如果 p95 >400

真实 FAIL。

---

# 81. 但先检查：

```text
environment remained valid throughout run
```

---

# 82. 建议 suite 周期性 environment check

例如每 cohort 前后。

---

# 83. 不需要每毫秒poll。

---

# 84. 如果中途 session lock

标记：

```text
INVALIDATED BY ENVIRONMENT CHANGE
```

而不是混入统计。

---

# 85. 不偷偷过滤锁屏样本

整次receipt invalid。

---

# 86. Resources

只有 Runtime+Performance有效后运行。

---

# 87. Resource suite可以很长

先打印预计阶段列表，不需要提供时间估计。

---

# 88. Resource suite应逐场景落盘partial evidence

避免：

```text
54分钟后失败
→ 前53分钟数据全部丢失
```

---

# 89. 但最终 Resources PASS只有：

```text
required scenarios all complete
```

---

# 90. Partial evidence状态：

```text
INCOMPLETE
```

不能 PASS。

---

# 91. 场景之间再次 environment preflight

例如：

```text
Source
Preview
Split
image cache
hidden
stress
```

每个大阶段前检查。

---

# 92. 如果中间锁屏

立即停止。

---

# 93. 不继续耗时跑无效数据。

---

# 94. Resources最终至少记录

```text
PWS median/max
Private Bytes median/max
Idle CPU p95
stress deltas
handles if supported
```

---

# 95. 当前 hard budgets保持

不重新微调。

---

# 96. 4K image transient peak

记录即可。

不是 release blocker，除非出现：

```text
OOM
crash
unbounded growth
```

---

# 97. Automated Receipt Set

最终必须存在：

```text
release.json
ci.json
runtime.json
performance.json
resources.json
```

实际命名按当前 schema。

---

# 98. 五份必须绑定：

```text
same source SHA
same EXE SHA
```

---

# 99. Readiness在人工之前

应返回：

```text
NOT_READY
```

但 blockers只剩manual/remote/decisions等合理项目。

---

# 100. 不应再出现

```text
stale performance
stale runtime
missing resources
```

---

# 101. Phase 13E — Manual Acceptance Campaign

Automated exact-candidate receipts全部有效后才开始正式人工验收。

避免人工测完后又发现candidate自动测试失败。

---

# 102. Manual artifact

必须使用：

```text
exact Phase13 ZIP / EXE
```

---

# 103. 不使用：

```text
cargo run
debug build
older local RC
```

---

# 104. Manual receipt头部显示

```text
source SHA
EXE SHA
ZIP SHA
```

让操作人确认。

---

# 105. Manual Session M1 — Editor / IME / Math

至少：

### Microsoft Pinyin

检查：

```text
English typing
Chinese composition
candidate window
commit
selection replacement
Ctrl+Z atomic commit
zoom50
zoom100
zoom300
Split
Docked
Opacity40
Alt+Tab away
return via click/tray
```

---

# 106. Microsoft Pinyin只要环境存在

必须真实执行。

---

# 107. WeChat Input Method

同等级。

---

# 108. 如果环境确实没装

```text
NOT TESTED
```

不能 PASS。

---

# 109. Traditional shortcuts

真实：

```text
Ctrl+Insert Copy
Shift+Delete Cut
Shift+Insert Paste
```

---

# 110. Image Shift+Insert

真实截图/clipboard。

---

# 111. Math conversion

真实：

```text
\(x\)
\[
y
\]
```

按钮：

```text
$x$
$$
y
$$
```

---

# 112. 一次 Undo

必须整体恢复。

---

# 113. Preview visual

至少：

```text
CJK/Latin
heading
list
code
table
raw HTML literal
math
image
```

---

# 114. Math visual

至少：

```text
inline baseline
fraction
sqrt
integral
sum
matrix
cases
display math
malformed fallback
```

---

# 115. Session M2 — Shell

必须真实：

```text
Taskbar absent
Alt+Tab absent
```

---

# 116. 关键 bug requirement

StickyMD focused：

```text
Alt+Tab once
```

必须切到另一个窗口。

---

# 117. 点击 StickyMD

重新focus。

---

# 118. Tray

真实 Explorer tray：

```text
显示/隐藏
置顶
退出
```

---

# 119. exactly 3 logical menu items

---

# 120. Close

```text
Close → tray
Alt+F4 → tray
```

---

# 121. Dock

分别：

```text
Top
Left
Right
```

---

# 122. Bottom

必须确认：

```text
does not dock
```

---

# 123. Capture

实际拖：

```text
near edge
```

验证约24 DIP容差感受。

---

# 124. nearest edge

corner附近：

最近边获胜。

---

# 125. tie

只有近似相等才priority：

```text
Top > Left > Right
```

---

# 126. Drag release

必须：

```text
DockedExpanded
```

不立即消失。

---

# 127. Focus away

约700ms collapse。

---

# 128. Pin ON

重复focus-away。

仍collapse。

这是 Phase11-B重要人工证明。

---

# 129. Sensor

约100ms reveal。

---

# 130. Hover reveal不抢foreground

---

# 131. pointer leave

约500ms collapse。

---

# 132. 220×120

真实：

```text
Source
Preview
Split
```

---

# 133. 检查：

```text
Close reachable
major controls usable
no catastrophic overlap
```

---

# 134. Zoom

```text
50
100
150
200
300
Ctrl+wheel
Ctrl+0
```

---

# 135. shell controls不能zoom。

---

# 136. Opacity

```text
40
70
96
100
```

---

# 137. 40%仍：

```text
clickable
focusable
IME works
not click-through
```

---

# 138. Theme

```text
Light
Dark
System
```

System切换Windows theme验证动态跟随。

---

# 139. Session M3 — Clipboard / Export / Recovery

Clipboard真实来源：

```text
Explorer PNG
Explorer JPEG
Snipping Tool
Paint if available
browser copied image
```

---

# 140. 验证：

```text
managed asset
format
Markdown insertion
Preview
Undo
Redo
```

---

# 141. User image safety

真实：

```text
note/images/user-important.png
```

经过：

```text
edit
undo
redo
restart
GC
export
quit
```

仍存在。

---

# 142. Fake managed-looking image

hash mismatch：

不得删。

---

# 143. Export dialog

真实：

```text
cancel
normal
space path
Chinese path
existing MD
```

---

# 144. working note不改变。

---

# 145. Hard-kill recovery

真实：

```powershell
Stop-Process -Id <pid> -Force
```

在：

```text
dirty typing
asset transaction if practical
```

至少完成计划中的 release-critical case。

---

# 146. 重启：

```text
no corruption
recovery choice correct
asset reconcile safe
```

---

# 147. Session M4 — Displays

如果设备可用：

```text
dual monitor
same DPI
mixed DPI
```

---

# 148. 拖到secondary。

---

# 149. Dock secondary。

---

# 150. Restart。

---

# 151. Disconnect secondary。

恢复primary。

---

# 152. DPI：

尽量：

```text
125
150
200
```

---

# 153. IME candidate位置在DPI变化后正确。

---

# 154. 如果无法物理完成某项

保持：

```text
NOT TESTED
```

---

# 155. Session M5 — Optional Platform

如果可用：

```text
sleep/resume
RDP
junction/symlink
Clean VM
```

---

# 156. Clean VM优先级很高

如果有条件：

使用 exact Phase13 portable ZIP。

---

# 157. VM不得要求：

```text
Rust
Git
Visual Studio
development tools
```

---

# 158. Clean VM最低验证：

```text
launch
note creation
typing
preview
math
tray
quit
```

---

# 159. Manual acceptance结果

最终输出：

```text
MANUAL PASS
FAIL
NOT TESTED
USER WAIVED
```

---

# 160. USER WAIVED只有 USER可提供

Agent不要询问44次。

最后聚合剩余 `NOT TESTED` 给 USER决定即可。

---

# 161. Manual FAIL

先分类：

```text
P0
P1
P2
P3
environment
```

---

# 162. P0/P1

必须修。

Candidate invalidated。

---

# 163. P2

报告给 USER。

不要自动决定阻塞与否。

---

# 164. P3

一般 defer。

---

# 165. Phase 13F — Local Readiness Consolidation

完成 automated + manual后运行：

```text
readiness
```

---

# 166. 在没有 remote evidence前

正常不能最终Release Ready。

但应达到：

```text
LOCAL RC READY — REMOTE QUALIFICATION REQUIRED
```

或项目现有等价状态。

---

# 167. 当前两个 USER decision仍可能 pending

### Version

```text
0.1.0
```

### Unsigned release policy

---

# 168. Agent不得自行批准

---

# 169. Phase13 local final blockers应只剩：

```text
USER decisions
remote workflow
downloaded artifact
```

或者真实manual遗留。

---

# 170. 如果 automated仍有 stale/missing receipt

Phase13没有完成。

---

# 171. 如果 exact candidate local readiness形成

输出：

```text
LOCAL EVIDENCE COMPLETE
```

---

# 172. 生成最终 local evidence summary

```text
dist/evidence/phase-13-local-readiness.json
```

---

# 173. 内容至少：

```text
candidate identity
automated statuses
manual statuses
performance
resource summary
P0/P1
remaining blockers
```

---

# 174. Phase 13G — Push / Remote Handoff

当前 USER 本轮没有授权：

```text
push
```

因此不要 push。

---

# 175. 当 local candidate ready时

报告：

```text
LOCAL RC READY — PUSH AUTHORIZATION REQUIRED
```

---

# 176. 不自动执行远端 workflow。

---

# 177. USER后续明确批准push后

Phase13继续，不创建Phase14。

---

# 178. Push only exact candidate

先：

```bash
git status --short
git rev-parse HEAD
```

---

# 179. 必须：

```text
HEAD == PHASE13_RELEASE_SOURCE_COMMIT
worktree clean
```

---

# 180. 然后：

```bash
git push origin main
```

不得 force。

---

# 181. 确认：

```text
origin/main == exact SHA
```

---

# 182. 运行 existing release workflow 的：

```text
workflow_dispatch
```

做 remote dry-run。

---

# 183. 不是 tag run。

---

# 184. 不生成draft release。

---

# 185. Remote workflow必须 exact candidate。

---

# 186. 下载 artifact。

---

# 187. Verify：

```text
package
checksums
SBOM
```

---

# 188. 对下载artifact运行：

```text
portable smoke
```

---

# 189. Remote receipt绑定：

```text
workflow run ID
source SHA
artifact ID
hash
```

---

# 190. 完成后：

```text
READY FOR TAG APPROVAL
```

如果其它 gates也关闭。

---

# 191. 不 tag

直到 USER明确：

```text
Approve tag v0.1.0 on <exact SHA>
```

---

# 192. Phase 13H — Final Decision

Phase13可能有这些终态：

```text
NOT RC READY

LOCAL RC READY — USER DECISIONS REQUIRED

LOCAL RC READY — PUSH AUTHORIZATION REQUIRED

REMOTE QUALIFIED — TAG APPROVAL REQUIRED
```

---

# 193. 如果发现 P0/P1

```text
NOT RC READY
```

---

# 194. 不再创建 Phase14

继续 Phase13修复/qualification。

---

# 195. Release Version Decision

当前：

```text
0.1.0
```

如果 USER尚未明确批准：

保持 blocker：

```text
PENDING USER APPROVAL
```

---

# 196. Unsigned Policy

当前：

```text
unsigned
```

如果没有代码签名证书，这是正常开源首版路径。

但 Agent不能批准。

保持：

```text
PENDING USER APPROVAL
```

---

# 197. 不生成自签证书

---

# 198. 不为了 SmartScreen买/伪造证书。

---

# 199. Architecture Audit

Phase13结束前必须确认：

```text
DocumentState sole text authority
ConfigCoordinator sole preference authority
window reducer sole shell logic authority
Preview derived
disk not runtime authority
generation stale drop
bounded cache
bounded queue
managed asset ownership proof
```

---

# 200. Qualification tooling不能侵入 product architecture

---

# 201. Product runtime dependencies

理想：

```text
delta = 0
```

---

# 202. Core/render

继续：

```text
unsafe=0
```

---

# 203. No WebView/Tauri/Tokio/DB/network

继续。

---

# 204. Final Dependency Audit

不升级正常工作的依赖。

---

# 205. cargo deny

必须PASS。

---

# 206. Duplicate versions

已有可接受项不要现在处理。

---

# 207. GitHub Actions

不要为了最新版升级。

---

# 208. Candidate冻结后不能修改workflow

否则 candidate invalid。

---

# 209. Performance数据最终报告

必须使用：

```text
exact Phase13 candidate
valid environment
```

---

# 210. 不引用 Phase11/12旧候选作为最终PASS。

---

# 211. Historical data可以比较

但明确：

```text
historical
```

---

# 212. Environment Guard Report

创建：

```text
docs/report/phase-13-qualification-plan.md
```

中记录设计即可。

动态结果不再commit。

---

# 213. Phase13 task

创建：

```text
docs/tasks/phase-13-exact-candidate-qualification.md
```

---

# 214. Status状态

开始：

```text
In Progress
```

---

# 215. Freeze后：

```text
Candidate Frozen — Qualification In Progress
```

不需要commit更新。

这只是运行时/聊天状态。

---

# 216. 如果 local证据完成：

```text
Local Qualification Complete — USER Decisions Required
```

---

# 217. 不为状态变化改source commit。

---

# 218. Phase 13 Acceptance

创建：

```text
docs/acceptance-cases/phase-13.md
```

---

# 219. 它只投影 qualification过程：

```text
environment gate
exact receipt binding
automated five-channel evidence
manual receipt integrity
candidate identity
```

---

# 220. 不重新复制所有产品AC正文

引用现有 matrix。

---

# 221. Phase13 final local report不要commit-after-freeze

最终详细动态结果：

```text
chat response
+
dist/evidence
```

---

# 222. 如果需要 source report模板

freeze前创建：

```text
docs/report/phase-13-final-qualification.md
```

写明：

```text
Runtime results are stored as hash-bound untracked evidence.
```

---

# 223. 不在测试完成后修改这个文件填数字

否则又破坏 candidate identity。

---

# 224. Final automated verification

Freeze前和exact candidate都应覆盖：

```bash
cargo fmt --check

cargo clippy --workspace --all-targets -- -D warnings

cargo test --workspace --locked

cargo build --workspace --release --locked

cargo test -p stickymd-core --release --locked
cargo test -p stickymd-render --release --locked
cargo test -p stickymd-win --release --locked

cargo deny check

git diff --check
```

---

# 225. Rust automation

至少：

```text
release
all --ci
runtime
performance
resources
readiness
```

---

# 226. 顺序按 Phase13 campaign。

---

# 227. Final package

必须重新生成 Phase13 exact candidate：

```text
EXE
ZIP
SBOM
SHA256SUMS
```

---

# 228. Candidate receipt

记录：

```text
source commit
EXE hash
ZIP hash
SBOM hash
Cargo.lock hash
toolchain
```

---

# 229. Package verification

必须PASS。

---

# 230. ASCII path

PASS。

---

# 231. Space path

PASS。

---

# 232. Chinese path

PASS。

---

# 233. Same-dir single instance

PASS。

---

# 234. Different-dir instances

PASS。

---

# 235. Final resource suite

不要只因为运行时间长而跳过。

但：

> 必须先通过 environment preflight。

---

# 236. 环境被锁时

立即停止并告诉 USER：

```text
Qualification environment is blocked by locked/non-interactive desktop.
Unlock the active Windows session and rerun Phase 13 evidence campaign.
```

不要继续做无意义结果。

---

# 237. Manual未执行时

同样诚实。

---

# 238. Final Response Format

严格：

# Phase 13 Qualification Result

## Candidate

```text
source commit:
version:
EXE SHA-256:
ZIP SHA-256:
SBOM SHA-256:
Cargo.lock SHA-256:
toolchain:
worktree:
```

## Qualification Environment

```text
status:
interactive:
desktop valid:
lock-state:
```

不要输出不必要私人信息。

## Architecture

```text
P0:
P1:
drift:
runtime dependency delta:
core unsafe:
render unsafe:
```

## Automated Evidence

| Channel | Exact SHA | Result |
|---|---|---|
| Release/package | | |
| Headless CI | | |
| Runtime | | |
| Performance | | |
| Resources | | |

## Startup

```text
Cold samples:
Cold p50:
Cold p95:
Cold <=400:

Warm samples:
Warm p50:
Warm p95:
Warm <=400:

Preferred warm target <=180:
informational only
```

## Resources

完整关键表。

## Runtime

列关键 runtime scenarios。

## Manual Acceptance

```text
MANUAL PASS:
NOT TESTED:
FAIL:
USER WAIVED:
```

按Session：

```text
M1 Editor/IME
M2 Shell/Dock
M3 Clipboard/Export/Recovery
M4 Displays
M5 Platform/Clean VM
```

## Exact Receipt Integrity

确认：

```text
all five automated receipts:
same source SHA
same EXE SHA

manual receipt:
same source SHA
same EXE SHA
```

## Package

```text
filename:
size:
verify:
ASCII:
space:
Chinese:
single-instance:
multi-instance:
```

## Readiness

逐项 blocker。

## Remaining USER Decisions

必须列：

```text
Release version 0.1.0:
APPROVED / PENDING

Unsigned release:
APPROVED / PENDING

Manual waivers:
...

Push:
NOT AUTHORIZED
```

## Remote Status

```text
origin exact:
workflow dispatch:
downloaded artifact:
```

## Git

```text
candidate commit:
origin/main:
ahead/behind:
worktree:
push=no
tag=no
release=no
```

## Recommendation

只能：

```text
NOT RC READY

LOCAL RC READY — USER DECISIONS REQUIRED

LOCAL RC READY — PUSH AUTHORIZATION REQUIRED

REMOTE QUALIFIED — TAG APPROVAL REQUIRED
```

最后：

> Do not tag or publish. Continue within Phase 13 after the next explicit USER authorization.

---

# 239. Phase 13 Definition of Done

- [ ] Phase13没有新增产品功能。
- [ ] Qualification environment preflight实现。
- [ ] LockApp/locked session可以fail fast。
- [ ] Environment blocked不冒充product failure。
- [ ] Environment blocked不冒充PASS。
- [ ] Runtime前先environment gate。
- [ ] Performance前先environment gate。
- [ ] Resources前先environment gate。
- [ ] Long resource suite分阶段保存partial evidence。
- [ ] Partial resources不能PASS。
- [ ] Phase13 task创建。
- [ ] Phase13 acceptance创建。
- [ ] Phase13 qualification report模板创建。
- [ ] Readiness五通道不削弱。
- [ ] Receipt exact-SHA binding保持。
- [ ] Manual receipt helper可用。
- [ ] Manual receipt不能自动PASS。
- [ ] Manual sessions只优化操作，不减少cases。
- [ ] 所有tracked preparation在candidate freeze前完成。
- [ ] Freeze后不再commit evidence。
- [ ] Final candidate SHA唯一。
- [ ] worktree clean。
- [ ] final package绑定candidate。
- [ ] final SBOM绑定candidate。
- [ ] final checksum绑定candidate。
- [ ] Release/package exact receipt PASS。
- [ ] Headless CI exact receipt PASS。
- [ ] Runtime exact receipt有效。
- [ ] Performance exact receipt有效。
- [ ] Resources exact receipt有效。
- [ ] Cold使用approved <=400 gate。
- [ ] Warm使用approved <=400 gate。
- [ ] Warm <=180只作为preferred target。
- [ ] 不再继续warm优化。
- [ ] P0=0。
- [ ] P1=0，或block。
- [ ] Source/Preview/Split资源重新测。
- [ ] Hidden资源重新测。
- [ ] Idle CPU重新测。
- [ ] Stress无unbounded增长。
- [ ] 4K image characteristic记录。
- [ ] M1 manual session执行或NOT TESTED。
- [ ] M2 manual session执行或NOT TESTED。
- [ ] M3 manual session执行或NOT TESTED。
- [ ] M4 manual session执行或NOT TESTED。
- [ ] M5 manual session执行或NOT TESTED。
- [ ] Pinyin真实状态。
- [ ] WeChat真实状态。
- [ ] Taskbar真实状态。
- [ ] Alt+Tab真实状态。
- [ ] Tray真实状态。
- [ ] Dock真实状态。
- [ ] Pin orthogonality真实状态。
- [ ] Zoom真实状态。
- [ ] Math conversion真实状态。
- [ ] Clipboard真实状态。
- [ ] Export真实状态。
- [ ] Crash recovery真实状态。
- [ ] Multi-monitor状态。
- [ ] Clean VM状态。
- [ ] Manual evidence绑定exact artifact。
- [ ] stale manual receipt拒绝。
- [ ] final local readiness执行。
- [ ] version decision仍由USER。
- [ ] unsigned policy仍由USER。
- [ ] manual waivers仍由USER。
- [ ] push仍由USER。
- [ ] 未push。
- [ ] 未tag。
- [ ] 未创建Release。
- [ ] 未创建Phase14。

完成当前可执行部分后立即停止。