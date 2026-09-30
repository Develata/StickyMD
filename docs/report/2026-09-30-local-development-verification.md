# 本地变更选测、兼容性夹具与耗时观察

日期：2026-09-30。实施基线：`d40eea948745521bf75a768ee32dcd4f7fbcc851`。
这是开发工具维护证据，不改变产品 runtime、发布权限、证据身份或人工验收状态。

## 背景与实施范围

提交型 CI 对 dirty worktree 保守全量是正确边界，但本地调用者此前需要手动选择模块。
多组 PowerShell 兼容性测试分别准备临时目录、宿主探测、编码和清理；已有资源和任务
日志也没有统一的只读耗时视图。这些维护成本不能用完整测试矩阵的运行时间代表。

本次按现有 plan 11 `modular-headless-ci` / `phase-verification-harness` 实施：

1. `dev-check` 读取 HEAD、index、worktree 与非 ignored 新文件，复用 CI 分类、反向依赖
   和 headless 任务图；`--plan` 给出路径、模块、命令和选择原因。
2. `tests/support` 集中独占临时目录、RAII 失败清理、PowerShell 5.1/7 宿主调用和有界诊断；
   各用例继续持有自己的真实断言，包路径检查增加 PS7 覆盖。
3. `timings` 读取既有 JSON/日志，分别展示本轮、嵌套、历史、Cargo 报告和固定等待预算。
   README 补充本地短检查与既有资源选测、失败优先、诊断续跑的使用顺序。

三项职责并行实现，Cargo 构建和最终检查集中协调；工具测试用独立进程和目录并发，
性能任务保持串行。本轮没有启动 GUI、资源 campaign、远程 workflow 或发布操作。

## 最终实现与规则归属

`development/{git,projection,mod}` 只负责本地事实采集和说明；路径归属、反向依赖和
检查范围继续由 `ci/selection`、module registry 与 `Checks` 持有，没有第二套分类规则。
一次 NUL porcelain 快照保留暂存后恢复为 HEAD 的改动，移动两端都参与选择。
冲突、未知路径、无有效 HEAD、传输错误和 registry drift 按请求 mode 回退完整 workspace。
完整回退使用 Cargo 的真实 workspace，显式 `modules run` 的 drift 拒绝接口保持不变。

`runner/headless/local` 复用原任务图和计时输出。适用的 test/build/clippy/dependency-policy
使用 locked 依赖。本地 Release/native gate 由 `local_build` 读取本轮 Cargo JSON 返回的
实际 EXE，验证 Windows manifest、bin target 和唯一成功完成记录；它尊重实际 target-dir，
不会读取旧 Promoted Candidate 或 Source Freeze 来代替新构建。本地成功不写资格化账本。

`timing_summary/{input,log,cargo,report}` 复用既有 JSON parser，输入全部验证后才输出。
重复路径别名、无效/缺失输入、非法数值、超过 64 MiB 或非普通文件均拒绝。
历史 receipt 格式 fixture 只验证读取语义，不能证明历史来源仍有效。
摘要固定为 `OBSERVATION_ONLY`，不合计重叠范围，wall-clock 与 agent 工时保持 unknown。
资源复用 case 的 `execution_seconds` 是本轮查验时间；只有明确的 origin 字段列为历史耗时。

生产 PowerShell 入口没有修改。ZIP、Syft、UIA/COM 和资源读取等 Windows 适配继续保留；
资源 sampling、资格化和人工边界也没有改动。本次没有新增依赖或更改 plan 合同。

## 审查发现与修复

并行审查发现本地 native gate 初版可能经旧候选 resolver 校验错误 EXE，现已绑定本轮
Cargo compiler-artifact；新增 malformed 旧收据与中文空格 target-dir 的真实构建回归。
正常资源日志还会混入没有 elapsed 的规划/复用行，现已区分这些行并加入实际格式测试。
非普通文件在打开前检查类型，避免直接打开 Unix FIFO 的阻塞；另保留打开句柄后的复核。

首轮完整测试曾发现新增自动化投影使用了治理不允许的 `NOT TESTED` 状态；改为已执行
用例的 `AUTOMATED PASS` 后重跑完整测试通过。严格 Clippy 发现的两个冗余闭包/clone
已修正，并重跑相关单元与 lint。人工行仍为 `NOT TESTED`。

## 实际验证

本轮日志保存在 ignored `target/development-tools-20260930/`，没有写入 release evidence。

