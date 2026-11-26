# M5 macOS 发布门：冻结 R 之前的整门跨模型复审

日期：2026-11-26。审查对象：`main` 的 `82bf466`（#30 合入后，版本 0.3.0），在固定于该提交的分离检出中只读审查。依据：
[M5 报告](2026-11-24-m5-macos-readiness-check.md)“修复轮复审与合并规则”一节的 USER 决定：冻结 R 之前、代码不再变动时，对整个就绪门做一次
完整的跨模型复审，它没有 BLOCKER，R 才冻结。威胁模型：`11` #macos-readiness-check“核对取自何处”的原文（只防诚实操作者的失误）。

## 结论

**R 不能在 `82bf466` 冻结。** 按诚实操作者模型，有 4 项 BLOCKER 属实（下表 1–4）。其中第 1 项是确定的：在当前代码上，R 上的源码测试一行
必然为 `NOT TESTED`，所以 macOS 就绪不可能成立。

Codex 自评的 BLOCKER 共 27 项，去重后 23 项。其余 19 项逐项核对后降级或不成立。降级的理由大多相同：
- 要手改、截断、拼接或回滚本地证据；
- 要用旧版工具，或预先摆好非常规路径；
- 要与另一条命令并发；
- 或者契约本来就没有这项要求。

降级只是我的定级，需要你确认（见“需要 Develata 决定”）。

## 审查方式

- **五个分区并行审查。** 每个分区一次 Codex（`gpt-6.2-astra`，effort `max`，只读沙箱，可搜网）：
  - A：R 上的源码测试运行器与收据；
  - B：就绪核对、候选与包检查；
  - C：账本与发布用 USER 记录；
  - D：证据生产者（`mac-resources` 含视图诊断、R03、身份字段、产品的视图报告）；
  - E：跨生产者、账本与核对程序逐跳追踪身份字段。
- **提示词。** 共用一段说明，写明以下各项。提示词与五份原文存在 ignored 的 `dist/evidence/m5-gate-review-2026-11-26/`。
  - 审查的 SHA；
  - 契约所在行：`11` 246–401、422–613、929–1134、1540–1568，以及两张验收矩阵；
  - 威胁模型原文；
  - 每条发现须同时标注两项：是否需要对手（honest-path / needs-adversary），以及是错误接受、过度拒绝还是其他；
  - 输出不超过约 400 行。
- **运行结果。** 五次都以 0 退出，各 18–29 分钟。首尾 HEAD 都是 `82bf466`，五份都是完整的审查，不是确认回复。
  - A、B、D、E 的 stderr 各有一行 `context compacted`，即上下文在审查途中被压缩过。审查文字是完整的，但不能排除压缩丢掉了部分已读的代码，
    所以这四个分区的覆盖可能比各自的“Not covered”写的更少。
  - C 的第一次启动因 `timeout` 的链接失效以 127 退出，不带时限重跑后完成。
- 五份原文的 SHA-256 前缀：A `e4730e53`、B `568bd705`、C `cb57002d`、D `3732e910`、E `a949e623`。

## 属实的 BLOCKER

