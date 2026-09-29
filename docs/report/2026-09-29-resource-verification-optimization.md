# 资源验收去重与模块成功复用（2026-09-29）

## 状态与授权

USER 在本次对话阅读三批方案后明确回复“可以按你的建议来改”。授权范围是验证工具的
场景去重、完整覆盖检查、共享依赖纠正、诊断计时与五类资源成功记录；不改变产品行为、
采样次数、预热/CPU 区间、性能阈值或桌面独占要求。本文先于实现写入；当前合同以
`docs/plan/11_testing_and_release.md` 为准。

## 背景、证据与根因

基线为 `c204afa836a3fe679513b3347f40cb0e4252b79f`，工作树干净。9 月 29 日历史桌面运行
Resources 耗时 8,808.412 秒，占六个桌面通道累计耗时约 91.5%。该次资源结果后来因样本
问题无效；这里只使用其执行时间，不恢复旧资格。

按 `runtime.rs` 场景及生成规则静态核对，基础/数学/图片组有 19 个名称、15 种独立测量：
Source/Preview/Split 的 20 公式各重复一次，空 Preview 也重复一次。固定等待可少
`3 × 5 × (30 + 60) + 5 × 30 = 1500` 秒。此为计划预算，不是实测加速。
窗口与 zoom 场景、Preview→Source 释放过程不参加上述去重。

另有三个实现问题：资源完整性仅由一个窗口任务代表；成功入口按文件路径识别模块而
未绑定请求范围；共享窗口/进程适配器未使所有实际使用它们的模块失效。旧 Resources
只有整组成功身份，checkpoint 仅为 INCOMPLETE 诊断，不能直接当作可恢复成功。

已尝试只读代码检查、历史日志计数和内存中的样本/配置 hash 比对；未为本次设计占用桌面。
现有 smoke 基线为 184 unit、9 CLI、2 wrapper 测试通过。

## 方案、职责与失败路径

1. Rust 资源注册表统一声明场景、操作历史、分组、固定预算与覆盖要求；执行器和收据校验
   消费同一注册表。每次命令只共享相同测量协议和初始输入的完整样本；别名明确记录样本来源。
2. 保留既有诊断入口。只有 Phase 14 完整 Resources 请求、canonical aggregate 输出、
   clean worktree 和有效 Promoted Candidate 可以更新五类子模块账本。选组/选 case 诊断
   不写正式账本；输出路径不能把诊断提升为正式验收。
3. 五类子模块为 source-preview、math、images、window、zoom。完成全部预期场景才原子
   登记成功；后续失败不抹除其他已完成模块。下一次完整入口重新计算兼容性，跳过兼容成功。
   窗口压力循环中断后整体重跑，不合并部分统计 cohort。
4. readiness 直接要求五类兼容成功，不信任 aggregate 的单个标志。旧 aggregate/诊断
   不自动迁移；原证据保留。每份复用必须保留 origin source/EXE/ZIP 与证据 hash。
5. 输入依赖继续保守传播。先修共享适配器漏边，再把资源执行器按职责拆出；共享构建输入、
   未分类输入仍保守失效。每次记录前后检查指纹，输入在测量过程中改变则拒绝登记成功。
6. 输出计划预算、逐模块/场景实际耗时、复用来源及失效原因。样本结构预检在等待前执行。

## 替代方案、代价与回退

- 仅加计时：可诊断，但不减少重复固定等待。
- 缩短采样或同桌面并发：改变测量语义，不在本次授权方案内。
- 只去重：预计少 25 分钟固定等待，但局部修改仍需整组重跑。
- 采用本方案：新增有界的场景注册和资源子模块收据；代价是一次新的资源资格化，以及
  更严格的覆盖/指纹回归。不会引入依赖或产品 runtime 开销。

回退实现时保留所有旧/新证据，不自动导入新账本到旧工具；由对应版本工具重新判定。
精确 artifact 检查继续绑定当前字节。旧发布 tag、资产和 USER 特例不受本次改动改写。

## 验证计划

无界面验证覆盖：19 名称映射 15 执行、1500 秒预算差、操作历史不误合并、失败不缓存、
缺项/重复项/筛选请求不得登记成功、共享依赖传播、资源输入隔离、失败保留兄弟成功、
旧记录不迁移、变更后拒绝登记、JSON/CLI 与包装器兼容。执行 fmt、严格 Clippy 和 smoke tests。
真实 GUI、CPU/内存数值与耗时收益仍须在独占 Windows 桌面另行验证，保持 NOT TESTED。

## Resolution（2026-09-29）

