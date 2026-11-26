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

由我定下、待你确认（可以否决）的三点（USER 的决定见本节末尾的 Resolution）：

- **R02 的统计量取各场景有效运行的中位数**，对照 `10` 目标表的硬失败条件“> 0.1%”。理由：这一行以往就按中位数判（按周期开关光标图层时 Split
  中位 0.103% 判为超过硬门）；`10` 的测量口径是“报告 median 与最大值”，没有写门看哪一个。替代：取最大值，更严，但一次读数就能使整行失败。
  这条规则在合同中标为提议；`mac-resources` 的退出码与证据的 `cpu_gate` 也随它。
- **V03 不由程序判定，仍可 waiver。** V03 要求基线与新包交替跑 `mac-latency` 与 `mac-resources`，呈现器计数只在 `mac-latency` 中；`mac-latency`
  是诊断，不进账本，也没有发布用的判定规则（发布时的“基线”是什么也没有定义）。要让它由程序判定，需要你先定规则（例如候选字节上 `mac-latency`
  全部有效且呈现器失败为 0），再让 `mac-latency` 进账本；在此之前每版由你 waiver。
- **ZIP 不参与分组。** R03 的干净组里，经别的 ZIP（含从 `.app` 打包）记下的 FAIL、NOT_TESTED 与被撤销的 PASS 同样使该组不干净；没有干净组时，
  候选字节上的 FAIL 判 FAIL，不论它经哪个 ZIP。比“把 ZIP 当作身份的一部分”更严：同一批里的开发运行也会影响发布判定，需要时换一个批次名。

验证（本机 macOS 27.2 arm64；实现提交 `8c47cfc`，其后一个提交只改文档与帮助文字）：

- `cargo test --locked -p stickymd-smoke`：703 个测试通过（单元 683、集成 20），3 个忽略；新增 19 个，其中 `staging` 的 3 个在 macOS 上用 `ditto`
  做出真实的发布布局 ZIP。改坏检查后（不核对 ZIP 摘要、不比较 bundle 清单）相应的 3 个坏样本测试失败，恢复后通过。
- `cargo clippy --locked --all-targets -D warnings`：八个 macOS 包无警告；`stickymd-smoke` 加 `--target x86_64-pc-windows-msvc` 无警告。`cargo fmt --check` 通过。
- `phase 00` 在已提交的干净树上 PASS；`phase 16` PASS；`phase 18` 的三个任务（governance、release tooling 与 macOS host 测试）全部 PASSED，
  最后的矩阵状态检查因 P18-P04 为 `NOT TESTED` 而失败，与 `c605f27` 相同（P04、P06、P07 需要真实的 `release-macos` 运行）。

未验证：

- schema 5 的 `mac-resources` 没有在 `mac-test` 中跑过（R03 的虚拟机运行见下节）。
- 新测试在 Windows 上只编译检查（clippy），由 CI 运行。

### Resolution（2026-11-24，生产者改动）

USER 在父会话（M5 就绪核对）中对上面三点与“推送并开 PR、在 `mac-test` 里跑一次 R03”的回复（原话，由父会话转达）：

> 都同意

据此：R02 取各场景有效运行的中位数，对照 `10` 的 0.1% 硬失败线；V03 在 USER 定下发布用的规则之前不由程序判定，保持 `NOT TESTED`，每版可
waiver；R03 的 ZIP 摘要不参与分组。`11` #macos-readiness-check 的状态行与 #phase-16-macos-host 的 R02 一条已改为已批准。

### R03 在 `mac-test` 中的运行（2026-11-24）

这是实现的验证，不是发布证据：ZIP 由本机打包，不是晋升的候选。

- **被测字节：** 在分支提交 `be6cce1` 的干净克隆中，`build-app.sh` 构建 app，`stickymd-package macos-package` 按发布布局打出
  `StickyMD-0.2.0-macos-arm64-portable.zip`：ZIP `f99e819c…`，bundle 清单 `14fd817a…`，可执行文件 `92531ef9…`，产品输入指纹 `01b7c840…`，
  签名 AD_HOC。本 worktree 嵌在主检出之下，主检出的 `.cargo/config.toml` 是祖先目录的 Cargo 配置，`app-inputs` 按设计拒绝在这里构建，所以改在克隆中构建。
- **驱动：** `74d5b52` 的开发版（unwind）`stickymd-smoke`，`30a86491…`。
- **命令：** 虚拟机（macOS 27.2，26B123，`hv_vmm_present` 为 1）中执行
  `stickymd-smoke mac-gui --app StickyMD.app --zip StickyMD-0.2.0-macos-arm64-portable.zip --only R03 --batch m5p-1`。
- **结果：** R03 PASS，六项检查全部通过，证据 `finished` 为真，顶层 `zip_sha256` 等于 ZIP 的摘要：
  - 运行前核对通过；
  - 解出的 `download/StickyMD/StickyMD.app` 以 App Translocation 的只读 nullfs 挂载启动，挂载行的源路径就是嵌套布局的路径；
  - 模态提示窗出现，stderr 给出“请先把 StickyMD.app 拖进‘应用程序’，或把整个 StickyMD 文件夹移到别处”；
  - Return 后退出，runningboard 记录为 `(0, 0, 256)`；
  - `StickyMD/` 文件夹前后同为 `LICENSE.txt`、`README.txt`、`StickyMD.app`、`THIRD_PARTY_NOTICES.txt`、`licenses`。
