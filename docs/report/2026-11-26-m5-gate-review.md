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
  - A、B、D、E 的 stderr 各有一行 `context compacted`。压缩发生在读代码途中，结论仍完整。
  - C 的第一次启动因 `timeout` 的链接失效以 127 退出，不带时限重跑后完成。
- 五份原文的 SHA-256 前缀：A `e4730e53`、B `568bd705`、C `cb57002d`、D `3732e910`、E `a949e623`。

## 属实的 BLOCKER

| # | 来源 | 问题 | 核实 | 定级理由 |
| --- | --- | --- | --- | --- |
| 1 | A1 | 忽略测试的归类表还写着旧名：`source_tests/ignored.rs:125` 是 `math_action::tests::phase11b_performance_math_action_stays_linear_in_shifted_paragraphs`，而这个测试在 `192d7a3`（#27，Comrak 1.2 升级）中已改名为 `…_after_reference_definitions`（`crates/stickymd-render/src/math_action.rs:916`） | 已核实（`rg` 与 `git log -S`） | 收据中 `stickymd-render` 库目标的 `--list --ignored` 必然列出新名字，按 `11` 判定 3（“清单列为 ignored 的每个测试都须在核对程序的归类表中……否则为 `NOT TESTED`”）源码测试行为 `NOT TESTED`；它不可 waiver，26 个 P16 源码行随之不成立。现有测试只查表内唯一与显式运行的角色，不对照真实的忽略测试 |
| 2 | A2 = E1 | 源码测试的目标发现、`--list`、主运行与显式运行都用默认构建目录（`source_tests/run.rs:269`、`:353`、`:417`、`:537`）；只有 `mac-gui` 驱动先清空 `target/macos-driver`（`:220`–`:234`） | 机制已核实：Cargo 对路径依赖按修改时间判断是否需要重建 | 只要构建目录中留着另一份源码编出、修改时间更新的产物（恢复或复制过的检出），旧测试程序就会被当作最新，其清单与结果自洽。修复轮 BLOCKER 1 对驱动的同类问题已按 honest-path 处理，这里同样处理 |
| 3 | B2 | `macos-promote --release-source <R>` 只检查 R 包含在 `origin/main` 中（`release/macos/promote.rs:114` `check_release`），不检查 HEAD 是否为干净的 R | 已核实 | `11` 第 1116 行：“源码测试与受影响的门的通过结论只认 R 上的运行……macOS 是 #macos-release-gate 的‘R 上的源码测试’与晋升的重新校验”。在别的检出上做的晋升，`macos-ready` 无从分辨：它在 R 上只重验字节，不重做晋升独有的 attempt 查询、按 artifact id 下载与对比 USER 副本 |
| 4 | D2 = E5 | `mac-gui` 中，启动产品之前的错误（会话目录、`ditto` 复制、副本可执行文件或 bundle 清单不符）都经 `Ok(Err(error)) => row.check("execution", false, error)` 记成产品 FAIL（`mac_gui/runner.rs:170`），运行照样标 `finished`；`record` 只拒绝观察恰为 `not started` 的行（`mac_ledger/record/evidence.rs:124`） | 已核实 | 例如临时卷空间不够、`ditto` 失败，就能留下 W01、F01 等人工行在候选字节上的 FAIL。`judge.rs` 的人工行在收据与 waiver 之前先看任何身份下的 FAIL（`ready/judge.rs:255`），而 `revoke` 对程序结果只撤销 PASS，所以同一候选没有文档内的恢复办法，只能换候选。R03 已把同类错误判为“机器的、未测”（`runner.rs:166`），其余行没有 |

## 降级或不成立的 BLOCKER 声明

“已核实”指我按所引位置读过代码，“未复现”指没有实际运行。降级的项仍建议修复，代价小的已标出。