| # | 来源 | 问题 | 核实 | 定级理由 |
| --- | --- | --- | --- | --- |
| 1 | A1 | 忽略测试的归类表还写着旧名：`source_tests/ignored.rs:125` 是 `math_action::tests::phase11b_performance_math_action_stays_linear_in_shifted_paragraphs`，而这个测试在 `192d7a3`（#27，Comrak 1.2 升级）中已改名为 `…_after_reference_definitions`（`crates/stickymd-render/src/math_action.rs:916`） | 已核实并实测：`git log -S` 找到改名的提交；`cargo test --locked -p stickymd-render --lib -- --list --ignored --format terse` 列出的是新名字 | 收据中 `stickymd-render` 库目标的 `--list --ignored` 必然列出新名字，按 `11` 判定 3（“清单列为 ignored 的每个测试都须在核对程序的归类表中……否则为 `NOT TESTED`”）源码测试行为 `NOT TESTED`；它不可 waiver，26 个 P16 源码行随之不成立。现有测试只查表内唯一与显式运行的角色，不对照真实的忽略测试 |
| 2 | A2 = E1 | 源码测试的目标发现、`--list`、主运行与显式运行都用默认构建目录（`source_tests/run.rs:269`、`:353`、`:417`、`:537`）；只有 `mac-gui` 驱动先清空 `target/macos-driver`（`:220`–`:234`） | 已核实：规定命令 `source_tests.rs:169` 不带 `--target-dir`，运行器也不设 `CARGO_TARGET_DIR`（`run.rs` 只设 `CARGO_TERM_COLOR` 与卷报告）；Cargo 对路径依赖按修改时间判断是否需要重建（据 Codex 所引 Cargo 源码，未实测） | 只要构建目录中留着另一份源码编出、修改时间更新的产物（恢复或复制过的检出），旧测试程序就会被当作最新，其清单与结果自洽。修复轮 BLOCKER 1 对驱动的同类问题已按 honest-path 处理，这里同样处理 |
| 3 | B2 | `macos-promote --release-source <R>` 只检查 R 包含在 `origin/main` 中（`release/macos/promote.rs:114` `check_release`），不检查 HEAD 是否为干净的 R | 已核实：入口 `promote.rs:303` 只调用 `check_release`、取晋升锁，再由 `checked_attempt`（`:268`）核对 GitHub 上 attempt 的来源与结论，全程不读本地 HEAD 与工作树 | `11` 第 1116 行：“源码测试与受影响的门的通过结论只认 R 上的运行……macOS 是 #macos-release-gate 的‘R 上的源码测试’与晋升的重新校验”。在别的检出上做的晋升，`macos-ready` 无从分辨：它在 R 上只重验字节，不重做晋升独有的 attempt 查询、按 artifact id 下载与对比 USER 副本 |
| 4 | D2 = E5 | `mac-gui` 中，启动产品之前的错误（会话目录、`ditto` 复制、副本可执行文件或 bundle 清单不符）都经 `Ok(Err(error)) => row.check("execution", false, error)` 记成产品 FAIL（`mac_gui/runner.rs:170`），运行照样标 `finished`；`record` 只拒绝观察恰为 `not started` 的行（`mac_ledger/record/evidence.rs:124`） | 已核实 | 例如临时卷空间不够、`ditto` 失败，就能留下 W01、F01 等人工行在候选字节上的 FAIL。`judge.rs` 的人工行在收据与 waiver 之前先看任何身份下的 FAIL（`ready/judge.rs:255`），而 `revoke` 对程序结果只撤销 PASS，所以同一候选没有文档内的恢复办法，只能换候选。R03 已把同类错误判为“机器的、未测”（`runner.rs:166`），其余行没有 |

## 降级或不成立的 BLOCKER 声明

“已核实”指我按所引位置读过代码，“未核实”指只有 Codex 的原文；除特别注明外都没有实际运行。降级的项仍建议修复，代价小的已标出。

