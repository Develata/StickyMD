# 产品 crate 读取被排除文件：缺口分析与选项

日期：2026-11-22。分支 `feat/package-crate-split`，提交 `51ba15a`（与 `64e4bfb` 只差一处 `docs/report/` 改动，产品代码相同），
工作树干净。只做分析，不改代码与合同。来由：[平台构建输入收窄记录](2026-11-21-platform-build-input-narrowing.md)的复审指出，
产品 crate 读取 `docs/` 等被排除区域属于另一个缺口，早于该分支（`docs/` 的排除来自 Phase 18 的分类器），见 `11`
#platform-artifact-reuse “剩余”一段。

## 结论

- **能发生，已验证。** 产品 crate 可以在编译期用 `include_str!` 等读取被排除的文件。之后只改这个文件时，两个平台的 Platform Build Input
  Fingerprint 都不变，`verify-reuse` 判为可沿用，而 macOS 的 release 二进制变了（见“正向对照”；Windows 的 exe 没有构建，同理推断）。现有检查都不拦这种读取：分类器只看路径，闭合世界检查只覆盖
  打包 crate。
- **当前树中没有，已验证。** 两个平台各自发布二进制的依赖闭包内，产品单元经 rustc 登记的文件都在 `crates/<成员>/src/` 与 `apps/<宿主>/src/` 下，
  都是该平台的输入；没有环境读取。唯一的构建脚本 `apps/stickymd-win/build.rs` 按源码只读自己目录中的文件；它在 Windows 主机上才执行的资源
  编译分支没有运行过（见“未验证”）。
- **推荐方案 B。** 在两个平台的 `self-check` 中加一条闭合检查：按固定发布构建的命令与 profile 做新编译，取发布二进制闭包内各单元的 dep-info，要求每个登记的
  文件都是分类器判定的该平台输入。规则直接调用分类器，排除表与检查不会漂移。本机实测，一次新编译 14–29 秒。需要改合同，须 Develata 批准。

## 读取的通道

| 通道 | 可能读到被排除文件吗 | rustc dep-info 能看见吗 | 现状 |
| --- | --- | --- | --- |
| 模块文件、`#[path]`、`include!`/`include_str!`/`include_bytes!`，清单里 `[lib]`/`[[bin]]` 的 `path` | 能 | 能（原样拼写，含 `..`） | 无检查 |
| `env!`/`option_env!` | 读环境，不读文件 | 能（`# env-dep:`） | 无检查 |
| 构建脚本读的文件 | 能 | 不能 | 只有 `apps/stickymd-win/build.rs` 一个（governance 清单）；它的读取属于“剩余” |
| 过程宏读的文件 | 能（相对调用方的 `CARGO_MANIFEST_DIR`） | 只有展开出 include 宏时能 | 无检查 |
| 产品运行时读文件 | 运行时读取不改变发布字节；仓库文件要进入发布物只能由打包复制 | — | 打包边界（另一份记录） |
| 原生链接输入（SDK、导入库） | 不在仓库中 | 不能 | 属于平台构建环境 |

## 普查（已验证：macOS 27.2 arm64，rustc/cargo 1.97.1）

**方法。** 每次运行都新建私有目录，同时作为 target 与 build 目录，所有 workspace 单元均为 `fresh: false`。依次执行：

```text
CARGO_BUILD_BUILD_DIR=<d>/build cargo check --locked <选择> --lib --bins --target <T> --profile <P> --target-dir <d>/target --message-format=json
```

- 第一组：`<选择>` 为两个平台各自的产品包（`-p` core/render/app/desktop，加 posix/mac 或 win），`<T>` 取两个平台的目标，`<P>` 取 dev 与 release。
- 第二组：使用固定发布构建本身的选择，即 Windows 目标 `--workspace --release`、macOS 目标 `-p stickymd-mac --release`。
- 之后读取 `deps/stickymd_*.d` 的逐文件行，逐条核对：是否为 `git ls-files` 中的精确路径，是否含 `..` 分量，以及按 `platform_inputs.rs`
  的排除表是否为该平台的输入。

**结果。**

| 平台 | 发布二进制闭包中的成员（`cargo tree -e normal,build`） | 登记的本地文件 |
| --- | --- | --- |
| Windows x64 | `stickymd-win`(bin)、core、render、app、desktop | win 43、core 13、render 57、app 76、desktop 50 |
| macOS arm64 | `stickymd-mac`(bin)、core、render、app、desktop、posix | mac 36、posix 9，其余同上 |

- 全部是 Git 索引中的精确路径，没有 `..`，没有绝对路径，都是该平台的输入，也没有 `# env-dep:`。dev 与 release 的文件集相同；`-p` 子集与固定
  选择对闭包内各成员给出相同的文件集。