- **证据：** 已忽略的 `dist/evidence/mac-vm-2026-11-24-m5p/`（`mac-gui.json` 的 SHA-256 为 `b3499710…`），在本 worktree 中。
- **账本：** 用 `mac-ledger record` 登记进真实账本的一份副本（真实账本未改动）。原有 337 行照常读取，新增的 R03 记录带 `zip_sha256`、批次 `m5p-1`。

见[审查修复报告](2026-11-24-m5-producers-review-fixes.md)：生产者一侧（scope B）的跨模型审查与处理。

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

**需要 Develata 决定（新）：** 当时写“代码对以下四项都 fail closed”，对 (b) 并不成立：没有 D04/D05 结果的五条下载记录仍能使下载路径 PASS
（第二次复审 finding 4）。S06、L08 与登记确实 fail closed，Cargo runner 维持现状。四项已于下文“第二次复审与 USER 决定”中决定并实现。

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

## 第二次复审与 USER 决定（2026-11-24）

**USER 决定**（Develata 在发起会话中的回答，由该会话“Develata 回收完成”转达，原词照录；四项都选了推荐方案）：

- (a) “并入源码测试收据（推荐）”：`same_key` 推广为 `explicit` 列表（门的条目、精确的 `--exact --include-ignored` 命令、测试名、计数、退出码）；
  L08 另记两种卷的大小写属性；S06 须恰有一项通过。
- (b) “outcome + --macos-major（推荐）”：D04 的结果为 `gatekeeper-blocked`、`translocated-prompt-exit`、`portable-start` 之一，D05 为 `blocked` 或
  `not-blocked`，经 `mac-ledger receipt --outcome` 记录，其他行不得带；`macos-ready` 必须给 `--macos-major <N>`，五行 `sw_vers` 的大版本须等于它。
- (c) “必填 --candidate-sha256（推荐）”：`receipt`/`waive` 必须给 `--candidate-sha256`，须等于登记时 resolver 给出的摘要；`release macos-candidate`
  打印它。USER 同意据此修改“摘要不手工输入”的写法。
- (d) “写成前提并拒明显入口（推荐）”：写进合同，作为运行器不证明的前提（与 workflow 的 Cargo home 同一类），并拒绝环境中的 `CARGO_TARGET_*_RUNNER`、
  `CARGO_BUILD_RUSTC_WRAPPER`、`RUSTC_WRAPPER`（实现另拒两个 workspace wrapper 变量）。

另：发起会话转达的规则——证据尚未定义的行不得能打印 PASS 或 `WAIVED`，每条都要坏样本——写进了 `11` #macos-readiness-check 的“一般规则”。

**复审**（Codex，审于 `2405b20`，原文在发起会话的 scratchpad）：10 项 BLOCKER、10 项 SHOULD_FIX、2 项 NICE_TO_HAVE、2 项 NEEDS_VERIFICATION。逐项核实后处理如下，
修复分六个提交（`e471b4a` 纯移动的拆分，`5c7071c`、`c6aac0c`、`bdb054b`、`1cf0a12`、`ae047b0`），所列守卫各去掉一次，看着它的坏样本变红再放回（第三次复审指出其中几个样本同时违反两项条件，这句话因此只对那些样本单独成立的守卫成立；
逐项结果见“第三次复审”）。

| 发现 | 处理 |
| --- | --- |
| 1 ZIP、checksum 与暂存 bundle 可互相矛盾 | 已修复：resolver 核对 checksum 文件内容、在暂存 ZIP 上跑包检查并比对 bundle、可执行文件、产品输入、签名方式与构建来源、核对 SBOM |
| 2 零项执行的二进制仍 PASS | 已修复：收据逐个测试记录各目标 `--list` 的清单与运行结果；Cargo runner 部分按决定 (d)（第三次复审 S3 收窄：只对清单非空的目标成立） |
| 3 L08 可用任意两个测试证明 | 已修复（决定 (a)）：显式运行须归属 stickymd-mac 的二进制，按名恰为其测试，且在主运行清单中；记录临时卷属性（第三次复审 B4：原为运行器自己临时目录的探测，不足以证明测试断言所在的卷，已改为测试自己测得两卷） |
| 4 下载路径没有 D04/D05 结果也 PASS | 已修复（决定 (b)） |
| 5 登记可把对 A 的观察绑给 B | 已修复（决定 (c)） |
| 6 人工行的 FAIL 被另一身份的 PASS 隐藏 | 已修复：人工行在收据前先看任何身份的 FAIL |
| 7 W12 在每个完整环境都失败时仍可 WAIVED | 已修复：先在每个身份内判定四行，没有完整干净组而有失败组即 FAIL |
| 8 V03 已登记的失败可被 waiver 覆盖 | 已修复：所有 `NOT TESTED` 行经同一个守卫，任一 FAIL 先判为 FAIL |
| 9 replacement ref 与核对端跳过索引标志 | 已修复：运行器与核对端共用按提交读取的严格身份，核对结束时复核 |
| 10 大小写探针截断文件、删除别的目录 | 已修复：独占创建临时目录，清理失败如实报告 |
| S1 旧驱动的 PASS 仍证明受影响的门 | **推迟**，待决（见下） |
| S2 账本与收据可能从未同时有效 | 已修复：两者在各自锁下作为一个快照读取；候选或检出在核对中改变则不打印结论 |
| S3 公共写入口可替换运行锁 | 已修复：收据与锁登记为保留路径，账本的目的地也拒绝它们 |
| S4 只读入口写文件、读账本绕过链接检查 | 已修复：读者只读打开锁、不创建；`plan` 与核对端都拒绝链接；身份查询不取可选索引锁 |
| S5 没有候选时跳过 `origin/main` 前提 | 已修复：入口即核对 |
| S6 按候选撤销接受未知字段 | 已修复：撤销恰有其字段 |
| S7 读管道出错可能占锁互等 | 已修复：重试中断，其他错误先结束子进程 |
| S8 坏样本不能区分其条件 | 部分：各加只改一个事实的样本，R03 样本按本分支实际证明的内容改名（第三次复审又找到几处，见下） |
| S9 忽略测试守卫只扫六个包、按行数 | 已修复：八个包、按测试名归类 |
| S10 P18-P04 的链接错位 | 已修复：按测试名重新生成 |
| N1 测试模块缺 plan_ref | 已修复 |
| N2 `judge.rs` 502 行 | 已修复：按“同一身份内的结果”（`groups.rs`）与“每行如何使用”（`judge.rs`）拆分 |
| V1 断电后的持久性 | 部分：删除旧收据后同步目录；原子写入后的目录同步属打包 crate，未改；合同写明保证的范围 |
| V2 正式候选的端到端证据 | 未变：需要冻结的 R 与真实晋升的候选 |