三批改动已在本地实现。资源场景、覆盖与预算由同一注册表持有；原生 Window/Zoom
执行器按职责拆出，五组成功分别归档。完整请求只复用兼容组；共享样本保留实际来源，
不从历史收据拼成新的 cohort。正式路径有请求范围防护及完整覆盖校验，通用 JSON
输出入口也拒绝写入 coordinator 持有的资源子收据。共享窗口/进程适配器已补全失效传播；
资源产品输入继续保守覆盖，共享 harness 变化使全部资源组失效，专属 Window/Zoom
harness 变化按组失效。指纹测试独立成文件以保持职责清楚。

在 Windows x64 上完成：

- `cargo test -p stickymd-smoke --locked --quiet`：196 unit、10 CLI、2 PowerShell wrapper，
  合计 208 项通过；包括真实 fixture bytes/config 等价性、五组缺失分别阻塞、候选/指纹
  变化拒绝登记、局部 CLI 请求拒绝并保留原始收据。
- `cargo fmt --all -- --check`、`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`
  与 `git diff --check` 通过。
- Phase 00 JSON governance 通过；`qualification modules` 只读检查正常，显示五个新资源组
  均为 `NO_LAST_SUCCESS`，没有把旧 aggregate 导入。现有其他成功记录按当前指纹判定失效。

本轮未启动原生 GUI/CPU/内存验收，P14-A46 保持 NOT TESTED；也未实测端到端耗时，
25 分钟仅为固定等待预算减少。未运行 Linux 构建（本机仅安装 Windows Rust target）。
下一轮正式资格化需要新的 Source Freeze / Promoted Candidate，首次建立五组完整证据。
变更仅涉及验收工具及其合同/投影/报告，没有更改产品运行时或依赖；未 commit、push 或发布。

## Resolution（2026-09-29，第二轮：候选构建与失败提前退出）

USER 确认继续优化后，本轮落实上一轮建议中优先级最高的两项：

1. Runtime/Performance/Resources 使用 Promoted Candidate 时，任务图移除不会用于测量的
   本地 Release EXE 构建，改为明确命名的候选校验，并在 headless 基准/桌面任务前执行。
   校验复用现有 Source Freeze、clean worktree、EXE/ZIP/SBOM hash、checksum 和 PE/native
   dependency 规则。缺失或损坏候选不能回退到本地 EXE。正式 Runtime/Performance 覆盖
   校验要求这个新任务；preflight/package/release/CI 原有构建图不变。
2. 现有 CPU any、隐藏窗口/缩放内存 max 门改为每个完整样本后判断。CPU 仍等完整
   60 秒窗口，不根据其中 10 秒分桶提前判失败；PASS 仍需五次样本与完整压力循环。
   收据保留超限样本、之前的观测、实际次数与门槛，部分 cohort 不生成完整统计。
   缩放增长超限也保留实际差值，负增长不会被误判失败。startup p95 流程未修改。

失败结果从资源执行器传到 runner 的统一 JSON 收尾路径，返回非零并停止后续任务。
失败和证据登记失败都保留本轮已收集的数据，不更新成功账本；清理失败保留原错误并
补充清理错误。新 cohort 与候选规划文件纳入实际消费者的输入指纹。对应 P14-A47/A48，
P14-A46 继续持有独占桌面的真实验收缺口。

验证结果（Windows x64）：

- 全量 smoke：206 unit、10 CLI、2 PowerShell wrapper，合计 218 项通过。
- 边界回归覆盖首轮/后续轮/最后一轮 CPU 失败、阈值相等仍完整采样、内存超限观测保留、
  负增长、候选缺失/损坏不构建、不继续后续任务、正式覆盖及五类账本不受失败影响。
- 最后调整门槛来源说明后，4 项 cohort 定向回归再次通过。
- fmt、严格 Clippy、`git diff --check`、Phase 00 JSON governance 通过。
  Phase 00 的通过只证明工具与文档治理，不代表 release readiness 或新候选验收。

一个五轮 CPU cohort 若首轮已经失败，可省掉后续四轮的固定等待：
`4 × (30 + 60) = 360 秒`，即 6 分钟；不含其他未再执行的场景。
这是执行计划预算，并非本机实测加速。候选构建省时未量化。
本轮未运行完整原生 GUI/CPU/内存验收，也未补充 Linux 构建。
进一步细分产品依赖与跨通道 headless 证据复用仍需独立核对输入/覆盖规则，本轮未实施。
没有修改产品 runtime、依赖或发布状态；未 commit、push 或发布。

