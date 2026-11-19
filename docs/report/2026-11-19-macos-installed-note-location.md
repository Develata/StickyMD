# macOS 安装版的便签位置

日期：2026-11-19。基线：`main` `7e2cdeb`。状态：**等待 USER 批准**；批准前不改 `docs/plan`、代码与 README。

## 起因

USER 原话：“让用户先拖进“应用程序”，很多开源软件都是这么做的。另外，数据位置，StickyMD 把 note/ 写在 app 旁边，而 rss-reader 一开始也是这么做的，但是后面为了 scoop 和 macos 的安装，都做了妥协，我想 stickymd 可能也要做这种妥协，StickyMD 把 note/ 写在 app 旁边仍然是压缩包解压后的默认行为。”

背景见 [macOS 公开分发决定（修订版）](2026-11-19-macos-public-distribution-decision-r2.md)。

## 结论

1. **按 `.app` 所在的位置决定便签放在哪里：**
   - `.app` 直接放在 `/Applications` 或 `~/Applications` 里，是“安装版”，便签在 `~/Library/Application Support/StickyMD/note/`；
   - 其他任何位置，包括解压出来的文件夹，都是“便携版”，便签仍在 `.app` 旁边的 `note/`，与现在完全相同。
2. **只看位置，不看写入是否失败。** 随机化的只读路径、没有写权限的目录，仍然提示后退出，不改用 `~/Library`。
3. **引入“便签根目录”（Note Root）：** 即包含 `note/` 的目录。
   - 便携版的 Note Root 是 Program Directory；安装版是 `~/Library/Application Support/StickyMD/`。
   - 可写检查与单实例身份都改为挂在 Note Root 上。便携版的行为因此不变；装了两个副本的安装版共用一张便签，并且互斥。
4. **Windows 与 Scoop 不变。** 路线 A 已经上线；StickyMD 拒绝 junction，是受管资源所有权证明的一部分，不能照 RSS-Reader 的 `persist` 去做。
5. **这是骨架级改动**，涉及 `01`、`02`、`05`、`08` 与 `AGENTS.md:18`，需要 USER 批准后才能实施。

## 证据等级

- **已验证**：本次读仓库或本机运行得到。
- **推断**：由上述推出，未实测。
- **未验证**：需要实验或另一环境。

## RSS-Reader 的做法（已验证，读源码与 manifest）

- **macOS：** 一律使用 `~/Library/Application Support/RSS-Reader`，没有便携模式。注释给的理由是“an app bundle is read-only once signed”（`RSS-Reader/crates/rssr-infra/src/store/local.rs:6`、`:72-77`）。
- **Linux：** 装在 `/usr/bin` 时用 `$XDG_DATA_HOME/rss-reader`，否则放在可执行文件旁边（`local.rs:4-5`、`:81` 起）。这是“按位置区分安装版与便携版”的先例。
- **Windows / Scoop：** 数据仍在可执行文件旁边；Scoop manifest 用 `"persist": "RSS-Reader"` 把它变成 junction（`scoop-bucket/bucket/rss-reader.json`）。
- **与 USER 记忆的出入：**
  - RSS-Reader 在 macOS 上并不区分安装版与解压版，而是一律放在 `~/Library`；
  - 在 Scoop 上也没有挪动数据，只是接受了 junction。
  - USER 要的“解压版放旁边、安装版另放”，对应的是它的 Linux 规则。

## StickyMD 现状

**已验证：**

- **合同：**
  - Program Directory 是“包含 `StickyMD.exe` 的目录”，是“便签的身份边界与单实例判定单位”（`01_terminology.md:29-34`）；
  - Note Directory 是 `<program-dir>/note/`，“也不是任何用户自定义的任意文件夹”（`01:36-41`）。
  - `02_positioning_and_scope.md`：
    - `:90-97` 规定一张便签模型：一个程序目录就是一张便签，复制整个目录得到第二张；
    - `:99-109` 规定所有数据只在程序目录内，不可写就提示后退出，并且“**禁止** fallback 到 AppData、LocalAppData、Documents、Registry 或任何用户配置目录”。
  - `05_document_persistence.md`：
    - `:104-128` 规定启动顺序：先建单实例，再检查可写，再建 `note/`，并重申 fallback 禁令；
    - `:130-142` 规定 macOS 的实例身份是对 Program Directory 的目录 fd 取 `flock`，唤醒 socket 名取自 `(dev, ino)`。
  - `08_assets_and_export.md:53`：受管资源所有权证明要求目录与文件都不是 reparse point。
  - `AGENTS.md:18`：“纯 portable：解压即用，无安装器，不写 AppData / Registry。”
