# StickyMD Phase 11 — RC Convergence, Constraint Calibration & Manual Acceptance Closure

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–10 已完成。

当前 Phase 10 Result：

```text
STOP — NOT RC READY
```

当前主要未关闭项：

```text
1. Warm startup p95 ≈ 392.263 ms
   existing target <=180 ms
   status = FAIL

2. 多项真实 Windows / IME / visual / multi-monitor manual acceptance
   status = NOT TESTED
```

Phase 10 已完成的 USER-approved UX contract 包括：

```text
traditional clipboard shortcuts
content zoom 50–300%
minimum window 220×120 DIP
tool-window identity
taskbar hidden
Alt+Tab hidden
24 DIP dock capture
nearest-edge docking
Top > Left > Right tie priority
opacity 40–100%
```

这些已冻结。

Phase 11 不得重新讨论或撤销，除非发现 correctness defect。

---

# 0. Phase 11 名称

> **Phase 11 — RC Convergence, Constraint Calibration & Manual Acceptance Closure**

---

# 1. Phase 11 的真正目标

本阶段不再增加产品功能。

目标只有：

```text
A. 审计并关闭真正的 RC blockers

B. 对性能 / 资源 gate 做一次符合工程宪法的重新校准

C. 修复仍然存在的 correctness / UX defects

D. 完成尽可能多的真实 Windows acceptance

E. 重新生成最终 RC candidate

F. 给 USER 一个可信的：
   RC READY
   / RC READY WITH USER-APPROVED RELAXATIONS
   / NOT RC READY
   结论
```

---

# 2. 本阶段最高治理修正

从 Phase 11 起，必须正式恢复工程宪法的原始优先级：

```text
Correctness / Functionality
>
Usability / UX
>
Foundational Ecosystem Compatibility
>
Maintainability / Diagnosability
>
Performance
>
RAM
>
Disk
>
Secondary Factors
```

因此：

> **性能数字不得凌驾于架构质量之上。**

---

# 3. Performance Gate 不等于 Architecture Invariant

必须明确区分：

## Architecture Invariant

例如：

```text
DocumentState is the only canonical text authority
user files must never be destructively managed without ownership proof
ordinary autosave must not silently overwrite an externally changed note
core/render unsafe = 0
no WebView
no runtime network fetch
```

这些不能因为性能而放宽。

---

## Engineering Performance Gate

例如：

```text
warm startup p95 <=180ms
cold startup p95 <=300ms
memory target
binary size target
preview latency target
```

这些是：

> engineering decision thresholds

而不是：

> 系统本体公理。

---

# 4. Performance Gate 的正确处理规则

如果某项性能 gate FAIL：

禁止直接：

```text
不停重构
不断堆缓存
增加第二条代码路径
拆穿 architecture boundaries
牺牲 correctness
```

必须依照以下流程：

```text
Measure
↓
Locate dominant cost
↓
Search simple/high-leverage fixes
↓
Apply only architecture-safe fixes
↓
Measure again
↓
If still failing:
Gate Reassessment
↓
USER decision
```

---

# 5. Gate Reassessment 只能发生在这些条件满足之后

只有同时满足：

```text
1. measurement methodology trustworthy

2. dominant cost understood

3. obvious duplicate work eliminated

4. obvious unnecessary initialization eliminated

5. no known simple algorithmic improvement remains

6. remaining optimization would require one or more of:
   - duplicate authority
   - new background subsystem
   - new large cache
   - bespoke font/index database
   - multiple renderer paths
   - platform special cases leaking across boundaries
   - meaningful failure-path expansion
   - architectural coupling
   - significant code complexity

7. current measured behavior is otherwise functionally acceptable
```

才允许提出：

```text
RECOMMEND GATE RELAXATION
```

---

# 6. Agent 没有权自行改变 Gate

Agent可以：

```text
recommend
```

但不能：

```text
declare new target authoritative
```

例如 warm startup仍约390ms时，可以提出：

```text
Recommend revising warm-start engineering gate
from <=180ms
to <=Xms
```

但最终只能由 USER批准。

---

# 7. 禁止“为了过 Gate 造屎山”

