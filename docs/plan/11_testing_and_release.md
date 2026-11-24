# 11_testing_and_release.md - 测试与发布合同

## Metadata

- `Layer`: Verification
- `Status`: Approved Contract
- `Version`: 0.5.0
- `Last Review`: 2026-11-24
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
  sleep/resume、RDP reconnect。其中需要额外机器、虚拟机或显示器的部分（`previous` 角色、Clean VM、
  双显示器与拔插、物理负坐标布局、RDP）是可选人工项：未测不阻断发布，已观察到的 FAIL 仍阻断（见下文人工发布政策）。
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
- 工程模块沿用 Cargo 边界：core、render、windows、smoke、两个 Phase 01 实验（Phase 15 起增加 app、desktop，见下文
  Phase 15 节；Phase 16 增加 mac）；与
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

<a id="phase-15-shared-core-migration"></a>
### Phase 15：共享实现迁移

Phase 15 按 [提案](../report/2026-10-23-shared-core-thin-platform-shell-architecture.md) 第 8 节分 A–D 四个里程碑，把
共享实现迁入 `stickymd-app` 与 `stickymd-desktop` 并接回 Windows（`03` 共享实现节）。首次实施时建立
`tools/smoke/phase-15.ps1` 与 `docs/acceptance-cases/phase-15.md`。只完成 A 时不得宣称 Windows 已成为完整薄壳。

| 里程碑 | 交付 | 出口（在下列每批出口之外） |
| --- | --- | --- |
| A | plan/projection 已批准；建立 `stickymd-app` 并同时登记其治理、CLI 任务、Linux/Windows 测试选择与资格化指纹归属；迁移 intent、纯 coordinator、配置模型、Search 与会话逻辑 | `stickymd-app` 在 Windows 与 Linux CI 上真实构建并执行语义测试，不以 `cfg(windows)` 排除主体或测试；原有语义测试随责任迁移并在新 package 下实际执行 |
| B | 入口依赖：三类无身份 I/O 回执的 Windows 修复已合并（`04` Generation 规则 6），且以下测试通过：身份失配的回执不产生任何副作用；有效的旧 generation 保存回执仍被接受（saved_generation 只推进到实际落盘版本，较新编辑保持 dirty）。按保存/恢复、预览、资产/导出、窗口/退出顺序接入 Capability Port，窗口动作按 `04` 的请求、回执与提交 | 每条链的成功/失败/过期回执测试与“隐藏被拒绝”场景 |
| C | 建立 `stickymd-desktop` 并同时登记其治理、任务与指纹归属；各 target 显式选择 winit/softbuffer backend features；迁移通用控件、绘制、命中与事件翻译（含 `06` 帧呈现规则） | `stickymd-desktop` 在 Windows 与 Linux CI 上真实构建；`age = 0/1/2` 呈现测试；Windows 桌面定向回归 |
| D | 迁移剩余业务分支，删除仅为迁移保留的转发 | 依赖与平台泄漏审查、适用 baseline、Windows 验证；未实现平台保持 `NOT TESTED` |
| E | 把不依赖文件语义的裁决移入 `stickymd-app`：配置文本规则、启动恢复裁决、粘贴批次、导出命名与引用改写。存储事务仍由宿主按 `05` 执行（USER 批准 2026-11-09） | Windows 现有测试断言不变；共享裁决测试在 Linux 也运行；Windows 桌面回归 |

C 与 D 在同一候选 SHA 收口：`ApplicationHandler` 依赖 `AppSession`，C 的出口回执与 D 的业务裁决迁出无法分先后。C 的 Linux 构建与
`age` 呈现测试在各自批次成立；Windows 桌面回归在 D 之后执行一次（USER 决定 2026-11-07）。

每批出口（A–D 都适用，不推迟到 D）：

1. **治理随代码移动：** 同一提交内更新受影响的硬编码路径检查（`unsafe` 禁令、窗口协调的平台类型禁令等），新增业务
   module 带 `plan_ref`；旧路径检查不能因代码迁出而空转。
2. **测试选择与指纹：** 验证本批新增、移动、删除路径的任务选择结果与全量任务并集；新路径缺规则或旧规则无命中即失败。
   旧收据保留历史身份，有效性按新指纹重新判断，纯搬迁不视为旧证据仍有效。
3. **数据字节兼容与回退：** 指定前一已验证检查点；在数据副本上执行无操作启动、保存与恢复，比对 note/config 的路径与
   预期字节、图片未被改写，再用回退到检查点的版本重新打开同一副本。受影响的 Windows 行为断言列为该批出口。
4. **性能：** 按 `10_performance_reliability.md#phase-15-migration-gates` 判定。

