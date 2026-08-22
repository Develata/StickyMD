# 10_performance_reliability.md - 性能与可靠性合同

## Metadata

- `Layer`: Verification
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-20
- `Scope`: 性能约束的性质定义（Target / Measurement Method / Hard Failure Condition / Future Benchmark Entry）与可靠性底线

---

## Purpose

定义 StickyMD 的性能与可靠性**约束性质**。本章不承诺未经测试的精确数字为事实。

> 下表中所有数字均为 **Initial Engineering Targets**：
> 尚未由实际实现验证，不得对外宣传。
> 它们只作为后续技术验证与 release gate 的起点基线。

## Boundary

- 量化实现与基准测试在技术验证阶段建立；本章只定义结构与底线。
- 文件安全 > 性能；IME 正确性 > 内存（宪法 1.4 优先级）。

---

## Owned Objects

Not applicable。性能报告只测量对象，不拥有运行时 authority。

## Inputs

Release artifact、固定 fixture、Windows 11 环境信息与可复现测量步骤。

## Outputs

带 median/p95/max 和环境元数据的证据；未测项必须标记 NOT TESTED。

## State Changes

Not applicable。测量不得修改产品 authority；缓存清理/隐藏等被测行为仍由其 coordinator 拥有。

---

## 结构性硬约束（不依赖测量即必须成立）

```text
1. idle CPU 应接近 0：空闲状态不持续 redraw，event loop 进入 wait。
2. 无永久 redraw loop：动画期间短暂 request redraw，结束后停止。
3. bounded caches：公式 raster ≤ 8 MiB、解码图片 ≤ 16 MiB，LRU 淘汰。
4. bounded undo：256 entries 或 4 MiB，先到先淘汰。
5. preview worker stale-drop：过期 generation 结果立即丢弃。
6. 无无界任务/线程增长：固定少量线程（UI / preview worker / I/O worker），
   不引入通用线程池或 async runtime。
7. 无浏览器运行时：不加载 WebView/JS 引擎。
8. file safety 高于性能：任何性能优化不得绕过原子写入与冲突模型。
9. IME correctness 高于内存：不得为省内存破坏输入法路径。
10. 测量必须在 Release build + Windows 11 下进行。
```

---

<a id="initial-engineering-targets"></a>
## Initial Engineering Targets（未验证，不得宣传）

### 测量口径（Measurement Method）

测试环境必须记录：Windows 11 build、CPU、RAM、DPI、显示器数量、release commit、
Rust 工具链、是否首次启动、是否开启 Defender、文档 fixture。
内存指标使用 Private Working Set 与 Commit Size；启动后等待 30 s；
所有动画结束；无调试器；Release build；重复 ≥ 5 次，报告 median 与最大值。

启动门使用 copied standalone Release EXE 与真实 `EDITOR_READY`（canonical note 已载入、
Source projection 已整形、首个可用 frame 已呈现且 IME 已启用）。最终资格化至少各取
30 个 cold/warm 样本，nearest-rank p95，不裁剪。每个样本必须使用 PID/nonce 唯一的
ready object，并确认前一进程、mutex、tray 与 worker 已退出；warm 在同一已 bootstrap
目录连续启动，cold 另行记录冷却条件。测量方法错误不得通过放宽门槛来掩盖。

### 目标表

| 场景 | Target | Hard Failure Condition | Future Benchmark Entry |
| --- | --- | --- | --- |
| 源码模式 20 KiB | ≤ 28 MiB | > 40 MiB | 内存基准 |
| 预览模式 20 KiB + 20 公式 | ≤ 40 MiB | > 52 MiB | 内存基准 |
| 分栏模式同上 | ≤ 48 MiB | > 64 MiB | 内存基准 |
| Hidden to tray（cache purge 后） | ≤ 24 MiB | > 36 MiB | 内存基准 |
| 空闲 CPU 60 s 平均 | ≤ 0.05% | > 0.1% | CPU 采样 |
| 冷启动到可输入 p95 | ≤ 180 ms | > 400 ms | 启动基准；原 300 ms 门槛在 Phase 10 经 USER 明确 waiver |
| 热启动到可输入 p95 | ≤ 100 ms | > 180 ms | 启动基准 |
| 100 KiB 输入延迟 p95 | ≤ 16 ms | > 25 ms | 编辑基准 |
| 1 MiB 输入延迟 p95 | ≤ 33 ms | > 50 ms | 编辑基准 |
| 20 KiB preview 构建 | ≤ 50 ms | > 100 ms | 预览基准 |
| 100 KiB preview 构建 | ≤ 200 ms | > 400 ms | 预览基准 |
| 1 MiB preview 构建 | ≤ 1 s（后台） | > 2 s（后台） | 预览基准 |
| Portable ZIP | ≤ 20 MiB | > 30 MiB | 发布检查 |