以下属于 Phase 11 明确禁止的优化形态：

```text
parallel startup state machine with duplicated initialization paths
temporary fallback renderer
temporary RichEdit startup editor
custom font database that duplicates fontdb authority
home-grown font indexing service
background daemon
new async runtime
new thread pool
persistent startup cache with migration burden
registry cache
AppData cache
multiple startup modes
platform-specific hacks scattered across modules
special-case if chains only for benchmark
benchmark-only behavior leaking into production
```

---

# 8. Optimization Admissibility Test

每一个非平凡性能优化，在实现前必须回答：

```text
1. What measured cost does it remove?

2. Expected gain?

3. Added lines / modules / state?

4. New failure paths?

5. New persistence/migration?

6. New synchronization?

7. Does it preserve module responsibility?

8. Can it be deleted/replaced locally later?

9. Does it reduce or increase conceptual complexity?
```

---

# 9. 默认拒绝规则

如果某项优化：

```text
measured benefit is small
+
complexity increase is material
```

则：

```text
REJECT OPTIMIZATION
```

即使它能让 benchmark再快一点。

---

# 10. 本阶段必须写入治理文档的修正

把这一原则加入：

```text
docs/plan/10_performance_reliability.md
```

明确：

> Performance gates are engineering decision thresholds subordinate to the Engineering Constitution priority order. They must not be met by introducing disproportionate architectural complexity.

---

# 11. 同时补一句

> When a performance target can no longer be reached through simple, cohesive, measurable improvements, further optimization requires an explicit gate-reassessment rather than architectural degradation.

---

# 12. 这不是降低标准

它是防止：

```text
benchmark-driven architecture drift
```

---

# 13. Feature Freeze 继续

Phase 11 禁止新增：

```text
new note features
new editing features
new export formats
new customization
new shortcuts
new docking behaviors
new cloud/network capabilities
new settings
```

---

# 14. 开始前必须读取

严格读取：

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
```

以及全部 Phase 9 / Phase 10：

```text
reports
risk reports
acceptance matrices
tasks
startup reports
RC reports
```

---

# 15. Repository Preflight

执行：

```bash
git status --short
git branch --show-current
git log -20 --oneline

cargo metadata --no-deps

cargo tree -p stickymd-core
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```text
starting commit
branch
working tree
```

不得：

```text
reset
clean
rebase
force
```

---

# 16. Phase 11 分为七部分

严格：

```text
11A — Blocker Reclassification
11B — Warm Startup Investigation
11C — Architecture-Safe Optimization Pass
11D — Performance Gate Reassessment
11E — Manual / Real-Environment Acceptance Closure
11F — Full RC Regression & Artifact Rebuild
11G — Final RC Decision
```

---

# 17. Phase 11A — Blocker Reclassification

创建：

```text
docs/report/phase-11-blocker-classification.md
```

---

# 18. 将所有剩余 blocker分三类

## Class A — Non-relaxable

包括：

```text
data loss
silent overwrite
user file deletion
security violation
crash in normal frozen v1 path
broken single-instance correctness
broken IME commit semantics
invalid authority
runtime network violation
```

这些：

```text
MUST FIX
```

---

## Class B — USER-relaxable engineering gate

例如：

```text
warm startup
cold startup
memory target
binary target
latency target
```

可以：

```text
PASS
FAIL
RECOMMEND RELAXATION
USER RELAXED
```

---

## Class C — Environment-dependent acceptance

例如：

```text
physical monitor unplug
specific IME
RDP
sleep/resume
```

状态：

```text
MANUAL PASS
NOT TESTED
FAIL
USER WAIVED
```

---

# 19. P0 / P1 必须重新汇总

目标：

```text
P0 = 0
P1 = 0
```

---

# 20. Warm Startup 是 Class B

明确：

```text
not an architecture invariant
```

---

# 21. Phase 11B — Warm Startup Investigation

当前：

```text
Warm p95 ≈392ms
Target <=180ms
```

不要立刻优化。

---

# 22. 第一步：重新确认 benchmark methodology

检查：

