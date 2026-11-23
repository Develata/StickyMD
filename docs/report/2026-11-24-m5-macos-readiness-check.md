# M5 macOS 就绪核对：实现范围与待决事项

日期：2026-11-24。分支 `feat/m5-macos-readiness`（从 `main` `d9bab10` 开出），未 push。依据：`11` Phase 18 节 M5 行与
#macos-release-gate；[Phase 18 验证记录](2026-11-20-phase-18-verification.md)、[构建输入收窄记录](2026-11-21-platform-build-input-narrowing.md)。

## 结论

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

本分支中由我定下、你可以否决的两点：

- **Windows 行不进 macOS 门**：P16-W15、W16（USER 豁免的 Windows 回归）与 V05（Windows 回归的人工收据）判为“不属于 macOS 发布物”，由 Windows
  readiness 管；P16-P01（打包脚本产出 ad-hoc 签名的 `.app`）由门的“包完整性”一行承担。
- **账本中同一字节上的 FAIL 与撤销**：沿用账本的规则，只把“产品输入指纹”换成“候选的两项字节身份”。一行在候选字节上、同一驱动、镜像与系统下
  有 PASS，且这组身份下没有 FAIL、NOT_TESTED 或被撤销的 PASS，才算 PASS；I01–I03 与 R03 只认 `--batch` 所指的批次。理由同账本：先后无法证明，
  所以任何这类记录都算数。

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

见文末，随实现补充。
