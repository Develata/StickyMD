# StickyMD Phase 12 — Final Release Qualification, Evidence Binding & v0.1.0 Release Handoff

你现在位于 StickyMD 本地 Git 仓库根目录。

这是 StickyMD v0.1.0 计划中的**最后一个阶段**。

Phase 0–11-B 已完成产品实现。

当前 origin repository：

```text
Develata/StickyMD
```

当前已知 origin `main` HEAD：

```text
d6ad84a126f218cb22cdcd4a93ff10e03102939c
```

当前 workspace version：

```text
0.1.0
```

当前 Phase 11 状态：

```text
Implementation complete
NOT RC READY
```

---

# 0. Phase 12 名称

> **Phase 12 — Final Release Qualification, Evidence Binding & v0.1.0 Release Handoff**

---

# 1. Phase 12 的本质

Phase 12 **绝对不是新的产品开发阶段**。

本阶段只负责：

```text
治理状态同步
        ↓
最终 release gate disposition
        ↓
冻结 exact release-source commit
        ↓
生成 exact local RC
        ↓
绑定人工验收 evidence
        ↓
远端 workflow 验证
        ↓
tag 前 USER gate
        ↓
GitHub draft release
        ↓
下载远端 artifact 再验证
        ↓
publish 前 USER gate
```

---

# 2. Phase 12 是最后 Phase

本阶段之后：

```text
不得自动创建 Phase 13
```

如果发现：

### P0 / P1 bug

在 Phase 12 内：

```text
修复
→ invalidate candidate
→ rebuild candidate
→ rerun affected qualification
```

如果发现：

### architecture-level disproof

才允许：

```text
STOP — architecture review required
```

而不是机械创建 Phase 13。

---

# 3. Feature Freeze 是绝对状态

Phase 12 不允许新增任何产品能力。

禁止：

```text
new editor features
new shortcuts
new Markdown features
new customization
new docking rules
new theme choices
new export format
new asset behavior
new settings
new shell behavior
new platform support
```

---

# 4. 允许的产品代码修改只有

```text
P0 bug fix
P1 bug fix
release-blocking correctness fix
release-blocking interoperability fix
```

P2/P3：

原则上：

```text
document
defer
```

不要为了第一版“最后再磨一下”不断改代码。

---

# 5. Phase 12 不继续追逐性能数字

Phase 11 已经证明：

```text
Warm startup current:
p95 ≈ 311.353 ms
```

剩余主要成本来自：

```text
native font discovery
cosmic-text shaping
Windows focus/native guards
```

进一步强制压向：

```text
180 ms
```

预期需要：

```text
persistent font DB
second renderer
background service
premature ready signal
```

等架构代价。

这些方案已被合理拒绝。

---

# 6. Warm Gate 当前治理状态

当前 authoritative gate：

```text
warm startup p95 <=180 ms
```

当前结果：

```text
FAIL
```

当前 Agent recommendation：

```text
revise release boundary to:
warm startup p95 <=400 ms
```

但：

```text
USER APPROVAL NOT YET RECORDED
```

---

# 7. Phase 12 绝不能自行批准 400 ms

除非 USER 明确 steering：

```text
USER APPROVES warm startup p95 release boundary <=400 ms.
```

或同等明确语言。

否则 final status中必须保持：

```text
Warm Gate:
BLOCKED — USER DECISION REQUIRED
```

---

# 8. 如果 USER 批准 400 ms

这不是：

```text
waive a failure
```

而应该正式记录为：

```text
USER-APPROVED ENGINEERING GATE RECALIBRATION
```

因为 Phase 11 已完成：

```text
measurement
optimization attempts
architecture-cost review
gate reassessment
```

---

# 9. Gate history必须保留

不得抹除：

```text
original target = 180 ms
measured ≈311 ms
recommended revised boundary =400 ms
USER decision
```

---

# 10. 不得修改历史 report 来伪装原来就是 400

应：

```text
append decision
```

---

# 11. Phase 12 开始前读取

严格执行所有适用 `AGENTS.md`。

完整阅读：

```text
AGENTS.md

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

并完整读取：

```text
Phase 9 final reports
Phase 10 final reports
Phase 11 reports
Phase 11-B reports
all current release checklist
all acceptance matrices
all open RISK reports
```

---

# 12. Repository Preflight

执行：

```bash
git status --short
git branch --show-current
git log -20 --oneline

git rev-parse HEAD
git rev-parse origin/main