```text
EDITOR_READY meaning
previous process fully exited
named mutex released
tray shutdown complete
ready event uniqueness
portable dir reuse
config state
note state
Defender interference
sampling method
```

---

# 23. 使用 Rust automation CLI 作为唯一统计 authority

不得 PowerShell 自己再计算 p95。

---

# 24. 样本数

至少：

```text
50 warm
30 cold
```

---

# 25. 输出 raw samples

机器 evidence中保存：

```text
all samples
p50
p90
p95
p99
max
mean
stddev
```

---

# 26. Startup milestone必须细化到足够定位

至少：

```text
process_start
main_enter
single_instance_ready
config_ready
document_ready
window_created
display_ready
font_begin
font_end
source_layout_begin
source_layout_end
tray_ready
window_visible
editor_ready
```

---

# 27. 重点确认

是否存在：

```text
font initialization
```

dominant。

---

# 28. 同时检查 warm 慢于 cold 的原因

这是重要异常。

分析：

```text
process teardown interference
filesystem/watch cleanup
single-instance object lifetime
tray teardown
Defender behavior
allocator/process startup variance
font database behavior
ready signal bug
```

---

# 29. 不允许只说“Windows波动”

必须有 evidence。

---

# 30. 统计 cold/warm distributions

不要只比较p95。

---

# 31. Phase 11 startup report

创建：

```text
docs/report/phase-11-warm-startup-analysis.md
```

---

# 32. Dominant Cost Pareto

列：

| Component | p50 | p95 | % of startup |

---

# 33. 如果 80%以上来自系统字体初始化

明确记录。

---

# 34. Phase 11C — Architecture-Safe Optimization Pass

只实施：

> 简单、局部、高收益、低耦合的优化。

---

# 35. 优先级 1：重复工作

检查：

```text
FontSystem constructed more than once?
font database scanned more than once?
display topology queried repeatedly?
config parsed twice?
tray constructed twice?
source layout duplicated?
font fallbacks resolved repeatedly?
```

---

# 36. 优先级 2：不必要工作

在 `EDITOR_READY` 前是否做了：

```text
preview-only initialization
math font work
image codec work
unused diagnostics
release metadata
large cache initialization
unseen UI work
```

能局部延后且不改变用户第一帧正确性的：

可以延后。

---

# 37. 但 Tray readiness不应随意延后

因为 Tool Window：

```text
no taskbar
no Alt+Tab
```

Tray是主要恢复入口之一。

---

# 38. 不得先显示 inaccessible window

---

# 39. 优先级 3：简单 font improvements

允许调查：

```text
avoid duplicate fontdb scan
reuse existing FontSystem
avoid eager fallback resolution
avoid eager Preview font system
avoid eager math fonts
```

---

# 40. 允许的算法优化特点

应类似：

```text
memoization of immutable result
remove duplicate scan
lazy initialize capability at first use
reuse one long-lived object
precompute O(n) map once instead of repeatedly
replace linear search with HashMap where it is actually hot
```

这些属于：

```text
simple / beautiful / high-leverage
```

---

# 41. 不鼓励为了几十ms

把代码变成：

```text
cache invalidation labyrinth
multiple initialization phases
cross-thread font authority
```

---

# 42. 每轮 optimization最多聚焦一个 dominant cause

过程：

```text
measure
patch
benchmark
review
```

---

# 43. 每个 patch必须记录 complexity delta

创建表：

| Change | Benefit | Complexity | Keep/Reject |

---

# 44. 如果优化收益小于噪声

撤销该优化。

---

# 45. 如果优化降低 maintainability

即使快一些：

优先撤销。

---

# 46. Phase 11 优化次数不应该无限

建议：

```text
最多 2–3 个有明确证据的 architecture-safe optimization passes
```

之后进入 Gate Reassessment。

不要继续猎取毫秒。

---

# 47. 这是 stop rule

目的：

防止：

```text
benchmark rabbit hole
```

---

# 48. 禁止以下 warm startup optimization

未经 USER architecture approval：

```text
persistent serialized font database
background font indexing service
registry cache
AppData cache
two-stage fake editor
temporary fallback font renderer
parallel FontSystem implementations
custom DirectWrite text backend
custom font scanner
process daemon
pre-launch process
```

