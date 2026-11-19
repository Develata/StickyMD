# macOS 公开分发决定

日期：2026-11-19。基线：`main` `7e2cdeb`。状态：**等待 USER 批准**；批准前不改 `docs/plan`、workflow、打包脚本与发布物。

起因：USER 看到 v0.2.0 发布说明中的“本 Release 不附 macOS 安装包”，认为应当附上。

## 结论

1. **v0.2.0 不附 macOS 包是按合同执行，不是遗漏。**
   - `02` 规定 macOS 只本机构建、暂不公开分发，公开分发需要 Developer ID 与公证，另行决定（`02_positioning_and_scope.md:115`、`:137`）。
   - 发布流水线只构建 Windows ZIP，0.2.0 的 macOS 构建也没有验收。
2. **建议从下一版本起，在 GitHub Release 附一个 macOS 便携 ZIP**（一个文件夹，内含 `StickyMD.app`）。不给 v0.2.0 补传。
3. **分两步决定。**
   - 第一步：批准方向、构建与发布物的分离方式，并做一次 Gatekeeper 打开时的下载实验。
   - 第二步：按实验结果定签名方式、发布门的具体行与 README 的首次打开步骤。
   - 实验之前，无法判断 ad-hoc 签名的下载包对普通用户是否可用。
4. **macOS 发布物与 Windows 完全分开。**
   - 独立 workflow：`xcode-27` runner；
   - 独立 run，独立的 checksum 文件与 SBOM；
   - 独立的候选收据。
   - Windows 现有的三件套、字节与晋升路径不变，macOS 失败不会拖住 Windows。
5. **macOS 附件单独设门。** 门不过，这一版只发 Windows，发布说明照实写明。
6. **引入这套合同的那一版，Windows 要按改动重新资格化。** `docs/plan/11` 是 Windows 资格化模块指纹的全局输入。
7. **若选 ad-hoc 公开 ZIP，`02:137` 必须改写。** 现文写的是“公开分发需 Developer ID 与公证”，ad-hoc 公开分发是推翻这句，不是执行“另行决定”。

## 证据等级

- **已验证**：本次读仓库、读 GitHub API 或本机运行得到。
- **推断**：由上述或公开文档推出，未实测。
- **未验证**：需要实验、另一环境或付费账号。

## 现状

**已验证：**

- **合同：**
  - `02_positioning_and_scope.md:115` 交付形态写“macOS 27+ arm64（本机构建，暂不公开分发）”；`:137` 写“本机 ad-hoc 签名；公开分发需 Developer ID 与公证，另行决定”。
  - 这两句来自[平台决定](2026-11-09-macos-platform-decision.md)`:57-58`：当时本机有效签名身份为 0 个，ad-hoc 包经 `spctl` 评估为 rejected，公开 ZIP 留待以后单独决定。
- **发布物的定义与校验都只有 Windows：**
  - `01_terminology.md:305-306` 把 Release Exact Artifact 定义为“单一 Windows x64 artifact 集合”，即 portable ZIP、`SHA256SUMS.txt` 与 `SBOM.spdx.json`；
  - `release.yml:28` 的打包 job 运行在 `windows-latest`，`:64-70` 只上传这三件；
  - 本地资格化按 run 的整体结论判定，只接受 `success`（`tools/stickymd-smoke/src/release/workflow.rs:20`，由 `qualification/remote.rs:67` 调用），并只认这三个文件名（`remote.rs:232`）；
  - 晋升拒绝第二个 ZIP（`release/promoted.rs:18-34`）；checksum 校验按 Windows ZIP 与 SBOM 的两行严格比对（`integrity.rs:95` 起）；
  - `promote-release.yml:22-27` 只接收 Windows ZIP 与 SBOM 的预期哈希；
  - 证明分两步：`:125-128` 以整份 `SHA256SUMS.txt` 为 provenance 的 subject，`:129-133` 为 Windows ZIP 附 SBOM 证明；
  - `:144` 的草稿说明写死为 Windows；`:145` 起以固定的三件上传草稿。
