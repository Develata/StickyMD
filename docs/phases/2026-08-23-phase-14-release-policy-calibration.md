# StickyMD Phase 14 — Release Policy Calibration, Startup Attribution & Qualification Closure

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–13 已完成产品实现、release qualification infrastructure 与多轮 exact-candidate qualification。

当前 Phase 13 结果：

```text
NOT RC READY
```

当前 Phase 13 exact candidate：

```text
source commit:
04fbf5f501584793090674de86ae54ecb1f222ec

version:
0.1.0

EXE SHA-256:
b65e86c596975a2c40743a1174248f817562e0de3f2c7ef078367a447d5e5fb6

ZIP SHA-256:
4fb6283cfe48dc8ec8000cd1ff09d01e0235a6f9418ea0d237fd7063962239c9

SBOM SHA-256:
cdd6a69fe54a3fc05273ff6cce5fec37868681f2ecc1240e140dae9d53d3d9ee
```

Phase 13 qualification environment：

```text
VALID
interactive
unlocked
input desktop valid
```

Phase 13 startup：

```text
Cold p95:
477.577 ms

Warm p95:
493.147 ms
```

---

# 0. Phase 14 名称

> **Phase 14 — Release Policy Calibration, Startup Attribution & Qualification Closure**

---

# 1. USER 本轮正式批准的 Release Decisions

以下全部已经获得 USER 明确批准。

不得再次询问，也不得标记为 pending。

## D14-01 — Release Version

```text
StickyMD release version:
0.1.0

Git tag when later authorized:
v0.1.0
```

状态：

```text
USER APPROVED
```

---

## D14-02 — Unsigned Release

StickyMD v0.1.0：

```text
unsigned Authenticode build accepted
```

状态：

```text
USER APPROVED
```

这是正式 release policy。

---

# 2. Unsigned Policy语义

不要：

```text
self-sign
generate development certificate
embed certificate
buy certificate
add Azure signing dependency
```

Phase14 不实现 code signing。

---

# 3. Release Docs必须说明

v0.1.0：

```text
Unsigned Windows x64 portable release.
Windows SmartScreen may show an unrecognized/unsigned application warning on first download/run.
```

保持事实性。

不要写成安全警告恐吓用户。

---

# 4. Unsigned不是 Product Failure

不得在 readiness中继续把：

```text
unsigned
```

作为 blocker。

状态：

```text
USER-APPROVED RELEASE POLICY
```

---

# 5. 后续 Signing

属于：

```text
post-v0.1.0 release infrastructure improvement
```

不是本阶段scope。

---

# 6. USER批准新的 Startup Gate体系

正式改为三层。

## Preferred Long-Term Target

```text
startup p95 <=180 ms
```

作用：

```text
post-v0.1.0 optimization direction
```

不阻塞 v0.1.0。

---

## Engineering Target

```text
startup p95 <=400 ms
```

作用：

```text
healthy native performance target
diagnostic warning threshold
```

不再作为 v0.1.0 absolute release blocker。

---

## v0.1.0 Release Hard Boundary

```text
Cold startup p95 <=550 ms
Warm startup p95 <=550 ms
```

状态：

```text
USER APPROVED
```

---

# 7. 当前 Phase13 数据

```text
Cold p95 =477.577 ms <=550
Warm p95 =493.147 ms <=550
```

因此：

> **按照新获 USER 批准的 v0.1.0 hard boundary，Phase 13 startup data本身通过 release boundary。**

---

# 8. 但不要直接复用为最终RC PASS

原因：

Phase14可能修改：

```text
qualification tooling
release policy docs
```

如果造成 source candidate SHA变化：

必须对新 exact candidate重新生成 qualification receipt。

---

# 9. 不再为了 startup数字继续优化产品

Phase14 hard rule：

```text
DO NOT MODIFY PRODUCT CODE TO CHASE 180/400 MS
```

---

# 10. Startup Attribution 的目的

Phase14仍应做一次：

```text
diagnostic attribution
```

但目的不是为了必须优化。

目的是回答：

> 当前约350ms median / 480–500ms p95的成本究竟主要属于StickyMD可控代码还是OS/font/window环境成本？

---

# 11. Attribution结论用于

```text
future optimization backlog
```

不自动改变当前release。

---

# 12. 只有发现

```text
clear correctness bug
or
simple/local/obvious duplicated >=50ms work
```