---

# 49. 如果这些才可能达到180ms

不要实现。

进入：

```text
Gate Reassessment
```

---

# 50. Phase 11D — Gate Reassessment

当合理优化完成后，重新跑：

```text
>=50 warm
>=30 cold
```

---

# 51. 如果 Warm <=180ms

```text
PASS
```

无需任何 gate change。

---

# 52. 如果 Warm >180ms

创建：

```text
docs/report/phase-11-warm-startup-gate-reassessment.md
```

---

# 53. Gate reassessment必须给出

```text
Current p50
Current p95
Current p99
Current max

Dominant cost

Optimizations tried
Benefits obtained
Rejected optimizations
Why rejected

Architecture cost required to go further

User-visible meaning of current startup

Recommendation
```

---

# 54. 不能把建议门槛拍脑袋

如果建议新 gate：

例如：

```text
<=400ms
```

必须基于真实 distribution。

---

# 55. 推荐 gate应留margin

例如 measured：

```text
p95 = 310ms
```

不应推荐：

```text
311ms
```

这种 benchmark-fitting。

---

# 56. 可以建议类似：

```text
<=350ms
<=400ms
<=450ms
```

根据数据。

---

# 57. 新 gate必须仍代表实际 UX要求

例如：

```text
editor ready under about 0.4 seconds
```

---

# 58. Agent只能：

```text
RECOMMEND USER RELAX WARM GATE TO <=Xms
```

---

# 59. USER没有批准时

Final：

```text
NOT RC READY — USER DECISION REQUIRED
```

---

# 60. 不许偷偷沿用 Phase10 392ms直接PASS

---

# 61. 同样规则适用于其它性能 gate

如果本阶段发现：

```text
memory
preview
zoom
image
```

略过目标但只有复杂hack才能压低：

先 Gate Reassessment。

---

# 62. Non-relaxable gate例外

下列不能用这个机制：

```text
user data safety
security
destructive ownership
silent overwrite
canonical authority
```

---

# 63. Phase 11E — Manual Acceptance Closure

这是本阶段另一主任务。

---

# 64. 先从 Phase10 汇总全部 NOT TESTED

生成：

```text
docs/report/phase-11-manual-acceptance.md
```

---

# 65. 不重复执行已经有可信 MANUAL PASS 的项

但 Phase10目前大部分仍未测。

---

# 66. 优先级分组

## Tier 1 — Release critical

```text
Microsoft Pinyin
WeChat Input Method
Taskbar absent
Alt+Tab absent
Alt+Tab away
Tray restore/quit
Left/Right/Top Dock
input guard
Opacity 40
Theme
Crash recovery
User asset safety
Native Export
```

---

## Tier 2 — Important environment

```text
125/150/200 DPI
dual monitor
mixed DPI
monitor disconnect
```

---

## Tier 3 — Environment optional

```text
sleep/resume
RDP
negative-coordinate physical setup
junction/symlink real OS test
```

---

# 67. Tier 1原则

如果环境存在：

必须实际执行。

---

# 68. Tier 1无法执行

必须：

```text
NOT TESTED
```

并进入 USER decision。

---

# 69. Microsoft Pinyin Final Matrix

真实：

```text
Floating
Left Dock
Right Dock
Top Dock
Split
Zoom 50
Zoom 100
Zoom 300
Opacity 40
Opacity 96
Alt+Tab away/back via click/tray
```

---

# 70. 检查

```text
candidate position
preedit
commit
selection replacement
Ctrl+Z
Esc
collapse guard
no duplicate commit
```

---

# 71. WeChat IME

同等级。

---

# 72. Tool Window Final

必须真实确认：

```text
Taskbar absent
Alt+Tab absent
focused StickyMD Alt+Tab switches away
click gives focus
tray restores
sensor restores
second instance restores
```

---

# 73. Dock Final

每个：

```text
Top
Left
Right
```

必须：

```text
capture ~24 DIP
nearest edge
tie behavior
expanded after release
focus-loss collapse
manual collapse
Esc
sensor reveal
detach
```

---

# 74. Bottom

验证：

