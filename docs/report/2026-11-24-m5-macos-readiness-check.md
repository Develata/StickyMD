# M5 macOS 就绪核对：实现范围与待决事项

日期：2026-11-24。分支 `feat/m5-macos-readiness`（从 `main` `d9bab10` 开出），PR [#23](https://github.com/Develata/StickyMD/pull/23)。依据：`11` Phase 18 节 M5 行与
#macos-release-gate；[Phase 18 验证记录](2026-11-20-phase-18-verification.md)、[构建输入收窄记录](2026-11-21-platform-build-input-narrowing.md)。

## 结论

下文写于 USER 决定之前；决定与之后的实现见文末“Resolution（2026-11-24）”，届时只剩 R03 与资源行的生产者改动未落地。

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

## 复审与修复（2026-11-24）

跨模型复审（Codex，scope A：就绪核对，审于 `b7ae1de`）给出 9 项 BLOCKER、8 项 SHOULD_FIX、1 项 NICE_TO_HAVE。逐项先在本分支上核实，再按现行合同修复；
需要改合同或由 USER 决定的部分 fail closed 并列在下面。A5 与镜像摘要规范化（A6 的一部分）属于 PR #24，由 M5 producers 修复（双方已约定文件分工）。

| 发现 | 处理 |
| --- | --- |
| A1 S06 的 exFAT 测试为 `#[ignore]`，默认运行却给 S06 PASS | 已修复（fail closed）：S06 改为单独一类，判 `NOT TESTED`；八个包中每个 `#[ignore]` 测试由测试逐个归类。如何在 R 上取得 S06 的结果待决 |
| A2 L08 默认集的测试在大小写敏感的临时卷上跳过只差大小写的检查 | 已修复（fail closed）：运行器在临时卷大小写敏感时拒绝运行。收据是否记录两种卷的属性待决 |
| A3 W12 缺本身时，注销行的 FAIL 被 waiver 盖住 | 已修复：四行先全部判定，FAIL 优先 |
| A4 人工行忽略候选字节上的程序 FAIL | 已修复：程序 FAIL 压过收据与 waiver |
| A5 错误的生产者可签出 R01–R03 | 转 #24（M5 producers） |
| A6 分组用了整个身份（含可选指纹）；门跳过“一镜像一系统” | 已修复：按驱动、镜像与系统分组，门调用同一 `check_system`。镜像摘要规范化转 #24 |
| A7 收据的目标清单只与自身比较 | 已修复：核对时在 R 上用 `cargo metadata` 独立列出目标；实际 23 项与 `285f0d4` 演练的收据一致 |
| A8 下载记录不核对签名与系统 | 部分修复：核对签名方式等于候选、五行同一 macOS 大版本。D04 的情形、D05 是否被拦截的可核对字段，以及本次目标大版本，待决 |
| A9 重跑失败后旧 PASS 仍可见，并发没有锁 | 已修复：先取运行锁、再删旧收据，之后才探测与运行；门以共享锁读取，运行中为 `NOT TESTED` |
| S1 经符号链接的 `--ledger` | 已修复：账本与锁文件是符号链接时拒绝 |
| S2 登记与核对期间候选可能被换 | 部分修复：核对结束时再解析一次候选，不同即不成立。收据在登记时声明“看的是哪个候选”需要改合同（现行合同规定摘要不手工输入），待决 |
| S3 assume-unchanged 与 Cargo runner | 部分修复：拒绝 assume-unchanged 与 skip-worktree。Cargo target runner 的核对待决（有效配置要读 Cargo home 与环境，属于发布 workflow 已列出的同类前提） |
| S4 包完整性接受可证伪的 `cargo_lock_sha256` 与 `app_inputs` | 已修复：resolver 重算 S 上 `Cargo.lock` 的摘要并读取暂存 bundle 的 `StickyMDInputs` |
| S5 `ok`/`FAILED` 与计数矛盾、溢出 | 已修复：矛盾即无法读取，求和不溢出，同键运行恰为两项且没有 measured |
| S6 发布记录读入比登记宽松 | 已修复：每种记录恰有其字段，读入与登记共用行白名单 |
| S7 Mode 守卫与 workflow 命令守卫偏弱 | 已修复：每行的 Mode 须恰为分类时的 Mode；规定命令逐字等于 self-check 那一步的命令 |
| S8 候选收据的读取错误被报成“从未晋升” | 已修复：只有不存在才是缺失 |
| NICE P18-L13 错链 | 已修复 |
| NEEDS_VERIFICATION 正式候选的端到端判定 | 未变：需要真实晋升的候选与 R 上的正式收据 |

**需要 Develata 决定（新）：** 代码对以下四项都 fail closed：S06 判 `NOT TESTED`；临时卷大小写敏感时运行器拒绝；下载路径只认本候选、签名方式一致、
五行同一大版本；登记与 Cargo runner 维持现状（核对结束时会再解析一次候选）。

**(a) P16-S06（exFAT，`#[ignore]`）与 P18-L08 大小写敏感一半在 R 上的证据格式。**
- A1（建议）：把收据的 `same_key` 一节推广为 `explicit` 列表，每项含门的条目（`same-key`、`P16-S06`）、规定命令（`--exact --include-ignored` 加测试名）、
  测试名、计数与退出码；L08 一项另记两种卷的大小写属性（运行器在临时卷上探测，另一项由测试自己挂载的映像保证），S06 一项要求恰一项通过。
  一次运行、一个收据、同一对开始与结束身份，格式只增不减。
- A2：S06 单独一个收据文件与运行命令。互不影响，但多一个文件与一套身份核对。
- A3：S06 改为 USER 逐版 waiver（矩阵中它是 Automated 源码行，需要改“源码行按现行规则”的写法）。最省事，但放弃一项能自动证明的门。

**(b) P18-D04/D05 必须记什么，以及门如何得知目标 macOS 大版本。**
- B1（建议）：下载记录增加 `outcome` 字段——D04 取 `gatekeeper-blocked`、`translocated-prompt-exit`、`portable-start` 三者之一（契约列出的三种情形），
  D05 取 `blocked` 或 `not-blocked`；`mac-ledger receipt` 用 `--outcome` 传入，其他行不得带。`macos-ready` 增加必填的 `--macos-major <N>`，五行的
  `sw_vers` 大版本都须等于它。D04 的三种情形都算完成（契约只要求区分并记录），D05 两种都算。
- B2：只加 `outcome`，大版本仍要求五行一致但不与外部输入比较。少一个参数，但不能发现“整组都在旧大版本上做”。
- B3：大版本取自候选的构建记录（`LC_BUILD_VERSION` 的最低系统）。不需要人输入，但最低系统不等于“这一版验收的目标系统”。

**(c) USER 收据与 waiver 的候选前提（S2 第二部分）。**
- C1（建议）：`receipt`/`waive` 增加必填的 `--candidate-sha256`，须等于登记时 resolver 给出的摘要，否则拒绝；摘要由 `release macos-candidate`
  （改为同时打印摘要）复制，不是“手工计算”。这改变现行“摘要不手工输入”的写法，需要批准。
- C2：不加参数，登记时打印候选摘要与晋升时间，请 USER 在原话中确认。不改格式，但不能机器核对。
- C3：维持现状（晋升是本机手动操作，约定不与验收并行），只在合同中写明这一前提。

**(d) 核对有效的 Cargo target runner（S3 第二部分）。**
- D1（建议）：列为运行器不证明的前提，与发布 workflow 中“Cargo home 与环境中的编译配置”同一类写进合同；运行器拒绝明显的入口：
  环境中的 `CARGO_TARGET_*_RUNNER`、`CARGO_BUILD_RUSTC_WRAPPER`、`RUSTC_WRAPPER`。
- D2：读取 Cargo home 与各级祖先目录的配置文件，拒绝任何 `runner`。更完整，但要自己实现 Cargo 的配置合并规则（`include`、`cfg(...)` 表等）。
- D3：不处理。
