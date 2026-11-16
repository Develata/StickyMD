# Windows 回归 P16-V05 与 Comrak 0.59 实机运行（2026-11-16）

本报告补齐 `feat/macos-host` 在 `ceba057` 之后 66 个提交留给 Windows 的验证项：

- P16-V05：方案 B 改了共享层的帧路径，需要 Windows 桌面回归。涉及 `shell/render.rs`、`frame/surface.rs`、`shell/window_runtime.rs`、`shell/caret_runtime.rs`、`shell/preview_runtime.rs`、`shell/handler.rs`、`host.rs`。
- Comrak 0.59 与编辑器修复在 Windows x64 上的实际运行：
  - [Comrak 升级报告](2026-11-12-comrak-upgrade-evaluation.md)“未验证”中的 Windows 项；
  - `2febab1`、`9088c57`、`74b3d74`。

本报告只记录这台机器上的事实。它不是资格化 suite，不产生 exact-candidate 证据，也没有写模块收据。被测 exe 是 Local Preflight Build。

## 环境

| 项 | 值 |
| --- | --- |
| 源码 | `feat/macos-host` @ `f8f56a38ed0cd909eeaec0fd77d5b5219cdc48f5`，已跟踪文件无改动 |
| 系统 | Windows 11 家庭版 中文版 25H2，10.0.26200 |
| CPU | 12th Gen Intel Core i7-12700H（x86_64） |
| 工具链 | rustc / cargo 1.97.1，host `x86_64-pc-windows-msvc` |
| 显示 | 单显示器 2560×1440，100%，工作区 `y = 0`（任务栏在底部） |
| 被测 exe | `cargo build --locked --release -p stickymd-win`，`target\release\stickymd-win.exe` SHA-256 `f8c6874ab9b88345179810bd2cde46e5abfadaa4b2d31c0fc6ab3ba8fff079ae` |
| 对照 exe | USER 本机的 `v0.1.3` 发布程序，SHA-256 `b6777837bcf18e07d9cb029c5216d6e764b68e7fd4d9d8679f1b685b9f488340` |
| 桌面驱动 | `stickymd-smoke` 的本地 preflight（`#[ignore]` 测试，`STICKYMD_PREFLIGHT_EXE` 指向被测 exe）。harness 要求干净 checkout，主 checkout 有一个与本次无关的未跟踪文件，所以在 `f8f56a3` 的干净 detached worktree 中编译运行。harness 测试二进制 SHA-256 `db44ef8a01515adcaebee1a1313fda7546909e79785b132b0e1a612c33596b78`。每项把 exe 复制进 `%TEMP%\stickymd-c2d-preflight-*` 的新目录后运行，不碰 USER 的便签目录 |
| 运行方式 | 前台工具调用，独占桌面；USER 运行前关闭了自己的 StickyMD，运行期间未碰鼠标键盘 |

## 自动化（主 checkout，PowerShell）

| 命令 | 退出码 | 结果 |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 | — |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | 含 `stickymd-win`；macOS 上的 clippy 不含它 |
| `cargo test --workspace --locked` | 0 | 1767 通过、0 失败、63 忽略 |
| `tools\smoke\phase-00.ps1` | 0 | `StickyMD smoke PASS: phase-00` |
| `tools\smoke\phase-05.ps1` | 0 | `StickyMD smoke PASS: phase-05` |
| `tools\smoke\phase-06.ps1` | 0 | `StickyMD smoke PASS: phase-06` |
| `tools\smoke\phase-07.ps1` | 0 | `StickyMD smoke PASS: phase-07` |
| `tools\smoke\phase-08.ps1` | 0 | `StickyMD smoke PASS: phase-08` |
| `tools\smoke\phase-11-b.ps1` | 0 | `StickyMD smoke PASS: phase-11-b` |
| `tools\smoke\phase-14.ps1` | 0 | `StickyMD smoke PASS: phase-14` |
| `tools\smoke\phase-15.ps1` | 0 | `StickyMD smoke PASS: phase-15` |
| `tools\smoke\phase-16.ps1` | 1 | `NOT_TESTED: the macOS host tests require a macOS host`。按设计，不是回归 |