> 拖到底边不会产生 Bottom Dock。

---

# 75. Small Window

真实：

```text
220×120 Source
220×120 Preview
220×120 Split
```

检查：

```text
controls usable
Close reachable
no catastrophic overlap
```

---

# 76. Zoom

真实：

```text
50
100
150
200
300
```

---

# 77. Ctrl+wheel

高分辨率设备如果可用。

---

# 78. Opacity

真实：

```text
40
70
96
100
```

---

# 79. 40%

确认：

```text
clickable
focusable
IME functional
not click-through
```

---

# 80. Visual Preview

至少：

```text
CJK/Latin
heading
table
code
raw HTML literal
math
image
```

---

# 81. Math visual

重点：

```text
baseline
fraction
sqrt
matrix
cases
display center
malformed error
```

---

# 82. Clipboard

真实：

```text
Ctrl+V text
Shift+Insert text
Ctrl+V screenshot
Shift+Insert screenshot
Explorer PNG
Explorer JPEG
browser image
```

---

# 83. Traditional shortcuts

真实：

```text
Ctrl+Insert
Shift+Delete
Shift+Insert
```

---

# 84. Export Dialog

真实：

```text
cancel
normal export
existing file overwrite
Chinese path
space path
```

---

# 85. Crash recovery

真实强杀。

---

# 86. Multi-monitor

如果硬件可用：

```text
secondary
dock secondary
mixed DPI
disconnect
```

---

# 87. Automated visual与manual分开

如果 Agent有截图/UI automation：

可以记录：

```text
AUTOMATED VISUAL PASS
```

但不能写：

```text
MANUAL PASS
```

---

# 88. Phase 11 不要求 Agent伪造人类验收

---

# 89. 如果执行环境完全无法做人工项

最终需要明确：

```text
implementation is technically converged,
manual release qualification remains USER-side.
```

---

# 90. Phase 11F — Full RC Regression

只有当代码修改完成：

freeze candidate commit。

---

# 91. 然后执行所有 automated release gates

包括：

```text
fmt
clippy
workspace tests
release tests
cargo deny
all smoke
phase10 smoke
phase11 smoke
package verify
```

---

# 92. Rust Automation CLI继续做 authority

不要再重构 smoke framework。

Phase10已经完成 automation consolidation。

---

# 93. Phase11只允许修：

```text
bug
missing evidence
incorrect gate
```

不做 automation redesign。

---

# 94. Final performance重新测

因为 Phase11 startup patch可能改变资源。

---

# 95. 必须重新测

```text
cold startup
warm startup
Source memory
Preview memory
Split memory
Hidden memory
idle CPU
input latency
Preview latency
Zoom relayout
image peak
```

---

# 96. 4K transient peak

继续报告。

不强制为了下降几MiB继续优化。

---

# 97. 如果仍约83MiB且稳定

可作为：

```text
known transient characteristic
```

---

# 98. 如果出现>几百MiB或OOM

则是 blocker。

---

# 99. Stress

继续：

```text
1000 dock cycles
100 tray cycles
100 zoom cycles
100 opacity
100 autosave
100 reload/conflict
image cache stress
```

---

# 100. 最终 dependency graph

不得为了 Phase11 startup加入 heavyweight runtime dependency。

理想：

```text
new runtime dependencies = 0
```

---

# 101. 如果真的需要 dependency

必须在 optimization admissibility里证明。

---

# 102. Package重新生成

Phase10 package作废。

生成新的：

```text
EXE
portable ZIP
SBOM
SHA256SUMS
```

---

# 103. Artifact hash全部重新记录

---

# 104. Phase 11 RC package仍不能叫 Stable

如果 USER未决定 tag：

```text
local-rc
```

---

# 105. Package smoke

至少：

```text
ASCII path
space path
Chinese path
same-dir instance
different-dir instances
```

---

# 106. Clean VM

如果可用：

执行。

否则：

```text
NOT TESTED
```

---

# 107. Phase 11G — Final RC Decision

创建：

```text
docs/report/phase-11-rc-readiness.md
```

---

# 108. Decision只能：

