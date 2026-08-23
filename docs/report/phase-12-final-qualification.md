# Phase 12 Final Qualification Report

## Executive Result

当前状态：**NOT RC READY — local qualification preparation in progress**。

Warm startup hard gate 已由 USER 校准为 400 ms；Phase 11 warm p95 311.353 ms 因而满足
v0.1.0 hard boundary，但没有满足 180 ms preferred target。mandatory manual evidence、release
version、unsigned policy、remote workflow 与 downloaded artifact evidence 尚未完成，不得 tag。

## Source Baseline

- starting commit: `d6ad84a126f218cb22cdcd4a93ff10e03102939c`
- starting branch: `main`
- starting tree: clean
- starting remote relation: `HEAD == origin/main`

最终 `RELEASE_SOURCE_COMMIT`、EXE/ZIP/SBOM/Cargo.lock SHA-256 与 Rust toolchain 由
`dist/evidence/release-candidate.json` 持有；该文件在 source freeze 后生成，不反向修改源码。

## Gate Calibration

| Metric | Preferred | v0.1.0 hard boundary | Latest measured p95 | Result |
| --- | ---: | ---: | ---: | --- |
| Cold startup | 180 ms | 400 ms | 300.692 ms | HARD PASS |
| Warm startup | 180 ms | 400 ms | 311.353 ms | HARD PASS; preferred missed |

Warm 400 ms 是 2026-08-23 USER-approved engineering gate recalibration，不是 waiver。

## Qualification Architecture

- Rust CLI owns task planning、receipt schema、exact-candidate USER decision projection、identity
  checking、manual recorder 与 readiness。
- PowerShell remains a thin stable entry and existing Windows package helper。
- receipts bind exact source commit、EXE、ZIP；stale or dirty evidence fails closed。
- manual recorder requires an interactive terminal and explicit `PASS` / `FAIL` / `NOT TESTED`。
- readiness has no `--force-ready` path。
- readiness requires five non-interchangeable exact local receipts: Release/package、headless CI、
  performance、runtime and resources。每份都校验 suite、唯一 required task、source/EXE identity、
  clean tree 与全部 PASS；Release/package 另绑定 ZIP。

## Known P0 / P1

- known product P0: 0。
- known automated product P1: 0。
- release blockers: mandatory human evidence and explicit USER/remote gates listed in the decision ledger。

## Automated Evidence

Freeze 后写入 `dist/evidence/automated-qualification.json`。Source-controlled report 不复制临时
receipt 的动态结果，以免制造 tested commit / report commit 循环。

### Invalidated candidate

初次冻结候选 `48327f4283da488826af9767076b7b12b56447d7` 已作废。其 product runtime 没有发现
控件故障，但 `stickymd-smoke` 运行时驱动未建立 Per-Monitor V2 DPI context：在 150% DPI
显示器上，Windows 向 smoke 进程返回虚拟化后的客户区坐标，驱动又按 1.5 缩放一次，消息
送达产品时再次被系统缩放。独立诊断构建记录到期望的 Preview 点击实际到达
`PhysicalPosition { x: 175, y: 39 }` 并命中 `ConvertMath`；Phase 8 的 Collapse 点击因此不能
成立。修复只改变 smoke 线程的 DPI awareness，不改变产品控件布局、命中算法或 authority。
所有旧 candidate/automated receipt 依 exact-SHA 规则自然失效，必须从新提交重新生成。

第二个候选 `5c205365d79968d4194c614be0a02e64b9525b20` 的完整 resources 任务在已通过
Source/Preview/Split 与 math resource 后，于 image resource 第 3 个 Source 样本的 cursor
parking 辅助调用失败。`SetCursorPos` 返回 false，但 Windows last-error 为 0；紧随其后的独立
调用成功且 `GetCursorPos` 确认到达目标。该候选因此仍按 fail-closed 规则作废。smoke 现对
cursor parking 使用 3 次、间隔 25 ms 的有界重试并确认实际坐标；持续失败仍使任务失败，
不会把真实 input-desktop 不可用误记为 PASS。

第三个候选 `59c5ca7d3ec632756da605619debeaa9cfa21cf9` 通过 Phase 12 Release 与 `all --ci`，
但 copied-Release Phase 8 runtime 在左侧 sensor hover 阶段停止。隔离诊断构建证明当前会话的
Windows `LockApp` 窗口覆盖输入桌面：从主屏左缘到正文区域的 `WindowFromPoint` 都返回
`Windows.UI.Core.CoreWindow` / `LockApp`，而不是已设为 topmost 的 StickyMD HWND；真实 hover
因此不能在本会话被验收，也没有被改写成 PASS。诊断同时暴露了一个独立 reducer 缺陷：
手动收起前已经聚焦时，IME/持久化等原因触发的同值 guard 刷新会被误当作“新获得焦点”，
从而过早撤销 collapsed sensor 的临时 topmost。实现现仅在 `false -> true` 焦点跃迁时撤销，
并在已聚焦窗口实际开始展开时清理；新增确定性 reducer regression test。该源码修复使
`59c5ca7` 的所有 exact receipts 作废；当前锁屏输入桌面仍不能替代 P12-M16 的人工验收。

第四个候选 `e6484833ab44a82fd0daadf0238596e469ead733` 通过 Release 10/10 与 exact
`all --ci` 16/16；portable、Preview/Split、RaTeX 与图片 runtime 也通过。其 Phase 8 sensor
runtime 仍因上述 `LockApp` 输入桌面阻塞。exact performance receipt 进一步真实记录 cold p95
`812.599 ms`、warm p95 `531.779 ms`，均超过 400 ms hard gate；cold/warm median 分别仅
`251.557 ms` / `361.149 ms`，慢样本同时集中在字体、source layout、show/focus 等多个里程碑，
符合当前被锁输入桌面/系统调度高方差而非本次 reducer 热路径回归的特征，但仍按门禁记为
FAILED，不以历史数据覆盖。

该失败还暴露了资格化工具的 fail-open 缺陷：readiness 原先只读取 Release/package 的
`automated-qualification.json`，没有强制 performance/runtime/resources。实现现要求五类
不可互换的 exact receipt，缺失、FAILED、dirty、stale 或 required task 不匹配均阻塞。
此 source-controlled 修复再次作废 `e648483` candidate；旧 FAILED receipts仅保留诊断价值。

## Manual Evidence

`docs/acceptance-cases/phase-12.md` 汇总 Tier A/B/C。当前全部保持 `NOT TESTED`；只有
`stickymd-smoke acceptance manual` 生成、且 SHA 与 candidate 一致的 receipt 才能参与 readiness。

## Remote / Downloaded Artifact

未授权 push，因而 `remote-workflow.json` 与 `downloaded-artifact-smoke.json` 当前不得伪造。

## Architecture Drift

No boundary drift. The focused/manual-collapse correction remains inside the existing Window Shell reducer;
product dependencies, authority ownership, and public capability axes are unchanged.

## Recommendation

**STOP — complete manual acceptance and pending USER decisions before remote/tag actions.**