- **第三方声明只按 Windows 依赖图生成：** 依赖图的根固定为 `stickymd-win`（`release/notices/graph.rs:49`），目标固定为 `x86_64-pc-windows-msvc`（`notices/mod.rs:38`）。macOS 宿主的依赖（如 `objc2`）不在其中。
- **验收身份：**
  - [v0.2.0 候选记录](2026-11-17-v0.2.0-candidate-preparation.md)`:40-41` 按当时的规则写明，macOS 证据绑定可执行文件的 SHA-256，0.2.0 构建没有重跑。
  - 此后合并的 `b624724`（2026-11-18）把锚点改为**产品输入指纹**（`11_testing_and_release.md:271-276`）。`git merge-base` 确认它不在 v0.2.0 的 source `fc97e83` 中。
  - 指纹由仓库输入与构建身份组成：`cargo -V`、`rustc -vV`、`xcode-select -p`、SDK 路径与版本等（`11:276`），不含机器标识。
  - 账本按指纹复用 PASS，允许 app 字节不同（`11:292`，`mac_ledger/decide.rs:51` 起）。
  - `mac-gui` 跳过 R03（`mac_gui/scenarios.rs:165-168`），所以 R03 不由它判定。
- **下载路径：**
  - P16-R03（`docs/acceptance-cases/phase-16.md:79`，“下载的包在 App Translocation 下提示后退出”）在当前指纹下为 `NOT TESTED`。
  - 历史收据（[VM 验收](2026-11-10-macos-vm-acceptance.md)`:510`）只限 Gatekeeper 评估关闭的条件。
  - 那次观察（`:456-467`）：一个带隔离标记、解压后未移动的 ad-hoc 包运行在 `/AppTranslocation/` 只读挂载下，提示“请先用访达把 StickyMD.app 移到别处”后以退出码 1 退出，所观察的下载目录没有写入。
  - 程序的行为由可写检查决定：
    - 启动先检查程序目录可写，不可写就提示后退出（`apps/stickymd-mac/src/main.rs:67-76`）；
    - `is_translocated()` 只用来选提示文字（`paths.rs:68-74`）。
    - 所以“提示后退出、不写文件”成立的前提是目录不可写，不是带隔离标记本身。
  - 已证实的只有一点：在那个环境里，不需要 Developer ID 也能触发随机化。
  - 平台决定的更正（`2026-11-09-macos-platform-decision.md:602-610`）说随机化与签名无关。但它只有 ad-hoc、Gatekeeper 关闭这一个样本，对其他签名方式是推断。
  - Gatekeeper 打开时，ad-hoc 包会先被拦下，选“仍要打开”之后是否同样随机化，未验证（`tools/manual/macos-r03-translocation.md:8-9`）。
- **Apple 支持页**“[打开来自未知开发者的 Mac App](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)”（2026-11-19 读取）：
  - 用户须在“系统设置 → 隐私与安全性”中点“仍要打开”。这个按钮只在尝试打开后约一小时内出现，点了还要输入登录密码，之后该 app 记为例外。
  - 该页没有谈 ad-hoc 签名，也没有谈随机化；读取的是默认版本，未确认是否对应 macOS 27。
