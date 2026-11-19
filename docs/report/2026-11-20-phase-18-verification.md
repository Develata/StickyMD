# Phase 18 验证记录（M1–M4）

日期：2026-11-20。分支 `claude/reverent-solomon-1e40f0`，未 push。依据：
[分发报告（修订版）](2026-11-19-macos-public-distribution-decision-r2.md)、
[安装版报告（修订版）](2026-11-19-macos-installed-note-location-r2.md)的 Resolution。

## 结论

合同（M1）、安装版（M2）与发布流水线代码（M3）已提交，macOS 本机基线全绿，`mac-test` 中的 `mac-gui` 与安装版冒烟已跑过一次。
两个发布 workflow 与晋升 workflow 写好但**没有 dispatch**：真实候选上的构建、晋升、TAG/DRAFT/PUBLISH 与下载路径（P08、P09、P04、
D01–D05）都要先 push 并由 USER 分别授权。Phase 18 因此仍是 In Progress；`stickymd-smoke phase 18` 按名运行时如实判为未就绪。

## 提交

| 提交 | 内容 |
| --- | --- |
| `99cc41d` | 合入两份报告 |
| `983f76b` | M1 合同落盘 |
| `fcfca73`、`0b5f81b`、`0278057`、`cfed863` | M2 安装版及其两轮审查修正 |
| `2fb96ed`、`86d4595`、`9741289`、`adc6816`、`77314d4` | M3 发布流水线、两平台晋升、Windows 资格化沿用，及审查修正 |
| `ecc8657` | 矩阵记下本地发布演练 |
| `5f47c0d` | 关闭 M3 审查与 Windows 沿用审查的发现 |
| `e302d8e` 及本报告所在提交 | 关闭 `5f47c0d` 的复审发现 |

每个里程碑经过一次 codex-reviewer（GPT，跨模型、只读）审查，修正后再做窄范围复审；复审的处理见“复审与遗留”。**复审之后的修正
（`e302d8e` 与本报告所在提交）没有再经过跨模型审查。**

## 已验证（本轮实际运行）

- **本机基线（本报告所在提交的工作树，macOS 27.2 arm64）：**
  - `cargo fmt --all --check` 通过。
  - `cargo clippy --workspace --exclude stickymd-win --all-targets --locked -- -D warnings` 通过；
    `cargo clippy -p stickymd-smoke --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings` 通过。
  - `cargo test --workspace --exclude stickymd-win --locked`：1705 项通过，0 失败。`stickymd-win` 只在 Windows 编译，未在本机构建。
  - `stickymd-smoke phase 00` PASS。`stickymd-smoke phase 18` 的三个任务（governance、release tooling tests、macOS host tests）
    均 PASSED，随后按名判定就绪时报 P18-P04 等 Automated 行仍是 `NOT TESTED`，这是预期的未就绪，不是回归。
- **第三方声明：** `release notices --platform windows-x64` 与 `--platform macos-arm64` 的输出与改动前逐字节相同
  （Windows 189 个运行时包，macOS 196 个）。回退许可证改为按已审查的 (crate, 版本) 绑定后，两份输出仍不变。
- **SBOM 核对用真实生成物复验：** 收紧后的规则（SPDX 2.3、每个包内文件恰好一条、有 `SPDXRef-` 标识、40 位 SHA-1、SHA-256 等于包内字节，
  包括 `LICENSE.txt` 与 `README.txt`）对演练中 Syft 1.50.0 为同一 ZIP 生成的 SBOM 通过。
- **发布 workflow 的 PowerShell 片段（pwsh 7 模拟）：** 草稿快照函数对单页、两页、无匹配、空列表、重复 tag、缺字段、空响应、tag 大小写不同
  九种响应全部给出预期结果；PUBLISH 的最后一步对“未变、发布前已变、已公开、发布后已变、发布后仍是草稿、tag 已移走”六种响应，只在发布前核对
  通过时发出一次 `draft=false`，任何不符都以非零退出，不再改动 release。
- **Git 视图：** 临时仓库中，graft 让普通 `git merge-base` 把旁支提交当成祖先，沿用判定照样拒绝（`GIT_GRAFT_FILE` 指向空设备，替换对象也已禁用）。
- **Windows 资格化登记：** 在真实提交上走生产入口，S 上运行的证据在只改文档的 R 上登记被拒，R 上运行的证据登记成功。
- **本地发布演练（`77314d4` 的工具，scratch 克隆中构建）：** `build-app.sh` → `macos-package`（绑定 product inputs）→
  `macos-sbom`（钉住的 Syft 1.50.0 darwin-arm64）→ `macos-verify-package` 全部通过；ZIP `2a4c6931…`、SBOM `d0e3894f…`，
  签名方式观察为 `AD_HOC`。
