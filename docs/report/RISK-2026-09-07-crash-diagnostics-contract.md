# crash.log 合同与当前实现差异（2026-09-07）

基线：`main` / `37fa6e0b6da3ae83c16bb29a42b605ad078ac3c2`。本报告不是新合同，也不修改 `v0.1.0` 发布身份。

**已核实事实**：`docs/plan/05_document_persistence.md` 的 portable 目录和 `10_performance_reliability.md` 的可靠性底线承诺 panic/关键故障创建 `crash.log`。当前 `apps/`、`crates/` 没有 `crash.log` 或 `set_hook` 实现；Windows 入口没有安装持久化 panic hook。`CONTRIBUTING.md#collect-diagnostics` 和 Bug 模板则准确说明当前没有原生日志，使用有限 stderr 和 Windows Application Error 辅助诊断。

复查方式：`rg -n 'crash.log|set_hook' apps crates docs/plan CONTRIBUTING.md .github/ISSUE_TEMPLATE`，并追踪 `apps/stickymd-win/src/main.rs`、`startup/` 与 persistence adapter。没有故意让用户实例崩溃，也没有创建日志或采集私人便签。

**基于证据的判断**：这是已批准合同与实现之间的缺口。公共说明如实描述发布现状，不构成撤销 plan 的授权。不能仅修改公共说明，声称日志已经存在；也不能直接修改 plan 迁就当前代码。

已评估的两个路径：

1. 补全最小本地诊断。建议先明确仅包含构建身份、固定错误分类、受控组件标识和时间，不写便签/剪贴板/自由文本 panic payload、用户路径或内存 dump；不引入上传、后台日志线程或常驻轮询。需要确定单文件上限、保留与原子替换规则，以及 panic/磁盘满/目录不可写时不阻塞退出、不递归 panic、不干扰 canonical note 的行为。panic 时复用可能已持锁的 I/O 队列有死锁风险，不能直接把普通保存接口塞进 hook。
2. 由维护者明确批准收缩观测合同，保留现行 stderr/Windows 事件诊断。实现代价较低，但会放弃应用内持久故障证据；必须先更新 plan，再同步投影。

推荐先确定路径 1 的记录字段、边界与故障协议，再实施。当前文档只规定存在日志，没有完整定义这些关键行为。现有用户授权覆盖普通修复，但同时要求数据边界、关键接口变化先审批，因此本轮没有新增日志写入路径。

后续验证应包括：独立合成进程 panic、错误分类脱敏、受控容量、已有日志替换、目录不可写、磁盘失败、并发故障、不改变 `note.md`、不向 AppData/Registry 写入。真实 Windows 崩溃事件、abort 和断电不等同于 Rust panic，覆盖范围必须分别说明。

**尚未验证**：故障发生率、真实用户诊断困难的频率、最小诊断实现的运行代价，以及异常终止时日志的可靠落盘程度。本报告没有给这些项目填入 PASS。

## Resolution — 2026-09-25

在 `c5606fe` 基线上重新检索 `apps/`、`crates/`、plan 与 CONTRIBUTING，仍未发现
`crash.log` 或持久 panic hook 的实现；计划承诺和公开诊断说明之间的上述差异仍然存在。
本次对已有运行时维护补丁的复审没有实施日志协议，也没有修改 plan 或升级此项验收状态。