```text
RC READY

RC READY WITH USER-APPROVED RELAXATIONS

NOT RC READY
```

---

# 109. `RC READY WITH USER-APPROVED RELAXATIONS`

只能在 USER明确批准某些：

```text
performance gate
manual acceptance waiver
```

后使用。

---

# 110. Agent自己推荐放宽不等于批准

---

# 111. Final Readiness 结构

必须分别回答：

```text
Architecture
Correctness
Data Safety
Security
UX
Performance
Memory
Manual Acceptance
Packaging
Supply Chain
```

---

# 112. Architecture Review

最终回答：

1. 是否为 warm startup 引入复杂旁路？
2. 是否出现双 FontSystem authority？
3. 是否出现 persistent startup cache？
4. 是否增加background service？
5. 是否为了benchmark破坏module boundaries？
6. 是否有新增跨层调用？
7. 是否有新增 mutable global？
8. 是否出现 production benchmark special-case？
9. 是否仍高内聚？
10. 是否仍低耦合？
11. 是否有模块职责模糊？
12. 是否有明显本可简单实现却复杂化？
13. 是否保留所有数据安全边界？
14. 是否仍零WebView/Tokio/network？
15. 是否仍 core/render unsafe=0？

---

# 113. Architecture Complexity Review

对 Phase11所有 implementation changes，列：

| Change | Lines | New State | New Threads | New Dependencies | Failure Paths |

目标：

```text
minimal
```

---

# 114. 如果 startup gain只有少量，但复杂度明显增加

必须：

```text
REVERT
```

---

# 115. Cohesion Review

任何超过：

```text
~250 lines
```

新增/显著增长文件：

review职责。

---

# 116. >500 handwritten lines

仍是 hard architecture review fuse。

不能仅为了 performance一直塞。

---

# 117. Algorithm Review

对于性能路径明确回答：

```text
time complexity
space complexity
why this algorithm
simpler alternative?
```

---

# 118. “漂亮简单算法优先”

正式写入 Phase11 task：

> Prefer simple algorithms with clear invariants and good asymptotic behavior over benchmark-specific complexity.

---

# 119. 不追求微优化

例如：

```text
unsafe memcpy
custom allocator
SIMD handwritten path
manual intrusive cache
```

除非 profile明确且局部。

默认不做。

---

# 120. Warm Gate final result

报告必须明确三种之一：

```text
PASS <=180ms

FAIL — recommend USER relaxation to <=Xms

FAIL — further simple optimization remains
```

---

# 121. 如果第二种

Phase11必须停止进一步复杂优化。

等待 USER。

---

# 122. 不应同时：

```text
recommend relaxation
+
继续大规模重构
```

---

# 123. Performance gate历史

保留：

```text
original target
measured
recommendation
USER decision
```

不要覆盖历史。

---

# 124. Phase11 Documents

必须创建：

```text
docs/tasks/phase-11-rc-convergence.md

docs/report/phase-11-blocker-classification.md
docs/report/phase-11-warm-startup-analysis.md
docs/report/phase-11-manual-acceptance.md
docs/report/phase-11-performance-final.md
docs/report/phase-11-rc-readiness.md

docs/acceptance-cases/phase-11.md
```

如果 Gate reassessment需要：

```text
docs/report/phase-11-warm-startup-gate-reassessment.md
```

---

# 125. Task状态

开始：

```text
Status: In Progress
```

---

# 126. 如果技术实现完成但manual未关

```text
Status: Implementation Complete — release qualification incomplete
```

---

# 127. 如果等待 USER gate decision

```text
Status: Implementation Complete — USER gate decision required
```

---

# 128. 全部通过

```text
Status: Completed — RC ready for USER review
```

---

# 129. Final Acceptance Matrix

必须继续覆盖：

```text
AC-001..AC-030
Phase10 UX contracts
Phase11 gate/manual outcomes
```

---

# 130. Final status vocabulary

```text
PASS
FAIL
BLOCKED
USER RELAXED
USER WAIVED
NOT TESTED
```

---

# 131. Performance Relaxation 与 Manual Waiver必须区分

例如：

```text
Warm startup:
USER RELAXED

WeChat IME unavailable:
USER WAIVED
```

