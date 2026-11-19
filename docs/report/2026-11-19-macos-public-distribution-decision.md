# macOS 公开分发决定

日期：2026-11-19。基线：`main` `7e2cdeb`。状态：**等待 USER 批准**；批准前不改 `docs/plan`、workflow、打包脚本与发布物。

起因：USER 看到 v0.2.0 发布说明中的“本 Release 不附 macOS 安装包”，认为应当附上。

## 结论

1. **v0.2.0 不附 macOS 包是按合同执行，不是遗漏。** `02` 规定 macOS 只本机构建、暂不公开分发，公开分发需要 Developer ID 与公证，另行决定
   （`02_positioning_and_scope.md:115`、`:137`）。发布流水线只构建 Windows ZIP，0.2.0 的 macOS 构建也没有验收。
2. **建议从下一版本起，在 GitHub Release 附一个 macOS 便携 ZIP**（一个文件夹，内含 `StickyMD.app`）。不给 v0.2.0 补传。
3. **分两步决定。** 第一步批准方向，并做一次 Gatekeeper 打开时的下载实验。第二步按实验结果定签名方式与 README 的首次打开步骤。
   实验之前，无法判断 ad-hoc 签名的下载包对普通用户是否可用。
4. **构建放在 release workflow 的 `xcode-27` runner 上。** 与 Windows ZIP 出自同一次候选构建，走同一套晋升与来源证明，不用本机构建后上传。
5. **macOS 附件单独设门。** 门不过，这一版只发 Windows，发布说明照实写明；Windows 的 readiness 与附件不变。
6. **若选 ad-hoc 公开 ZIP，`02:137` 必须改写。** 现文写的是“公开分发需 Developer ID 与公证”，ad-hoc 公开分发是推翻这句，不是执行“另行决定”。

## 证据等级

- **已验证**：本次读仓库、读 GitHub API 或本机运行得到。
- **推断**：由上述或公开文档推出，未实测。
- **未验证**：需要实验、另一环境或付费账号。

## 现状

**已验证：**

- `02_positioning_and_scope.md:115` 交付形态写“macOS 27+ arm64（本机构建，暂不公开分发）”；`:137` 写“本机 ad-hoc 签名；公开分发需 Developer ID 与公证，另行决定”。
- 这两句来自[平台决定](2026-11-09-macos-platform-decision.md)`:57-58`：当时本机有效签名身份为 0 个，ad-hoc 包经 `spctl` 评估为 rejected，公开 ZIP 留待以后单独决定。
- 发布流水线只有 Windows：
  - `release.yml:28` 的打包 job 运行在 `windows-latest`，`:66-70` 只上传 Windows ZIP、`SHA256SUMS.txt`、`SBOM.spdx.json`；
  - `promote-release.yml:22-27` 只接收 Windows ZIP 与 SBOM 的预期哈希，`:126-133` 只为 Windows ZIP 生成证明，`:144` 的草稿说明写死为 Windows。
- 第三方声明只按 Windows 依赖图生成：`tools/stickymd-smoke/src/release/notices/mod.rs:38`、`:81` 固定为 `stickymd-win` 在 `x86_64-pc-windows-msvc` 上的依赖。
  macOS 宿主的依赖（如 `objc2`）不在其中。
- 验收身份：
  - [v0.2.0 候选记录](2026-11-17-v0.2.0-candidate-preparation.md)`:40-41` 按当时的规则写明，macOS 证据绑定可执行文件的 SHA-256，0.2.0 构建没有重跑。
  - 此后合并的 `b624724`（2026-11-18）把锚点改为**产品输入指纹**（`11_testing_and_release.md:271-276`）。`git merge-base` 确认它不在 v0.2.0 的 source `fc97e83` 中。
  - 指纹包含构建身份：`cargo -V`、`rustc -vV`、`xcode-select -p`、SDK 路径与版本等（`11:276`）。换一台机器或另一个 Xcode 构建，就是新身份，账本里的 PASS 不能沿用。
