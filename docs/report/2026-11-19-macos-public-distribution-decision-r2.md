# macOS 公开分发决定（修订版）

日期：2026-11-19。基线：`main` `7e2cdeb`。状态：**等待 USER 批准**；批准前不改 `docs/plan`、workflow、打包脚本与发布物。

取代[初版](2026-11-19-macos-public-distribution-decision.md)，按两轮 codex 审查修订，处理见末节。

起因：USER 看到 v0.2.0 发布说明中的“本 Release 不附 macOS 安装包”，认为应当附上。

## 结论

1. **v0.2.0 不附 macOS 包是按合同执行，不是遗漏。**
   - `02` 规定 macOS 只本机构建、暂不公开分发，公开分发需要 Developer ID 与公证，另行决定（`02_positioning_and_scope.md:115`、`:137`）。
   - 发布流水线只构建 Windows ZIP，0.2.0 的 macOS 构建也没有验收。
2. **建议从下一版本起，在 GitHub Release 附一个 macOS 便携 ZIP**（一个文件夹，内含 `StickyMD.app`）。不给 v0.2.0 补传。
3. **分两步决定。**
   - 第一步批准方向、发布物的分离方式、发布门的规则，并做一次 Gatekeeper 打开时的下载实验。
   - 第二步按实验结果定签名方式、发布门的具体行，以及 README 的首次打开与升级步骤。
   - 实验之前，无法判断 ad-hoc 签名的下载包对普通用户是否可用。
4. **macOS 发布物与 Windows 完全分开。**
   - 独立的 workflow（`xcode-27` runner）与 run；
   - 独立的 checksum 文件、SBOM 与候选收据。
   - Windows 现有的三件套、字节与晋升路径不变，macOS 失败不会拖住 Windows。
5. **macOS 附件单独设门。** 门不过，这一版只发 Windows，发布说明照实写明。最终候选从真实下载到再次打开的一遍，必须通过，不可 waiver。
6. **引入这套合同的那一版，Windows 要整体重新资格化。** `docs/plan/` 下的改动命中 Windows 资格化的全部模块。
7. **若选 ad-hoc 公开 ZIP，`02:137` 必须改写。** 现文写的是“公开分发需 Developer ID 与公证”，ad-hoc 公开分发是推翻这句，不是执行“另行决定”。

## 证据等级

- **已验证**：本次读仓库、读 GitHub API 或本机运行得到。
- **推断**：由上述或公开文档推出，未实测。
- **未验证**：需要实验、另一环境或付费账号。

## 现状

### 已验证

- **合同：**
  - `02_positioning_and_scope.md:115` 交付形态写“macOS 27+ arm64（本机构建，暂不公开分发）”；`:137` 写“本机 ad-hoc 签名；公开分发需 Developer ID 与公证，另行决定”。
  - 这两句来自[平台决定](2026-11-09-macos-platform-decision.md)`:57-58`：当时本机有效签名身份为 0 个，ad-hoc 包经 `spctl` 评估为 rejected，公开 ZIP 留待以后单独决定。
- **发布物的定义与校验都只有 Windows：**
  - `01_terminology.md:307-308` 把 Release Exact Artifact 定义为“单一 Windows x64 artifact 集合”，即 portable ZIP、`SHA256SUMS.txt` 与 `SBOM.spdx.json`；
    `:314-317` 的 Promoted Candidate 固定为单一 staging，绑定 ZIP/EXE/SBOM；
  - `release.yml:28` 的打包 job 运行在 `windows-latest`，`:64-70` 只上传这三件；
  - 本地资格化：
    - 按指定 attempt 查询 run 的结论，只接受 `success`（`qualification/remote.rs:40-48`、`:67`，`release/workflow.rs:20`）；
    - 但按整个 run 列出同名 artifact（`remote.rs:308-312`）；
    - 只认这三个文件名（`remote.rs:232`）；
  - 晋升拒绝第二个 ZIP（`release/promoted.rs:18-34`）；checksum 校验按 Windows ZIP 与 SBOM 的两行严格比对（`integrity.rs:95` 起）；
  - `promote-release.yml`：
    - `:22-27` 只接收 Windows ZIP 与 SBOM 的预期哈希；
    - 证明分两步：`:125-128` 以 `SHA256SUMS.txt` 中列出的文件（ZIP 与 SBOM）为 provenance 的 subjects，`:129-133` 为 Windows ZIP 附 SBOM 证明；
    - `:144` 的草稿说明写死为 Windows，`:145` 起以固定的三件上传草稿。