没有驳回的发现。资源行：发起会话转达“证据尚未定义的行不得 WAIVED”，而 `11` 已批准“资源行可由 USER 逐行 waiver”（USER 2026-11-24）；本分支保留后者
（waiver 是已定义的证据），只加了 FAIL 优先。请 Develata 确认。

**需要 Develata 决定（第二次复审之后）：**

1. **SHOULD_FIX 1：门要不要只认 R 的验证驱动。** 现在不同驱动、镜像、系统的干净运行各自都算（已批准的规则）；若 R 修正了某行的检查，旧驱动的 PASS
   仍可沿用。
   - A（建议）：`macos-ready` 增加必填 `--driver <stickymd-smoke 二进制>`，`mac-gui` 行只认该驱动 SHA-256 的组，与 `mac-ledger plan` 的复用规则一致；
     FAIL 仍不论驱动都算。
   - B：只对驱动输入指纹变了的行要求新驱动（需要按行的驱动输入登记，工作量大）。
   - C：维持现状，在合同中写明旧驱动的 PASS 可沿用。
2. **核对取自何处**（`11` #macos-readiness-check 中标为提议的一条）：写明门在 R 上重算什么、取自本地记录的是什么、不证明什么，使下一次复审有边界可依。
   建议批准原文。

**验证**（本机 macOS 27.2.1 arm64）：`stickymd-smoke` 692 + 20 个测试通过；clippy 在八个 macOS 包上与 `stickymd-smoke --target x86_64-pc-windows-msvc`
都无警告；fmt 通过。运行器的真实演练（`ae047b0`，不是正式收据）：schema 2 收据中 23 个目标的 1991 个测试逐个对上清单（1958 通过、33 忽略、0 未运行），
same-key 2/2 与 S06 1/1 通过，三项判定都为 PASS，开始与结束都干净。`macos-ready` 在同一提交上因 R 不在 `origin/main` 中而在判定前拒绝——这正是 S5 的
守卫；分支上无法给出逐行结果，逐行判定由坏样本测试覆盖，真实的逐行输出要等 R 进入 `main`。

## 第三次 USER 决定（2026-11-24）

Develata 在发起会话中的回答，由该会话“Develata 回收完成”转达，原词照录：

- SHOULD_FIX 1：“必填 --driver（推荐）”。`macos-ready` 增加必填的 `--driver <R 上的 stickymd-smoke 二进制>`；`mac-gui` 行只认驱动 SHA-256 等于它的组，
  与 `mac-ledger plan` 相同；`FAIL` 不论驱动都算。已实现：`groups::joint` 只把 R 的驱动的组算作干净组，判定为 `NOT TESTED` 的行仍由同一个守卫查任何驱动的
  `FAIL`；坏样本为旧驱动的 PASS 不算、旧驱动的 FAIL 仍算并挡住 waiver；去掉守卫时前者变红。原先“另一身份的干净运行成立”的样本改为同一驱动、另一 VM 镜像。
- “核对取自何处”：“批准原文（推荐）”，`11` 中去掉了“提议”的标记。
- waiver：发起会话说明，此前的措辞针对证据未定义的行经漏洞得到 PASS 或 `WAIVED`，不推翻已批准的规则；资源行在 `NOT TESTED` 时可按候选逐行 waiver，
  已记下的 `FAIL` 优先，维持现状。

## 第三次复审（2026-11-24）

**USER 决定（威胁模型）**：Develata 在发起会话中选了“只防诚实操作者的失误（推荐）”，由该会话“Develata 回收完成”转达：门防的是诚实操作者的失误、
过期／不完整／不一致的证据与产品回归；不防恶意的本地篡改、并发的替换后恢复、伪造的工具输出与有意重定向的环境。已写进 `11` #macos-readiness-check
“核对取自何处”的不证明的前提，并注明锁、首尾复查与对 Git、Cargo 环境变量的拒绝只挡显而易见的入口。合并标准随之定为：下一轮在这一模型下没有
BLOCKER 即合并。

**复审**（Codex，审于 `2c5c6ea`，原文在发起会话的 scratchpad）：6 项 BLOCKER、7 项 SHOULD_FIX、1 项 NICE_TO_HAVE、1 项 NEEDS_VERIFICATION。
发起会话按上面的模型把 B1、B2、B3、B6 降为“便宜的拒绝”，B4、B5 仍须完整修复。逐项核实后处理如下：

