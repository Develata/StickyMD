# Phase 14 Smoke Process Isolation

## Executive Result

`QUALIFICATION HARNESS DEFECT` 已修复。产品启动代码、产品 runtime dependency 与产品行为均未改变。

## Symptom

一次正式 Performance 运行的 warm-start p95 异常升至 `586.09 ms`。运行后发现一个父 smoke 进程已经
退出、但仍存活的 copied-Release `StickyMD.exe`；其路径位于 smoke 临时目录。清理该确认为测试所有的
进程后，12 次诊断采样的 250 ms warm cohort p95 为 `316.55 ms`，12/12 低于 550 ms hard boundary；
1000 ms 间隔诊断 p95 为 `310.79 ms`。因此固定 250 ms warm 间隔不是该次失败的根因。

这些短诊断数据只用于根因归属，不替代正式 30 cold + 50 warm receipt。

## Root Cause

runtime resource 测量在启动 copied-Release GUI child 后，仍有若干 fallible 操作。原实现只在函数尾部
显式调用 `stop_child`；其中任意 `?` 提前返回都会绕过该清理。相同所有权模式也分散存在于 G3/G4/G5
desktop qualification。遗留 GUI 进程会争用 CPU、窗口/输入环境和同桌面调度，因而能够污染后续
Performance/Resources 的测量事实。

## Correction

- `ChildGuard` 成为 smoke GUI child 的唯一 owning wrapper。
- owner 正常离开作用域、错误返回或 Rust unwind 时，`Drop` 先尝试判断进程状态；仍存活则 `kill`，随后
  `wait` 回收进程对象。
- runtime、G3、G4、G5 继续保留显式正常关闭，以验证应用预期生命周期；RAII 负责所有异常路径兜底。
- Startup、Resources、MathResources、ImageResources、WindowResources、ZoomResources 在创建本轮临时
  目录和启动进程前，枚举现存 `StickyMD.exe`。只有 executable 位于已知 smoke 临时目录的实例才被归类
  为 stale test process；发现后立即 fail closed。
- 预检是只读的，不自动终止任何进程，也不会把普通 portable/user 目录中的 StickyMD 当成测试实例。

## Regression Evidence

- 先在没有 `Drop` cleanup 的 guard 原型上运行中途故障测试，子进程越过 owner scope 写入 sentinel，测试
  按预期失败。
- 加入 RAII cleanup 后，同一测试通过；这直接覆盖“启动成功后，后续 fallible step 返回错误”的路径。
- stale preflight 测试启动已知 smoke 临时目录中的 `StickyMD.exe`，验证 preflight 返回错误，同时确认该
  进程仍存活；测试代码只在断言后回收自己的 fixture。
- 分类测试验证普通 portable/user 路径不属于工具所有目录。
- runtime scenario 测试锁定六个 Performance/Resources 入口必须执行隔离预检。

## Boundary

RAII 保证 Rust 普通返回和 unwind 的进程回收；操作系统强制终止 smoke 父进程时不会执行 `Drop`。本轮
没有引入 Windows Job Object，因为当前观测缺陷来自可恢复的普通错误路径，RAII 是更小且充分的修复。
若未来出现父进程硬终止导致的重复泄漏证据，再单独评估 Job Object，不把未观测复杂度提前引入工具。

## Recommendation

运行 targeted tooling tests；确认预检环境干净后，只重跑一次正式 30 cold + 50 warm Performance，不重跑
完整矩阵或 Resources。

## Verification Result

Targeted tooling verification：

- `cargo test -p stickymd-smoke --locked`：101 passed。
- `cargo test -p stickymd-smoke --release --locked`：101 passed。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`：PASS。
- `cargo fmt --check`、`git diff --check`：PASS。

随后仅运行一次正式 Phase 14 Performance；没有重跑 Runtime、Resources 或完整矩阵。receipt 为
`dist/evidence/performance-qualification.json`，绑定当前 dirty tooling worktree：

| Cohort | Samples | p50 | p95 | max | 550 ms gate |
| --- | ---: | ---: | ---: | ---: | --- |
| cold | 30 | 366.23 ms | 471.80 ms | 509.82 ms | PASS |
| warm | 50 | 514.43 ms | 687.47 ms | 739.06 ms | **FAIL** |

正式运行的 environment/governance、全部 Release micro-benchmarks 与 Release build 均 PASS；唯一失败是
warm startup hard gate。warm attribution 的 `source_layout.p95 = 290.39 ms`，
`process_overhead.p95 = 136.82 ms`。运行退出后再次枚举进程，`StickyMD.exe` 数量为 0，说明本轮 harness
修复确实关闭了错误路径的 child 泄漏，但遗留进程不是 warm 性能失败的充分解释。

按 USER 的单次正式运行约束，本报告不把失败解释为 PASS，也不自动进行第二次 Performance。

## Warm Failure Diagnosis And Contract Calibration

同一当前 EXE、相同 20 KiB Source fixture 的干净短诊断得到：

| Interval | Samples | p95 |
| --- | ---: | ---: |
| 250 ms rapid restart | 12 | 316.55 ms |
| 1000 ms warm cache | 12 | 310.79 ms |

这证明 `250 ms` 并不会确定性触发产品退化；但正式失败 receipt 与后续 A/B/A 诊断显示，连续快速重启会
放大当前 Windows qualification host 的非稳态。慢样本的 child CPU time 与 wall time 同向增加，主要落在
Cosmic Text `FontSystem::new()` 的系统字体目录扫描和首次 Source shaping 区间，而不是单纯等待 CPU 调度。
当前证据可排除 stale smoke child、Defender 实时扫描、固定降频和产品版本性能断崖，但不足以把原因唯一
归给某个 Windows scheduler/service；本机缺少 WPA/WPAExporter，不能进行可靠 ETW 栈归因。

USER 于 2026-08-28 批准：正式 warm-cache cohort 在前一 child 完全回收后等待 `1000 ms`；原 `250 ms`
改为独立命名的 rapid-restart stress diagnostic，不参与 v0.1.0 release hard gate。180/400/550 ms 三层
门槛、30/50 样本数、nearest-rank p95 与不裁剪规则均保持不变。旧 250 ms Performance receipt 不会被
追认成新合同下的 PASS；下一次 exact-candidate qualification 必须生成新的 1000 ms warm-cache receipt。