才允许考虑产品修改。

---

# 13. 若只是

```text
OS scheduling
font discovery
Windows focus
loader variance
Defender
```

则：

```text
NO PRODUCT CHANGE
```

---

# 14. Product Freeze

保持：

```text
HARD FROZEN
```

---

# 15. Phase14禁止新增

```text
new editor capability
new UI
new shortcut
new renderer
new cache architecture
new service
new persistent font DB
new thread
new runtime dependency
```

---

# 16. Performance Gate治理修正

Phase13采用：

```text
Performance FAIL
→ abort Resources
```

USER现已明确否决这种全局fail-fast策略。

---

# 17. Phase14正式修改 Qualification Orchestration

独立 evidence channel必须独立执行。

---

# 18. Global Fail-Fast 只允许以下情况

```text
P0 correctness/security defect
candidate identity mismatch
wrong executable
invalid qualification environment
receipt corruption/schema invalidity
data-safety failure
```

---

# 19. Individual Channel FAIL

例如：

```text
Performance FAIL
Runtime FAIL
Resources FAIL
```

默认行为：

```text
record FAIL
continue independent qualification channels
```

---

# 20. 示例

如果：

```text
Performance FAIL
```

仍继续：

```text
Resources
Manual acceptance
Package
other independent evidence
```

---

# 21. Readiness最后统一判断

不要途中提前丢失独立证据。

---

# 22. 依赖关系仍可fail-fast

例如：

```text
Runtime证明candidate无法可靠启动
```

如果后续Performance依赖可正常运行：

可以合理跳过。

但必须是：

```text
dependency-based abort
```

不是：

```text
any failure aborts everything
```

---

# 23. Environment Fail-Fast保持

Phase13已实现 qualification environment gate。

继续：

```text
locked / LockApp / noninteractive
→ ENVIRONMENT_BLOCKED
→ expensive GUI qualification not run
```

这个设计正确。

---

# 24. Risk-Based Manual Acceptance Policy

USER已明确批准：

> 不再采用“44 项中任何一行 NOT TESTED 都自动阻塞 release”的 policy。

---

# 25. 重要

不得删除44项 acceptance cases。

44项仍然：

```text
存在
可追踪
保留状态
```

改变的是：

```text
release risk policy
```

不是requirements。

---

# 26. 三层Manual Risk Policy

正式定义：

```text
Tier A — Release Critical Human Evidence
Tier B — Important Environment-Dependent Evidence
Tier C — Extended / Edge Platform Evidence
```

---

# 27. Tier A

v0.1.0 默认必须：

```text
MANUAL PASS
```

除非 USER明确针对具体case/group waiver。

---

# 28. Tier A包括

至少：

```text
Microsoft Pinyin
WeChat Input Method

Taskbar absence
Alt+Tab absence
Alt+Tab switching away
Tray show/hide/quit

Top Dock
Left Dock
Right Dock
No Bottom Dock
sensor basic behavior
Pin/auto-hide orthogonality

Preview basic visual
Math basic visual
Image basic visual

native Export dialog

hard-kill recovery
```

---

# 29. Traditional clipboard/zoom/math conversion

可以在同一 guided Tier A session 中真实观察。

---

# 30. Tier A不要变成44次重复操作

通过：

```text
guided manual sessions
```

完成。

---

# 31. Tier B

重要，但高度依赖环境：

```text
dual monitor
mixed DPI
monitor disconnect
real 125%
real 150%
real 200%
```

规则：

```text
environment available → test
environment unavailable → USER may waive as one explicit Tier-B group
```

---

# 32. Tier B waiver

必须记录：

```text
USER_WAIVED_TIER_B
```

不能写：

```text
PASS
```

---

# 33. Tier C

例如：

```text
RDP
sleep/resume
rare physical negative-coordinate layouts
real junction/symlink extended case
extended clean-VM permutations
```

---

# 34. Tier C release policy

如果已有：

```text
strong deterministic automated coverage
```

则：

```text
NOT TESTED
```

可以不阻塞 v0.1.0。

---

# 35. Tier C不得改PASS

状态仍保留：

```text
NOT TESTED — NON-BLOCKING FOR v0.1.0
```

---

# 36. Waiver必须版本绑定

任何：

```text
USER WAIVER
```

必须绑定：

```text
release version
candidate source SHA
case/group
```

---

# 37. 不允许永久waiver