工作区测试中与本次相关、在 x64 上实际运行并通过的有：

- `autolinked_emails_do_not_overflow_the_preview_worker_stack`：Comrak 0.59 的 512 KB 栈回归，debug 和 release 各跑一次；
- `shell::host_presenter_tests` 11 个、`shell::hidden_frames_tests` 6 个；
- `math_text::tests` 12 个，以及 `preview::source_map`、`preview::semantic_conversion` 的非性能测试。

## Release 基线（x64）

命令为 `cargo test --locked --release -p stickymd-render --lib <过滤词> -- --ignored --nocapture --test-threads=1`，退出码全部为 0，测试内的门槛全部通过。这台机器上的数字与 Apple Silicon 不直接可比，只作记录。

| 过滤词 | 通过 | 代表数字（median / p95） |
| --- | --- | --- |
| `phase5_` | 5 | 1 MiB 预览：总计 93.7 / 109.5 ms，其中 Comrak 解析 24.7 / 28.3 ms |
| `phase6_` | 3 | 1 MiB、500 个公式：总计 88.5 / 96.0 ms |
| `phase11b_` | 3 | 数学按钮 1 MiB / 1000 公式：25.9 / 28.6 ms；上移段落：124.7 / 128.4 ms；分隔符转换：2.8 / 3.1 ms |
| `phase14_` | 1 | 选区几何投影：13.7 / 13.9 µs |

Comrak 0.59 新增的 `memchr-n` / `fearless_simd` 在运行时选 SIMD 分支。上面的测试与基线都在这台 x86_64 CPU 上运行，覆盖了 Apple Silicon 覆盖不到的分支。具体选中了 SSE 还是 AVX 没有单独记录（推断为 AVX2，未核实）。

## 桌面 preflight

每项一次前台调用，输入清理均为 `None`。

| 测试 | 结果 | 通过的检查 |
| --- | --- | --- |
| `qualification::g4::preflight::c2d_search_local_preflight`（G4-03） | ok，24 s | 15/15：撤销/重做、`ctrl-insert`、`shift-insert`（文本、DIB、文件拖放发布资产）、`shift-delete`、Preview 忽略传统编辑、Search 的选择、替换、单步撤销、Esc 返回、视图快捷键保持会话 |
| `qualification::g4::preflight::math_conversion_local_preflight`（G4-04） | ok，3 s | 4/4：`math-convert-durable`、`math-convert-source-refresh`、`math-undo-durable`、`math-undo-one-step` |
| `qualification::g3::export_local_preflight`（G3-02） | ok，5 s | 4/4：导出发布、受管图片复制一次、只改写本地资产引用、工作笔记不变 |
| `qualification::g4::preflight::c2d_tray_local_preflight`（G4-01） | ok，62 s | 20/20：托盘图标与三项菜单、隐藏/显示（含取得焦点、保留窗口与源码）、置顶同步、退出保存、保存失败时取消退出（标题、保留文本、窗口可见、重试）、关闭与 Alt+F4 只隐藏、单进程 |
| `qualification::g4::preflight::c2d_dock_local_preflight`（G4-02） | ok，141 s | 54/54：左、右、上（Pin OFF）与右（Pin ON）的 snap、reveal、leave、collapse、focus guard、reexpand；`covered-sensor-real-hover`；手动收起；Esc 各项；透明度弹窗；Pin 配置；Source 拖动滚动与 Preview/Split 滚动。W15 记的是 53 项；W15 没有留下逐项名单，差的一项未逐名对比，名单以本次日志为准 |
| `qualification::g5::preflight::acceptance_automation_shell_local_preflight`（G5-01） | ok，3 s | 截图 `tool-window.png` |
| `qualification::g5::preflight::acceptance_automation_compact_local_preflight`（G5-02） | ok，7 s | 截图 `source-220x120.png`、`preview-compact-bottom.png`、`split-compact.png`，目视内容正确 |
| `qualification::g5::preflight::acceptance_automation_presentation_local_preflight`（G5-03） | 新包 6 次中 5 次 ok，`v0.1.3` 4 次中 3 次 ok | 失败的两次报同一个错误：`cannot read layered-window alpha: 参数错误。 (os error 87)`，详见下文 |
| `qualification::g5::preflight::acceptance_automation_rendering_local_preflight`（G5-04） | 新包与 `v0.1.3` 都失败 | `window pixels did not stabilize for G5-04/preview-top before 12s`，详见下文 |