不能混。

---

# 132. Rust CLI Evidence

Phase11 performance数据必须由 Rust CLI生成机器证据。

---

# 133. JSON里保存

```text
commit
exe hash
suite version
samples
statistics
gate
gate source
status
```

---

# 134. 如果 USER之后放宽gate

不要改历史measurement JSON。

新增：

```text
decision metadata
```

---

# 135. Automated Commands

最终至少：

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

# 136. Automated Rust Smoke

运行实际 current CLI：

```text
ci
smoke
runtime
performance
package
readiness
```

按实际命令结构。

---

# 137. all.ps1

仍可作为 thin wrapper：

```powershell
tools/smoke/all.ps1 -Ci
```

---

# 138. No duplicated gate logic

再次确认。

---

# 139. Forbidden Dependency Scan

```bash
cargo tree | rg \
"tauri|wry|webview|cef|chromium|tokio|async-std|wgpu|reqwest|hyper|rusqlite"
```

---

# 140. Network Scan

继续。

---

# 141. Unsafe Audit

```bash
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
rg "\bunsafe\b" apps/stickymd-win/src
```

---

# 142. Core

必须：

```text
unsafe=0
```

---

# 143. Render

必须：

```text
unsafe=0
```

---

# 144. Windows unsafe

允许现有 adapter。

新增必须 justified。

---

# 145. USER Data Safety重新回归

必须：

```text
managed ownership
fake managed filename
user file
symlink/reparse
exit GC
crash recovery
```

---

# 146. Tool Window重新回归

---

# 147. Zoom重新回归

---

# 148. Traditional shortcuts重新回归

---

# 149. Dock nearest-edge重新回归

---

# 150. Opacity40重新回归

---

# 151. Package重新验证

---

# 152. Phase11 artifact

报告：

```text
EXE SHA256
ZIP SHA256
SBOM SHA256
size
commit
```

---

# 153. Phase10 artifact

明确：

```text
superseded
```

---

# 154. Git Commit建议

如果有真正代码变更：

```text
docs: calibrate Phase 11 release gate governance

perf(startup): remove measured startup redundancy

fix(rc): resolve final Windows acceptance defects

test(rc): close Phase 11 release qualification

docs: record Phase 11 RC readiness
```

---

# 155. 不为了有commit而改代码

如果 Warm gate最后只是：

```text
USER relaxation decision
```

没有必要写无价值optimization patch。

---

# 156. Git规则

```text
push = no
tag = no
release = no
```

---

# 157. Final Response格式

必须严格：

# Phase 11 Result

## Preconditions

```text
Phase 10 result
USER approval
starting commit
```

## Engineering Governance Calibration

说明：

```text
performance gates subordinate to architecture quality
non-relaxable invariants
relaxable engineering gates
```

## Blocker Classification

表：

```text
ID
class
severity
status
```

## Warm Startup Methodology

完整说明。

## Warm Startup Distribution

```text
samples
p50
p90
p95
p99
max
```

## Startup Cost Breakdown

表。

## Optimizations Considered

表：

```text
change
expected gain
actual gain
complexity
decision
```

## Optimizations Rejected

尤其记录：

```text
rejected because architecture cost disproportionate
```

## Warm Startup Final

三选一：

```text
PASS <=180ms

FAIL — recommend relaxation to <=Xms

FAIL — further simple work remains
```

## Gate Reassessment

若发生：

完整说明。

## Manual Acceptance

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

## Tool Window

```text
taskbar
Alt+Tab
Alt+Tab away
tray
sensor
second-instance
```

## Dock

```text
Top
Left
Right
24DIP
nearest edge
tie rule
no Bottom
```

## Zoom / Compact Window / Opacity

完整结果。

## Data Safety

```text
atomic save
OCC
crash
assets
GC
user-file protection
```

## Final Performance

完整。

## Final Memory

完整。

## 4K Image Peak

完整。

## Final Idle CPU

完整。

## Architecture Complexity Review

表：

```text
new state
new threads
new deps
new failure paths
```

## Cohesion / Coupling Review

结论。

## Dependency Audit

## Security

