# 超宽独立公式横向滚动修复（2026-10-22）

## 背景与复现

USER 在 `preview-acceptance-0f4f465/01-formulas` 的 300% 长公式样例中发现：
窄窗口内公式右端被裁掉，没有 `02-wide-content` 中表格与代码的横向滚动条。
USER 随后确认原人工步骤 1–7 除此问题外全部通过；逐项反馈和旧 EXE 身份保留在
[原验收报告的 Resolution](2026-10-22-preview-resource-acceptance.md)。

## 根因与尝试

`preview/layout.rs` 仅为代码/HTML literal 注册共享横向布局；表格在自己的布局路径
注册，独立公式遗漏。公式尺寸及位图本身完整，窗口外部分无法通过交互访问；这不是
8 MiB 缓存耗尽或数学解析错误。原压力测试验证了位图回收、底部绘制和源码保留，
没有验证窄视口内公式右端可达性。

新增回归先在原实现失败，错误为 `overwide display math needs a scrollbar`。
修复让独立公式复用现有块视口和语义滚动目标；同时使 `MathBackground` 与代码背景
一样固定，防止公式横移后底色移动。短公式仍沿用原有居中规则。

第一次修复后测试另有一处夹具断言错误：要求溢出距离大于整个视口，但该公式在
520 px / 100% 下只溢出部分视口。将条件修正为实际合同要求的正溢出，产品逻辑不变。
随后横向测试 5 项通过、1 项 Release 专用诊断按设计 ignored。

## 验证范围与架构

- 新回归覆盖 320/520 px、100%/300%，公式首尾、原子选择与完整复制、滚动条两端、
  块外像素不变、底色固定、回滚像素还原，以及短公式居中且无滚动条。
- 计数器验证横移不增加 Markdown parse、全文 layout、数学 parse/layout 或 rasterization；
  数学缓存计费字节不增长。既有表格、代码及列表标记横移测试继续通过。
- 复用 `HorizontalLayout`、Editor Session 偏移、worker 帧校验和统一裁剪/命中转换；
  不新增公开接口、长期状态、运行时依赖或公式专属 Shell 分支。
- 采用块内横移；没有通过缩小字体或自行换行 TeX 来隐藏溢出，也不改变缓存预算。
  Plan 06 与功能、AC-039/040 投影先明确超宽公式的可读行为，骨架与权威边界不变。

## 待完成项

全量检查、新 Release EXE 与包身份随后追加。新程序的真人复验保持 `NOT TESTED`，
长公式滚动仍是合并前最后一个人工阻塞项。原 1–7 其余通过结论保留在旧包身份下；
不冒用旧 EXE 的资源测量结果，也不将这次局部验收描述成完整发布资格化。

## Resolution（2026-10-22，自动化验证）

- `cargo test --workspace --locked`：1,208 passed，50 ignored，无失败；ignored 项保留
  原有 Release/桌面专项入口，不视作通过。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`
  与 `git diff --check` 通过；Phase 00 governance contracts / acceptance readiness 通过。
- `cargo test -p stickymd-render --lib --release --locked phase6_ -- --ignored --nocapture --test-threads=1`：
  3 项通过，覆盖公式冷/热路径、文档基线和 8/16/32/64 MiB 缓存诊断。
- 300% / 800×300 的 100 及 2,000 个长公式诊断均无底部错误占位。默认 8 MiB 下分别
  首屏生成 2 张位图、初始计费 1,260,382 bytes，四次滚动后计费 3,831,902 / 3,931,462 bytes；
  build 为 16.047 / 267.524 ms，四次滚动合计 9.013 / 11.654 ms。只作本机诊断，
  不据此声称统计加速或 OS 内存上限。
- 20 KiB / 100 KiB / 1 MiB 的数学文档 warm total p95 为 3.593 / 12.691 / 138.242 ms，
  通过既有门；相应 cold 数值为 331.698 / 265.128 / 440.127 ms，未与 warm 门混同。

日志和治理 JSON 保存在 ignored `target/display-math-scroll-fix/`。生产修改仅涉及
`preview/layout.rs` 的块资格判定与 `preview/paint.rs` 的背景绘制；不需要 Shell 新逻辑。