- **CI 的 macOS runner：**
  - 源码 CI 的 `macos-latest`（run `44465398229`）实际是镜像 `macos-26-arm64`，系统 macOS 26.7.2，SDK 最高 macOS 26.5。
  - 本机 2026-11-14 构建的 0.1.3：`LC_BUILD_VERSION` 为 `minos 11.0`、`sdk 27.2`。
  - 预览镜像 `xcode-27`（[runner-images#14404](https://github.com/actions/runner-images/issues/14404)，2026-07-16 起预览，2026-09-16 起底层为 macOS 27）：
    macOS 27.0.1，Xcode 27.0、27.1（默认）、27.2，SDK macOS 27.0 与 27.2，预装 Rust 1.99.1（仓库由 `rust-toolchain.toml` 固定为 1.97.1）。
- **Windows 的连带影响：**
  - `docs/plan/11_testing_and_release.md` 在 Windows 资格化模块指纹的全局输入中（`qualification/module_ledger/fingerprint.rs:254`）；
  - 全阶段 readiness 只排除 Phase 16（`governance.rs:853-858`）。

## 约束：一个公开的 macOS 包要满足什么

1. **目录就是便签身份。** `note/` 与 `StickyMD.app` 同级，不进 bundle；复制整个目录得到第二张便签（`02` macOS 宿主节）。不用安装器（`AGENTS.md:18`）。
2. **发布字节只来自 release workflow。** 本机构建只算 Local Preflight Build（`11:507-510`）；唯一来源是 release workflow（`11:513`）；晋升不得重新构建（`11:986-1003`）。
3. **验收绑定发布的那份字节。** 指纹复用是开发期的节省，不足以证明发布字节；R03 的结论从不沿用（`11:293`）。
4. **下载路径不能丢数据。** 程序目录不可写时提示后退出、不写文件（R03）；分发方式不能绕开这条，也不能让用户误开另一张便签。
5. **不能削弱 Windows。** Windows 的三件套、字节、晋升与资格化判定不因 macOS 改变。

## 决定项

### 1. 形态：一个文件夹的 ZIP

| 方案 | 结果 |
| --- | --- |
| **ZIP**（建议） | 解压得到一个文件夹，文件夹就是便签身份，与 Windows 便携 ZIP 一致 |
| DMG | 挂载后只读（推断）。从映像里直接运行会触发可写检查而退出；要求用户先拷出来，比 ZIP 多一步 |
| Homebrew cask | 把 `.app` 装进 `/Applications`，升级时替换 bundle，便签会落在 `/Applications/note`。与 Scoop 按版本建目录的冲突同类（推断） |
| pkg 安装器 | 违反“无安装器”（`AGENTS.md:18`） |

建议的包内结构：

```text
StickyMD-<version>-macos-arm64-portable.zip
└─ StickyMD/
   ├─ StickyMD.app
   ├─ README.txt                （macOS 首次打开与升级步骤，按第 4 项实验的结果写）
   ├─ LICENSE.txt
   ├─ THIRD_PARTY_NOTICES.txt   （以 stickymd-mac 为根、aarch64-apple-darwin 为目标的依赖图）
   └─ licenses/
```

- 用 `ditto -c -k --keepParent` 打包，保留 bundle 的签名与符号链接（推断；R03 探针用的也是它）。
- 不预建 `note/`，与 Windows 相同（`11:1024`）。
- README 写明支持 macOS 27+、Apple Silicon，更早的版本未验收；升级时保留旧 `note/` 的做法与 Windows 相同：退出后连同 `note/` 复制整个目录。

### 2. 构建与发布物：独立的 macOS 集合

| 方案 | 结果 |
| --- | --- |
| **A：独立 workflow（如 `release-macos.yml`），在 `xcode-27` 上，对同一 Source Freeze 单独 dispatch**（建议） | macOS 有自己的 run id 与 artifact，Windows 候选不受它成败影响 |
| B：在现有 `release.yml` 里加 macOS job | 本地资格化按整个 run 的结论判定（`workflow.rs:20`），macOS job 失败会连带拒绝已完好的 Windows 候选 |
| C：`macos-latest`（现为 macOS 26） | SDK 最高 26.5，低于验收所用的 27.2；AppKit 有些行为按链接时的 SDK 版本切换（推断），发布的程序与测过的程序可能行为不同 |
| D：本机构建后上传 | 违反 `11:513`，没有来源证明 |

macOS 集合：

```text
StickyMD-<version>-macos-arm64-portable.zip
SHA256SUMS-macos-arm64.txt
SBOM-macos-arm64.spdx.json
```

- Windows 的 `SHA256SUMS.txt`、`SBOM.spdx.json`、文件名与字节都不变，Scoop 读取的清单因此也不变（文件名为建议）。
- **候选与晋升：**
  - macOS 有自己的候选收据，绑定同一 source、自己的 run 与 attempt、ZIP 与 SBOM 的哈希；
  - 晋升按 run 自行下载，核对字节后原子 staging，与 Windows 的规则相同（`11:500-609`），但用独立的 staging 目录与 resolver。
- **证明：** macOS 的 provenance 以 `SHA256SUMS-macos-arm64.txt` 为 subject，SBOM 证明绑定 macOS ZIP。
- **获准附件集合只有两种：**“仅 Windows”与“Windows + macOS”。
  - macOS 只有在它的候选收据与发布门都成立时才能加入。
  - DRAFT 与 PUBLISH 比对草稿上的**完整文件名集合与哈希**：缺件、多件、残留的旧附件都拒绝。
  - 现在的草稿步骤只上传固定的三件（`promote-release.yml:145` 起），不检查清单外的旧附件（推断）。
- **授权：** TAG、DRAFT、PUBLISH 仍是三项独立的 USER 授权，DRAFT 的授权写明附件集合。
- **`xcode-27` 上的未验证项：**
  - `build-app.sh` 能否直接运行：指纹会拒绝仓库外的 Cargo 配置（`mac_ledger/build.rs:61-81`），镜像里有没有这样的文件未查；
  - 镜像默认 Xcode 是 27.1（SDK 27.0），要与本机的 SDK 27.2 一致，job 需先选 `Xcode_27.2.app`；
  - `xcode-27` 仍是预览标签，可能排队或不稳定，转正后标签可能改名。
  - 实施时先在该 runner 留一份构建记录：工具版本、指纹、解压后的签名校验。

### 3. 签名：先做实验，再定

| 方案 | 代价 | 用户第一次打开 |
| --- | --- | --- |
| A：ad-hoc（现状） | 无费用，无密钥 | 被 Gatekeeper 拦下；到“隐私与安全性”点“仍要打开”并输入登录密码（Apple 支持页）。之后是否随机化，未验证 |
| B：Developer ID + 公证 | Apple Developer Program 每年 99 美元（`2026-11-09-macos-platform-decision.md:610`）；证书与公证凭据作为 workflow secret；公证要把程序上传给 Apple；Apple 要求 hardened runtime 与安全时间戳，而 `build-app.sh:72` 现在用 `--timestamp=none`。hardened runtime 对本程序有无影响，未测 | 不再出现“未知开发者”拦截（推断）。随机化是否仍取决于隔离标记，不买账号无法测 |

方案 B 若选，还需要：

- **能拒绝错误签名的门：** `codesign --verify --strict` 对 ad-hoc 包也通过（`build-app.sh:73`），不能证明签名身份。要核对预期的 Developer ID 身份、hardened runtime、时间戳与公证结果。
- **顺序：** 若附票（staple），先给 `.app` 附票，再制作最终 ZIP，然后才生成哈希、SBOM 与候选收据；晋升阶段不再签名或重新打包。
- **USER 明确批准：** 费用；凭据的持有者；哪些受保护的 environment 与 ref 能调用签名步骤。

无论选哪一种，`02:137` 都要改：选 A 改成“公开 ZIP 用 ad-hoc 签名，首次打开需在隐私与安全性中允许”；选 B 要补上签名与公证进入 macOS workflow 的合同。

### 4. 一次性实验：Gatekeeper 打开时的下载路径

**目的：** 回答两件事：ad-hoc 包对普通用户有没有可用的首次打开路径；README 应该怎么写。

**准备：**

- 用 `main` 的 `build-app.sh` 构建一个包，按第 1 项的结构做成 ZIP，记下 ZIP 的 SHA-256。所有情形用这同一份 ZIP。
- 隔离标记按 `tools/manual/macos-r03-translocation.md` 第 2 步的方法添加；另做一次真实的 Safari 下载，确认模拟的标记与真实下载一致。托管方式实验前再定。
- 从 Tart 基础镜像克隆一台“Gatekeeper 打开”的基准机：USER 在里面执行 `sudo spctl --global-enable`，并用 `spctl --status` 确认状态。
- **每个情形都从这台基准机重新克隆一台一次性虚拟机。** 原因有二：“仍要打开”会把 app 记为例外，前一个情形的授权会污染后一个；实验结束后直接丢弃克隆，不需要把 Gatekeeper 改回去。

**分工：**

- **USER：**
  - 打开 Gatekeeper；
  - 点“仍要打开”并输入登录密码（agent 不输入密码）；
  - 用访达移动文件。通过 SSH 驱动访达要用 System Events，会弹自动化授权框。
  - 目视记录对话框文字：通过 SSH 截屏缺少屏幕录制授权。
- **agent：**
  - 解压前后、移动前后，分别记录 ZIP、外层 `StickyMD/`、`StickyMD.app` 的隔离标记；
  - 记录实际启动进程的 PID、路径与挂载、提示、退出码；
  - 对相关目录做递归前后比对；
  - 能启动时写入一个唯一标记，退出后重开，核对 `note.md` 的字节。

**情形：**

| 情形 | 操作 | 预期与要记下的 |
| --- | --- | --- |
| G1 | 解压后在访达中双击 `StickyMD/StickyMD.app` | 拦截框文字；点“仍要打开”并输入密码后是否随机化；若随机化，应提示后退出且不写文件 |
| G1b | 接 G1：收到提示退出后，用访达把整个 `StickyMD/` 移到空目录 `A/`，再打开 | 是否解除随机化；能写时唯一的新文件应为 `A/StickyMD/note/note.md` |
| G2 | 解压后先用访达把整个 `StickyMD/` 移到空目录 `A/`，再首次打开 | 同 G1b |
| G3 | 解压后用访达只把 `StickyMD.app` 移到空目录 `B/`，再首次打开 | 能写时唯一的新文件应为 `B/note/note.md`；原 `StickyMD/` 目录不变 |
| G4（仅诊断） | 解压后对 `StickyMD/` 执行 `xattr -dr com.apple.quarantine`，再打开 | 是否还有拦截、是否随机化。只作对照，不作推荐步骤 |

- 在终端里 `mv` 不等于访达移动，未测的 `mv` 不作结论。
- 目标目录必须是空的：移进已有便签的目录会打开已有的 `note/`，或唤醒已在运行的实例（`main.rs:62-66`），得到“能写”的假阳性。每个情形都核对原目录与邻近目录没有变化。

**结果到决定的对应：**

| 结果 | 第 3 项的建议 |
| --- | --- |
| G1b 或 G2 能正常写便签 | A：README 写“仍要打开”加“用访达移动整个文件夹”这一两步 |
| 只有 G3 能写 | 交 USER 决定：只移 `.app` 会把便签身份换到新的上一级目录，README 很难写得不出错 |
| 没有“仍要打开”按钮，或 G1–G3 都不能写 | A 只剩让用户执行 `xattr`。交 USER 在 B 与“继续只发 Windows”之间选 |

结果写进本报告的 Resolution 一节。

### 5. 发布门：macOS 附件单独判定

具体行在实验后随合同改动一并列出；这里先定类别与规则。

| 类别 | 判定工具 | 证据绑定 | 可否 waiver |
| --- | --- | --- | --- |
| 包完整性：哈希、签名校验、仅 arm64、Info.plist、ZIP 白名单、SBOM、第三方声明 | macOS workflow 与晋升 | macOS 候选收据 | 不可 |
| 数据安全：R03（程序目录不可写时提示后退出、不写文件） | R03 探针；若[六级阶梯提案](2026-11-19-macos-acceptance-ladder.md)获批，由程序判定 | 本次发布的 ZIP | 不可 |
| 正常首次打开：按选定签名方式，从带隔离标记的 ZIP 打开到能写便签 | 新增一行，判据按第 4 项实验写 | 本次发布的 ZIP | 逐行 USER waiver |
| Phase 16 的自动化行 | `mac-gui`；R01、R02 由 `mac-resources` 测量（`phase-16.md:77-78`） | 本次发布的 bundle | 逐行 USER waiver |
| Phase 16 的人工行 | 阶梯提案获批则按其办法；未获批时沿用现行的 USER 收据 | 本次发布的 bundle | 逐行 USER waiver |

- **按行判定，不看退出码。** 发布门要求逐行检查必需行。
  - `mac-gui` 只在有 `FAIL` 时返回错误，有 `NOT TESTED` 行时仍成功退出（`mac_gui/runner.rs:158-159`）。
  - 缺行、`NOT TESTED`、`FAIL` 或身份不符，都不能形成 macOS 就绪收据。
- **绑定字节，不只绑定指纹。** VM 运行的证据要记下本次发布 ZIP 与 bundle 可执行文件的 SHA-256。账本里同指纹、不同字节的 PASS，在发布门中不算数。
- **判据分开。** “正常下载后能首次打开”与“已随机化时安全退出”是两条不同的判据，不合并为一行。
- **门不过：** 这一版只发 Windows，发布说明写明原因；不使用整版例外。
- **阶段归属：**
  - 新阶段的验收在 Windows readiness 中要像 Phase 16 一样排除（`governance.rs:853-858`），只在点名时判定；
  - 回归测试覆盖“仅 Windows”与“Windows + macOS”两种发布。

### 6. Windows 的影响

- **引入合同的那一版：** 改 `docs/plan/11` 会使 Windows 资格化模块的指纹变化（`fingerprint.rs:254`），已有的模块收据随之失效，这一版要按受影响模块重新资格化 Windows。
  - 改 `docs/plan/01` 是否也在全局输入中，实施时按指纹规则核对。
- **之后的版本：** macOS 门的成败不改变 Windows 的判定标准、附件与晋升输入。

### 7. 时机：下一版本，不补 v0.2.0

- v0.2.0 的发布说明写明不附 macOS 包，现有附件是经过证明的一组字节。
- 从 `fc97e83` 重新构建，得到的是没有验收的新身份。那份源码的 `build-app.sh` 早于指纹锚定，账本也无从复用。

## 改动范围（批准后）

- **`docs/plan/01`：** `:305-306` Release Exact Artifact 的定义，改为按平台的发布物集合。
- **`docs/plan/02`：** `:115` 的交付形态，`:137` 的签名与分发。
- **`docs/plan/11`：**
  - 新的阶段节；
  - `:500-609` 的来源、staging、resolver 与失败路径；
  - `:986-1003` 的触发与步骤；
  - `:1005-1027` 的发布物。
- **验收：** 新的阶段矩阵（包完整性、R03、正常首次打开）与 `tools/smoke/phase-XX.ps1`；`docs/coverage-matrix.md`。
- **workflow：**
  - 新增 macOS 的 release workflow；
  - `promote-release.yml` 的输入、证明、草稿说明，以及附件集合的完整比对。
- **`tools/stickymd-smoke`：**
  - `release/notices` 的依赖图根与目标参数化（`graph.rs:49`、`mod.rs:38`、`mod.rs:81`）；
  - macOS 的打包白名单、SBOM、checksum；
  - `qualification/` 的候选收据、下载与 resolver；
  - `governance.rs` 的阶段选择。
- **README（中英）的 macOS 一节，`docs/release-checklist.md`。**
- **阶段编号：** 建议作为一个新阶段实施，编号待定；Phase 17 仍在自己的分支上进行。

## 风险与未决

- 第 4 项实验的结果未知，签名方式、发布门的“正常首次打开”行与 README 都取决于它。
- `xcode-27` 是预览镜像。
- Apple 支持页的版本未确认，且没有谈 ad-hoc 签名与随机化。
- **部署目标：**
  - 平台决定 `:53` 建议部署目标与 `LSMinimumSystemVersion` 同为 27.0；
  - `build-app.sh` 以只含 `HOME` 与固定 `PATH` 的环境构建（`:34` 起），没有设置部署目标，所以二进制用 Rust 的默认值；2026-11-14 的构建为 `minos 11.0`（只在那份构建上验证）。
  - 直接调用 `mac-ledger inputs` 时会拒绝 `MACOSX_DEPLOYMENT_TARGET`（`mac_ledger/build.rs:19`）。
  - LaunchServices 按 Info.plist 拒绝低于 27.0 的系统（推断）；直接执行可执行文件不受此限。
  - 对外只写“macOS 27+ 支持、更早版本未验收”。是否把二进制下限也定为 27.0 另行决定，改了会改变指纹，并要在最终候选的 Mach-O 上核对。
- 方案 B 的凭据进入公开仓库的 workflow，要限制在受保护的 environment 与 ref 内。

## 待 USER 决定

**第一步（现在）：**

1. **方向：** 从下一版本起附 macOS 便携 ZIP，不补 v0.2.0。建议同意。
2. **构建与发布物：** 独立的 macOS workflow（`xcode-27`）与独立的 checksum、SBOM、候选收据；获准附件集合只有“仅 Windows”与“Windows + macOS”。建议同意。
3. **发布门的规则：** 第 5 项的类别与规则；门不过只发 Windows。建议同意，具体行在实验后列出。
4. **实验：** 做第 4 项。需要 USER 打开 Gatekeeper，点“仍要打开”并输入密码，用访达移动文件，目视记录对话框。建议做。

**第二步（实验之后）：**

5. 签名：ad-hoc 或 Developer ID；`02:137` 按所选改写。
6. 发布门的具体行，以及“正常首次打开”的判据。
7. README 的首次打开与升级步骤。

## 审查记录

- **第 1 轮 codex 审查**（2026-11-19，gpt-6.2-astra，high，只读）针对 `bab80d5`，提出 3 项 BLOCKER、7 项 SHOULD_FIX、1 项 NICE_TO_HAVE、2 项 NEEDS_VERIFICATION。逐项对照代码核实后全部采纳。
- **BLOCKER 的处理：**
  - macOS job 失败会连带拒绝 Windows 候选：改为独立 workflow 与独立 run；
  - 发布物定义与校验只认 Windows 三件：改为独立的 macOS 集合，并把 `01:305` 列入改动范围；
  - “仍要打开”的例外污染后续情形：改为每个情形从基准机重新克隆。
- **SHOULD_FIX 的处理：**
  - G3 的目标目录与假阳性；
  - R03 的前提收窄到“目录不可写”；
  - 指纹与确切字节的区别；
  - 发布门的类别与 waiver 规则；
  - Windows 的重新资格化与阶段排除；
  - 方案 B 的签名门与附票顺序；
  - “不能运行”改为“未验收”。
- **其余：**
  - 三处行号已改到实际执行点；
  - 实验的状态确认改为克隆与丢弃；
  - Scoop 的疑问因 Windows 清单不变而消失。