完成标准还包括：编辑、Search、配置、保存、预览、冲突、恢复、资产与退出规则各只有一个共享所有者；Windows 重新验证文件
替换/冲突/图片安全、IME、托盘可达性、透明度、置顶、贴边、多屏、剪贴板/导出（自动化与人工结果分别记录；剪贴板多格式
选择按 [plan 08 剪贴板验证](08_assets_and_export.md#clipboard-priority-verification)）。共享层完成、Windows 回归、跨平台应用可用与发布 readiness
分别报告；测试替身只证明接口与共享语义，不证明原生实现正确。

<a id="phase-16-macos-host"></a>
### Phase 16：macOS 宿主

依据 [决定报告](../report/2026-11-09-macos-platform-decision.md) 与 `02` macOS 节。验收见 `docs/acceptance-cases/phase-16.md`，自动化由 Rust CLI 持有，`tools/smoke/phase-16.ps1` 是稳定入口。

| 里程碑 | 交付 | 出口 |
| --- | --- | --- |
| M0 | 基线：macOS 移除没有 `head` 表的字体 | 四个共享 crate 的测试在 macOS 全部通过 |
| M1 | 建立 `apps/stickymd-mac` 与原生 POSIX 层 `crates/stickymd-posix`，同时登记 `unsafe` 治理、CI 选择与指纹归属；存储、单实例、监视、启动 | 真实存储通过 `storage_conformance!`；`05` macOS 节的失败路径有测试；macOS 测试与 clippy 显式包含两个 crate |
| M2 | 窗口、托盘、唤醒、光标图层、系统终止；共享层 S2、S4、S6、S7、S8 | Windows 现有断言不变（S7、S8 按批准的新行为更新）；验收表各行按下文的验收阶梯判定：`Automated` 由各行写明的程序判定（虚拟机中的 `mac-gui` 或单元测试），`Agent` 由两名 agent 判定，`Review` 与 `Manual` 由 USER 判定 |
| M3 | 输入、剪贴板、导出；共享层 S3 | `08` macOS 节的格式验证；基线测量 |
| M4 | `.app` 打包脚本、ad-hoc 签名、人工验收清单 | 未运行项保持 `NOT TESTED` |

共享层改动的 Windows 桌面回归只能在 Windows 机器上运行；S8 按 USER 决定不单独回归，验收表记为 USER 豁免。

**macOS 帧呈现（方案 B，USER 批准 2026-11-14，[报告](../report/2026-11-14-macos-plan-b-iosurface.md)）的出口：**

- **自动化：**
  - 宿主呈现路径的共享层测试：三种结果、暂缓重试、光标降级、隐藏释放与再显示；
  - 呈现器的单元测试：缓冲选择、BGRA 写入与行跨度、创建失败不崩溃；
  - 虚拟机里同一批次基线与新包交替跑 `mac-latency` 与 `mac-resources`，以及 `mac-gui`。
- **人工（V04）：** USER 把新包与正式包并排目视比较颜色、方向和撕裂。按下文的验收阶梯转换后改为：颜色与方向由程序比对像素；新包与基线包用同一脚本录屏，USER 看打字、滚动、拖动缩放、切换视图时的闪白、残影与错帧（第 5 级）；扫描线级的撕裂在合成后的录像里看不出，列为未测。转换完成之前仍按前一句做。
- **Windows：** 共享层 `render.rs`、`frame/surface.rs` 的改动需要 Windows 桌面回归。

`mac-gui` 的结果按行记入账本 `dist/evidence/mac-gui-ledger/records.jsonl`，由 `stickymd-smoke mac-ledger` 管理（USER 批准 2026-11-12，[复用提案](../report/2026-11-11-macos-gui-rerun-reuse.md)）。
证据的锚点是**产品输入指纹**，不是 app 的字节（USER 批准 2026-11-17，[锚定提案](../report/2026-11-17-macos-evidence-anchoring.md)）：

- **产品输入指纹**：`stickymd-package app-inputs` 在仓库根目录对能进入 `build-app.sh` 所建 app 的全部 tracked 文件计算，与功能模块指纹共用同一算法，
  根 `Cargo.toml` 与 `Cargo.lock` 的发布版本同样规范化。指纹写进 bundle，所以计算它的代码在打包 crate 中，是两个平台的构建输入（#platform-artifact-reuse）；
  `mac-ledger` 与 `mac-gui` 读取与比对它时调用同一实现。除列为不能进入 app 的目录与文件（Windows 宿主、`tools/`、`docs/`、`.github/`、`experiments/`、
  根 `tests/`、`assets/`、`dist/`、根目录的说明文档与 `deny.toml`）外，任何文件都是输入；新文件只会导致重跑，不会导致误复用。
- **构建身份**：指纹最后接上目标 `aarch64-apple-darwin`、`--release --locked`、`cargo -V`、`rustc -vV`、`xcode-select -p`、SDK 路径与版本、
  `clang --version`、`ld -v`。
- **干净的构建环境**：`build-app.sh` 在仓库根目录、以只含 `HOME` 与固定 `PATH`（先系统目录，链接驱动因此是系统的 `cc`，再接调用者 `cargo` 所在目录）的环境
  计算指纹并构建，所以调用者的其他环境变量进不了构建。仓库之外存在 Cargo 会合并的配置文件（各级祖先目录与 Cargo home 中的 `config`/`config.toml`）时，
  `app-inputs` 拒绝；Cargo home 按脚本以参数传入的 `HOME` 定位，Cargo 配置的 `[env]` 改不了它。需要非空的 `HOME`；
  直接调用 `app-inputs` 时，`RUSTFLAGS`、`RUSTC` 与各种 wrapper、`CARGO_PROFILE_*`、`CARGO_BUILD_*`、`CARGO_TARGET_*`、`CARGO_RESOLVER_*`、
  `MACOSX_DEPLOYMENT_TARGET`、`SDKROOT`、C 工具链的 `CC`/`CFLAGS`/`LDFLAGS` 等变量也会被拒绝。
- **信任假设（不进指纹的输入）**：
  - 仓库内被跟踪的 Cargo 配置若 `include` 仓库外的文件，或指向仓库外的程序、lockfile 与路径依赖，这些外部内容的变化不进指纹（现有配置只有 Windows 的 rustflags）；
  - 保留下来的 `HOME`（rustup 的 `settings.toml` 与工具链目录、`~/.cargo` 中的依赖缓存）与调用者 `cargo` 所在目录：工具版本输出相同、内容不同的工具链或标准库不会被发现；
  - Xcode 选中的开发目录中，同路径、同版本的工具被替换不会被发现；
  - 依赖的 build script 读取仓库外文件、系统工具读取用户目录下配置的情况，没有穷尽核查。
- **写入 bundle**：`build-app.sh` 先算指纹，以固定的 `--target-dir` 构建并从同一目录复制可执行文件，生成 Info.plist（指纹写进 `StickyMDInputs`）后再算一次，
  两次不同即拒绝，相同才签名。两次检查之间被改动又改回的输入无法发现。

- **记录**：账本只追加。每次追加都在排他锁下读取、校验，再原子发布，公共证据写入口不得写入账本目录。`record` 从同一次读取的证据字节中，记下一次运行的每一行，连同以下身份：app 的产品输入指纹（`mac-gui` 从 bundle 读出写进证据）、驱动的 SHA-256（由 `mac-gui` 写进证据；运行期间不得替换驱动）、虚拟机的 `sw_vers`，以及运行者传入的镜像 digest；app 可执行文件与 bundle 清单的 SHA-256 一并记下，只供审计。一个 digest 只能对应一个系统版本。`record` 只登记在虚拟机中跑完的运行（证据为 schema 2，且 `finished` 为真：驱动在最后一行结束后才写 `true`，检查点与被取消、因异常或清理失败而中止的运行都写 `false`；更早的证据不能登记，要重跑，不得手工补写；`hv_vmm_present` 为 1，且未用 `--allow-host`），每行的结论须与它的各项检查一致（有 FAIL 即 FAIL，否则有未测或没有检查即 NOT_TESTED，全部通过才 PASS）；同一份证据的同一行不登记两次。墙钟的开始时间不能证明两次运行的先后（会被校时改变），所以账本不按时间排序，复用规则也不依赖先后（见下条）。账本里的摘要按不区分大小写读取。
- **复用**：`plan` 只读账本，不启动被测程序，需要传入目标虚拟机运行前的 `sw_vers`。某行在账本里已有 PASS，且产品输入指纹、驱动、镜像、系统四项与当前完全一致时，复用这次 PASS；只看最近的一次 PASS：同一身份下这一行记过任何 FAIL 或 NOT_TESTED，或有任何一次 PASS 被 USER 撤销（都不论先后）时，不复用，该行列入要运行的清单，也不回退去复用更早的 PASS。被撤销的 PASS 与 FAIL 同样处理：导入顺序和墙钟都不能证明哪次运行在撤销之后，所以重新导入同一次运行改写过字节的证据、或撤销之后才登记的较早运行，都不能恢复复用。因此同一身份下失败过或被撤销过的行，直到身份改变之前每次都重跑（I01–I03 与 R03 只看本批，在新批次重跑）；历史记录保留；这次 PASS 记录的 app 字节与当前不同时，作为审计信息列出。没有产品输入指纹的 app 或记录（指纹引入之前的）不参与复用。其余行列入要运行的 `--only` 清单。FAIL 与 NOT_TESTED 从不复用。
- **人工收据**：`receipt` 按行记录 USER 的确认，绑定 app 的产品输入指纹（app 可执行文件的 SHA-256 只供审计）；没有指纹的 app 不能记收据。指纹变了，收据不再成立，`plan` 会列出待重新确认的人工行。Mode 为 `Review` 的行同样以收据记录 USER 的判定，收据与 Agent 的判定记录绑定相同的字段（同一次运行的证据摘要、行与子项、所看画面的角色与哈希、判定标准的版本、对照）；账本记录这种收据之前，`receipt` 拒绝 Review 行，Review 行不能为 `MANUAL PASS`。I01–I03 与 R03 从不沿用：收据、运行结果与判定都只代表这一批验收。批次由 `--batch` 命名：运行的批次由 `mac-gui --batch` 在运行开始时写进证据，`record` 从证据读取，不能在登记时指定（旧运行因此不能被算作新一批）；收据的批次在 `receipt` 时指定。只有本批的结果计数：同批次内的补跑可以沿用，下一批即使四项身份相同也要重做；不给批次时这四行一律待做。V05 是 Windows 回归，收据在 Windows 回归报告里，不用 `mac-ledger receipt`。发布用的收据、waiver 与下载路径记录绑定候选 receipt 的摘要，
  见 #macos-readiness-check；本节的复用不读它们。
- **R03 的输入 ZIP**（USER 2026-11-24，[M5 报告](../report/2026-11-24-m5-macos-readiness-check.md)“生产者改动”节）：`mac-gui --zip <ZIP>`
  让 R03 直接使用给出的 ZIP，不从 `--app` 打包。运行任何一行之前，驱动先把 ZIP 复制成一份私有副本，摘要、成员清单与解包都取自这份副本：
  成员须恰为发布清单（#macos-release-artifact），且只是普通文件与文件夹、展开后不超过 256 MiB，两者都在解包之前由 `tar -tvf` 核对；
  解出的每一项也须是普通文件或文件夹（不跟随链接），其中 `StickyMD/StickyMD.app` 的 bundle 清单与可执行文件两项 SHA-256 须都等于 `--app`；
  任一不符即拒绝整次运行（不写成 R03 的 FAIL）。R03 运行时把 ZIP 复制进下载目录，复制件的 SHA-256 须等于检查时的值，成员再按上面核对一次；
  隔离标记先加在这份复制件（本次创建的普通文件）上，解开、确认解出的根是文件夹且整棵树只有文件与文件夹之后，再以 `xattr -w -r -s` 加在解出的
  整个文件夹上；解出的 bundle 再按两项摘要核对一次，进程结束后还要再核对一次。
  “不写文件”（USER 2026-11-24 选定“A：kqueue 观测”，[审查修复报告](../report/2026-11-24-m5-producers-review-fixes.md)“需要 Develata 决定”，#24 第二轮
  审查 B9）由两部分组成，被观察的是 `.app` 所在文件夹（便携版的 `note/` 会写在那里）：
  - 写入观测：打开产品之前，驱动对这个文件夹及其中的每个文件与文件夹（不跟随链接，只能是普通文件与文件夹）以 `O_EVTONLY | O_NOFOLLOW` 打开，
    在一个 kqueue 上逐个登记 `EVFILT_VNODE`（`EV_ADD | EV_CLEAR | EV_RECEIPT`，逐个核对登记结果）；然后取运行前的快照，快照的条目须恰为登记的条目。
    本次的 StickyMD 进程都结束之后、取运行后的快照之前，以零超时反复读出 kqueue 直到为空。计为写入的是写（`NOTE_WRITE`）、扩展（`NOTE_EXTEND`）、
    链接数变化（`NOTE_LINK`）、改名（`NOTE_RENAME`）与删除（`NOTE_DELETE`）：改动文件内容（含以相同字节重写）、截断、在任一文件夹中建立、删除或
    改名一项（含建了又删）、增减硬链接，都至少使一个登记的条目带上其中之一（本机 APFS 上逐项测得，见 `native/vnodes.rs` 的测试）。属性变化
    （`NOTE_ATTRIB`：权限、扩展属性与时间）不计入写文件，只记下哪些条目有，因为 Gatekeeper 自己会改 bundle 的隔离属性。登记、读出失败是 R03 的错误
    （见下）。不证明的前提：经共享内存映射写入、在同步（`msync`）之前又复原的内容不产生事件，快照也看不到（本机测得）；USER 2026-11-24 接受它为前提
    （原话“接受作为前提（推荐）”）。
  - 快照：运行前后的递归快照，每个文件的大小、SHA-256 与修改时间，每个文件夹的修改时间，每个链接的目标。
  判定：快照不同为 FAIL；有条目带计为写入的标志为 FAIL（kqueue 不说明是谁写的；虚拟机中别无他事，驱动在两次快照之间只读）；观察到的写入不论产品
  自行退出还是由驱动结束都为 FAIL（第三次复审）。没有读出观测、登记的条目与运行前的快照不符、没有看到写入但产品没有自行退出、或有条目被撤销
  （`NOTE_REVOKE`，观测中断）为 `NOT TESTED`；其余为 PASS。这一项的检查名为
  “nothing written in the folder holding the package (kqueue and snapshots)”，只比快照的旧驱动的检查表因此不能登记为 PASS。
  对照坏样本：`mac-gui --r03-write-restore`（校准，只随 R03）在登记与运行前的快照之后，以相同字节重写 `StickyMD.app/Contents/Info.plist` 并恢复其
  修改时间：快照看不出，写入观测使这一项为 FAIL。这样的证据带 `"calibration":"r03-write-restore"`，而且从不标 `finished`（修复轮 B1：不认识这个字段的旧版 `record` 也因未跑完而拒绝），
  `record` 拒绝登记。
  R03 的证据在这一行中另记它的观察事实 `facts`：ZIP 摘要、隔离属性、进程路径、挂载行、提示窗口、stderr、回答与退出、退出记录、两次快照、登记的
  条目与每个条目读出的原始标志。
  R03 的任何错误（会话目录、打开、观察、附件、清理；提示窗口的截图取不到或存不下，或看到提示却没有 `prompt.ppm`，修复轮 B2；读不出 stderr）
  都不是产品结果：另一个 StickyMD 正在运行、下载目录不能从被测字节布置、LaunchServices 打不开、
  查询本身失败（`pgrep`、`ps`、`xattr`、窗口列表、系统日志的读取或保存；`ps` 查不到的 PID 只在 `kill -0` 答“没有这个进程”时才算已退出）、
  R03 的进程或随机化挂载在它之后仍在（清理时重读挂载表；进程查询、写入观测的读出等错误都先放着，等进程与挂载核对完再返回，修复轮 5）、或 bundle 在运行中变了，这一行都记为 `NOT TESTED`，
  整次运行停下，证据不标 `finished`，因此不能登记（第三次复审 B4）。隔离属性不存在是观察（预设一项为 `NOT TESTED`），`xattr` 失败则是错误。
  `--out` 见下文“各生产者只写自己的行”的输出规则；附件与检查点都独占创建，迹中记下每个附件的 SHA-256。证据（仍为 schema 2）
  在顶层记 `zip_sha256`，R03 的检查表这时以“opened the candidate ZIP as given”开头，观察为 `sha256=<摘要>`。`record` 只把 ZIP 摘要记进 R03
  那条自动化记录的可选字段 `zip_sha256`，证据有 ZIP 摘要而没有 R03 行即拒绝；没有这个字段的旧记录照常读取。给了 `--zip` 而本次不跑 R03 时拒绝。
  不给 `--zip` 时探针照旧从 `.app` 打包（Phase 16 的批次没有候选 ZIP），证据与记录都不含 ZIP 摘要，发布门不计（#macos-readiness-check）。
- **各生产者只写自己的行**（USER 2026-11-24 的 Resolution 之后，审查修复）：没有 `tool` 成员的证据是 `mac-gui` 或 `mac-logout` 的：
  `mac-logout` 的证据在环境中记有 `case`，只能登记那一种情况的注销行；`mac-gui` 的证据不记 `case`，只能登记 `mac-gui` 运行的行。
  `"tool":"mac-resources"` 的证据只能登记 R01、R02，且不得带 `zip_sha256`；其他 `tool` 拒绝。账本里 R01、R02 的自动化记录以独立的种类
  `resources` 写出，读账本时种类与行不符的记录（例如旧版导入器写成 `automated` 的资源行）使账本被拒。R03 的 PASS 必须带它的观察事实
  `facts`：`record` 以与驱动相同的判定从这些事实重算 R03 的检查表，与证据写下的检查名、结论与观察逐项相同才登记（带 `facts` 的 R03 行不论结论都这样
  核对；#24 第二轮审查 S4）。重算出的 PASS 恰好带六项检查（隔离标记预设、随机化启动、提示、回答后退出、退出码 1、不写文件），给了 ZIP 时最前面是
  打开该 ZIP 的检查，其中的摘要等于证据顶层的 `zip_sha256`，事实中的 ZIP 摘要也须等于它。事实中有写入观测时，这条记录另记 `"write_watch":true`
  （只写为 true，只随 ZIP 摘要）；发布门只认带它的 R03 PASS（第三次复审 B1，#macos-readiness-check）。没有 `facts` 的 R03 行（比写入观测更早的
  驱动）不登记 PASS；它的 FAIL 或 NOT_TESTED 只在恰好带 R03 的检查（今天的名字，或只比快照的驱动的“nothing written beside the package”）时登记，
  只带 `execution` 一项的 R03 行（机器的错误，第三次复审 B4）不登记。
  `--image` 须为镜像 digest（`sha256:<64 位十六进制>`，可带 `<名称>@` 前缀，只有一个 `@`；`repo@sha256:A@sha256:B` 这样带两个摘要的不是引用，修复轮 B3）。账本按内容摘要 `sha256:<十六进制>`（小写）分组，名称与大小写
  不影响身份：同一摘要的两种写法属于同一组，账本保留写入时的字节。旧行用同一语法读取（第三次复审 B5）：不合这一语法的镜像（例如旧版 CLI 收下的
  标签，或只是结尾像 digest 的字符串）保留原字节，身份不明；发布门遇到候选字节上身份不明的记录时，这一行没有干净的组（`NOT TESTED`），它的 FAIL
  照样算。
  **输出目录**（第三次复审 B2）：三个工具的 `--out` 都不得落在被测 bundle、便携版的便签目录、安装版的 Note Root（`~/Library/Application Support/StickyMD/`
  下任一键，`05` #note-root）或 `mac-gui` 账本目录里；两侧都先解析现存祖先中的链接，并不区分大小写地比较。第一次运行时创建 `--out`。`--out` 里已有
  本工具可能写的任一文件名（不区分大小写）即拒绝：`mac-gui` 为 `mac-gui.json`、`F01.ppm`、`<行>-stderr.log` 与 `R03-*`；`mac-logout` 为
  `mac-gui.json` 与 `W12-logout-*`；`mac-resources` 为 `mac-resources.json`。`--zip` 落在 `--out` 里也拒绝（按解析后的真实位置比较，经 `..` 的相对路径照常可用，修复轮 8）。之后每个文件独占创建，只有本次的
  检查点会被替换。不证明的前提（核对的威胁模型，#macos-readiness-check）：保护安装版 Note Root 用的是 `$HOME`，产品用当前用户的家目录；
  刻意改动 `$HOME`，或把账本目录链接到别的名字，不在防护之内。
  **读器版本**（第三次复审）：新的读器能读旧行；但第一条 `kind:"resources"` 记录写入之后，本 PR 之前的读器会因不认识这一种类而拒绝整本账本，
  所以先在所有用这本账本的地方升级到本 PR 之后的 `stickymd-smoke`，再登记 `mac-resources` 的运行。
- **资源运行进账本**（USER 2026-11-24，同上）：`mac-resources` 的证据升为 schema 6（schema 5 起带 `"tool":"mac-resources"`；schema 6 每次运行三次采样，见 R02），带 `"tool":"mac-resources"`，另记 bundle 清单 SHA-256、
  产品输入指纹（bundle 有时）、驱动的 SHA-256、`sw_vers` 原文、`kern.hv_vmm_present`、活动显示器的个数与 `finished`（最后一次运行之后才写 `true`；
  检查点与因清理失败而中止的运行写 `false`）。`record` 接受它：与 `mac-gui` 证据一样只登记在虚拟机中跑完的运行，身份取同样的四项（系统同样由
  `sw_vers` 得出，所以一个镜像 digest 仍只对应一个系统），bundle 清单与可执行文件两项摘要都记下；场景、内存门、预热与测量时长须等于当前工具的设定，
  测量的笔记须是工具的固定笔记（摘要、20480 字节、20 个公式），schema 5 及更早的证据不能登记。环境须自洽：`sw_vers` 的版本与构建号等于内核报告的，
  版本只含数字与点，缩放等于显示器的像素除以点数；工具在每次采样时核对主显示器的模式（模式标识、刷新率与大小）、主显示器的 ID 和活动显示器的集合（读取时留有余量，可能被截断的列表不用）都未变，
  变了这次运行无效（第三次复审）。就绪与 30 s 期限由同一次观察决定：30 s 之后才看到的窗口是超时，不是一次有效运行。`record` 从各次运行的原始事实重新得出
  P16-R01 与 P16-R02 两行，与证据写下的结论及原因不符即拒绝；场景与整份证据的状态字段不作依据。已标 `finished` 的证据须有全部四个场景、每个场景
  恰有 `repetitions`（至少 5）次运行，运行状态须是工具会写的值之一，含有清理错误即拒绝（工具在第一次清理失败时就停下）。每个场景的运行须依次是
  第 1 至第 n 次。有效运行指状态为 `VALID`、没有清理错误、恰有三次原始采样（第一次、59.9 s 之后的中间一次、60 s 之后的最后一次；中间一次的时刻与计数都在另两次之间），
  且由第一次与最后一次采样和 Mach 时基重新算出的读数与证据写下的读数完全相同的运行；
  它还须按驱动的顺序发生：有进程、没有退出记录、30 s 内就绪，Hidden 在就绪之后经菜单隐藏，第一次采样在就绪（Hidden 为隐藏）之后至少 30 s 的预热之后；
  `VALID` 却不满足这些的运行使整份证据被拒。测量窗口须在 60 s 到 60.5 s 之间（`10` 的 60 s 平均；2026-11-10 至 11-14 的虚拟机运行最多晚 20 ms），
  超出的运行无效。每次运行启动前，复制出的 bundle 的清单与产品输入指纹须等于证据记下的值。按 `10`，测量环境是 macOS 27，内存在显示缩放 2 下测；
  不是 macOS 27.x 时两行都为 NOT_TESTED，缩放不是 2 或活动显示器不止一个时 R01 为 NOT_TESTED。
  各场景是否真的以所配置的视图打开，程序尚不能确认（主窗口没有辅助功能元素）。在 USER 定下确认办法之前（[审查修复报告](../report/2026-11-24-m5-producers-review-fixes.md)
  “需要 Develata 决定”），R01、R02 都不判 PASS：运行完整且读数都在门内时为 NOT_TESTED，原因写明视图未确认；FAIL 照常。`record` 比较证据写下的每行结论时
  同时比较原因，所以同为 NOT_TESTED 而原因不同的结论也会被拒。
  - R01：每个场景恰有 `repetitions`（至少 5）次运行且全部有效、每次的 `phys_footprint` 都不超过 `10` #macos-gates 的临时硬门为 PASS（视图确认之前为
    NOT_TESTED，见上）；
    任一有效运行超过硬门为 FAIL；其余为 NOT_TESTED。
  - R02（USER 2026-11-24 批准）：每个场景取有效运行 60 s 空闲 CPU 的中位数，对照 `10` 目标表的硬失败条件“> 0.1%”。四个场景都完整（同 R01）
    且中位数都不超过 0.1% 为 PASS（视图确认之前为 NOT_TESTED，见上）；完整的场景其中位数被证明超过 0.1% 为 FAIL（见下）；其余为 NOT_TESTED。取中位数沿用这一行以往的判定（按周期开关光标
    图层时 Split 中位 0.103% 判为超过硬门）；替代是取最大值，更严，但单次读数即可使整行失败。唤醒次数仍只作观察。
    窗口可能晚到至多 0.5 s（#24 第二轮审查 B8），而只看窗口两端说不出 CPU 花在哪一段（第三次复审 B3 的反例：60.5 s 窗口两端各 31 ms，平均 0.1025%，
    任一 60 s 窗口至多 0.052%）。所以每次运行在第一次采样之后 59.9 s 再采一次（USER 2026-11-24 选定“A：第三次采样”，[审查修复报告](../report/2026-11-24-m5-producers-review-fixes.md)
    “需要 Develata 决定：R02 的 60 s 窗口”）：第一次到最后一次之间的 CPU 时间不少于前 60 s 窗口的（上界），第一次到中间一次的不多于它的（下界，中间
    一次落在 60 s 之内时；窗口恰为 60 s 时两者相同）。成立看上界对 60 ms（60 s 的 0.1%）预算的中位数，FAIL 看下界的中位数超过 60 ms，且场景完整
    （五次以上全部有效）；都以原始计数与 Mach 时基精确比较（第三次复审）。FAIL 的原因写出下界的中位数（修复轮 7）。两者之间（下界不超过、上界超过）这一场景无法判定，为 NOT_TESTED，须重测；
    剩下的不确定只来自中间一次与最后一次之间的约 0.1–0.5 s。
  - V03 不由 `mac-resources` 判定：它要求同一批次中基线与新包交替运行 `mac-latency`，呈现器计数只在 `mac-latency` 的证据中，而
    `mac-latency` 是诊断，不进账本，也没有发布用的判定规则。
- **自动化行**：验收表中 Mode 为 `Automated`、由 `mac-gui` 执行的行，只认账本里与当前产品输入指纹、驱动、镜像、系统四项一致的 PASS，不需要人工收据。P16-W12 的真实注销会结束图形会话，由 `mac-logout` 经 SSH 执行，未保存文本、导出对话框打开、保存失败三种情况各在一次新开机的虚拟机中运行，分别记为账本行 `W12-logout-text`、`W12-logout-dialog`、`W12-logout-fault`（USER 2026-11-19）；这三行与 `mac-gui` 的 W12 在同一身份下都有可复用的 PASS 时，W12 才算通过。指纹变了，这些行要在新身份下重新运行（`plan` 会列出）；还没有这样的 PASS 时状态为 `NOT TESTED`。
- **验收阶梯**（USER 批准 2026-11-19，[提案](../report/2026-11-19-macos-acceptance-ladder.md)）：每一行（必要时拆为子项）用下列第一个可行的方式判定。可行指它的检查能拒绝事先准备的坏样本；做不到的行留在 `Manual` / `NOT TESTED`，不硬转。
  1. 程序断言（`Automated`）；
  2. agent 在虚拟机里自行操作，依据辅助功能树、日志、文件与进程状态判定（`Agent`）；
  3. agent 判定程序截下的图（`Agent`）；
  4. 程序截图，USER 判定（`Review`）；
  5. 程序录屏，USER 判定效果（`Review`）；
  6. USER 亲手操作并判定（`Manual`）。
  整行要求每个必需子项都在各自级别上通过；任一子项 FAIL 即 FAIL，缺证据为 `NOT TESTED`。改一行的 Mode 仍按行经 USER 批准。
- **Agent 行**：两名判定者独立判定，一名是不带父会话历史的同模型 subagent，一名是 codex；互不可见，不告知对照与正确答案，对照顺序打乱。每名判定者对每个子项给出 PASS、FAIL 或 NOT_TESTED（缺证据或无法辨认）。子项结论：两者都 PASS 为 PASS，两者都 FAIL 为 FAIL；证据本身缺失（截图或录像没有取得）为 `NOT TESTED`，重跑后再判；其余组合都升级为第 4 级，交 USER 看同一组画面，子项在 USER 的 Review 收据之前为 `NOT TESTED`，USER 的结论为准。整行的 Mode 不因升级而改变：仍为 `Agent` 的行，当每个子项都有两名 agent 的 PASS 或 USER 对该子项的 Review 收据 PASS 时为 `AGENT PASS`，矩阵同时引用这些判定与收据；有子项 FAIL 时为 `BLOCKED`；其余为 `NOT TESTED`。账本记录判定之前，`Agent` 行不能为 `AGENT PASS`。每条判定标准启用前先用已知坏样本校准。判定记录引用同一次运行的证据摘要、行与子项、所用画面的角色与哈希、判定标准的版本、对照、判定者与实际模型，以及虚拟机中 agent 工具的 SHA-256；判定标准、对照或判定者策略一变，旧判定不再复用。
- **撤销**：USER 抽查推翻某个 PASS 或判定时，`revoke` 追加一条撤销记录。撤销自动化 PASS（按证据摘要指名）之后，这一行在同一身份下（I01–I03 与 R03 限同一批次）不再复用任何 PASS（见上条）。撤销人工收据（按 app 的产品输入指纹指名）作用于这一行在该指纹下此前登记的全部收据，不分批次；之后 USER 重新确认而登记的收据有效（收据由 USER 当场登记，账本中的先后就是确认的先后）。撤销只能指向账本里已有的结果（读账本时同样校验）。验收表引用了被撤销的结果时，在同一提交把该行改为 `NOT TESTED`。治理检查只读验收表文字，不读本机账本（CI 没有账本），所以这一同步由提交者负责，`revoke` 成功后会提示；同理，I01–I03 与 R03 是否已在本批完成，由 `plan --batch` 回答，Phase 16 的就绪检查不判断。
- **虚拟机中的 agent 工具**：第 2、3 级的操作与截图在 `mac-test` 中进行，不在 USER 桌面上运行；工具以单个可执行文件复制进虚拟机，不在虚拟机里安装运行时。
- **范围**：不按产品模块复用，这一级留待 Windows RC3-B 解决之后。这一节不改变 Phase 16 不是资格化 suite 的性质。

<a id="phase-17-copy-and-notices"></a>
### Phase 17：复制语义、复制侧条、提示条与文末留白（下一版本）

- **依据：** [提议报告](../report/2026-11-14-copy-semantics-rail-notices.md)，USER 批准 2026-11-14。
- **时机：** 设计于 macOS 版本发布之后合入；代码在下一版本实现。在此之前，任何发布版本都不含这些能力。
- **验收：** 见 `docs/acceptance-cases/phase-17.md`，未运行的项保持 `NOT TESTED`。
- **自动化：** 由 Rust CLI 持有；首次实施时建立 `tools/smoke/phase-17.ps1`。

| 里程碑 | 交付 | 出口 |
| --- | --- | --- |
| M1 | 提示条（`04` #notice-state、`09` #notices）：定时提示栈与派生层、失败不丢弃、故障展示与标记、Shared Desktop 浮层、例行自动保存静默、按可用选项给按钮 | 每个类别的时长与入栈测试；到期回执按 id 校验；派生层在定时提示到期后恢复；“保留本地”写入失败时故障原因与冲突按钮同时可见；现有发出点按 `09` 归类规则归类 |
| M2 | Preview 文末留白（`09` #vertical-scrollbars） | 绘制、滚轮、滑块与同步共用一个末端函数的测试；短文阈值两侧的行为 |
| M3 | Preview 复制语义第一期（`06` #preview-copy-semantics） | 规则 1–6 的单元测试与两条不变式（每个字节取自该帧源码快照并保持顺序；对象内容取自对象文本）；随机选区测试；位置上移段落、转义与实体、列表序号、引用内代码、表格、陈旧帧 |
| M4 | 复制侧条（`06` #preview-copy-rail） | 命中、阈值、降级与可见区间测试；侧条宽度在视觉验收后固定 |

- **每个里程碑的共同出口：**
  - Preview 帧时间、Source 输入延迟与内存基准不劣于改动前（`10`）；
  - macOS 在 `mac-test` 虚拟机中做视觉验收，Windows 宿主另测。
- **落地时同步更新：** 落地时取代的现行条款、验收行与测试，列在 `phase-17.md` 的“落地时更新”一节。

<a id="phase-18-macos-distribution"></a>
### Phase 18：macOS 安装版与公开分发（下一版本）

- **依据：** [分发报告（修订版）](../report/2026-11-19-macos-public-distribution-decision-r2.md)与[安装版报告（修订版）](../report/2026-11-19-macos-installed-note-location-r2.md)
  末尾的各 Resolution，USER 批准 2026-11-19。两份初版已被取代。
- **时机：** 与 Phase 17 同属下一版本。安装版必须在第一个公开的 macOS 包之前完成。
- **验收：** 见 `docs/acceptance-cases/phase-18.md`；自动化由 Rust CLI 持有，`tools/smoke/phase-18.ps1` 是稳定入口。与 Phase 16 一样，Phase 18 不进入
  Windows 的全阶段 readiness，只在点名时判定。
- **每版的 macOS 发布证据**（候选收据、虚拟机运行、R03、下载路径、USER waiver）记在该版的候选报告与 ignored `dist/evidence/` 下，不写进验收矩阵：
  `docs/acceptance-cases/` 下的任何改动都会使 Windows 全部资格化模块的收据失效（#module-success-ledger）。矩阵只记录 Phase 18 实现本身的验收。

| 里程碑 | 交付 | 出口 |
| --- | --- | --- |
| M1 | 合同：`01` Note Root 与发布术语、`02` #portable-and-installed、`04`、`05` #note-root、`08`、`09`、本节与 Release 合同；投影与矩阵 | `phase 00` 通过 |
| M2 | 安装版：位置判定纯函数、键、家目录查询、Note Root 句柄链与加锁、启动阶段与运行期共用一个 `note/` 句柄、提示文字 | 矩阵 L 系列的单元、存储与实例测试；Windows 行为不变 |
| M3 | 发布流水线：`release-macos` workflow、darwin 依赖图的第三方声明、macOS SBOM 与 checksum、按 artifact id 的候选收据与本地晋升、晋升 workflow 的完整附件比对、Platform Build Input Fingerprint 与沿用规则 | 矩阵 P 系列的工具测试；两个 workflow 都能 dispatch（实际 dispatch 由 USER 授权） |
| M4 | 验证 | 本机基线；`mac-test` 中的 `mac-gui`；候选包上的下载路径（矩阵 D 系列） |
| M5 | macOS 就绪核对：按 #macos-release-gate 逐行核对候选收据、账本、waiver 收据与 R 上的源码测试收据 | 坏样本能判出失败；核对程序与打包 crate 的拆分在冻结 R 之前完成，正式收据在实际的 R 上运行后产生 |

<a id="macos-release-gate"></a>
#### macOS 发布门

| 类别 | 判定 | 证据身份 | 可否 waiver |
| --- | --- | --- | --- |
| 包完整性：各文件哈希、签名完整（`codesign --verify --strict`）与签名方式确为 ad-hoc（`codesign -dv` 观察到 `Signature=adhoc`，且没有 Team ID）两项分别检查、仅 arm64、Info.plist lint、ZIP 成员白名单、README 写明 macOS 27+ 与 Apple Silicon、SBOM、第三方声明 | `release-macos` workflow 与本地晋升 | macOS 候选收据（签名方式按观察结果记录） | 不可 |
| 同键互斥：同一个键的两个安装副本互斥（`05` #note-root 第 9 条），大小写敏感与不敏感两种卷都有结果 | 实例测试，在 R 上运行；大小写敏感卷的测试标为 `#[ignore]`（要挂载磁盘映像），须显式运行（P18-L08） | 源码身份 R，两种卷各自的结果 | 不可 |
| R03：最终 ZIP 实际触发随机化后提示退出，所观察的目录不写文件；没有触发随机化为 `NOT TESTED` | R03 探针直接接收并核对候选 ZIP（不从 `.app` 重新打包）；验收阶梯获批后由程序判定 | 本次发布的 ZIP | 不可 |
| 真实下载路径：浏览器下载、拖进“应用程序”或移动文件夹、访达首开、保存、退出后重开（安装版报告第 8 节） | 在第一个公开包的候选上做一次；签名方式或 macOS 大版本改变时重做（USER 未单独表态，按建议记为此方案）。系统拦下时才点“仍要打开”，并记录拦截与否 | 做那次的 ZIP | 不可（需要做的那一版） |
| Phase 16 中依赖 app 运行的行 | 各自的工具（`mac-gui`、`mac-resources`、`mac-latency`），或获批的阶梯办法；否则 USER 收据 | 本次发布的 bundle | 逐行 USER waiver |
| Phase 16 中不依赖发布字节的行（如 S01 存储测试） | 源码测试 | 源码身份 R | 按现行规则 |
| R 上的源码测试：在发布提交 R 上，以 `cargo test --locked` 显式选择四个共享 crate、`stickymd-posix`、`stickymd-mac`、`stickymd-package` 与 `stickymd-smoke`，不加测试名过滤，默认测试集（不含 `#[ignore]` 的测试）全部通过。`release-macos` 在构建来源 S 上跑的同组测试只是构建前自检（#platform-artifact-reuse），不能代替；标为 `#[ignore]` 的诊断与性能测试不在此行，需要它们的行（如上方同键互斥）各自要求 | macOS 主机上 R 的 checkout，开始与结束时 HEAD 都等于 R、工作树都干净 | 源码身份 R：收据（格式由 M5 定，见 #macos-readiness-check）记下 R、主机、完整命令、每个测试二进制执行与忽略的数目，以及开始和结束时的源码身份；身份不符、缺包或命令带过滤即无效 | 不可 |

- **按行判定，不看退出码。** `mac-gui` 有 `NOT TESTED` 行时仍成功退出，发布门逐行检查。
- **waiver 与收据分开。** 不可 waiver 的行必须 PASS。可 waiver 的行要么 PASS，要么保持 `NOT TESTED` 并有一份绑定本候选的 USER waiver 收据。
  缺行、`FAIL`、身份不符都不能形成 macOS 就绪。
- **绑定字节，不只绑定指纹。** 虚拟机运行证据中的 bundle 清单 SHA-256 与可执行文件 SHA-256，必须分别等于候选 ZIP 解出的那份 bundle；
  账本里同指纹、不同字节的 PASS，在发布门中不算数。只记了可执行文件摘要的人工记录不足以证明 bundle；发布用的人工收据绑定候选 receipt 的摘要，
  或同时绑定这两项字节身份。M5 的核对要拒绝“从 `.app` 重新打包的 ZIP”与“可执行文件相同、bundle 不同”两种坏样本。按 #platform-artifact-reuse
  沿用的字节不变，绑定这份字节的证据仍按其自身规则判定（见该节“沿用后”）。
- **阶梯办法的前提：** 验收阶梯中的程序与 agent 判定，要先用坏样本证明能判出失败，才能用于发布门。
- **门不过，整个版本不发布。** 获准的附件集合只有“Windows + macOS”一种（#release-attachment-set）；不使用整版例外。
  若 macOS 暂不可用，是否放宽这一条由 USER 另行决定。
- macOS 就绪与 Windows readiness 是两份独立判定，发布需要两者都成立；Windows readiness 的判定标准、附件与晋升输入不因 macOS 改变。

<a id="macos-readiness-check"></a>
##### 就绪核对与 R 上的源码测试收据（M5）

状态：源码测试收据的格式按上表“格式由 M5 定”的授权定下。USER 2026-11-24 批准（[M5 报告](../report/2026-11-24-m5-macos-readiness-check.md)
末尾的 Resolution）M5 实现时的四项提议——Phase 16 行的分类（W15、W16、V05 不进 macOS 门，P01 由包完整性一类承担）、账本判定（某一身份下的
FAIL 不妨碍另一身份的干净运行）、同键互斥的结果并入源码测试收据、核对程序只在 R 的干净检出中运行——以及下文的发布用 USER 记录。USER 同日
（经发起会话转达，M5 报告“第二次复审与 USER 决定”）另决定：(a) P18-L08 与 P16-S06 的显式运行并入源码测试收据；(b) P18-D04、D05 记下观察到的结果，
`macos-ready` 必须给出 `--macos-major`；(c) 发布用收据与 waiver 必须给出所检查候选的摘要；(d) 有效的 Cargo target runner 写成运行器不证明的前提，
并拒绝环境中的明显入口。USER 同日（同样经发起会话转达）又决定：`mac-gui` 行只认 R 的驱动（`--driver`，第二次复审 SHOULD_FIX 1 的方案 A），
并批准下文“核对取自何处”一条的原文。USER 同日又定下核对的威胁模型（第三次复审；原文见“核对取自何处”的不证明的前提）。
R03 与资源行的生产者改动按同一 Resolution 另开分支落地（#phase-16-macos-host 的“R03 的输入 ZIP”与“资源运行进账本”）。USER 2026-11-24 批准
（[M5 报告](../report/2026-11-24-m5-macos-readiness-check.md)“生产者改动”节的 Resolution）实现时的三项提议：R02 取各场景中位数；V03 在 USER 定下
发布用的规则之前不由程序判定，每版可 waiver；R03 的 ZIP 不参与分组。

- **一般规则：** 证据尚未定义的行没有通往 PASS 或 `WAIVED` 的路径；判定只读记录下的事实；已记下的 `FAIL` 先于 waiver 与收据，waiver 只代替没有取得
  的证据。
- **源码身份**（运行器与核对程序共用）：HEAD 按提交读取（禁用替换对象与 graft，与 #platform-artifact-reuse 的 Git 视图相同）；
  `git --no-optional-locks status --porcelain --untracked-files=normal` 为空；索引中没有 assume-unchanged 或 skip-worktree 标志（`git ls-files -v` 的
  小写或 `S`，它们使干净的状态不能证明什么）；环境中没有把 Git 指向别的仓库、工作树、索引或对象库的变量（`GIT_DIR`、`GIT_WORK_TREE`、`GIT_INDEX_FILE`、
  `GIT_OBJECT_DIRECTORY`、`GIT_ALTERNATE_OBJECT_DIRECTORIES`、`GIT_COMMON_DIR`），且 `git rev-parse --show-toplevel` 规范化后就是运行所在的根。
- **源码测试的运行：** `stickymd-smoke release macos-source-tests --release-source <R>` 在 macOS 主机上依次：
  1. 核对源码身份为干净的 R，环境中没有 `CARGO_TARGET_*_RUNNER`、`CARGO_BUILD_RUSTC_WRAPPER`、`RUSTC_WRAPPER` 与两个 workspace wrapper 变量（决定 (d)），
     不符即不运行、不写收据，旧收据不动；
  2. 取得运行锁（`dist/evidence/.release-source-tests-macos.lock`，另一次运行或正在读收据的核对持有时拒绝；锁文件尚不存在时，先删除运行记录并同步
     目录再建锁，所以没有锁时留下的旧运行记录与收据不会因为新建的锁而算数，即使这次运行在记下自身之前停止，修复轮 BLOCKER 2）；收据路径上若有文件而它不是源码测试收据
     （`kind` 不是 `macos-release-source-tests`），保留它并失败退出；原子写本次运行的身份（开始时间与进程号）到运行记录
     `dist/evidence/.release-source-tests-macos.run`，再删除旧收据并同步其目录。核对只认名字等于运行记录中身份的收据，所以此后的任何失败，包括删除旧收据
     之前的中断，都不会留下更早的 PASS（第四次复审 BLOCKER 2）；
  3. 以同一组包的 `cargo test --locked … --no-run --message-format=json` 求出测试二进制，以 `cargo metadata --no-deps --locked` 求出做文档测试的库；
     对每个二进制运行 `--list --format terse` 与 `--list --ignored --format terse`，对每个库运行 `cargo test --locked -p <包> --doc -- --list …`，得到各目标
     自己的测试清单；
  4. 运行规定命令 `cargo test --locked -p stickymd-core -p stickymd-render -p stickymd-app -p stickymd-desktop -p stickymd-posix -p stickymd-mac -p stickymd-package -p stickymd-smoke`
     （与 `release-macos` 的 `self-check` 那一步逐字相同，测试核对两者一致），stdout 与 stderr 合为一个管道按序解析，每个测试的 `test <名> ... <结果>`
     行与该目标的清单逐个合并；
  5. 依次显式运行 `cargo test --locked -p stickymd-mac --bin stickymd-mac -- --exact --include-ignored <测试名>`：P18-L08 的两个测试与 P16-S06 的 exFAT 测试
     （决定 (a)）。P18-L08 那次运行把 `STICKYMD_VOLUME_REPORT` 设为本次运行自己的临时目录中的一个文件：默认集中的测试在它断言所用的临时目录上、映像测试在
     挂载点上各自探测大小写，追加一行 `temporary <结果>` 或 `image <结果>`（结果为 `case-insensitive` 或 `case-sensitive`）；运行器要求每个卷恰好一行，
     否则记入 `problems`。默认集中的测试在大小写敏感的卷上仍跳过只差大小写的检查，所以卷由测试自己测得，而不由运行器的临时目录推断（第三次复审 B4）；
  6. 在 R 上构建 `mac-gui` 驱动：`CARGO_PROFILE_RELEASE_PANIC=unwind cargo build --locked --release -p stickymd-smoke --target-dir target/macos-driver --message-format=json`，
     构建前清空 `target/macos-driver`，所以别的提交留下的构建缓存不能代替 R 的构建（修复轮 BLOCKER 1）；记下 Cargo 所报的那个 `stickymd-smoke`
     二进制的 SHA-256（第四次复审 BLOCKER 1）。字节取决于构建目录的路径（链接器的 UUID），目录固定，所以同一检出上重跑得到同一个驱动；虚拟机中的
     `mac-gui` 运行用的就是这个文件。构建失败时记下原因，不影响源码测试的结论；
  7. 再次读取源码身份，原子写 `dist/evidence/release-source-tests-macos.json` 后释放运行锁。
  运行器只为输出设 `CARGO_TERM_COLOR=never`。每个进程记下它怎样结束：退出码、被信号结束、没有启动、或启动后不能跟到结束（读管道出错时先结束子进程再
  等待它）。主运行已得出结果之后，P18-L08 卷报告的临时目录建不起来只记入 `problems`，已取得的结果照常写入收据（第四次复审）。
- **收据（schema 3；schema 1 与 2 不迁移）：** `release_source`；`run`（本次运行的身份，与运行记录相同）；`host`（`sw_vers` 的产品名、版本与构建号，`uname -m`，`kern.hv_vmm_present`，`cargo -V`，
  `rustc -V`）；`start` 与 `end` 各为 `{head, clean}`；`command` 与 `exit`（退出码为数，否则为 `signal`、`not-started` 或 `unknown`）；`expected`（发现的测试二进制与文档测试，以包、种类与名字标识，不记路径）；
  `binaries`（每一项：目标、libtest 的 passed、failed、ignored、measured、filtered_out 与运行数，以及 `cases`：该目标清单中的每个测试的全名、是否列为
  ignored、运行打印的结果 `ok`/`failed`/`ignored`/`not-run`）；`explicit`（依次为 `same-key` 与 `P16-S06`，每项：门的条目、命令、测试名、该段归属的目标、
  计数、按名的结果、`listed`（规定的测试中，stickymd-mac 的二进制在任何运行之前自己的清单所列的那些）、`exit`，`same-key` 另记 `temporary_volume` 与
  `image_volume`，即测试测得的结果，没有测得时为 null，只有 `same-key` 可以不为 null，且只能是两个词之一）；`driver`（构建驱动的命令，与 `sha256`
  （64 位十六进制）或构建失败的 `error` 二者之一）；`problems`（运行器无法确定的事）。`run` 须是运行器自己的形式（纳秒与进程号，以 `-` 相连）；
  `binaries` 的 `counts` 在输出于汇总之前中断时为 null。整份严格解析，每个对象恰有这些字段（修复轮 SHOULD_FIX 9）。
- **判定**（依次）：
  1. 身份：`release_source` 等于给出的 R，`start` 与 `end` 都是干净的 R，`command` 与两条显式运行的命令、测试名逐字等于规定值，主机为 macOS、arm64；
     任一不符，源码测试行、同键互斥行与 S06 都为 `NOT TESTED`。
  2. 记下的失败：某项 failed 非 0、某个测试的结果为 `failed`，或进程以非 0 退出码或信号结束，任一单独成立，源码测试行即为 `FAIL`；显式运行同样。没有
     启动或不能跟到结束的进程没有结果，为 `NOT TESTED`，但它打印出的失败仍为 `FAIL`（第四次复审）。身份成立后，记下的失败先于下面的完整性：核对时
     列不出 R 的测试目标，收据记下的失败仍为 `FAIL`（第三次复审 S2）。
  3. 完整：`problems` 为空；`expected` 等于核对时在 R 上由 `cargo metadata --no-deps` 列出的目标（八个包中 `test` 为真的目标，构建脚本除外，加上做文档
     测试的库）；`binaries` 与之是同一集合且每项只出现一次；每项 filtered_out 为 0，清单中的每个测试都运行了一次，结果为 `ignored` 当且仅当清单列它为
     ignored，计数等于这些结果（不溢出、measured 为 0）。读输出时，libtest 的 `ok`/`FAILED` 与失败数矛盾、汇总行不完整（不以 `finished in <秒>s`
     结束）即为无法读取；段头打印的路径须逐个路径组件是 Cargo 构建出的可执行文件路径的结尾；清单、ignored 清单与运行结果中每个名字只出现一次，
     ignored 清单是清单的子集，运行打印的测试都在清单中，否则记入 `problems`，不并入第一项，同名结果中有 `FAILED` 时该测试为 `failed`（第三次复审 B3、
     S4；第四次复审）。测试名后 libtest 表示模式的后缀（` - compile fail`、` - compile`、` - should panic`）去掉后再与清单对照，结果行在第一个
     ` ... ` 处分开（忽略原因可以含它），一个 `Doc-tests` 段可以有 rustdoc 的多组 `running` 与汇总（合并的与单独的文档测试），计数相加；测试二进制
     只有一组。libtest 在汇总之前打印 `failures:` 与失败测试捕获的输出，其中的任何行（含 `running N tests`）都不当作控制行；输出在汇总之前中断（如超过
     运行器的大小上限）时，已读到的部分照样归属到目标与清单中的名字，已打印的 `FAILED` 仍为 `FAIL`，没有汇总的目标不能 PASS（修复轮 BLOCKER 3）。
     清单列为 ignored 的每个测试都须在核对程序的归类表中（按整个目标，即包、种类与名字，以及全名；第四次复审 BLOCKER 3，修复轮 BLOCKER 4），否则为
     `NOT TESTED`。
  4. 显式运行：该段归属 stickymd-mac 的二进制，运行的测试按名恰为规定的那些、都为 `ok`，计数恰为它们，`listed` 恰为它们（取自该二进制自己的清单，
     不依赖主运行是否跑到它：Cargo 在第一个失败的二进制之后停止，第四次复审；清单中出现两次的测试，`--exact` 只运行一次，为 `NOT TESTED`，修复轮
     BLOCKER 5）；`same-key`
     另须 `temporary_volume` 为 `case-insensitive`、`image_volume` 为 `case-sensitive`，两者各自单独决定。否则为 `NOT TESTED`。
- **核对程序：** `stickymd-smoke release macos-ready --release-source <R> --macos-major <N> --driver <R 上的 stickymd-smoke> [--batch <名>]` 只读，在 macOS 主机上运行：
  1. 源码身份须为干净的 R（门用 R 的工具判定，见 #platform-artifact-reuse“沿用后”），R 须包含在 `origin/main` 中；
  2. 读矩阵并核对分类表；在运行锁的共享锁下读源码测试收据（锁文件只读打开、不创建；运行进行中为 `NOT TESTED`），在账本写入锁的共享锁下读账本（与写入器
     和 `plan` 同样校验，含一个镜像只对应一个系统；账本、锁与收据的路径不得是符号链接，检查路径时除不存在外的任何错误都使核对停止），两把锁一直持到
     判定完成。收据或账本存在而其锁文件不存在时不读，连内容也不读：它不是在锁下写成的，收据为 `NOT TESTED`（在 R 上重跑），账本为错误（第三次复审
     B5）；收据只在运行记录存在、且收据的 `run` 等于其中的身份时才读入，否则为 `NOT TESTED`（第四次复审 BLOCKER 2）。R 的驱动是收据所记、在 R 上
     构建的那个：收据属于 R（判定 1 的身份）、构建命令为规定的命令、`--driver` 所给文件的 SHA-256 等于收据所记的，三者都成立时 `mac-gui` 行才认它的组，
     否则没有一组能使 `mac-gui` 行 PASS（第四次复审 BLOCKER 1）；
  3. 经 resolver 取得候选：除每个暂存文件与 receipt 相符外，暂存的 checksum 文件须恰好列出 receipt 中 ZIP 与 SBOM 的摘要，暂存的 ZIP 本身须通过包检查，
     其中的 bundle、可执行文件、产品输入指纹、签名方式与构建来源须等于 receipt，包检查算出的 ZIP 摘要须等于 receipt 中的，SBOM 须描述 ZIP 中的成员；
     另外重算构建来源上 `Cargo.lock` 的摘要；
  4. 在 R 上以 `cargo metadata` 列出测试目标；判定；
  5. 再次读取源码身份与候选的整个状态（不存在、不能解析、解析到哪个 receipt），与开始时不同即不打印任何结论并失败退出；否则逐行打印，全部成立才成功退出。没有强制通过的选项。
  判定是纯函数，坏样本测试在所有平台运行。
- **核对取自何处**（USER 2026-11-24 批准原文）： 核对时在 R 上重算的：源码身份；R 的测试目标；分类表与矩阵中的 Mode；候选的暂存字节、ZIP 内容、checksum 文件、
  SBOM 与 `Cargo.lock` 摘要。取自本地记录、核对时不能重现的：源码测试收据中每个测试的结果与主机事实（收据由 R 的运行器在 R 上写成，手工改写的收据
  不能被发现）、账本中的程序结果（由 `mac-ledger record` 从虚拟机运行的证据导入）与 USER 原话。不证明的前提：Cargo home 与上级目录中的 Cargo 配置
  （含 target runner，决定 (d)）、主机上工具链与 SDK 的内容（版本输出相同的替换不能发现）、断电后的持久性（删除旧收据时同步了目录；原子写入后的
  目录同步属于打包 crate 的写入器，未证明）。
  威胁模型（USER 2026-11-24 决定，原话“只防诚实操作者的失误”）：核对防的是诚实操作者的失误、过期／不完整／不一致的证据与产品回归；不防恶意的本地
  篡改、并发的替换后恢复、伪造的工具输出与有意重定向的环境。上面的锁、首尾复查与对 Git、Cargo 环境变量的拒绝只挡住这些情形中显而易见的入口，
  不是对它们的证明。
- **Phase 16 行的分类**是核对程序中的一张表，测试核对它恰好覆盖矩阵中的全部 P16 行，且每行的 Mode 恰为分类时的 Mode（Mode 一改，例如 Manual
  改为 Review，即须重新分类）：
  - 源码行，由 R 上的源码测试承担：B01、B02、S01–S05、S07–S15、W13、W14、W17、K01、C01–C04、V01、V02；
  - S06 的测试标为 `#[ignore]`（要挂载 exFAT 映像），凭收据中它的显式运行判定（决定 (a)）；八个包在 macOS 上被忽略的每个测试都在核对程序的一张归类
    表中（按包、目标的种类与名字、libtest 全名，及其作用；移到另一种类的同名目标也须重新归类：没有门的行依赖它，或由某个显式运行承担），判定时与收据中各目标 `--list --ignored` 列出的名字逐个比对，
    所以不论 `ignore` 写成什么形式（单独的属性、一行或多行的 `cfg_attr`、文档测试的 `ignore`）都被发现；新增或移动的忽略测试须先归类（第四次复审
    BLOCKER 3）；
  - `mac-gui` 行，账本中候选字节上的 PASS：W02–W04、W07–W12、K02、E01；W12 另要求三条注销行在同一组身份下 PASS；
  - 人工行，发布用 USER 收据或 waiver：W01、W05、W06、I01–I03、F01、V04；
  - 资源行：R01、R02 由账本中候选字节上的 `mac-resources` PASS 判定（账本判定同 `mac-gui` 行）；V03 见“尚无来源的行”；
  - R03 是门的单独一类；P01 由“包完整性”一类承担；W15、W16、V05 是 Windows 回归，不属于 macOS 发布门。
- **账本判定：** 只看可执行文件与 bundle 清单两项摘要都等于候选的记录，按驱动、镜像与系统分组；产品输入指纹不参与分组（字节相同则指纹相同，记录也
  可能没有它）。只有驱动的 SHA-256 等于 R 的驱动（`--driver` 所给、且为 R 上源码测试的运行所构建的那个，见核对程序第 2 步）的组能使一行 PASS（与
  `mac-ledger plan` 只为当前驱动复用 PASS 相同；USER 2026-11-24；第四次复审 BLOCKER 1），
  `FAIL` 不论驱动都算。一行所需的各行（W12 为它与三条注销行，其他行为它自己）先在每一组内一起判定：某一组中每一行都有 PASS 且没有 FAIL、NOT_TESTED 或被撤销
  的 PASS，这一行才 PASS（先后无法证明，与 #phase-16-macos-host 的复用规则相同）；没有这样的组而某一组中有一行失败时为 `FAIL`；其余为 `NOT TESTED`。
  任何为 `NOT TESTED` 的行，只要这一行或它所需的行在候选字节上（不论哪个身份）记下过 `FAIL`，即为 `FAIL`，waiver 不再考虑；人工行在收据之前同样先看
  这些 `FAIL`；R03 在本批中候选字节上记下的 `FAIL`，不论另一组是否干净，也是 `FAIL`（第三次复审 S1）。I01–I03 与 R03 只看 `--batch` 所指的批次，
  不给批次时不计。
  R03 另要求干净的组中有一次 PASS 的 `zip_sha256` 等于候选 ZIP 的 SHA-256，且记有 `"write_watch":true`（“不写文件”经写入观测判定，
  #phase-16-macos-host）；ZIP 不参与分组，组内经别的 ZIP（含从 `.app` 打包、没有 ZIP 摘要的运行）记下的 FAIL、NOT_TESTED 与被撤销的 PASS 同样使
  该组不干净；只有经别的 ZIP 或没有写入观测的 PASS 时为 `NOT TESTED`，所以账本里已有的、只比快照的驱动写下的 R03 PASS 不使门成立（第三次复审
  BLOCKER 1）。R03 不可 waiver。
- **发布用 USER 记录**（USER 2026-11-24）：记在账本 `dist/evidence/mac-gui-ledger/records.jsonl` 中，与其他记录同样只追加、在排他锁下原子发布。
  每条记录含候选 receipt 的 SHA-256（`candidate_receipt_sha256`；它绑定 ZIP 与 bundle 的两项字节身份）、记录所在的桌面、USER 的原话、可选的批次与时间。
  #phase-16-macos-host 的复用规则不读它们。
  - 收据与 waiver 必须以 `--candidate-sha256 <摘要>` 给出 USER 检查时的候选（决定 (c)）：摘要由 `stickymd-smoke release macos-candidate --release-source <R>`
    打印，USER 照抄，不手工计算；登记时 resolver 对 R 解析出的候选不是它时，什么都不记。
  - `mac-ledger receipt --release-source <R> --candidate-sha256 <摘要> --rows <ids> --quote <原话> [--batch <名>] [--system <文件>] [--outcome <结果>]`：
    Phase 16 的人工行记为 `release-receipt`（行的检查与指纹收据相同：Review、Agent 与 Windows 行拒绝）；P18-D01–D05 记为 `download`，须给 `--system`
    （走这条路径的系统的 `sw_vers`），并记下候选的签名方式与该系统；P18-D04 须以 `--outcome` 记下 `gatekeeper-blocked`、`translocated-prompt-exit` 或
    `portable-start`，P18-D05 记下 `blocked` 或 `not-blocked`，一次只记其中一行，其他行不得带结果（决定 (b)）。`--app` 与 `--release-source` 只能给一个。
  - `mac-ledger waive --release-source <R> --candidate-sha256 <摘要> --rows <ids> --quote <原话> [--batch <名>]`：记为 `release-waiver`，只接受可 waiver
    的行（`mac-gui` 行、人工行、资源行与 V03），拒绝 R03、源码行、P01、Windows 行与下载路径。
  - `mac-ledger revoke --rows <id> --release-source <R> --quote <原话>`：按候选摘要撤销这一行在该候选下此前的全部发布记录；之后重新确认的记录有效
    （账本中的先后就是确认的先后）。
  - 读入与登记同样严格：每种记录（含撤销）恰有其字段，收据只给人工行、waiver 只给可 waiver 的行、下载记录只给 P18-D01–D05 且结果合于其行；`--ledger`
    与其锁文件不得是符号链接，也不得占用源码测试收据、其锁或其运行记录（按命令所在仓库的根判断；除不存在外，任何检查错误都使命令不读不写，第三次复审
    B6）。`--ledger` 在命令开始时按当前目录一次解析成绝对路径，检查、加锁与写入都用它；在 macOS 上与保留路径的比较不分大小写，尚不存在的文件也一样
    （默认的卷不分大小写；在区分大小写的卷上这多拒几个名字，第四次复审 BLOCKER 4），路径本身与它的每一级上级都这样比较（建立上级目录会把保留名
    变成目录，修复轮 SHOULD_FIX 7）。
  - 判定：人工行有本候选的发布收据为 PASS；可 waiver 的行在其余判定为 `NOT TESTED`、没有任何 `FAIL` 且有本候选的 waiver 时为 `WAIVED`；下载路径五行
    各有本候选的记录、每条的签名方式等于候选的、`sw_vers` 整份材料中恰有一个 `ProductVersion` 标签（末尾缺了值的、值紧贴冒号的标签都算一个，修复轮
    BLOCKER 6）、其后的值由一到三段数字
    组成且大版本等于 `--macos-major`（这是对所记材料的格式核对，不是对机器的证明，第三次复审 S7；第四次复审 BLOCKER 5）、D04 与 D05 各有合于其行的结果时成立。只认本候选的记录：
    以前候选的下载路径在签名方式与大版本不变时沿用的规则尚未实现，因此 fail closed。I01–I03 的记录只认 `--batch` 所指的批次。`WAIVED` 与 PASS 一样
    使该行成立。
- **尚无来源的行**：V03。它要求 `mac-latency` 与基线交替的运行，`mac-latency` 不进账本、没有发布用的判定规则（#phase-16-macos-host
  “资源运行进账本”），所以核对程序判它 `NOT TESTED`，可由 USER waiver；要让它由程序判定，须先由 USER 定下发布用的规则并让 `mac-latency` 进账本。
  R01、R02 没有候选字节上的记录时同样为 `NOT TESTED`，可 waiver；一旦记下 FAIL 便不能 waiver。
  普通 `mac-gui` 行与资源行在一组身份下 `FAIL`、另一组干净时 PASS（已批准的分组
  规则）；人工行与 R03 的 `FAIL` 不被另一组的 PASS 盖住。

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

<a id="physical-input-injection"></a>
### 物理输入注入

- smoke CLI 对真实桌面的键盘与鼠标输入（`SendInput`）以及光标放置（`SetCursorPos`，含只写回当前坐标的环境探针）只经一个
  注入模块。`keybd_event`、`mouse_event`，以及该模块之外的 `SendInput` 与 `SetCursorPos`（含 PowerShell helper）由治理文本
  扫描拒绝。helper 只做 UIA 定位、读取与 `InvokePattern`，并区分"观察到为空"与"无法观察"；它需要的光标放置、按键或点击由
  Rust 侧经该模块注入。发给目标窗口的合成指针消息（`WM_MOUSEMOVE` 等）只送达该窗口、不改变桌面输入状态，不是物理输入，也
  不证明物理投递，但同样服从下述共享停止。
- 该模块只在 `cfg(panic = "unwind")` 构建中注入；abort 构建只编译返回错误的桩，因为 abort 路径上未释放的按下无法由 `Drop`
  兜底。
- 每个按下先登记到发起它的操作，再注入；同一键或按钮同时只由一个操作持有（Windows 对每个键只记一个状态，第二个持有者的
  释放会结束第一个的按下），已被持有时拒绝再次按下或恢复性释放。每个操作只释放自己登记的按下，按与按下相反的顺序：正常
  路径显式、有界地释放，左键守卫的 `Drop` 与操作的 unwind 守卫兜底；释放返回后才注销登记。unwind 时（操作本身 unwind，或
  case 在持有左键时 unwind）先停止再释放；unwind 中的每个释放各尝试一次，其中再次发生的 panic 被接住且其 payload 不析构，
  不得逃出析构而使进程 abort、也不得中断其余释放。每次 `SendInput` 以返回的插入数判定：少于请求数即失败；完整插入只证明
  事件进入了输入流，不证明目标收到。左键按下与释放后各在有界时间内采样 `GetAsyncKeyState`，等待其读到相应状态：到时限仍未
  读到即失败：时限当刻取得的读数计入、读到不符即失败，时限之后取得的读数无论读到什么都不计（USER 2026-10-19
  决定：释放后 Windows 处理输入期间仍可能读到按下，故等待而非一次采样）；读到"未按下"也不证明目标收到释放（无法读取状态时同样读到未按下）。
- 任一释放失败（短计数、到时限仍读到按下、`Drop` 中的释放失败）、上述 unwind、或锁被此类 panic 毒化，都触发进程内共享停止：
  此后该进程的新注入、光标放置与合成指针消息一律拒绝（已登记的按下仍可释放），exact runner 不再执行注入输入的 G3/G4 case，
  它们不产生 PASS，收据如实记录未执行的原因。case 的业务断言与输入清理分别判定，清理失败的 case 即使业务断言成立也不是
  PASS。
- 注入前须核验目标：键盘输入时目标窗口为前台、激活且有焦点；托盘菜单的 Esc 只在前台线程处于弹出菜单模式且菜单所有者属于
  被测进程时注入，查询失败或所有者无法读取即为错误，未注入 Esc 也不证明菜单已关闭（关闭只认 helper 观察到关闭）；鼠标按下
  前立即读回光标，以读回位置为落点，该点下的根窗口须为目标窗口（托盘为 Explorer 的托盘窗口），需要精确落点的操作（托盘
  图标）另要求读回位置与目标点一致。不满足即不注入；核验与按下之间光标仍可能移动，这一残余竞态如实记录。托盘菜单的打开须
  核验从"观察到关闭"到"观察到打开"的变化，无法观察即失败；每个失败出口都报告各次尝试已取得的观察（含等待中任一次
  无法观察的轮询已读到的部分菜单项与光标读回位置，它们只作诊断）与清理结果。
- 自动化必须包含：可替换注入后端的单元测试覆盖短计数、释放失败后的共享停止、操作 unwind 与锁毒化后的释放与停止、unwind 中
  再次 panic（含析构会 panic 的 payload）的隔离、按后端时钟的采样时限、操作之间互不释放、以及停止后仍可释放已登记的按下；
  独占桌面上取证实际 abort 构建到达拒绝桩且零注入（绕过收据复用）、托盘菜单从关闭到打开、以及迁移后的 G3/G4 case。

### 验收矩阵

每个 `phase-XX.md` 必须逐项列出：稳定 ID、映射的 plan/AC、验证模式、仓库内入口、
当前状态与剩余证据。矩阵是 `docs/plan` 与全局 AC 的验证投影，不得发明或放宽需求。

**验收方式按验收阶梯选择**（USER 2026-11-19 提出，2026-11-20 推广到全部验收表）：每一行（必要时拆为
子项）用下列第一个可行的方式验证：

1. 程序断言；
2. agent 自行操作，依可观察的状态（辅助功能树、日志、文件、进程）判定；
3. agent 判定程序截下的图；
4. 程序截图，USER 判定；
5. 程序录屏，USER 判定效果；
6. USER 亲手操作并判定。

可行指它的检查能拒绝事先准备的坏样本；做不到就留在较低一级，不硬转。新增、复审或重新确认验收行时
先按这一顺序审视：每个人工行或人工子项要么上移，要么在矩阵或报告中写明不能上移的原因。Phase 16 以
`Automated`、`Agent`、`Review`、`Manual` 四种 Mode 表达六级，判定细则见 Phase 16 节；其他验收表
沿用现有 Mode、状态词与就绪规则，这一顺序只决定自动化做到哪一步，不引入新 Mode。改一行的 Mode
仍按行经 USER 批准；资格化 suite 的 exact-candidate 规则与正式人工矩阵的收据规则不因此改变。

状态词固定为：

```text
AUTOMATED PASS   当前提交上的仓库内可重复入口已经通过
MANUAL PASS      当前候选上的正式人工矩阵已执行：每个目标环境角色的全部必需场景均有有效 PASS 观察，
                 并引用完整环境/步骤/结果收据
AGENT PASS       仅限 Phase 16 矩阵：两名 agent 依同一身份下同一次运行的证据独立判定通过，意见不一致的子项
                 以 USER 的 Review 收据为准（见 Phase 16 的验收阶梯）
NOT TESTED       人工项或 Agent 行尚未完成，或只有一次性/不可重复记录
BLOCKED          自动化或人工验证已知失败，或环境阻止执行
```

禁止使用模糊的 `PASS`、`CONDITIONAL` 或模块存在来代替验收证据。一次性终端命令、
未提交脚本、主观观察、旧 commit 收据均不能把人工项从 `NOT TESTED` 提升为 PASS。

### Phase 12 发布资格状态

Source-controlled Phase matrix 使用上述四个通用状态；`AGENT PASS` 只用于 Phase 16 矩阵。Phase 12 的 ignored exact-artifact
release receipts 另有两种 USER authority 投影，二者不得混用：

```text
USER-APPROVED GATE   USER 批准工程 hard boundary 校准；不是人工验收豁免
USER WAIVED          USER 明确豁免列出的人工 case/group；未列出的 NOT TESTED 仍阻塞
```

Phase 12 source decision template 与 Source Freeze-bound `dist/evidence/release-decisions.json` projection
只允许 `PENDING`、`USER APPROVED`、`USER REJECTED`、`NOT APPLICABLE`。Rust automation 只能
记录 USER 已明确给出的决定，不能自行批准；PUSH 决定必须在 remote workflow 前绑定 source identity。
人工 waiver 只接受具体 `WAIVER-P12-Mxx` key 与 Tier B 组 key `WAIVER-TIER-B-v{version}`，不接受全局 blanket
waiver；逐项 key 与 Tier B 组 key 不跨 tier。可选人工项（Tier O 与 `previous` 角色）未测本就不阻断，不需要也不设
waiver；原目标环境档案 key `WAIVER-PROFILE-PREVIOUS-v{version}` 已退役（USER 决定 2026-10-31），不再被接受。其 source/version identity 来自相同
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
  人工项注册表：`current` 按 tier 与 waiver 规则处置；`previous` 需要另一台机器，其全部项为可选人工项（见下文人工
  发布政策）。
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
  观察在日志 schema 3 / 观察包 schema 2 中使用 `instances` 数组：M34 为零，普通场景为一，M45 为二；PID 非零且互异，
  每个 EXE 摘要均匹配候选。计数、唯一性与摘要使用共同判定。旧 schema 不自动改写或升级，文件保留；新上下文依既有声明／重置协议建立。
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
  ID（该摘要必须等于 candidate 的 EXE 摘要，否则观察无效；记录、导入与 readiness 共用这一判定）；同目录第二次启动、
  双副本、跨断屏/睡眠/RDP 保留原进程与原 note（不得重启或重新放入 fixture 来完成恢复）等
  要求由注册表按场景声明；预期的断连期间环境探测失败不判产品 FAIL。fixture 随工具提供：输入法项用空白 note，
  其余用渲染压力 note 与全部图片格式（含超限图片）。PASS 要求提交时运行的 StickyMD 恰好是注册表为该场景声明的实例
  集合（在存储锁内、写入之前确认）：普通场景为记录器启动的那一个；`TwoCopies`（只用于 P12-M45 两份便携副本）为记录器
  从两个不同副本目录启动的那两个；M34 没有实例。该例外不适用于同目录第二次启动。候选已启动但未在限时内完成初始化时，记录器报告原因，观察者可记录该实例的
  FAIL 或不记录，PASS 被拒绝；启动之前的准备失败（复制、fixture、摘要不符）不形成观察。实例以挂起状态创建、加入 Job 后才恢复运行，随记录器退出（含中断）
  而终止，StickyMD 自己启动的进程（如打开链接的浏览器）不受影响；创建与加入 Job 之间被强制终止时会留下一个从未运行的
  挂起实例，下一次会话拒绝启动并指明它来自中断的记录器。恢复挂起线程失败（含挂起计数不为 1）时该实例被停止、场景
  不记录；这些原生失败路径只验证了判定逻辑，未做原生故障注入。辅助进程（解压、进程枚举、UIA）同样在包含其子孙的 Job 中
  启动，退出或超时后其子孙一并结束。会话目录持有会话锁，创建、删除与清理都在会话根目录的锁下串行；正常退出时删除，异常退出后由下一次会话清理
  失去锁且名称符合会话格式的目录（每次清理的条目数、删除数与耗时有上限，其余留待以后），无法判定的保留并报告，根目录
  为链接时拒绝。开始前确认没有其他 StickyMD 实例；
  记录器不终止它未启动的进程。
- **特殊人工场景的过程证据**：M05 第二次启动先采样隐藏、最小化或实际 dock 收起几何，记录带时间戳的状态直到恢复；
  采样有上限，连续性和双击早于恢复由观察者明确声明。PASS 还要求恢复、激活、键入落入原窗口以及提交时仅原 PID。
  已证实的产品失败可记 FAIL；激活被系统拒绝、采样缺失或前置条件不足不形成观察。退出保存失败由记录器只读自己的 fixture，
  回读属性、保存前基线和退出失败标题，所有返回路径恢复属性；恢复失败不形成 PASS。M35 右／下排列记录主副屏实际矩形，
  分别要求顶部／左侧对齐。以上事实存入观察的 `declared`，与观察者声明使用不同键；不以自动检查代替人工回答。
- **其他机器**：主工作区导出观察任务（`acceptance manual export`，含 candidate receipt 与目标收据的原文、注册表
  摘要、`base`，以及所选场景尚未解除的 FAIL 的 ID、登记序号与说明，记录器在重测前展示）；工具二进制以观察包模式在
  被测机器上运行（`acceptance manual record`），不需要仓库，所需 fixture 随工具提供，helper 只用 Windows 自带的
  PowerShell `Expand-Archive` 与 `tasklist`，工具二进制只导入 Windows 自带的 DLL。工具内置的注册表必须与任务的注册表
  摘要相同，否则拒绝运行；记录器按任务携带的 candidate receipt 校验 ZIP 与 EXE。观察包文件不得预先存在，每个回答在
  下一个问题之前写入，会让包超过上限的回答被拒绝并结束会话；包路径位于某个仓库内（含经链接解析后的目录）时受该仓库的
  公共证据路径防护。主工作区在排他锁内整包导入（`acceptance manual import`），复用现有 candidate 校验：包头的
  candidate receipt、上下文、目标收据与注册表摘要必须与日志头相同，包的 `base` 不得超出日志，角色与实例的 EXE 摘要按
  同一判定重算，同 ID 同内容的记录跳过，任一不符则整包不写。
- **M34 Clean VM**：先完成首次运行观察，之后才复制并运行工具；该观察标为人工声明，记录 VM 基线与包的取得方式，
  不冒充事前环境证据。场景开始时的采集、由记录器启动被测实例，以及 Phase 14"人工观察前先查询交互环境"都不适用
  于这一次首次运行；工具事后在同一 VM 中采集环境事实并据此归属角色，事后采集与首次运行的人工声明分开记录，观察者
  声明两者之间未更改系统。
- **信任边界**：信任 USER，不防伪造观察；严格解析、摘要与上下文绑定防止的是混用、陈旧与损坏。
- **上限**：观察包 1 MiB，单个日志 10 000 条，备注 2 KiB。
- **人工义务映射**：Phase 8 验收矩阵中 Mode 为 Manual 的每一行，以及 `09` 的 Esc 行为表、垂直滚动条、透明度与退出保存
  失败条款，按 `(来源, 断言, 参数)` 映射到 P12 人工项场景、G3/G4 exact 检查、源码级自动化（单元测试、本地预检或正式
  Runtime 模块收据分开列出）、单元测试（只用于 exact 层无法稳定制造的状态，写明理由，不证明原生输入投递）、经 USER
  批准的退役（决定只授权其列出的条目），或尚未落实的缺口。治理测试核对来源区段与逐条款的文本摘要和条数（来源文字变化
  即需重新核对分解）、每个断言声明的必需参数集合被同一断言的映射恰好覆盖、目标存在。readiness 无论通过与否都在普通
  输出与 JSON 中列出全部缺口与退役项，不得把人工矩阵表述为完整；映射完整也不等于验收通过。
- **验证**（实现时进入 Phase 14 矩阵）：角色唯一匹配，以及类型错误、读取失败、两次采集不同、零匹配与多匹配的
  拒绝；迟到的旧 PASS 与旧 FAIL、重复与冲突 ID；部分覆盖、FAIL 与无效证据优先于 waiver、Tier C 与可选项；上下文失效、日志
  缺失、零字节或损坏与重置（含满额日志的未解除 FAIL 合并带入）、写入失败不留半更新、锁等待有界；FAIL 先于备注落盘，guided step 逐项回答；原进程与原 note 保留、
  M34 例外；观察任务快照一致、无仓库记录、整包导入的拒绝路径；人工义务映射的来源摘要、参数覆盖与目标存在；
  readiness 始终列出缺口与退役项。

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

唯一的 Windows Release Exact Artifact 必须来自已批准 Source Freeze 的 GitHub `release` workflow（构建来源可以是按
#platform-artifact-reuse 可沿用的祖先提交）；macOS 集合见本节末的 macOS 小节：

```text
Source Freeze
→ local/source preflight
→ explicit USER PUSH authority
→ push + CI
→ release workflow build/package/verify
→ record successful run/attempt/artifact id
→ qualification CLI 核对已记录 artifact id 的服务端身份（id、名称、未过期、所属 run 与 head）后按该 id 下载 ZIP + SHA256SUMS + SBOM
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
- smoke 证据（所有套件）为 schema 3：每个结果带 `owners` 数组，只有单元测试 owner 核验任务的结果可以非空；schema 2
  证据无效，不迁移。证据序列化 runner 在开始时采集的源码身份（HEAD 与工作树是否干净），写出前复核，身份变化即追加失败结果；
- 人工义务映射中由单元测试持有的义务以一个或多个 owner `(package, target, path)` 声明（一项义务需要几个测试共同证明时
  全部列出，全部核验才算有 owner），`target` 只允许 `lib` 或 `bin:<名称>`（使用 libtest 默认 harness 的单元测试目标；
  工作区清单的 `harness` 键只允许出现在 `[[bench]]`）。Windows 上 `all --ci` 的
  tests 部分包含 owner 核验任务：以 `cargo test --no-run --message-format=json` 的 `compiler-artifact` 建立目标到测试二进制的
  映射，对每个含 owner 的二进制执行 `--list` 与 `--list --ignored`（owner 须在前者恰好出现一次、不在后者中），再以
  `--exact <owner> --nocapture` 让每个 owner 单独一个进程执行。构建、两次列出与每次执行都须成功退出、输出完整（未超时、未截断）
  并解析成功；任一不满足即任务失败，且不贡献已核验 owner。任务前后复核源码身份与开始时一致。本地 headless CI 收据中 owner 任务的
  结果列出已核验 owner；readiness 只在该收据有效、绑定当前 Source Freeze，且已核验集合恰为映射中的 owner 集合时，把这些义务算作
  有 owner，否则全部列为待执行证明。正式收据须在冻结提交的独占 checkout 与构建目录上产生（运行期间没有其他源码或产物写者）；
  复核只发现漂移，不证明期间不变。核验针对意外情形（测试被删除、改名、忽略、cfg 排除、过滤失配、失败或未执行）；owner 测试须把
  控制权正常交回 libtest，测试体自身提前成功退出与断言遗漏由代码审查承担。定向运行（`modules run`、dev-check）按所选 package
  投影 owner，没有适用 owner 时连发现构建一起跳过，且不写收据；
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
ZIP 路径；qualification CLI 必须按 remote receipt 记录的 artifact id 自行下载权威副本：先向 GitHub 核对该 id 的 artifact
名称、未过期、所属 run 与 head 均与收据一致，再按该 id 下载，确认
ZIP/checksum/SBOM 与用户副本逐字节一致后才允许 staging。

- manual recorder 必须是交互式 human receipt recorder，逐场景只接受显式
  `MANUAL_PASS` / `MANUAL_FAIL` / `NOT_TESTED`（后者不写入日志），不得从 process/status 自动推断人工 PASS；
  guided step 覆盖多项时逐项询问；
- stale module fingerprint 不参与 readiness；G3/G4/G5 成功记录还必须包含 origin ZIP/EXE、运行 harness、
  clean worktree 与各组预期逐项结果，只有当前 registry 指纹兼容时才能复用；
- G3/G4/G5 exact 收据为 schema 2：每个 case 结果带 `checks` 数组，列出该 case 实际记录的检查标签。标签写作 `名称` 或
  `名称:k=v[,k=v…]`，名称、键与值只用小写 ASCII 字母、数字与 `-`，参数键严格升序；每个 case 的必需标签集合与 case 表
  同源、一处静态声明，由检查函数在其断言成立之后记录（同一标签多次成立只记一次）。只有结果为 `PASSED` 且记录集合与
  声明集合完全相等（无缺失、多余或重复）的 case 才算成功；G5 声明空集合，截图要求不变。失败或标签不符的结果仍完整
  写入收据作为诊断，但不能作为成功证据；schema 1 收据无效，不迁移。标签相等只证明声明的检查都已到达，不独立证明断言
  语义，断言与标签的对应由代码审查承担；
- readiness 对 P0/P1、未批准 hard gate、mandatory manual NOT TESTED、exact package、remote
  evidence 与 USER decision fail closed；不得提供 `--force-ready`；
- readiness 读取的每份收据（candidate、Source Freeze、decision、remote/downloaded、headless、manual 与模块
  evidence）都整份严格解析为 JSON 对象，每个字段、case 与 decision 只从所属对象读取；重复键、截断、尾随
  输入或类型不符即为无效，不以文本搜索寻找字段；
- exact evidence 记录的 Windows build 必须含具体版本与构建号（如 `Microsoft Windows 10.0.26200.9457`），
  由 `ver` 输出读取（按控制台代码页有损解码后只取 ASCII 版本号）；读取失败记为 `UNKNOWN` 并使收据无效，不得以
  `OS` 环境变量（`Windows_NT`）代替；
- freeze 后 source/manifest/lock/runtime asset 或 release tooling 变化仍要求重建 Source Freeze。下游证据由
  evidence class 决定：exact-byte evidence 必须重建；module evidence 只在其 registry 输入指纹变化时 stale。新的 Source Freeze 上，
  某一平台的发布物按 #platform-artifact-reuse 可以沿用时，原样重新 Promote 同一 artifact，不重新构建。

<a id="macos-release-artifact"></a>
#### macOS 发布物、候选收据与晋升（Phase 18）

macOS 集合与 Windows 完全分开：独立的 workflow、run、checksum 文件、SBOM、候选收据、staging 目录与 resolver。

- **构建：** `release-macos` workflow 只接受 `workflow_dispatch`，三个发布 job 运行在 `xcode-27` runner 上（依赖策略 job 在 `ubuntu-latest`），候选提交必须在 `origin/main` 上。它先以
  `xcode-select` 选定 Xcode 27.2（SDK macOS 27.2），再用 `build-app.sh` 构建并 ad-hoc 签名，并留一份构建记录：`sw_vers`、Xcode 与 SDK 版本、
  `cargo -V`、`rustc -vV`、产品输入指纹（#phase-16-macos-host）、`codesign -dv` 的结果、架构与 `LC_BUILD_VERSION`。签名维持 ad-hoc（`02` macOS 节）。
- **打包：** Rust CLI 生成包目录，`ditto -c -k --norsrc --keepParent` 打包：

  ```text
  StickyMD-<version>-macos-arm64-portable.zip
  └─ StickyMD/
     ├─ StickyMD.app
     ├─ README.txt                第一次打开、安装版与便携版、升级与迁移步骤；含 Source commit 行
     ├─ LICENSE.txt
     ├─ THIRD_PARTY_NOTICES.txt   以 stickymd-mac 为根、aarch64-apple-darwin 为目标的依赖图
     └─ licenses/
        ├─ SIL-OFL-1.1.txt
        └─ KaTeX-fonts-NOTICE.txt

  SHA256SUMS-macos-arm64.txt      ZIP 与 SBOM 两行
  SBOM-macos-arm64.spdx.json
  ```

  不预建 `note/`。workflow 把 ZIP 重新解出后校验：成员白名单、`codesign --verify --strict`、仅 arm64、Info.plist lint 与 `StickyMDInputs`、
  README 的 Source commit 行；然后上传。
- **artifact 名带 attempt：** `stickymd-macos-arm64-release-<attempt>`，每个 attempt 的 artifact 因此可以区分；候选收据记录该成功 attempt
  实际产出的 artifact id。
- **本地晋升（macOS 主机）：** 按 run id 与 attempt 查询该 attempt 的结论（须为 `success`，workflow 须为 `release-macos`），确认 artifact 的名字带
  这个 attempt、id 属于这个 run，按 artifact id 下载，与用户提供的副本逐字节比对，再校验 checksum、SBOM、包结构、签名完整与签名方式、架构，然后原子 staging：

  ```text
  dist/exact-candidate-macos/
  ├─ StickyMD-<version>-macos-arm64-portable.zip
  ├─ SHA256SUMS-macos-arm64.txt
  ├─ SBOM-macos-arm64.spdx.json
  └─ StickyMD/StickyMD.app
  ```

  全部校验通过后才写 `dist/evidence/release-candidate-macos.json`。receipt 记录构建来源 SHA、version、`Cargo.lock` SHA-256、产品输入指纹、
  构建来源上的 Platform Build Input Fingerprint 及其算法版本、workflow run/attempt、artifact id/name、ZIP/checksum 文件/SBOM/可执行文件/bundle 清单的
  SHA-256、target 与观察到的签名方式，不记录机器绝对路径与发布提交。macOS 的 artifact-bound runner 只经这个 resolver 取得 bundle。
- **与发布提交的关系：** Windows 的 Source Freeze 在 Windows 主机上，macOS 晋升不读它。晋升与 macOS 就绪核对都显式接收发布提交 R（与
  发布授权中的是同一个值），要求 R 包含在 `origin/main` 中、构建来源等于 R 或按 #platform-artifact-reuse 可以沿用；不以“当前 HEAD”暂代 R。
- **失败路径：** 与 Windows 相同，fail closed：下载不完整、attempt 不是 `success`、artifact 不属于该 attempt、哈希、checksum、SBOM、签名、架构或包结构
  不符时，既有 macOS candidate 不得被半更新，也不写 receipt；receipt 缺失时 resolver 拒绝使用 staging 目录，不从本地构建补位。

<a id="platform-artifact-reuse"></a>
#### 跨提交沿用平台发布物（USER 批准 2026-11-19）

- **规则：** 每个平台的发布物记下构建来源提交 S。发布提交 R（打 tag 的提交，即 Windows Source Freeze 的提交）可以沿用在 S 上构建的某一平台
  发布物，当且仅当 S 是 R 的祖先，且该平台的 Platform Build Input Fingerprint（`01`）在 S 与 R 上相同。两个平台对称适用：修 macOS 不重建 Windows，
  反之亦然。同一提交上一侧偶发失败时只重跑那一侧的 workflow。
- **指纹（算法 v2）：** 在提交的 Git tree 上，对每个输入路径按路径字节排序，依次写入路径、文件模式、内容长度与内容本身，连同平台名与算法版本
  计算一个 SHA-256；不读工作树，也不使用 Git 的替换对象。算法版本同时标识序列化与排除表：两者任一改变都升版本，所以同一 tree 在两个版本下的值
  不可比较。v2 相对 v1 只改了排除表（不再计入验证工具，见下文），序列化不变。输入路径上的条目必须是普通文件（模式 `100644` 或 `100755`），符号链接与子模块
  使指纹失败（链接的 blob 只是目标路径名，目标内容会漏出指纹）。两个 SHA 只有都是本仓库中的提交（对象本身的类型为 commit，注解 tag 不算）
  时才参与判定。发布 workflow 不得调用仓库内的 action（governance 检查），
  否则那些 action 也要计入。
- **排除表**（逐项理由；其余路径，包括未分类的新路径，都是输入）：
  - 两个平台共同：`docs/`（文档不进入发布物，也不参与构建）；根目录的 `AGENTS.md`、`CHANGELOG.md`、`CONTRIBUTING.md`、`README.md`、
    `README.en.md`、`README.zh-CN.md`、`SECURITY.md`（包内 README.txt 由打包工具中的模板生成）；`experiments/`（不是 workspace 成员）；
    `tools/manual/`（人工步骤说明）；`assets/readme/`（仓库 README 的插图）；`.github/` 下除本平台发布 workflow 之外的文件（CI、定时任务与
    晋升 workflow 不构建发布物）；`tools/stickymd-smoke/` 下除 `Cargo.toml`、`build.rs` 与分类器 `src/release/platform_inputs.rs` 之外的文件
    （验证工具，不产出发布字节，见下文打包 crate 一条）。
    `tests/` 与 `tools/smoke/` 不排除：没有逐项证明构建与打包都不读它们，保守计入。
  - 仅 Windows 排除：`apps/stickymd-mac/` 与 `crates/stickymd-posix/` 下除各自 `Cargo.toml` 与 `build.rs` 之外的文件（只在 macOS 编译）、
    `.github/workflows/release-macos.yml`。
  - 仅 macOS 排除：`apps/stickymd-win/` 下除 `Cargo.toml` 与 `build.rs` 之外的文件、`.github/workflows/release.yml`、`tools/release/`（Windows 的 PowerShell 打包与校验脚本）。
  - 每个 workspace 成员的 `Cargo.toml` 对两个平台都是输入：另一宿主的清单可以在 `Cargo.lock` 不变时改变 feature 合并，macOS 的产品输入指纹
    （写进 Info.plist）也读取每个成员的清单。每个成员的 `build.rs` 同样是两个平台的输入：Cargo 不需要清单条目就会发现并在编译该成员时运行它，
    Windows 的 workspace 构建会编译每个成员。governance 以 Cargo metadata 核对 workspace 的构建脚本恰为清单中的一个（见下文“边界的证明”），
    其他成员要有构建脚本须先改清单。分类器仍把 `apps/stickymd-mac/`、`crates/stickymd-posix/`、`tools/stickymd-smoke/` 下的 `build.rs` 计入两个平台，
    是有意的：清单检查失守时，出现的构建脚本仍会改变两个平台的指纹。这些文件名按 ASCII 大小写不敏感匹配：在大小写不敏感的卷上，Cargo 会把
    `Build.rs` 当作 `build.rs`。
  - 分类器 `tools/stickymd-smoke/src/release/platform_inputs.rs`（排除表与序列化）对两个平台都是输入：它就是沿用规则本身，它的改变是沿用规则的
    改变，不属于“只改验证工具”。
  - 版本号、`Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`、`deny.toml`、`LICENSE`、`THIRD_PARTY_NOTICES.md`、`assets/` 的其余部分按原样计入。
  - **打包 crate `tools/stickymd-package/` 整体计入两个平台**（USER 批准 2026-11-21）：发布字节中由仓库工具产出的部分都出自它——Windows 与 macOS 的
    包目录与 ZIP、包内 README 与第三方声明、SBOM 与 checksum，以及 `build-app.sh` 写进 macOS bundle 的产品输入指纹（#phase-16-macos-host），
    打包前的门（Windows 的 PE import 门，#portable-windows-runtime；macOS 的包检查，#macos-release-artifact），连同它们依赖的公共代码（JSON、哈希、
    原子写入、临时目录、路径身份与输出守卫）。它只依赖标准库，不依赖 `stickymd-smoke`；`stickymd-smoke`
    依赖它。产出字节的调用点——`build-app.sh`、Windows 的打包与 SBOM 脚本及 `release` 中调用它们的步骤、`release-macos` 的打包与 SBOM
    步骤与打包前的门——只调用 `stickymd-package` 的二进制，不经过 `stickymd-smoke` 的命令行。`stickymd-smoke` 的晋升、资格化与校验调用同一份库代码。
  - **边界的证明（闭合世界，USER 决定 2026-11-22）：** crate 依赖由编译器保证打包代码引用不到验证代码，这是放宽的前提。编译器看不到的边，
    即生产链中不经过 Rust 依赖的输入，由 governance 测试按清单检查。每条规则只用四类可判定的事实：与 `stickymd-smoke` 中固定清单或固定副本的
    逐字比较（行尾统一为 LF 后）、对文件全文的原样搜索、Cargo 与 rustc 自己报告的事实、Git 索引中的路径与文件模式（检查在被检提交的干净检出上
    运行）。检查不解析 Rust、PowerShell 或 YAML 的语法，清单之外一律拒绝。Git 索引中的路径不得含 `\`；Cargo 与 rustc 报告的路径只在 Windows 上
    把 `\` 视为分隔符，其他平台上含 `\` 的路径被拒。
    - **Cargo metadata：** 打包 crate 没有依赖、没有构建脚本、没有 feature；workspace 中没有过程宏成员（它们在编译器里运行），二进制名只用 ASCII
      字母、数字、`_` 与 `-`，不分大小写不重复（被排除的成员无法在 workspace 构建的输出中顶替产品）；每个成员的清单在 Git 索引中是普通文件，按索引中的路径两个平台都
      计入（落在排除区的新成员须先改分类器）；workspace 的构建脚本恰为清单（成员与路径成对）：`stickymd-win` 的 `apps/stickymd-win/build.rs`；
      `Cargo.lock` 中含所属成员名的行恰为它自己的 `name = "stickymd-win"`，即解析图中没有任何包（含 workspace 之外的包）依赖它。
    - **编译器读过的文件：** 固定的 workflow 以 dev profile（`cargo run`）运行打包工具，Windows 的发布 job 还以 release profile 构建 workspace。
      governance 在运行它的平台上，为每次检查新建私有目录（同时作为 Cargo 的 target 与 build 目录），对打包 crate 的 lib 与 bin target（库的各种 crate-type 都算 lib）
      按这两个 profile 各运行一次 `cargo check`。在每个 profile 中，打包 crate 的每个 lib 与 bin target 恰有一个新编译（`fresh: false`）的编译单元，它的
      dep-info 登记它自己的入口文件；dep-info 的规则行与逐文件行列出同一组文件，否则拒绝。这些 dep-info 登记的本地源文件与文件依赖都在
      `tools/stickymd-package/` 内，在 Git 索引中是普通文件（不是符号链接），按索引中的原样拼写（因此不含 `..` 与绝对路径），且两个平台都计入；
      需要反转义的路径（含空格或 `$`）被拒。dep-info 中不得有环境读取（`# env-dep:`）：由环境变量在编译期选择的文件（例如以 `env!` 拼出的
      include 路径）在 `self-check` 与 `build-package` 中可能不同。模块文件、`#[path]` 与三个 include 宏读入的文件，无论怎么写、经过别名还是宏展开，
      都由 rustc 登记。这只证明 Rust 前端登记的输入：标准库来自固定的 Rust 工具链（`rust-toolchain.toml`，计入）；原生链接器、SDK 与导入库（如
      Windows 打包代码链接的 `Bcrypt.lib`）属于平台构建环境（runner 镜像），不由本检查证明。两个平台的 `self-check` 都运行这项检查（Windows 经
      `tools/smoke/all.ps1 -Ci` 中的 governance，macOS 经 `stickymd-smoke` 的测试），各自覆盖本平台 `cfg` 下编译的文件。
    - **构建脚本：** 清单中的构建脚本的全文中没有 `../`、`..\` 与 `".."`。固定的 Windows 发布 job 以 workspace 构建，会运行它；固定的 macOS 发布构建
      只选择 `stickymd-package`（`cargo run`）与 `stickymd-mac`（`build-app.sh`），而 `Cargo.lock` 中没有包依赖 `stickymd-win`，所以它只在 Windows
      构建中运行，
      它所在的成员目录计入 Windows 指纹。它自己声明的模块与引入的文件不在打包 crate 的 dep-info 检查之内，由下文“产品发布图”在 Windows 主机上
      核对；名字搜索覆盖其中位于 `apps/` 与 `crates/` 下的跟踪文件。
    - **脚本：** Windows 产出字节的脚本按保守清单检查：`tools/release/` 下的 `package.ps1`、`generate-sbom.ps1`、`invoke-package.ps1`、
      `package-archive.ps1`、`syft-fetch.ps1`、`syft-execute.ps1`（从固定的 `build-package` job 可达），以及保守纳入的 `generate-third-party-notices.ps1`
      （独立入口，经包装函数启动打包工具）与 `archive-facts.ps1`（打包库直接启动，供校验与晋升使用）。每个都按此拼写在 Git 索引中是普通文件，
      计入 Windows 指纹，且是不含 NUL 的合法 UTF-8（其他编码中名字的字节可能不连续）。这些脚本与打包 crate 中编译器读过的文件里，每处 `.ps1`（不分
      大小写）紧前的文件名（ASCII 字母、数字、`.`、`_`、`-` 组成的最长串）都在清单中，且文件名位于文件起点，或前一个字节是 ASCII 且不是 `/` 或 `\`；
      `build-app.sh` 中没有 `.ps1`。
    - **workflow：** 每个发布 workflow 的全文等于 `stickymd-smoke` 中的固定副本；副本恰为分类器计入的 `.github/` 文件（两个发布 workflow），副本经审查
      具有上文 `dependency-policy` 与三个发布 job 的结构，改动 workflow 须同时改副本。
    - **清单与合同：** 代码中的脚本清单、构建脚本清单、profile 与下文“产品发布图”的闭包外成员清单恰为本条所列；测试按本条原文核对，
      改清单须同时改合同。
    - **名字：** 在打包 crate 中编译器读过的文件、清单中的脚本、`build-app.sh`、`crates/` 与 `apps/` 下被 Git 跟踪的全部文件（产品 crate 与构建脚本，
      含构建脚本可能声明的模块），以及每个固定副本中从 `  build-package:` 行到 `  verify-artifact:` 行之间的文本里原样搜索（不分大小写，注释也算）：
      不出现 `stickymd-smoke`、`stickymd_smoke`、`invoke-smoke.ps1`、`Invoke-StickyMdReleaseTool` 与 `tools/smoke/`（两种分隔符）。
    - **产品发布图（USER 决定 2026-11-22，“采用 B”）：** 在本次 `cargo check` 中，受检单元由 Rust 前端登记的文件都计入本平台指纹。governance 在 Windows 与 macOS
      主机上各检查本平台。Cargo 所启动的编译器（设置了 `RUSTC` 时为它，否则为 `PATH` 中的 `rustc`）以 `-vV` 报告的主机三元组须为本平台的三元组，
      否则检查失败：Windows 为 `x86_64-pc-windows-msvc`，因为固定构建不加 `--target`；macOS 为 `aarch64-apple-darwin`，因为过程宏在主机上编译和运行，
      交叉检查登记的文件可能不同。这一判定与检查器自身为哪种架构编译、处理器是否仿真无关。在其他系统上不适用（例如只跑 governance 的 Linux CI；
      两个平台的 `self-check` 都在原生主机上运行）。
      - **构建：** 每次检查新建私有目录（同时作为 Cargo 的 target 与 build 目录），按固定发布构建的选择与 profile 运行一次 `cargo check`：Windows 为
        `--workspace --release`，不加 `--target`，与 `release` 的 `cargo build --workspace --release --locked` 相同（feature 按同一组成员合并）；macOS 为
        `-p stickymd-mac --release --target aarch64-apple-darwin`，与 `build-app.sh` 相同。`build-app.sh` 的全文（行尾统一为 LF 后）等于 `stickymd-smoke`
        中的固定副本，改动脚本须同时改副本。两份固定副本中各恰有一行（去掉首尾空白后）运行 `cargo build`：Windows 发布 workflow（见上文 workflow
        一条）中为 `run: cargo build --workspace --release --locked`；`build-app.sh` 中为
        `clean cargo build -p stickymd-mac --release --locked --target "$target" --target-dir "$repo/target"`，且它恰有一行以 `target=` 开头，
        为 `target=aarch64-apple-darwin`。检查侧同样带 `--locked`；脚本中其余行的执行语义由已审的全文承担。
      - **范围：** 发布二进制是宿主成员（Windows 为 `stickymd-win`，macOS 为 `stickymd-mac`）中与成员同名的 bin target，宿主成员恰有这一个 bin。它的
        依赖闭包由该平台的 `cargo metadata --filter-platform` 中 normal 与 build 边求出。按 target 计，下列每个 target 恰有一个新编译的单元：宿主成员的
        那个 bin、宿主成员的 lib（若有）、闭包内其余成员的 lib，以及闭包内成员的构建脚本（若有）。同一 target 以不同 feature 编译两次（例如同时作为
        normal 与 build 依赖）也被拒，出现时须先改本条。闭包外的 workspace 单元只能属于下列成员，不检查（它们不进入发布二进制，例如在 Windows 上编译
        为桩的两个 macOS 成员）：Windows 为 `stickymd-mac`、`stickymd-posix`、`stickymd-package`、`stickymd-smoke`，macOS 为空；其他单元一律拒绝。
        依赖图中不得有 workspace 之外的本地包（metadata 中 `source` 为空的非成员，例如经 `[patch]` 或 path 依赖引入、由 `workspace.exclude` 排除的包），
        否则它们的文件不在任何检查之内。
      - **文件：** 受检的 lib 与 bin 单元的产物须位于 build 目录下本平台目标的位置：Windows 为 `release/deps`（即没有生效的 `build.target`），macOS 为
        `aarch64-apple-darwin/release/deps`。这些单元的 dep-info 按上文同样读取（规则行与逐文件行一致，不得有环境读取），并登记该单元自己的入口文件（metadata 的 `src_path`）；
        构建脚本单元的规则只指向这份 dep-info 与同名的可执行文件。登记的每个本地文件在 Git 索引中是普通文件，按索引中的原样拼写（因此不含 `..`
        与绝对路径），且为本平台的输入（分类器 `is_input`）。与上文一样，检查只观察本 job 的编译（见前提）。
    - **Cargo 配置：** Git 索引中首个路径分量按 ASCII 大小写不敏感等于 `.cargo` 的文件恰为普通文件 `.cargo/config.toml`（按此拼写；在大小写不敏感的
      卷上，`.CARGO/config` 也会被 Cargo 读取），其全文（行尾统一为 LF 后）等于 `stickymd-smoke` 中的固定副本。副本经审查，不转引
      其他配置文件（`include`），也不让 rustc 从文件读取参数（`@文件`），因为这两种读取都不登记在 dep-info 中。

    **前提：** `self-check` 与 `build-package` 运行同一份固定 workflow、同一 runner 标签、同一检出与工具链；两者的有效编译配置（runner 环境中的
    `RUSTFLAGS`、`CARGO_BUILD_TARGET`、`RUSTC`、Cargo home 中的配置，含 `build.rustc` 与 `build.target`）相同。检查只能观察本 job 的编译，这一前提不由它证明。能检查的只有一部分：两个发布 workflow 的固定副本不含下列字面串（原样
    搜索、不分大小写：`RUSTFLAGS`、`CARGO_ENCODED_RUSTFLAGS`、`RUSTC_WRAPPER`、`RUSTC_WORKSPACE_WRAPPER`、`CARGO_BUILD_TARGET`、
    `CARGO_BUILD_BUILD_DIR`、`CARGO_TARGET_DIR`、`RUSTUP_TOOLCHAIN`、`RUSTC:`、`RUSTC=`、`env:RUSTC`），仓库内的 Cargo 配置计入指纹并等于固定副本（见“Cargo 配置”）。
    其他拼写与其他改变编译的变量（如 `CARGO_PROFILE_*`）、runner 镜像与 Cargo home 的配置，以及各调用点对环境的处理（例如 `build-app.sh` 以
    `env -i` 清空环境后再运行打包工具，而检查器与 workflow 中的其他调用继承环境）仍属前提。打包 crate 没有依赖与 feature，workspace 的 feature 合并改变不了它。

    **剩余：** 以下不能由这些事实证明，由代码审查承担：
    - 清单中的文件在运行时做什么，例如运行时拼出的名字或路径（`Join-Path` 的目录参数）、不含上述名字的命令，以及已审文件在运行时选择读取的
      被排除文件（之后只改被读文件时，指纹不变）。
    - 参与构建、但不由受检单元的 dep-info 登记的读取与副作用：
      - 任何构建脚本运行时读写什么；受检构建脚本单元自己的模块与引入的文件由“产品发布图”核对。
      - 过程宏在编译器中的文件访问，只要它不展开为 `include!`、`#[path]` 等登记形式；包括闭包外成员与 registry 包所用的过程宏。
      - registry 包与 git 包：registry 归档由 `Cargo.lock` 中的校验和锁定，git 来源由精确的提交锁定（依赖策略 `deny.toml` 禁止 git 来源），但它们的
        构建脚本与过程宏读什么同属此列。
      - 编译器后端与链接器读入的文件，例如 `global_asm!` 中的 `.incbin`、链接参数指向的文件。
      - `cargo check` 与 `cargo build` 的展开差异，例如按输出种类选择 include 的过程宏。
    - 检查器与固定副本本身位于被排除的验证工具中，它们的改动由代码审查与 R 上的重新校验承担。

    在这些前提成立、上述审查也成立时，产出字节的代码既不运行验证工具，也不读它计入范围之外的文件。版本规范化按根清单的成员表读取每个成员的
    清单，其中的 `tools/stickymd-smoke/Cargo.toml` 是计入的输入。
  - **只改验证工具时：** 即 `tools/stickymd-smoke/` 中上文三个例外以外的文件。两个平台都可以沿用，不重新构建；按下文“沿用后”，受影响的门用
    R 的工具在同一份字节上重跑。
  - **产出字节的 job 只运行计入指纹的代码（USER 决定 2026-11-21，方案 A）：** 每个发布 workflow 在依赖策略 job（`dependency-policy`，
    `cargo deny`）之后分为三个 job，各在自己的 runner 上运行：
    1. `self-check`（`needs: dependency-policy`）：版本读取、fmt、Clippy 与源码测试（Windows 为 `tools/smoke/all.ps1 -Ci`，macOS 为共享 crate、宿主与两个工具 crate 的测试），会运行
       `stickymd-smoke`；
    2. `build-package`（`needs: self-check`）：只运行计入本平台指纹的代码：祖先关系核对（Git）、产品构建、打包前的门（Windows 的 PE import 门、
       macOS 的包检查，都在打包 crate 中）、打包、SBOM、构建记录，然后上传 artifact；
    3. `verify-artifact`（`needs: build-package`）：下载刚上传的 artifact，用 `stickymd-smoke` 在这份字节上校验（Windows 为包校验；macOS 为完整的
       三件套：恰好 ZIP、checksum 文件与 SBOM，三者彼此一致），并在摘要中写下构建来源的平台指纹。

    macOS 的 artifact 以构建它的 attempt 命名（#macos-release-artifact），所以 macOS 的重试从 `build-package` 起重跑（或整个 workflow 重跑）；只重跑
    `verify-artifact` 找不到本 attempt 的 artifact，以失败结束。Windows 的 artifact 名不带 attempt，只重跑 `verify-artifact` 会取回同一 run 中已上传的
    artifact；Windows 按 attempt 收紧另行决定（见上文“触发与步骤”）。

    run 的结论要求全部 job 都成功，晋升与 TAG、DRAFT、PUBLISH 照旧只认成功的 run 与它上传的 artifact。被排除的代码因此不与产出字节的步骤共用
    文件系统或环境；governance 按上文“边界的证明”中的 workflow 一条核对 job 结构与固定副本。
  - **自检与校验的证据地位：** S 上 `self-check` 与 `verify-artifact` 的通过结论不是发布证据；源码测试与受影响的门的通过结论只认 R 上的运行：
    Windows 是绑定 Source Freeze 的本地 headless CI 收据与 Promote 的重新校验，macOS 是 #macos-release-gate 的“R 上的源码测试”与晋升的重新校验。
    构建本身的事实仍绑定 S，不能在 R 上重现：成功的 run 与 attempt、artifact id、构建来源，以及构建记录中的 SDK、工具链与签名观察。
    `ci.yml`、`scheduled.yml` 与本地 cache action 仍不计入。
- **构建来源 S 与发布提交 R 的核对对象：**
  - 绑定 S：构建 run 的 head、包内 README 的 `Source commit`、两个平台 candidate receipt 中的构建来源。
  - 绑定 R：Windows Source Freeze、tag、TAG/DRAFT/PUBLISH 的授权、晋升 workflow 检出与签发证明的提交。
  - 远端记录、Promote、resolver、readiness、macOS 就绪核对与晋升 workflow 的三个 operation 共用同一个沿用判定：S 等于 R，或在 R 上重算两侧
    指纹并确认 S 是 R 的祖先且指纹相同。Windows 的 candidate receipt 只含 S、run/attempt/artifact 与字节身份，所以同一 artifact 在 R 的
    Source Freeze 上原样重新 Promote 得到同一 receipt，按其摘要绑定的人工观察日志仍可用。
  - 沿用时另写 `dist/evidence/release-reuse-<platform>.json`：平台、S、R、两侧指纹与算法版本，以及（Windows）candidate receipt 的摘要。
- **沿用后：** 沿用只保留发布字节与候选身份。模块证据按当前 Qualification Module Fingerprint 判定，人工观察按当前人工上下文判定，
  exact-byte 门按当前规则在同一份字节上重跑。例如只改验收矩阵的提交：两侧产物都可以沿用，但 Windows 全部模块的收据失效（#module-success-ledger）。
  沿用时模块证据记下的运行提交（Runtime、Performance 的 `commit`，G3–G5 的 `harness_commit`）是 R 而不是 S：它等于 S 或能按同一判定沿用 S，
  就与该候选对应。readiness 还要求沿用记录在 S≠R 时存在、在 S=R 时不存在，且与 Promote 会写出的字节相同。
- **来源证明：** 发布说明为每个平台写明构建来源提交、run 与 attempt。晋升 workflow 签发的证明来自它自己的运行（OIDC 中的 workflow ref 与 SHA），
  描述的是晋升这次运行，不是构建。DRAFT 与 PUBLISH 从 tag 触发，并核对运行的 SHA 等于 R；TAG 从 `main` 触发，核对 R 包含在 `main` 中。
  证明能否表述“这些字节由 S 编译”，在第一次跨提交 PUBLISH 前检查实际签发的 statement（未验证）。

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
工作区内部 crate 互相只按 `path` 引用、不写版本要求，并设 `publish = false`（cargo-deny 以 `allow-wildcard-paths` 放行；USER 批准 2026-11-17），
所以升发布版本只改上述两个文件，member manifest 不变。
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

M1..M5 manual sessions 只允许共享 setup，不改变 P12-M01..P12-M47 的逐项 authority。Phase 14
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
六个高内聚组：G4-01 tray 菜单、close/show/dirty quit；G4-02 主屏 Left/Top/Right、2 DIP 感应条、
24/25 DIP capture、`Top > Left > Right`、700/100/500 ms 与 focus/IME/Pin guard；G4-03 legacy
clipboard shortcuts、Preview 只读与 Source Search 的键盘查找/替换、视图停放恢复；G4-04 真实 toolbar 数学分隔符转换、源码即时投影、literal
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

人工观察前优先复用已有客观检查。G4-05 还应覆盖隐藏与原生最小化后的同 HWND 唤醒、
无辅助激活的焦点恢复、立即输入与持久文件保持，以及两个受控便携副本各自托盘的显示/隐藏隔离。
G5-02 验证最小尺寸与恢复尺寸使用实际窗口 DPI；G5-03 验证三种视图的缩放上下限、
Ctrl+滚轮 5% 步进与重置、窗口几何不随内容缩放，以及 40/70/96/100 四档原生整窗 alpha。
G4-06 的两种真实输入法同时覆盖上述四档透明度的提交与撤销。
偏好切换不得改写 Markdown，正常退出后重启必须恢复已提交偏好。以上使用现有 G4/G5
入口和失败传播，不建立新的人工 PASS 来源；视觉判断和未具备的物理环境仍独立记录。

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
`(项, 角色)` 集合），不跨版本继承。

可选人工项（USER 决定 2026-10-31）：需要额外机器、虚拟机或显示器才能执行的人工观察不阻断发布，也不需要 waiver。
它包括 Tier O 的全部项（Clean VM、双显示器同 DPI / 混合 DPI、运行中拔插、RDP、物理负坐标布局），以及 `previous`
角色下的全部项。可选项的 `NOT TESTED` 计入 READY；已观察到的 `MANUAL_FAIL` 与无效证据仍阻断。汇总逐项逐角色列出
可选项的 `NOT TESTED`，并写明 READY 不表示这些环境已经测过。单屏改缩放、睡眠唤醒等本机可完成的项不属于可选项。

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
- std-only 打包 crate `stickymd-package` 必须解析 exact Release PE32+ 的普通 import table 与 delay-load
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

1. `release.yml`（Windows）与 `release-macos.yml`（macOS，#macos-release-artifact）只接受显式 `workflow_dispatch`，分别从
   Source Freeze 或可沿用的构建来源提交构建、测试、deny、检查 manifest、（Windows）native-runtime import 或（macOS）签名与架构、
   生成 portable ZIP/checksums/license/SBOM 并上传 workflow artifact，按 #platform-artifact-reuse 在 `dependency-policy` 之后分为 `self-check`、`build-package`、
   `verify-artifact` 三个 job；它们不创建 tag、draft 或 publish。两个平台各自 dispatch，
   一侧失败只重跑那一侧。
2. 两个平台分别本地下载并 Promote 各自的 artifact，完成全部 artifact-bound qualification：Windows readiness 必须为 `READY`，
   macOS 就绪按 #macos-release-gate 成立。
3. USER 分别授权 TAG、DRAFT-RELEASE、PUBLISH 后，`promote-release.yml` 以独立
   `workflow_dispatch` operation 执行对应动作。每次 dispatch 显式携带发布提交 SHA、tag、Windows 候选的 workflow run id 与 artifact id、
   macOS 候选的 workflow run id、attempt 与 artifact id，以及 Release Attachment Set（`01`）：六个文件的名称与预期 SHA-256。
   TAG 用它们核对两个候选与 R；DRAFT 的授权固定这六件；PUBLISH 引用 DRAFT 批准的同一组。
4. 每个 operation 都确认两个 artifact id 分别属于指定的 run（macOS 的 artifact 名带 attempt，再核对该 attempt 的结论；Windows 沿用现行做法，
   核对 run 的结论与 artifact 名，按 attempt 收紧 Windows 另行决定），按 id 下载，逐文件比对 Release Attachment Set，
   并核对每个平台的构建来源等于发布提交或按 #platform-artifact-reuse 可沿用。TAG operation 只为发布提交创建 tag；DRAFT operation 为每个平台以其 checksum 文件列出的文件生成
   provenance attestation、为每个 ZIP 附 SBOM attestation，然后创建或更新 draft，并在上传后核对草稿上的附件恰为该集合；
   PUBLISH operation 重新下载草稿的全部附件，比对完整的文件名集合与哈希（缺件、多件、残留的旧附件都拒绝），再只把既有 draft 公开。
5. **PUBLISH 的写入约束：** GitHub 没有条件更新，workflow 只能缩小、不能消除“最后一次核对与公开之间被别人改动”的窗口，所以由 USER 在授权
   PUBLISH 时一并保证：版本 tag（`refs/tags/v*`）由仓库 ruleset 禁止更新与删除；列清能写 release 的人、App、token 与 workflow，暂停其中
   本 workflow 以外的自动写入；PUBLISH 运行期间不从网页或 CLI 手动改动草稿与 tag。workflow 自身在公开前后都按 release id 复读草稿与附件，
   并核对版本 tag 仍指向 R；公开后发现不符时让 job 失败，**不自动改回草稿**（那是另一次远程写，不在 PUBLISH 授权之内），如何处置由 USER 决定。

任一 promotion operation 都不得重新运行 Cargo build/package。artifact run id、artifact id、（macOS）attempt、构建来源或 hash 不匹配必须 fail closed；
不得自动选择“最新同 commit build”替代已验收 artifact，也不得只发布一个平台。TAG/DRAFT/PUBLISH 仍是三个独立 USER authority，
前一项批准不授权后一项。

<a id="release-attachment-set"></a>
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

下一版本起（Phase 18），同一个 Release 还附 macOS 集合（#macos-release-artifact）：`StickyMD-<version>-macos-arm64-portable.zip`、
`SHA256SUMS-macos-arm64.txt` 与 `SBOM-macos-arm64.spdx.json`。Windows 三件的文件名与字节不变，Scoop 读取的清单因此也不变。
获准的附件集合只有这六件一种（#release-attachment-set）。

- v0.1.0 不发布独立 symbols ZIP；若未来版本新增调试符号资产，必须先更新本合同、打包
  allowlist 与 exact-artifact 校验，不能把未生成的文件列为强制发布物。
- 不预创建用户 `note/`（首次运行创建）。
- v1 不提供：MSI、MSIX、Microsoft Store、自动更新器、管理员安装、
  Program Files 安装，macOS 的 pkg 安装器、DMG 与 Mac App Store。Windows 代码签名可后续加入，不阻塞开源 v1；macOS 维持 ad-hoc 签名（`02`）。
- License：MIT；数学字体为 OFL 1.1，release 必须附带相应声明。

### unsafe 边界合同

- `stickymd-core` / `stickymd-render` / `stickymd-app` / `stickymd-desktop`：`#![forbid(unsafe_code)]`（后两者自建立起）。
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
- 任一平台的发布门未过（Windows readiness 不是 `READY`，或 macOS 就绪不成立）：整个版本不发布，不只发布另一个平台。
- 草稿上的附件与 Release Attachment Set 不一致（缺件、多件、残留旧附件或哈希不符）：拒绝 DRAFT 或 PUBLISH。
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

自动更新、遥测上报、在线 CI 缓存之外的云服务、Windows x64 与 macOS arm64 之外的平台发布（Debian 交付形态另行批准）。
