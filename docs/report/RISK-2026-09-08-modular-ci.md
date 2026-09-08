# 模块化 CI 与 PowerShell 职责迁移影响分析

日期：2026-09-08。基线 `c5300ab11ac64370c308aa3464f6e2f11376b2b7`。
工作树包含此前的未提交维护修改；本任务保留这些修改。

## 已核实事实与问题

- `.github/workflows/ci.yml` 的每次 main push / PR 都运行 Windows workspace tests、全部
  headless Release performance、workspace Clippy/build，以及 Linux core/render 验证。
- tests/performance 的职责分片已存在，但还没有按变更选择的工程模块入口。
- `qualification/module_ledger.rs` 的六个模块为 Runtime、Performance、Resources、G3、G4、G5；
  它们证明候选功能资格化，不能把普通 CI 的未运行测试标成它们的 `REUSED PASS`。
- `docs/plan/11_testing_and_release.md` 的 CI 与完成门要求每次 Windows CI 执行完整
  `all --ci` 或任务并集等价的 tests/performance 分片。日常增量 CI 需要明确调整这条合同。
- PowerShell 的阶段入口已经很薄。`package-path.ps1` 仍持有包名选择与 Git 状态判断，
  可以先迁入 std-only Rust CLI；ZIP/原生 UI 适配不必随之整体重写。

## 拟调整的边界

1. Rust 持有 headless CI 模块定义、任务归属、依赖传播和选择计划；工作流只提供事件事实、
   隔离 runner、缓存和执行参数，不另写一套路径分类或 PASS/FAIL 规则。
2. 第一层模块按现有 Cargo 边界划分：core、render、windows、smoke，以及两个 Phase 01
   实验 crate。测试/性能任务属于唯一模块；完整模块任务并集必须与完整入口等价。
3. 本地可指定一个或多个模块。日常 push / PR 可以按 Git 变更选择直接模块及反向依赖；
   core 变化覆盖 core/render/windows，render 变化覆盖 render/windows，smoke 变化覆盖 tooling。
4. Cargo manifest/lock、toolchain、共享构建配置、CI/规划器本身及契约变化保守选择全部；
   未知路径、无法确定基线或比较失败也回退全量，不静默忽略。纯报告/说明文档仍运行治理检查。
5. 完整 `all --ci` 保留；手动完整模式、定时完整检查及发布流程使用完整集合。
   日常部分测试的成功只证明被选模块，必须在输出中列出选择范围及原因，不能充当
   完整 headless qualification receipt、exact-byte gate 或人工验收证据。
6. 暂不改变候选功能 last-success 账本格式。CI 的 Cargo 缓存只复用构建产物，不伪装为测试成功。
   GitHub jobs 保持隔离；本地同一桌面的 GUI、IME、焦点、剪贴板和资源测量仍然串行。

## 收益、代价与失败路径

预期收益是减少无关测试与重复构建等待，而不是降低产品运行时内存。具体节约量需要远程
workflow receipt 才能确认，本地任务数量不能替代远程耗时实测。

主要风险是模块映射漏项、跨模块依赖漏传、Git 比较基线缺失，以及把部分成功误认作完整成功。
用未知输入全量回退、依赖闭包测试、完整任务并集测试、明确的计划/收据范围，以及定时全量
复核约束这些风险。工作流不采用高权限自托管 runner，也不在 PR 中使用发布凭据。

兼容性：现有 Phase PowerShell 路径及完整 CLI 保留；包路径助手保留 PowerShell 函数签名，
将选择逻辑委托 Rust。旧 tag、已发布 ZIP、Source Freeze 与 Promoted Candidate 不变化。
回滚可恢复 CI 对完整入口的调用；包路径迁移与 CI 选择是独立切片，不互相依赖。

## 实施与验证切片

- A：迁移包路径选择；验证单包、多包、clean/dirty、缺失/歧义及带空格路径，PS 只转发。
- B：模块执行入口；验证单模块、重复选择去重、未知模块拒绝、任务唯一归属与完整并集。
- C：变更选择与 GitHub matrix；验证新增/删除/重命名、依赖传播、共享与未知输入回退、
  基线缺失、空矩阵处理、失败/取消传播、JSON 和工作流静态结构。
- D：定向 Rust tests、fmt/Clippy、Phase 00 治理；远程运行、耗时、发布/桌面证据保持未验证，
  除非 USER 后续明确授权相应外部操作。

## 待 USER 决定

请求批准上述 CI 合同调整：允许日常 push / PR 按受影响模块及依赖执行，保留完整入口、
手动/定时完整检查与发布证据门。批准前不修改现行全量 CI 要求及选择性跳过行为。
已授权、现有合同可覆盖的包路径职责迁移可以独立推进。

## Resolution — 2026-09-08

USER 已在本任务明确回复“可以按你的推荐方案来”，批准上述 CI 合同调整。
按本报告的保守选测、反向依赖、全量回退、完整入口及发布证据边界实施；
此批准不包含 push、远程 workflow dispatch、tag 或发布操作。