## Resolution（2026-09-29，第三轮：共享前置测试、规划去重与计时）

USER 回复“按你的建议继续”后，本轮落实 Runtime/Performance 共享完整 workspace tests、
资源规划指纹去重与细分计时；对更细的资源产品依赖只做静态审查。
先补充 plan 11 `shared-headless-prerequisite`，再实现 Rust 工具与 P14-A49..A51 投影。

### 实施与边界

- 仅完整 Phase 14 Runtime/Performance 的 canonical 请求可以复用
  `cargo test --workspace --locked`。原任务覆盖要求不变，独立的 source-bound 成功保存在
  `dist/evidence/source-success/workspace-tests.json`，不替代 headless CI 全集或任何原生测量。
  成功绑定 clean Source Freeze、全部 Git 枚举的仓库输入 bytes、实际 smoke EXE、Rust/Cargo、
  主机/工作目录及执行设置；执行前后及复用前重新核对身份。
- 测试失败、输入变化或成功文件写入失败返回失败，保留旧成功；后续桌面失败不撤销已完成
  的完整前置测试。损坏/不匹配收据导致重跑。partial、诊断和 CI 不创建共享记录，公开输出
  不能覆盖该内部收据。复用明确标明 SOURCE_BOUND、REUSED_PASS、来源 source 与指纹。
- 配置处理采取明确的支持范围：空 Cargo 配置及现有静态 MSVC CRT 配置可共享；未知的
  Rust/Cargo 覆盖、StickyMD 测试过滤或其他 Cargo 配置照常完整执行，不使用或写入缓存。
  实际回归发现 Cargo 注入的 `RUSTUP_TOOLCHAIN_SOURCE` / `RUST_RECURSION_COUNT` 会触发
  初版保守规则，已将这两项纳入身份材料；本机普通 Cargo 环境随后实测走到共享分支。
  收据只保留摘要，不输出原始环境或配置值。
- 资源规划一次枚举路径、每组计算一次摘要，并将该摘要传给兼容性判断；后者继续校验
  成功 schema、归档 hash、完整覆盖及 origin。复用与登记仍独立读取当前输入，不能使用
  规划快照代替最终检查。原摘要字节协议不变。核对发现 `rerun_reason` 原本只检查文件是否
  存在，未重复哈希，因此没有修改它。
- runner 统一记录每项任务实际耗时并保留失败观测；共享前置检查分别记录身份核对和
  本轮测试时间。复用时的 `workspace.origin_run_seconds` 只表示历史用时。
  资源规划输出总计与各组摘要耗时。新增职责分别位于 `workspace_tests`、其 `identity`
  子模块和 `runner/timing`，未新增依赖或产品运行时对象。

### 资源产品依赖审查

| 场景 | 当前代码可确认的跨域行为 | 决定 |
| --- | --- | --- |
| Source/Preview、math、images | 共用同一 EXE、启动与布局准备；Source/Preview 切换及 worker 生命周期互相关联 | 保留全部产品输入 |
| window | `runtime/resources/window.rs` 调用 `run_window_leak_cycles`；该流程包含保存、外部冲突和图片压力，结束后恢复固定文档 | 不能缩成只有窗口文件 |
| zoom | Split fixture 包含 20 个公式和 12 张图片，执行 toolbar 切换及 zoom relayout/cache growth | 不能缩成只有缩放文件 |

`Cargo.toml` 的 Release 仍为 fat LTO / 单 codegen unit。静态审查不能证明“某功能路径未被
显式操作”就排除了该产品变更对真实 CPU/内存的影响，因此五组继续使用 `ALL_PRODUCT`。
此前已落实的 window/zoom 专属 harness 隔离保留。下一步应先获取完整新候选的计时与资源
证据，再评估是否值得进一步拆分产品依赖；本轮不宣称该项已经优化。

### 本轮验证

- `cargo test -p stickymd-smoke --locked --quiet`：220 unit、10 CLI、2 PowerShell wrapper，
  共 232 项通过。相对上一轮增加 14 项，覆盖共享成功/拒绝条件、真实 Git/freeze 身份、
  规划摘要、新输入/归档变化和计时保留。
- 真实 Git fixture 创建干净提交与 Source Freeze，实际核对工具链、EXE、Cargo 配置和输入
  bytes，随后编辑源码验证拒绝执行。其测试执行器是受控回调，不是实际桌面验收。
- fmt、严格 smoke Clippy `--all-targets -- -D warnings` 与 `git diff --check` 通过；
  编译后的 CLI Phase 00 JSON 回归通过，新增计时不破坏单份 JSON stdout。

