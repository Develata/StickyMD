# 11_testing_and_release.md - 测试与发布合同

## Metadata

- `Layer`: Verification
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-09-08
- `Scope`: v1 测试类别、逐阶段 smoke、验收证据与发布形态合同

---

## Purpose

定义 StickyMD v1 的验证体系、逐阶段可重复 smoke、验收证据状态与发布形态。

## Boundary

- 验收案例的具体内容在 `docs/acceptance-cases/`；本章定义类别与规则。
- 性能目标的性质定义在 `10_performance_reliability.md`。

---

## Owned Objects

测试 fixture、golden baseline、Source Freeze、Qualification Module Registry、Last Successful Module
Receipt、Target Environment Receipt、Manual Observation Log、Promoted Candidate 与 release artifact manifest；这些是验证证据，不是产品运行时 authority。

## Inputs

冻结 plan/feature/acceptance contract、候选 commit、锁定依赖、测试环境与 fixture。

## Outputs

PASS/FAIL/NOT TESTED 命令结果、每模块 last-success 收据、人工观察日志与逐项逐角色汇总、差异 artifact、checksums、
SBOM、Source Freeze 与唯一 Promoted Candidate。

## State Changes

验证只推进阶段/发布 gate 状态，不得通过修改产品数据或放宽 contract 来制造 PASS。

---

## 测试类别合同

### Unit（单元）

- 文本编辑：UTF-8 byte range、CJK、emoji、combining mark、selection 替换、
  undo grouping、256/4 MiB 限制、IME commit 一次撤销。
- Markdown 转换：所有 CommonMark block、GFM 表格、task list、strikethrough、
  autolink、四种公式 delimiter、转义 dollar、code 中公式标记、raw HTML literal、
  reference link/image、本地/远程图片、malformed input 不 panic。
- 数学 fixture：分数、根式、上下标、积分、求和、极限、矩阵、cases、align、
  可伸缩括号、Greek、`\mathbb`、`\mathbf`、`\operatorname`、Unicode 数学字符、
  错误公式、超长公式。
- 文件：UTF-8 BOM、CRLF/LF、混合换行、原子替换、temp 恢复、config 损坏、
  无效 UTF-8、外部删除、自身写入 watcher 忽略、脏冲突。
- 图片：编码保留、bitmap 转 PNG、hash 去重、多图粘贴、managed/user 区分、
  move to trash、undo restore、redo re-trash、启动恢复、启动清理、路径穿越、
  remote 不下载、超限占位符。

<a id="property-tests"></a>
### Property（property-based）

- 任意 Unicode TextDelta 不破坏 UTF-8。
- undo 后恢复原文；redo 后恢复编辑后文本。
- 任意图片事务最终与引用状态一致。
- 任意窗口几何变化后窗口仍在至少一个工作区内。
- 任意配置缺字段时使用默认值。
- Markdown AST 转换不 panic。

### Fuzz

```text
fuzz_markdown_to_owned_ast
fuzz_render_tree_builder
fuzz_managed_asset_scanner
fuzz_local_path_normalizer
fuzz_text_delta
```

定时运行，不阻塞普通快速 CI。

### Golden（golden tests）

数学与预览使用固定测试字体，覆盖 Light/Dark × 100/150/200% DPI；
允许极小 anti-aliasing tolerance，不允许大范围 mismatch。

### 手工 Windows 11 验收