## Package

```text
EXE hash
ZIP hash
SBOM hash
size
```

## Acceptance Matrix

```text
AC-001..AC-030
Phase10 UX
```

## Remaining NOT TESTED

逐项。

## USER Decisions Required

例如：

```text
warm startup gate relaxation
remaining manual waivers
release version
tag
```

## Architecture Drift

```text
None
```

或报告。

## Verification

全部。

## Git

```text
commits
push = no
tag = no
release = no
```

## Final Recommendation

只能：

```text
RC READY

RC READY WITH USER-APPROVED RELAXATIONS

NOT RC READY
```

最后：

> Awaiting USER release/gate decision. Do not push, tag, or create a GitHub Release automatically.

---

# 158. Phase 11 Definition of Done

只有全部满足才结束：

- [ ] Performance governance修正写入plan。
- [ ] Performance gate与architecture invariant正式区分。
- [ ] Feature freeze保持。
- [ ] Remaining blockers重新分类。
- [ ] P0=0。
- [ ] P1=0，或明确block release。
- [ ] Warm benchmark methodology重新审核。
- [ ] Warm samples>=50。
- [ ] Cold samples>=30。
- [ ] EDITOR_READY定义未被放水。
- [ ] Previous process exit确认。
- [ ] ready event无stale bug。
- [ ] startup milestone完整。
- [ ] warm>cold原因得到证据分析。
- [ ] dominant cost明确。
- [ ] 仅尝试architecture-safe optimization。
- [ ] 每个optimization有before/after。
- [ ] 无收益优化被撤销。
- [ ] 无复杂度不成比例优化保留。
- [ ] 没有persistent font DB。
- [ ] 没有background service。
- [ ] 没有第二text renderer。
- [ ] 没有第二font authority。
- [ ] 没有benchmark special-case污染production。
- [ ] Warm<=180，或正式Gate Reassessment。
- [ ] Agent未自行放宽gate。
- [ ] manual acceptance汇总完成。
- [ ] Tier1 manual尽可能执行。
- [ ] Microsoft Pinyin真实状态。
- [ ] WeChat真实状态。
- [ ] Taskbar真实状态。
- [ ] Alt+Tab真实状态。
- [ ] Alt+Tab away真实状态。
- [ ] Tray真实状态。
- [ ] Top Dock真实状态。
- [ ] Left Dock真实状态。
- [ ] Right Dock真实状态。
- [ ] No Bottom真实/自动验证。
- [ ] 24DIP capture验证。
- [ ] nearest-edge验证。
- [ ] Zoom验证。
- [ ] 220×120验证。
- [ ] Opacity40验证。
- [ ] Clipboard traditional shortcuts验证。
- [ ] Native Export状态。
- [ ] Crash recovery状态。
- [ ] user-file safety PASS。
- [ ] dual monitor状态。
- [ ] mixed DPI状态。
- [ ] disconnect状态。
- [ ] final automated regression完成。
- [ ] final startup重新测。
- [ ] final memory重新测。
- [ ] final idle CPU重新测。
- [ ] final input latency重新测。
- [ ] final Preview重新测。
- [ ] 4K image peak重新记录。
- [ ] no linear leak。
- [ ] runtime dependencies无不必要增长。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] no WebView。
- [ ] no Tokio。
- [ ] no DB。
- [ ] no runtime network。
- [ ] final package重新生成。
- [ ] final hashes重新生成。
- [ ] SBOM重新生成。
- [ ] Phase10 artifact标记superseded。
- [ ] AC-001..030 final matrix完成。
- [ ] Phase10 UX final matrix完成。
- [ ] Phase11 reports完成。
- [ ] architecture complexity review完成。
- [ ] cohesion/coupling review完成。
- [ ] fmt PASS。
- [ ] clippy PASS。
- [ ] tests PASS。
- [ ] Release build PASS。
- [ ] cargo deny PASS。
- [ ] smoke PASS到正确readiness gate。
- [ ] git diff --check PASS。
- [ ] working tree clean或明确解释。
- [ ] 未push。
- [ ] 未tag。
- [ ] 未创建Release。

完成后立即停止。