下一版本重新评估。

---

# 38. USER可亲自运行一次15–30分钟GUI验收

因此 Phase14必须生成一个：

> **简短、顺序化、不会让USER面对44行逐条操作的 guided manual campaign**

---

# 39. 创建

```text
tools/manual/phase-14-guide.md
```

或现有最适合位置。

---

# 40. 更推荐同时由 Rust CLI支持

例如：

```text
stickymd-smoke manual guided
```

---

# 41. 不需要复杂TUI

简单：

```text
print instruction
wait for Enter
record PASS/FAIL/NOT_TESTED
```

即可。

---

# 42. Manual Guided Campaign分3个主session

目标：

```text
~15–30 minutes total under normal conditions
```

这是操作设计目标，不是自动化保证。

---

# 43. Session G1 — Editor / IME / Rendering

使用 exact Release candidate。

准备一个代表性note。

---

# 44. G1至少覆盖

```text
Microsoft Pinyin typing
WeChat IME typing

English/CJK mixed text

Ctrl+Insert
Shift+Delete
Shift+Insert

Zoom:
50
100
300
Ctrl+wheel
Ctrl+0

Math delimiter conversion:
\(...)
\[...\]

single Undo restores conversion

Preview basic rendering
math rendering
image rendering

Opacity40 + typing
```

---

# 45. G1 Pinyin重点

观察：

```text
composition
candidate position
commit
no duplicate text
no phantom commit
undo
```

---

# 46. G1 WeChat同理

---

# 47. 如果某IME没安装

记录：

```text
NOT TESTED
```

Tier A blocker需USER后续决定。

---

# 48. Session G2 — Tool Window / Tray / Dock

覆盖：

```text
StickyMD absent from taskbar
StickyMD absent from Alt+Tab
```

---

# 49. StickyMD focused：

```text
Alt+Tab once
```

必须成功切到另一个应用。

---

# 50. Tray

确认：

```text
显示/隐藏
置顶
退出
```

---

# 51. Close

```text
Close → Tray
Alt+F4 → Tray
```

---

# 52. Dock

依次：

```text
Top
Left
Right
```

---

# 53. Bottom

确认不Dock。

---

# 54. 每一edge不必完整重复全部状态机

可以这样覆盖：

### Top

```text
snap
focus loss collapse
sensor reveal
```

### Left

```text
manual collapse
Esc
detach
```

### Right

```text
Pin ON
focus away
still collapse
hover reveal/leave
```

这样覆盖所有contracts但减少重复。

---

# 55. 检查 nearest-edge

在一个corner附近做一次。

---

# 56. 220×120

只需：

```text
resize near minimum
verify Source/Preview/Split remain usable
```

不需要每mode做长测试。

---

# 57. Theme

快速：

```text
Light
Dark
System
```

---

# 58. Session G3 — Clipboard / Export / Recovery

至少：

```text
Snipping Tool screenshot paste
Explorer PNG or JPEG paste
browser image paste if convenient
```

---

# 59. 检查

```text
Preview
Undo
Redo
```

---

# 60. Export

```text
Ctrl+Shift+S
native dialog
space/Chinese path if practical
```

---

# 61. Hard-kill

准备dirty text。

强制kill。

重新打开。

检查：

```text
canonical note intact
recovery behavior correct
```

---

# 62. User asset safety

可与自动化为主。

如果G3方便：

确认一个手工图片未被删除。

---

# 63. Manual Sessions不删除underlying cases

每个G1/G2/G3 observation必须映射回：

```text
phase-12/13/14 acceptance case IDs
```

---

# 64. 一个human action可以支持多个case

前提：

```text
观察事实确实相同
```

---

# 65. 不得一个：

```text
looks good
```

直接PASS 20项。

---

# 66. Manual Receipt

继续exact artifact binding。

至少：

```text
source SHA
EXE SHA
version
Windows build
session
case result
```

---

# 67. Phase14 Startup Attribution

在修改release gate之后，仍做一次诊断性 attribution。

---

# 68. 优先使用 Windows官方 tracing

如果当前环境已有：

```text
WPR/WPA
```

使用。

---

# 69. 不要求为了Phase14永久加入Windows Performance Toolkit依赖

这是：

```text
developer diagnostic
```

---

# 70. Trace当前 exact binary

目标至少选择：

```text
one representative fast launch
one representative median launch
one representative slow launch
```