- **Windows 的 workspace 构建会编译 Windows 排除的文件：** 两个 macOS 成员在 Windows 上编译为桩，dep-info 登记了 `apps/stickymd-mac/src/main.rs`
  与 `crates/stickymd-posix/src/lib.rs`。这两个单元不在 `stickymd-win` 的依赖闭包中（`stickymd-win` 的清单不依赖它们，`cargo tree` 确认），
  不进入 `stickymd-win.exe`。由此可知，产品规则的范围应是发布二进制的闭包，而不是 workspace 构建编译的全部单元。
- **构建脚本：** 它的编译单元（`build_script_build-*.d`）只登记 `apps/stickymd-win/build.rs`，运行输出 `rerun-if-changed=StickyMD.manifest`。
  按源码，它只读同目录的 `StickyMD.manifest`（计入 Windows），并在 Windows 主机上经 winresource 编译资源；本机是 macOS 主机，资源编译为空操作。
- **过程宏**（发布图中，按 Cargo 的 `proc-macro` 单元统计）：两个平台都有 bytemuck_derive、fearless_simd_macros、phf_macros、rust-embed-impl、
  serde_derive、thiserror-impl（1.x 与 2.x 两个版本）；macOS 另有 foreign-types-macros，Windows 另有 windows-implement 与 windows-interface。
  读文件的只有 rust-embed-impl，经 `ratex-katex-fonts` 的 `#[folder = "fonts/"]` 读取 registry 中该 crate 自带的字体，不读仓库。release 下这 22 个
  字体文件以绝对路径登记在该 crate 的 dep-info 中；dev 下登记 0 个，运行时从 registry 读取（dev 构建不发布）。
- **源码中的 include 与 `#[path]` 都在测试代码或 bench 中**，lib 与 bin 的编译不包含它们，读的是本 crate 文件或计入的 `tests/fixtures/`：
  `crates/stickymd-desktop/src/title.rs:301`（`mod tests` 自 :209）、`crates/stickymd-desktop/src/shell/mod.rs:549`（:316 起）与 `#[cfg(test)]`
  的 `shell/title.rs`、`shell_pins_tests.rs`、`startup_tests.rs`、`phase15_host_tests.rs`，`apps/stickymd-win/src/persistence/storage.rs:810`
  （:388 起），`crates/stickymd-render/src/preview/pipeline.rs:540`–`:552`、`:1781`，`crates/stickymd-render/src/preview/copy/tests.rs:468`，
  `crates/stickymd-core/benches/release_baseline.rs:10`。
- **耗时：** 本机 registry 已在缓存中，新编译 Windows 目标 `--workspace --release` 用时 28.7 秒（含两个工具 crate），macOS `-p stickymd-mac --release`
  用时 13.6 秒。

## 正向对照（已验证，只在 scratchpad 的克隆中进行）

1. 在 `51ba15a` 的克隆上提交 C1（`3d4b9fa`）：`crates/stickymd-core/src/lib.rs` 加入
   `pub const CONTROL_TEXT: &str = include_str!("../../../docs/control.md");`，并新建 `docs/control.md`（内容 `one`）。
2. 提交 C2（`34ca182`）：只把 `docs/control.md` 改成 `two`。
3. 两个平台在 C1 与 C2 上的指纹相同：Windows 为 `0dcd7ec6…`，macOS 为 `e77e2b80…`。`release verify-reuse --build-source C1 --release-source C2`
   对两个平台都返回 `ancestor:…`，退出码 0。
4. dep-info 原样登记 `crates/stickymd-core/src/../../../docs/control.md`：rustc 不规范化路径，所以检查要么拒绝 `..`，要么先规范化再分类。
   只看前缀会漏掉它，本次普查最初的统计就漏了，后来补了 `..` 核对。
5. 发布二进制：在 C1 上再提交 D1（`2dd9403`），让 `apps/stickymd-mac/src/main.rs` 的 `main` 以 `std::hint::black_box` 使用该常量；
   提交 D2（`96befc6`），只改 `docs/control.md`。以 `build-app.sh` 中的构建命令 `cargo build -p stickymd-mac --release --locked --target
   aarch64-apple-darwin` 构建（继承环境，未经 `env -i`）：
   - D1 在两个不同的新 target 目录中构建，得到同一个 `stickymd-mac`，`6ee3c75a…`，说明它可复现；
   - D2 得到 `e0a4eec7…`；
   - D1→D2 的 `verify-reuse` 对两个平台仍判为可沿用，退出码 0。