| 检查 | 结果 |
| --- | --- |
| 实施前 `cargo test -p stickymd-smoke --locked ci::` | 18 passed |
| 最终 `cargo test -p stickymd-smoke --locked` | 324 unit passed、10 ignored；24 integration passed |
| Clippy 修正后 `development::` / `timing_summary::` targeted unit | 分别 10 / 12 passed |
| Windows ignored `actual_locked_local_build_ignores_stale_qualification_receipts` | 1 passed；locked/offline std-only tiny build，不启动 EXE；坏旧收据保持原文 |
| `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` | PASS |
| `cargo fmt --all --check`、`git diff --check` | PASS |
| 旧/新 CLI 同输入对照 | modules list、smoke module plan、dirty CI plan、Phase 14 route 的退出码与完整 stdout 相同 |
| 当前实际 `dev-check --plan` | shared planner 改动选择六个模块、九项任务，`NOT_RUN` |
| 独立中文空格目录的真实 docs-only `dev-check` | 只执行 governance / fmt 两项，PASS，不产生 qualification evidence；fixture 已移除 |
| 读取上述新日志与完整测试日志 | 两份输入独立、`OBSERVATION_ONLY`；没有推断 wall-clock / agent 工时 |
| PowerShell 5.1 / 7 实际 wrapper tests | 参数、中文空格路径、失败退出码、编码与 CWD 恢复通过；使用本轮预构建 CLI |

真实 Git 回归覆盖 staged/net-zero、unstaged、untracked、删除、移动、submodule 改动和
merge conflict，并检查只读规划不改 index。计划还覆盖未知输入、registry drift 和逐任务原因。
timings 覆盖缺失/重复/非法数值、来源与预算分离、任意 CWD 和后续输入失败不输出部分结果。

## 数据、收益与限制

本次结构检查中，既有上层 PowerShell 测试调用由 17 次变为 14 次：删除四次独立 PS7
探测、增加一次 PS7 包路径检查。这是调用数量，不能当成 wall-clock 加速比例。
共享夹具增加了清理和诊断回归；没有声称总源码行数下降。

本轮 docs-only 实测的两个 task elapsed 分别为 0.482385 s、2.050673 s。
完整 smoke 日志独立报告 Cargo test profile 10.07 s、unit binary 31.41 s、integration
binary 35.62 s。它们是本机本轮观察，包含缓存和调度条件，没有与等价旧实现做时间基准，
因此不据此声称性能提升，不推算“30–45 小时”能减少多少。

已证实的收益是规则只有一份、dirty 本地改动可自动选择并解释、兼容性准备统一，
且 docs-only 实际运行不执行代码测试。后续应先读取真实选测与耗时记录，再决定优化
编译等待、固定等待或重复运行；不应重建另一套 conformance 框架。

尚未验证：Linux 本轮编译/Clippy/执行及 Unix FIFO 回归、真实新增 workspace drift 的全量执行、
其他 target 配置、完整本地九任务执行、远程 CI、GUI/性能/完整资源 campaign、人工验收。
这些未验证项没有被短测试或历史日志标为通过，产品 readiness 不因此改变。

## Resolution — 2026-09-30 提交后复审

审查范围为 `1a357df`、`07083c3` 两批提交。发现并修复两项问题：

1. **P1，CRLF 检出导致负面测试误报。** `governance.json` 没有强制 LF 属性，Windows
   `core.autocrlf=true` 的新 worktree 会使用 CRLF。旧测试删除最后两个字节时只删掉换行，
   未损坏 JSON。隔离 smoke-only `dev-check` 因此以非零退出，单元结果为 323 passed /
   1 failed。现改为明确删除对象闭合符，分别验证 LF、CRLF 和无末尾换行的有效与损坏输入；
   不依赖检出换行策略来维持测试正确性。
2. **P2，CI 彩色日志漏掉编译耗时。** CI 设置 `CARGO_TERM_COLOR=always`，Cargo 的
   `Finished` 标签带 ANSI SGR 序列。实际 `cargo build --locked --color always` 成功日志
   被原 `timings` 以 `no recognized timing records` 拒绝。现由独立 `ansi` helper 在日志
   解析前忽略 SGR 颜色，保留行号与原文件；无颜色行不分配副本。相同原始日志修复后读出
   Cargo 报告的 0.26 s。非法耗时/状态仍拒绝，JSON 收据解析和观察范围保持原合同。