| 发现 | 处理 |
| --- | --- |
| B1 Git 可被环境变量指向别的仓库、工作树或索引 | 已修复：两个入口共用的身份读取拒绝 `GIT_DIR`、`GIT_WORK_TREE`、`GIT_INDEX_FILE`、`GIT_OBJECT_DIRECTORY`、`GIT_ALTERNATE_OBJECT_DIRECTORIES`、`GIT_COMMON_DIR`，并要求 `rev-parse --show-toplevel` 规范化后等于运行的根；每个变量单独被拒的样本 |
| B2 候选摘要与包内容可来自不同字节 | 已修复（比较）：包检查返回的 ZIP 摘要须等于 receipt 中的。单一快照读取未做：只在并发替换后恢复时才有区别，属于模型之外 |
| B3 同一测试先 `ok` 后 `FAILED` 可折叠成 PASS | 已修复：清单、ignored 清单与运行结果中重复的名字、不在清单中的 ignored 项与运行项都记入 `problems`，不并入第一项 |
| B4 L08 可没有执行大小写断言却留下成功回执 | 已修复：两个测试在 `STICKYMD_VOLUME_REPORT` 下各自在断言所用的目录上探测大小写并写下结果；运行器要求每个卷恰好一行，收据记 `temporary_volume` 与 `image_volume`，判定要求两者分别为 `case-insensitive` 与 `case-sensitive`，各自单独决定；运行器自己的探测已删除。L08 现在说明的是：两个测试都通过，且测试自己测得这两种卷；不说明默认集中的测试在别的卷上也会断言 |
| B5 锁文件不存在时无锁读取 | 已修复：收据存在而锁文件不存在为 `NOT TESTED`（重跑），账本存在而锁文件不存在为错误；两者都不在无锁时消费数据 |
| B6 自定义账本路径可替换保留的锁 | 已修复：按命令所在仓库的根判断保留路径，除不存在外的任何元数据错误都阻止读写 |
| S1 R03 已知的 FAIL 降为 NOT TESTED | 已修复：R03 先查本批中候选字节上的 FAIL；两种导入顺序都断言恰为 FAIL |
| S2 不完整盖过已记录的失败 | 已修复：列不出 R 的目标时收据记下的失败仍为 FAIL；显式运行的失败计数单独即为 FAIL |
| S3 清单与运行同为零的目标仍可通过 | 收窄：第二次复审“零项执行仍 PASS 已修复”只对清单非空的目标成立。清单与运行同时报零要 libtest 自己出错，不是这一模型下的操作失误；没有加非空要求（真正为空的文档测试合法） |
| S4 同名异目录的可执行文件、截断的尾行 | 已修复：段头的路径须逐个组件是 Cargo 构建出的可执行文件路径的结尾；汇总行须以完整的 `finished in <秒>s` 结束；各有反例 |
| S5 ignored 清单测试的盲点 | 部分：源码扫描也认 `#[cfg_attr(…, ignore…)]`，显式运行的角色精确比较。仍只在属性后几行内找函数名；运行时的权威是收据中各目标的 `--list --ignored` |
| S6 首尾复查只覆盖开始时有效的候选 | 已修复：首尾比较候选的整个状态（不存在、不能解析、解析到哪个 receipt）。没有坏样本：要在判定期间改变候选 |
| S7 自相矛盾的 `sw_vers` 材料 | 已修复：恰有一个 `ProductVersion`，一到三段数字；合同写明这是格式核对，不是对机器的证明 |
| NICE 单独提取 ignored 清单测试 | 推迟 |
| NEEDS_VERIFICATION 逐守卫证明 | 已补：见下 |

没有驳回的发现。

**逐守卫的移除结果**（去掉一个守卫，只跑它的样本，看它变红，再放回；脚本 `r3_mutations.py` 在共享笔记目录）：以下 27 个守卫各自的样本都变红——显式运行的
退出码、失败用例、失败计数、运行数、通过数与计数之和；主运行的失败用例与失败计数；逐目标计数之和；重复的运行项、重复的清单项、不在清单中的
ignored 项；截断的尾行与完整路径；两种卷各自的条件与卷报告的重复行；R03 的 FAIL 优先；列不出目标时失败优先；唯一的 `ProductVersion` 与数字段；
收据与账本缺锁；账本的保留路径与元数据错误；Git 重定向变量；同 bundle 异可执行文件。没有移除结果的：B2 的 ZIP 摘要比较（只在模型之外的并发下
才不同）、B1 在 `capture` 入口处的调用与 `show-toplevel` 等于根的核对（样本测的是变量表）、S6 的首尾状态比较、S5 的 `cfg_attr` 分支（它本身是测试）。

冗余的条件改为删去并注明：显式运行中“没有 ignored”“没有 measured”两项由失败先返回、`ran == passed == 测试数` 与计数之和推出；逐目标核对中
“measured 为 0”由其余四项与计数之和推出。对它们写单独的样本不能区分什么。

收据仍为 schema 2，`explicit` 多了 `image_volume`，没有升版本号：第三次复审之前写成的 schema 2 收据在逐字段的严格解析中被拒，须在 R 上重跑。

M5 producers 转来的 #23 部分：同 bundle、异可执行文件的样本即 `another_executable_alone_does_not_count`（K02），它所测的比较在 `groups.rs` 中（#24 的资源行
也经同一函数），去掉可执行文件一项时它变红；本分支的资源行不读字节，资源行的同类样本属于 #24 的 `ready/tests/producers.rs`。本分支的 `with_the_user_records_every_row_but_r03_holds`
中 S06 凭显式运行 PASS，去掉 R03 前后的就绪对照只差 R03，不需另外排除。P18-P04 与 G01–G03 的行号按测试名重新生成。