- **`mac-test` 虚拟机（`0278057` 的产品，2026-11-19/20）：**
  - `mac-gui`：PASS 8（W02、W03、W07、W08、W09、W11、K02、E01），FAIL 0，NOT_TESTED 12（W04 离开起点的时间区间不确定；W12 未驱动注销）。
    bundle 的 product inputs `b79609cc…`、可执行文件 `3ffc7f03…`、driver `f130aa13…`。
  - 安装版冒烟：放进 `/Applications` 后 Note Root 为 `~/Library/Application Support/StickyMD/StickyMD`，各层 `drwx------`；第二个进程
    退出且目录树不变；`~/Applications` 得到同一个键；`StickyMD Math.app` 是另一个键，可同时运行；放在别处的文件夹是便携版，写在 app 旁边，
    Application Support 不变。
  - 虚拟机已清理并停止，已告知“下一步任务”会话。
- **Windows 测试的离线执行（在本机 pwsh 7.x 上模拟）：** `release_remote.ps1` 对 TAG、DRAFT、PUBLISH 的四个实际 workflow 步骤
  输出 `REMOTE_STEPS=PASS`；`release_workflow.rs` 的 Windows 测试所抽取的“Verify original candidate workflow identity”步骤，
  对成功、失败结论、错误的 workflow 文件与查询失败四种输入分别得到 0、1、1、23 的退出码与预期调用记录。

## 推断（有依据但未执行）

- **Windows 产品行为不变：** 产品代码没有改动 Windows 宿主与共享层；Windows 的打包文件名、checksum 格式与 SBOM 生成器未变，第三方声明逐字节相同。
  Windows 发布字节相等没有实测：本分支没有在 Windows CI 上运行。
- **Windows 资格化工具的行为变了，且没有在 Windows 上运行：** 模块成功只登记 HEAD 上的运行、Source Freeze 复核提交里的 `Cargo.lock`、Promote
  最后写 candidate receipt、readiness 从首次读取到写回执都持锁、沿用判定的 Git 视图。这些在 macOS 上有单元测试与临时仓库测试，Windows 目标的
  clippy 只证明能通过类型检查。
- **`0278057` 之后的产品改动只在错误路径**（`cfed863`：不可写提示带出 Note Root、探测文件名冲突时的清理），由
  `apps/stickymd-mac` 的单元测试覆盖；虚拟机证据仍是 `0278057` 的产品。重跑应在 dispatch 得到的候选 bundle 上做，那时证据身份才是发布字节。
- **workflow：** 三个 YAML 均能解析；`concurrency` 组按 tag 串行化三个操作；PUBLISH 比对草稿的 release id、`draft` 与 `tag_name`，以及
  各 asset 的 id、名称与大小（不含更新时间；六个文件的内容另在下载后按 SHA-256 核对），并按 release id 发布。发布前后的复读只比较 tag 名，
  不核对版本 tag 是否仍指向 R。GitHub REST 的实际字段（发布列表、运行的 `path` 是否带 `@ref`）按文档推断，已对两种 `path` 形式都放行。

## 未验证

- P18-P08：`release-macos` 在 `xcode-27` 上构建（需要 push 与 USER 授权 dispatch）。
- P18-P09：真实候选上的 TAG、DRAFT、PUBLISH（各需 USER 授权）。
- P18-P04：本地晋升的完整流程（需要一次真实的 `release-macos` 运行）。
- P18-P06：Windows 主机上带沿用的 Promote、resolver 与 readiness 完整流程。
- P18-P07 的 workflow 一级、Windows PowerShell 5.1 宿主上的两个 Windows 测试、本分支的 Windows CI。
- P18-D01–D05：候选包上的下载路径；R03 探针接收候选 ZIP 的部分在验收阶梯分支上。
- M5：macOS 就绪核对（`11` #macos-release-gate）尚未实现，下一版本发布前必须完成。

## 需要 Develata 决定

1. **构建输入的边界：** 整个 `tools/stickymd-smoke/` 算两个平台的构建输入（打包、声明与校验都在其中），只改验证工具也会让两边都不能沿用，
   比报告批准的“改了的门在同一字节上重跑”更保守。反方向上，复审指出 `release-macos` 跑的工具测试会读 `ci.yml`、`scheduled.yml` 与本地
   cache action，它们按批准的排除表不算输入，只改它们会改变候选的测试结论而不改变指纹。两处都是“构建输入”与“候选上跑的验证”如何划界，
   需要你定：保持现状、把这些文件纳入，或明确哪些验证在 R 上独立重跑。
2. **两份 MIT 回退文本的署名（已按版本处理，剩重建文本待你接受）：** 回退改为按 (crate, 版本) 取上游原文，来源逐行记在
   `assets/licenses/fallback-provenance.md`：9 个 crate 版本照录其上游修订根目录的 MIT 原文（Steven Sheldon）；10 个较新的版本照录上游
   `LICENSE.md`，另附重建的 `objc2-MIT.txt`（上游那些修订没有 MIT 全文）；`dispatch` 0.2.0 上游没有许可证文件，仍用重建的 `dispatch-MIT.txt`。
   两份重建文本的署名是否完整上游无法确认（objc2 #826），`dispatch2` 还列有作者 Mary；首个公开 macOS 包之前由你决定是否接受。