cargo metadata --no-deps
```

记录：

```text
local HEAD
origin/main HEAD
ahead/behind
working tree
```

---

# 13. 当前预期

如果没有新的 USER/local change：

```text
local HEAD = d6ad84a...
origin/main = d6ad84a...
```

如果不一致：

不要猜。

记录真实状态。

---

# 14. Phase 12 分为九部分

严格顺序：

```text
12A — Governance Synchronization
12B — Release Gate Decision Ledger
12C — Exact Candidate Freeze
12D — Human Evidence Binding
12E — Final Local Qualification
12F — Remote Workflow Validation
12G — Tag / Draft Release Gate
12H — Downloaded Artifact Qualification
12I — Final Publish Handoff
```

---

# 15. Phase 12A — Governance Synchronization

首先修正当前治理状态漂移。

---

# 16. Root AGENTS.md

当前摘要仍写：

```text
Phase 10 implementation complete
```

必须更新。

建议：

```text
Phase 11/11-B implementation complete.
Phase 12 release qualification is in progress.
The product feature set is frozen.
```

---

# 17. 不能写

```text
RC READY
stable
released
```

除非后续 gate真正关闭。

---

# 18. 更新 Phase State

至少检查：

```text
README.md
AGENTS.md
docs/overview/*
docs/release-checklist.md
```

是否有 Phase10/旧状态漂移。

---

# 19. 不要大规模重写历史 report

历史 report保持历史。

只修：

```text
current-state summary
```

---

# 20. 创建 Phase 12 Task

```text
docs/tasks/phase-12-final-release-qualification.md
```

---

# 21. Task 初始状态

```text
Status: In Progress
```

---

# 22. 创建 Phase 12 release qualification matrix

```text
docs/acceptance-cases/phase-12.md
```

它是：

> release-level projection

不是重新定义产品 requirements。

---

# 23. Phase 12B — Release Decision Ledger

创建：

```text
docs/report/phase-12-release-decisions.md
```

---

# 24. Decision Ledger 至少包含

```text
Warm startup release boundary
Release version
Manual acceptance waivers
Unsigned build policy
Push authorization
Tag authorization
Draft release authorization
Publish authorization
```

---

# 25. 每项只有

```text
PENDING
USER APPROVED
USER REJECTED
NOT APPLICABLE
```

---

# 26. Agent不能自己填 USER APPROVED

---

# 27. Release Version

当前：

```text
Cargo workspace = 0.1.0
```

Candidate tag：

```text
v0.1.0
```

但是：

> workspace version存在 ≠ USER已经批准 tag version。

---

# 28. USER没有明确批准前

状态：

```text
Release Version:
PENDING USER APPROVAL
```

---

# 29. Phase 12 不改 version 到 1.0.0

绝对禁止。

---

# 30. Manual Waiver

任何：

```text
NOT TESTED
```

如果 USER决定接受：

记录：

```text
USER WAIVED
```

不能写：

```text
PASS
```

---

# 31. Warm gate approval后的 Plan update

如果 USER批准：

在：

```text
docs/plan/10_performance_reliability.md
```

加入当前 release boundary：

```text
Warm startup:
preferred target <=180 ms
v0.1.0 release hard boundary <=400 ms
```

---

# 32. 这是我建议的两层形式

而不是删除180：

```text
Preferred engineering target:
<=180 ms

v0.1.0 release boundary:
<=400 ms
```

这样以后仍有优化方向。

---

# 33. Cold 同理

如果已有：

```text
preferred <=300
approved release boundary <=400
```

保持历史。

---

# 34. Phase 12C — Exact Candidate Freeze

当前最大证据问题之一：

```text
packaged candidate =23d2a410...
current HEAD =d6ad84a...
```

Phase12必须彻底消除这种 ambiguity。

---

# 35. 先完成所有 source-controlled修改

包括：

```text
AGENTS state summary
Phase12 task
release decision template
acceptance template
release scripts bug fixes if any
```

---

# 36. 在 candidate freeze 之后

不得继续修改：

```text
apps/
crates/
Cargo.toml
Cargo.lock
rust-toolchain.toml
build scripts
release scripts
.github/workflows/
runtime resources
```

除非 candidate被明确 invalidated。

---

# 37. Product Candidate Commit

定义：

```text
RELEASE_SOURCE_COMMIT
```

---

# 38. Freeze条件

必须：

```text
working tree clean
all automated source tests pass
no known P0/P1
```

---

# 39. Candidate Commit 可以包含

最终治理 current-state文档。

这样避免：

```text
candidate commit
→ docs commit
→ HEAD mismatch
```

---

# 40. Freeze之后的人工 evidence

**不得要求再commit到release source。**

这是 Phase12重要治理改进。

---

# 41. 为什么

否则永远出现：

```text
tested artifact commit A
evidence report commit B
final HEAD B
exact artifact belongs to A
```

循环。

---

# 42. Phase12引入 Release Evidence Receipt

人工/远端验证 evidence 应绑定 artifact，而不是通过修改source commit绑定。

---

# 43. 建议输出

```text
dist/evidence/
├─ release-candidate.json
├─ automated-qualification.json
├─ manual-acceptance.json
├─ remote-workflow.json
└─ downloaded-artifact-smoke.json
```

---

# 44. `dist/` 不commit

继续。

---

# 45. Release Candidate Receipt

至少：

```json
{
  "schema_version": 1,
  "source_commit": "...",
  "version": "0.1.0",
  "cargo_lock_sha256": "...",
  "exe_sha256": "...",
  "zip_sha256": "...",
  "sbom_sha256": "...",
  "rustc": "...",
  "target": "x86_64-pc-windows-msvc"
}
```

---

# 46. Receipt由 Rust automation CLI 生成

不要手工拼 JSON。

---

# 47. Evidence receipt是 automation authority的一部分

继续遵循 Phase10/11：

```text
Rust CLI owns logic
PowerShell thin wrapper
```

---

# 48. 不增加 runtime dependency

tools only。

---

# 49. Exact package

从：

```text
RELEASE_SOURCE_COMMIT
```

生成：

```text
StickyMD-0.1.0-local-rc-<shortsha>-windows-x64-portable.zip
```

---

# 50. 再生成

```text
SBOM.spdx.json
SHA256SUMS.txt
```

---

# 51. 验证

```text
package allowlist
PE x64
manifest
PerMonitorV2
asInvoker
licenses
no note/
no user data
no PDB in portable package
no proprietary fonts
```

---

# 52. Local candidate ID 一旦生成

以后 manual acceptance全部绑定：

```text
source commit
EXE hash
ZIP hash
```

---

# 53. 如果任何产品/release-input代码修改

candidate：

```text
INVALIDATED
```

必须：

```text
rebuild
new hash
new manual receipt
```

---

# 54. Docs-only evidence不再要求修改 source commit

使用：

```text
dist/evidence
```

和最终 USER-facing report。

---

# 55. Phase 12D — Human Evidence Binding

Phase11有：

```text
21 Phase11 manual NOT TESTED
5 Phase11-B manual NOT TESTED
```

Phase12必须汇总为一个最终 manual matrix。

---

# 56. 不要求人工手改 Markdown表格

扩展 Rust CLI：

```text
stickymd-smoke acceptance manual
```

或当前CLI等价命令。

---

# 57. 这个命令不是自动验收

它只是：

> **human receipt recorder**

---

# 58. 命令行为

逐项显示：

```text
Case ID
What to do
Expected result
Current exact artifact identity
```

然后明确接受：

```text
PASS
FAIL
NOT TESTED
```

---

# 59. 只允许 human/operator显式输入

不得根据 process status自动填写 MANUAL PASS。

---

# 60. Manual receipt输出

```text
dist/evidence/manual-acceptance.json
```

---

# 61. 每项至少：

```json
{
  "case_id": "...",
  "status": "MANUAL_PASS",
  "source_commit": "...",
  "exe_sha256": "...",
  "environment": {...},
  "note": "..."
}
```

---

# 62. 不需要存人工身份PII

例如：

```text
operator = "USER"
```

即可。

---

# 63. 环境信息

自动采集可以包括：

```text
Windows build
CPU
RAM
GPU
monitor count
DPI
```

---

# 64. IME Version

如果能自动安全读：

记录。

否则人工 note。

---

# 65. Phase12 Manual Tier A — 必须优先

至少：

```text
Microsoft Pinyin
WeChat IME

Taskbar absent
Alt+Tab absent
Alt+Tab switches away correctly

Tray
Close→Tray
Tray Show
Tray Quit

Top Dock
Left Dock
Right Dock
No Bottom

24 DIP capture
nearest-edge/tie behavior
sensor reveal
Pin/auto-hide orthogonality

220×120 Source
220×120 Preview
220×120 Split

Zoom
Opacity 40
Theme

Preview visual
Math visual
Image visual

Traditional clipboard shortcuts
real image clipboard

native Export dialog
hard-kill recovery
```

---

# 66. Tier B

```text
dual monitor
mixed DPI
monitor disconnect
125/150/200%
```

---

# 67. Tier C

```text
sleep/resume
RDP
physical negative-coordinate layout
junction/symlink real test
```

---

# 68. Tier A未测试

原则上：

```text
BLOCK RELEASE
```

除非 USER逐项/按明确组批准 waiver。

---

# 69. 不要自动建议“waive all manual”

必须让 USER看清具体未测内容。

---

# 70. Pinyin必须真实

模拟不算。

---

# 71. WeChat必须真实

如果环境没有：

```text
NOT TESTED
```

---

# 72. Taskbar / Alt+Tab

Win32 style readback：

```text
AUTOMATED PASS
```

不等于：

```text
MANUAL PASS
```

---

# 73. Tool Window人工特别检查

必须：

```text
StickyMD absent from taskbar
StickyMD absent from Alt+Tab
focused StickyMD → Alt+Tab switches away
clicking StickyMD → focus returns
IME works after focus
tray restores
sensor restores
second instance restores
```

---

# 74. Pin正交

真实：

```text
Docked
Pin ON
focus away
```

仍：

```text
700ms collapse
```

---

# 75. Delimiter conversion

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

一次Undo全部恢复。

---

# 76. Manual Evidence Integrity

manual receipt必须检查：

```text
receipt EXE hash
==
current candidate EXE hash
```

否则：

```text
STALE RECEIPT
```

---

# 77. stale receipt不得计入 release qualification

---

# 78. Phase 12E — Final Local Qualification

Exact candidate完成后重新跑：

```text
all automated release gates
```

---

# 79. 至少：

```bash
cargo fmt --check

cargo clippy --workspace --all-targets -- -D warnings

cargo test --workspace --locked

cargo build --workspace --release --locked

cargo test -p stickymd-core --release --locked
cargo test -p stickymd-render --release --locked
cargo test -p stickymd-win --release --locked

cargo deny check
```

---

# 80. Rust CLI

运行：

```text
all --ci --json
performance
runtime
release
package
readiness
```

按当前真实命令。

---

# 81. Phase12 readiness CLI 必须读取

```text
release candidate receipt
automated receipt
manual receipt
USER decision ledger
```

---

# 82. 它不能自己批准 USER decision

---

# 83. Final Automated Performance

重新记录：

```text
cold startup
warm startup
Source PWS
Preview PWS
Split PWS
Hidden PWS
idle CPU
input latency
preview latency
image peak
```

---

# 84. 不再针对通过的性能进行微优化

只：

```text
measure
record
```

---

# 85. Warm gate

如果 USER批准400：

当前类似：

```text
311ms
```

应：

```text
PASS <=400
```

---

# 86. 仍保留

```text
preferred target <=180
```

为后续优化目标。

---

# 87. 如果USER没批准

readiness必须返回：

```text
BLOCKED
```

---

# 88. Final Architecture Audit

只读检查：

```text
DocumentState authority
Config authority
Window reducer authority
asset ownership
bounded queues/caches
generation stale drop
```

---

# 89. Core/Render

必须：

```text
unsafe = 0
```

---

# 90. 禁止依赖

继续扫描：

```text
WebView
Tauri runtime
Tokio
DB
runtime network
```

---

# 91. Final destructive IO audit

检查全部：

```text
remove
rename
delete
move
```

paths。

---

# 92. 最终 P0/P1

必须：

```text
P0 = 0
P1 = 0
```

才能进入 tag approval gate。

---

# 93. Phase 12F — Remote Workflow Validation

当前 repo已有：

```text
.github/workflows/release.yml
```

并支持：

```text
workflow_dispatch
```

---

# 94. Phase12不要重写 workflow，除非发现真实bug

当前已经包含：

```text
dependency policy
Windows exact build
fmt/clippy/smoke
package
SBOM
checksums
package verify
upload artifact
tag-only attestation
tag-only draft release
```

---

# 95. Action pinning

当前已有 SHA pin。

Phase12只：

```text
verify
```

不要因为有新版就自动升级。

---

# 96. Release pipeline稳定优先

除非：

```text
current action is deprecated/security-broken
```

否则 release前不要升级major。

---

# 97. Remote workflow_dispatch 的目的

它必须证明：

> origin 上 exact candidate source 可以在 GitHub-hosted runner 中 clean build/package/verify。

---

# 98. 但是当前 USER尚未授权新的 Phase12 commit push

因此：

如果：

```text
RELEASE_SOURCE_COMMIT != origin/main
```

不能运行 exact remote validation并假装正确。

---

# 99. 没有 USER push authorization

Phase12本地完成后报告：

```text
READY FOR PUSH AUTHORIZATION
```

并停止远端步骤。

---

# 100. USER明确批准 push 后

才能：

```bash
git push origin main
```

或当前批准 branch。

---

# 101. 禁止 force push

---

# 102. Push 后

确认：

```text
origin/main == RELEASE_SOURCE_COMMIT
```

---

# 103. 然后运行 workflow_dispatch

推荐：

```bash
gh workflow run release.yml --ref main
```

---

# 104. 必须记录 run ID

---

# 105. 等待当前 run 终态

不要启动多个重复 workflow。

---

# 106. Remote结果

必须：

```text
success
```

---

# 107. 下载 workflow artifact

使用 exact run。

---

# 108. 运行本地 verify-package

对：

```text
downloaded GitHub artifact
```

再次验证。

---

# 109. Remote receipt

生成：

```text
dist/evidence/remote-workflow.json
```

记录：

```text
source commit
workflow run ID
conclusion
artifact ID/name
artifact hashes
```

---

# 110. workflow_dispatch不会attest/release

这是预期。

因为当前 workflow 的：

```text
attest-and-draft
```

只在：

```text
tag push
```

时运行。

---

# 111. 这正是 tag前安全 dry-run

不要改变。

---

# 112. Remote workflow失败

分类：

```text
source failure
workflow failure
runner/environment failure
```

---

# 113. 不因 flaky failure立刻改产品

先看 logs。

---

# 114. Phase 12G — Tag / Draft Release Gate

只有以下全部成立：

```text
P0=0
P1=0

warm gate PASS or USER-approved recalibration

mandatory manual acceptance:
PASS or explicit USER waiver

exact local candidate PASS

exact remote workflow_dispatch PASS

release version USER approved

release source USER approved

push completed
```

才能向 USER报告：

```text
READY FOR TAG APPROVAL
```

---

# 115. Agent不得因为 Phase12被批准就自动 tag

必须等新的明确 USER steering，例如：

```text
USER APPROVES tag v0.1.0 on <exact SHA>.
```

---

# 116. 没有这句

```text
tag = no
```

---

# 117. Tag必须

```text
v0.1.0
```

前提 USER批准 version。

---

# 118. Annotated Tag

优先：

```bash
git tag -a v0.1.0 <SHA> -m "StickyMD v0.1.0"
```

---

# 119. Tag必须指向

```text
RELEASE_SOURCE_COMMIT
```

不是：

```text
whatever HEAD happens to be
```

---

# 120. Push tag

仅 USER明确授权后：

```bash
git push origin v0.1.0
```

---

# 121. Tag push将触发 current release workflow

预期：

```text
build/package
→ checksums/SBOM
→ upload
→ attest
→ create DRAFT release
```

---

# 122. 绝不能自动发布非draft

当前 workflow应只产生：

```text
draft
```

---

# 123. 如果 workflow意外产生 published release

立即报告：

```text
RELEASE WORKFLOW SAFETY FAILURE
```

不要继续。

---

# 124. Tag workflow必须成功

所有 jobs：

```text
success
```

---

# 125. Attestation必须成功

至少：

```text
portable ZIP provenance
SBOM attestation
```

---

# 126. Draft release存在

但：

```text
isDraft = true
```

---

# 127. 不在 Phase12自动 publish

---

# 128. Phase 12H — Downloaded Artifact Qualification

从 GitHub draft release / exact workflow下载：

```text
portable ZIP
SHA256SUMS
SBOM
```

---

# 129. 这是最终可能公开给用户的artifact

所以必须重新验证。

---

# 130. Checksum

```text
SHA256SUMS
```

必须匹配。

---

# 131. Attestation Verify

如果 GitHub CLI当前支持：

```bash
gh attestation verify <portable-zip> --repo Develata/StickyMD
```

---

# 132. 记录结果

---

# 133. Downloaded package verify

再运行：

```text
verify-package
```

---

# 134. 解压到

```text
ASCII path
space path
Chinese path
```

---

# 135. Final downloaded-artifact smoke

至少：

```text
launch
note persistence
Source typing
Preview
math
image
tray
quit
```

---

# 136. Exact Release Artifact Critical Manual Smoke

不一定重复全部26项完整人工矩阵。

因为完整人工矩阵已经绑定：

```text
same release source/product implementation
```

但必须对远端最终 artifact执行一个 critical smoke：

```text
launch
Microsoft Pinyin if available
ToolWindow / Alt+Tab
Tray
one dock edge
save/restart
```

---

# 137. 如果remote binary与local candidate binary bit-identical

记录。

---

# 138. 如果不同

这并不自动失败。

Rust/MSVC build未必跨machine bit-reproducible。

---

# 139. 但必须证明：

```text
same exact source commit
same Cargo.lock
same Rust toolchain
same target
package verification PASS
```

---

# 140. 不假装 reproducible build

---

# 141. 如果已有 reproducibility contract

按真实结果。

---

# 142. Clean VM

如果可用：

必须优先使用：

```text
downloaded remote artifact
```

执行。

---

# 143. Clean VM至少

```text
Windows 11 x64
no Rust
no Git
no dev environment requirement
```

---

# 144. 检查

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

# 145. 如果 Clean VM仍无法执行

```text
NOT TESTED
```

是否发布交 USER。

---

# 146. 下载后 evidence

```text
dist/evidence/downloaded-artifact-smoke.json
```

---

# 147. Phase 12I — Final Publish Handoff

当：

```text
draft release ready
artifact verified
critical smoke done
```

报告：

```text
DRAFT RELEASE READY FOR USER PUBLISH DECISION
```

---

# 148. Agent不能 publish

除非 USER再次明确：

```text
USER APPROVES publication of draft v0.1.0.
```

---

# 149. 这必须是单独的最终决策

Tag approval：

```text
!=
```

Publish approval。

---

# 150. 为什么分开

因为 tag workflow之后还有：

```text
remote artifact
attestation
download verification
```

这些必须先检查。

---

# 151. USER发布批准后

Agent才可以：

```bash
gh release edit v0.1.0 --draft=false
```

或 current safe equivalent。

---

# 152. 发布前再次读取

```text
isDraft == true
tag == v0.1.0
target commit correct
assets correct
```

---

# 153. 禁止 `--latest` 等额外语义

除非 USER要求。

---

# 154. v0.1.0 可以成为 latest吗？

默认 GitHub自然认为最新稳定release即可。

不要额外操作。

---

# 155. Release Notes

正式 draft release notes应比当前自动模板丰富。

但不要营销化。

---

# 156. Phase12可以准备

```text
docs/release-notes/v0.1.0.md
```

---

# 157. 但如果 USER未批准 version

先：

```text
docs/release-notes/0.1.0-draft.md
```

或 existing convention。

---

# 158. Release Notes应包含

```text
What StickyMD is
Windows 11 x64
portable model
one-note-per-directory
Source/Preview/Split
Markdown/GFM
RaTeX math
managed images
export
tray/dock
theme/opacity
zoom
math delimiter conversion
known limitations
unsigned binary notice
```

---

# 159. 不写

```text
blazing fast
zero memory
perfect
bug-free
```

之类无法支持的宣传。

---

# 160. Known Limitations

真实写：

例如：

```text
Windows 11 x64 only
portable directory must be writable
remote images are not downloaded
no installer
unsigned unless signing provided
```

以及真正遗留的 waiver。

---

# 161. Warm target如果USER放宽

不要在 release note说：

```text
failed startup target
```

这是工程内部信息。

除非真实用户体验存在known issue。

---

# 162. README

最终 release状态只在真正 publish 后改：

```text
Latest stable: v0.1.0
```

---

# 163. Tag之前可以准备

但不声称已发布。

---

# 164. Phase 12 acceptance hierarchy

最终状态必须有：

```text
AUTOMATED PASS
MANUAL PASS
USER WAIVED
USER-APPROVED GATE
NOT TESTED
FAIL
```

---

# 165. `USER-APPROVED GATE`

例如：

```text
Warm p95 <=400ms
```

不是 waiver。

---

# 166. `USER WAIVED`

例如：

```text
RDP not tested
```

是 acceptance waiver。

---

# 167. 两者绝不能混。

---

# 168. Release Blocking Matrix

创建：

```text
dist/evidence/release-readiness.json
```

由 Rust CLI生成。

---

# 169. 它读取

```text
automated evidence
manual evidence
decision ledger projection
remote evidence
```

---

# 170. 最终逻辑

```text
if P0/P1:
    NOT_READY

if unapproved hard gate:
    NOT_READY

if mandatory manual NOT_TESTED without waiver:
    NOT_READY

if exact package absent:
    NOT_READY

if remote required but absent:
    NOT_READY

else:
    READY
```

---

# 171. 不允许 `--force-ready`

不得存在绕过 flag。

---

# 172. 可以有

```text
--explain
```

输出 blockers。

---

# 173. Manual receipt tamper

至少验证：

```text
source commit
exe hash
schema version
```

---

# 174. 不需要 cryptographic signing manual receipt

SHA binding够用。

---

# 175. Source Freeze规则

一旦开始 manual acceptance：

如果修改：

```text
apps/
crates/
Cargo.lock
Cargo.toml
runtime resources
release scripts
```

立即：

```text
invalidate candidate
invalidate manual receipts
```

---

# 176. 哪些修改不必 invalidate？

只有完全不参与artifact的：

```text
external/untracked evidence
```

---

# 177. 即便 docs commit

为了 exact source简单起见：

> candidate freeze后不要再commit任何东西，直到 USER决定 tag。

---

# 178. 这是 Phase12的推荐工作模式

所有 Phase12后冻结 evidence：

```text
dist/evidence/
```

---

# 179. Candidate Freeze前

把所有需要commit的 current-state docs修好。

---

# 180. Candidate Freeze后

```text
git status --short
```

必须保持 clean。

---

# 181. Manual evidence不使worktree dirty

dist已gitignored。

---

# 182. Remote evidence同理。

---

# 183. Phase12 Reports

源码内在freeze前创建模板：

```text
docs/report/phase-12-final-qualification.md
docs/report/phase-12-release-handoff.md
```

---

# 184. Template可以写：

```text
Final runtime evidence is stored in dist/evidence and bound to release candidate hashes.
```

---

# 185. 不在 freeze后反复改报告

---

# 186. Final USER-visible report

Agent最终聊天输出是最新动态报告。

不需要为了更新聊天结论再commit。

---

# 187. Release Workflow Static Audit

当前 workflow已有 good properties。

重新确认：

```text
tag version matches Cargo
tag commit on main
minimum permissions
SHA-pinned actions
no pull_request_target
no curl|sh
exact build --locked
package verification
SBOM
checksums
attest
draft only
```

---

# 188. `workflow_dispatch`

不得产生 draft release。

当前应如此。

---

# 189. tag push

才：

```text
attest-and-draft
```

---

# 190. Current workflow actions

不要仅因为新版存在就升级。

Release freeze期间：

```text
known-working pinned SHAs > needless churn
```

---

# 191. Security advisory例外

如果当前 action release有真实security问题：

升级并重新qualification。

---

# 192. Cargo Dependency Freeze

Phase12：

```text
Cargo.lock frozen
```

---

# 193. 不做

```text
cargo update
```

---

# 194. cargo deny duplicate warnings

如果只是已审计 duplicate：

保持。

不要在最终phase为“干净输出”乱升级。

---

# 195. Runtime Dependency

理想：

```text
new runtime dependencies = 0
```

---

# 196. Final Performance

Phase12不继续优化。

只证明没有 candidate drift：

```text
warm
cold
memory
idle
typing
preview
```

---

# 197. 如果出现显著 regression

例如：

```text
warm +100ms
PWS +10MiB
typing p95 crosses hard gate
```

调查。

---

# 198. 不是每个 1–2% variation都修。

---

# 199. Manual Fail 处理

如果 manual case：

```text
FAIL
```

分类。

---

# 200. P0/P1

必须修。

---

# 201. 修复后

```text
candidate invalidated
```

重新：

```text
test
freeze
package
manual affected matrix
remote
```

---

# 202. 不需要重跑完全无关manual case

如果能证明 bug fix影响域。

---

# 203. 但关键 startup/package evidence必须重跑。

---

# 204. P2 visual defect

记录。

USER决定是否 release blocker。

---

# 205. P3

defer。

---

# 206. Manual Matrix没有要求“零瑕疵”

要求：

```text
correct
usable
frozen behavior works
```

---

# 207. Clean Architecture Final Audit

必须再次确认：

```text
DocumentState sole authority
ConfigCoordinator sole committed preference authority
Window reducer logical shell authority
disk durable representation only
Preview derived
worker stale generation
asset ownership proof
bounded caches/queues
```

---

# 208. File Cohesion

不为 release随意拆文件。

---

# 209. 不进行“final cleanup refactor”

这是经典 release风险。

---

# 210. Dead Code

如果 compiler warning为0：

不要为美观做无证据重构。

---

# 211. `cargo fix`

不要全仓执行。

---

# 212. Formatting

当然保持。

---

# 213. Final local verification

候选 freeze后至少：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
cargo deny check
git diff --check
```

---

# 214. Rust CLI

全部 release suites。

---

# 215. Package

完整验证。

---

# 216. `git status`

必须 clean。

---

# 217. Commit identity

必须和 receipt一致。

---

# 218. `origin/main`

在未 push前可以不同。

receipt必须明确：

```text
remote_synced = false
```

---

# 219. Push后

```text
remote_synced = true
```

---

# 220. Phase12 Task状态

可能有这些：

```text
In Progress

Local Qualification Complete — USER Decisions Required

Local RC Ready — Push Authorization Required

Remote Qualification Complete — Tag Approval Required

Draft Release Ready — Publish Approval Required

Completed — v0.1.0 Published
```

---

# 221. 不要直接从第一状态跳 Completed。

---

# 222. 本阶段可能需要多轮 USER steering

这是正常的。

不创建新Phase。

---

# 223. USER Steering Gate 1

可能需要：

```text
Approve warm startup boundary <=400ms.
Approve release version 0.1.0.
Approve specific manual waivers.
Approve push of exact candidate commit.
```

---

# 224. USER Steering Gate 2

本地+remote dry-run后：

```text
Approve tag v0.1.0 on <SHA>.
```

---

# 225. USER Steering Gate 3

draft release验证后：

```text
Approve publication of draft v0.1.0.
```

---

# 226. 不把这三个 approval合并

除非 USER真的一次性明确授权全部。

---

# 227. Phase12 Artifact Naming

Local：

```text
StickyMD-0.1.0-local-rc-<sha12>-windows-x64-portable.zip
```

---

# 228. Tag workflow：

```text
StickyMD-0.1.0-windows-x64-portable.zip
```

或当前 package script真实tag命名。

---

# 229. 不手动改 package naming

遵循 package script。

---

# 230. Release package size

当前约：

```text
3.72 MiB
```

hard gate：

```text
<=30 MiB
```

无需继续优化。

---

# 231. Memory

当前远低hard gate。

不要继续为几MiB动代码。

---

# 232. Preview

Phase11已非常快。

不要继续优化。

---

# 233. 4K image

当前已改善且安全。

不要继续优化除非 manual发现实际卡死/OOM。

---

# 234. Warm

如果 USER批准400：

停止性能优化。

这是重要 stop rule。

---

# 235. Phase 12 Definition of Done — Local Qualification

- [ ] Root AGENTS状态同步。
- [ ] README/current summaries无Phase10漂移。
- [ ] Phase12 task创建。
- [ ] Phase12 acceptance创建。
- [ ] release decision ledger创建。
- [ ] warm gate USER状态准确。
- [ ] version USER状态准确。
- [ ] manual waiver状态准确。
- [ ] no feature changes。
- [ ] P0=0。
- [ ] P1=0。
- [ ] release source commit冻结。
- [ ] working tree clean。
- [ ] exact EXE构建。
- [ ] exact ZIP构建。
- [ ] exact SBOM。
- [ ] exact SHA256SUMS。
- [ ] release-candidate receipt生成。
- [ ] package verify PASS。
- [ ] automated evidence绑定exact hash。
- [ ] manual receipt tooling完成。
- [ ] manual receipt不能自动PASS。
- [ ] stale receipt识别。
- [ ] final performance记录。
- [ ] final resource记录。
- [ ] final architecture audit。
- [ ] cargo deny PASS。
- [ ] no high severity advisory。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] no runtime network。
- [ ] no WebView。
- [ ] no Tokio。
- [ ] no DB。

---

# 236. Definition of Done — Manual

- [ ] Microsoft Pinyin MANUAL PASS / USER WAIVED / explicit blocker。
- [ ] WeChat IME MANUAL PASS / USER WAIVED / blocker。
- [ ] Taskbar。
- [ ] Alt+Tab。
- [ ] Alt+Tab away。
- [ ] Tray。
- [ ] Top Dock。
- [ ] Left Dock。
- [ ] Right Dock。
- [ ] No Bottom。
- [ ] 24DIP / nearest edge。
- [ ] sensor。
- [ ] Pin orthogonality。
- [ ] zoom。
- [ ] 220×120。
- [ ] opacity40。
- [ ] theme。
- [ ] Preview visual。
- [ ] Math visual。
- [ ] image visual。
- [ ] delimiter conversion + one-step Undo。
- [ ] traditional shortcuts。
- [ ] clipboard image sources。
- [ ] native export。
- [ ] hard-kill recovery。
- [ ] dual monitor or status。
- [ ] mixed DPI or status。
- [ ] monitor disconnect or status。
- [ ] remaining environment cases explicitly classified。

任何未执行：

```text
NOT TESTED
```

不能自动变PASS。

---

# 237. Definition of Done — Remote Pre-Tag

只有USER允许 push后：

- [ ] exact candidate push。
- [ ] origin/main exact SHA。
- [ ] workflow_dispatch run。
- [ ] remote dependency policy PASS。
- [ ] remote Windows build PASS。
- [ ] remote smoke PASS。
- [ ] remote package PASS。
- [ ] remote SBOM PASS。
- [ ] remote package verify PASS。
- [ ] workflow artifact下载。
- [ ] downloaded artifact本地verify PASS。
- [ ] remote-workflow receipt生成。
- [ ] no tag yet。

---

# 238. Definition of Done — Tag/Draft

只有USER明确批准tag后：

- [ ] annotated `v0.1.0` exact tag。
- [ ] tag push。
- [ ] tag release workflow PASS。
- [ ] attestation PASS。
- [ ] SBOM attestation PASS。
- [ ] draft GitHub Release创建。
- [ ] release仍Draft。
- [ ] remote release artifact下载。
- [ ] checksum verify。
- [ ] attestation verify。
- [ ] package verify。
- [ ] downloaded critical smoke。
- [ ] clean VM if available。
- [ ] publish仍未执行。

---

# 239. Definition of Done — Publish

只有USER明确批准publish后：

- [ ] draft身份再次确认。
- [ ] tag/SHA确认。
- [ ] artifact确认。
- [ ] release notes确认。
- [ ] known issues确认。
- [ ] publish。
- [ ] published release URL确认。
- [ ] release仍绑定exact `v0.1.0` tag。
- [ ] no additional source changes accidentally included。

---

# 240. Phase 12 Final Report

最终/每个 USER gate前都更新聊天结果。

Source内模板：

```text
docs/report/phase-12-final-qualification.md
```

---

# 241. Final Response Format — Local Gate

# Phase 12 Local Qualification Result

## Candidate

```text
source commit:
version:
EXE hash:
ZIP hash:
SBOM hash:
Cargo.lock hash:
```

## Architecture

```text
P0:
P1:
drift:
```

## Warm Gate

```text
preferred target:
release boundary:
USER status:
measured p95:
result:
```

## Manual Acceptance

```text
MANUAL PASS:
NOT TESTED:
FAIL:
USER WAIVED:
```

并列具体 blockers。

## Automated Qualification

全部。

## Package

全部。

## Remote Status

```text
origin exact?
workflow dispatch?
```

## USER Decisions Required

明确逐项。

## Recommendation

只能：

```text
NOT RC READY

LOCAL RC READY — PUSH AUTHORIZATION REQUIRED

READY FOR TAG APPROVAL
```

---

# 242. Final Response Format — Tag Gate

远端dry-run完成后：

# Phase 12 Pre-Tag Result

```text
exact SHA
origin sync
remote workflow run
remote artifact
manual status
gate decisions
```

Recommendation：

```text
READY FOR TAG APPROVAL
```

或：

```text
NOT READY
```

---

# 243. Final Response Format — Draft Gate

tag workflow完成：

# Phase 12 Draft Release Result

```text
tag
tag SHA
workflow run
artifact hashes
attestation
draft release
download verification
critical smoke
```

Recommendation：

```text
DRAFT RELEASE READY FOR PUBLISH APPROVAL
```

或：

```text
DO NOT PUBLISH
```

---

# 244. Final Response Format — Published

只有真的 USER批准并publish：

# StickyMD v0.1.0 Release Result

```text
version
tag
source SHA
release URL
artifact
SHA-256
SBOM
attestation
manual qualification
known limitations
```

最后：

```text
StickyMD v0.1.0 published.
```

---

# 245. Phase 12 Architecture Review

最终必须亲自回答：

1. Phase12是否增加任何产品能力？
2. 是否为发布做无必要refactor？
3. Release source commit是否exact？
4. Manual receipt是否绑定exact artifact？
5. 是否解决了“candidate != HEAD”证据循环？
6. Warm gate是否只有USER批准后才改变？
7. 180ms preferred target是否仍保留？
8. Manual NOT TESTED是否有被冒充PASS？
9. P0/P1是否为0？
10. User-file ownership是否仍安全？
11. Atomic save/OCC是否仍安全？
12. Tool Window是否仍可恢复？
13. IME是否真实状态？
14. release workflow是否仍draft-only？
15. Actions是否SHA-pinned？
16. workflow_dispatch是否不创建release？
17. Tag是否严格指向approved SHA？
18. Publish是否独立USER gate？
19. release package是否无user data？
20. 是否未捆绑Microsoft proprietary fonts？
21. SBOM/license是否完整？
22. core/render unsafe是否仍0？
23. 是否无WebView/Tokio/network/DB？
24. 是否避免为了最后性能数字制造复杂度？

---

# 246. 不允许 Agent自行做的事情

在没有相应 USER明确授权前：

```text
DO NOT:
push new Phase12 candidate
create tag
push tag
publish GitHub Release
approve warm gate
approve version
waive manual acceptance
```

---

# 247. 允许自动执行的事情

```text
read repo
modify local repo for Phase12 governance/tooling
commit local Phase12 preparation
build
test
benchmark
package
generate SBOM
generate hashes
record local evidence
run manual receipt helper with USER
```

---

# 248. 当达到一个 USER gate

停止。

不要猜下一步。

---

# 249. 不创建 Phase 13

无论哪个 gate停下：

都属于：

```text
Phase 12 continuation
```

USER steering后继续本阶段。

---

# 250. 最终 Stop Rule

如果当前没有 warm gate USER approval，或者 mandatory manual acceptance仍未处理：

即使所有自动化都通过：

```text
DO NOT TAG
```

如果本地qualification完整但还没push授权：

```text
STOP AT:
LOCAL RC READY — PUSH AUTHORIZATION REQUIRED
```

如果远端dry-run完整但没tag授权：

```text
STOP AT:
READY FOR TAG APPROVAL
```

如果draft完整但没publish授权：

```text
STOP AT:
DRAFT RELEASE READY FOR PUBLISH APPROVAL
```

完成当前可执行部分后立即停止并报告。