- **第三方声明只按 Windows 依赖图生成：** 依赖图的根固定为 `stickymd-win`（`release/notices/graph.rs:49`），目标固定为 `x86_64-pc-windows-msvc`（`notices/mod.rs:38`）。macOS 宿主的依赖（如 `objc2`）不在其中。
- **验收身份：**
  - [v0.2.0 候选记录](2026-11-17-v0.2.0-candidate-preparation.md)`:40-41` 按当时的规则写明，macOS 证据绑定可执行文件的 SHA-256，0.2.0 构建没有重跑。
  - 此后合并的 `b624724`（2026-11-18）把锚点改为**产品输入指纹**（`11_testing_and_release.md:271-276`）。`git merge-base` 确认它不在 v0.2.0 的 source `fc97e83` 中。
  - 指纹由仓库输入与构建身份（`cargo -V`、`rustc -vV`、`xcode-select -p`、SDK 路径与版本等，`11:276`）组成，不含机器标识；换一台机器构建可能改变指纹，以实算为准。
  - 账本按指纹复用 PASS，允许 app 字节不同（`11:292`，`mac_ledger/decide.rs:51` 起）。
  - `mac-gui` 的证据同时记下可执行文件与 bundle 清单的 SHA-256（`mac_gui/runner.rs:230`）。它只在有 `FAIL` 时返回错误，有 `NOT TESTED` 行时仍成功退出（`runner.rs:158-159`）。
  - `mac-gui` 跳过 R03（`mac_gui/scenarios.rs:165-168`）。
  - Phase 16 的自动化行由不同工具持有，例如：
    - S01 由存储测试判定（`phase-16.md:18`）；
    - P01 检查本地 ad-hoc 打包（`:76`）；
    - R01、R02 由 `mac-resources` 测量（`:77-78`）；
    - V03 还用 `mac-latency`（`:89`）。
- **下载路径：**
  - P16-R03（`phase-16.md:79`，“下载的包在 App Translocation 下提示后退出”）在当前指纹下为 `NOT TESTED`；历史收据只限 Gatekeeper 评估关闭的条件（[VM 验收](2026-11-10-macos-vm-acceptance.md)`:510`）。
  - 那次观察（`:456-467`）：一个带隔离标记、解压后未移动的 ad-hoc 包运行在 `/AppTranslocation/` 只读挂载下，提示“请先用访达把 StickyMD.app 移到别处”后以退出码 1 退出，所观察的下载目录没有写入。
  - 程序的行为由可写检查决定：启动先检查程序目录可写，不可写就提示后退出（`apps/stickymd-mac/src/main.rs:67-75`）；`is_translocated()` 只用来选提示文字（`paths.rs:68-74`）。
  - 可写检查先创建探针文件，再写入、同步、改名，失败时尝试删除（`paths.rs:76` 起）。所以“不写文件”只对那次的只读挂载成立：只读挂载下，创建就失败了。
  - 已证实的只有一点：在那个环境里，不需要 Developer ID 也能触发随机化。
- **Apple 支持页**“[打开来自未知开发者的 Mac App](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)”（2026-11-19 读取）：
  - 用户须在“系统设置 → 隐私与安全性”中点“仍要打开”。这个按钮只在尝试打开后约一小时内出现，点了还要输入登录密码，之后该 app 记为例外。
  - 该页没有谈 ad-hoc 签名，也没有谈随机化；读取的是默认版本，未确认是否对应 macOS 27。