| 来源 | 问题 | 核实 | 定级与理由 |
| --- | --- | --- | --- |
| A3 | 从备份恢复整个 `dist/evidence` 后，旧的 S06 PASS 重新成立 | 未核实（据 Codex 原文；锁、运行记录与收据同在 `dist/evidence` 下是已知设计） | **不是 BLOCKER。** 这是对本地证据的回滚，锁与运行记录都在同一目录中，任何同目录设计都挡不住。建议把“证据目录被整体回滚”写进“不证明的前提”（改 plan，须你批准） |
| C1 = E2 | 账本缺锁时，一次失败的 `record` 先建锁，之后核对就读这份账本；核对的报错文字还建议手建空锁（`mac_ledger/run.rs:42`、`:132`、`:158`） | 机制属实 | **不是 BLOCKER。** 锁的作用是挡并发写，它证明不了账本是新的：从备份连锁一起恢复旧账本，结果相同。可作 NICE：账本已存在而锁不存在时，写端同样拒绝 |
| B1 | 第二次解析与晋升并发时，可能打印已被替换的旧候选的结论；`macos-ready` 不取晋升锁 | 已核实（晋升锁在 `promote.rs:235`，核对不取） | SHOULD_FIX：要两条命令并发。代价小：核对在最后一次解析与输出期间持晋升锁的共享锁，并把源码身份的复查移到解析之后 |
| B3 | `--system` 材料中不在词首的第二个 `ProductVersion:` 不计数（`ready/receipts.rs:98` 按空白分词，只数以标签开头的词） | 已核实 | SHOULD_FIX：`sw_vers > 文件` 写不出这种材料，要手工拼接。代价小 |
| B4 | 包检查只查 `SIL-OFL-1.1.txt` 存在，不查内容 | 已核实：打包时把仓库文件规范化后写入（`tools/stickymd-package/src/macos/package.rs:254`），解包检查只比对成员名单（`content::check_files`） | SHOULD_FIX：这是发布输入的回归，文件在仓库中受审；契约只要求“附带相应声明”。是否钉正文摘要须你定 |
| C2 | 资源证据的 R01、R02 两行，账本只剩前一行也能读入（测试 `mac_ledger/producer_tests.rs` 明确断言单行可读） | 已核实 | SHOULD_FIX：要截断或拼接账本，原子写入不会产生。代价小：读完整本后，按证据摘要核对两行成对 |
| C3 | 用 v0.2.0 的 `mac-ledger record` 导入未完成的检查点，现行门会采信其中的 PASS | 未核实（据 Codex 原文） | SHOULD_FIX：要用旧版工具。修复需要记录版本标记（设计，须你定） |
| C4 | `--ledger` 选中正式锁文件，并与核对并发，可替换正在使用的锁 | 未核实（据 Codex 原文） | SHOULD_FIX：要误选锁文件且并发。代价小：保留路径中加入锁文件名 |
| C5 | `--ledger dist/new/../link` 在建上级目录之后跟随符号链接 | 未核实（据 Codex 原文） | SHOULD_FIX：要事先摆好指向账本的链接 |
| C6 | `automated`、`resources`、`manual` 记录忽略未知字段（`mac_ledger/record/codec.rs:115` 起逐个取已知字段，不核对字段集合）；契约要求“每种记录恰有其字段” | 已核实 | SHOULD_FIX：要手改账本。属契约漂移，代价小 |
| C7 | 名字段不合语法、摘要合法的镜像引用，按摘要得到身份 | 已核实（`record.rs:104` 只取摘要） | SHOULD_FIX：身份本来就由摘要决定，判定不变；契约字面要求名字不合语法时为“身份不明” |
| D1 = E3 | `mac-logout` 对原 `.app` 取 bundle 摘要后复制，只复核副本的可执行文件（`mac_gui/logout/live.rs:268`，`mac_common/fixture.rs:31`），证据写的是原件的 bundle 摘要 | 已核实 | SHOULD_FIX：要输入的 bundle 在这次运行中被改动。但它与 `mac-gui` 主路径（`runner.rs:420` 复核副本清单）和 `mac-resources` 不一致，修复只需照抄两行，冻结前应修 |
| D3 | 视图诊断的“完整”只看 `preview_frame.is_some()`，而绘制时尺寸不符的预览帧会画占位（`crates/stickymd-desktop/src/shell/render.rs:177`、`:243`） | 已核实 | SHOULD_FIX：报告的视图取自窗格布局，仍然正确；偏差只在“第一个真实帧”的时机，不改变资源行的判定 |
| D4 | 采样时不核对窗口仍在屏 | 已核实：`mac_resources/runner.rs:393` 只对 Hidden 调用 `stays_hidden`，可见场景在采样时不查窗口 | SHOULD_FIX：契约没有这项要求，只有产品回归才触发；是否补进契约须你定 |
| D5 | 第一次到中间一次的计数不查单调（`mac_resources/rows.rs:62` 只查中间一次到最后一次），`pid:0`、负的等待时长都可通过 | 已核实 | SHOULD_FIX：要伪造证据（同一进程的累计计数不会减少）。契约明文要求，代价小 |
| D6 | 30 s 就绪期限在产品 `spawn` 之后才开始计时 | 已核实：`spawn` 在 `mac_resources/runner.rs:214`，计时起点 `started = Instant::now()` 在 `:343` | NICE：两者之间只隔一次函数返回（推断，未测） |
| D7 | 时间戳在读完计数之后才取 | 未核实（据 Codex 原文） | SHOULD_FIX（契约字面）。以下是推断：上界取自更长的实际窗口，仍偏保守，不产生错误的 PASS；但一次本应作废的运行可能仍计入，下界也可能包含超过 60 s 的 CPU，造成错误的 FAIL（过度拒绝） |
| D8 | R03 仍接受产品已不再使用的旧建议“请先用访达把 StickyMD.app 移到别处”，且提示文字只按子串检查 | 已核实（产品中已无此句；`mac_gui/tests.rs:1578` 用 `any` 核对） | SHOULD_FIX：产品文字另有自己的测试（`apps/stickymd-mac/src/paths.rs:307`）。代价小：删掉旧句 |
| E4 | W12 的三条注销行可以来自同一次开机：账本不记 `boot_session` | 已核实：证据写 `boot_session`（`mac_gui/logout/live.rs:305`），`record/evidence.rs` 只取 `case`，`record/codec.rs` 没有这个字段 | SHOULD_FIX：契约要求“各在一次新开机的虚拟机中运行”。恢复快照得到的会话状态与新开机等价，这一点是推断，未验证。是否记录并比较 `boot_session`（改账本格式）须你定 |