**验证**（本机 macOS 27.2.1 arm64，`4e36ebd`）：`stickymd-smoke` 706 + 20 个测试、`stickymd-mac` 124 个测试通过；clippy 在八个 macOS 包上与
`stickymd-smoke --target x86_64-pc-windows-msvc` 都无警告；fmt 通过。运行器的真实演练（`4e36ebd`，不是正式收据）：23 个目标的 2005 个测试逐个对上清单
（1972 通过、33 忽略、0 未运行），`problems` 为空，即按完整路径归属与重复名检查在真实输出上没有误拒；same-key 2/2 与 S06 1/1 通过，两个测试自己测得
临时卷 `case-insensitive`、映像 `case-sensitive`；三项判定都为 PASS，开始与结束都干净。`macos-ready` 的逐行输出仍要等 R 进入 `main`。

## 第四次复审（2026-11-24）

**USER 决定（收敛规则）**：Develata 在发起会话中选了“修完本轮只审修复（推荐）”，由该会话“Develata 回收完成”转达：修完本轮的发现；下一轮复审只看本轮的
修复 diff（`2b27850..<新 head>`），不再审整个 PR；它没有 BLOCKER，#23 即合并。此后在本轮没有改动的代码中发现的问题，记为跟踪的后续项，须在冻结 R 之前
修完，记在下面“冻结 R 之前的后续项”之下。

**复审**（Codex，审于 `d9bab10..2b27850`，诚实操作者的模型，原文在发起会话的 scratchpad）：6 项 honest-path BLOCKER、8 项 SHOULD_FIX、1 项
NEEDS_VERIFICATION。发起会话核实了 BLOCKER 3 与 5；其余逐项核实后处理如下。收据随之升为 schema 3（运行身份、进程怎样结束、显式运行的清单、驱动），
schema 1 与 2 被拒，须在 R 上重跑。

| 发现 | 处理 |
| --- | --- |
| BLOCKER 1 `--driver` 可以是任何文件 | 已修复：运行器在 R 上按 `mac-gui` 的文档命令构建驱动，记下 Cargo 所报二进制的 SHA-256；核对只在收据属于 R、构建命令为规定命令、`--driver` 的摘要等于收据所记时认它，否则没有一组能使 `mac-gui` 行 PASS；FAIL 不论驱动都算 |
| BLOCKER 2 中断的重跑使旧收据重新可读 | 已修复：运行在锁下、删除旧收据之前原子写运行记录 `dist/evidence/.release-source-tests-macos.run`，收据记下同一身份；核对只读身份相符的收据。样本按“缺锁的旧收据 → 重跑在删除前中断 → 再核对”的顺序 |
| BLOCKER 3 忽略测试的归类只扫源码 | 已修复：归类表移入核对程序（macOS 上的 33 个忽略测试，按包、目标与 libtest 全名，取自真实收据），判定时与各目标 `--list --ignored` 逐个比对；源码扫描的测试删去。样本为一个未归类、如多行 `cfg_attr` 那样被忽略的测试 |
| BLOCKER 4 自定义账本路径与保留路径不是同一个文件 | 已修复：`--ledger` 在入口按当前目录一次解析成绝对路径，检查、加锁与写入共用；在 macOS 上与保留路径（含运行记录）比较不分大小写；运行器删除旧收据前确认它是源码测试收据，不是则保留并报错 |
| BLOCKER 5 末尾残缺的 `ProductVersion` 标签不计数 | 已修复：先数整份材料中的全部标签，恰为一个再读其后的值 |
| BLOCKER 6 L08 探测两次、`exists()` 吞掉错误 | 已修复：只探测一次，报告与分支用同一个结果，`try_exists` 的错误使测试失败。没有单元样本：改动在测试本身，由真跑验证 |
| SHOULD_FIX L08 临时目录失败丢掉已有的 FAIL | 已修复：记入 `problems`，已取得的主运行结果照常写入。没有样本：要让系统临时目录不可写 |
| SHOULD_FIX 没有启动与被信号结束都记为 `None` | 已修复：进程的结束分为退出码、信号、没有启动、不能跟到结束；后两者没有结果（`NOT TESTED`），它打印出的失败仍为 FAIL |
| SHOULD_FIX Cargo 提前停止时显式运行找不到清单 | 已修复：显式运行记下 stickymd-mac 的二进制在任何运行之前自己的清单所列的测试（`listed`），不再依赖主运行 |
| SHOULD_FIX libtest 的 ` - compile` 后缀与 rustdoc 的多组 | 已修复并对照 rustc 1.99.1 的真实输出（临时的 2024 edition crate：合并与单独的文档测试、`compile_fail`、`no_run`、`should_panic`、含 ` ... ` 的忽略原因）：去掉三种模式后缀、在第一个 ` ... ` 处分开、文档测试段的多组计数相加；样本即那份输出 |
| SHOULD_FIX 缺锁时先读内容、路径检查吞掉错误 | 已修复：缺锁时只看是否存在，不读内容；检查路径时除不存在外的错误都使读取停止。“没有读”本身不能由样本观察，检查出错有样本 |
| SHOULD_FIX S06 的 `filter_map(Result::ok)` | 已修复：读出全部目录项的名字，有一项读不出即测试失败；卸载之后再断言。第一次真跑（`4b6980f`）抓到修复本身的错：保留的 `DirEntry` 使目录一直打开，卷忙而卸载失败，S06 为 FAIL；改为只保留名字后，显式运行通过 |
| SHOULD_FIX 重新打包 ZIP 的样本到不了摘要比较 | 已修复：checksum 文件由真正的生成器写成，只差 ZIP 的摘要，断言具体的 `Checksum mismatch`；另加“检查的是 A、登记时已换成 B”的样本 |
| SHOULD_FIX README、plan、coverage 的措辞 | 已修复：README 的命令带上 `--macos-major`、`--driver`、`--candidate-sha256` 与 `--outcome`，写明 S06 只凭显式运行；R03 写成“本批的 FAIL 为 FAIL，其余为 NOT_TESTED”；普通 `mac-gui` 行在另一组干净时 PASS。G02 的引文说明已在 `ec70cbe` 改正 |
| 另：同名先 `ok` 后 `FAILED` | 已修复：保留失败，同时记为问题 |
| 另：Git 重定向变量的样本 | 已修复：样本自己写出六个名字，不再读被测的表 |
| 另：结束时读不出身份 | 已修复：原始的 Git 错误记入原因 |
| NEEDS_VERIFICATION | 见下面的移除结果与真跑 |

