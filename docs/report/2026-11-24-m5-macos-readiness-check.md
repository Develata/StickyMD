# M5 macOS 就绪核对：实现范围与待决事项

日期：2026-11-24。分支 `feat/m5-macos-readiness`（从 `main` `d9bab10` 开出），PR [#23](https://github.com/Develata/StickyMD/pull/23)。依据：`11` Phase 18 节 M5 行与
#macos-release-gate；[Phase 18 验证记录](2026-11-20-phase-18-verification.md)、[构建输入收窄记录](2026-11-21-platform-build-input-narrowing.md)。

## 结论

下文写于 USER 决定之前；决定与之后的实现见文末“Resolution（2026-11-24）”，届时只剩 R03 与资源行的生产者改动未落地；它们随后在另一分支落地，见文末“生产者改动”。

M5 不只是一个核对程序：发布门七类中有三类今天没有任何生产者能给出门要求的字节身份，另有两类收据的格式与记录方式合同没有交给 M5。
本分支只实现合同已确定或明确交给 M5 的部分（“R 上的源码测试”收据与其运行器、逐行核对的程序本身）；其余各项列在“需要 Develata 决定”，
核对程序对它们判 `NOT TESTED` 并写明缺什么，不猜格式。因此，**在这些事项落地之前，macOS 就绪不可能成立**，这是 fail closed 的预期结果。

## 发布门各行的证据现状

| 门的类别 | 证据与身份字段 | 今天的生产者 | 格式 |
| --- | --- | --- | --- |
| 包完整性 | `dist/evidence/release-candidate-macos.json`；resolver 重验暂存字节 | 有（`release macos-promote`） | `11` #macos-release-artifact 已定 |
| 同键互斥（P18-L08） | R 上两种卷各自的结果 | 无 | 未定 → 本分支并入源码测试收据（见下） |
| R03 | 本次发布的 ZIP 的 SHA-256 | **无**：探针把被测 `.app` 自己打成 ZIP（`mac_gui/translocation.rs` 中的 `ditto -c -k --keepParent`），`mac-gui` 没有 ZIP 输入，证据不记 ZIP 摘要——正是门点名的坏样本 | 未定 |
| 真实下载路径（P18-D01–D05） | 做那次的 ZIP | **无**记录器 | 未定 |
| Phase 16 依赖 app 运行的行 | bundle 清单与可执行文件两个 SHA-256 | `mac-gui` 与 `mac-logout`：有（账本记两项）；`mac-resources`：**只记可执行文件**（schema 4），不进账本；`mac-latency`：记两项但不进账本；账本的人工收据：**只记可执行文件与产品输入指纹**，门说这不够 | 发布用收据与 waiver 未定 |
| Phase 16 不依赖发布字节的行 | 源码身份 R | 无 | 由源码测试收据承担 |
| R 上的源码测试 | R、主机、完整命令、每个测试二进制的数目、开始与结束的源码身份 | 无 | **合同交给 M5**（本分支定义） |

## 本分支实现的部分

1. **R 上的源码测试收据**（格式写进 `11` #macos-release-gate）：`stickymd-smoke release macos-source-tests --release-source <R>` 在 macOS 主机上
   核对 HEAD 等于 R 且工作树干净，以 `--no-run --message-format=json` 与 `cargo metadata` 求出应运行的测试二进制与文档测试，再运行合同规定的那一条
   `cargo test --locked`（八个包，不加过滤），按二进制记下通过、失败、忽略、测量与过滤掉的数目；同键互斥的两个测试（其中一个 `#[ignore]`）以
   `--exact --include-ignored` 显式运行并单独记下；结束时再核对源码身份，然后写 `dist/evidence/release-source-tests-macos.json`。
   核对一方只认事实：命令逐字等于规定的命令、二进制集合等于发现的集合、八个包都在、没有失败与过滤、两种卷的测试都通过、开始与结束都是 R 且干净。
2. **逐行核对**：`stickymd-smoke release macos-ready --release-source <R> [--batch <名>]` 经 resolver 取得候选（resolver 改为返回它读到的候选事实与
   receipt 摘要），读账本与源码测试收据，按 Phase 16 每一行的分类判定，打印逐行结论；任一行不是 PASS（或可 waiver 行的 waiver）即失败退出。
   判定是纯函数，所有平台都编译测试，坏样本逐项改一个事实并断言行与原因。
3. **Phase 16 行的分类**是一张数据表，测试核对它恰好覆盖矩阵中的全部 P16 行，且分类与各行的 Mode 一致（Mode 一改，测试就要求重新分类）。

本分支中由我定下、待你确认（可以否决）的四点：

- **Windows 行不进 macOS 门**：P16-W15、W16（USER 豁免的 Windows 回归）与 V05（Windows 回归的人工收据）判为“不属于 macOS 发布物”，由 Windows
  readiness 管；P16-P01（打包脚本产出 ad-hoc 签名的 `.app`）由门的“包完整性”一行承担。
- **账本中同一字节上的 FAIL 与撤销**：沿用账本的规则，只把“产品输入指纹”换成“候选的两项字节身份”。一行在候选字节上、同一驱动、镜像与系统下
  有 PASS，且这组身份下没有 FAIL、NOT_TESTED 或被撤销的 PASS，才算 PASS；I01–I03 与 R03 只认 `--batch` 所指的批次。理由同账本：先后无法证明，
  所以任何这类记录都算数。
- **同键互斥并入源码测试收据**：门对这一行的证据只写“源码身份 R，两种卷各自的结果”，没有格式；运行器在同一次运行中以 `--exact --include-ignored`
  显式跑这两个测试，结果作为收据的 `same_key` 一节，与源码测试共用开始与结束的源码身份。
- **核对程序只在 R 的干净检出中运行**：`macos-ready` 在 HEAD 不是 R 或工作树不干净时拒绝，使门的分类表与规则就是 R 的工具（`11`
  #platform-artifact-reuse“沿用后”：受影响的门用 R 的工具重跑）。代价：不能在别的检出上预览一个 R 的就绪结论。

## 需要 Develata 决定

1. **发布用 USER 收据与 waiver 收据的格式与存放（门要求，合同未交给 M5）。** 建议：扩展现有账本，而不是另开一个文件——账本已有排他锁、原子追加、
   `--quote` 与撤销。具体为：`mac-ledger receipt` 增加 `--release-source <R>`，经 resolver 取得当前候选，在收据中写 `candidate_receipt_sha256`
   （不手工输入摘要）；新增 `mac-ledger waive --release-source <R> --rows <ids> --quote <原话>`，记录种类 `waiver`，同样绑定候选 receipt 摘要，
   拒绝不可 waiver 的行（R03 与门中标“不可”的类别）。撤销沿用 `revoke`，按候选摘要指名。代价：账本多一种记录，旧版工具读到新记录会拒绝整本账本
   （工具随仓库走，影响限于回退到旧提交时）。替代方案是另开 `dist/evidence/macos-release-receipts.jsonl`，代价是 USER 收据分散在两处。
2. **真实下载路径的记录（P18-D01–D05，本版不可 waiver）。** 建议：同一个账本记录器，行名用 `P18-D01`…`D05`，绑定候选 receipt 摘要（它已绑定 ZIP
   摘要），并记下签名方式与 macOS 大版本，供以后判断“签名方式或大版本改变时重做”。本分支对后者 fail closed：只认绑定本候选的记录。
3. **生产侧缺口（不在 M5 核对之内，但门在它们之前无法成立）：**
   - R03 探针接收候选 ZIP（`mac-gui --zip`，不再从 `.app` 打包），核对 ZIP 中的 bundle 与 `--app` 相同，并把 ZIP 的 SHA-256 写进证据与账本；
   - `mac-resources` 记 bundle 清单摘要，并与 `mac-latency` 一样能登记进账本；或者你对 P16-R01、R02、V03 逐行 waiver。
   建议作为 M5 之后、冻结 R 之前的独立一步（各自改证据格式与账本），每项先做坏样本。
4. **这些事项的顺序**：1 与 2 是核对程序的输入格式，定了之后在本分支补上记录器与判定（小）；3 是生产者改动，另开分支。

## 验证

提交 `285f0d4`（实现）之上：

- 本机（macOS 27.2.1 arm64）`cargo test --locked -p stickymd-smoke -- release::macos`：35 个测试通过，其中新增 29 个（`ready/` 下 28 个与 `promote.rs` 中 resolver 的 1 个；P18-G01、G02）；
  `cargo clippy --locked -p stickymd-smoke --all-targets -- -D warnings` 在 macOS 与 `--target x86_64-pc-windows-msvc` 上都无警告。
- 运行器演练（不是正式收据，R 尚未冻结）：在 `285f0d4` 的干净 worktree 上 `release macos-source-tests --release-source 285f0d4…`，
  Cargo 列出的 23 项（17 个测试二进制、6 个库的文档测试）全部运行并归属，1919 通过、33 忽略、0 过滤、0 失败；同键互斥两个测试通过
  （区分大小写的那个挂载了磁盘映像，11.85 s）；开始与结束都是干净的 `285f0d4`，两行都判 PASS。
- 核对演练：同一提交上 `release macos-ready --release-source 285f0d4… --batch rc1`，没有候选，打印 58 行：29 PASS（源码测试、同键互斥与 27 条源码行）、
  26 NOT_TESTED（包完整性与随它的 P01、下载路径、11 条 `mac-gui` 行、8 条人工行、3 条资源行、R03）、3 EXCLUDED（W15、W16、V05），以非零状态退出。

- 文档与守卫提交之后：`phase 00`（governance）PASS；`phase 16` PASS；`phase 18` 的三个任务（governance、release tooling 与 macOS host 测试）全部 PASSED，
  其最后的矩阵状态检查因 P18-P04 为 `NOT TESTED` 而失败——`main` 上 P04、P06、P07 已是如此（需要真实的 `release-macos` 运行），与本分支无关；
  `cargo deny --locked check` PASS；八个 macOS 包的 `cargo clippy --all-targets -D warnings` 无警告（`--workspace` 在 macOS 上因 Windows 宿主不能编译而不适用）。
  `dev-check` 的完整模式在 macOS 上拒绝运行（“selected Windows checks require a Windows host”，`NOT_TESTED`），Windows 一侧留给 CI。
- `macos-ready` 之后加了守卫：只在 R 的干净检出中运行（门用 R 的工具判定）。在干净的 `ee37948` 上：`--release-source ee37948…` 打印 58 行，
  源码测试与同键互斥都因收据属于 `285f0d4` 而 `NOT_TESTED`（55 NOT_TESTED、3 EXCLUDED），退出 1；`--release-source 2222…`（不是 HEAD）在判定前
  拒绝并说明 HEAD，退出 1。之后 `phase 00` 在已提交的树上 PASS。
- 本 worktree 的 `dist/evidence/release-source-tests-macos.json` 是 `285f0d4` 上的演练收据，不是正式收据（ignored，不提交）。

未验证：带真实候选与账本的核对（需要一次 `release-macos` 运行与晋升）；正式收据只能在冻结的 R 上产生。

## Resolution（2026-11-24）

USER 对“需要 Develata 决定”四项与四项待确认提议的回复（原话）：

> 均同意按你的建议来做

据此：

1. **发布用 USER 收据与 waiver**：扩展账本，不另开文件。新记录种类 `release-receipt`、`release-waiver` 与 `download` 都绑定 resolver 给出的
   候选 receipt 摘要（`mac_ledger/release.rs`）；`mac-ledger receipt --release-source <R>`、新命令 `mac-ledger waive`、`mac-ledger revoke --release-source <R>`。
   waiver 只接受依赖 app 运行的行（`mac-gui` 行、人工行、资源行），不覆盖 FAIL。旧版工具读到新种类会拒绝整本账本，这一代价已接受。
2. **真实下载路径**：同一个记录器，行名 `P18-D01`…`D05`，须给 `--system`，记下候选的签名方式与走这条路径的系统；
   核对只认本候选的记录（fail closed）。
3. **生产侧缺口**（R03 探针接收候选 ZIP；`mac-resources` 记 bundle 清单摘要并能登记进账本）：另开分支做。在此之前 R03 使门不成立；资源行可逐行 waiver。
4. **四项提议**全部确认：行分类、账本判定、同键互斥并入源码测试收据、`macos-ready` 只在 R 的干净检出中运行。`11` #macos-readiness-check 的状态行
   改为已批准，并写明上述格式与判定。

实现与验证：判定加入 `WAIVED`（与 PASS 一样使行成立）；P18-G03 记录新增的 11 个测试（账本侧 4 个、判定侧 7 个）。本机 `cargo test --locked -p stickymd-smoke
-- release::macos mac_ledger` 65 个测试通过；`stickymd-smoke` 的 clippy 在 macOS 与 `x86_64-pc-windows-msvc` 上都无警告。带真实候选的记录器
（`receipt --release-source`、`waive`）需要先有一次晋升，尚未运行。

## 生产者改动（2026-11-24）

分支 `feat/m5-producers`（从 `feat/m5-macos-readiness` 的 `c605f27` 开出），实现 Resolution 第 3 项。合同写进 `11` #phase-16-macos-host 的
“R03 的输入 ZIP”与“资源运行进账本”两条，#macos-readiness-check 的分类、账本判定与“尚无来源的行”随之更新；验收见 Phase 18 P18-G04。

1. **R03 打开候选 ZIP。** `mac-gui --zip <ZIP>`：运行任何一行之前把 ZIP 解进临时目录，核对其中 `StickyMD/StickyMD.app` 的 bundle 清单与可执行文件两项
   SHA-256 都等于 `--app`，不符即拒绝整次运行。R03 把 ZIP 原样复制进下载目录（复制件须与检查时同一摘要）、加隔离标记后解开，观察 `StickyMD/`
   文件夹（便携版的 `note/` 写在那里，不是下载目录的顶层），解出的 bundle 再核对一次。证据顶层记 `zip_sha256`，账本只记进 R03 的记录（可选字段，
   旧记录字节不变）。不给 `--zip` 时照旧从 `.app` 打包，证据不含 ZIP 摘要。门只认干净组中打开了候选 ZIP 的 PASS。
2. **资源行进账本。** `mac-resources` 证据升为 schema 5：两项字节身份、产品输入指纹、驱动摘要、`sw_vers`、`kern.hv_vmm_present`、`finished` 与按行的
   结论 `rows`。`mac-ledger record` 接受跑完的 VM 运行，从各次运行重算 R01、R02，与写下的结论不符即拒绝；门对 R01、R02 的判定与 `mac-gui` 行相同。

由我定下、待你确认（可以否决）的三点：

- **R02 的统计量取各场景有效运行的中位数**，对照 `10` 目标表的硬失败条件“> 0.1%”。理由：这一行以往就按中位数判（按周期开关光标图层时 Split
  中位 0.103% 判为超过硬门）；`10` 的测量口径是“报告 median 与最大值”，没有写门看哪一个。替代：取最大值，更严，但一次读数就能使整行失败。
  这条规则在合同中标为提议；`mac-resources` 的退出码与证据的 `cpu_gate` 也随它。
- **V03 不由程序判定，仍可 waiver。** V03 要求基线与新包交替跑 `mac-latency` 与 `mac-resources`，呈现器计数只在 `mac-latency` 中；`mac-latency`
  是诊断，不进账本，也没有发布用的判定规则（发布时的“基线”是什么也没有定义）。要让它由程序判定，需要你先定规则（例如候选字节上 `mac-latency`
  全部有效且呈现器失败为 0），再让 `mac-latency` 进账本；在此之前每版由你 waiver。
- **ZIP 不参与分组。** R03 的干净组里，经别的 ZIP（含从 `.app` 打包）记下的 FAIL、NOT_TESTED 与被撤销的 PASS 同样使该组不干净；没有干净组时，
  候选字节上的 FAIL 判 FAIL，不论它经哪个 ZIP。比“把 ZIP 当作身份的一部分”更严：同一批里的开发运行也会影响发布判定，需要时换一个批次名。

验证（本机 macOS 27.2 arm64，提交前的工作树，基于 `c605f27`）：

- `cargo test --locked -p stickymd-smoke`：703 个测试通过（单元 683、集成 20），3 个忽略；新增 19 个，其中 `staging` 的 3 个在 macOS 上用 `ditto`
  做出真实的发布布局 ZIP。改坏检查后（不核对 ZIP 摘要、不比较 bundle 清单）相应的 3 个坏样本测试失败，恢复后通过。
- `cargo clippy --locked --all-targets -D warnings`：八个 macOS 包无警告；`stickymd-smoke` 加 `--target x86_64-pc-windows-msvc` 无警告。`cargo fmt --check` 通过。

未验证：

- 虚拟机里的实际运行：`--zip` 下的 R03（嵌套布局的 `.app` 是否同样被随机化、挂载行的匹配与提示）与 schema 5 的 `mac-resources` 运行都没有在
  `mac-test` 中跑过；需要你同意后在虚拟机里各跑一次。
- 新测试在 Windows 上只编译检查（clippy），由 CI 运行。