未运行完整原生 GUI/CPU/内存资格化或 Linux 构建，P14-A46 保持 NOT TESTED；未实测新候选
Runtime→Performance 的端到端节省。新计时字段用于下一次定位剩余耗时，不能据此补写
已经实现的加速百分比。变更保留在本地工作树，未 commit、push 或发布。

## Resolution（2026-09-29，完整独立 review 与修复）

USER 明确要求派一个 subagent 独立 review 当前全部修改并修复问题。独立审查者从空白
对话上下文读取工作树、HEAD 差异及契约，覆盖本次三轮优化的 20 个 tracked 修改与 17 个
untracked 文件；审查时主工作树保持不变。审查者在自己的临时源码副本复现以下 3 项 P2，
然后由主 agent 实现修复并交回独立复核。

1. **相对 Cargo 配置路径漏入身份。** 从仓库子目录启动、`CARGO_HOME=cache` 时，
   身份捕获读取调用者目录下的配置，实际 Cargo 则使用仓库工作目录下的配置。
   后者改成非法 TOML 后，旧实现仍保留同一可复用指纹，而真实 Cargo 报解析失败。
   修复为按 Cargo 子进程工作目录解析。新增独立子进程回归在旧实现上失败，修复后通过。
2. **资源组原始观测缺失，硬门结果未复核。** 共享 cache 原来仅保存统计；独立的 math
   成功记录缺少三个共享 cohort 的原始 samples/gates。完整性验证只核对计数和统计项，
   允许无样本、无门槛甚至超出既有硬门的 PASSED 收据。现改为缓存完整观测，复用样本以
   `shared_from` 标明同一来源，子归档自包含。`resource_plan/observations.rs` 核对每个
   cohort 的五个唯一 run、单位、有限/非负观测、统计一致性、等价共享来源与适用硬门。
   既有 CPU 0.1%、hidden 36 MiB、zoom 64 MiB 和增长 8 MiB 阈值集中到同一注册表；
   未给原来没有内存硬门的基础/压力样本新增阈值，负增长仍合法。
3. **Windows 文件路径别名绕过内部收据保护。** `canonicalize()` 返回的 verbatim 路径
   未与普通路径统一，失败诊断可覆盖同一物理 shared-success 文件。现先解析文件或最近
   存在父目录的文件系统身份，再判断正式/内部路径；入口执行前和 emit 写入前均保护。
   回归覆盖 verbatim、Windows 短路径、junction 和尚未创建的目标，验证旧成功 bytes 保留。

修复补齐现有 P14-A42/A43/A49 的可验证条件，同步 plan、验收投影、coverage、README 与
Phase 14 薄入口注释。原始样本、来源和门槛校验是工具内部证据完整性修复，不改变产品行为，
也不将共享样本算成额外独立测量。此前失败留下的旧成功仍保留，但不完整或不兼容的旧资源
记录不能继续参与 readiness。

定向回归：resource 21 项与 qualification 73 项通过，包含真实 Git/freeze、Cargo 配置与
Windows 文件系统别名 fixture。没有创建主仓库 Source Freeze/候选或正式测量收据，
没有启动原生桌面验收。独立审查未把 HEAD 已有的 archive 并发窗口算成本次新增问题；
Linux、原生资源、并发压力测试及实际端到端提速仍未验证。

独立复核补充确认第一项的两个路径边界：空 `CARGO_HOME` 使真实 Cargo 回退到用户目录，
Windows `C:cache` 则依赖进程的盘符工作目录。现对空值使用用户目录回退，对拼接后仍不是
绝对路径的配置禁用共享并正常运行完整测试。同一隔离子进程回归扩展为普通相对路径、空值、
Windows 盘符相对路径三种输入，全部通过；审查者独立重跑的后两种复现也已转为通过。

最终独立复核结论：3 项 P2 全部关闭，未发现新增或未关闭问题。审查者对全部修复完成静态
复核，并独立重跑上述两个追加边界；全量工具基线由主 agent 执行，没有将两者混写成双份
独立全量运行。

验证结果：`cargo test -p stickymd-smoke --locked --quiet` 通过 227 项单元测试、10 项 CLI
测试及 2 项 wrapper 测试，共 239 项。随后增加的配置路径边界通过对应真实 Cargo 定向回归。
严格 `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`、
`cargo fmt --all -- --check` 与 `git diff --check` 均通过。耗时不是性能基准；P14-A46 仍为
NOT TESTED。所有修复保留在本地工作树，未 commit、push 或发布。

