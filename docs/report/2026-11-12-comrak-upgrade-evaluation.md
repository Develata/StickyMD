# 评估：Comrak 从 0.54 升级（2026-11-12）

## 结论

建议分两步升级。

**第一步：现在把 Comrak 从 `=0.54.0` 升到 `=0.59.0`，代码不用改。**

最主要的理由是一个已经在本机复现的崩溃：

- **0.54 的崩溃。** 在开启 autolink 的方言下（StickyMD 的预览就是），一段里有大约 3000 个 email（约 21 KB），Comrak 就会在预览 worker 的 512 KB 栈上溢出，整个进程以 SIGABRT 中止。上游公告是 [GHSA-xg9p-p4jc-c46g](https://github.com/kivikakk/comrak/security/advisories/GHSA-xg9p-p4jc-c46g)，等级 high，0.55.0 修复。`v0.1.3` 与当前分支都受影响，Windows 也一样。
- **0.59 上的验证。** 这个崩溃消失。本机工作区测试全部通过，`cargo-deny` 通过，Windows 目标的交叉 `cargo check` 也通过。另外它修好了多行行内节点的结束列（[kivikakk/comrak#855](https://github.com/kivikakk/comrak/pull/855)），还修了若干病态输入的性能问题。

**第二步：等 Comrak 1.0.0 正式版发布后再升 1.0，并删掉两处绕行代码。**

- **引用定义导致的位置上移。** 只有 `1.0.0-rc.1` 修了；StickyMD 当前是保守跳过，见 [RISK 报告](RISK-comrak-054-inline-positions.md)。
- **表格 `\|` 的列号偏移。** 1.0 自己修正后，StickyMD 再修正一次就会错。

`1.0.0-rc.1` 在本机同样无需改代码就能编译，有 9 个测试失败，原因都已查清（见下文）。但它是候选版，正式版之前还可能改动，所以不建议现在依赖它。

无论升到哪个版本，产品代码和 `Cargo.lock` 都会变，Windows 与 macOS 的资格化都要重跑。这两个平台本来就因 `5e57b5a` 和 `2febab1` 要重跑，现在升级不会多出一轮。

## 背景

- `2febab1` 修改数学 `$` 按钮时，六轮 codex 审查暴露出 Comrak 0.54 的两个位置缺陷，USER 选择了保守跳过（RISK 报告方案 A）。根本解是升级 Comrak，于是提出本评估。
- USER 2026-11-12 要求在本会话做评估：先出报告，批准后再改。
- 依赖规则：
  - `deny.toml` 只允许 crates.io，许可证有白名单；
  - 工具链固定为 `rust-toolchain.toml` 的 1.97.1；
  - `Cargo.lock` 是 Windows 模块账本的全局输入（`tools/stickymd-smoke/src/qualification/module_ledger/fingerprint.rs:224–226`）。

## 上游各版本（GitHub releases 与 crates.io，2026-11-12）

| 版本 | 日期 | 与 StickyMD 相关的变化 |
| --- | --- | --- |
| 0.55.0 | 09-06 | 修复 autolink 的两个 DoS（GHSA-xg9p-p4jc-c46g，high）：email 递归会栈溢出，结尾的 `)` 会引发平方级开销 |
| 0.56.0 | 10-06 | MSRV 升到 1.89；修复多行行内节点的位置（#855）、表格行在缩进不同时的位置、CRLF 下表格前段落的位置；修复若干病态输入（强调、宽松 autolink、脚注）；autolink 有三处改为与 cmark-gfm 一致；删除 `tagfilter` 扩展（StickyMD 未使用） |
| 0.57–0.59 | 10-21 至 10-26 | 只新增 Obsidian/wikilink 扩展、取消回调等 API，StickyMD 不用；修复 HTML 表格渲染的平方级开销（StickyMD 不用 HTML 渲染） |
| 1.0.0-rc.1 | 11-12 | 修复段首引用定义删除后的起始行与偏移（`12e800670`）；表格 `\|` 之后的列号改为按原文报告；块内容中的 `\r\n`、`\r` 统一为 `\n`；未闭合代码围栏、多行引用块等的位置有较大改动；新增 sourcepos fuzzer 并修了一批位置问题 |

所有版本的许可证都是 BSD-2-Clause。0.59 新引入的依赖与许可证如下，都在 `deny.toml` 的白名单内：

| 依赖 | 许可证 |
| --- | --- |
| `fearless_simd` 1.2.0 及其 macros | Apache-2.0 OR MIT |
| `memchr-n` 0.1.10 | MIT OR Apache-2.0 |
| `phf` 0.14 | MIT |

0.59 同时移除了 `jetscii`。

## 本机实测（Apple M6，macOS 27.2.1，rustc 1.97.1，基于 `54f0ba6`）

所有试验都在工作区里临时修改 `Cargo.toml` 与 `Cargo.lock`，结束后已恢复，不留改动。

### 崩溃复现

探针：在一个 512 KB 栈的线程上（与 `crates/stickymd-app/src/execution/preview_worker.rs:68–71` 的预览 worker 相同）用 `PreviewParser` 解析 `"a@b.co ".repeat(N)`。

| N（字节数） | 0.54 | 0.59 | 1.0.0-rc.1 |
| --- | --- | --- | --- |
| 500（3.5 KB） | 正常，1500 个节点 | — | — |
| 3000（21 KB） | `thread 'stickymd-preview' has overflowed its stack`，`fatal runtime error: stack overflow, aborting`，SIGABRT | 正常，9000 个节点 | — |
| 30000（210 KB） | — | 正常，90000 个节点 | 正常，90000 个节点 |

进程中止前，最后一次自动保存之后的编辑会丢失。从记事本、网页粘贴大量 email 就能触发。

### 编译与测试

| 版本 | 编译 | 工作区测试（`cargo test --workspace --exclude stickymd-win --locked`） | 其它 |
| --- | --- | --- | --- |
| 0.54（现状） | 通过 | 1599 通过、0 失败 | `cargo deny check` 通过 |
| 0.59.0 | 通过，代码无改动 | 1599 通过、0 失败 | `cargo deny check` 通过，重复版本警告与 0.54 相同；`cargo check --target x86_64-pc-windows-msvc --workspace --all-targets` 通过 |
| 1.0.0-rc.1 | 通过，代码无改动 | 1590 通过、9 失败（见下） | — |

rc.1 的 9 个失败分两类：

- **3 个是引用定义绕行的测试。**
  - 涉及：`flow/editor` 的 `phase11b_math_button_leaves_paragraphs_shifted_by_reference_definitions`、`math_text` 的 `phase11b_equals_lines_reject_positions_that_disagree_with_the_formula`、`semantic_conversion` 的 `phase11b_conversion_skips_blocks_shifted_by_reference_definitions`。
  - rc.1 下位置已经正确，按钮会处理真正的公式、不动标题，例如 `[x]: u "$a\n=\nb$"\n12345678$a\n=\nb$` 变成 `…12345678$a=b$`。这是正确行为，这几个测试断言的是 0.54 下的“拒绝”。
- **6 个是表格 `\|` 测试**（`crates/stickymd-render/tests/table_math_pipes.rs`）。
  - rc.1 已按原文报告列号，StickyMD 的 `TableCellSourceMap`（`crates/stickymd-render/src/preview/source_map.rs`）又修正一次，所以偏了。
  - 试验中停用这次修正后，该文件 7 个测试全部通过。

### 性能（Release，同一时段依次运行）

| 基线 | 0.54 | 0.59 | rc.1 |
| --- | --- | --- | --- |
| Phase 5 预览 1 MiB（median / p95） | 22.4 / 25.0 ms | 23.3 / 25.1 ms | 23.9 / 25.4 ms |
| Phase 11-B 分隔符转换 1 MiB（p95） | 1.8 ms | 0.8 ms | 0.6 ms |
| Phase 11-B 按钮 1 MiB（p95） | 15.1 ms | 11.3 ms | 断言失败（见下） |
| Phase 6 公式、Phase 14 选区几何 | 通过 | 通过 | 通过 |

- 预览管线以排版为主，三个版本持平。
- rc.1 上 Phase 11-B 按钮基准的断言失败，原因同上：引用定义段里的 `\(q\)` 现在会被正确转换，而基准断言的是 0.54 下保持原样。
- 本机负载不同，绝对值会有波动：`54f0ba6` 记录的一次运行中，0.54 的按钮 p95 为 26.7 ms。

## 0.59 对 StickyMD 的行为影响

- **预览中哪些文字成为链接，可能改变。** 0.56 的 autolink 修正与 cmark-gfm 对齐，可能改变预览里个别文字是否成为链接。现有测试未见差异，属于预览可见行为的小变化。
- **位置修正。** #855、表格行、CRLF 下段落的位置变得更准确，源码映射只会更对。依赖这些位置的绕行代码仍然有效：
  - `math_text.rs` 的 `formula_end` 从结束行上找 closer，本来就与正确位置一致；
  - 引用定义的检测在 0.59 仍然需要；
  - `TableCellSourceMap` 在 0.59 仍然需要（0.59 下表格测试全部通过）。
- **病态输入改为线性或有界。** 都是安全方向的改动。

## 可选路径

| 方案 | 做法 | 好处 | 代价 |
| --- | --- | --- | --- |
| A（建议） | 现在升到 `=0.59.0`；同步代码注释、plan 07 和 RISK 报告中“Comrak 0.54”的说法；1.0 正式版发布后再做第二步 | 立即修掉已复现的崩溃与 #855；零代码改动，风险最小 | 引用定义与表格 `\|` 的绕行代码要保留到 1.0 |
| B | 现在升到 `=1.0.0-rc.1`，删除引用定义检测与 `TableCellSourceMap`，更新约 9 个测试 | 一步到位，删掉绕行代码 | 依赖候选版，正式版前可能再变；`\r\n` 归一化等改动面更大，需要更多核实 |
| C | 暂不升级 | 无改动 | 已复现的崩溃保留；与安全公告和“数据安全优先”的宪法冲突 |

## 采用 A 的具体改动

1. `Cargo.toml`：`comrak = { version = "=0.59.0", default-features = false }`；`Cargo.lock` 用 `cargo update -p comrak` 更新。
2. 改正文字：
   - plan 07 中“Comrak 0.54 删除段首的链接引用定义……”一条改为“Comrak 1.0 之前的版本”，并写明升级到 1.0 后重新评估；
   - 代码注释中“Comrak 0.54”的说法同样改为“1.0 之前”；
   - #855 已在 0.56 修好，`formula_end` 的说明改为“作为保守核对保留”。
3. 新增回归测试：用 512 KB 栈的线程解析 `"a@b.co ".repeat(3000)`，要求不崩溃。
4. 在本报告与 RISK 报告追加 Resolution。
5. 验证：
   - fmt、clippy、工作区测试、`cargo deny check`；
   - Windows 目标的 `cargo check`；
   - Phase 00；
   - Phase 5/6/11-B/14 的 Release 基线；
   - codex-reviewer 审查。
6. 资格化：
   - Windows 模块收据与桌面回归，在 Windows 机器上运行；
   - macOS 的自动化行在 VM 中重跑，人工收据在新版本上做。
   这些与 `2febab1` 要求的重跑合并进行。

## 未验证

- 0.59 在 Windows 机器上的实际运行与桌面回归。本机只做了交叉 `cargo check`。
- 0.56 的 autolink 修正在真实笔记中引起的预览差异，只有现有测试覆盖。
- rc.1 的 `\r\n` 归一化，以及未闭合代码围栏、多行引用块位置的改动，对 Source/Preview 映射（选区、复制、滚动同步）的影响。现有测试没有失败，但没有专门核查。

## 待 USER 决定

1. 采用 A、B、C 中的哪一个（建议 A）。
2. 采用 A 时，是否同意第二步等 Comrak 1.0.0 正式版发布后另行评估。

## Resolution：USER 选择方案 A（2026-11-12）

USER 选择“A 先升 0.59”。USER 另外说明：StickyMD 自己的 1.0.0 正式版，在 USER 主动提出之前不考虑。这一点不影响上游依赖；第二步仍按原计划，在 Comrak 1.0 正式版发布后另行评估。

### 改动

- `Cargo.toml`：`comrak = { version = "=0.59.0", default-features = false }`；`Cargo.lock` 用 `cargo update -p comrak` 更新，依赖变化与上文一致。产品代码不变。
- 版本说法按 0.59 的事实更正：
  - 涉及：plan 07、Phase 11-B 验收，以及 `preview/{model,source_map,semantic_conversion}.rs`、`math_text.rs` 及其测试、`flow/editor.rs` 中的注释。
  - 内容：#855 在 0.56 已修，结束行上找 closer 的逻辑作为保守核对保留；引用定义错位与表格 `\|` 列号在 0.59 仍存在，上游 1.0.0-rc.1 起修复。
- 新增回归测试 `autolinked_emails_do_not_overflow_the_preview_worker_stack`（`crates/stickymd-render/src/preview/parser.rs`）。在 512 KB 栈上解析 3000 个 email：0.54 上 SIGABRT，0.59 上通过（本机两种版本都实际运行过）。

### 验证（本机，提交前的工作树）

- `cargo fmt --all --check`、`cargo clippy --workspace --exclude stickymd-win --all-targets --locked -- -D warnings` 通过。
- `cargo test --workspace --exclude stickymd-win --locked`：1600 个通过、0 失败。
- `cargo deny check`：advisories、bans、licenses、sources 均 ok。
- `cargo check --locked --target x86_64-pc-windows-msvc --workspace --all-targets` 通过。
- `stickymd-smoke phase-entry 00` PASS。
- Release 基线（`cargo test --locked --release -p stickymd-render --lib <过滤词> -- --ignored`）：Phase 5、6、11-B、14 全部通过。

| 基线 | 结果 |
| --- | --- |
| Phase 5 预览 1 MiB（median / p95） | 21.3 / 24.6 ms |
| Phase 11-B 分隔符转换 p95 | 0.95 ms |
| Phase 11-B 按钮 p95 | 9.9 ms |
| 23,717 个引用定义段落 | 按钮中位数 31.3 ms，单次解析 5.9 ms |

### 审查（codex-reviewer，effort high，只读）

结论：PASS，无 BLOCKER。核实了以下几点：

- 两处绕行代码在 0.59 上仍然必要且正确：引用定义仍只删内容、不调整起始行；表格仍报告缩短后的列号。
- email 回归测试确实命中旧版的崩溃路径。
- 新增依赖都来自 crates.io，许可证都在白名单内，没有新增重复依赖族。

指出的待验证项与已知行为变化，已写入下文。

### 已知的预览行为变化（0.56 起与 cmark-gfm 对齐，符合“由 Comrak 决定”的契约）

- `http://localhost/x`、`HTTP://www.example.com/` 由普通文本变为自动链接。
  - 由此，若公式紧贴在这类 URL 之后（源码推断，未单独运行），例如 `HTTP://example.com/\(x\)`，这段会整段成为链接，`$` 按钮不再转换其中的公式。
- 链接文字里带裸 URL、外面再套一层链接时，0.54 会让 URL 吞掉外层语法，0.59 改为正确的外层链接（上游 kivikakk/comrak#843）。
- 多行行内代码、引用块内表格、CRLF 段落、列表中的缩进代码围栏，位置或字面内容得到修正。

### 未验证

- **Windows x64 上的实际运行。** 0.59 新增的 `memchr-n` / `fearless_simd` 有运行时 SIMD 分派，Apple Silicon 的运行和交叉 `cargo check` 都覆盖不到 SSE/AVX 路径。需要在 Windows 机器上运行新增回归、表格 source-map 与数学按钮测试，并记录 CPU 与构建身份。这与 `2febab1` 要求的 Windows 回归合并进行。
- 真实笔记中 autolink 变化带来的预览差异，只有现有测试覆盖。

## 补充：复审后的更正（2026-11-12）

- **plan 06 漏改。** codex 复审指出，`docs/plan/06_markdown_math_rendering.md` 要求“升级 Comrak 时方言选项需重新核验并更新本章”，升级时没有做。现已补上：记下当前版本 0.59.0；方言选项不变、其它扩展保持关闭；并列出随版本改变、由 Comrak 决定的解析结果（自动链接的识别，以及源码位置）。
- **授权依据。** 预览中自动链接的变化，本报告上文已列为已知行为变化，USER 在此基础上选择了方案 A。
- **Windows x64 实机运行。** 仍待在 Windows 机器上验证。