没有驳回的发现。

**逐守卫的移除结果**（第三轮的 27 项之外，脚本 `r4_mutations.py` 在共享笔记目录）：以下 18 个守卫各自的样本都变红——驱动的构建命令、`--driver`
等于收据所记的驱动；收据的运行身份与运行记录的存在；忽略测试的归类；保留路径的大小写折叠与运行记录；只删除可识别的收据；唯一的 `ProductVersion` 标签；
显式运行的清单；没有结果的进程；模式后缀、在第一个 ` ... ` 处分开、文档测试的多组；保留同名的失败；驱动构建须成功；检查路径的错误；登记时候选未换。
没有移除结果的：L08 的单次探测与 `try_exists`、S06 的目录枚举（都在产品测试里，由真跑验证）、卷报告目录失败时照常发布、缺锁时不读内容、账本路径一次
解析成绝对路径（入口处的一行）。

### 冻结 R 之前的后续项

按上面的收敛规则，下列各项不在本轮的修复里，须在冻结 R 之前处理：

1. `release/macos/promote.rs`（1000 行，含测试，计于 `bd48bcc`）同时承担晋升与发布时的重新解析；按复审所说的边界把 resolver 拆出。
2. `mac_ledger/record.rs`（619 行）同时承担账本的表示、编解码与一致性检查；按这三者拆分。
3. `src/cli.rs`（1780 行）在 base 已超线，主要是命令声明与分派；拆分与否在冻结 R 之前定下。
4. （修复轮复审 15）只有可合并文档测试的 2024 edition 库，`--list --format terse` 也会输出合并测试的计时尾行；清单解析器拒绝它，并清空所有已取得的
   清单。对 rustdoc 清单单独识别官方的计时尾行，并保留其他目标的清单。
5. （修复轮复审 16）`--driver` 文件不存在或不可读时，取摘要的错误在读账本之前使命令退出，输出里看不到账本中已有的 FAIL；把它交给已有的
   `driver: Err` 判定路径，其余行照常报告。
6. （修复轮复审 17）L08 的 `count_keys` 对目录项直接计数，一项读不出的目录项也算一项；先传播逐项错误再计数。

此后的复审若在本轮没有改动的代码中发现问题，追加到这里。

**验证**（本机 macOS 27.2.1 arm64）：`stickymd-smoke` 718 + 20 个测试、`stickymd-mac` 124 个测试通过；clippy 在八个 macOS 包上与
`stickymd-smoke --target x86_64-pc-windows-msvc` 都无警告；fmt 通过。运行器的真实演练：
- `4b6980f`：S06 为 FAIL，见上表 S06 一行（修复本身使卷卸载失败）；演练留下的 exFAT 映像已卸载、其临时目录已删除。
- `e4e4891`（不是正式收据）：schema 3 收据中 23 个目标的 2017 个测试逐个对上清单（1984 通过、33 忽略且都在归类表中、0 未运行），`problems` 为空，
  收据的 `run` 等于运行记录；same-key 2/2 与 S06 1/1 通过，两个测试自己测得临时卷 `case-insensitive`、映像 `case-sensitive`；驱动的摘要
  `34d619e1…` 等于 `target/release/stickymd-smoke` 的（与 `4b6980f` 上的相同：`stickymd-smoke` 的源码两者之间没有变）；三项判定都为 PASS，开始与
  结束都干净。六个文档测试目标在本仓库中都没有文档测试，rustdoc 的多组输出只由临时 crate 的真实输出验证。

另见：本机还挂着一个约五天前的映像 `stickymd-test-root-case-sensitive-62516-…`，不是本轮的运行留下的，未动。

## 修复轮复审与合并规则（2026-11-24）

**USER 决定（合并规则）**：Develata 在发起会话中选了“再修一轮就合并（推荐）”，由该会话“Develata 回收完成”转达：修完这一轮的 6 项、自测通过，#23
即合并，不再为合并而复审；此后的发现都记入“冻结 R 之前的后续项”。在冻结 R 之前、代码不再变动时，对整个就绪门做一次完整的跨模型复审，它没有 BLOCKER，
R 才冻结。这一条取代上面“只审修复”的规则在合并上的作用。

**复审**（Codex，审于 `2b27850..bd48bcc`，原文在发起会话的 scratchpad）：6 项 honest-path BLOCKER、5 项 SHOULD_FIX、3 项 NICE_TO_HAVE、3 项
后续 SHOULD_FIX。逐项核实后处理如下；后续的 3 项（15–17）按发起会话的要求列入上面的“冻结 R 之前的后续项”第 4–6 条。