---

# 71. 分析类别

```text
StickyMD user-mode CPU
font discovery
shaping
Windows loader
filesystem IO
thread waits
window creation
focus/foreground
Defender/system work if visible
```

---

# 72. 不要求毫秒精确归因到每函数

需要的是：

```text
dominant categories
```

---

# 73. Attribution Decision

只能：

```text
NO PRODUCT OPTIMIZATION NEEDED
```

或：

```text
ONE SIMPLE LOCAL OPTIMIZATION JUSTIFIED
```

或：

```text
STARTUP CORRECTNESS ISSUE FOUND
```

---

# 74. 如果只是系统/字体成本

```text
NO PRODUCT CHANGE
```

---

# 75. 如果发现简单重复工作

实现前必须满足：

```text
local
cohesive
no new authority
no persistence
no new thread
no new runtime dependency
large measured benefit
```

---

# 76. 因为550ms已经是release boundary

即使发现：

```text
simple 10ms improvement
```

也不要改candidate。

放future backlog。

---

# 77. 只有明显：

```text
>=50ms
```

且代码非常简单才值得release前改。

即便如此也必须审慎。

---

# 78. Phase14 Automation Execution Policy

正式改变：

```text
collect all independent evidence
```

---

# 79. 推荐顺序

```text
Environment
Release/package
Headless CI
Runtime
Performance
Resources
Manual
Readiness
```

---

# 80. 某channel FAIL

记录。

继续其它独立channel。

---

# 81. Resources必须运行

即使Performance：

```text
>550
```

只要：

```text
environment valid
runtime able to execute
candidate identity valid
```

---

# 82. Manual也可以继续

如果失败不需要立刻改binary。

---

# 83. 只有确认要修改product candidate

才停止manual，避免生成stale receipt。

---

# 84. GitHub Actions Test Stratification

Phase14正式记录三层：

```text
Layer 1 — GitHub-hosted deterministic CI
Layer 2 — Dedicated Windows qualification lab
Layer 3 — Human interaction acceptance
```

---

# 85. Layer 1 — GitHub-hosted

当前public repo使用 GitHub-hosted Windows。

适合：

```text
fmt
clippy
cargo test
cargo deny
headless smoke
state reducer
geometry
semantic Markdown/math
package
SBOM
checksum
fault injection
deterministic runtime-independent tests
```

---

# 86. 可以运行粗粒度performance smoke

例如：

```text
detect catastrophic regression
```

但不要把：

```text
550ms startup absolute gate
```

绑定 GitHub-hosted runner。

---

# 87. Hosted Performance允许的是

例如：

```text
startup did not regress >2x relative baseline
```

若未来建立稳定统计。

Phase14不强制实现。

---

# 88. Layer 2 — Dedicated Windows Lab

未来用于：

```text
absolute startup timing
PWS
Private Bytes
idle CPU
real shell automation
fixed DPI
possibly multi-monitor
```

---

# 89. 但 public repo禁止直接使用普通self-hosted runner

安全policy：

```text
DO NOT attach a privileged persistent self-hosted runner directly to public StickyMD workflows.
```

---

# 90. 推荐future designs

### Option A — Pull-Based Local Lab

```text
GitHub-hosted builds exact candidate
→ workflow artifact
→ local trusted Windows lab downloads exact artifact
→ Rust CLI qualifies it
→ evidence returned/uploaded manually
```

推荐。

---

# 91. Option B — Private Release Lab Repo

```text
private StickyMD-release-lab
→ protected manual workflow
→ dedicated self-hosted Windows runner
```

也可接受。

---

# 92. Phase14不要求搭建self-hosted runner

只需：

```text
document future test architecture
```

避免扩大release scope。

---

# 93. Layer 3 — Human

专门负责：

```text
IME
visual appearance
Alt+Tab subjective/real shell presence
Tray
Dock feel
native dialog
```

---

# 94. 人工层尽量小

这就是risk-based policy的目的。

---

# 95. Version Decision

正式更新：

```text
0.1.0 = USER APPROVED
```

---

# 96. Unsigned Decision

正式：

```text
unsigned v0.1.0 = USER APPROVED
```

---

# 97. 从 readiness blockers移除这两项

---

# 98. Release Documentation

README / release notes加入：

```text
StickyMD v0.1.0 is distributed as an unsigned Windows executable.
Windows SmartScreen may prompt on first run.
```