## 其余发现（SHOULD_FIX、NICE、NEEDS_VERIFICATION）

以下各项**未逐条核实**，只按 Codex 原文归档，修复时逐项核对：
- **A 分区：**
  - A4 首次运行的两个进程竞争时删掉胜者的运行记录；
  - A5 串行 libtest 的结果行被 Git 的提示拆开；
  - A6 失败诊断中嵌套的 `test result:`；
  - A7 只有 `--list --ignored` 失败时丢掉已打印的 FAILED；
  - A8 第二组文档测试截断时仍发布第一组的计数；
  - A9 两个坏样本没有单独检验各自的条件；
  - A10 `run` 格式过宽。
- **B 分区：**
  - B5 账本路径检查出错后仍打印逐行结论；
  - B6（= C8）另一个大版本上补记的下载收据遮住原版本的收据；
  - B7 本地 SBOM 与 `macos-verify-artifact` 可混用两个提交；
  - B8 SPDX 正样本本身不合 2.3；
  - B9 再次晋升会删除暂存 `.app` 旁用户写下的便签；
  - B10 PUBLISH 不核对发布说明中的构建来源；
  - B11 矩阵只改行序也使核对退出。
- **C 分区：**
  - C9 旧镜像标签冲突使整本账本失效；
  - C10 按候选的撤销不记桌面；
  - C11（= E7）`receipt --app` 静默忽略 `--candidate-sha256`；
  - C12 `P16-P18-D01` 被剥成 `P18-D01`；
  - C13 schema 4 资源证据的拒绝原因不具体；
  - C14 D04、D05 单行限制的两种读法（NEEDS_VERIFICATION）。
- **D 分区：**
  - D9 `mac-latency` 逐轮不复核副本清单；
  - D10 R03 最终读不到 stderr 时当作空文本；
  - D11 R03 第一次 `open` 出错时跳过清理；
  - D12 R03 重判接受零项登记（needs-adversary）；
  - D13 视图报告写失败时 `eprintln!` 可能 panic；
  - D14 下载目录中的 ZIP 没有再核对成员名单。
- **E 分区：**
  - E6 合法批次名 `-h` 能进证据，却不能登记同批收据；
  - E8 README 把资源行跨组 FAIL 的优先级写反了。

其中 B9（自动删除用户的便签）与 D13（诊断开启时产品可能 panic）虽不改变门的判定，但触及 `AGENTS.md` 的数据与稳定性禁令，建议与 BLOCKER 一起修。

## 覆盖范围

**各分区自述未审的部分**（据原文）：
- 全部五次都没有构建、运行测试、启动虚拟机或触发 workflow，也没有读取实际的候选 ZIP、账本与 USER 收据。
- A：没有实测运行器的并发交错，也没有取得两个宿主进程互斥、提示后退出的现场证据。
- B：A、C、D、E 的实现只沿 B 的调用链看过；没有动态复现并发时序。
- C：16 个文件都审过；没有在真实 APFS、虚拟机或候选上复现。
- D：`mac_gui/latency/runner.rs` 与 `mac_gui/logout/live.rs` 只审到字节身份与结果归属，延迟模型与三种注销场景的业务断言没有完整审计。
- E：没有穷举以下各项：
  - `source_tests/output.rs` 的全部 libtest 文本形式；
  - `mac_resources/rows.rs` 的数值边界；
  - `translocation/live.rs` 的 Darwin 事件行为；
  - SBOM 校验器的完整 SPDX 语义。

  `#release-attachment-set` 只核对了 macOS 三件附件与两边就绪的关系，没有审 Windows readiness，也没有审远端六件附件的发布事务。