- 下载路径（P16-R03，`docs/acceptance-cases/phase-16.md:79`，“下载的包在 App Translocation 下提示后退出”，当前指纹下为 `NOT TESTED`）：
  - 2026-11-13 的虚拟机探针中，带隔离标记、解压后未移动的 ad-hoc 包运行在 `/AppTranslocation/` 只读挂载下，提示“请先用访达把 StickyMD.app 移到别处”后以退出码 1 退出，
    下载目录没有任何写入（[VM 验收](2026-11-10-macos-vm-acceptance.md)`:456-467`）。
  - 这次只观察了 Gatekeeper 评估关闭的情况；Gatekeeper 打开时，ad-hoc 包会先被拦下，选“仍要打开”之后是否同样随机化，未验证
    （`tools/manual/macos-r03-translocation.md:8-9`）。
  - 平台决定的更正（`2026-11-09-macos-platform-decision.md:602-610`）：随机化取决于隔离标记，以及包有没有被访达移动过，与签名无关。这一结论只在 ad-hoc 包、Gatekeeper 关闭的条件下测得。
- Apple 支持页“[打开来自未知开发者的 Mac App](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)”（2026-11-19 读取）：
  - 用户须在“系统设置 → 隐私与安全性”中点“仍要打开”。这个按钮只在尝试打开后约一小时内出现，点了还要输入登录密码，之后该 app 记为例外。
  - 该页没有谈 ad-hoc 签名，也没有谈随机化；读取的是默认版本，未确认是否对应 macOS 27。