---

# 99. 不给不必要复杂教程

可简短说明：

```text
If you trust the release downloaded from the official GitHub repository, Windows may require “More info” → “Run anyway”.
```

---

# 100. 不写关闭Defender

绝对禁止建议：

```text
disable antivirus
disable SmartScreen globally
```

---

# 101. Package

仍：

```text
unsigned
```

---

# 102. SBOM/checksums/attestation仍提供供应链信任

强调：

```text
SHA-256
GitHub attestation
source provenance
```

---

# 103. Phase14 exact candidate规则

先完成所有需要commit的：

```text
policy docs
qualification tooling
risk tier mapping
guided manual helper
orchestration change
```

---

# 104. 然后：

```text
freeze new exact candidate SHA
```

---

# 105. 之后动态qualification evidence全部：

```text
dist/evidence/
```

---

# 106. 不commit动态结果

避免candidate/evidence循环。

---

# 107. Phase14 source deliverables

freeze前：

```text
docs/tasks/phase-14-release-gate-calibration.md
docs/acceptance-cases/phase-14.md
docs/report/phase-14-release-policy.md
docs/report/phase-14-startup-attribution-plan.md
docs/reference/qualification-execution-model.md
```

文件名按当前治理习惯微调。

---

# 108. 更新

```text
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md
docs/release-checklist.md
AGENTS.md
```

---

# 109. Performance Plan必须写成

```text
Preferred startup p95:
<=180 ms

Engineering target:
<=400 ms

v0.1.0 release hard boundary:
<=550 ms
```

---

# 110. 清楚说明：

```text
Preferred/engineering target misses do not automatically block v0.1.0.
Hard boundary does.
```

---

# 111. Testing Plan必须记录

```text
independent qualification channels do not globally fail-fast on ordinary gate failure
```

---

# 112. Risk-based manual policy进入Plan

不是只写report。

因为这是长期release testing policy。

---

# 113. 但 Tier mapping可以是acceptance projection

Plan只写规则。

---

# 114. Phase14 automated qualification

新exact candidate至少：

```text
Release/package
Headless CI
Runtime
Performance
Resources
```

全部采集。

---

# 115. Performance最终判断

```text
Cold p95 <=550
Warm p95 <=550
```

---

# 116. 若<=400

记录：

```text
engineering target met
```

---

# 117. 若400–550

记录：

```text
engineering target missed
v0.1.0 release boundary met
```

这不阻塞release。

---

# 118. 若>550

```text
release hard boundary FAIL
```

但继续Resources等独立evidence。

---

# 119. Resources继续现有hard budgets

不要放宽。

当前历史数据已经远低于预算。

---

# 120. Resource suite中途lock

仍：

```text
environment invalidates receipt
```

停止当前resources。

不影响已经完整的其它channel receipt。

---

# 121. Readiness

最终读取：

```text
exact release
exact ci
exact runtime
exact performance
exact resources
manual tier results
USER decisions
remote status
```

---

# 122. Tier-based readiness逻辑

类似：

```text
Tier A FAIL / NOT_TESTED:
block unless explicit USER waiver

Tier B NOT_TESTED:
USER group disposition required

Tier C NOT_TESTED:
non-blocking when automated contract exists
```

---

# 123. 不允许 blanket：

```text
manual_missing > 0 => fail
```

旧逻辑废止。

---

# 124. Manual status仍逐行展示

透明度不能降低。

---

# 125. 44 cases仍报告统计

例如：

```text
MANUAL PASS 18
NOT TESTED 26
```

但 readiness根据Tier。

---

# 126. 不通过减少case数量“改善数字”

---

# 127. Clean VM

建议 Tier B 或 Tier C？

对于v0.1.0：

定为：

```text
Tier B
```

如果有环境尽量测试。

没有环境可USER waiver。

---

# 128. Junction/symlink real case

定：

```text
Tier C
```

因为已有强 automated ownership/boundary coverage。

---

# 129. Sleep/RDP

Tier C。

---

# 130. Multi-monitor

Tier B。

---

# 131. Mixed DPI

Tier B。

---

# 132. Pinyin/WeChat

Tier A。

---

# 133. Preview/Math/Image basic visual

Tier A。

---

# 134. Export dialog

Tier A。

---

# 135. Hard kill recovery

Tier A。

---

# 136. Tool Window / Tray / basic Dock