（第 4 步之前还比较过 `stickymd-core` 的 rlib，但同一提交在两个目录中构建出的 rlib 本身就不同，那次比较不能作为证据，已舍弃。）

## 选项

**A. 维持现状。** 缺口继续写在“剩余”里，由代码审查承担。没有成本。风险是审查者须记住排除表，而 `01` 对指纹的定义要求排除表“只列可证明
不进入该平台发布物”的路径，对产品 crate 而言这一条目前没有证明。

**B. 产品发布图的闭合 dep-info 检查（推荐）。**
- **在哪里运行：** 两个平台的 `self-check`，与打包 crate 的检查同处（Windows 经 `tools/smoke/all.ps1 -Ci` 中的 governance，macOS 经 `stickymd-smoke`
  的测试）。各自检查本平台；macOS 主机也能交叉检查 Windows 目标（本次已验证可行），可以作早期反馈，但 Windows 的证据以 Windows 主机为准。
- **编译：** 新建私有目录，命令与固定发布构建相同：Windows 为 `cargo check --workspace --release`，与固定命令一样不加 `--target`；macOS 为
  `-p stickymd-mac --release --target aarch64-apple-darwin`。只用 release profile，因为产品只以 release 发布。
- **范围：** 由 `cargo metadata --filter-platform` 求出发布二进制的 normal 与 build 依赖闭包，闭包内每个 workspace 成员的 lib 或 bin 恰有一个新编译的
  单元，dep-info 的规则行与逐文件行一致。
- **规则：** 每个登记的本地文件须满足三条：在 Git 索引中是普通文件；拼写与索引原样一致（因此拒绝 `..` 与绝对路径，现无）；
  `platform_inputs::is_input(P, path)` 为真。另外不得有 `# env-dep:`（现无；将来用到 `env!("CARGO_PKG_VERSION")` 时，再加由 Cargo 设置的变量的固定
  允许表）。
- **过程宏（可选的绊线）：** 每个平台发布图中的过程宏包名为固定清单（现按包名为 Windows 8 个、macOS 7 个），新增一个就失败，直到审查后更新清单。
  只记包名，所以升级版本不触发。它不证明过程宏的行为，只把“新增一个能读文件的宏”变成必经审查的事件。release 下 rust-embed 一类展开为
  include 宏的读取，应以绝对路径出现在调用方的 dep-info 中，被上面的规则拒绝（推断：只在 registry crate `ratex-katex-fonts` 上观察到绝对路径，
  没有在产品 crate 中试过）。
- **构建脚本单元：** 同一次编译也写出清单中构建脚本单元的 dep-info，用同一规则核对，它自己声明的模块与 include 就从“剩余”移入可证明；
  只有它运行时读的文件仍属剩余。
- **成本：**
  - 代码：把 `governance/package_boundary/compiled.rs` 的 `files_in` 从“一个包的 target、两个 profile”推广为“固定选择、闭包单元、一个 profile”，
    再加规则与坏样本（include `docs/`、`..`、绝对路径、env-dep、被排除成员的文件、新过程宏）。估计 150–250 行，含测试（推断）。
  - 运行时间：每个平台的 `self-check` 多一次新编译，本机 14–29 秒，CI runner 上未测。
  - 合同：`11` #platform-artifact-reuse 的“边界的证明”加一条，“剩余”删去产品 crate 一句。
  - 指纹：不受影响。检查位于不计入的验证工具中，分类器不变，不升算法版本。
- **留在剩余的：** 构建脚本运行时读的文件与过程宏不经 include 宏的读取；`self-check` 与 `build-package` 编译相同这一前提，和打包 crate 的检查一样，
  B 不证明它。

**C. 在 `build-package` 中核对实际构建的 dep-info。** 证明的是产出字节的那一次编译本身，因此消除了跨 job 的前提。代价也明显：
- 产出字节的 job 只运行计入的代码，核对程序与分类表须移入 `stickymd-package`，这是结构改动；
- Windows 的 `--workspace` 构建含两个被排除的桩单元，要按 metadata 过滤；
- `build-app.sh` 在本地使用 `$repo/target`，其中的 dep-info 可能过期，所以只在新的 CI runner 上有意义；
- 违规要到发布时才暴露，而不是在 CI 中。

适合作为以后整体消除跨 job 前提的一部分，现在不推荐。

**D. 收窄排除表（例如计入 `docs/`）。** 构建之后每个只改文档的提交（包括发布记录本身）都会使两个平台不能沿用，违背 #platform-artifact-reuse
的目的，也不能一般地覆盖其他排除区。不推荐。