| 发现 | 处理 |
| --- | --- |
| BLOCKER 1 驱动构建复用默认构建目录，旧缓存可以代替 R 的构建 | 已修复：在固定的 `target/macos-driver` 中构建，构建前清空。实测：同一来源的 release 构建在另一路径下有 48 个字节不同（含链接器的 `LC_UUID`），同一路径从空目录重建则字节相同；所以目录固定，同一检出上重跑得到同一个驱动，虚拟机运行与 `--driver` 用的都是这个文件（README 已写明） |
| BLOCKER 2 新建的锁使缺锁时留下的旧记录与收据复活 | 已修复：锁文件尚不存在时，先删除运行记录并同步目录再建锁；样本按“旧的成对记录与收据、缺锁 → 重跑建锁后、记下自身之前停止 → 核对”的顺序 |
| BLOCKER 3 输出截断使已解析的 FAIL 降为 NOT_TESTED | 已修复：读输出时保留中断之前读到的部分，已打印的 `FAILED` 归属到目标与清单名字，仍为 FAIL；没有汇总的目标为 `counts: null`，不能 PASS；`failures:` 之后到汇总之前的失败输出不当作控制行。样本从残缺的原始输出走到最终判定 |
| BLOCKER 4 归类丢掉目标种类 | 已修复：归类表与查找用整个目标（包、种类、名字）；样本只改种类 |
| BLOCKER 5 `listed` 丢掉重复的名字 | 已修复：按清单中出现的次数保留，显式行上重复即为 `NOT TESTED`；生产者与判定各有样本 |
| BLOCKER 6 `ProductVersion:28.0` 不计数 | 已修复：以 `ProductVersion:` 开头的词都是标签，值紧贴冒号时取其后的部分 |
| SHOULD_FIX 7 大小写不同的保留名作为上级目录 | 已修复：路径本身与它的每一级上级都按 macOS 的规则比较；样本为尚不存在的、大小写不同的上级 |
| SHOULD_FIX 8 结束时的 Git 错误没有进收据 | 已修复：记入 `problems`。没有样本：要让 Git 在运行末尾失败 |
| SHOULD_FIX 9 schema 3 的值没有格式检查 | 已修复：驱动摘要须为 64 位十六进制，运行身份须为运行器自己的形式，卷字段只在 `same-key` 上、只能是两个词之一 |
| SHOULD_FIX 10 样本不能单独区分条件 | 已修复：错 `kind` 的单个合法 JSON、只错名字或只错种类的驱动产物、经登记入口（`candidate_with`）的候选比对 |
| SHOULD_FIX 11 P18-G01 行首仍写 schema 2 | 已修复 |
| NICE 12 G03 的 `append.rs` 引文 | 已修复（`82a3848` 已改正行号；本轮按实际覆盖分别引比较与登记入口两个样本） |
| NICE 13 L08 第一轮也探测 | 已修复：探测只在只差大小写的那一轮，报告与判断共用一次结果 |
| NICE 14 `promote.rs` 的行数 | 已修复：写为 1000 行并注明计于 `bd48bcc` |

**逐守卫的移除结果**（脚本 `r5_mutations.py` 在共享笔记目录）：以下 13 个守卫各自的样本都变红——清空驱动目录；新建锁前删除运行记录；`failures:` 块；
中断时保留已读部分；归类中的种类；显式行的重复；清单按次数保留；紧贴冒号的标签；上级目录的大小写比较；驱动摘要、运行身份与卷字段的格式；登记入口的
候选比对。没有移除结果的：驱动命令中的 `--target-dir`（命令与判定用同一常量）、结束时的 Git 错误进 `problems`、L08 探测的位置。

**验证**（本机 macOS 27.2.1 arm64，`6740860`，即合并进 `main` 的 #23 head）：`stickymd-smoke` 726 + 20 个测试、`stickymd-mac` 124 个测试通过；clippy
在八个 macOS 包上与 `stickymd-smoke --target x86_64-pc-windows-msvc` 都无警告；fmt 通过；`phase 00`、`phase 16` 通过，`phase 18` 的最后一项只因 `main`
上已有的 P18-P04 `NOT TESTED` 而失败。运行器的真实演练（不是正式收据）：schema 3 收据中 23 个目标的 2025 个测试逐个对上清单（1992 通过、33 忽略且都在
归类表中、0 未运行，没有 `counts` 为 null 的目标），`problems` 为空，收据的 `run` 等于运行记录；same-key 2/2 与 S06 1/1 通过，两个测试自己测得临时卷
`case-insensitive`、映像 `case-sensitive`；驱动在先清空的 `target/macos-driver` 中构建，摘要 `1ed0c204…` 等于 `target/macos-driver/release/stickymd-smoke`
的；三项判定都为 PASS，开始与结束都干净。新测试在 Windows 上只做了 clippy，没有实际运行。


## Resolution：冻结 R 之前的后续项（2026-11-26）

Develata 2026-11-26 在协调会话中决定由该会话处理冻结前清单；对第 3 项选“拆分（推荐）”。分支 `chore/pre-freeze-items`，自 `dab45d9`（#27 合入后的
`main`）切出。

1. **已拆（`8adfba1`）。** resolver（`Resolved`、`receipt_present`、`resolve` 及其两个测试）原样移入 `release/macos/resolve.rs`。`promote.rs` 留下晋升本身，
   以及它写下的布局常量、`check_release` 与 `reuse_record`，后三者改为 `pub(super)` 供 resolver 使用。`ready.rs`、`mod.rs`、`mac_ledger/append.rs`
   改指新模块；P18-P04、P18-G02 引用的行号按测试名重新生成。
2. **已拆（`d5aa88a`）。** `mac_ledger/record.rs` 留下表示：类型、共用的小函数、`validate` 与 `normalized`。另外三个子模块：
   - `record/codec.rs`：一行 JSON 的写与读，以及 `read_all`；
   - `record/consistency.rs`：`consistent_with`；
   - `record/evidence.rs`：`from_evidence`。它把运行证据读成记录，是本条没有点名的第四种职责，一并分出。

   原有的 `record::` 路径经再导出保留。原先错挂在 `label` 上的 `consistent_with` 文档注释放回原处。