- 系统：当前与前一个受支持 Windows 11 版本（目标环境角色 `current`、`previous`，见
  [人工观察记录](#manual-observation-log)）；100/125/150/200% DPI；
  单显示器、双显示器（同 DPI / 混合 DPI / 左侧 / 上方）、运行中断开外接、
  sleep/resume、RDP reconnect。
- 输入法：微软拼音、微信输入法（验证项见 `07_editor_and_ime.md`）。真实 profile 的可客观功能事实进入
  exact-candidate 自动化；候选窗位置、遮挡、字体与动画等视觉质量保留人工。
- 窗口与文件矩阵见 `09_windows_shell.md`、`05_document_persistence.md`。

### 文件故障注入

写失败、替换失败、kill 进程后 temp 恢复、config 损坏、外部删除、无效 UTF-8、
双实例、无写权限。

### 内存测量

按 `10_performance_reliability.md` 的测量口径执行。

---

## CI 合同（方向性，后续阶段落地）

- Windows CI：fmt/Clippy、headless tests、headless Release performance 与 release build 可以拆成
  独立 GitHub-hosted jobs 并发执行；各 job 使用隔离 runner，不共享 GUI、进程或测量环境。
  Rust CLI 必须证明完整分片/模块任务并集与完整 `all --ci` 去重任务图一致；日常选测规则见
  [模块化 headless CI](#modular-headless-ci)。
- Portable-core job：在 Linux runner 上只构建平台无关 crates（防止平台无关代码
  被 Win32 污染）；目的不是发布 Linux app。
- Scheduled：advisories、依赖更新 dry-run（不自动合并）、fuzz smoke、
  sanitizer/Miri 平台无关核心、许可证报告。
- 手动完整 CI、定时 CI 和发布准备必须执行完整 headless 集合，不沿用日常选测省略的任务。
- 失败日志与 math/preview diff 作为 artifact 上传。

<a id="phase-verification-harness"></a>
## 逐阶段验证入口合同

每个 Phase 从创建时起必须同时拥有：

```text
tools/smoke/phase-XX.ps1
docs/acceptance-cases/phase-XX.md
```

缺少任一文件，该 Phase 不得标记 `Completed`。历史 Phase 也必须回填，不因已有一次性
报告或终端输出而豁免。

### 自动化入口

- `tools/smoke/phase-XX.ps1` 只能是薄入口；断言、任务规划、去重、进程退出码传播与
  收据输出由 std-only Rust CLI `stickymd-smoke` 持有。
- PowerShell 脚本不得复制测试判断或产品业务逻辑。
- Rust CLI 是 automated smoke、performance gate、runtime process measurement 与 readiness
  聚合的主要 authority；硬阈值必须集中在 Rust 侧或由 plan 单向投影，禁止 Rust/PowerShell
  各持一份不同数字。
- CLI 同时提供 human-readable 与 `--json` machine-readable 输出。JSON 至少包含稳定的
  `schema_version`、commit、artifact SHA-256（适用时）、suite 与逐项 status/evidence；
  `0` 表示所有请求的自动门通过，非零表示失败、阻塞或所请求能力不可验证。
- Rust CLI 属于开发验证面，不是 StickyMD runtime dependency，不进入 portable 发布包。
- `all --ci` 合并 Phase 的无界面任务图并按 task identity 去重；CI 不应为了逐 Phase
  显示而重复运行相同 workspace 测试。
- `all --ci --ci-shard=tests|performance` 只允许 GitHub-hosted CI 使用；两个分片可以在独立
  runner 并发，任务并集必须等于未分片的 `all --ci`，且完整入口继续保留给本地全量复核。
- `all --ci` 还必须执行全部无界面的 Release 性能入口；稳定硬阈值可以作为失败门，
  机器相关测量值只作诊断，不得冒充跨机器承诺。
- 本地 `--performance` 是同一组性能入口的显式复跑方式；`--runtime` 会创建原生窗口，
  只允许显式本地运行，不得偷偷进入 headless CI。
- 本地修复默认运行受影响 Phase/模块的定向 smoke。Resources 可用
  `--resources --resource-module=source-preview|math|images|window|zoom` 单独复核；该结果是
  定向诊断证据，不写正式成功账本。完整 Resources 入口按下述资源子模块契约选择需要
  重跑的模块，最终必须覆盖整个矩阵；不能将任意定向诊断拼成完整成功。
- 同一个交互桌面的 GUI runtime、clipboard、tray、物理鼠标/焦点与资源测量不得并发；多个
  app 会争用共享系统状态并污染性能数据。只有隔离 Windows 会话/机器才允许并发这些通道。
- 自动化等待必须优先等待可观察事实：进程退出、typed reducer state、窗口/文件/clipboard
  acknowledgement、JSON receipt 或 artifact hash。事实已经成立时立即继续；不得为了“稳妥”再追加
  无条件固定 sleep。只能在产品合同本身包含时间边界（例如 650 ms autosave、700 ms auto-hide、
  动画）或平台暂时没有可观察 acknowledgement 时使用有界轮询/等待，并必须允许 early exit、记录
  timeout 与最后观察状态。
- Headless 单元/集成、互不写同一目录的 CLI smoke 与 GitHub-hosted isolated jobs 可以按模块并发；
  任务图必须声明读写资源，不能让多个进程争用同一 Cargo build output、runtime note、evidence path
  或 named object。编译可由 Cargo 自身增量/依赖图共享一次产物，禁止为表面并发重复构建相同 target。
- 同一物理桌面即使能把四个窗口放在四个角，也不能并行执行需要 focus、keyboard、mouse、clipboard、
  tray、foreground、global hotkey、dock 或精确资源数据的验收。严格只读、无输入、无共享托盘/剪贴板、
  独立 portable directory 的截图采样可以作为定向诊断并发，但不能替代正式 GUI receipt；startup、
  performance、memory/CPU/resource gate 始终单独运行，避免相互预热和资源竞争。
- 正式 startup Performance 的 warm-cache cohort 必须遵循
  `10_performance_reliability.md#initial-engineering-targets`：前一进程完全退出后固定等待
  `1000 ms`。`250 ms` rapid-restart 只属于定向诊断，必须以独立名称报告且不得生成或替代
  warm-cache release receipt。
- CLI 自身必须有任务规划、JSON schema/序列化与 exit-code 单元测试。cargo fmt/clippy/test/deny
  保持 CI 原生命令；成熟的 Windows package/GUI helper 可继续由 PowerShell 承担，但不得
  复制 Rust 已拥有的 gate 判断。

<a id="modular-headless-ci"></a>
### 模块化 headless CI

- 日常 main push / PR 可执行受影响工程模块及其反向依赖；模块注册表、路径归属、依赖闭包、
  检查范围和聚合判定由 Rust CLI 唯一持有。GitHub workflow 只提供事件基线、隔离 runner、
  缓存、参数转发和任务结果，不复制路径分类规则。
- 工程模块沿用 Cargo 边界：core、render、windows、smoke、两个 Phase 01 实验；与
  Qualification Module Registry 的 Runtime/Performance/Resources/G3/G4/G5 区分。
  显式本地模块入口只运行请求范围；日常 CI 规划器负责补全反向依赖。
- PR 比较 base commit 与实际被 checkout 的 merge commit；push 比较 before commit 与
  当前 checkout。完整 SHA、Git 对象存在性及比较结果必须验证；无基线、零 SHA、对象缺失、
  比较失败、非 UTF-8 路径或无法识别的路径一律回退完整入口。删除和重命名覆盖旧、新归属。
- Cargo manifest/lock、toolchain、共享构建配置、CI/workflow/规划器、authority/验收合同变化
  选择全部。纯说明/报告文档仍运行 fmt 与治理检查；未知文件不能因扩展名或目录相似被忽略。
  模块/依赖注册与 Cargo 事实不一致时回退原完整 `all --ci`，不能以旧注册表遗漏新 crate。
- 所选模块运行既有 tests 与 headless Release performance；适用的 Windows Clippy、Release
  build/native-runtime gate、Linux portable-core 与依赖政策检查由同一计划给出。
  Release build 适用于 windows 及其上游变化；完整回退执行所有原有检查。
- 完整 `all --ci` 与 tests/performance 分片保留。手动、定时及发布流程始终全量；
  完整与定向 CI 计划均声明 scope、source SHA、基线、原因及调度任务清单；显式模块入口
  可输出所选范围的具体命令清单。计划本身标为 `NOT_RUN`。
- CI 聚合门仅在所有计划内 job 成功，且省略的 job 与计划一致时通过；失败、取消、缺少规划
  输出、意外 skip 或无法识别的状态必须失败。无受影响模块的文档变更可只完成公共治理门。
- 部分 CI 成功只证明选择范围，不写候选功能 last-success ledger，不产生 exact artifact 或
  人工验收证据。Cargo 缓存只保存构建/下载产物，不包含资格化账本、便签数据或发布证据。
  同分支被新提交取代的日常 CI 可取消；取消不得被聚合为成功。
- 验证必须覆盖路径分类、反向依赖、增删/重命名、共享/未知输入、缺失基线、全量并集、
  空矩阵、状态聚合及工作流结构。远程耗时收益须以实际 workflow 数据确认。

<a id="qualification-process-isolation"></a>
### GUI qualification process isolation

- Rust smoke CLI 启动的每个 StickyMD GUI child 必须在 `spawn` 成功后立即移交给统一的
  RAII owner。正常路径可以显式等待或终止；任何 `?` 提前返回、错误传播或 Rust unwind 都必须
  由 owner 的 `Drop` 执行 best-effort `kill + wait`，不能依赖函数末尾的手工清理语句。
- Startup Performance 与所有 Resources 模块在创建 fixture 或启动新 child 前，必须枚举当前
  Windows session 中既有的 `StickyMD.exe`。若其可执行文件位于 smoke tooling 自己命名的系统
  temp qualification root（含人工记录器的会话根目录；按报告路径或其最终路径（解析 8.3 短名与 junction）中任一级
  目录名识别，不要求位于本进程的 TEMP 下，因为另一终端可能使用不同的 TMP），则判为 stale smoke-owned process，当前测量 fail closed，且不得生成
  Performance/Resources PASS receipt。无法读取或解析某个 `StickyMD.exe` 的映像路径、或进程枚举提前中断时，隔离
  无法确认，同样 fail closed；列出后已退出的进程不计入。
- stale-process preflight 只观测并报告 PID/count，不把完整 executable path 写入机器可读 evidence。
  它不得自动终止 preflight 前已经存在的进程，也不得把用户自己的 portable StickyMD 当作可清理
  对象；用户进程的关闭权始终属于 USER。
- preflight 不能替代 RAII：前者防止旧运行污染新 evidence，后者保证当前运行的所有普通错误路径
  收敛 child 生命周期。两者都只属于 verification tooling，不进入产品 runtime 或 portable ZIP。
- 自动化必须包含：模拟 post-spawn 中途失败后 child 已退出的真实进程回归；smoke-owned temp path
  被识别、普通用户路径不被识别的纯分类测试；检测到 stale process 时 preflight 返回非零且不杀进程。

### 验收矩阵

每个 `phase-XX.md` 必须逐项列出：稳定 ID、映射的 plan/AC、验证模式、仓库内入口、
当前状态与剩余证据。矩阵是 `docs/plan` 与全局 AC 的验证投影，不得发明或放宽需求。

状态词固定为：

```text
AUTOMATED PASS   当前提交上的仓库内可重复入口已经通过
MANUAL PASS      当前候选上的正式人工矩阵已执行：每个目标环境角色的全部必需场景均有有效 PASS 观察，
                 并引用完整环境/步骤/结果收据
NOT TESTED       人工项尚未完成，或只有一次性/不可重复记录
BLOCKED          自动化或人工验证已知失败，或环境阻止执行
```

禁止使用模糊的 `PASS`、`CONDITIONAL` 或模块存在来代替验收证据。一次性终端命令、
未提交脚本、主观观察、旧 commit 收据均不能把人工项从 `NOT TESTED` 提升为 PASS。

### Phase 12 发布资格状态

Source-controlled Phase matrix 继续只使用上述四个状态。Phase 12 的 ignored exact-artifact
release receipts 另有两种 USER authority 投影，二者不得混用：

```text
USER-APPROVED GATE   USER 批准工程 hard boundary 校准；不是人工验收豁免
USER WAIVED          USER 明确豁免列出的人工 case/group；未列出的 NOT TESTED 仍阻塞
```

Phase 12 source decision template 与 Source Freeze-bound `dist/evidence/release-decisions.json` projection
只允许 `PENDING`、`USER APPROVED`、`USER REJECTED`、`NOT APPLICABLE`。Rust automation 只能
记录 USER 已明确给出的决定，不能自行批准；PUSH 决定必须在 remote workflow 前绑定 source identity。
人工 waiver 只接受具体 `WAIVER-P12-Mxx` key、Tier B 组 key `WAIVER-TIER-B-v{version}` 与目标环境档案 key
`WAIVER-PROFILE-PREVIOUS-v{version}`，不接受全局 blanket waiver；档案 key 是限定于 `previous` 角色的跨 tier 例外，
逐项 key 与 Tier B 组 key 不跨 tier。其 source/version identity 来自相同
Source Freeze；每份人工 waiver 还绑定当前人工观察上下文，并明确列出所处置的 `(项, 角色)` 集合，所有种类使用同一
校验器，旧格式不迁移。waiver 只处置 `NOT TESTED` 的 `(项, 角色)`，不覆盖 `MANUAL_FAIL` 或无效证据，也不删除已有
PASS 观察；最终 manual PASS 仍额外绑定 Promoted Candidate。

<a id="manual-observation-log"></a>
### 人工观察记录与目标环境档案

人工观察是独立的证据类别，不是 Last Successful Module Receipt：它同时记录 `MANUAL_PASS` 与 `MANUAL_FAIL`，
绑定当前 Promoted Candidate，不跨候选复用。

- **目标环境收据**：Promote 之后由 USER 以交互命令声明，绑定 candidate receipt 摘要并含随机上下文 ID。`current`、
  `previous` 两个角色各写明 Windows 11 功能版本、对应构建分支、USER 依据微软发布历史声明的获准完整构建号
  （`CurrentBuild.UBR`）及其来源与选择日期、允许的 `EditionID`（具体 SKU）与原生架构。选择规则：选择日期时，面向
  通用设备的 General Availability Channel 中仍在服务期的最新与次新功能版本，排除只面向特定新设备的分支。工具
  不联网，不自行推断目标。收据缺失、损坏或含糊时 authority 无效，任何 waiver 都不能替代。两个角色都覆盖完整的
  人工项注册表，仍按 tier 与 waiver 规则处置。
- **角色匹配**：记录器在被测机器上读取 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`（64 位视图）的
  `DisplayVersion`、`CurrentBuild`、`EditionID`（`REG_SZ`）与 `UBR`（`REG_DWORD`），以及 `IsWow64Process2` 成功返回
  的 `pNativeMachine`（x64 进程运行于 ARM64 主机不算原生 x64）；采集有超时与输出上限，在每个场景开始与提交时各
  采集一次，两次不同或任一失败即拒绝该观察。功能版本、构建分支、完整构建号、SKU 与架构全部相符才匹配一个角色，
  且必须唯一，匹配为零或不唯一即拒绝登记。记录、导入与 readiness 共用同一谓词，按观察内记录的被测环境事实重算
  角色，观察声明的角色必须与重算结果一致。人工观察的 Windows 身份只由本谓词决定，不读取 `ver`。匹配只表示系统是 USER 批准的公开服务构建之一：不判定 Insider 登记
  状态，也不证明完整环境等价。获准构建列表是可接受的替代集合，不要求每个构建都完成一遍矩阵；每条观察显示实际
  构建号；公开的可选预览质量更新可由 USER 按具体构建列入。资格化期间机器更新后构建号不在列表内，需要新的目标
  收据与上下文，这会使两个角色的进度与全部人工 waiver 一并重新开始。
- **人工项注册表**：工具内声明每项的 ID、tier、必需场景、对被测实例的要求与 waiver 范围，与 `phase-12.md` 的
  Manual 集合精确相等；记录器、校验与 decision key 校验都读取它。注册表变化即新上下文。
- **观察日志**：`dist/evidence/` 中只追加的日志，头部绑定 candidate receipt 摘要、目标收据摘要与注册表摘要。每条
  观察有稳定 ID（会话 ID 与会话内序号）和 `base`（会话开始或观察任务导出时的日志长度），状态只能是 `MANUAL_PASS`
  或 `MANUAL_FAIL`；`NOT_TESTED`、跳过与中止从不写入。某场景的 FAIL 一经观察即落盘（先于可选备注）；备注是追加在其观察之后的
  独立记录，每条观察至多一条，同观察同文本的重复为 no-op、不同文本拒绝。同 ID 同内容
  的重复导入为 no-op，同 ID 不同内容拒绝。日志与观察包在读取前先检查字节上限。
- **状态归约**（每个 `(项, 角色)`）：场景为 FAIL 当且仅当存在某条 FAIL 观察，且没有任何 `base` 已包含它的 PASS；
  否则有 PASS 即为 PASS，都没有则未覆盖。该判定与登记顺序无关：迟到的旧 PASS 不能清除较新的 FAIL，迟到的旧 FAIL
  在正式重测前保持阻断。证据无效 → 阻断；任一必需场景为 FAIL → `MANUAL_FAIL`；全部必需场景为 PASS →
  `MANUAL PASS`；其余 → `NOT TESTED`，并列出已覆盖与缺失场景。
- **并发与恢复**：不被替换的 sidecar 锁 `dist/evidence/manual.lock`；记录、导入与目标声明取排他锁，Promote 替换与 Source
  Freeze 作废 candidate 时也取排他锁（提交时的 candidate 复核与写入因此不被插入；锁内复核只读 candidate 收据，不
  等待仓库子进程），readiness 与观察任务导出取共享锁，等待有上限。导出在同一把锁内从同一份日志快照取得上下文、`base` 与未解除的 FAIL。写失败
  非零退出；观察包整体校验后一次原子提交。声明目标收据的同一事务写入合法的零观察日志（只有头部）；此后日志缺失、
  零字节或损坏都视为日志丢失并阻断，不得当作首次初始化。日志达到条数上限后拒绝新观察。日志损坏即阻断；重建日志需要 USER 决定
  `RESET-MANUAL-LOG-<digest>`（`<digest>` 为现有日志文件字节的 SHA-256，日志缺失时为空字节串的 SHA-256）并产生新
  上下文，旧观察任务、观察包与 waiver 都不继承；重置决定在汇总中可见。日志仍可读（如达到条数上限）时，其未解除的
  FAIL 按 `(项, 角色, 场景)` 合并为每个三元组一条带来源引用的 FAIL，作为新上下文的首批观察并保持阻断；三元组数远小于
  条数上限，重置后必有追加容量。旧格式 `manual-acceptance.json`（schema 1）不转换为观察：原文件保留，
  readiness 报告其已被取代。
- **被测实例**：记录器校验候选 ZIP、解压到会话目录、按场景放入 fixture 并自行启动 EXE，观察记录 EXE 摘要与进程
  ID；同目录第二次启动、双副本、跨断屏/睡眠/RDP 保留原进程与原 note（不得重启或重新放入 fixture 来完成恢复）等
  要求由注册表按场景声明；预期的断连期间环境探测失败不判产品 FAIL。fixture 随工具提供：输入法项用空白 note，
  其余用渲染压力 note 与全部图片格式（含超限图片）。PASS 要求记录器启动的进程在提交时仍在运行且是唯一的
  StickyMD（在存储锁内、写入之前确认）。候选已启动但未在限时内完成初始化时，记录器报告原因，观察者可记录该实例的
  FAIL 或不记录，PASS 被拒绝；启动之前的准备失败（复制、fixture、摘要不符）不形成观察。实例以挂起状态创建、加入 Job 后才恢复运行，随记录器退出（含中断）
  而终止，StickyMD 自己启动的进程（如打开链接的浏览器）不受影响；创建与加入 Job 之间被强制终止时会留下一个从未运行的
  挂起实例，下一次会话拒绝启动并指明它来自中断的记录器。恢复挂起线程失败（含挂起计数不为 1）时该实例被停止、场景
  不记录；这些原生失败路径只验证了判定逻辑，未做原生故障注入。辅助进程（解压、进程枚举、UIA）同样在包含其子孙的 Job 中
  启动，退出或超时后其子孙一并结束。会话目录持有会话锁，创建、删除与清理都在会话根目录的锁下串行；正常退出时删除，异常退出后由下一次会话清理
  失去锁且名称符合会话格式的目录（每次清理的条目数、删除数与耗时有上限，其余留待以后），无法判定的保留并报告，根目录
  为链接时拒绝。开始前确认没有其他 StickyMD 实例；
  记录器不终止它未启动的进程。
- **其他机器**：主工作区导出观察任务（含完整 candidate receipt、目标收据、注册表片段、`base`，以及所选场景尚未
  解除的 FAIL 的 ID、登记序号与说明，记录器在重测前展示）；工具二进制以观察包模式在被测机器上运行，不需要仓库，
  所需 fixture 与 helper 随工具提供；主工作区在锁内严格导入，并复用现有 candidate 校验。
- **M34 Clean VM**：先完成首次运行观察，之后才复制并运行工具；该观察标为人工声明，记录 VM 基线与包的取得方式，
  不冒充事前环境证据。场景开始时的采集、由记录器启动被测实例，以及 Phase 14"人工观察前先查询交互环境"都不适用
  于这一次首次运行；工具事后在同一 VM 中采集环境事实并据此归属角色，事后采集与首次运行的人工声明分开记录，观察者
  声明两者之间未更改系统。
- **信任边界**：信任 USER，不防伪造观察；严格解析、摘要与上下文绑定防止的是混用、陈旧与损坏。
- **上限**：观察包 1 MiB，单个日志 10 000 条，备注 2 KiB。
- **已知覆盖缺口**：Phase 8 人工义务尚未全部映射到 P12 人工项。readiness 无论通过与否都在普通输出与 JSON 中列出
  这些缺口，不得把人工矩阵表述为完整。
- **验证**（实现时进入 Phase 14 矩阵）：角色唯一匹配，以及类型错误、读取失败、两次采集不同、零匹配与多匹配的
  拒绝；迟到的旧 PASS 与旧 FAIL、重复与冲突 ID；部分覆盖、FAIL 与无效证据优先于 waiver 与 Tier C；上下文失效、日志
  缺失、零字节或损坏与重置（含满额日志的未解除 FAIL 合并带入）、写入失败不留半更新、锁等待有界；FAIL 先于备注落盘，guided step 逐项回答；原进程与原 note 保留、
  M34 例外；观察任务快照一致、无仓库记录、整包导入的拒绝路径；readiness 始终列出已知缺口。

<a id="release-artifact-authority"></a>
### Source freeze、remote artifact promotion 与 exact evidence

Phase 12 source freeze 前提交所有 source-controlled 治理、工具和报告。clean HEAD 先生成 ignored
`dist/evidence/release-source-freeze.json`，绑定 source commit、workspace version、`Cargo.lock`
SHA-256、target 与 harness。Source Freeze 是 source-only evidence 和动态 USER action decision 的身份边界，
不是最终发布 artifact。

本机 `target/release/stickymd-win.exe`、本地 ZIP 与本地 SBOM 只构成 Local Preflight Build。它们可以尽早
验证源码、构建、包结构、developer-runtime import 与 release tooling，但不得写入最终
`release-candidate.json`，也不得被 Runtime/Performance/Resources/G3/G4/G5/manual readiness 当作最终
发布字节。相同 source commit 的独立 Windows build 只有在另行证明完整 reproducible-build contract 后
才可要求 bit-identical；v0.1.0 不作该声明。

唯一 Release Exact Artifact 必须来自已批准 Source Freeze 的 GitHub `release` workflow：

```text
Source Freeze
→ local/source preflight
→ explicit USER PUSH authority
→ push + CI
→ release workflow build/package/verify
→ record successful run/attempt/artifact id
→ qualification CLI 按已记录 run/name 下载 ZIP + SHA256SUMS + SBOM
→ 与用户指定的 downloaded copy 逐文件 byte/hash 比对
→ verify checksum/SBOM/package/native runtime/runtime smoke
→ atomically Promote into dist/exact-candidate/
→ artifact-bound qualification
```

Promote transaction 只有在全部输入验证成功后才写 `dist/evidence/release-candidate.json`。candidate receipt
必须记录 source SHA、version、Cargo.lock SHA-256、workflow run/attempt、artifact id/name、ZIP/EXE/SBOM
SHA-256、target 与 unsigned policy；不得记录机器绝对路径。canonical ignored staging 固定为：

```text
dist/exact-candidate/
├─ StickyMD-<version>-windows-x64-portable.zip
├─ SHA256SUMS.txt
├─ SBOM.spdx.json
└─ StickyMD/StickyMD.exe
```

所有 artifact-bound runner 必须通过一个 candidate resolver 获取 staged EXE/ZIP，禁止在 qualification
模块各自硬编码 `target/release/stickymd-win.exe`。Source Freeze 建立前，开发期 targeted smoke
可以显式使用 Local Preflight Build；Source Freeze 建立后即进入 release qualification，candidate receipt
缺失、格式错误或 source/hash/staging 不匹配时都必须 fail closed，不得静默 fallback 到本地 EXE。

ignored `dist/evidence/` 中：

- source-only receipt 绑定 Source Freeze，不要求 final EXE hash；本地 headless CI 收据必须恰好包含 runner 为
  `all --ci` 规划的全部任务（按顺序、每项 `PASSED`）及任务集标记，不另维护任务名单；
- remote workflow receipt 绑定 source/run/attempt/artifact id，不复制尚未产生的 final EXE hash；
- downloaded/promotion receipt 与 candidate receipt 绑定实际 ZIP/EXE/SBOM hash；
- downloaded/package/checksum/SBOM、PE/native-runtime 与 portable-runtime 直接证明当前 artifact bytes，
  必须绑定 Promoted Candidate 的适用 ZIP/EXE/SBOM hash；
- Runtime、Performance、Resources、G3、G4、G5 使用下文定义的 Last Successful Module Receipt；人工观察是独立的
  证据类别（见[人工观察记录](#manual-observation-log)），绑定当前 Promoted Candidate；上述模块记录成功来源 candidate，但由当前 Qualification
  Module Fingerprint 决定能否复用；
- readiness 只接受当前 Promoted Candidate 的 exact-byte evidence、与当前模块指纹兼容的 last-success evidence，
  以及当前人工观察上下文中经归约的人工观察（`MANUAL_FAIL` 与无效证据优先于任何 waiver）。

每次 Promote 或 candidate identity 变化，旧 exact-byte evidence 必须变 stale。Source-bound 与功能资格化
evidence 不按整个 source commit 或“最新 candidate”机械失效，而是由声明的模块输入指纹判断。相关产品、共享、
harness、contract、manifest/lock/toolchain 输入完全相同时可以复用最后成功；任一适用输入变化只要求重跑受影响
模块。无法分类的 tracked path 必须保守命中共享/全局失效集合。不得因为同 source 的独立 build 产生不同 EXE
hash 就自动重跑无关功能模块，也不得用 source equivalence 跳过 exact-byte gate。

Promotion failure path 必须保持 fail closed：下载不完整、checksum/SBOM/source identity 不符、
ZIP 不能解压、PE/native-runtime/runtime smoke 失败时，既有 Promoted Candidate 不得被半更新，
也不得写成 PASS receipt。原子 staging 成功但 receipt 写入失败时，candidate resolver 仍必须因
缺失 receipt 而拒绝使用该目录；不得从本地 preflight 补位。
为避免把同 source 的本地 build 误归因为 remote artifact，Promote 不得只相信调用者给出的
ZIP 路径；qualification CLI 必须按 remote receipt 的 run/name 自行下载权威副本，确认
ZIP/checksum/SBOM 与用户副本逐字节一致后才允许 staging。

- manual recorder 必须是交互式 human receipt recorder，逐场景只接受显式
  `MANUAL_PASS` / `MANUAL_FAIL` / `NOT_TESTED`（后者不写入日志），不得从 process/status 自动推断人工 PASS；
  guided step 覆盖多项时逐项询问；
- stale module fingerprint 不参与 readiness；G3/G4/G5 成功记录还必须包含 origin ZIP/EXE、运行 harness、
  clean worktree 与各组预期逐项结果，只有当前 registry 指纹兼容时才能复用；
- readiness 对 P0/P1、未批准 hard gate、mandatory manual NOT TESTED、exact package、remote
  evidence 与 USER decision fail closed；不得提供 `--force-ready`；
- readiness 读取的每份收据（candidate、Source Freeze、decision、remote/downloaded、headless、manual 与模块
  evidence）都整份严格解析为 JSON 对象，每个字段、case 与 decision 只从所属对象读取；重复键、截断、尾随
  输入或类型不符即为无效，不以文本搜索寻找字段；
- exact evidence 记录的 Windows build 必须含具体版本与构建号（如 `Microsoft Windows 10.0.26200.9457`），
  由 `ver` 输出读取（按控制台代码页有损解码后只取 ASCII 版本号）；读取失败记为 `UNKNOWN` 并使收据无效，不得以
  `OS` 环境变量（`Windows_NT`）代替；
- freeze 后 source/manifest/lock/runtime asset 或 release tooling 变化仍要求重建 Source Freeze。下游证据由
  evidence class 决定：exact-byte evidence 必须重建；module evidence 只在其 registry 输入指纹变化时 stale。

<a id="module-success-ledger"></a>
### Qualification Module Registry 与 last-success ledger

Rust smoke CLI 必须用一个注册表统一声明稳定 qualification module：模块 ID、runner、产品输入、共享输入、
harness、权威 plan/acceptance contract、依赖传播和 evidence class。PowerShell、readiness 与各 runner 不得各自
维护另一份路径判断。注册表属于 verification tooling，不进入产品 runtime。

模块指纹必须对排序后的相对路径与实际文件 bytes 计算稳定 SHA-256；不能使用 mtime、提交时间、目录枚举顺序、
“是否最新 candidate”或只看 `git diff` 的易漂移判断。同一 planning invocation 可缓存共享文件 digest，但不得把
全部仓库内容同时复制到内存。共享 core、Cargo manifests/lock、toolchain、build/release 配置和 contract 变化按
注册依赖保守传播；任何 tracked path 无法分类时，planner 必须使保守共享集合 stale，而不是默认忽略。
产品文件按显式规则归入产品域：每次会话的启动路径或事件循环每轮都会执行的代码（应用状态机及其逐帧 UI、启动、
配置与偏好、intent 路由、编辑会话、note 加载与恢复检查、窗口与平台初始化、帧表面、默认视图的源码渲染与共享
core），以及任何编辑或偏好变化之后的持久化（autosave、自身写入的 reconciliation、note 与配置的原子发布）是每个
启动产品的模块的输入，归全部产品域；只经某个功能到达的代码归该功能所服务的域；尚未追踪闭包的文件保持
全部产品域。缩窄必须给出"入口 → 调用/回调/Effect → 共享状态读写 → 验收断言"的依据，拆文件本身不是依据。
没有显式规则的产品文件在运行时归全部产品域，治理检查（每次 CI plan 都运行）拒绝它；不再命中任何 tracked 文件的规则
同样被拒绝。
决定 evidence 能否被接受的规则（模块 evidence 合同、注册表、账本与记录格式）属于全局输入；只展示或汇总
已有判定的工具代码（状态报告与 release readiness 汇总）不是任何功能模块的输入，由工具自身测试与 CI 覆盖。

功能模块指纹（v2）对两类文件做发布版本规范化，其余输入仍逐字节参与：根 `Cargo.toml` 去掉
`[workspace.package]` 唯一的 `version` 行；`Cargo.lock` 去掉 workspace member（由 `[workspace].members`
与各 member 的 `[package].name` 白名单确定，且 member 必须 `version.workspace = true`）对应、无 `source`、
版本等于 workspace version 的 `version` 行。根 manifest、全部 member manifest 与被跟踪的 `Cargo.lock` 在
规划时作为一组判定：任一文件不在这一窄语法内（重复字段，包括 lock 顶层格式版本及 `source`/`checksum`/
`dependencies` 的重复；dotted/inline key、多行字符串、未知表、member 缺失或重复、带版本的 member 依赖引用、
lock 与 manifest 版本不一致、lock 未被跟踪），两份文件都回退为原始字节；只有与判定时逐字节相同的内容才使用
规范化形式。规范化算法版本写入指纹头；Source Freeze、workspace-tests identity 与 exact-byte 门始终使用原始字节。
发布说明、README/CHANGELOG/CONTRIBUTING/SECURITY、AGENTS 指南、coverage matrix、release checklist、
`docs/{adr,overview,features}/`、README 图片与许可证文本，以及人工验收代码（记录器、人工项注册表、观察存储、归约
与 readiness 人工汇总）和 `tools/manual/` 指南，不是任何功能模块执行或读取的输入，不使功能模块失效（workspace
tests 的身份仍覆盖这些代码；各证据写入方共用的人工存储保留路径与锁不在此列，仍是全部功能模块的输入）；
其中由 governance 检查或打包读取的文件继续由 CI 选择与 exact-byte 门覆盖。`docs/plan/`、`docs/acceptance-cases/`
因跨域约束仍对全部功能模块保守传播。

成功记录存放在克隆级共享存储 `<git common dir>/stickymd/qualification-ledger/`，同一克隆的所有 linked
worktree（包括每次发版新建的隔离 worktree）解析到同一位置；git 不可用时 fail closed，不退回私有存储。
每个模块按输入指纹保存成功记录（`modules/<module>/<fingerprint>.json`），不同指纹互不覆盖，查找是
按当前指纹的直接路径；同指纹重跑替换自身记录，每个模块保留最近 8 份。归档、发布与清理都在存储的
OS 文件锁（`.lock`，进程退出即释放；等待上限 120 s，超时报错而不是无限阻塞）下串行；读取方持共享锁
从记录跟随到 evidence 与 companion 文件，清理因此不会删除正在被读取或刚被其他写入方发布的内容。查找在同一把
共享锁内依次校验：记录（固定字段集的严格 JSON，evidence 名称由模块与摘要决定）、evidence 字节摘要、该模块的
evidence 合同与每个 companion 归档的摘要。evidence 合同对整份文档严格解析，每个值只从所属对象读取，重复键、
截断或尾随输入即无效；登记使用同一合同，不完整、失败或身份不符的收据在登记时即被拒绝。因此"可复用"与
"readiness 合格"是同一判定：记录存在但任一校验失败时为 `INVALID_LAST_SUCCESS`，拒绝复用并显示原因，正式重跑
完整通过后替换该记录并补回缺失或损坏的 companion 归档。写入方只读取一次工作区收据，对同一份字节完成校验、摘要与
归档；截图同样对实际归档的字节计算摘要。刚发布的记录无论时间戳是否并列或回拨都被保留。清理遇到任何不可读
或无法解析的记录、目录项或 evidence 即停止删除并报告，不把"无法确定是否被引用"当成未引用；截图引用从所有
保留 evidence 的 `results[].artifacts[]` 结构化提取，不受当前工具 case 列表限制。成功记录发布后清理失败只
告警，不撤销成功。清理所用记录必须是完整、严格可解析的 JSON，用于决定截图引用的 evidence 必须与记录中的
摘要一致；G5 每个 case 只统计自身 JSON 对象内的截图。整个共享存储（含 `.lock`）与旧路径一样是
保留路径：别名解析后的路径含 `/stickymd/qualification-ledger/` 段（忽略大小写），或落在由 `.git` 元数据文件
（不运行 git）定位、并解析过 junction 的存储根之内，均拒绝；`.git` 存在却无法解释（指针为空、目标不是现有
目录、`commondir` 无效，或 `.git`/`commondir` 本身是悬空链接）时同样拒绝。存储在取得锁时检查根以下所有
条目（含 `.lock`，且在打开前检查），出现 junction 或符号链接即拒绝读写，因此静态的子目录重定向无法让外部
路径冒充受信任记录。该保证覆盖遵守同一锁协议的 qualification 进程与静态链接；不遵守锁协议、在持锁期间
替换存储目录的外部进程不在防御范围内（路径级 API 无法消除这一竞态）。每次取锁的扫描在约 900 个条目时
实测约 5 ms。普通 smoke 与诊断的 `--evidence-file` 不得指向其中：

```text
no record for current fingerprint
    -> RUN REQUIRED

record for current fingerprint, fully valid
    -> REUSED PASS（不启动 runner；显示 origin candidate/source/version）

record for current fingerprint, but record/evidence/companion invalid
    -> RUN REQUIRED (INVALID_LAST_SUCCESS, reason shown)

RUN REQUIRED -> complete PASS
    -> atomically publish the record for (module, current fingerprint)

RUN REQUIRED -> FAIL / ENVIRONMENT BLOCKED / USER ABORTED / unwind
    -> return non-zero; do not create or replace any record
```

记录只允许 `PASSED`；失败、中止和 partial result 不是正式成功收据。它们可以输出到当前命令终端或临时诊断，
但不得改变 ledger authority。成功 evidence 先写入按内容 hash 命名的不可变归档（`evidence/`），G5 截图等
companion 文件按 SHA-256 归档到 `artifacts/` 并在归档前校验，再用同目录 temp + atomic replace 发布记录；
readiness 从共享存储校验这些归档，不依赖原 worktree 中的文件。旧记录保留不代表当前有效：readiness 每次
重新计算当前指纹；readiness 与状态报告共用一次查找快照（全部有记录模块的一次指纹批量计算、一把共享锁），
不重复校验。工作区内旧的 `dist/evidence/module-success/` 不再是 authority，也不自动导入（指纹算法
已变化），但仍是保留路径，普通 smoke 与诊断不得写入。发布归档必须复制 readiness 引用的共享存储 evidence。

package/download/checksum/SBOM/PE/native-runtime 等精确字节门不进入功能 ledger，仍直接绑定当前
ZIP/EXE/SBOM 并在 artifact hash 改变后重跑。功能行为模块可以跨 candidate 复用，但必须保留 origin identity，且
产品、共享、harness 与 contract 输入（按上述规范化）逐字节兼容；readiness 将复用 evidence 内的 source、
EXE/ZIP 与 version 与该记录的 origin 比较，不与当前 candidate 比较。
readiness 输出必须区分 `RAN PASS` 与 `REUSED PASS`；二者都是通过，但后者不能伪装成在当前 candidate 上实际启动。

candidate freeze、release readiness 或 USER 明确要求时，planner 只运行指纹 stale 的功能模块与当前 artifact
需要的 exact-byte gate，不再机械重跑完整 Phase 0–14 Campaign。需要重新观察兼容模块时使用现有定向诊断命令；
诊断输出不改变 last-success ledger。

<a id="shared-headless-prerequisite"></a>
### Runtime / Performance 共享无界面前置检查

- 完整 Phase 14 Runtime 与 Performance 的 canonical 请求可以共享同机、同输入的完整
  `cargo test --workspace --locked` 成功。它只替代任务图中的 workspace tests，不替代
  headless CI 全集、原生桌面、startup、资源或 exact-byte 证据。
- 共享记录绑定有效 Source Freeze、clean worktree、全部仓库输入 bytes、实际 harness、
  Rust/Cargo 工具链、命令参数、主机与工作目录，以及适用的环境/Cargo 配置身份。
  无法完整识别的配置或测试筛选条件禁止复用；普通诊断、选模块和 CI 不写该记录。
  相对 Cargo 配置路径必须按实际 Cargo 子进程的工作目录解析，不能使用调用者所在子目录。
  空 `CARGO_HOME` 使用用户目录回退；Windows 带盘符的相对路径等不能可靠解析的配置禁止共享。
- 执行前后重新获取身份；发生变化、测试失败、中止或记录写入失败均返回非零，不得
  把 partial 结果登记为成功。记录以原子替换保留最近一次完整成功，损坏或不匹配时重跑。
  只在完整测试本身通过后登记，后续桌面失败不撤销已完成的 source-bound 前置检查。
- 复用结果明确标为 `REUSED_PASS` 并保留来源与输入身份；正式通道的 task coverage
  继续要求这项任务。输出规划、身份检查、任务执行耗时；历史运行时间不能冒充本轮耗时。
- 资源规划同轮只枚举一次输入清单，每组指纹计算一次并传给兼容性判断。复用/登记阶段
  的当前身份核对不使用规划缓存；指纹字节格式和依赖范围不因此改变。
  同轮多个组可流式共读相同输入，保持各组排序后的原始 bytes 协议，不缓存整个仓库内容。
  临时指纹流须完整 flush 后才能哈希；任何输入读取或输出失败使整批失败，清理仅限本次
  exclusive create 持有的临时文件。批次总耗时与逐组兼容性检查用时分别报告。

<a id="resource-module-qualification"></a>
### 资源场景去重与子模块资格化

Resources 是 source-preview、math、images、window、zoom 五类兼容成功的聚合投影，
不再拥有可独立替代五类证据的整组 last-success。Rust 资源注册表统一持有场景定义、
分组、操作历史、采样协议、覆盖要求和固定等待预算，runner/readiness 不得另建跳过清单。

- 同一次命令的相同样本、初始配置、操作历史及采样协议可以共享一份完整测量；别名必须
  指向实际测量来源，不能伪装为额外独立样本。每个独立场景仍保留原有次数和时间窗口。
  Preview→Source 释放、窗口压力循环等有状态过程不得只按最终画面合并。
- 只有 `phase 14 --resources` 的无筛选完整请求，显式写入 canonical
  `dist/evidence/resources-qualification.json`，才进入子模块资格化。要求 clean worktree、
  有效 Promoted Candidate、独占桌面与适用环境检查。选组/选 case 请求写入此路径必须
  在执行前拒绝；诊断不会因为输出文件名而登记成功。
  Runtime/Performance 的 canonical 输出同样要求对应的完整 Phase 14 单一模式请求；
  登记与复用均须按 Rust task plan 核对全部任务，单个成功标志不代表完整覆盖。
  入口保护按文件系统身份识别正式/内部收据，包括 Windows verbatim、短路径与目录别名；
  未创建文件按最近存在的父目录解析，在执行前及输出写入前均执行保护。
- 五类账本分别绑定其产品/共享/harness/样本/合同输入及 origin identity。每类完成全部
  必需场景后才能原子更新；缺项、重复项、失败、INCOMPLETE 或被中断的 cohort 不合格。
  一个模块失败不覆盖其他完整成功；中断的窗口压力循环整段重跑。
  每组成功归档须自包含五次原始观测与适用的既有硬门；共享 cohort 保留这些数据并通过
  `shared_from` 标明原始场景，不算作新的独立采样。完整性检查核对实际 run、单位、
  统计与原始观测的一致性及硬门结果，不能只相信计数标记或 PASSED 字样。
- 记录前重新核对 source/candidate、clean tree 与本次测量开始时的模块指纹。输入变化
  拒绝登记。下一次完整请求只复用当前指纹兼容的子模块，输出来源与重跑原因。
- readiness 必须核对五类成功及各自完整覆盖；aggregate 文件仅作本次执行进度/来源摘要。
  旧 `resources.json` 账本、旧整组收据与定向诊断均不自动拆分或导入，原文件保留。
- 窗口控制、进程生命周期等共享适配器的变化必须使所有实际消费者失效。未知 tracked
  输入保守失效；成功文件路径本身不能决定请求具备哪一种资格。
- 预检样本结构后才开始等待；报告计划固定等待预算、各场景/模块实际用时及共享来源。
  预算与模拟执行只能证明计划开销，不构成真实桌面速度、资源或产品质量证据。
- 每个实际执行的资源组在长采样前，用独立临时程序副本验证真实窗口激活、点击路由和
  Source/Preview/Split 切换的持久化确认。探针不产生资源样本，测量使用新进程与原 fixtures。
  遮挡时拒绝输入，并在现场记录可查询的 HWND/PID、可执行文件名和窗口类；不得记录其他
  应用的标题、完整路径或关闭其窗口。探针不能保证后续桌面始终不受干扰。
- 资源执行器向 runner 上报当前组、场景、轮次和预热/转换/CPU/压力阶段。指定 JSON
  evidence 输出时，在同目录的 `.progress.json` sidecar 原子保存进度；它始终是诊断进度，
  不能作资格化收据。剩余时间只表示未执行的固定等待预算，不承诺完成时刻。
  每个完整基础/数学/图片场景以及完整缩放 cohort 结束后，将当前组已收集数据写入显式
  INCOMPLETE 检查点；窗口压力过程不因此拆成可复用片段。写盘在采样窗口外，失败须返回
  非零且保留原始错误；最终结果仍由既有组/命令证据持有。
- `--resource-resume` 是显式的 Phase 14 Resources 诊断选项，必须指定 `target/` 下 ignored
  的独立 JSON evidence 路径，防止检查点改变输入指纹；正式 canonical 输出及其文件系统
  别名拒绝此选项。缓存只位于 ignored
  `target/resource-diagnostics/v2/`，公共 evidence writer 不得覆盖该内部目录。
  缓存完整通过五次采样的基础/数学/图片场景，或完整 Window/Zoom 组。整组缓存必须通过
  原始观测、统计、硬门、fixture 和压力完成校验，并成功清理本轮临时程序；不从中段恢复。
  缓存绑定全部 tracked/untracked 非 ignored 输入、source、实际 EXE/harness bytes、
  主机/启动/登录 session、单显示器尺寸/DPI/work area、有效桌面状态及执行设置身份。
  无法充分识别的环境（含未支持的多显示器配置）禁用缓存并完整运行；缓存不保证瞬时
  系统负载相同。身份在采样及缓存读取前后、原子记录前重检，运行中发生变化则中止该次诊断。
  记录最长有效期 24 小时，校验内容摘要、每次原始观测、统计、协议和既有硬门；缺失、
  损坏、过期或不匹配时从头重测该完整场景，失败轮次不得与其他运行拼接。
  复用必须标为 DIAGNOSTIC_REUSED，记录历史来源及原执行耗时；本轮耗时另记。
  旧诊断 JSON 不自动导入，缓存和复用输出不参与任何正式成功登记或 readiness。
  持久缓存可在同一完整身份内查找注册且严格等价的基础/数学/图片场景：内容、模式、
  fixture、操作历史、预热和 CPU 协议必须一致。每个候选独立验证；返回结果转换为请求
  场景的名称，但明确保留原场景、原始观测、创建时间与历史耗时。历史结果不得重新保存
  或延长有效期，Window/Zoom 不参与跨单元等价复用。计划与实际执行使用相同查找规则。
  诊断执行前输出每个单元的预期复用/重跑、失效原因和预计剩余固定等待预算。同一命令
  等价场景按首次出现单元计算，避免重复扣减。计划只是假设当前身份不变的预测，实际复用
  必须再次检查；进度预算只在实际命中后扣减。`--resource-plan` 配合 `--resource-resume`
  只读取计划并向 stdout 输出 NOT_RUN JSON；不构建、不启动产品、不写指定 evidence 或账本。
  缺少程序或无法确定身份时明确显示禁用原因。缓存最新索引仅用于解释 source、程序、
  harness、输入或环境变更，不能授权读取其他身份记录。不得打印私有主机/用户/环境值。
  身份采集按 Git、实际程序/harness hash、环境、输入 hash 和总计分别报告耗时，仅在
  采样窗口外执行。可合并相邻重复采集，但缓存读取前后及采样后的新鲜检查不可省略；
  原子发布前仍须重检。同命令共享场景绕过持久缓存读取时，必须独立检查当前身份。
  新建诊断 Store 后立即生成计划时，初始采集可作为计划读取前检查，读取后仍重新采集。
  同一资源组剩余场景全部命中时，可在该组桌面探针后作为一个批次读取：批次前后重新
  采集全部身份，逐记录验证完整性，全部通过后才消费和扣减预算。部分命中不得暂存命中
  子集供后续测量后使用，须回到逐场景执行；计划、旧批次或失败提示均不能替代鲜活检查。
  `--resource-failure-first` 必须配合诊断续跑，仅将本次已选范围内的上次失败单元及其资源组
  提前；所有构建/候选/环境前置任务保持原位。被选中的失败单元绕过历史缓存重新完整测量，
  不缩短五次采样或压力段。Window/Zoom 只作为整组排序，不能用组内过滤拆分压力协议。
  每次诊断续跑可在内部缓存目录原子记录失败单元提示；提示仅含注册单元、时间和版本，
  校验和或 24 小时有效期不符、缺失、超出本次范围时明确回退固定顺序，不扩大选择范围。
  提示可跨 source/harness 改动用于排序，但不授权任何成功复用；成功数据仍须完整身份验证。
  只有重新测量并验证完整的成功才能清除对应旧提示，历史命中不清除；清除前重读比较版本，
  已观察到的新提示不覆盖。此提示不提供跨进程调度锁。
  提示写入在采样窗口外，失败保留原错误和已收集样本；只读计划不修改提示。
- Runtime/Performance/Resources 消费 Promoted Candidate 时，以显式的候选校验任务替代
  不参与测量的本地 Release build；仍须完整验证 Source Freeze、clean tree、EXE/ZIP/SBOM
  hash、checksum 与 native-runtime。候选缺失或失效即失败，不能回退本地构建；Local
  Preflight、package/release 与 headless CI 的原有构建职责保持不变。
- Resources 的任一样本超限或最大值硬门，在一个完整样本已确定失败时立即停止后续
  采样及任务。CPU 必须完成原有 60 秒平均窗口后再判断；窗口内分桶不能提前判失败。
  失败收据保留实际样本、已完成次数、门槛及错误，不写 last-success。成功仍须完成
  全部五次采样、原预热/采样时长及完整压力循环。此规则不应用于 startup p95 门。

### Phase 14 qualification environment、独立证据通道与 partial evidence

Phase 14 exact-candidate campaign 在任何 GUI runtime、performance、resources 或人工观察（M34 Clean VM 的首次运行
观察除外，见[人工观察记录](#manual-observation-log)）前，
必须先用 verification tooling 查询当前 Windows session 的实际交互条件。统一状态为：

```text
VALID
ENVIRONMENT_BLOCKED
UNSUPPORTED
ERROR
```

只有 `VALID` 可以继续形成 GUI 证据。锁屏、断开的 session、不可访问 input desktop、当前进程
无法写入物理 cursor position 或缺失交互 shell 等情况必须以
`NOT_TESTED — ENVIRONMENT BLOCKED` 和非零退出码 fail fast；它既不是产品 FAIL，也不是 PASS。
cursor capability probe 只能把当前坐标写回原位，不得产生可见位移。机器可读环境事实不得包含
窗口标题、用户名或完整路径。该检测只能位于 smoke/tooling adapter，不得进入产品 runtime。

Resources 长矩阵必须在主要场景之间重检环境，并在每个完成场景后覆盖写入 partial receipt。
未完成 receipt 必须显式包含 `INCOMPLETE`，readiness 仍要求最终 receipt 的所有 result 都为
`PASSED`，因此 partial evidence 不能冒充完整 PASS。

M1..M5 manual sessions 只允许共享 setup，不改变 P12-M01..P12-M44 的逐项 authority。Phase 14
保留 G1/G2 guided sessions；每个 guided observation 必须显式映射到一项或多项相同观察事实的
case ID，并为每项分别回答 `MANUAL_PASS` / `MANUAL_FAIL` / `NOT_TESTED`（后者只在会话输出中出现，不写入观察
日志），不能用“整体看起来正常”
提升整组状态。

G3 clipboard/export/recovery/asset-safety 路径由本地、串行、独占交互桌面的 exact-candidate
自动化持有。Rust CLI 负责隔离候选目录、标准 Windows clipboard producer、进程/文件断言、
候选身份和 receipt；PowerShell/UI Automation 只可作为 native save dialog 与 tray menu 的薄适配，
不得持有 PASS/FAIL 规则。每项使用独立候选副本，数据安全断言一项失败即整项 FAIL，禁止套用
desktop jitter 成功率。GitHub-hosted CI 只运行该 harness 的无界面 parser/receipt/fixture 测试，
不得在非交互 runner 启动 G3 GUI。Explorer、Snipping Tool、browser 本身的 UI 操作可作附加人工
spot check，但标准 clipboard 格式的 exact-EXE 集成结果才是该组可重复的 release evidence。

G4 tray/dock/editor-compatibility/identity/real-IME 路径复用同一 exact-candidate 生命周期与收据合同，并保持
六个高内聚组：G4-01 tray 菜单、close/show/dirty quit；G4-02 主屏 Left/Top/Right、3 DIP 感应条、
24/25 DIP capture、`Top > Left > Right`、700/100/500 ms 与 focus/IME/Pin guard；G4-03 legacy
clipboard shortcuts 与 Preview 只读；G4-04 真实 toolbar 数学分隔符转换、源码即时投影、literal
safety 与单次 Undo；G4-05 真实 junction canonical identity、同 HWND 唤醒与第二实例零 durable write；
G4-06 临时激活 Microsoft Pinyin / WeType 真实 TSF profile，以物理键盘验证 Source/Search composition、
commit/cancel、selection replace 与一次 Undo，并在结束前恢复原 active profile。
G4-02 的物理拖动必须区分两种终态：Floating 定位要求窗口停在请求坐标附近；Dock/capture 操作允许产品在
释放鼠标后把窗口归一化到工作区边缘。后者不得再用固定物理像素比较“请求坐标 == 最终坐标”，而必须以
DIP 转换后的输入阈值、稳定的 HWND geometry 与产品持久化 dock fact 共同确认。特别是 125/150/200% DPI
下，24 DIP 本来就可能大于 24 physical pixels；测试不得让 96-DPI 偶然相等掩盖单位错误。
TSF active-profile 回读只证明输入桌面选择了指定 profile；目标窗口的 `GetKeyboardLayout` 只证明 HKL / LANGID，
当多个 TIP 共享 `0x0804` 或 profile 不提供 substitute layout 时，不得把它当作中文转换子模式的确认。
`WM_IME_CONTROL` 的 open/native 回读同样只能作为兼容性预置，不能替代真实按键行为。G4-06 必须以“拼音物理
按键未写入 canonical text”为 composition acknowledgement；每次 composition 前都必须重新激活并回读同一 profile，
再对目标 HWND route，不能把“窗口原本已有 focus”当成无需重申 profile。若本 profile 会话尚未成功证明过 composition，
首次探针成为普通 ASCII edit 时，必须先取消残余 composition、Undo 回到原文，再且仅再执行一次被测 profile 的
用户等价物理 `Shift` 模式纠正；纠正后必须再次激活并回读目标 TSF profile、重新 route 目标 HWND，并重新设置及
回读 open/native compatibility state，然后才允许第二次行为探针。若会话已经证明过 composition，后续 ordinary ASCII 只能再次重申 profile，不得
盲目 `Shift`。第二次仍为 ASCII 必须 FAIL，不得追加 sleep 或无限重试。任何被工具执行的模式纠正必须在 profile
restore 和 child teardown 之前对称恢复；正常路径恢复失败必须使 case 失败，错误返回与 unwind 路径由 RAII
best-effort 恢复。
G3/G4 都必须串行且独占桌面，不能并发争抢 clipboard、tray、窗口焦点或鼠标；单项诊断 receipt
不能替代完整六组 receipt。P12-M11/M12 的 mixed-DPI 实机事实仍属于 G2 人工验收。
G4-01 的 tray UIA adapter 必须把物理右键视为请求而非菜单已打开的 acknowledgement：每次尝试都要
重新解析 `StickyMD` tray icon、将鼠标移动到当前 icon rectangle 并回读实际 cursor position；只有目标
进程的产品 menu items 已出现才算打开成功。第一次没有出现时最多允许一次相同语义的有界重试，禁止
无限点击或仅延长固定 sleep。菜单检查后的 Escape 必须等待目标 menu items 消失，避免旧 popup 污染下一步。
最终失败必须报告尝试次数、icon/cursor geometry 与实际观察到的 product menu item names，不能把“物理
右键未路由”误报成“显示菜单项不存在”。这些动作只增强 verification adapter；tray lifecycle、窗口可见性、
同 HWND、文本与 durable save 的 PASS/FAIL authority 仍由 Rust CLI 持有。

G5 shell/compact/presentation/rendering 路径复用相同 exact-candidate identity、独立候选副本与独占
交互桌面。G5-01 以真实 HWND style 和 focus transition 持有 P12-M03/M04 的可机械 shell eligibility；
G5-02..04 自动驱动 compact、zoom、opacity、theme 与 rendering stress，并把窗口截图的相对路径和
SHA-256 写入 exact receipt。截图适配器只采集像素，不得判 PASS。真实输入法候选窗视觉、物理
mixed-DPI/多屏、System 主题实际切换和首次视觉判断仍由人工 authority 持有；G5 companion evidence
只能减少重复操作，不能把这些观察静默升级为人工 PASS。G3、G4、G5 必须串行执行。

Phase 14 固定顺序为 Source Freeze → Local/Source Preflight → explicit PUSH authority → remote CI/release
workflow → download/Promote → Environment → Runtime → Performance → Resources → G3 → G4 → G5 → Manual
→ Readiness。Local Preflight 不生成 final candidate；Promote 前不得执行 final artifact-bound qualification。
Environment invalid、candidate identity mismatch、P0/security/data-safety failure 或 receipt schema corruption
是全局停止条件；普通 Runtime、Performance 或 Resources failure 必须分别记录并继续运行仍独立且安全的
后续通道。尤其 Performance failure不得跳过 Resources，Resources failure 也不得抹去 Performance receipt。

<a id="desktop-repetition-jitter-policy"></a>
### Desktop repetition jitter policy

只有显式标记为桌面输入/焦点/窗口调度抖动的重复 GUI qualification 可以使用成功率处置。
适用集合必须包含至少 100 个相互独立的 copied-Release run；每个失败必须保留 run、stage、窗口
activation/geometry 与原始错误。对该集合使用严格不等式：

```text
success rate >= 98% -> PASS
success rate < 98%  -> FAIL
```

98% 边界为包含关系，不设置中间人工处置状态。即使整组 PASS，所有失败 run 的 stage、activation/
geometry 与原始错误仍必须保留。该策略是工程经验阈值，不得描述成严格的正态三西格玛推断。

以下情况永远不适用成功率容错：deterministic unit/integration test、canonical text 或 durable file
不一致、保存/恢复/原子替换错误、进程崩溃、security/P0 failure、resource/performance hard gate、
receipt identity/schema 错误，以及任何未能分类为桌面环境抖动的失败。只要集合中出现一项上述
blocking failure，整组仍为 FAIL。完整 Resources 自身不因该策略自动放宽；低频交互失败只能由
独立 reducer 形成满足样本数的 jitter receipt 后，再按本节处置。

人工发布政策按风险分层：Tier A 是 release-critical human gate，除非 USER 明确批准具体
case/group waiver，否则必须 PASS；Tier B 是环境依赖 gate，可由 USER 对绑定版本与 exact source
的明确组 waiver 处置；Tier C 的 `NOT TESTED` 在对应自动化合同已 PASS 时不阻断，已观察到的
`MANUAL_FAIL` 仍阻断。waiver 的绑定与范围见上文 Phase 12 发布资格状态（版本、Source Freeze、人工观察上下文与
`(项, 角色)` 集合），不跨版本继承。缺 `previous` 角色观察时，
USER 可用档案 key 处置为 `USER WAIVED` 并计入 READY；汇总逐项逐角色列出，并写明这表示"符合批准的发布政策"，
不表示技术验收全部完成。

v0.1.0 允许 unsigned Authenticode distribution；package/receipt 必须明确记录 unsigned，README
与 release notes 必须说明 Windows reputation warning 及 checksum/attestation 验证方法。自动化
不得伪造签名字段，也不得因为缺少 Authenticode 签名而判 package failure。

CI evidence 分三层：GitHub-hosted CI 只运行 deterministic build/test/package，不执行绝对
550 ms startup 或资源门；本地可信/专用 Windows qualification host 负责 absolute performance/
resources；真实 TSF profile 的 composition/commit/cancel/Undo/Search 等客观事实由 exact-candidate
自动化负责，候选窗视觉、普通 UI 视觉、tray/docking 主观观感与物理显示拓扑由 human acceptance 负责。不得把对外
公开仓库连接到高权限、长期在线的自托管 runner；未来优先使用 pull-based local lab 或隔离的
private release lab。

### CI 与完成门

- 完整 Windows CI 必须调用 Rust CLI 的 `all --ci`，或调用经 CLI 单元测试证明并集等价的
  `tests` + `performance` 分片，覆盖所有能够无界面执行的 Phase 任务；日常 push / PR 可按
  [模块化 headless CI](#modular-headless-ci)执行定向集合，不能将其描述为完整资格化通过。
- Phase 专用 PowerShell 入口保留给本地定位与独立复核；CI 使用合并任务图避免重复工作。
- 人工项保持 `NOT TESTED` 不会使 headless CI 失败，但会阻止对应 Phase / release gate
  被描述为完整通过。
- smoke 只证明其任务清单；synthetic IME 不能替代真实 profile exact case，真实 profile 功能 case 也
  不能替代候选窗视觉、多显示器、性能测量或故障现场等明确的人工验收。

---

## Release 合同

<a id="portable-windows-runtime"></a>
### Portable Windows runtime

- Windows x64 Release 必须能在未安装 Rust toolchain、Visual Studio 或独立 Visual C++
  Redistributable 的原生 Windows 11 上启动；只允许依赖该系统自带的 Win32 DLL 与 API set。
- `x86_64-pc-windows-msvc` 通过仓库级 target 配置静态链接 MSVC CRT。该配置只是构建输入，
  不能单独作为 artifact 自包含性的证据。
- std-only `stickymd-smoke` 必须解析 exact Release PE32+ 的普通 import table 与 delay-load
  import table，并拒绝 `VCRUNTIME*`、`MSVCP*`、版本化 `MSVCR*`、`CONCRT*`、debug UCRT 以及
  GNU C/C++ runtime DLL。Release/package task graph 与 GitHub Release workflow 都必须在打包前
  执行该 gate。
- 除 `api-ms-win-*` / `ext-ms-win-*` API set 外，PE gate 使用经审查的 Windows inbox DLL
  allowlist；未知 import fail closed，不能只依赖 developer-runtime 黑名单。
- Windows 11 自带的 `api-ms-win-crt-*` API set 与 `msvcrt.dll` 不属于独立开发者环境，允许导入。
- PE import gate 证明 artifact 没有已知的外置开发者 runtime 依赖，但不能替代 Clean Windows 11
  VM 人工运行。该人工项在真实执行前必须保持 `NOT TESTED`。

### 触发与步骤（方向性）

候选构建与发布晋升必须分离，禁止 tag 事件重新 build 已验收 candidate：

1. `release.yml` 只接受显式 `workflow_dispatch`，从 Source Freeze 构建、测试、deny、检查 manifest、
   native-runtime import、生成 portable ZIP/checksums/license/SBOM 并上传 workflow artifact；它不创建 tag、
   draft 或 publish。
2. 本地下载并 Promote 该 artifact，完成全部 artifact-bound qualification，readiness 必须为 `READY`。
3. USER 分别授权 TAG、DRAFT-RELEASE、PUBLISH 后，`promote-release.yml` 以独立
   `workflow_dispatch` operation 执行对应动作。每次 dispatch 显式携带 source SHA、原 candidate
   workflow run id、预期 ZIP/SBOM SHA-256 与 tag。
4. TAG operation 只为 exact source 创建 tag；DRAFT operation 从指定历史 workflow run 下载同一 artifact，
   重新校验预期 hash 后生成 provenance/SBOM attestation 并创建 draft；PUBLISH operation 在重新下载并校验
   draft assets 后只把既有 draft 公开。

任一 promotion operation 都不得重新运行 Cargo build/package。artifact run id 或 hash 不匹配必须 fail closed；
不得自动选择“最新同 commit build”替代已验收 artifact。TAG/DRAFT/PUBLISH 仍是三个独立 USER authority，
前一项批准不授权后一项。

### 发布物

```text
StickyMD-0.1.0-windows-x64-portable.zip
└─ StickyMD\
   ├─ StickyMD.exe
   ├─ README.txt
   ├─ LICENSE.txt
   ├─ THIRD_PARTY_NOTICES.txt
   └─ licenses\
      ├─ SIL-OFL-1.1.txt
      └─ KaTeX-fonts-NOTICE.txt

SHA256SUMS.txt
SBOM.spdx.json
```

- v0.1.0 不发布独立 symbols ZIP；若未来版本新增调试符号资产，必须先更新本合同、打包
  allowlist 与 exact-artifact 校验，不能把未生成的文件列为强制发布物。
- 不预创建用户 `note/`（首次运行创建）。
- v1 不提供：MSI、MSIX、Microsoft Store、自动更新器、管理员安装、
  Program Files 安装。代码签名可后续加入，不阻塞开源 v1。
- License：MIT；数学字体为 OFL 1.1，release 必须附带相应声明。

### unsafe 边界合同

- `stickymd-core` / `stickymd-render`：`#![forbid(unsafe_code)]`。
- `stickymd-win`：`#![deny(unsafe_op_in_unsafe_fn)]`；所有 unsafe 只位于
  `platform/windows/` 或经批准的 RichEdit fallback，紧邻 `SAFETY` 注释，
  不把裸句柄泄漏到核心层。

### 依赖治理合同

- 保留 `Cargo.lock`，正式构建使用 `--locked`。
- 新增依赖前检查：许可证、transitive、二进制体积、MSRV、现有依赖能否完成。
- 禁止依赖清单见根 `AGENTS.md`；例外需 ADR + USER 批准。

---

## Failure Paths

- release gate 未过（测试/内存/体积）：不发布。
- 依赖 advisory：按 scheduled 报告处理，不静默忽略。
- 手工验收未完成的 draft release：不得公开。
- Phase smoke 入口缺失、矩阵缺失或矩阵声称无可重复证据的 PASS：阶段 gate 失败。
- Rust CLI 中途失败：立即返回非零退出码；后续任务不伪造 PASS。

## Configuration

Not applicable。

## Lifecycle

Phase 建立时先创建 smoke 入口与矩阵；实施中持续更新自动化映射；收尾时在当前提交上
执行入口并更新状态。后续 Phase 不删除早期入口。release 以 Definition of Done 全过为准。

## Extension / Replacement Points

Rust smoke CLI 的 task 实现、fuzz 引擎、golden 容差策略。CLI 可整体替换，但稳定的
`tools/smoke/phase-XX.ps1` 与矩阵路径不变。

## Performance Critical Paths

合并 smoke 任务必须按 identity 去重；不得因 Phase 数量增加而重复执行相同 workspace
测试。产品性能目标仍由 `10` 持有。

## Verification

- `tools/smoke/phase-00.ps1` 验证治理文件、阶段矩阵、入口结构、AC 编号、plan_ref、
  本地文档链接与禁止依赖。
- `tools/smoke/all.ps1 -Ci` 等价调用 Rust CLI 的合并 headless 任务图，并包含 Release
  性能入口但不包含窗口 runtime smoke。
- CI Windows job 必须执行该合并入口。

## Non-Goals

自动更新、遥测上报、在线 CI 缓存之外的云服务、多平台发布。
