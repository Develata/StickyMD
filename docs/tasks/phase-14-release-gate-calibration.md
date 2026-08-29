# Phase 14 — Release Gate Calibration and Qualification Closure

## Status

Implementation Reopened — USER 已批准 remote workflow artifact promotion（Option B）与最小 exact 重验；
qualification authority/tooling 正在收敛，所有旧 local-build-bound evidence 已失效。

## Purpose

在产品 runtime 默认冻结的前提下，校准 v0.1.0 startup release boundary，完成 startup attribution、
风险分层人工验收、独立 evidence channel 与 local readiness 收口。

## Prerequisites

- Phase 13 exact candidate 自动化通道已形成可审计收据。
- USER 已明确批准 v0.1.0、unsigned distribution、550 ms cold/warm hard boundary、
  Tier A/B/C manual policy 与 independent evidence collection。
- push、tag、draft release、publish 均未授权。

## Scope

- 更新 `docs/plan/10` / `docs/plan/11` 发布合同。
- `stickymd-smoke` 增加 Phase 14、三层 startup threshold、startup attribution、G1/G2 guided
  manual sessions、G3/G4/G5 exact-candidate automation、risk-tier readiness 和 independent channel collection。
- 建立 Source Freeze 与 Local Preflight；将 successful GitHub release workflow artifact 下载、验证并
  Promote 为唯一 exact candidate；Source Freeze 不变时复用 source-only evidence，仅重新收集
  downloaded、Runtime、Performance、Resources、G3、G4、G5、Manual、Readiness artifact-bound evidence。
- 对 exact-candidate 人工验收中 USER 实际观察到的 release-blocking 缺陷作最小、回归绑定的纠正；
  每次 tracked correction 都作废旧候选并重新开始资格化。
- 实现 USER 在候选冻结前明确批准的 Split 语义滚动同步（默认开启、可关闭）、当前 Source
  纯文本查找/替换（大小写开关、无正则），以及 `$` 数学转换控件标识。
- 对 Release 运行时内存做 source/preview/split/cache 分模块归因；只实施有前后测量收益、保持
  correctness/latency 的低复杂度优化。
- 修复 USER 在正式人工验收中确认的 Preview 选择几何模型缺陷：保留 Cosmic Text cluster 事实，
  以 viewport-local projection 统一 hit-test、selection paint 与 copy range，不新增 shaping dependency。
- 修复 Source 查找面板的 caret/IME/mouse geometry、Ctrl+F toggle、Ctrl+H 同 session expand、方向键
  语义与 Find-only replacement guard；不改变查找算法或 Document authority。
- 将不必要固定等待改为 observable acknowledgement + bounded early-exit；仅并发独立 headless 模块，
  同一物理桌面的输入、tray、clipboard、dock 与资源通道保持串行。
- 将 Microsoft Pinyin / WeType 的客观功能矩阵迁入 G4 exact-candidate 自动化；TSF profile 临时切换必须
  fail closed 并恢复原 profile，候选窗纯视觉继续由 G1 人工持有。
- 统一 smoke GUI child 的进程所有权：所有正常错误返回与 unwind 均由 RAII owner 执行 kill + wait；
  Performance/Resources 在启动前检测遗留的 smoke-owned 测试进程并 fail closed，但不得自动终止任何
  启动前已存在的 StickyMD 实例。
- 将正式 warm-cache startup 进程间隔校准为 `1000 ms`；原 `250 ms` 只保留为命名清晰的
  rapid-restart 诊断，不生成或替代发布门收据，门槛和样本统计方法不变。

## Out of Scope

- 未经 USER 明确授权的其他产品 runtime、产品依赖、Markdown/IME/window/persistence 行为变化。
- 为达到 180/400 ms target 做追逐式优化。
- push、tag、remote workflow、draft release 或 publish。

USER 在最终候选前补充批准 portable runtime gate：Release EXE 不得要求 Rust/C/C++ 开发环境或
另装 Visual C++ Redistributable。该项属于既有 portable ZIP 发布合同的资格化加固，不新增产品
runtime capability；以静态 MSVC CRT 和 exact PE import 自动检查共同实现，Clean VM 仍保留人工
`NOT TESTED` gate。

实现与交叉验证证据见
[`phase-14-portable-runtime-hardening.md`](../report/phase-14-portable-runtime-hardening.md)。

## Authority and Freeze