- **CI 的 macOS runner：**
  - 源码 CI 的 `macos-latest`（run `44465398229`）实际是镜像 `macos-26-arm64`，系统 macOS 26.7.2，SDK 最高 macOS 26.5。
  - 本机 2026-11-14 构建的 0.1.3：`LC_BUILD_VERSION` 为 `minos 11.0`、`sdk 27.2`。
  - 预览镜像 `xcode-27`（[runner-images#14404](https://github.com/actions/runner-images/issues/14404)，2026-07-16 起预览，2026-09-16 起底层为 macOS 27）：
    macOS 27.0.1，Xcode 27.0、27.1（默认）、27.2，SDK macOS 27.0 与 27.2，预装 Rust 1.99.1（仓库由 `rust-toolchain.toml` 固定为 1.97.1）。
- **Windows 的连带影响：**
  - `docs/plan/` 与 `docs/acceptance-cases/` 下的改动命中 Windows 资格化的全部模块（`qualification/module_ledger/fingerprint.rs:218-219`）；
  - 全阶段 readiness 只排除 Phase 16（`governance.rs:859-862`）。

### 推断与未验证

- 平台决定的更正（`2026-11-09-macos-platform-decision.md:602-610`）说随机化与签名无关，但只有 ad-hoc、Gatekeeper 关闭这一个样本。对其他签名方式，这是推断。
- Gatekeeper 打开时 ad-hoc 包会先被拦下：这是 `tools/manual/macos-r03-translocation.md:9` 的说法，未在本环境实测。
- 选“仍要打开”之后是否仍随机化，未验证。

## 约束：一个公开的 macOS 包要满足什么

1. **目录就是便签身份。** `note/` 与 `StickyMD.app` 同级，不进 bundle；复制整个目录得到第二张便签（`02` macOS 宿主节）。不用安装器（`AGENTS.md:18`）。
2. **发布字节只来自 release workflow。** 本机构建只算 Local Preflight Build（`11:507-510`）；唯一来源是 release workflow（`11:513`）；晋升不得重新构建（`11:986-1003`）。
3. **验收绑定发布的那份字节。** 指纹复用是开发期的节省，不足以证明发布字节；R03 的结论从不沿用（`11:293`）。
4. **下载路径不能丢数据，也不能让用户误开另一张便签。**
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
- README 写明支持 macOS 27+、Apple Silicon，更早的版本未验收。
- 升级步骤：
  1. 退出旧程序；
  2. 保留旧目录不动；
  3. 把旧的 `note/` 复制进新解压的 `StickyMD/`；
  4. 打开新程序，核对内容后再删旧目录。

### 2. 构建与发布物：独立的 macOS 集合

| 方案 | 结果 |
| --- | --- |
| **A：独立 workflow（如 `release-macos.yml`），在 `xcode-27` 上，对同一 Source Freeze 单独 dispatch**（建议） | macOS 有自己的 run 与 artifact，Windows 候选不受它成败影响 |
| B：在现有 `release.yml` 里加 macOS job | 本地资格化按 run 的结论判定（`workflow.rs:20`），macOS job 失败会连带拒绝已完好的 Windows 候选 |
| C：`macos-latest`（现为 macOS 26） | SDK 最高 26.5，低于验收所用的 27.2；AppKit 有些行为按链接时的 SDK 版本切换（推断），发布的程序与测过的程序可能行为不同 |
| D：本机构建后上传 | 违反 `11:513`，没有来源证明 |

macOS 集合：

```text
StickyMD-<version>-macos-arm64-portable.zip
SHA256SUMS-macos-arm64.txt
SBOM-macos-arm64.spdx.json
```

Windows 的 `SHA256SUMS.txt`、`SBOM.spdx.json`、文件名与字节都不变，Scoop 读取的清单因此也不变。文件名为建议。

**候选与晋升：**

- macOS 的候选收据绑定以下各项：
  - 同一 source；
  - 成功的 run 与 attempt，以及**该 attempt 实际产出的 artifact id**；
  - 三件文件各自的 SHA-256，包括 checksum 文件本身。
- 晋升按 artifact id 下载，核对字节后原子 staging，与 Windows 的规则相同（`11:500-609`），但用独立的 staging 目录与 resolver。
- 现有 Windows 代码按 attempt 查结论、按整个 run 列 artifact（`remote.rs:40-48`、`:308-312`）。是否一并收紧，另行判断。

**证明：** macOS 的 provenance 以 `SHA256SUMS-macos-arm64.txt` 列出的文件为 subjects，SBOM 证明绑定 macOS ZIP。

**获准附件集合与授权：**

- 获准附件集合只有两种：“仅 Windows”与“Windows + macOS”。macOS 只有在它的候选收据与发布门都成立时才能加入。
- DRAFT 的授权写明附件集合及各文件的摘要。
- PUBLISH 的授权引用 DRAFT 已批准的那一组，并比对草稿上的**完整文件名集合与哈希**：缺件、多件、残留的旧附件都拒绝。
- 现在的草稿步骤只上传固定的三件（`promote-release.yml:145` 起），不检查清单外的旧附件（推断）。
- TAG、DRAFT、PUBLISH 仍是三项独立的 USER 授权。

**`xcode-27` 上的未验证项：**

- `build-app.sh` 能否直接运行：指纹会拒绝仓库外的 Cargo 配置（`mac_ledger/build.rs:61-81`），镜像里有没有这样的文件未查。
- 镜像默认 Xcode 是 27.1（SDK 27.0），要与本机的 SDK 27.2 一致，job 需先选 `Xcode_27.2.app`。
- `xcode-27` 仍是预览标签，可能排队或不稳定，转正后标签可能改名。

实施时先在该 runner 留一份构建记录：工具版本、指纹、解压后的签名校验。

### 3. 签名：先做实验，再定

| 方案 | 代价 | 用户第一次打开 |
| --- | --- | --- |
| A：ad-hoc（现状） | 无费用，无密钥 | 预计被 Gatekeeper 拦下，要到“隐私与安全性”点“仍要打开”并输入登录密码（Apple 支持页；本环境待实验）。之后是否随机化，未验证 |
| B：Developer ID + 公证 | Apple Developer Program 每年 99 美元（`2026-11-09-macos-platform-decision.md:610`）；证书与公证凭据作为 workflow secret；公证要把程序上传给 Apple；Apple 要求 hardened runtime 与安全时间戳，而 `build-app.sh:72` 现在用 `--timestamp=none`；hardened runtime 对本程序有无影响，未测 | 不再出现“未知开发者”拦截（推断）。随机化是否仍取决于隔离标记，不买账号无法测 |

方案 B 若选，还需要：

- **能拒绝错误签名的门：** `codesign --verify --strict` 对 ad-hoc 包也通过（`build-app.sh:73`），不能证明签名身份。要核对预期的 Developer ID 身份、hardened runtime、时间戳与公证结果。
- **顺序：** 若附票（staple），先给 `.app` 附票，再制作最终 ZIP，然后才生成哈希、SBOM 与候选收据；晋升阶段不再签名或重新打包。
- **P01 改为按签名方式核对：** P01 现在要求“ad-hoc 签名、`codesign --verify` 通过”（`phase-16.md:76`），选 B 后要随之修改。
- **USER 明确批准：** 费用；凭据的持有者；哪些受保护的 environment 与 ref 能调用签名步骤。

无论选哪一种，`02:137` 都要改：选 A 改成“公开 ZIP 用 ad-hoc 签名，首次打开需在隐私与安全性中允许”；选 B 要补上签名与公证进入 macOS workflow 的合同。

### 4. 一次性实验：Gatekeeper 打开时的下载路径

**目的：** 回答两件事：ad-hoc 包对普通用户有没有可用的首次打开路径；README 应该怎么写。

**准备：**

- **包：** 用 `main` 的 `build-app.sh` 构建一个包，按第 1 项的结构做成 ZIP，记下 ZIP 的 SHA-256。所有情形都用这同一份 ZIP。
- **隔离标记：** 按 `tools/manual/macos-r03-translocation.md` 第 2 步的方法添加；另做一次真实的 Safari 下载，确认模拟的标记与真实下载一致。托管方式在实验前定。
- **基准机：**
  - 从 Tart 基础镜像克隆一台“Gatekeeper 打开”的基准机，记下镜像 digest 与 `sw_vers`。
  - USER 在里面执行 `sudo spctl --global-enable`，并用 `spctl --status` 确认。
  - 基准机上从未打开过这个 app。
- **一次性克隆：** 每个独立情形都从基准机重新克隆一台虚拟机，并在克隆里再确认一次 `spctl --status`。
  - 原因一：“仍要打开”会把 app 记为例外，前一个情形的授权会污染后一个。
  - 原因二：实验结束直接丢弃克隆，不需要把 Gatekeeper 改回去。

**分工：**

- **USER：**
  - 打开 Gatekeeper；
  - 点“仍要打开”并输入登录密码（agent 不输入密码）；
  - 用访达移动文件。通过 SSH 驱动访达要用 System Events，会弹自动化授权框；
  - 目视记录对话框文字。通过 SSH 截屏缺少屏幕录制授权。
- **agent：**
  - 解压前后、移动前后，分别记录 ZIP、外层 `StickyMD/`、`StickyMD.app` 的隔离标记；
  - 记录实际启动进程的 PID、路径与挂载、提示、退出码；
  - 对相关目录做递归前后比对；
  - 能启动时写入一个唯一标记，退出后重开，核对标记。

**判据：** 能写，指同时满足以下几条：

- 唯一标记只出现在目标便签的 `note/note.md`；
- 新文件只出现在目标便签的 `note/` 下，正常布局含 `note.md` 与 `config.toml`；
- 原解压目录与邻近目录没有变化；
- 退出重开后标记仍在。

**情形：**

| 情形 | 操作（顺序） | 记下的 |
| --- | --- | --- |
| G1 | 克隆 1：解压后在访达中双击 `StickyMD/StickyMD.app`；被拦下就点“仍要打开”并输入密码 | 是否被拦、拦截框文字；之后能写，还是随机化后提示退出（此时应不写文件） |
| G1b | 只在 G1 随机化后提示退出时做，沿用克隆 1：用访达把整个 `StickyMD/` 移到空目录 `A/`，再打开 | 是否解除随机化、能否写 |
| G2 | 克隆 2：解压后先用访达把整个 `StickyMD/` 移到空目录 `A/`，再首次打开；被拦下就点“仍要打开” | 同 G1 |
| G3 | 克隆 3：解压后用访达只把 `StickyMD.app` 移到空目录 `B/`，再首次打开；被拦下就点“仍要打开” | 能写时目标便签是 `B/note/`；原 `StickyMD/` 不变 |
| G4（仅诊断） | 克隆 4：解压后对 `StickyMD/` 执行 `xattr -dr com.apple.quarantine`，再打开 | 是否还有拦截、是否随机化。只作对照，它的结果不用来推出任何推荐步骤 |

- 在终端里 `mv` 不等于访达移动，未测的 `mv` 不作结论。
- 目标目录必须是空的：移进已有便签的目录会打开已有的 `note/`，或唤醒已在运行的实例（`main.rs:62-66`），得到“能写”的假阳性。

**结果到决定的对应：**

| 结果 | 第 3 项的建议与 README |
| --- | --- |
| G1 被拦，“仍要打开”后直接能写 | A；README 写“仍要打开” |
| G1 随机化后退出，G1b 能写 | A；README 按实测顺序写：先“仍要打开”，收到提示后用访达移动整个文件夹再打开 |
| G2 能写（不论 G1b） | A；README 写：先用访达移动整个文件夹，再打开并“仍要打开” |
| 只有 G3 能写 | 交 USER 决定：只移 `.app` 会把便签身份换到新的上一级目录，README 很难写得不出错 |
| G1–G3 都不能写，或没有“仍要打开”按钮 | A 不可用。交 USER 在 B 与“继续只发 Windows”之间选 |
| 根本没有拦截 | 先查该克隆的 `spctl --status`；状态无误再按能否写套用上面各行 |
| 证据不全（标记、PID、目录比对缺一） | 该情形重做，不作结论 |

结果写进本报告的 Resolution 一节。

### 5. 发布门：macOS 附件单独判定

具体行在实验后随合同改动一并列出；这里先定类别与规则。

| 类别 | 判定 | 证据身份 | 可否 waiver |
| --- | --- | --- | --- |
| 包完整性：哈希、签名校验、仅 arm64、Info.plist lint（`build-app.sh:62`）、ZIP 白名单、SBOM、第三方声明 | macOS workflow 与晋升 | macOS 候选收据 | 不可 |
| 真实下载路径：最终候选 ZIP 经真实浏览器下载，访达首开，保存，退出后重开 | 新增一行；判据按第 4 项实验的成功分支写，执行办法在第二步与签名一起定 | 本次发布的 ZIP | 不可 |
| 数据安全：R03（实际触发随机化后提示退出，所观察的目录不写文件） | R03 探针；若[六级阶梯提案](2026-11-19-macos-acceptance-ladder.md)获批，由程序判定，没有触发随机化就是 `NOT TESTED`（该提案 `:67`） | 本次发布的 ZIP | 不可 |
| Phase 16 中依赖 app 运行的行 | 由各自的工具判定：`mac-gui`、`mac-resources`、`mac-latency`，或获批的阶梯办法；未获批时沿用现行的 USER 收据 | 本次发布的 bundle | 逐行 USER waiver |
| Phase 16 中不依赖发布字节的行（如 S01 存储测试） | 源码测试 | 源码身份，与现行规则相同 | 按现行规则 |

规则：

- **按行判定，不看退出码。** `mac-gui` 有 `NOT TESTED` 行时仍成功退出，发布门要逐行检查。
- **waiver 与收据分开。** 不可 waiver 的行必须 PASS。可 waiver 的行，要么 PASS，要么保持 `NOT TESTED` 并有一份绑定本候选的 USER waiver 收据。缺行、`FAIL`、身份不符都不能形成 macOS 就绪收据。
- **绑定字节，不只绑定指纹。** VM 运行证据中的 bundle 清单 SHA-256 与可执行文件 SHA-256，必须分别等于候选 ZIP 解出的那份 bundle。账本里同指纹、不同字节的 PASS，在发布门中不算数。
- **阶梯办法的前提。** 阶梯提案中的程序与 agent 判定，要先按该提案用坏样本证明能判出失败，才能用于发布门。
- **门不过：** 这一版只发 Windows，发布说明写明原因；不使用整版例外。
- **阶段归属：** 新阶段的验收在 Windows readiness 中要像 Phase 16 一样排除（`governance.rs:859-862`），只在点名时判定；回归测试覆盖“仅 Windows”与“Windows + macOS”两种发布。

### 6. Windows 的影响

- **引入合同的那一版：** 改 `docs/plan/01`、`docs/plan/11` 与验收矩阵，会命中 Windows 资格化的全部模块（`fingerprint.rs:218-219`），已有模块收据全部失效。这一版要整体重新资格化 Windows。
- **之后的版本：** macOS 门的成败不改变 Windows 的判定标准、附件与晋升输入。

### 7. 时机：下一版本，不补 v0.2.0

- v0.2.0 的发布说明写明不附 macOS 包，现有附件是经过证明的一组字节。
- 从 `fc97e83` 重新构建，得到的是没有验收的新身份。那份源码的 `build-app.sh` 早于指纹锚定，账本也无从复用。

## 改动范围（批准后）

- **`docs/plan/01`：**
  - `:307-308` Release Exact Artifact 的定义，改为按平台的发布物集合；
  - `:314-317` Promoted Candidate 的单一 staging。
- **`docs/plan/02`：** `:115` 的交付形态，`:137` 的签名与分发。
- **`docs/plan/11`：**
  - 新的阶段节；
  - `:500-609` 的来源、staging、resolver 与失败路径；
  - `:986-1003` 的触发与步骤；
  - `:1005-1027` 的发布物；
  - Phase 16 节中 P01 与签名方式的关系。
- **验收：**
  - 新的阶段矩阵（包完整性、真实下载路径、R03）与 `tools/smoke/phase-XX.ps1`；
  - `docs/coverage-matrix.md`。
- **workflow：**
  - 新增 macOS 的 release workflow；
  - `promote-release.yml` 的输入、证明、草稿说明、附件集合的完整比对。
- **`tools/stickymd-smoke`：**
  - `release/notices` 的依赖图根与目标参数化（`graph.rs:49`、`mod.rs:38`、`mod.rs:81`）；
  - macOS 的打包白名单、SBOM、checksum；
  - `qualification/` 的候选收据（按 artifact id）、下载与 resolver；
  - `governance.rs` 的阶段选择。
- **文档：** README（中英）的 macOS 一节，`docs/release-checklist.md`。
- **阶段编号：** 建议作为一个新阶段实施，编号待定；Phase 17 仍在自己的分支上进行。

## 风险与未决

- 第 4 项实验的结果未知，签名方式、真实下载路径的判据与执行办法、README 都取决于它。
- 真实下载路径每版都要过一遍。若选 A，其中“仍要打开”要输入密码，每版都需要 USER 参与一次；这是第 3 项的一项代价。
- `xcode-27` 是预览镜像。
- Apple 支持页的版本未确认，且没有谈 ad-hoc 签名与随机化。
- **部署目标：**
  - 平台决定 `:53` 建议部署目标与 `LSMinimumSystemVersion` 同为 27.0；
  - `build-app.sh` 以只含 `HOME` 与固定 `PATH` 的环境构建（`:34` 起），没有设置部署目标，所以二进制用 Rust 的默认值；
  - 2026-11-14 的构建为 `minos 11.0`，只在那份构建上验证；
  - 直接调用 `mac-ledger inputs` 时会拒绝 `MACOSX_DEPLOYMENT_TARGET`（`mac_ledger/build.rs:19`）；
  - LaunchServices 按 Info.plist 拒绝低于 27.0 的系统（推断）；直接执行可执行文件不受此限。
  - 对外只写“macOS 27+ 支持、更早版本未验收”。是否把二进制下限也定为 27.0 另行决定；改了会改变指纹，并要在最终候选的 Mach-O 上核对。
- 方案 B 的凭据进入公开仓库的 workflow，要限制在受保护的 environment 与 ref 内。

## 待 USER 决定

**第一步（现在）：**

1. **方向：** 从下一版本起附 macOS 便携 ZIP，不补 v0.2.0。建议同意。
2. **构建与发布物：** 采用第 2 项：
   - 独立的 macOS workflow（`xcode-27`）；
   - 独立的 checksum、SBOM 与按 artifact id 的候选收据；
   - 获准附件集合只有“仅 Windows”与“Windows + macOS”。
   建议同意。
3. **发布门的规则：** 第 5 项的类别与规则，含“真实下载路径不可 waiver”，门不过只发 Windows。建议同意，具体行在实验后列出。
4. **实验：** 做第 4 项。需要 USER 参与以下几步。建议做。
   - 在基准机打开 Gatekeeper；
   - 点“仍要打开”并输入密码；
   - 用访达移动文件；
   - 目视记录对话框。

**第二步（实验之后）：**

5. 签名：ad-hoc 或 Developer ID；`02:137` 按所选改写。
6. 发布门的具体行，以及真实下载路径的判据与执行办法。
7. README 的首次打开与升级步骤。

## 审查记录

### 第 1 轮 codex 审查

2026-11-19，gpt-6.2-astra，high，只读，针对初版 `bab80d5`。提出 3 项 BLOCKER、7 项 SHOULD_FIX、1 项 NICE_TO_HAVE、2 项 NEEDS_VERIFICATION，对照代码核实后全部采纳。

- **BLOCKER：**
  - 改为独立 workflow 与 run；
  - 改为独立的 macOS 集合，并把 `01` 列入改动范围；
  - 每个实验情形从基准机重新克隆。
- **SHOULD_FIX：**
  - G3 的空目标目录与假阳性；
  - R03 的前提收窄到“目录不可写”；
  - 指纹与确切字节的区别；
  - 发布门的类别与 waiver 规则；
  - Windows 的重新资格化与阶段排除；
  - 方案 B 的签名门与附票顺序；
  - “不能运行”改为“未验收”。
- **其余：** 行号改到实际执行点；实验的状态确认改为克隆与丢弃；Scoop 的疑问因 Windows 清单不变而消失。

### 第 2 轮 codex 审查

同模型，high，只读，针对修订中间稿 `19e4db3`。13 项中 4 项已解决、9 项部分解决，另有 2 项修订引入的问题。剩余各项都已核实并在本文处理：

- **BLOCKER，实验判据：** 正常首开会同时建 `note.md` 与 `config.toml`（`startup/bootstrap.rs:182` 起的首开测试），所以判据改为“唯一标记只在目标便签、新文件只在其 `note/` 下”。决定表按各分支的实际顺序列出，补上“直接能写”“没有拦截”“证据不全”三种结果；G1b 限定为沿用 G1 的克隆；G4 只作诊断。
- **BLOCKER，waiver 自相矛盾、最终候选可能没走过用户路径：** 新增不可 waiver 的“真实下载路径”行；waiver 行保持 `NOT TESTED` 并另附 USER waiver 收据；R03 必须实际触发随机化；阶梯办法须先过坏样本。
- **SHOULD_FIX，attempt 与 artifact：** 收据改为绑定成功 attempt 实际产出的 artifact id 并按 id 下载；PUBLISH 引用 DRAFT 批准的同一组附件与摘要。
- **SHOULD_FIX，证明的 subjects：** 改正为“清单列出的文件”；收据另记 checksum 文件本身的摘要；`01:314-317` 列入改动范围。
- **SHOULD_FIX，G2 与 G1b 的顺序、升级步骤：** 按分支分别写 README 顺序；升级改为把旧 `note/` 复制进新解压的目录。
- **SHOULD_FIX，证据标签：** “Gatekeeper 打开时先拦下”移入推断与未验证；“不写文件”限定为那次的只读挂载。
- **SHOULD_FIX，bundle 身份：** 发布门要求 bundle 清单 SHA-256 与可执行文件 SHA-256 都与候选一致。
- **修订引入，报告回改：** 中间稿就地重写了初版，违反 `docs/report/AGENTS.md:14`。初版正文已恢复为 `bab80d5` 的原文，文末追加 Resolution，修订内容移到本文。
- **修订引入，自动化行的工具归属：** 改为“由各自工具判定”，并区分源码身份与发布字节身份；选 B 时 P01 随之修改。
- **其余：**
  - `01` 的改动同样命中全部模块，已据此改写第 6 项；
  - 行号 `01:307-308`、`governance.rs:859-862`、`build-app.sh:62` 已改正；
  - 实验记录基准镜像、`sw_vers` 与各克隆的 Gatekeeper 状态。

## 更正（2026-11-19，提交后自查）

本节更正正文，正文不改；与正文冲突处以本节为准。

### 1. Windows 的代价：结论 6 与第 6 项

- **已验证：** 模块指纹在 `fingerprint.rs:215-217` 先排除报告、任务、发布说明等文档，以及人工专用与判定投影文件（`:274-300`、`:313-338`），然后 `docs/plan/` 与 `docs/acceptance-cases/` 下的任何文件都命中全部模块（`:218-219`）。验收矩阵不在排除之列。
- **下一版本本来就要整体重新资格化 Windows（推断）：**
  - Phase 17 分支 `feat/phase-17-copy-notices` 已改动 `docs/plan/01`、`04`、`06`、`07`、`09`、`11` 与多份验收矩阵；
  - v0.2.0 的发布说明（`docs/release-notes/0.2.0.md:5`）写明 Phase 17 属于下一版本。
  - 若 macOS 合同与 Phase 17 落在同一版本，不另增一轮。
- **新的耦合（正文第 6 项“之后的版本”不成立）：**
  - 每次把 macOS 的验收记录写进 `docs/acceptance-cases/` 下的矩阵，都会使 Windows 全部模块的收据失效。
  - Phase 16 现在就在矩阵的 Evidence 列里记录逐次结果。
  - 二选一：
    - 每版的 macOS 证据记在报告与 ignored 证据目录，不改矩阵；
    - 或者把 macOS 矩阵排除出 Windows 的模块指纹。后者是指纹规则的改动，要单独说明理由并经 USER 批准。
  - 实施时定，倾向前者，因为它不改规则。

### 2. 实验中 USER 要做的事：第 4 项

正文少算了登录：

- 每台克隆的地址不同，都要从 Terminal.app 新开一个 SSH master，由 USER 输入虚拟机密码（`mac-test` 的规则）。
- 访达移动与读对话框要求虚拟机带窗口运行，不能用 `--no-graphics`。

逐台估计如下：

| 虚拟机 | SSH 登录 | 管理员密码 | “仍要打开”并输入密码 | 访达移动 | 其他 |
| --- | --- | --- | --- | --- | --- |
| 基准机 | 1 | 1（`spctl --global-enable`） | — | — | — |
| 克隆 1（G1、G1b） | 1 | — | 1 | 0–1（G1b） | 读拦截框 |
| 克隆 2（G2） | 1 | — | 1 | 1 | 读拦截框 |
| 克隆 3（G3） | 1 | — | 1 | 1 | 读拦截框 |
| 克隆 4（G4） | 1 | — | — | — | — |
| Safari 下载 | 在某台克隆中做 | — | — | — | 1 次下载 |

合计：约 5 次登录、1 次管理员密码、3 次“仍要打开”、2–3 次访达移动、1 次下载。实验一次做完。

### 3. “真实下载路径”改为单独的决定项

正文第 5 项把它定为每版都要过、不可 waiver。这一条来自审查，不是 USER 提出的。若选 ad-hoc，每版都要 USER 输入一次密码，与“人工行尽量少”的要求冲突。因此从发布门规则中拿出，单列为第一步的决定项 3b：

| 方案 | 做法 | 代价 |
| --- | --- | --- |
| **甲：只在需要时做**（建议） | 在第 4 项实验中做一次；以后只在签名方式或 macOS 大版本改变时重做。每版仍对最终 ZIP 跑 R03 | 某一版的最终字节可能没走过真实首开路径。依据是 Gatekeeper 的行为取决于签名方式、系统与隔离标记，不取决于程序内容（推断） |
| 乙：每版都做，不可 waiver | 每版对最终 ZIP 走一遍下载、访达首开、保存、退出重开 | 选 ad-hoc 时每版要 USER 输入一次密码；选 Developer ID 时能否由程序完成，未验证 |

正文“待 USER 决定”第 3 项因此改为两项：

- **3a：** 第 5 项的其他类别与规则，门不过只发 Windows；
- **3b：** 真实下载路径选甲还是乙。

### 4. 证据标签

以下三点依据第 1、2 轮审查所引的官方文档，本文作者没有直接读原文：

- 公证要求 hardened runtime 与安全时间戳（第 3 项的方案 B 一行）；
- `actions/attest` 以清单列出的文件为 subjects；
- Scoop 只在 `autoupdate` 时读取 `SHA256SUMS.txt`。

审查记录中“对照代码核实”只指仓库内的代码与文档。