**没有分配给任何分区的部分：**
- `mac-gui` 中 W02–W12、K02、E01 各场景本身的断言（`mac_gui/scenarios.rs`、`hover.rs`、`editing.rs`、`lifecycle.rs` 等）。
  它们决定一条 PASS 是否名副其实，但属于 Phase 16 的产品验收，不是门的判定逻辑。
- `mac-latency` 的模型。
- 平台构建输入指纹与沿用规则中，晋升与 `macos-ready` 不调用的部分（`release/platform_inputs*` 的其余代码；已在 #20、#21 审过）。

## 需要 Develata 决定

1. **定级。** 是否接受上表的降级（19 项不算 BLOCKER）。
2. **修复范围。**
   - 建议本轮修：4 项 BLOCKER；代价小的 SHOULD_FIX，即 B1、B3、C2、C4、C6、D1、D5、D8，加上 B9、D13；每项先有坏样本。
   - 要你另定的：B4（钉 OFL 摘要）、C3（记录版本标记）、D4（采样时查窗口）、E4（记 `boot_session`）、A3（把回滚写进“不证明的前提”）。
     这几项都要改 plan 或账本格式。其余留作跟进。
3. **BLOCKER 3 的修法。** 晋升要求 HEAD 为干净的 R，并且 `macos-ready` 要能看出这次晋升做在 R 上。候选收据的字节身份不应随 R 改变，所以建议另写一份
   “R 上的晋升复核”记录，绑定 R 与候选收据的摘要，核对时读它。这改变 `11` #macos-release-artifact 的流程，属于须你批准的合同改动。
4. **修复之后的复审。** 按“完整复审没有 BLOCKER 才冻结 R”的规则，本轮不满足。可以选：
   - 修完只审修复的差异，没有 BLOCKER 即冻结；
   - 修完审修复的差异，同时补审上节“没有分配给任何分区的部分”中属于门的代码（沿用规则的其余部分）；
   - 再做一次完整的整门复审。

   建议第二种：完整复审已经做过，按上述威胁模型确认的 BLOCKER 只有 4 项；而前几轮的经验是，每次完整复审都会带来新的、越来越依赖对手的声明。
   代价是：本轮压缩过的四个分区如果漏看了什么，修复差异的审查也补不回来。

## USER 决定（2026-11-26）

Develata 在本会话中对上节四项都选了推荐的一项：
1. 定级：接受（4 项 BLOCKER，其余按上表降级）。
2. 修复范围：4 项 BLOCKER，加 B1、B3、C2、C4、C6、D1、D5、D8、B9、D13；每项先有坏样本。B4、C3、D4、E4、A3 与其余各项留作跟进。
3. BLOCKER 3：另写一份“R 上的晋升复核”记录，绑定 R 与候选收据的摘要，由 `macos-ready` 读取；相应修改 `11` #macos-release-artifact 与
   #macos-readiness-check。
4. 修复之后：审修复的差异，同时补审本轮没有分配给任何分区的门代码（沿用规则的其余部分）；没有 BLOCKER 即冻结 R。

## 修复轮（2026-11-26）

`337cfc2` 先写下本轮的契约改动，`a9164ec` 改正了 B9 的措辞。修复分四组并行，各在自己的分支上实现，合入 `fix/m5-gate-review`：
- 源码测试：A1、A2；
- 核对与晋升：B1、B2、B3、B9；
- 账本：C2、C4、C6；
- 证据生产者：D1、D2、D5、D8、D13。

每项都先有坏样本，并做过删去守卫的检查。`935546f` 对齐了矩阵的行号，并引用了新样本。