唯一产品 authority 不变；Phase 14 tracked delta 原则上仅限 plan/projection/docs 与 verification tooling；
USER 在 exact-candidate 人工验收中报告的 release-blocking implementation defect 可以按原 plan 作最小纠正。
tracked freeze commit 后所有动态收据只写 ignored `dist/evidence/`。任何 tracked source 改变都会
创建新 Source Freeze 并使旧 candidate receipt 失效；candidate 只能由 remote workflow artifact Promote。

## Deliverables

- `tools/smoke/phase-14.ps1`
- `docs/acceptance-cases/phase-14.md`
- `docs/report/phase-14-release-policy.md`
- `docs/report/phase-14-startup-attribution-plan.md`
- `docs/report/phase-14-final-qualification.md`
- `docs/reference/qualification-execution-model.md`
- `tools/manual/phase-14-guide.md`
- exact Phase 14 evidence bundle and final readiness report
- `dist/evidence/g3-exact-qualification.json`（ignored dynamic receipt）及其 fail-closed validator
- `dist/evidence/g4-exact-qualification.json`（ignored dynamic receipt）及其 fail-closed validator

## Verification

- Phase 14 targeted Rust CLI tests and smoke contract trace。
- workspace fmt / strict Clippy / tests / Release build / deny。
- exact copied candidate Release, headless, Runtime, Performance, Resources and Readiness channels。
- product runtime/dependency delta audit。
- USER-observed regression 的 named tests，以及扩展后的三边/顶角 copied-Release smoke。
- G3/G4/G5 exact desktop groups 分别在独立候选副本中串行通过，并生成 clean、同 commit、同 ZIP/EXE
  identity 的完整 group receipt；单项诊断收据不计入 readiness。
- G4 physical-drag harness 必须区分 Floating requested-position 与 Dock application-resolved 终态，并以
  150% DPI 回归防止 DIP/physical-pixel 单位混用；G4-06 首次模式纠正后的二次探针前必须完整重申
  profile/route/open/native，仍只允许一次有界纠正。
- 远端 CI 故障归因与可诊断输出；隔离 runner CI 分片并发、模块化 Resources 定向入口与
  “分片并集 = 完整任务图”回归。

## Candidate Defect Correction

旧 exact candidate 在人工验收中暴露多项 implementation defect：同一行局部 selection 误涂其它
逻辑行；math delimiter conversion 后 Source projection 未立即进入 layout；Split/Preview 之间切换时
clean preview 未按新 viewport relayout；真实 winit move loop 未提交 Dock，导致三边失焦自动收起不
工作；从旧 Dock 直接换到另一边时错误地先 detach，导致需要第二次拖动。修复不改变 Document
authority、Markdown/math semantics 或 runtime dependency；内容缩放导致工具栏 paint/hit 坐标分离的
问题通过 document scale 与 DPI-only shell scale 分权修复。Phase 8
copied-Release smoke 改用真实指针拖动与真实 shell 失焦，同时补齐 left/top/right、两个顶角和 Pin
ON/OFF 正交路径，并连续执行 Left -> Top -> Left -> Right、禁止以 Floating 中间态掩盖直接换边；
人工 G2 在新候选上重跑前仍为 `NOT TESTED`。详见
`docs/report/phase-14-candidate-defect-remediation.md`。

随后对变宽 Preview 文本的正式 G1 复测又证明：旧 selection 蓝框虽不再跨逻辑行，但仍按整段
grapheme 数量比例估算 x/boundary，导致蓝框与实际 byte selection/copy 不一致。该缺陷是几何模型
错误，不是偶发桌面抖动；修复期间 Phase 14 保持 reopened，不得生成或复用旧 exact candidate。

## Resources Failure Triage

旧 candidate `1d533357ac072605b350b0523f2957597341bc62` 的 Phase 8 hidden-window
resource matrix 在外部 reload 后注入 Enter 时失败。分段、组合与降阶复现没有重现产品状态机
错误；根因归类为 `QUALIFICATION HARNESS DEFECT`：旧 harness 用固定 350 ms sleep 推断 source
projection 已完成，既没有验证 editor projection，也没有记录 foreground/focus/geometry 等 shell
事实。

修正保持产品 runtime 与 runtime dependency delta 为零：Rust smoke CLI 新增有界
`window-stress` reducer、typed shell-state wait 和基于真实 Ctrl+A/C clipboard projection 的 ready
gate。tracked tooling 变化使旧 candidate 失效；必须在新 freeze commit 上重新收集 Release/package、
headless CI、Runtime、Performance 与 Resources。Resources PASS 前不开始正式 manual receipt。