| 来源 | 问题 | 核实 | 定级与理由 |
| --- | --- | --- | --- |
| A3 | 从备份恢复整个 `dist/evidence` 后，旧的 S06 PASS 重新成立 | 机制属实 | **不是 BLOCKER。** 这是对本地证据的回滚，锁与运行记录都在同一目录中，任何同目录设计都挡不住。建议把“证据目录被整体回滚”写进“不证明的前提”（改 plan，须你批准） |
| C1 = E2 | 账本缺锁时，一次失败的 `record` 先建锁，之后核对就读这份账本；核对的报错文字还建议手建空锁（`mac_ledger/run.rs:42`、`:132`、`:158`） | 机制属实 | **不是 BLOCKER。** 锁的作用是挡并发写，它证明不了账本是新的：从备份连锁一起恢复旧账本，结果相同。可作 NICE：账本已存在而锁不存在时，写端同样拒绝 |
| B1 | 第二次解析与晋升并发时，可能打印已被替换的旧候选的结论；`macos-ready` 不取晋升锁 | 已核实（晋升锁在 `promote.rs:235`，核对不取） | SHOULD_FIX：要两条命令并发。代价小：核对在最后一次解析与输出期间持晋升锁的共享锁，并把源码身份的复查移到解析之后 |
| B3 | `--system` 材料中不在词首的第二个 `ProductVersion:` 不计数（`ready/receipts.rs:98`） | 已核实 | SHOULD_FIX：`sw_vers > 文件` 写不出这种材料，要手工拼接。代价小 |
| B4 | 包检查只查 `SIL-OFL-1.1.txt` 存在，不查内容 | 已核实 | SHOULD_FIX：这是发布输入的回归，文件在仓库中受审；契约只要求“附带相应声明”。是否钉正文摘要须你定 |
| C2 | 资源证据的 R01、R02 两行，账本只剩前一行也能读入（测试 `mac_ledger/producer_tests.rs` 明确断言单行可读） | 已核实 | SHOULD_FIX：要截断或拼接账本，原子写入不会产生。代价小：读完整本后，按证据摘要核对两行成对 |
| C3 | 用 v0.2.0 的 `mac-ledger record` 导入未完成的检查点，现行门会采信其中的 PASS | 未复现 | SHOULD_FIX：要用旧版工具。修复需要记录版本标记（设计，须你定） |
| C4 | `--ledger` 选中正式锁文件，并与核对并发，可替换正在使用的锁 | 未复现 | SHOULD_FIX：要误选锁文件且并发。代价小：保留路径中加入锁文件名 |
| C5 | `--ledger dist/new/../link` 在建上级目录之后跟随符号链接 | 未复现 | SHOULD_FIX：要事先摆好指向账本的链接 |
| C6 | `automated`、`resources`、`manual` 记录忽略未知字段（`mac_ledger/record/codec.rs:115`）；契约要求“每种记录恰有其字段” | 已核实 | SHOULD_FIX：要手改账本。属契约漂移，代价小 |
| C7 | 名字段不合语法、摘要合法的镜像引用，按摘要得到身份 | 已核实（`record.rs:104` 只取摘要） | SHOULD_FIX：身份本来就由摘要决定，判定不变；契约字面要求名字不合语法时为“身份不明” |
| D1 = E3 | `mac-logout` 对原 `.app` 取 bundle 摘要后复制，只复核副本的可执行文件（`mac_gui/logout/live.rs:268`，`mac_common/fixture.rs:31`），证据写的是原件的 bundle 摘要 | 已核实 | SHOULD_FIX：要输入的 bundle 在这次运行中被改动。但它与 `mac-gui` 主路径（`runner.rs:420` 复核副本清单）和 `mac-resources` 不一致，修复只需照抄两行，冻结前应修 |
| D3 | 视图诊断的“完整”只看 `preview_frame.is_some()`，而绘制时尺寸不符的预览帧会画占位（`crates/stickymd-desktop/src/shell/render.rs:177`、`:243`） | 已核实 | SHOULD_FIX：报告的视图取自窗格布局，仍然正确；偏差只在“第一个真实帧”的时机，不改变资源行的判定 |
| D4 | 采样时不核对窗口仍在屏 | 已核实 | SHOULD_FIX：契约没有这项要求，只有产品回归才触发；是否补进契约须你定 |
| D5 | 第一次到中间一次的计数不查单调（`mac_resources/rows.rs:62` 只查中间一次到最后一次），`pid:0`、负的等待时长都可通过 | 已核实 | SHOULD_FIX：要伪造证据（同一进程的累计计数不会减少）。契约明文要求，代价小 |
| D6 | 30 s 就绪期限在产品 `spawn` 之后才开始计时 | 已核实 | NICE：差距只有返回所需的时间 |
| D7 | 时间戳在读完计数之后才取 | 未复现 | SHOULD_FIX（契约字面）：上界取自更长的实际窗口，仍偏保守，不产生错误的 PASS；最坏是一次本应作废的运行仍计入 |
| D8 | R03 仍接受产品已不再使用的旧建议“请先用访达把 StickyMD.app 移到别处”，且提示文字只按子串检查 | 已核实（产品中已无此句；`mac_gui/tests.rs:1578` 用 `any` 核对） | SHOULD_FIX：产品文字另有自己的测试（`apps/stickymd-mac/src/paths.rs:307`）。代价小：删掉旧句 |
| E4 | W12 的三条注销行可以来自同一次开机：账本不记 `boot_session` | 已核实 | SHOULD_FIX：契约要求“各在一次新开机的虚拟机中运行”，但恢复快照得到的会话状态与新开机等价。是否记录并比较 `boot_session`（改账本格式）须你定 |

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
   - 再做一次完整的整门复审。

   前几轮的经验是，每次完整复审都会有新的、越来越依赖对手的声明。