Tier A。

---

# 137. Detailed DPI permutations

Tier B。

---

# 138. Phase14 guided manual helper

必须最终输出receipt映射：

```text
G1 → case IDs
G2 → case IDs
G3 → case IDs
```

---

# 139. 如果USER实际执行

由USER观察结果录入。

---

# 140. Agent不要代替USER视觉判断。

---

# 141. Startup attribution工具

WPR trace文件不要commit。

---

# 142. 存：

```text
dist/evidence/startup-traces/
```

或临时目录。

---

# 143. 只把总结写到receipt。

---

# 144. 如果WPR不可用

不要为此阻塞Phase14。

使用现有 milestone attribution。

状态：

```text
ETW attribution NOT AVAILABLE
```

即可。

---

# 145. ETW不是release hard gate

它是diagnostic。

---

# 146. No Product Change default

强烈默认：

```text
Phase14 product runtime code delta = 0
```

---

# 147. Tooling changes不能进入runtime。

---

# 148. 新 runtime dependencies

预期：

```text
0
```

---

# 149. Qualification CLI可扩展

但不要再大重构。

---

# 150. Phase14之后不要再创建新 qualification architecture

这应该是最终release policy。

---

# 151. GitHub-hosted workflow

现有release workflow继续。

---

# 152. 不加入绝对本机performance gate

GitHub-hosted：

```text
does not decide <=550ms
```

---

# 153. Local trusted qualification receipt

决定absolute performance。

---

# 154. Future dedicated lab

记录proposal，不阻塞v0.1.0。

---

# 155. Remote workflow仍用于

```text
clean build
package
SBOM
checksum
attestation
artifact reproduction
```

---

# 156. Push/tag仍未自动授权

Phase14没有新的remote授权。

---

# 157. Local readiness完成后

输出：

```text
LOCAL RC READY — PUSH AUTHORIZATION REQUIRED
```

---

# 158. 版本/unsigned已经不再是USER blocker

---

# 159. 剩余可能USER决定

主要：

```text
Tier B waiver
Tier A exceptional waiver if any
push
tag
publish
```

---

# 160. 不自动建议waive Tier A

先让USER跑guided campaign。

---

# 161. Phase14 Verification

freeze前：

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
cargo deny check
git diff --check
```

---

# 162. Freeze candidate后重新：

```text
release
all --ci
runtime
performance
resources
readiness
```

---

# 163. Independent channel behavior测试

必须新增unit/integration tests：

```text
Performance FAIL does not skip Resources
Resources FAIL does not erase Performance
Runtime ordinary gate FAIL does not erase other independent receipts
Environment BLOCKED aborts dependent expensive suites
Identity mismatch globally blocks
P0/data safety globally blocks
```

---

# 164. Manual tier readiness tests

至少：

```text
Tier A NOT_TESTED → blocked
Tier A PASS → eligible
Tier A USER_WAIVED → eligible but reported waiver

Tier B NOT_TESTED → USER disposition required
Tier B group waived → eligible

Tier C NOT_TESTED + automation PASS → non-blocking
```

---

# 165. Waiver version binding test

v0.1.0 waiver：

不能自动适用于：

```text
v0.1.1
```

---

# 166. Candidate binding

Manual receipt绑定exact candidate。

---

# 167. Unsigned policy receipt

记录：

```text
version =0.1.0
policy = unsigned accepted
USER APPROVED
```

---

# 168. 不需要signed-exe fields。

---

# 169. Package不因unsigned失败。

---

# 170. Phase14 Final Result Format

必须：

# Phase 14 Result

## USER Decisions Applied

```text
version 0.1.0:
USER APPROVED

unsigned v0.1.0:
USER APPROVED

startup release boundary:
<=550 ms USER APPROVED

risk-based manual policy:
USER APPROVED

independent evidence collection:
USER APPROVED
```

## Candidate

```text
source SHA
EXE SHA
ZIP SHA
SBOM SHA
Cargo.lock SHA
```

## Architecture

```text
product code delta
runtime dependency delta
P0
P1
drift
```

## Startup Attribution

```text
method:
WPR/ETW or milestone

dominant costs:
...
```

Decision：

```text
NO PRODUCT OPTIMIZATION NEEDED
```

或真实其它结果。

## Startup Qualification

表：

```text
Cold p50/p95
Warm p50/p95