**验证**（本机 macOS 27.2 arm64，`935546f`）：
- fmt 通过；clippy `-D warnings` 在八个 macOS 包与 `stickymd-smoke --target x86_64-pc-windows-msvc` 上都没有警告。
- 八个包的 `cargo test --locked`：2129 个通过，0 个失败，33 个忽略。
- `phase 00` 与 `phase 16` 通过。
- `phase 18`：四个任务都 PASSED，其中包括新的忽略测试归类表核对；收尾只因 `main` 上已有的 P18-P04 `NOT TESTED` 而失败。
- 运行器的真实演练（不是正式收据）：`release macos-source-tests --release-source 935546f…`，源码测试、同键互斥与 S06 三项都为 PASS。
  - 收据：23 个目标，2129 个通过，33 个忽略，`problems` 为空，首尾都是干净的 `935546f`。

## 修复差异的复审（2026-11-26，`935546f`）

按 USER 决定 4，复审分三个分区，都是 Codex（`gpt-6.2-astra`，effort `max`），在固定于 `935546f` 的分离检出中只读进行：
- 1：源码测试、核对程序与晋升的修复；
- 2：账本与证据生产者的修复；
- 3：补审沿用规则。

三次都以 0 退出，各 21–28 分钟。1 与 3 的 stderr 各有一行 `context compacted`，2 没有。提示词与原文在
`dist/evidence/m5-gate-review-2026-11-26/r2-*`，SHA-256 前缀：1 `90b681c6`，2 `96fb466e`，3 `1ab5b5e7`。

**结果：** Codex 自评共 12 项 BLOCKER，其中 1 有 5 项、2 有 2 项、3 有 5 项，另有 14 项 SHOULD_FIX 与 1 项 NICE。按诚实操作者模型定级如下。
“已核实”指我读过所引的代码。

| 来源 | 问题 | 核实 | 定级与理由 | 修法与代价 |
| --- | --- | --- | --- | --- |
| R1-4 | B3 的修复用 `split_once("ProductVersion:")`，把 `TargetProductVersion:` 也当作标签；旧实现反而拒绝这种材料 | 已核实（`ready/receipts.rs:101`） | 本轮修复带进来的回退。要选错 `--system` 文件才会出现，但这是修复使接受变宽 | 便宜：标签前须是字段名的边界 |
| R2-2 | D8 只核对提示行的开头，两段建议各按子串查找，所以否定这两段建议的句子也 PASS | 已核实（`mac_gui/translocation.rs:227`） | 要一次刻意改写的产品回归才触发 | 便宜：与 `05` 第 130 行的原句整句比较 |
| R2-1 | 旧驱动（修复之前构建的）在启动之前出错时，写的是 `execution` FAIL 并标 `finished`，现在的 `record` 仍会登记 | 已核实（旧 `fixture.rs:29`、`:33`，旧 `runner.rs:418` 的固定文字） | 两个失误叠加才发生：用错了驱动，又碰上复制失败。属过度拒绝 | 便宜：按旧驱动固定的报错文字拒绝登记 |
| R1-1 | 第 3–5 步只设 `CARGO_TARGET_DIR`，继承了环境中的 `CARGO_BUILD_BUILD_DIR`（仓库别处已经显式设了它） | 已核实 | 陈旧缓存一类：要环境中设了这个变量，且构建缓存中留着修改时间更新的旧产物 | 便宜：一并设到清空的目录中 |
| R1-2 | 文档的 `cargo run` 入口可能从陈旧的默认构建目录启动旧版核对程序 | 未核实 | 陈旧缓存一类（同上） | 不能靠工具自身解决（第一次调用必须可信）；建议写成“不证明的前提” |
| R1-5 | `phase 18` 的归类表核对用默认构建目录；成员比对没有按次数比较 | 未核实 | 前一半属陈旧缓存一类；重复的名字在 R 上的收据判定会被拒 | 便宜：比较排序后的完整序列 |
| R1-3 | B9 按文件名的模式认定 ZIP，也不查 `StickyMD.app` 是不是目录 | 已核实（`promote/install.rs:169`） | 要在 staging 里放一个合乎这个模式的 ZIP | 便宜：只认旧 receipt 中那一个 ZIP，`.app` 须为目录 |
| R3-1 | `origin/main` 以短名称传给 Git，同名的本地分支或标签会先被解析 | 未核实 | 要有一个名为 `origin/main` 的本地分支 | 便宜：用 `refs/remotes/origin/main` |
| R3-2 | 独立的 `verify-reuse` 不核对检出是干净的 R，Windows 的发布清单采信它的结论 | 未核实 | 与 B2 同类：在错的检出上运行 | 中等：Windows 侧要一个可移植的源码身份核对 |
| R3-3 | 分类器文件若改名（如移到 `platform_inputs/mod.rs`），就会从两个平台的指纹中消失 | 未核实 | 要先有这样的重构；当前的树没有 | 便宜：在每棵树中核对分类器路径存在 |
| R3-4 | Windows 的沿用判定对两个相同的 SHA 不检查提交是否存在 | 未核实 | 要把账本恢复到缺少旧提交的检出 | 便宜 |
| R3-5 | 恢复的证据目录中，断开的沿用记录链接会让晋升写下成功的复核记录，而 resolver 拒绝这个候选 | 未核实 | 门本身会拒绝，不是错误接受 | 便宜 |

