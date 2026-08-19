# phase-01-technical-spike-report.md - Phase 1 技术 Spike 总报告（决策）

- `Date`: 2026-08-19
- `Type`: 阶段技术验证决策报告
- `Status`: Completed，待 USER 批准（Phase 2 前置门）

plan_ref: docs/plan/03_system_architecture.md ; docs/plan/06_markdown_math_rendering.md ;
docs/plan/07_editor_and_ime.md ; docs/plan/09_windows_shell.md ; docs/plan/10_performance_reliability.md

配套基线：
- `phase-01-dependency-baseline.md`（版本 + 禁用项审计）
- `phase-01-windows-api-baseline.md`（Win32 触达清单）
- `phase-01-performance-baseline.md`（性能数据）

---

## 1. Spike 清单与证据位置

| 编号 | 主题 | 目录 | 证据 |
| --- | --- | --- | --- |
| 1A | workspace foundation | 根 workspace | 生产构建通过 |
| 1B | 窗口/帧缓冲 | `experiments/phase-01/window` | `RESULTS.md` |
| 1C | 文本/IME | `experiments/phase-01/text` | `RESULTS.md` |
| 1D | Markdown/数学 | `experiments/phase-01/markdown` | `RESULTS.md` + `COMRAK_NOTES.md` |
| 1E | 持久化 | `experiments/phase-01/persistence` | `RESULTS.md`（18 测试通过） |

## 2. Executive Decision（按技术路径）

判定语义：**PASS** = 可无条件采用；**CONDITIONAL** = 采用但附必须跟踪的条件；
**FAIL** = 该路径不可用，需换方案。

### 2.1 呈现链路（winit 0.30 + softbuffer 0.4 + tiny-skia 0.12）— **PASS**

- 证据：idle CPU 0%、无持续重绘、WS 18.31 MB 恒定；dirty-only 重绘；
  Win32 不透明度/圆角薄适配可用。
- 条件（非阻塞）：真实 DPI/多显示器复测；damage/局部重绘在生产评估。

### 2.2 文本塑形与投影模型（cosmic-text 0.19 + canonical String 投影）— **CONDITIONAL**

- 证据：canonical `String` → cosmic-text Buffer（`set_rich_text` 作投影）成立；
  script-based 字体 runs（CJK/Latin 分段）成立；undo；剪贴板适配成立。
  进程内 SELFTEST：insert=PASS、undo=PASS、clipboard self-check=PASS。
- **条件（关键）**：**交互式 IME 组合输入（微软拼音/微信输入法等）为人工项，
  Phase 1 NOT TESTED**。进入编辑器里程碑前必须完成人工 IME 验证；若失败，按
  plan 07 评估 RichEdit fallback（平级实现）。
- 已知字体事实：本机 `仿宋_GB2312` 未安装、`FangSong`/`Times New Roman` 存在。

### 2.3 Markdown 解析（Comrak 0.54）— **CONDITIONAL**

- 证据：arena→owned 投影成立（sourcepos 保留、arena 可丢弃）；GFM table/tasklist/
  strikethrough、raw HTML 字面保留；4 种数学定界符均解析并渲染。
- **条件 1（语义约束）**：`NodeMath.dollar_math` 对所有 math 节点恒为 true（Comrak
  硬编码），**无法区分 `$`/`\(` 或 `$$`/`\[` 的定界符风格**，仅 `display_math` 区分
  inline/display。若需回显源风格，须经 sourcepos 回扫原文。详见 `COMRAK_NOTES.md §3`。
- **条件 2（性能）**：1024 KiB 全量重解析 ~126 ms，超单帧预算；生产须增量/视口裁剪。

### 2.4 数学渲染（RaTeX 0.1.14 parser/layout/render）— **PASS**

- 证据：parse→layout→display-list→PNG 全链路 5/5 成功；错误路径返回结构化
  ParseError（含位置），可降级显示原文，不 panic；`embed-fonts` 免外部字体。
- 条件（非阻塞）：生产渲染桥接按 plan 06 方案 A（上游 `render_into_pixmap`）或
  方案 B（PNG→位图贴图）落地，二选一在 Phase 2+ 定。

### 2.5 持久化原语（Win32 适配）— **CONDITIONAL**

- 证据：canonical dir→SHA-256 身份成立（大小写归一、稳定、可区分）；跨进程单实例
  （mutex+event）成立；原子保存无残留/无半文件、CRLF 转换成立；恢复/冲突规则
  18 测试通过；clippy 0 warning。
- **条件（NOT TESTED，需后续复测）**：真实 junction/symlink 解析、ACL/只读卷权限
  拒绝、kill-mid-save 端到端残留、FlushFileBuffers 断电持久性（硬件级）。这些是
  环境/硬件限制，**判定为 NOT TESTED 而非 FAIL**。

### 2.6 判定汇总

| 路径 | 判定 |
| --- | --- |
| 呈现链路 | **PASS** |
| 文本/IME（cosmic-text） | **CONDITIONAL**（人工 IME 待验证） |
| Markdown（Comrak） | **CONDITIONAL**（定界符风格折叠 + 增量解析） |
| 数学（RaTeX） | **PASS** |
| 持久化（Win32） | **CONDITIONAL**（4 项环境复测） |

**无 FAIL 路径。** 冻结技术栈全部可用；条件项均已被明确识别并有落地方案。

## 3. Recommendation（供 USER 决策）

### Recommendation A —— 按冻结栈直接进入 Phase 2（**推荐**）

- 内容：五条路径全部采用已验证的冻结栈；将 §2 各 CONDITIONAL 作为**跟踪项/里程碑
  门禁**写入后续阶段，而非阻塞当前推进。
- 依据：无 FAIL；条件项均为「人工验证/环境复测/性能策略」类，可并行安排，不影响
  Phase 2（核心文档模型）的纯逻辑实现。
- 风险：人工 IME 若后续失败，需在编辑器里程碑前切换 RichEdit fallback（plan 07 已
  预留平级实现，成本可控）。

### Recommendation B —— 采用但先关闭关键条件门

- 内容：进入 Phase 2 的同时，**优先安排**交互式 IME 人工验证与 ≥1 项持久化环境复测
  （建议真实 junction + ACL），在 Phase 2 完成前回报结果。
- 依据：把最大不确定性（IME）尽早收敛。
- 代价：Phase 2 前需插入人工/环境验证窗口，排期略延长。

### Recommendation C —— 对单一路径重 spike

- 内容：仅当 USER 对某路径证据不满意时，针对性重做 spike（例如 IME 改用独立 IME
  测试程序、或 Comrak 换解析器对比）。
- 依据：保留回退。
- 代价：延迟 Phase 2；当前证据下**无必要**。

**建议：Recommendation A。**

## 4. 对规格的影响

- 不需要修改 `docs/plan` 的骨架契约。两条语义/性能事实（Comrak 定界符折叠、巨型
  文档增量解析）应作为**实现约束**反映在 Phase 2+ 的设计文档与 plan 06/10 的
  后续细化中，但不构成契约变更。
- 依赖冻结版本清单见 `phase-01-dependency-baseline.md`，待批准后可写入生产 workspace。

## 5. Phase 2 前置门状态

- 门条件：Phase 1 recommendation = APPROVE / WITH CONDITIONS（USER 接受）。
- 当前：本报告给出 Recommendation A（PASS 无 FAIL，附条件项跟踪）。
- **待 USER 批准后进入 Phase 2（核心文档模型）。**