最终工作树的 `cargo run -p stickymd-smoke --locked --quiet -- phase 00 --json` 返回零，
stdout 可作为单份 JSON 解析；governance contracts 与 acceptance readiness 两项均为
PASSED。此处仅是 Phase 00 文档/工具治理检查，不代表产品 release readiness 或原生资格化。

## Resolution（2026-09-29，第四轮：共享输入单次读取与缓冲指纹）

USER 回复“继续优化”后，检查剩余重复工作：资源规划已经只枚举一次 Git 文件清单，
但五组仍逐组打开相同源码、写入未缓冲的临时流并执行 `sync_all`。首次定向计时观察到
1,798 次输入打开、18,856,883 bytes 源码读取，五组合计约 3.87 秒（包含该计时探针额外的
文件计数读取）；这只证明规划开销，不是完整 Resources 实测。

现由 `module_ledger/fingerprint/stream` 统一持有单组与批量的 v1 字节流：同一批次按原排序
只打开每个所需输入一次，将 bytes 写入所有消费组的独立缓冲流。文件大小从已打开的句柄
读取，实际读取长度不一致即失败。内存仅保留路径清单、有界复制缓冲和每组 64 KiB 输出
缓冲，不载入全部仓库内容。临时流显式 flush、关闭后继续调用既有 SHA-256 adapter；它是
本次命令的私有计算输入，不需要证据发布所需的持久化同步，正式原子证据写入没有改动。

批次只有全部流与哈希成功后才返回结果。exclusive create 成功才取得清理权，成功、错误和
unwind 关闭句柄后清理；冲突路径原 bytes 保留。规划完成后的 reuse/record 继续独立枚举与
重新读取当前输入，不延长缓存有效期。`RESOURCE_FINGERPRINT_BATCH` 记录实际输入文件数、
读取字节数与批次耗时，`RESOURCE_COMPATIBILITY` 单独记录各组收据检查耗时，避免把共同
读取时间重复归给每组。

原摘要格式、依赖范围与既有门槛保持原合同；这属于 verification tooling 的 I/O 实现迭代，
没有新增产品对象、依赖或发布状态。计划 11 与 P14-A50/A51 的投影补齐批量读取、临时文件
所有权和计时语义。保留旧串行实现为 test-only 协议对照与显式性能探针，默认测试不运行
本机性能对照；无 GUI 或资格化收据副作用。

复现命令：

```powershell
cargo test -p stickymd-smoke --bin stickymd-smoke --locked --quiet resource_planning_profile -- --ignored --nocapture --test-threads=1
```

同一工作树、同一文件清单的三轮局部对照（第二轮交换执行顺序；不包含 Git 枚举、候选
校验或 GUI 采样）：

| 轮次 | 旧实现 / 秒 | 批量实现 / 秒 | 逐组摘要对比 |
| --- | ---: | ---: | --- |
| 1 | 4.645909 | 1.720804 | 五组全部相同 |
| 2 | 3.045031 | 1.830764 | 五组全部相同 |
| 3 | 3.026127 | 1.802051 | 五组全部相同 |

本机该局部对照的中位数为 3.045031 → 1.802051 秒，约减少 41%。每轮输入打开从
1,808 次降到 364 次，源码读取从 18,920,818 降到 3,811,258 bytes，均约减少 80%。这组三轮
使用加入实现和回归后的工作树，因此文件数与首次探针不同；新旧两种实现每轮的比较输入
完全相同。数据不构成整体验收提速、跨机器承诺或原生 CPU/内存验收证据，P14-A46 保持
NOT TESTED。

定向回归 13 项通过；另 1 项显式计时探针默认忽略，已用上述命令单独运行通过。回归覆盖
全部模块与 workspace 身份的旧协议一致性、空文件/二进制/中文路径/跨缓冲区输入、实际
共享读取数量、同长度修改与新增/删除文件、真实打开/flush 失败、冲突文件和 unwind 清理。

最终验证：`cargo test -p stickymd-smoke --bin stickymd-smoke --test cli_exit --locked --quiet`
通过 230 项单元与 10 项 CLI 回归；上面的计时探针作为 1 项 ignored test 已单独运行通过。
严格 Clippy（`--all-targets --locked -- -D warnings`）、fmt、`git diff --check`、Phase 00
JSON 治理检查均通过。上轮已经通过的两个 PowerShell wrapper 回归本轮未重跑，包装器仅
增加说明注释。没有运行原生 GUI/资源资格化、Linux 构建或新的并发压力验收；没有创建
Source Freeze、candidate 或正式成功收据，未 commit、push 或发布。
