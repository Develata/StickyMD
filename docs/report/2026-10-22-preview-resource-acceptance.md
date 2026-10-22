# Preview 资源实测与人工验收包（2026-10-22）

## 结论与范围

Source / Preview / Split 的真实 Release 进程各完成五轮资源采样，通过现有内存与空闲
CPU 门。图片压力样例切回 Source 的首次测量在第四轮被物理鼠标定位检查中断；保留
FAILED 记录后，另一次完整五轮测量通过。首次中断的根因未确认，不将复测解释为
桌面交互稳定性已通过资格化。

独立人工验收包已准备好。P05-R03、P06-R03 和图片外部更新的真人观察仍为
NOT TESTED。本轮不修改生产代码、测试框架、性能门或工程骨架；没有创建 Source
Freeze、Promoted Candidate 或正式发布资格化收据，不继承历史发布特例。

## 构建与测量身份

- 工作目录：`E:\gitclone\StickyMD-render-resources`；分支：`codex/preview-resource-optimization`。
- 测量时 clean source：`0f4f465d3a0dac48803b81786b99aee05d8bc9f3`。
- EXE：`target/release/stickymd-win.exe`，8,642,048 bytes，Release / locked 构建。
- EXE SHA-256：`17EB0856F86976C6087F08F92B98E3C83955E463BEDC8CA593A5724E69261BEF`。
- Rust 测量 CLI SHA-256：`9E3E6B4FABE114E91E757C3A19A6C176A3A1B3DE1240BE358FA90B0742D76BD6`。
- Cargo.lock SHA-256：`E72CD60347A2FC7FF01CE1B51C5E94BD341E6CB5ADF1874B0A9E3E9625D39453`。

本报告的后续文档提交不改变上述已测 EXE 身份。原 `E:\gitclone\StickyMD` 仍在
`codex/c2d-c3-coverage`，未用于写入、切分支或构建；另一 session 的改动保持原状。

环境为 Windows 11 10.0.26200、i7-12700H（14 核 / 20 逻辑核心）、RAM
16,962,281,472 bytes、rustc 1.97.1 / LLVM 22.1.6。桌面检查 VALID，单显示器、未锁屏。
窗口实际为 520×680 px，与默认 520×680 DIP 对应的 100% 比例一致；未独立记录 DPI
API 数值，不作为 mixed-DPI 验收。Defender 查询的 AntivirusEnabled / RealTimeProtectionEnabled
均为 false；未修改安全设置。机器仍可能有其他 session 活动，因此这是本机诊断。

## 方法与结果

