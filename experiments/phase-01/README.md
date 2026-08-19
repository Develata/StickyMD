# experiments/phase-01 — Phase 1 Technical Risk Spikes

本目录是 **experimental** 技术验证代码，不是正式骨架实现（宪法 9.3）。

规则：

- 每个子目录是**独立 crate**（自带空 `[workspace]`），不属于生产 workspace。
- 删除整个 `experiments/phase-01` 后，生产 workspace 必须仍可构建。
- Spike 代码不得演化为产品；正式化决定只能在 Phase 1 结论之后做出。
- Spike 与生产代码之间不允许出现反向依赖：production crates 不依赖 experiments。

## 子目录

| Spike | 验证目标 | 结果 |
| --- | --- | --- |
| `window/` | winit + softbuffer + tiny-skia、idle 行为、DPI、opacity、圆角 | [RESULTS](window/RESULTS.md) |
| `ime/` | cosmic-text 编辑 + winit IME + 字体 run | [RESULTS](ime/RESULTS.md) |
| `markdown-math/` | Comrak owned AST + RaTeX 管线 + benchmark | [RESULTS](markdown-math/RESULTS.md) |
| `persistence/` | canonical dir、单实例、atomic save、recovery、冲突模型 | [RESULTS](persistence/RESULTS.md) |

## 运行方式

每个 spike 是独立 crate：

```powershell
cd experiments/phase-01/<spike>
cargo run --release
```

## 汇总

- 总报告：`docs/report/phase-01-technical-spike-report.md`
- 依赖基线：`docs/report/phase-01-dependency-baseline.md`
- Windows API 基线：`docs/report/phase-01-windows-api-baseline.md`
- 性能基线：`docs/report/phase-01-performance-baseline.md`