Phase 10 还必须测量 50/100/300% zoom 的 Source/Preview/Split、数学/图片缓存、220×120
窗口与 Tool Window 资源差异；缩放不得造成 Markdown 重解析、无界 raster/cache 或
按滚轮事件逐次配置写盘。

分栏模式同时保留源码与预览布局，允许有限内存例外，但不得持续增长。

### 文档规模支持范围

```text
典型：0–100 KiB
支持：0–1 MiB
容忍：1–5 MiB
```

超过 5 MiB：源码编辑仍尽力工作；Preview 可显示性能警告并要求手动继续；
不允许崩溃或无响应；不把 StickyMD 发展成大文件编辑器。

---

## 内存策略（结构性）

- RaTeX 字体按需加载；Source-only 模式不主动初始化数学字体。
- Preview 不可见时释放图片 decode cache；Hidden 状态释放公式 raster。
- 不缓存完整历史 preview；新结果替换旧结果后立即释放旧树。
- 不使用通用线程池；不引入自定义 allocator，除非基准证明明确收益。
- 不在每次按键复制整个文档；preview worker snapshot 时产生一次 `Arc<str>`。
- 空闲时不持续轮询。

## Release Profile（方向性）

```text
opt-level = 3、lto = fat、codegen-units = 1、panic = abort、strip symbols
```

可另设 release-size profile，但正式默认优先运行性能。

---

## 可靠性底线（Reliability）

| 项 | 约束 |
| --- | --- |
| 保存 | 原子替换；不产生半写文件；失败可见、不静默 |
| 冲突 | 脏 buffer + 外部变化必须显式 Conflict，不得偷偷覆盖 |
| 恢复 | 崩溃残留 temp 可被发现并供用户选择 |
| 资产 | 用户文件永不自动删除；GC 仅作用于可证明拥有的 managed 文件 |
| 退出 | 保存失败时保持运行并报错 |
| 观测 | 崩溃/关键故障记录 crash.log（架构内建，不事后补） |

---

## Failure Paths

- 超出 Hard Failure Condition：视为 release blocker，进入优化或风险报告流程。
- 无法定位原因的内存超标：按 Agent Stop Conditions 记录
  `docs/report/RISK-<topic>.md`，不绕过规格。

## Configuration

Not applicable（性能参数为内部固定值）。

## Lifecycle

Targets 在技术验证阶段实测校准；校准结论以 report + plan 更新形式落盘。Phase 10
先移除重复的 taskbar-list 初始化路径，使最终冷启动 cohort 的 p95 从 394.881 ms 降至
343.220 ms；仍无法稳定满足原 300 ms 门槛，因此按 USER 的明确授权将冷启动 hard gate
放宽为 400 ms。该结论是显式 waiver，不得回写成“原 300 ms 门槛通过”。热启动 hard
gate 仍为 180 ms，除非 USER 另行明确 waiver。

## Extension / Replacement Points

分配器、文本存储结构（String→rope）均为受基准驱动的替换点。

## Performance Critical Paths

见各能力章节；优化顺序固定（先查持续 redraw → 重复分配 → 无界 cache → …… → 最后才考虑 allocator），禁止凭感觉优化；
每项优化必须有 before benchmark / patch / after benchmark / regression test。

## Verification

基准入口在技术验证与 Phase 10 建立；手工验收含空闲 CPU 观察与隐藏后内存观察。

## Non-Goals

不承诺未测量数字；不做跨平台性能比较；不建立持续性能回归基础设施之外的遥测。