沿用 [Plan 10](../plan/10_performance_reliability.md#initial-engineering-targets) 和
[Plan 11](../plan/11_testing_and_release.md#resource-module-qualification) 的 Rust 执行器。
每个场景五个全新 copied standalone 进程 / 便签目录，预热 30 s，再采样 60 s 空闲
CPU；没有历史数据复用。CPU 按 20 个逻辑核心归一化，0 是采样精度下的值。
前三组使用相同 20 KiB / 20 公式样例；不是人工包内 300% 长公式的资源结果。

| 场景 | 私有工作集 median / max（MiB） | Private Bytes median / max（MiB） | idle CPU median / max（%） |
| --- | ---: | ---: | ---: |
| Source | 13.50 / 13.61 | 15.70 / 15.91 | 0.001302 / 0.013020 |
| Preview | 11.09 / 11.21 | 12.25 / 12.37 | 0.001302 / 0.001302 |
| Split | 16.91 / 17.00 | 19.47 / 19.60 | 0.002604 / 0.011718 |
| 图片压力 Preview → Source，完整复测 | 17.65 / 17.92 | 21.45 / 21.65 | 0.000000 / 0.001302 |

前三组私有工作集均低于各自 40 / 52 / 64 MiB hard gate，也低于 preferred target；
全部完整采样的空闲 CPU 低于 0.1% hard gate 和 0.05% target。Private Bytes 单独列出，
未混同为驻留内存。原始 JSON 还保留逐次样本、p95 和进程峰值。

最后一组是现有 `source-after-preview-cache-release` 场景：20 KiB、420 个不同 BMP
引用，先在 Preview 预热 30 s，真实点击切到 Source，等待配置确认及 5 s 后测量。
它的内存是额外观察，没有新增内存门。没有切换前同时点的配对样本，也没有把整个
进程峰值归因给缓存，因此不能把 peak 减 current、或与普通 Source 的差值当作
缓存释放量。五次独立启动也不能证明长期连续使用没有增长；具体缓存释放的自动化
覆盖见 [实现报告](2026-10-22-preview-resource-implementation.md)。

## 首次输入中断与复测

首次图片场景只有三个完整样本，第四轮 `transition` 阶段报
`physical input target is not ready`：物理光标 priming 请求 `(1031,187)`，读取到
`(979,208)`。报错字段的 root HWND 为初始化值 0，不能据此断言某个外部窗口抢占。
未确认是用户输入、桌面调度还是其他来源；没有为重跑修改注入容差或程序逻辑。

环境复查 VALID，新的 Source/Preview/Split 点击探测通过，再完整重跑五次。
首次 FAILED 与第二次 PASSED 的独立日志均保留，没有拼接样本或删除失败。
未执行 [100 次独立桌面重复处置](../plan/11_testing_and_release.md#desktop-repetition-jitter-policy)，
也未使用成功率容错把原失败改判通过。若需要关闭该交互稳定性缺口，应在独占桌面
使用现有 window-stress / view-mode reducer 留证，再按现有合同处置；本报告不代替它。

## 人工验收入口

本地目录 `dist/preview-acceptance-0f4f465/`，压缩包同名 `.zip`，11,901,261 bytes。
三个目录使用同一 EXE，各有独立 `note/note.md` 与配置；文件校验和、完整 runtime
dependency notices / font licenses、原始样例备份及两次资源证据均随包提供。
静态检查确认 100 个公式、12 列表格、3 条原始代码行、12 次图片引用、5 个可解码 BMP。
PE 检查确认没有 developer runtime imports；ZIP 内 41 个文件逐一校验匹配。

按包内 `先读我.txt` 操作，在 `结果记录.txt` 留下观察人、时间、显示比例和异常：

1. `01-formulas`：默认 Preview / 300%，查看 F001、F050、F100 并回滚，复制公式；
   切 Source / Preview / Split、主题和缩放，确认没有缓存压力错误占位、错位或丢失。
2. `02-wide-content`：宽表与长代码各自横移到最右，试滚轮、拖动、轨道；核对选区、
   复制、链接位置。代码仍为三行，与 `expected-code.txt` 对照；随后编辑、resize、zoom。
3. `03-images`：用随包变体替换同尺寸图片、改尺寸，临时改名再放回；刷新后确认
   内容、后文位置、缺图占位及恢复正确。每次做完从托盘退出，再开下一目录。

上述操作映射已有 AC-039/040/041，不新增产品要求。P05-R03/P06-R03 保持
NOT TESTED，不能以本报告或资源 JSON 代替真人观察。

## 复现与证据

构建入口：`cargo build -p stickymd-win --release --locked`。
资源入口：`target/debug/stickymd-smoke.exe phase 14 --resources --resource-module=source-preview --json --evidence-file=target/preview-acceptance-0f4f465/source-preview.json`。
图片入口使用 `--resource-module=images`，设置 `STICKYMD_SMOKE_RESOURCE_CASE=source-after-preview-cache-release`，
分别写入独立 evidence 路径，结束后恢复环境变量。这是 selective diagnostic，不是完整 Images 组。

本地原始日志、host/environment JSON、构建及封包检查位于 ignored
`target/preview-acceptance-0f4f465/`；交付包的 `evidence/` 保留测量副本。

| 文件 | SHA-256 |
| --- | --- |
| source-preview.json | 1DE179F4BEFA0BC7DDA38E4DC849B87C221C1743D95DACC75AE6527B602BE709 |
| source-after-preview.json（首次 FAILED） | D0B14713B17270C727DD2A9D6CB35B305DF6C632F8A90BA127CD9063EC7617FD |
| source-after-preview-retry1.json | 7C12BAC1100C64ABCBD97396BCA9F0ABD99479E755F45D9C8F0A83BB73F1501C |
| preview-acceptance-0f4f465.zip | E8B093ECABC7C06F05CC8E12CA73DE18C4CAC8584FE69699010FA4BB665E9211 |

## 剩余边界与规格影响

- 人工视觉、指针、复制及图片更新尚未观察；首次输入定位中断尚未归因。
- 未补齐 Hidden、完整 Zoom / Images / Math 矩阵、mixed-DPI、连续缩放的 OS 内存增长，
  也未做长期字体数据库 heap attribution。完整发布资格化仍需单独执行。
- 约 525 KB 极端单行首次整形仍约 4–5 s，沿用实现报告中的未解决边界。
- 不依据本轮结果改变缓存预算、工程门、权限、运行时职责或当前发布 readiness。

## Resolution（2026-10-22，文档检查）

文档及验收入口同步后，`git diff --check` 通过；Phase 00 的 governance contracts 与
acceptance readiness 均 PASSED（文档治理结果，不是产品发布 readiness）。该次检查收据
位于 `target/preview-acceptance-0f4f465/final-governance.json`，SHA-256 为
`380D9A4EA8D274372A1CD65913AF053C0D69DF6DD6C653A267A67CE4EB1BE896`。
测量进程已全部回收，交付包内 EXE 与已测构建哈希一致。

## Resolution（2026-10-22，USER 人工反馈）

USER 在本次对话先提供 300% 长公式被右侧裁剪、没有横向滚动条的截图，随后明确确认：
“除了我刚刚提到的问题1的长公式没有2里面那种左右的滚轮以外，1-7里面提到的内容都pass”。
反馈对应上述 `preview-acceptance-0f4f465` 验收包；显示 DPI 与具体操作时间未单独提供。

| 对话中的人工步骤 | USER 结果 | 范围 |
| --- | --- | --- |
| 1：300% 下 F001/F050/F100 往返阅读 | BLOCKED | 除超宽公式缺少横向滚动条外通过；被裁掉的右端无法阅读 |
| 2：公式选择复制、视图/主题/缩放切换 | MANUAL PASS | 本次验收包及 USER 已执行的操作 |
| 3：表格/代码横向滚轮、拖动与轨道 | MANUAL PASS | 同上 |
| 4：横移后的选择、复制、链接及三行代码 | MANUAL PASS | 同上 |
| 5：编辑、resize、zoom 后无旧帧或错误命中 | MANUAL PASS | 同上 |
| 6：图片同尺寸与尺寸变化的外部刷新 | MANUAL PASS | 同上 |
| 7：图片缺失与恢复 | MANUAL PASS | 同上 |

这份反馈补齐本次局部改进的人工观察，不回填历史发布矩阵，也不等同于完整桌面资格化。
长公式横向阅读是合并前最后一个人工阻塞项；修复后的新 EXE 仍需 USER 针对该项复验。
原包与资源测量身份保留，新构建不得冒用其 OS 资源数值。
