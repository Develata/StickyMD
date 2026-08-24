# Phase 14 CI Failure Triage and Smoke Modularization

## Scope

本报告只处理 verification tooling、CI 拓扑与跨平台测试前提；不改变 StickyMD 产品 authority、
Markdown/math 语义、runtime dependency 或发布物结构。

## Remote Failure Evidence

| Run | Commit | Channel | Root cause / evidence |
| --- | --- | --- | --- |
| CI `32615325124` | `d6ad84a126f218cb22cdcd4a93ff10e03102939c` | Linux portable-core | `math::engine::tests::cjk_text_formula_uses_native_fallback_without_panicking` 在 DejaVu Sans 不含中文字形时错误要求非透明像素 |
| Scheduled `32692242570` | same | deterministic core/render | 与上项同一断言、同一字体路径，确定为可重复的测试环境前提错误 |
| CI `32615325124` | same | Windows headless | `phase5_semantics` 失败，但 smoke 只投影 Cargo `stderr`，丢失 test harness `stdout` 中的具体失败正文；旧 run 无法进一步可靠归因 |
| CI `32743600654` | `64f452f97523a190732d4f847dc09a7e95e374f1` | Windows headless tests shard | 新的双流诊断暴露 `phase5-owned-outline.txt` 在 Windows checkout 中为 CRLF，而运行时 outline 固定为 LF；语义完全相同，fixture 比较错误地绑定了 worktree 换行 |

Linux portable-core 的目的只是证明平台无关 crates 不受 Win32 污染，不承诺 runner 安装 CJK
字体。修正后的跨平台测试使用 mixed CJK/Latin fixture，验证无 panic、几何与 RGBA buffer 不变量及
可用 Latin run；纯 CJK 可见像素断言只在目标产品平台 Windows 执行。产品的 Windows native
fallback 要求没有放宽。

smoke failure projection 同时保留 stdout/stderr 的尾部，各自有界为 32 KiB，既保留最后断言，
又避免冷编译日志无限放大 JSON 与 Actions 输出。

## Parallel CI Topology

Windows CI 拆为互不依赖的隔离 runner jobs：

1. format + strict Clippy；
2. headless tests shard；
3. headless Release performance shard；
4. workspace release build。

Linux portable-core 与 dependency-policy 继续独立并发。Rust CLI 保留完整 `all --ci`，并提供
`--ci-shard=tests|performance`；单元测试比较 task identity 集合，硬性证明两个 shard 的并集等于
完整去重计划。并发降低 wall time，但会增加 GitHub-hosted 总计算分钟数；这是明确的时间/成本
交换，不伪装成减少总 CPU 工作。

## Local Modular Verification

日常修复使用受影响 Phase 入口。Phase 14 Resources 额外支持：

```text
--resources --resource-module=source-preview
--resources --resource-module=math
--resources --resource-module=images
--resources --resource-module=window
--resources --resource-module=zoom
```

每次仍由 Rust CLI 构造任务，PowerShell 只转发参数。定向结果用于故障定位，不满足 exact
candidate 的完整 Resources receipt；候选冻结、发布资格化或明确全量请求才执行完整 Campaign。

## GUI Concurrency Boundary

不在同一交互桌面并发多个 GUI smoke。窗口焦点、物理鼠标、clipboard、tray 与 input desktop 是
共享资源；并发会制造竞态。资源/性能场景还共享 CPU、内存压力和系统缓存，并发会污染测量。
只有不同隔离 Windows 会话或机器才能安全并发这些通道。

Docker image 当前没有产品或验证消费者，因此不加入 CI。若后续出现明确容器化工具链，再单独
评估，而不是预先增加维护面。

## Candidate Impact

verification tooling、workflow 与测试均为 tracked source。按 Phase 14 freeze contract，旧
exact-candidate receipts 失效；形成下一候选前必须完成与变更风险匹配的定向验证，最终 release
资格仍需一次完整 exact-candidate campaign。

## Targeted Verification Evidence

- WSL/Linux 独立 target：修正后的 mixed CJK/Latin math test PASS。
- Windows `phase5_semantics`：连续 20 次 PASS；旧 remote run 的具体断言因旧 stdout 丢失缺陷
  无法追溯，不把当前通过倒推成旧 run 的虚构根因。
- Windows checkout newline 回归：`.gitattributes` 固定 render `.txt` fixtures 为 LF；golden
  comparison 同时只规范化 CRLF boundary 并保留孤立 CR，因而不再依赖 Git checkout 的
  `core.autocrlf` 投影。
- CI `tests` shard：完整 Phase 1 + workspace test task set PASS，suite=`all-ci-tests`。
- CI `performance` shard：全部无界面 Release baseline PASS，suite=`all-ci-performance`。
- Phase 8 copied-Release runtime：先通过失败证据发现驱动在鼠标按下后用固定等待猜测
  native move/size 已启动；修正为 client cursor projection + `hwndCapture`/`hwndMoveSize`
  显式握手后，最终版本连续 3 次 PASS；覆盖
  Left -> Top -> Left -> Right、Pin ON/OFF、两个顶角、close-to-tray/wake 与独立目录实例。
- smoke CLI：72 unit tests + 2 CLI exit tests PASS。