- **宿主：** `apps/stickymd-mac/src/paths.rs:29-40` 取 `.app` 的上一级作为 Program Directory，`:53` 把 `note/` 放在它下面。宿主代码不读取家目录。
- **“应用程序”文件夹：** `/Applications` 为 `root:admin`、`drwxrwxr-x`，本机用户在 `admin` 组；本机没有 `/Applications/note`。

**推断：** 照现在的规则把 `StickyMD.app` 拖进“应用程序”，会出现两种结果：

- 管理员用户：便签写在 `/Applications/note/`。所有用户共用这一张，“应用程序”里也会多出一个散落的 `note` 文件夹。
- 非管理员用户：目录不可写，提示后退出。

两种都不是用户期望的结果。

## 判定规则的候选

| 方案 | 做法 | 结果 |
| --- | --- | --- |
| **A：按位置**（建议） | `.app` 的父目录 canonical 后恰为 `/Applications` 或 `<家目录>/Applications`，即为安装版；其余都是便携版 | 规则是纯函数，可以单测；随机化路径不匹配，仍走便携版，在只读路径上提示后退出，行为不变；解压版默认便携，符合 USER 的要求 |
| B：一律 `~/Library`（RSS-Reader 在 macOS 上的做法） | 不再有便携版 | 最简单；但放弃了 macOS 上“复制目录得到第二张便签”，与 USER 说的“解压后默认仍在旁边”不符 |
| C：写入失败时回退到 `~/Library` | 先试旁边，失败就换位置 | 同一个包会因为目录权限不同而悄悄换成另一张便签；随机化路径也会回退，用户以为在用下载的那份，便签其实写进了 `~/Library`。违背现有禁令的本意，拒绝 |
| D：标记文件 | 包内或旁边放一个“便携”标记 | 解压版要靠 ZIP 里带标记；用户只拖 `.app` 时标记留在原处，效果与 A 相近，却多一个可能被误删的文件。不建议 |

方案 A 的补充：

- **子文件夹：** `/Applications/工具/StickyMD.app` 这样的子文件夹不算安装版，仍是便携版，与现在相同。规则只认两个确切位置，便于说明与测试。
- **家目录：** 取自用户数据库（如 `getpwuid_r`），不读 `$HOME`（推断的设计），与 `05:137` 不用 `$TMPDIR` 的理由相同：环境变量可以被改，便签位置不应随之漂移。

## 安装版的细节

- **便签根目录：**
  - 安装版的 Note Root 是 `<家目录>/Library/Application Support/StickyMD/`，`note/` 的布局与便携版相同：`note.md`、`config.toml`、`images/`、`.trash/`。
  - Note Root 不存在时创建，权限设为只有本人可访问（推断合适）。
  - Note Root 与 `note/` 都不得是符号链接，与 `08:53` 的所有权证明对应。
- **启动顺序：** 把 `05:109-119` 中的“Program Directory”换成 Note Root：
  1. 解析 Note Root；安装版先创建这个空目录，不写任何文件；
  2. 对 Note Root 建立单实例；
  3. 在 Note Root 内做可写检查与卷检查（`RENAME_EXCL`）；
  4. 创建 `note/`，其余顺序照旧。
  - “第二实例在任何 durable 文件创建前退出”仍然成立：建一个空目录是幂等的，两个实例同时创建也只会有一个成功、另一个看到已存在（推断）。
- **实例身份：**
  - 对 Note Root 的目录 fd 取 `flock`，唤醒 socket 名取自 Note Root 的 `(dev, ino)`，算法不变。
  - 便携版的 Note Root 就是 Program Directory，所以行为不变。
  - 安装版的 `/Applications` 与 `~/Applications` 两个副本共用一个 Note Root，第二个唤醒第一个后退出。
