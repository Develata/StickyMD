# 11_testing_and_release.md - 测试与发布合同

## Metadata

- `Layer`: Verification
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-28
- `Scope`: v1 测试类别、逐阶段 smoke、验收证据与发布形态合同

---

## Purpose

定义 StickyMD v1 的验证体系、逐阶段可重复 smoke、验收证据状态与发布形态。

## Boundary

- 验收案例的具体内容在 `docs/acceptance-cases/`；本章定义类别与规则。
- 性能目标的性质定义在 `10_performance_reliability.md`。

---

## Owned Objects

测试 fixture、golden baseline、验证收据与 release artifact manifest；这些是验证证据，
不是产品运行时 authority。

## Inputs

冻结 plan/feature/acceptance contract、候选 commit、锁定依赖、测试环境与 fixture。

## Outputs

PASS/FAIL/NOT TESTED 收据、差异 artifact、checksums、SBOM 与 portable ZIP 候选。

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

- 系统：当前与前一个受支持 Windows 11 版本；100/125/150/200% DPI；
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
  Rust CLI 必须证明 CI 分片任务并集与完整 `all --ci` 去重任务图一致。
- Portable-core job：在 Linux runner 上只构建平台无关 crates（防止平台无关代码
  被 Win32 污染）；目的不是发布 Linux app。
- Scheduled：advisories、依赖更新 dry-run（不自动合并）、fuzz smoke、
  sanitizer/Miri 平台无关核心、许可证报告。
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
  定向诊断证据，不能冒充最终候选的完整 Resources receipt。候选冻结、发布资格化或 USER
  明确要求全量时才运行完整 Campaign。
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

<a id="qualification-process-isolation"></a>
### GUI qualification process isolation

- Rust smoke CLI 启动的每个 StickyMD GUI child 必须在 `spawn` 成功后立即移交给统一的
  RAII owner。正常路径可以显式等待或终止；任何 `?` 提前返回、错误传播或 Rust unwind 都必须
  由 owner 的 `Drop` 执行 best-effort `kill + wait`，不能依赖函数末尾的手工清理语句。
- Startup Performance 与所有 Resources 模块在创建 fixture 或启动新 child 前，必须枚举当前
  Windows session 中既有的 `StickyMD.exe`。若其可执行文件位于 smoke tooling 自己命名的系统
  temp qualification root，则判为 stale smoke-owned process，当前测量 fail closed，且不得生成
  Performance/Resources PASS receipt。
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
MANUAL PASS      当前提交上的正式人工矩阵已执行，并引用完整环境/步骤/结果收据
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

Phase 12 source decision template 与 exact-candidate `dist/evidence/release-decisions.json` projection
只允许 `PENDING`、`USER APPROVED`、`USER REJECTED`、`NOT APPLICABLE`。Rust automation 只能
记录 USER 已明确给出的决定，不能自行批准；人工 waiver 必须使用具体 `WAIVER-P12-Mxx`
key，不接受 blanket waiver。

### Exact-artifact evidence receipts

Phase 12 source freeze 前提交所有 source-controlled 治理、工具和报告；freeze 后的 candidate、
automated、manual、remote、downloaded-artifact 与 readiness receipts 写入 ignored
`dist/evidence/`，绑定 source commit、EXE SHA-256 与适用的 ZIP SHA-256。这样人工/远端
证据不会为了“写回报告”再制造一个不同 HEAD。

- manual recorder 必须是交互式 human receipt recorder，只接受显式
  `MANUAL_PASS` / `MANUAL_FAIL` / `NOT_TESTED`，不得从 process/status 自动推断人工 PASS；
- stale source/EXE receipt 不参与 readiness；G3/G4/G5 exact receipt 还必须绑定 ZIP、运行 harness commit、
  clean worktree 与各组预期逐项结果，不能用旧候选或开发期 dirty receipt 替代；
- readiness 对 P0/P1、未批准 hard gate、mandatory manual NOT TESTED、exact package、remote
  evidence 与 USER decision fail closed；不得提供 `--force-ready`；
- freeze 后若任何 source、manifest/lock、runtime asset 或 release tooling 改变，所有 receipts
  失效并必须重建。

### Phase 14 qualification environment、独立证据通道与 partial evidence

Phase 14 exact-candidate campaign 在任何 GUI runtime、performance、resources 或人工观察前，
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
case ID，并为每项记录 `MANUAL_PASS` / `MANUAL_FAIL` / `NOT_TESTED`，不能用“整体看起来正常”
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
TSF active-profile 回读只证明输入桌面选择了指定 profile；目标窗口的 `GetKeyboardLayout` 只证明 HKL / LANGID，
当多个 TIP 共享 `0x0804` 或 profile 不提供 substitute layout 时，不得把它当作中文转换子模式的确认。
`WM_IME_CONTROL` 的 open/native 回读同样只能作为兼容性预置，不能替代真实按键行为。G4-06 必须以“拼音物理
按键未写入 canonical text”为 composition acknowledgement；每次 composition 前都必须重新激活并回读同一 profile，
再对目标 HWND route，不能把“窗口原本已有 focus”当成无需重申 profile。若本 profile 会话尚未成功证明过 composition，
首次探针成为普通 ASCII edit 时，必须先取消残余 composition、Undo 回到原文，再且仅再执行一次被测 profile 的
用户等价物理 `Shift` 模式纠正。若会话已经证明过 composition，后续 ordinary ASCII 只能再次重申 profile，不得
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

Phase 14 固定本地顺序为 Environment → Release/package → headless CI → Runtime → Performance →
Resources → Manual → Readiness。Environment invalid、candidate identity mismatch、P0/security/
data-safety failure 或 receipt schema corruption 是全局停止条件；普通 Runtime、Performance 或
Resources failure 必须分别记录并继续运行仍独立且安全的后续通道。尤其 Performance failure
不得跳过 Resources，Resources failure 也不得抹去 Performance receipt。

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
`MANUAL_FAIL` 仍阻断。waiver 只绑定声明的版本、source SHA 与 case/group，不跨版本继承。

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

- Windows CI 必须调用 Rust CLI 的完整 `all --ci`，或调用经 CLI 单元测试证明并集等价的
  `tests` + `performance` 分片，覆盖所有能够无界面执行的 Phase 任务。
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

tag `v*` 触发：版本一致性校验 → 测试 → deny → release build → manifest 检查
→ native-runtime import gate → smoke test → portable ZIP → SHA-256 checksums → 许可证 notice → SBOM
→ provenance/attestation → draft release → 人工 Windows 11 验收后发布。

### 发布物

```text
StickyMD-0.1.0-windows-x64-portable.zip
├─ StickyMD.exe
├─ README.txt
├─ LICENSE.txt
├─ THIRD_PARTY_NOTICES.txt
└─ licenses\
   ├─ SIL-OFL-1.1.txt
   └─ KaTeX-fonts-NOTICE.txt

StickyMD-0.1.0-SHA256SUMS.txt
StickyMD-0.1.0-symbols.zip
SBOM.spdx.json
```

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