- CI 的 macOS runner：
  - 源码 CI 的 `macos-latest`（run `44465398229`）实际是镜像 `macos-26-arm64`，系统为 macOS 26.7.2，SDK 最高 macOS 26.5。
  - 本机构建的 app 链接 SDK 27.2，`LC_BUILD_VERSION` 为 `minos 11.0`、`sdk 27.2`（2026-11-14 的 0.1.3 构建）。
  - 预览镜像 `xcode-27`（[runner-images#14404](https://github.com/actions/runner-images/issues/14404)，2026-07-16 起预览，2026-09-16 起底层为 macOS 27）：
    macOS 27.0.1，Xcode 27.0、27.1（默认）、27.2，SDK macOS 27.0 与 27.2，预装 Rust 1.99.1（仓库由 `rust-toolchain.toml` 固定为 1.97.1）。

## 约束：一个公开的 macOS 包要满足什么

1. **目录就是便签身份。** `note/` 与 `StickyMD.app` 同级，不进 bundle；复制整个目录得到第二张便签（`02` macOS 宿主节）。不用安装器（`AGENTS.md:18`）。
2. **发布字节只来自 release workflow。** 本机构建只算 Local Preflight Build（`11:513` 起）；晋升不得重新构建（`11:986-1003`）。
3. **验收绑定发布的那份构建。** CI 构建是新的产品输入指纹，`mac-gui` 的自动化行要在这个身份下重跑；R03 的结论从不沿用（`11:293`）。
4. **下载路径不能丢数据。** 带隔离标记启动时，StickyMD 已有的行为是提示后退出，不写任何文件（R03）。分发方式不能绕开这条。

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
   ├─ README.txt                （macOS 首次打开步骤，按第 4 项实验的结果写）
   ├─ LICENSE.txt
   ├─ THIRD_PARTY_NOTICES.txt   （stickymd-mac 在 aarch64-apple-darwin 上的依赖图）
   └─ licenses/
```

- 用 `ditto -c -k --keepParent` 打包，保留 bundle 的签名与符号链接（推断；R03 探针用的也是它）。
- 不预建 `note/`，与 Windows 相同（`11:1024`）。

### 2. 构建位置：release workflow 的 `xcode-27` runner

| 方案 | 结果 |
| --- | --- |
| **A：release workflow 新增 macOS job，运行在 `xcode-27`**（建议） | 发布字节与 Windows ZIP 出自同一次候选 run，可以按同一 run id 晋升并生成来源证明 |
| B：`macos-latest`（现为 macOS 26） | SDK 最高 26.5，低于验收所用的 27.2；AppKit 有些行为按链接时的 SDK 版本切换（推断），发布的程序与测过的程序可能行为不同。系统也低于 `LSMinimumSystemVersion` 27.0，在 runner 上无法用 LaunchServices 启动 app |
| C：本机构建后上传 | 违反 `11:513`，没有来源证明 |

方案 A 的未验证项：

- `build-app.sh` 能否在该镜像上直接运行：指纹会拒绝仓库外的 Cargo 配置（`tools/stickymd-smoke/src/mac_ledger/build.rs:62-73`），而镜像里有没有 `~/.cargo/config.toml` 未查。
- 镜像默认 Xcode 是 27.1（SDK 27.0）。要与本机的 SDK 27.2 一致，job 需要先选 `Xcode_27.2.app`。
- `xcode-27` 仍是预览标签，可能排队或不稳定；转正后标签可能改名。

### 3. 签名：先做实验，再定

| 方案 | 代价 | 用户第一次打开 |
| --- | --- | --- |
| A：ad-hoc（现状） | 无费用，无密钥 | 被 Gatekeeper 拦下；到“隐私与安全性”点“仍要打开”并输入登录密码（Apple 支持页）。之后是否随机化，未验证 |
| B：Developer ID + 公证 | Apple Developer Program 每年 99 美元（`2026-11-09-macos-platform-decision.md:610`）；证书与公证凭据要作为公开仓库 release workflow 的 secret；公证要把程序上传给 Apple；`build-app.sh:72` 现在用 `--timestamp=none`，公证需要安全时间戳与 hardened runtime（推断），hardened runtime 对本程序有无影响未测 | 不再出现“未知开发者”拦截（推断）。随机化仍取决于隔离标记（推断，Developer ID 下无法在不购买账号时测） |

无论选哪一种，`02:137` 都要改：选 A 改成“公开 ZIP 用 ad-hoc 签名，首次打开需在隐私与安全性中允许”；选 B 要补上签名与公证进入 release workflow 的合同。

建议：第 4 项实验之后再定。

- 若 ad-hoc 包点“仍要打开”后能正常写便签，或者用访达移动整个文件夹就能解除随机化，选 A，README 写这一两步。
- 若随机化在“仍要打开”之后仍在，而移动文件夹也解除不了，A 就只剩让用户在终端执行 `xattr -dr com.apple.quarantine`，这时再权衡 B。

### 4. 一次性实验：Gatekeeper 打开时的下载路径

在 `mac-test` 虚拟机中，用 `main` 的 `build-app.sh` 构建的包，按 `tools/manual/macos-r03-translocation.md` 第 2 步的方法制作带隔离标记的 ZIP，
内容按第 1 项的文件夹结构，再用“归档实用工具”解压。

分工：

- **USER：**
  - 在虚拟机里打开 Gatekeeper 评估（`sudo spctl --global-enable`），实验后恢复（`sudo spctl --global-disable`）。agent 不改安全设置。
  - 点“仍要打开”并输入登录密码。agent 不输入密码。
  - 用访达移动。agent 通过 SSH 驱动访达要用 System Events，会弹自动化授权框。
- **agent：** 制作 ZIP，然后按 R03 的命令判据观测：进程路径是否含 `/AppTranslocation/`、`mount`、stderr、目录列表、`note/` 落在哪里。
  对话框的文字由 USER 目视记录：通过 SSH 截屏缺少屏幕录制授权。

| 情形 | 操作 | 要记下的 |
| --- | --- | --- |
| G1 | 解压后直接双击 `StickyMD/StickyMD.app` | 拦截框文字；点“仍要打开”并输入密码后是否启动、是否随机化、`note/` 位置 |
| G2 | 重新解压，先用访达把整个 `StickyMD/` 文件夹移到另一目录，再打开 | 同上 |
| G3 | 重新解压，用访达只把 `StickyMD.app` 移出文件夹，再打开 | 同上；`note/` 是否落在新位置的上一级 |
| G4 | 重新解压，对文件夹执行 `xattr -dr com.apple.quarantine StickyMD`，再打开 | 是否还有拦截、是否随机化 |

在终端里 `mv` 不算访达移动，所以 G2、G3 只用访达。结果写进本报告的 Resolution 一节，再据此决定第 3 项和 README。

### 5. 发布门：macOS 附件单独判定

- **release workflow（自动）：**
  - 沿用 `build-app.sh:72-79` 的 `codesign --verify --strict` 与仅 arm64 检查，以及 Info.plist 的 lint；
  - 新增 ZIP 内容白名单、checksum、macOS ZIP 的 SBOM，以及按 macOS 依赖图生成的第三方声明。
- **晋升：**
  - candidate 收据（`11:531` 起）增加 macOS ZIP 与 SBOM 的哈希；
  - `promote-release.yml` 增加对应的预期哈希输入，为 macOS ZIP 生成证明；
  - 草稿说明不再写死“Windows”。
- **确切字节上的验收：**
  - 在本机下载候选的 macOS ZIP，核对哈希后解压；
  - 在 `mac-test` 中对这份 bundle 跑 `mac-gui`（新身份，全部自动化行都要跑）；
  - R03 每次都跑。若[六级阶梯提案](2026-11-19-macos-acceptance-ladder.md)获批，R03 由程序判定。
- **人工行：** 按六级阶梯提案的结果。建议剩下的人工行阻断 macOS 附件，与 Windows 的必需人工项一致；逐行 waiver 照旧，不用整版例外。
- **门不过：** 这一版只发 Windows，发布说明写明原因。Windows 的 readiness 与附件不受影响。

### 6. 命名与兼容

- 新增 `StickyMD-<version>-macos-arm64-portable.zip` 与 `SBOM-macos-arm64.spdx.json`（命名为建议）。
  Windows 的文件名、`SBOM.spdx.json` 与晋升脚本的现有输入都不改。
- `SHA256SUMS.txt` 会多一行。Scoop manifest 按文件名从中读取 Windows ZIP 的哈希，多出的一行是否影响它，未验证；发布前在 bucket 的 CI 里核对。

### 7. 时机：下一版本，不补 v0.2.0

- v0.2.0 的发布说明写明不附 macOS 包，现有附件是经过证明的一组字节。
- 从 `fc97e83` 重新构建，得到的是没有验收的新身份。那份源码的 `build-app.sh` 早于指纹锚定，账本也无从复用。

## 改动范围（批准后）

- `docs/plan/02`：`:115` 的交付形态，`:137` 的签名与分发。
- `docs/plan/11`：
  - Phase 16 节或新的阶段节；
  - `:513-531` 的候选来源与 staging；
  - `:986-1003` 的触发与步骤；
  - `:1005-1027` 的发布物。
- 验收：新的阶段矩阵（包检查行、下载路径行）与 `tools/smoke/phase-XX.ps1`。
- workflow：`release.yml` 新增 macOS job；`promote-release.yml` 的输入、证明、附件与草稿说明。
- `tools/stickymd-smoke/src/release/`：声明的目标与包名参数化（`notices/mod.rs:38`、`:81`），macOS ZIP 的打包白名单、SBOM 与晋升收据。
- README（中英）的 macOS 一节，`docs/release-checklist.md`。
- 建议作为一个新阶段实施，编号待定；Phase 17 仍在自己的分支上进行。

## 风险与未决

- 第 4 项实验的结果未知，签名方式与 README 都取决于它。
- `xcode-27` 是预览镜像。
- Apple 支持页的版本未确认，且没有谈 ad-hoc 签名与随机化。
- 部署目标：
  - 平台决定 `:53` 建议部署目标与 `LSMinimumSystemVersion` 同为 27.0；
  - 但 `build-app.sh` 没有设置部署目标，指纹又拒绝调用者传入的 `MACOSX_DEPLOYMENT_TARGET`（`mac_ledger/build.rs:19`），所以二进制用 Rust 的默认值；
  - 2026-11-14 的构建为 `minos 11.0`（只在那份构建上验证）。
  - LaunchServices 按 Info.plist 拒绝低于 27.0 的系统（推断）；直接执行可执行文件则不受限制。
  - 影响小。是否改为 27.0 另行决定，改了会改变指纹。
- macOS 27 以下与 Intel Mac 不能运行，发布说明与 README 写明。
- 方案 B 的证书与公证凭据放在公开仓库的 workflow secret 中，需要限制在受保护的 environment 内（推断）。

## 待 USER 决定

**第一步（现在）：**

1. 方向：从下一版本起附 macOS 便携 ZIP，不补 v0.2.0。建议同意。
2. 构建位置：release workflow 的 `xcode-27` runner。建议同意。
3. 发布门：macOS 附件单独判定，门不过只发 Windows；剩余的人工行阻断 macOS 附件。建议同意。
4. 实验：在 `mac-test` 中做第 4 项的 G1–G4，需要 USER 打开 Gatekeeper、点“仍要打开”并输入密码、用访达移动。建议做。

**第二步（实验之后）：**

5. 签名：ad-hoc 或 Developer ID；`02:137` 按所选改写。
6. README 的首次打开步骤。

## Resolution：审查后由修订版取代（2026-11-19）

- 第 1 轮 codex 审查（gpt-6.2-astra，high，只读）针对本文 `bab80d5` 提出 3 项 BLOCKER：
  - macOS job 与 Windows 同 run，失败会连带拒绝 Windows 候选；
  - 发布物定义与各校验器只认 Windows 三件；
  - “仍要打开”的例外会污染后续实验情形。
  另有 7 项 SHOULD_FIX、1 项 NICE_TO_HAVE、2 项 NEEDS_VERIFICATION，逐项对照代码核实后全部采纳。
- 本文正文保持原样，不再待决，由[修订版](2026-11-19-macos-public-distribution-decision-r2.md)取代。
- 两轮审查各项的处理见修订版末节。