- **模式切换：**
  - 把 app 从“应用程序”挪走或挪进去，会换成另一张便签；原来那张原样留在原处，不会丢失。
  - 用户靠 README 得知这一点。产品内显示便签位置是新功能，超出 v1 的冻结边界，本报告不提。
- **迁移：**
  - 便携版与安装版之间不自动导入，也不自动删除，符合“不自动删除用户文件”。
  - README 写明：先退出，再把旧的 `note/` 复制到 Note Root。
  - RSS-Reader 从 v0.1 升级时也是这样处理的（其 README `:83`）。
- **随机化提示：**
  - 现在的提示是“请先用访达把 StickyMD.app 移到别处”。有了安装版后，更合适的写法是“请先把 StickyMD.app 拖进“应用程序”，或把整个文件夹移到别处”。
  - 另一会话刚写了断言现有文字的 R03 场景，改字要与它协调，实施时再定。

## Windows 与 Scoop

- **不变。** 路线 A（manifest 复制钩子）已于 2026-11-19 上线，`pre_uninstall` 冻结在每份已有的安装里。
- **RSS-Reader 的 `persist` 做法不能照搬：** 它会把 `note/` 变成 junction，而启动时拒绝 junction，这条规则本身就是受管资源所有权证明的一部分（`08:53`）。
- **将来：** 如果 Windows 也要“安装版”，例如 Scoop 安装时把便签放在 persist 目录，可以复用 Note Root 的概念。但判定规则、如何从复制钩子迁移，要另写报告。
- **`AGENTS.md:18`：** “不写 AppData / Registry”对 Windows 仍然成立；macOS 的安装版会写 `~/Library`，这句要改成分平台的写法。

## Debian

Note Root 的概念同样适用于 Debian：安装在系统目录时用 `$XDG_DATA_HOME/stickymd`，否则用便携版。具体规则仍按 `02` 的 Debian 节另行批准，本报告不定。

## 对分发与验收的影响

- **ZIP 结构不变：** 仍是 `StickyMD/StickyMD.app` 加 README，解压后就是便携版。
- **README 写两条路：**
  - 推荐：把 `StickyMD.app` 拖进“应用程序”再打开，得到安装版。
  - 要便携或多张便签：用访达把整个 `StickyMD` 文件夹移到别处再打开。
- **Gatekeeper 实验：** 之前的 G1–G4 是按“移动文件夹”设计的。规则批准后改为围绕“拖进应用程序”重新设计，在此之前暂停。
- **新增验收（推断）：**
  - 判定规则的单元测试：各种路径、子文件夹、随机化路径、开发构建；
  - 安装版首次启动时在 Note Root 下建立布局；
  - 两个安装副本互斥；
  - 随机化路径仍提示后退出。
- **Windows 的连带影响：** 这些 plan 改动会让 Windows 资格化的全部模块收据失效（修订版报告更正第 1 节），与 Phase 17 落在同一版本时不另增一轮（推断）。

## 改动范围（批准后）

- **`01`：** Program Directory 不再无条件是“便签的身份边界”；新增 Note Root；Note Directory 改为 `<note-root>/note/`。
- **`02`：**
  - `:90-97` 一张便签模型；
  - `:99-109` 的“Portable 目录身份”改为“便携版与安装版”；
  - fallback 禁令改写为“禁止按失败回退；模式只由位置决定”；
  - macOS 宿主节表格中的“便签目录”一行。
- **`05`：** `:104-128` 启动顺序与可写检查；`:130-142` 实例身份的对象。
- **`08`：** 所有权证明中的目录改为相对 Note Root。
- **其他：** `AGENTS.md:18`；`docs/features/` 与 `docs/acceptance-cases/` 的投影；README（中英）。
- **代码：** `apps/stickymd-mac/src/paths.rs` 的判定与家目录读取；家目录查询属于原生调用，放在批准的平台目录里。

## 风险与未决