复验使用中文空格路径的新 worktree，保留 Git 实际检出的 CRLF JSON，只覆盖本轮修复的
Rust 源文件；不复制主工作树中的 LF fixture。完整执行 smoke-only `dev-check` 的五项
任务通过：治理、fmt、locked 严格 Clippy、locked cargo-deny、所选测试。其中单元
326 passed / 10 ignored，集成 24 passed（包含 PowerShell 5.1/7）；隔离 worktree 已移除。
主工作树另通过 14 项 timing targeted tests、fmt、diff 检查及更新文档后的 Phase 00。
新 `timings` 成功读取这次完整日志的五个任务观察，仍为 `OBSERVATION_ONLY`。

README、P00-A13 和覆盖映射同步补充彩色日志与换行兼容性。复现及复验日志位于 ignored
`target/review-cli-20260930/`。这关闭了 smoke-only 本地执行路径的验证缺口；完整九任务、
Linux、远程 CI、GUI/资源/性能 campaign 和人工验收仍未在本轮验证，没有推算性能收益。

## Resolution — 2026-09-30 push 后 CI 路径别名回归

已读取 source `619811ee94026fbb2f55fecbd77bbff2dd6d9e17` 的
[CI run 36787581253](https://github.com/Develata/StickyMD/actions/runs/36787581253)。
Windows tests 分片的 `headless` 集成目标为 23 passed / 1 failed，唯一失败是
`release_remote::actual_tag_and_draft_steps_reject_bad_observations_before_remote_mutations`，
Windows PowerShell 5.1 报 `Remote adapter changed caller state`。聚合门据此失败；
Linux smoke、portable-core、Windows lint、release/native-runtime、headless performance、
依赖政策与 plan/governance jobs 均通过。这补充的是该提交的远程无界面证据，不是新候选资格。

本地原有普通路径用例通过。定位发现断言把 `(Get-Location).Path` 与原始环境变量路径
直接比较；PowerShell 在首次 `Set-Location` 时就会展开路径别名，因此恢复成功也可能误报。
在同一 Rust 用例中只将调用者目录改为 `fixture/.` 对应的尾部点段路径后，旧断言稳定失败，
新增诊断明确显示 expected 有尾部 `\.`、observed 无尾部 `\.`。远端旧日志没有区分位置与
编码，也没有记录原始 TEMP 字符串，因此尚不能从该日志单独确认远端具体使用了哪种别名。

现按既有 package/phase wrapper 用例的口径，在进入目录后保存实际 caller location，
每个成功/拒绝分支完成后严格比较该位置，并独立保留 code page 936 的编码恢复断言。
Rust fixture 固定传入尾部点段，确保本地不依赖 NTFS 8.3 配置也能覆盖这一失效路径；
路径仍含中文与空格，使用原有 PowerShell 5.1/7 宿主，不增加子进程批次。生产适配脚本、
工作流、Rust 发布规则与授权边界均无需变更。

修复后 targeted `release_remote::` 通过，完整
`cargo test -p stickymd-smoke --locked --test headless` 为 24 passed / 0 failed，
包括双 PowerShell 宿主。`cargo fmt --all --check`、严格 locked Clippy、Phase 00 治理与 diff 检查通过。
集成日志保存在 ignored `target/ci-remote-20260930/headless.log`。
README、REL-CLI-17 与覆盖映射同步说明路径别名及实际 CWD/编码检查。
本次没有重跑 GUI、资源或完整性能 Campaign；修复提交的远程 CI 仍待下一次 push 验证。

## Resolution — 2026-09-30 修复提交的远程 CI 完成

用户 push 后，source `be1322e43624908b7ad04c7c7942d2c3ecb9ed09` 的
[CI run 36790130061](https://github.com/Develata/StickyMD/actions/runs/36790130061)
已完成，结论为 `success`。九个 jobs（含最终 `CI result`）全部成功，覆盖 plan/governance、
Linux smoke CLI lint/tests、Windows release build、依赖政策、Windows fmt/lint、
Linux portable-core、Windows headless tests 与 performance。此前失败的 Windows tests
分片通过；这补齐该修复提交的远程 CI 验证，不反推旧失败日志中具体 TEMP 别名的形式。

此次观察未触发或重跑远程工作流；运行来自用户 push。普通 CI 成功不产生新的 Source Freeze、
Promoted Candidate、资源 campaign 或人工验收证据，也不提升 `v0.1.1` 的技术 readiness。