### 两项失败为什么不算回归

两项失败都用 `v0.1.3` 对照过：同一个 harness、同一台机器，只换 exe。

- **G5-03 的 alpha 读取错误。** 新包和 `v0.1.3` 交替运行。新包第 1 次失败，之后 5 次全部通过；`v0.1.3` 第 1 次通过、第 2 次失败、后 2 次通过。错误文字完全相同。这是早于本分支就存在的间歇失败。
  - 推断：harness 调 `GetLayeredWindowAttributes` 时，窗口恰好不在可读状态（例如透明度刚切到 100%、layered 属性被去掉）。未核实。
- **G5-04 的截图不稳定。** 新包与 `v0.1.3` 都在第一张截图 `preview-top` 处超时，两边的截图内容相同。
  - 推断：窗口半透明，后面的桌面（含正在刷新的 Claude 窗口）在变，导致截图一直不收敛。未核实。
  - G5-04 后续的截图与检查两边都没有跑到，属于未测。

### 帧内容的逐像素比对（V05）

V05 关心共享帧路径改动后画出来的内容。上面的 preflight 只断言窗口与 UIA 状态，不读像素。G5-03 会截 16 张窗口截图，于是把新包（第 5 次）与 `v0.1.3`（第 5 次）的同名截图逐像素比较，尺寸都是 520×680。

- **不透明的四张**（`opacity-100`、`theme-light`、`theme-dark`、`theme-system`）：
  - 去掉 2 px 窗口边和标题栏（`y < 36`）后，三张逐像素相同；
  - `theme-light` 有 26 个像素不同，全部在 x = 24、y = 232–257，是光标闪烁的相位；
  - 窗口边与标题栏的差异来自透出的桌面背景。
- **其余十二张**（三种视图各三档缩放，透明度 40/70/96）：
  - 新包与 `v0.1.3` 的抽样差异都在 5 万量级像素，最大通道差 39–231；
  - 这些窗口半透明，背景随桌面变化，所以只目视：文字、公式、选区、分栏与缩放的形状两边一致，颜色、方向正确，没有错位或残块。

结论：在这些静止帧上，新包画出的窗口内容与 `v0.1.3` 相同（已核实）。

## 逐项结论

| 检查项 | 结论 | 依据 |
| --- | --- | --- |
| P16-V05 静止帧内容 | 通过 | G5-03 逐像素比对、G5-01/02 截图目视，以及 G4-01/02 的隐藏/显示、收起/展开、滚动全部通过 |
| P16-V05 交互中的撕裂、闪烁、残影 | 未测 | preflight 不在打字、滚动、拖动时连续取帧；需要 USER 在本机目视，或由 USER 决定豁免 |
| P16-V05 高 DPI | 未测 | 本机只有 100% |
| Comrak 0.59 在 x64 上的运行 | 通过 | 工作区测试、email 栈回归（debug + release）、Phase 5/6/11-B/14 release 基线 |
| 编辑器修复（`2febab1`、`9088c57`、`74b3d74`） | 通过（自动化） | `math_text`、`semantic_conversion` 单测，G4-04 桌面数学按钮与撤销 |
| G5-04 渲染截图 | 未测（既有问题） | 新包与 `v0.1.3` 都卡在第一张截图，与本分支无关 |

## 未做

- 模块收据（`qualification modules`）与 exact-candidate 证据：本地 preflight 不产生它们。
- G4-05（身份与第二实例）、G4-06（输入法）、G3 其余长流程：不在本次范围内，未运行。
- 没有修改产品代码，没有发现回归。
- preflight 截图与 G5 证据留在临时 worktree 的 `dist\evidence\g5-artifacts\`，没有提交。