- **外置卷：** 外置卷上的 `/Volumes/X/Applications` 不算安装版。规则只认系统卷上的这两个位置，这一点要写进 README。
- **多用户：** 同一台 Mac 的不同用户，在安装版下各有自己的便签，比现在共用 `/Applications/note` 好。
- **备份：** Time Machine 默认会备份 `~/Library/Application Support`（推断），便签随之进入备份，与便携版放在用户自选目录时的情况不同。
- **随机化下的 Finder 拖动：** 用访达把 `.app` 拖进“应用程序”能解除随机化，这一点有 Apple DTS 的说明（移动会清除随机化状态，[thread 724969](https://developer.apple.com/forums/thread/724969)）；拖进“应用程序”这一具体操作未在本环境实测。

## 待 USER 决定

1. 采用方案 A：只有直接放在 `/Applications` 或 `~/Applications` 里的 `.app` 是安装版，其余都是便携版。
2. 引入 Note Root，可写检查与实例身份改挂在 Note Root 上。
3. 便携版与安装版之间不自动迁移，由 README 说明。
4. Windows 与 Scoop 不变。
5. 随机化提示的文字与另一会话协调后，在实施时定。

另有一项仍待 USER 回答：macOS 公开包是推迟到购买 Developer ID 之后，还是照 RSS-Reader 的方式发 ad-hoc 包。

## Resolution：首轮审查结果，方案暂不推进（2026-11-19）

### 审查与核实

codex 首轮审查（gpt-6.2-astra，high，只读）针对本文提交前的草稿，提出 3 项 BLOCKER、6 项 SHOULD_FIX、1 项 NICE_TO_HAVE、2 项 NEEDS_VERIFICATION。前两项 BLOCKER 已在本机核实。

- **同一个 app 可能选出两张便签（已核实）：**
  - `realpath` 对 `/System/Volumes/Data/Applications/…` 保留 Data 卷前缀，对 `/Applications/…` 则不保留；两处 `stat -f '%d %i'` 相同（`16777233 3945941`）。
  - 所以按路径字符串比较的方案 A 会把同一位置分进两种模式。
  - 修法是按文件系统身份 `(dev, ino)` 比较父目录与安装目录，并把“身份无法确定”单列为提示后退出。
- **加锁的目录与读写的目录可能不是同一个（已核实）：**
  - `crates/stickymd-posix/src/instance.rs:48` 用 `File::open(program_dir)` 按路径打开再加锁，之后的探针、布局与 bootstrap 又各自按路径打开。
  - 中途目录被替换时，两边会分离。
  - 修法是从可信父目录打开 Note Root，在同一个 fd 上加锁，其后的操作都相对这个 fd。
- **手工迁移可能覆盖已有便签（审查意见，未另行实测）：** 安装版首次启动后目标已有正文与恢复文件，“复制旧 `note/` 过去”需要写清：
  - 退出两边的全部副本；
  - 先备份；
  - 只复制到不存在的目标；
  - 两边都有数据时停止，由用户自己选。
- **其余意见：**
  - 外置家目录的规则自相矛盾；
  - 锁前建目录的例外需要单独批准；
  - 互斥不等于唤醒成功；
  - 合同改动表漏了 `01:24,27,45`、`04:443-444`、`09:290-291,342-343`、`05:62,79`、`08:115-118`、`AGENTS.md:16-17` 等处；
  - 末节重提了已决定的签名问题；
  - RSS-Reader 的类比有三处超出证据；
  - 备份与 Finder 拖动的说法需要实测。
- 审查原文在会话 scratchpad（`macdist-installed-location-review-20261119-191544/review.md`），不进仓库。

### 状态

- USER 在审查期间表示“stickymd 几乎不用改，就能发 macos 版本”。
- 第一版 macOS 包可以不做安装版：README 让用户用访达把整个 `StickyMD` 文件夹移到想放的位置再打开。用访达移动整个文件夹能否解除随机化，仍需实测。
- 本方案因此暂不推进，正文保持原样。日后若要做安装版，按上述意见另出修订版，并重新审查。
- 末节“待 USER 决定”中签名的那一项已由修订版报告记为 ad-hoc，此处不再待决。
