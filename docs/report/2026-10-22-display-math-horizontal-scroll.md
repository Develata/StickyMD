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

## Resolution（2026-10-22，新验收程序）

修复提交 `613e82fe58cfd379a3a1c802b919512620a03e29` 在 clean worktree 以
`cargo build -p stickymd-win --release --locked` 构建成功。独立目录为
`dist/formula-scroll-613e82f/`，ZIP 为 `dist/formula-scroll-613e82f.zip`。
默认 Preview / 300% / 520×720 DIP；沿用经原包哈希验证的 100 个长公式，增加 1 个短公式。
没有覆盖旧验收目录、用户修改过的便签或其他 session 的 worktree。

| 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| StickyMD.exe | 8,642,048 | C2414C2D274D22BCBC3F76E5379412D4504045076200134623B2976391FBC97B |
| formula-scroll-613e82f.zip | 4,063,457 | C8563E43776AA79A35FBC41DC81741F5721F3610B7CCB340B9A0B2897B1AD72B |

PE 依赖检查无 developer runtime imports；12 项文件校验和及 ZIP 内全部 13 个文件逐项
校验通过。`Cargo.lock` 与旧验收包一致，复用对应 runtime notices / 字体许可；新包提供
独立 `build-identity.json`、原始样例备份、复验说明和待填写记录。原始日志及构建/封包脚本
留在 `target/display-math-scroll-fix/`；本段之后的纯文档提交不改变 EXE 的 source 身份。

只需 USER 重点复验最后的阻塞项：

1. F001/F050/F100 的滑块、轨道及 Shift+滚轮能到左右首尾，底色固定。
2. 横移后选区对应可见公式，复制仍为完整公式源码。
3. 短公式居中无滑块；Split、窗口宽度、100%/300% 与主题变化后仍可正确横移。

新 EXE 的人工复验为 `NOT TESTED`，尚未合并或 push。旧包其余 1–7 的 USER 通过记录
继续保留；只在本次补验通过后按既有授权处理合并。没有重跑新 EXE 的 OS 资源矩阵，
上文计数器/局部 Release 诊断与旧 EXE 的 OS 实测严格区分。

## Resolution（2026-10-22，USER 补验通过）

USER 在收到上述新程序及三项复验清单后明确回复：“公式滚动也 pass，之前出现的是什么bug？什么原因？”
本次补验绑定 `formula-scroll-613e82f/StickyMD.exe`，source 为 `613e82f`，EXE SHA-256
为 `C2414C2D274D22BCBC3F76E5379412D4504045076200134623B2976391FBC97B`；记录时再次核对
该 EXE 哈希一致。观察人是 USER，日期为 2026-10-22；具体操作时间和 Windows 显示比例未单独提供。

| 本次局部人工补验 | 状态 | 反馈与步骤来源 |
| --- | --- | --- |
| F001/F050/F100 左右首尾、滑块/轨道/Shift+滚轮 | MANUAL PASS | USER 确认上一条三项复验清单通过 |
| 横移后选择、完整公式复制、背景固定 | MANUAL PASS | 同上 |
| 短公式居中、Split/窗口宽度/缩放/主题切换 | MANUAL PASS | 同上 |

原包其余 1–7 的通过反馈与本次补验共同关闭本次局部改进的人工阻塞项；不要求重复已通过项目。
该矩阵是本地维护验收记录，不补造环境数据，也不升级历史发布、完整桌面或 exact-candidate 资格化。

合并前分支核对发现，本任务基点 `35dea17` 已包含另一 session 尚未合入 `origin/main`
(`5a8f0ca`) 的 30 个提交；另一 session 的当前分支另有 7 个后续提交。
直接合入 main 会附带那 30 个提交，因此在完成验收记录后，先向 USER 明确合并顺序。
未改写另一 session 的分支、工作区或未跟踪文件，未 push。