preferred 180
engineering 400
release boundary 550
```

结果分别列：

```text
preferred
engineering
release
```

## Qualification Channels

| Channel | Result | Exact |
|---|---|---|
| Release | | |
| CI | | |
| Runtime | | |
| Performance | | |
| Resources | | |

明确：

```text
ordinary channel FAIL did/did not abort other channels
```

## Manual Risk Policy

```text
Tier A:
Tier B:
Tier C:
```

列case映射。

## Guided Manual Campaign

```text
G1
G2
G3
```

结果。

## Manual Statistics

```text
MANUAL PASS
NOT TESTED
FAIL
USER WAIVED
NON-BLOCKING NOT TESTED
```

## Tier A Result

必须明确。

## Tier B Result

必须明确。

## Tier C Result

必须明确。

## Resources

完整。

## Performance

完整。

## Package

完整。

## Unsigned Release

```text
policy accepted
SmartScreen notice added
no self-signing
```

## GitHub Actions Strategy

```text
GitHub-hosted:
deterministic CI/package

Absolute performance:
local trusted Windows qualification

Future:
dedicated Windows lab
```

## Readiness

列剩余blockers。

## Remote

```text
push
workflow
tag
release
```

## Recommendation

只能：

```text
NOT RC READY

LOCAL RC READY — TIER-B/USER DECISION REQUIRED

LOCAL RC READY — PUSH AUTHORIZATION REQUIRED
```

最后：

> Do not tag or publish. Continue release handoff only after explicit USER authorization.

---

# 171. Phase14 Definition of Done

- [ ] version 0.1.0记录为USER APPROVED。
- [ ] unsigned v0.1.0记录为USER APPROVED。
- [ ] unsigned从release blocker移除。
- [ ] SmartScreen factual notice加入release docs。
- [ ] 不创建self-signed certificate。
- [ ] 不增加signing dependency。
- [ ] preferred startup180保留。
- [ ] engineering target400保留。
- [ ] v0.1.0 hard boundary550记录为USER APPROVED。
- [ ] 400 miss不自动block v0.1.0。
- [ ] >550才是startup release failure。
- [ ] 不再追逐startup性能。
- [ ] startup attribution完成或明确不可用。
- [ ] attribution不为了数字推动复杂重构。
- [ ] qualification global fail-fast规则修正。
- [ ] Performance ordinary FAIL不阻止Resources。
- [ ] Resources ordinary FAIL不抹去其它evidence。
- [ ] Environment invalid仍global stop。
- [ ] Candidate identity invalid仍global stop。
- [ ] P0/data safety仍global stop。
- [ ] 44项manual requirements保留。
- [ ] manual cases全部Tier化。
- [ ] Tier A policy实现。
- [ ] Tier B policy实现。
- [ ] Tier C policy实现。
- [ ] waiver版本绑定。
- [ ] guided G1创建。
- [ ] guided G2创建。
- [ ] guided G3创建。
- [ ] USER可在15–30分钟常规情况下完成核心campaign。
- [ ] manual receipt exact-artifact binding。
- [ ] Pinyin Tier A。
- [ ] WeChat Tier A。
- [ ] ToolWindow/Tray/Dock Tier A。
- [ ] Preview/Math/Image basic visual Tier A。
- [ ] Export Tier A。
- [ ] hard-kill recovery Tier A。
- [ ] dual monitor/mixed DPI Tier B。
- [ ] Clean VM Tier B。
- [ ] sleep/RDP Tier C。
- [ ] junction/symlink extended Tier C。
- [ ] GitHub-hosted testing scope文档化。
- [ ] absolute550ms gate不放GitHub-hosted。
- [ ] future dedicated Windows lab策略记录。
- [ ] public repo direct persistent self-hosted runner不采用。
- [ ] runtime dependencies delta=0。
- [ ] product architecture不变。
- [ ] exact candidate freeze。
- [ ] Release exact receipt。
- [ ] CI exact receipt。
- [ ] Runtime exact receipt。
- [ ] Performance exact receipt。
- [ ] Resources exact receipt。
- [ ] Independent channel tests PASS。
- [ ] readiness tier tests PASS。
- [ ] final package重新生成。
- [ ] SBOM重新生成。
- [ ] hashes重新生成。
- [ ] worktree clean。
- [ ] 未push。
- [ ] 未tag。
- [ ] 未release。

完成当前可执行部分后停止。