SHOULD_FIX 中代价小、建议同轮修的有：
- R1-6 第一次晋升时的无锁窗口；
- R1-7 清空目录时上级的符号链接；
- R1-8 首尾快照把“取不到摘要”当作相等；
- R1-9 复核记录的路径应列入保留路径；
- R1-11 保留下来的 staging 与 PID 撞名；
- R2-3 `--ledger` 的上级目录名以 `.lock` 结尾；
- R2-5 资源证据被第三条程序结果引用；
- R2-7 PID 超出 `u32`；
- R1-12 矩阵行号。

其余留作跟进：
- R1-10 复核记录的空白与成员顺序；
- R2-4 启动之前的 panic 在检查点中记 FAIL；
- R2-6 菜单等待的下限；
- R3-6 浅检出；
- R3-7 排除目录中的 `.gitattributes`。

**判断：** 这一轮没有出现与第一轮那 4 项同等分量的问题，即对诚实操作者确定发生、无需叠加条件的错误判定。新增的 BLOCKER 声明，大多是“陈旧的构建缓存”这一类，或要求非常规的输入、
路径或引用名。与 2026-11-24 的几轮一样，每轮审查都会在相邻的代码中找到新的、越来越依赖特殊条件的声明。

## USER 决定（2026-11-26，第二轮）

Develata 对四项都选了推荐的一项：
1. 再修一轮就冻结：修上表中标为“便宜”的各项，以及上文列出的 SHOULD_FIX，每项先有坏样本；全量基线与运行器演练通过即合并、冻结 R，
   不再做跨模型复审；其余留作跟进。
2. 把陈旧的构建缓存写进 `11` 的“不证明的前提”；运行器第 3–5 步与驱动的清空目录保留，并一并设 `CARGO_BUILD_BUILD_DIR`。
3. R3-2（独立的 `verify-reuse` 核对干净的 R）本轮修。
4. 回收已合并的 `StickyMD-wt-fix-src-tests` worktree 及其分支，其中 `172f542` 上的演练收据随之删除。

## 第二轮修复（2026-11-26）

`419e4a5` 先写下这一轮的契约改动：
- 陈旧构建缓存写为“不证明的前提”；
- R1-1、R1-7、R2-1、R2-2，以及 R3-1 至 R3-4 各自的契约文字。

修复分四组，合入 `fix/m5-gate-review`：
- 源码测试与沿用规则：R1-1、R1-5、R1-7、R3-2、R3-3、R3-4；
- 核对与晋升：R1-3、R1-4、R1-6、R1-8、R1-9、R1-11、R3-1、R3-5；
- 账本：R2-1、R2-3、R2-5；
- 证据生产者：R2-2、R2-7。

每项先有坏样本，并做过删去守卫的检查。另外：
- `93d5c03` 改正了 README 中资源行跨组 FAIL 的优先级（E8）。
- `fb4ad6a` 对齐了矩阵的行号并引用新样本，改正了源码身份移到 `release/source_identity.rs` 后失效的链接，以及卷报告错位的引用（R1-12）。