3. **USER 选拆分（`a4c34c1`）。** `cli.rs` 留下 `CommandLine`、`StandaloneRecord`、命令解析与 `package-path`（385 行）。其余移入 `cli/`：
   - `phase.rs`、`manual.rs`、`qualification.rs`、`options.rs`、`args.rs`；
   - `tests.rs`：原内联测试，模块路径仍是 `cli::tests`。

   各处 `crate::cli::` 路径经再导出保留。

   三处拆分都是纯搬移。按行排序后与 `dab45d9` 比较，只差路径、可见性、import、模块头与 rustfmt 换行。测试名集合与 `dab45d9` 相同：`stickymd-smoke` 与
   `stickymd-mac` 共 926 个。
4. **已修（`5565384`）。** 计时尾行用 rustc 1.99.1 实测：取一个只有可合并文档测试的 2024 edition 临时 crate，`cargo test --doc -- --list --format terse`
   的 stdout 末行为 `all doctests ran in 0.48s; merged doctests compilation took 0.16s`，加 `--ignored` 时只有这一行。修复如下：
   - `output::doc_listing` 只在文档测试清单中、只在末行、只一次时接受这一行。出现在别处、出现两次、格式不符，或出现在测试二进制的清单里，仍为错误。
   - `listings` 逐个目标列清单：一个目标失败只记为该目标的问题，其他目标保留清单。

   样本即上面的真实输出。
5. **已修（`c1c9b3f`）。** `--driver` 取不到摘要时，错误交给 `r_driver`，与驱动不符同样处理：没有 `mac-gui` 行能 PASS，各行照常判定并输出，
   `driver_sha256` 输出为 `null`。样本：取摘要的错误保留在 `r_driver` 的结果中，在这个结果下 P16-W03 的已记 FAIL 仍为 FAIL。“命令不再提前退出”本身
   没有单元样本，因为 `ready_command` 要 R 的干净检出。
6. **已修（`629a52a`）。** `count_keys` 先收集名字并传播逐项错误，再计数。大小写敏感卷的测试在因此失败之前先卸载映像；只保留名字，没有目录项让卷
   保持打开（S06 的教训）。没有单元样本，因为读不出的目录项无法按需制造。显式运行
   `cargo test -p stickymd-mac -- --ignored on_a_case_sensitive_volume_case_makes_another_key` 通过，没有留下新挂载的映像。
   第 6 项改的是 `apps/stickymd-mac` 的测试，属于 macOS 平台的构建输入；升版本号本来就要使两个平台都在 R 上重建。

**拆分带出的三处问题，在随后的提交中修正。** 拆分时测试名的比较与 macOS 上的 clippy 都没有发现它们；全量测试、Windows 目标的 clippy 与模块指纹的治理测试发现了。
- `resolve` 模块漏了 `promote` 的 `#[cfg(target_os = "macos")]`：rustfmt 按字母重排 `mod` 声明，新行没有带上条件。`stickymd-smoke` 因此在 Windows 目标上不能编译。
- `ci/selection.rs` 把 `cli.rs` 列为改动即全量跑 CI 的共享路径，`cli/` 下的文件不在其中。样本在修正前为红：`cli/options.rs` 的改动不会选全量。现已补上 `cli/`，样本加了这两条路径。
- 子模块原名 `cli/manual`，`manual::` 路径触发了模块指纹的“只有人工模块代码才点名人工模块”扫描（误报）。已改名为 `cli/acceptance`，不放宽扫描。

**删去守卫的变异：** 每次只删一处守卫，相应测试都失败；恢复后与原文件逐字节相同，测试通过。
- 第 4 项的两处：不认尾行；一处失败清空全部清单。
- 第 5 项：`r_driver` 忽略取摘要的错误。

**验证**（本机 macOS 27.2 arm64，`6791f02`）：
- `cargo fmt --all --check` 通过。
- clippy `-D warnings` 无警告：八个 macOS 包，以及 `stickymd-smoke --target x86_64-pc-windows-msvc`。
- 八个包的 `cargo test --locked`：23 个测试套件，2097 个通过，0 个失败，33 个忽略。其中 `stickymd-smoke` 单元测试 782 个、集成测试 20 个，
  `stickymd-mac` 124 个。
- 拆分前（`dab45d9`）的同一组测试：`stickymd-smoke` 775 + 20 个，`stickymd-mac` 124 个。新增的 7 个是本轮的样本。
- 新测试在 Windows 与 Linux 上没有实际运行，由 CI 运行。

**仍未做：** 本节之前列出的、对整个就绪门的一次完整跨模型复审，仍须在冻结 R 之前做，且没有 BLOCKER。

另见：本机仍挂着上文提到的旧映像 `stickymd-test-root-case-sensitive-62516-…`，本轮未动。

## Resolution：整门跨模型复审（2026-11-26）

上一节“仍未做”的那次完整复审已在 `82bf466`（#30 合入后，版本 0.3.0）上做过，见[整门复审报告](2026-11-26-m5-gate-review.md)。按诚实操作者的
威胁模型，有 4 项 BLOCKER 属实，其中之一（忽略测试的归类表还写着 #27 改名前的测试名）使 R 上的源码测试一行在当前代码上必然为 `NOT TESTED`。
所以 R 不能在这个提交冻结；修复范围、降级是否成立与修复后的复审方式，等 USER 决定。