**否决：对产品源码做文本搜索**（例如禁止出现 `docs/` 字面量）。`concat!`、宏与 `env!` 都能绕过，打包边界正因此放弃了词法分析
（见收窄记录“第五轮复审”一节的判断“词法检查这条路不收敛”与其后的“Resolution（2026-11-22）：闭合世界”）。

## 未验证

- Windows 主机：`build.rs` 的 `#[cfg(windows)]` 资源编译分支、winresource 及其构建依赖读什么；Windows 上 dep-info 的路径写法（`\`）。本报告中
  Windows 目标的全部事实（文件集、过程宏清单）都来自 macOS 主机上的交叉 `cargo check`。
- CI runner 上的耗时；上文秒数只是本机 `cargo check`。
- 原生链接输入（SDK、导入库），以及 release 二进制的 fat LTO 链接是否引入别的本地文件。
- 除 rust-embed-impl 外的过程宏：按用途推断不读文件，没有逐一审源码。

## 相关但不在本题范围内

- macOS 的产品输入指纹（`tools/stickymd-package/src/app_inputs/classify.rs:8`–`:22`，写进 Info.plist，也用作证据锚定）整体排除 `tests/` 与
  `assets/`。产品代码若在非测试代码中读取这两处，它同样漏算；本次普查表明现在没有这样的读取。B 使用的 dep-info 事实同样可以核对这份分类表。
  按两张表推断，它的输入是平台 macOS 输入的子集，所以不会出现指纹不变而 Info.plist 中的值变化的情况。

## 待 Develata 决定

1. 是否采用 B（推荐）。
2. 是否加过程宏包名绊线（推荐加：维护成本只在新增过程宏依赖时产生）。
3. 在哪里实施：推荐在 `feat/package-crate-split` 合并后另开分支。它复用该分支的 `compiled.rs`，但缺口早于该分支，不宜加长它的审查。

## Resolution（2026-11-22）：采用 B

Develata 的回复：“采用 B”。决定 2（过程宏绊线）与决定 3（在哪里实施）没有回答。实施时按以下假设进行，它们都不是 USER 决定：

- **不做过程宏绊线。** 它只是一道防线，证明不了什么。更重要的是，Windows 主机上的过程宏集合无法在本机核实：`winresource` 是按主机 cfg
  选择的构建依赖，macOS 主机上的交叉检查看不到它；`cargo metadata --filter-platform` 又比实际编译多出 `zerocopy-derive`。清单一旦写错，
  Windows 的 self-check 就会失败。过程宏不经 include 宏的读取留在“剩余”。
- **分支与提交位置。** 分支为 `feat/product-input-closure`，在 `feat/package-crate-split` 的 `51ba15a` 上开出，因为该分支尚未合并，而
  `compiled.rs` 与“边界的证明”只在那里。合并前随它 rebase。

### 实现

- **合同 `11` #platform-artifact-reuse：**
  - 新增“产品发布图”一条；
  - “清单与合同”补上闭包外成员清单，并要求两个固定发布命令按原样仍在；
  - “构建脚本”与“剩余”相应收窄；剩余中只留下构建脚本运行时读的文件，以及过程宏不经 include 宏读入的文件。
- **代码：**
  - 新增 `tools/stickymd-smoke/src/governance/product_boundary.rs` 与测试，接在 governance 的 `verify` 中、打包边界检查之后；
  - `package_boundary/compiled.rs` 只做增量改动：放宽可见性，把 `parse` 的规则读取提为 `parse_rules`，新增 `parse_build_script`。`parse` 的行为不变。
- **验收：** P18-P11（macOS 主机）、P18-P12（Windows 主机，NOT TESTED）；coverage matrix 与 Phase 18 任务表同步。

### 运行位置与成本（与分析时的设想不同）

检查与打包边界检查同处于 governance 的 `verify`，所以不只在 self-check 中运行：
- **macOS 或 Windows 主机：** 每次 phase smoke、每次 `modules run`（都以 governance 开头）和每次 `cargo test -p stickymd-smoke`，都会做一次新编译，
  本机 macOS 约 14 秒。
- **Windows PR CI：** 每个 headless matrix job 各运行一次，CI 上的耗时未测。
- **Linux**（CI 的 plan job 与 smoke CLI job）：只比较两个固定发布命令。
- **macOS PR CI：** job 只测选中的 crate，不一定包含 `stickymd-smoke`，所以 macOS 侧的违规最迟在 `release-macos` 的 self-check 暴露。

若成本不可接受，有两种做法：把私有目录换成保留 registry 依赖的固定目录，每次只清掉闭包成员的单元，`fresh: false` 的要求不变；或者只在 self-check
中运行。两者都是之后的选项。