**验证**（本机 macOS 27.2 arm64，`fb4ad6a`）：
- fmt 通过；clippy `-D warnings` 在八个 macOS 包与 `stickymd-smoke --target x86_64-pc-windows-msvc` 上都没有警告。
- 八个包的 `cargo test --locked`：2143 个通过，0 个失败，33 个忽略。
- `phase 00` 与 `phase 16` 通过。
- `phase 18`：四个任务都 PASSED，收尾只因 P18-P04 的 `NOT TESTED` 而失败。
- 运行器的真实演练（不是正式收据）：源码测试、同键互斥与 S06 都为 PASS。
  - 收据：23 个目标，2143 个通过，33 个忽略，`problems` 为空，首尾都是干净的 `fb4ad6a`。
  - 驱动命令以 `CARGO_BUILD_BUILD_DIR=target/macos-driver/build` 开头。
- Windows 与 Linux 上的测试只由 CI 运行，包括 R3-2、R3-3、R3-4 的样本，以及只在 Windows 编译的几个 `reuse_tests`。

**按 USER 决定 1（第二轮）**，这一轮不再做跨模型复审；CI 通过、合并之后，R 即可冻结。

## 冻结 R 之后的跟进

以下各项不阻塞本版，留作跟进：
- **第一轮留下的：**
  - B4 钉 OFL 正文的摘要；
  - C3 账本记录的版本标记；
  - D4 采样时核对窗口在屏；
  - E4 记录并比较 `boot_session`；
  - A3 把证据目录被整体回滚写进前提；
  - 以及“其余发现”一节中没有在本轮修的各项。
- **第二轮留下的：**
  - R1-10 复核记录的空白与成员顺序；
  - R2-4 启动之前的 panic 在检查点中记 FAIL；
  - R2-6 菜单等待的下限；
  - R3-6 浅检出；
  - R3-7 排除目录中的 `.gitattributes`；
  - 三个发布 workflow 仍以短名称 `origin/main` 调用 Git（在 CI 的新检出中没有同名的本地分支）。
- **修复中发现的：**
  - CI 的 macOS 作业不运行 `phase 18`，A1 的归类表核对目前只在本机运行 `phase 18` 时生效；
  - `docs/release-checklist.md` 的 Windows 一步称 `verify-reuse` 会写下 `dist/evidence/release-reuse-windows-x64.json`，而这个命令只打印（本轮之前就如此）；
  - `mac_ledger` 未要求 `automated` 行号必须有生产者（C6 的扩展）。

## 更正：两轮修复的验证范围（2026-11-26）

上文两处“每项都先有坏样本，并做过删去守卫的检查”说得过满。

**由各组子代理实施并报告、我没有逐项重做的：** 坏样本与删去守卫的检查。按其报告，有这些例外：
- D1、D2、D13 与 D8 的判定样本在修复前没有一次失败的运行，只经删去守卫的检查验证；
- R1-11 的后半（只清理本次移开的目录）删去守卫后仍为绿，那一半样本只固定行为；
- R1-8 中 `receipt_present` 的读取错误使核对停止，这一点没有样本；
- 账本追加之前对整本做的成对核对（`run.rs`）没有样本，因为 `record` 总是一次写两行；
- B1 中“锁一直持到打印之后”与“先候选、后源码身份”的顺序，都没有删去守卫的检查。

**我亲自做的：**
- 合并后的全量基线与运行器演练（各轮“验证”所列）；
- 用 `fb4ad6a` 之后的读取器读真实账本的副本：337 行全部读入，原账本前后都是 `6feb4e7e…`；
- 确认 `verify-reuse` 与 `refs/remotes/origin/main` 的调用方都在干净的 R 上、引用存在的环境中运行：发布 workflow 都以 `fetch-depth: 0` 检出，`verify-reuse` 只由发布清单在本机调用；
- 读过这些会改变判定的函数：第一轮的 `steps.rs` 与 `resource_pairs`，第二轮的 `major_version`、`old_pre_launch`、`resource_pairs`、`on_main`、`foreign`、`candidate::confirm`，以及 `verify-reuse` 的身份核对。

`d12d29e` 把第二轮已实现、`419e4a5` 没有写进契约的规则补进了 `11`：R1-3、R1-4、R1-6、R1-8、R1-9、R1-11、R2-3、R2-5、R3-5。