3. **Windows 本地 Promote 按 id 下载（已做）：** 先向 GitHub 核对收据记录的 artifact id 的名称、未过期、所属 run 与 head，再按该 id 下载；
   `11` 与发布清单已同步。按 attempt 收紧 Windows 仍按 `11` 另行决定。
4. **push 与 dispatch：** 什么时候 push 本分支，以及 `release`、`release-macos`、`promote-release` 的 dispatch，都等你授权。
5. **PUBLISH 的写入约束（workflow 部分已做，仓库设置等你）：** 不自动改回草稿；公开前后按 release id 复读并核对版本 tag 仍指向 R，
   不符即失败。`11` 发布晋升第 5 条与发布清单写明你在授权 PUBLISH 时的保证：仓库 ruleset 禁止更新与删除 `refs/tags/v*`（需要你在仓库设置里
   建立），列清能写 release 的人、App、token 与 workflow 并暂停其他自动写入，PUBLISH 运行期间不手动改草稿与 tag。

## 复审与遗留

`5f47c0d` 的两份复审（发布工具、Windows 资格化）给出 5 项 BLOCKER：4 项已关闭，PUBLISH 的写入窗口（B2）只做了缓解，见下文“仍未处理”。
已关闭的 4 项：

- TAG 在确认 R 属于主线之前就运行了 R 中的 action 与工具：三个操作都在 checkout 之后、任何仓库内代码之前，用 runner 自带的 Git 核对
  `HEAD == R` 与 R 在 `origin/main` 上。
- macOS 晋升的回滚与成功边界：读旧证据只把“不存在”当缺失；按步骤记录改动，失败时逆序撤回（旧 receipt 最后恢复）并报告撤不回的步骤；
  新 receipt 写成即为成功，之后的清理失败只警告。
- readiness 在首次读取之后才取锁：改为一开始就持共享锁，直到回执写完。
- graft 可让非祖先通过沿用判定：所有提交、树、blob 与祖先查询都经同一个 Git 视图，禁用替换对象并忽略 graft。

其余已处理：PUBLISH 的发布列表分页并逐项检查字段，发布前后都按 release id 读取，发布后若不是核对过的 release 与附件就让 job 失败
（不自动退回草稿）；macOS 晋升锁改为操作系统的文件锁，进程被杀也会释放，不再留下挡住下一次晋升的锁文件；resolver 要求 receipt
字段完整、artifact 名与 attempt 对应、沿用记录与 Promote 写出的逐字节相同、S=R 时不得有沿用记录；workflow 文件路径的 `@` 后缀不能是另一个
workflow 文件名；composite action 先把所有 id 与 attempt 校验为正整数，并核对 GitHub 回答的 artifact id；Windows 的 attempt 取自同一份已核对的
运行观察，发布说明写明是“最新 attempt”；独立的 `platform-inputs` 命令也要求提交对象；现有 Source Freeze 复核提交里的 `Cargo.lock`。

仍未处理，留给后续或你的决定：

- **PUBLISH 的剩余窗口（B2，以写入约束关闭，见“需要 Develata 决定”第 5 条；平台本身仍无法强制）：** GitHub 没有条件更新，发布前最后一次读取与 PATCH 之间，本 workflow 以外的写入者（例如有写权限的人
  手动改草稿）仍可改动。复审给的关闭条件是“约束所有写入者”或“证明本 workflow 是唯一写入者”，两者都没有做到：现在只有并发组串行化本
  workflow、governance 禁止候选 workflow 写 release，以及发布后按 id 复读、不符即失败。
- **resolver 返回的是固定路径：** 返回之后若另一次晋升替换了它，runner 用到的就是新候选。晋升是本机的手动操作，约定不与 artifact-bound 运行并行；
  要彻底解决需要运行期间持读锁。
- Windows 本地 Promote 原先按 run 与名字下载，已改为按记录的 artifact id 下载（见上文第 3 项）；按 attempt 收紧仍另行决定。
- 进程内沿用判定缓存按仓库路径区分，同一路径的仓库在进程存活期间被替换不会重验；历史证据的相等 SHA 快路径不查对象是否存在
  （查找时该 S 已与当前候选比对）。
- 并发组名按 GitHub 规则不分大小写；版本 tag 的大小写由现有 tag 校验决定，未改。
- 两个 Windows 测试在 Windows PowerShell 5.1 上的实际运行。

## 合并提示

与 `feat/phase-17-copy-notices` 合并时预计冲突：`release/cli.rs` 的 P17/P18 命令、`governance.rs`、coverage 的来源摘要、plan `01` 的元数据。