后续将 compact resize smoke 改为真实物理输入时，审计确认锁定的 winit 0.30.13 错误地把栈上
`POINTS` 地址作为 `WM_NCLBUTTONDOWN.lParam`。这不是资源 warm-up 修正的一部分；Phase 14 另在
既有 Windows native-message adapter 内作最小 signed-screen-coordinate repair，保持 winit/Win32
move-size lifecycle 权威且不新增 runtime dependency。详见
`docs/report/phase-14-candidate-defect-remediation.md`。

静态 CRT 复跑同时暴露 Phase 11-B 旧 `replace_range` 循环在 1 MiB/1000 formula 上的近似
`O(n*k)` 搬移；已改为单向 `O(n+k)` 文本构建，p95 从 278--304 ms 降到 4.1 ms，语义和 authority
不变。证据写入同一 candidate defect remediation report。

首次 exact-candidate Runtime campaign 还暴露 qualification input activation 缺陷：posted preview click
不授予 Windows foreground/focus，posted zoom chord 也不可靠更新 winit modifier state。smoke 已统一在
真实快捷键/projection probe 前建立并验证前台输入目标，并用成对物理 key events 驱动 zoom；Phase 10
targeted Runtime 与完整 Phase 14 Runtime 均已通过。该变化仅属于资格化工具，不改变产品 runtime。

USER 后续提供的 mixed Markdown/RaTeX 暴力文档已纳入仓库内确定性 fixture；宏包依赖
仅按当前 RaTeX 支持改写为标准等价表达。Phase 5/6 Rust smoke 现在覆盖全量语义、
27 个唯一公式布局/raster、窄/宽与高低缩放、深滚动图片懒加载；Phase 5 copied-Release
Preview/Split runtime 改用该 fixture 并验证 canonical source 字节不变。人眼观感仍留在 G1
`NOT TESTED`，不由 headless 像素断言替代。

## Result

source-controlled policy、CLI、CI、guided manual 与 readiness contract 已实现并通过冻结前基线。
G3/G4 exact-candidate harness 已完成开发态五项全通过；G5 exact harness 已完成四组开发态全通过，
并为 compact、presentation 与 rendering 生成候选绑定截图。clean receipt 仍必须在本次提交生成的新
candidate 上重新采集。
动态 evidence 与最终 recommendation 只在 exact source freeze 后产生；人工项目不会因此自动 PASS。

本轮 reopened implementation slice 已完成：Preview 选择改为 viewport shaping-cluster 几何与 visual-row
locator；Search 字段使用单一 layout authority 提供 paint/hit/caret/IME geometry；Ctrl+F/Ctrl+H reducer、
方向键与 Find-only guard 已回归。G5 rendering 的三个固定 2 s 等待改为最多 12 s、连续稳定截图即提前
结束的 bounded acknowledgement。无界面任务继续由既有 CI tests/performance shards 并发；同一物理
桌面的 foreground、鼠标、clipboard、tray、dock 与 resource measurement 不并发，避免把相互干扰写成
正式 PASS。真实 Microsoft Pinyin / WeType 的客观功能矩阵已迁入 G4-06 exact automation，候选窗出现、
位置与视觉仍由 G1 人工判断；自动化实现与边界见 `phase-14-real-ime-automation-design.md`。自动化结果见
`phase-14-preview-selection-geometry-design.md`；新 exact candidate 尚未生成。

资格化工具的进程隔离缺陷也已关闭：旧 runtime harness 在启动 GUI child 后仍存在可由 `?` 提前返回、
绕过尾部显式清理的路径，曾遗留 copied-Release 进程并污染后续 startup 采样。现由统一 RAII owner
覆盖 runtime、G3、G4、G5 的 GUI child，且 Startup/Resources 六个测量入口先执行只读遗留进程预检；
预检只拒绝继续，不替用户终止现有便签。根因与验证见
`docs/report/phase-14-smoke-process-isolation.md`。

同一当前 EXE 的干净定向诊断显示：`250 ms` / `1000 ms` cohort p95 分别为 `316.55 ms` /
`310.79 ms`，说明二者都可在安静主机上满足 550 ms 边界；正式失败时则出现跨 cohort 的主机非稳态。
USER 于 2026-08-28 批准把正式 warm-cache 间隔设为 `1000 ms`，用来降低快速进程重启语义对发布
性能事实的污染；`250 ms` 继续保留为 rapid-restart stress diagnostic。
