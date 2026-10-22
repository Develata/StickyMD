# Preview 资源与宽内容实现验证（2026-10-22）

## 结论与身份

本轮按 USER 批准计划先更新 Plans 04/06/08/10 与验收投影，再完成生产实现。
实现和本轮自动化验证已完成；P05-R03、P06-R03 的真实窗口人工观察仍为
NOT TESTED。本报告是维护证据，不改变既有 release 的技术 readiness 或 artifact 身份。

- 独立工作目录：E:\gitclone\StickyMD-render-resources。
- 分支：codex/preview-resource-optimization。
- 基线生产源码：35dea172cbe445bd00f7fdab53f5dca1ff3bad4e。
- 实现提交：bc086ca5983c08350331ccba7eec405d332fc839。
- 基线诊断只增加测量入口、测试计数和测试预算设置；未把它当作历史发布包重建。
- 当前测量在提交前的相同实现上完成；本报告后续提交只整理证据及完成状态。
- 原 E:\gitclone\StickyMD checkout 和另一 session 的工作未用于写入或切换分支；未 push。

环境：Windows 11 10.0.26200，Intel i7-12700H（14 cores / 20 logical processors），
rustc 1.97.1（8bab26f4f，2026-07-14），x86_64-pc-windows-msvc，LLVM 22.1.6。
Release 为 opt-level 3、fat LTO、1 codegen unit、panic abort。机器存在其他 session，
因此耗时是本机诊断，不据此声称普遍加速比例。Cargo.lock 未改变，SHA-256：

    E72CD60347A2FC7FF01CE1B51C5E94BD341E6CB5ADF1874B0A9E3E9625D39453

## 实现与职责边界

| 工作 | 实现归属 | 边界与取舍 |
| --- | --- | --- |
| 公式尺寸与位图分离 | render/math/engine、preview/math_layout、preview/paint | 全文仅持有源码共享引用、尺寸、基线及映射；MathEngine 借出当前绘制位图，布局无法永久持有它 |
| 公式缓存压力 | 既有 ByteLru 与 MathEngine | 缓存可淘汰；未接纳的合法单图顺序临时绘制后释放；源码/单图安全限制独立 |
| 文字初始化 | preview/text_layout | 使用 Buffer::new_empty，避免空串整形；使用同一 Cosmic Text 排版权威 |
| 图片同轮复用 | preview/image_requests、image_resources | 每轮缓存元数据、实际内容哈希、失败目标尺寸；不保留编码文件缓冲，不形成跨刷新文件事实缓存 |
| 宽表与代码 | table_layout、horizontal、paint；app/preview_horizontal | 表格独立测量列宽；代码保留原始行；共享小型块视口和语义滚动目标 |
| 字形与数学布局增长 | glyph_cache、math/engine | 各资源持有者内部回收；字体系统仍按线程独占 |

横向滚动支持横向滚轮、Shift+滚轮、拖动滑块和点击轨道。绘制、裁剪、选区、
命中、链接和 tooltip 使用同一块偏移，复制仍读取完整 projection。列表标记保持原位。
偏移为 Editor Session 的临时状态，按 generation、宽度和有效字号校验；worker 的
不可变请求及结果校验包含偏移，旧帧不能覆盖新的请求。普通横移不重新解析或重排全文。

宽表横移同时更新可见图片的持有范围，释放离屏图片引用；单独滚动代码块不会重读图片。
所有读取仍经现有 PreviewImageSource/Windows adapter；DocumentState 继续独占正文。

新增模块均声明 plan_ref。边界审查确认没有新增依赖、运行时线程、调度器、持久配置
或全局内存管理层。paint 的基础绘制函数移到私有 drawing 模块；缓存与输入分别放入
职责明确的叶子模块。约 520 行的 PreviewPipeline 和约 530 行的 app/preview_runtime
超过文件 review 参考线，已检查：它们继续只协调当前预览生命周期/会话与结果应用，
公式计费、图片复用、块几何和横向输入细节分别由上述模块持有，没有按行数机械拆分。
math/engine 和 paint 超过 soft warning，但仍各自负责单一资源引擎/绘制过程。

## 公式预算评估

重现 fixture 由 resource_tests::dense_formulas 生成：100 或 2,000 个互异长分式，
800×300 viewport、300% content scale。每档先构建顶部，再跳中部、底部、顶部、底部。

基线全文布局持有位图引用，缓存无法淘汰这些引用。因此提高预算只推迟错误出现：

| 总缓存候选 | 基线 100 公式初始位图数 | 基线 100 公式计费字节 | 基线底部错误数（100 / 2,000） |
| --- | ---: | ---: | ---: |
| 8 MiB | 13 | 8,230,550 | 2 / 2 |
| 16 MiB | 26 | 16,587,990 | 2 / 2 |
| 32 MiB | 52 | 33,302,870 | 2 / 2 |
| 64 MiB | 100 | 64,161,110 | 0 / 2 |

新实现四档结果相同：

| 公式数 | 初始位图数 | 初始计费字节 | 四次跳转后计费字节 | 底部错误数 | 文档高度 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 | 2 | 1,260,382 | 3,831,902 | 0 | 15,908 px |
| 2,000 | 2 | 1,260,382 | 3,931,462 | 0 | 316,108 px |

采用 8 MiB 驻留缓存，独立保留单公式 RGBA ≤8 MiB、单边 ≤16,384 px，
源码 ≤64 KiB、每文档 ≤2,000 公式的既有保护。另用仅 1,024 bytes 缓存同时显示六个
长公式，证实一屏需求大于缓存时仍能完成绘制、保持复制/几何，且绘制后缓存可为空。

这不是把整篇文档压进 8 MiB。位图工作集由驻留缓存加当前单公式临时位图组成，
painter 的 outline、font、framebuffer、布局等另外计费。Pixmap::take 还移除了
数学 raster 返回时的完整像素复制。语法或单公式资源错误仍独立传播。

本轮 Release 全套中的 8 MiB 档构建为 26.253 ms（100 个）和 397.595 ms（2,000 个）。
之后五组独立进程复测第一组为 20.316 / 323.737 ms。缓存字节和可读性稳定；
耗时受运行环境影响。旧版底部画错误文本，新版在那里生成真实公式，所以不把
旧版四次滚动约 4–7 ms 与新版约 10–15 ms 当作相同工作的速度回退。

### 独立诊断进程峰值

前后各五次独立进程，交替运行同一个公式预算矩阵；20 ms 采样 Private Bytes，
并读取进程报告的 Peak Working Set。每次均核对只运行一个诊断且退出码为 0。

| 指标（MiB） | 基线 median / max | 新实现 median / max |
| --- | ---: | ---: |
| 采样 Private Bytes 峰值 | 112.910 / 114.469 | 14.004 / 14.965 |
| Peak Working Set | 120.438 / 120.703 | 19.809 / 20.020 |

这是包含 8/16/32/64 MiB 四档的整个测试进程峰值；不是默认桌面应用的常驻内存，
不是 Private Working Set，也不是完整 Source/Preview/Split 资格化。
20 ms 采样可能漏掉短暂 Private Bytes 峰值。没有据此改动全应用内存门槛。

测量 EXE SHA-256：

- baseline: 09AF8EC1B25CBDA331AECEEC7E93B24BA0C637E03B8773934A1BF317E28EF2BF
- after: 113505F794394C57911E7E4295A7468B6FEA2AC8879562B0962A1DD283A2FEC8

## 文字初始化、图片与长期缓存

仅改变文字初始化时，同一 fixture 的几何/像素 fingerprint 三组均保持一致。
20 次布局的 p50 如下；100 KiB 组未测到改善，不声称所有内容都加速。

| fixture | 原 p50 | 只移除空串初始化后的 p50 | fingerprint |
| --- | ---: | ---: | --- |
| 20 KiB 混合重复块 | 1.008 ms | 0.906 ms | 311fbf3d154cf3b8 |
| 100 KiB 混合重复块 | 3.617 ms | 3.639 ms | e1f3fdd2183ba7f8 |
| 20 KiB 互异块 | 9.273 ms | 7.750 ms | 2bdac3074b1d6f22 |

图片 fixture 为 60 个段落、15,550 bytes，30 次切换可见范围：

| 指标 | 基线 | 新实现 |
| --- | ---: | ---: |
| 全文布局次数增量 | 0 | 0 |
| metadata inspect | 1,800 | 30 |
| 完整编码读取 | 75 | 30 |
| 解码缓存计费字节 | 16,512 | 16,512 |
| p50 / p95 | 0.511 / 0.619 ms | 0.959 / 1.157 ms |

读取与重复工作减少是已测事实；此次总体耗时没有改善。保留每轮一次实际内容哈希，
同尺寸修改、删除/重建、离屏尺寸变化和不同容器复用的回归测试均通过。
每个唯一可见图片在新刷新中仍需一个受现有 64 MiB 文件限制的完整编码缓冲。
尚无大编码文件基准证明流式校验值得增加 adapter 接口和二次读取路径，所以本轮
不引入流式方案，也不声称已消除这项临时分配。

固定 300% 字号连续替换 120 批互异 CJK（每批 100 字符），旧 glyph cache 末次
有 11,520 条目、24,145,745 像素 bytes；新策略绘制后超过 8 MiB 估计驻留目标即
整体释放本地 SwashCache，末次为 96 条目、205,722 像素 bytes。后者是一次回收周期
中的快照，不代表长期典型占用。估计计费包括每条 128 bytes 元数据与像素 capacity；
仅条目数改变时扫描。它不限制字体数据库或单帧临时峰值，且字体数据库未做 heap attribution。

512 个不同长公式的 DisplayList 旧实现约 42,995,306 计费 bytes，新实现最终
8,313,822 bytes；条目和字节双重约束为 512 / 8 MiB。估计包括动态 items、字体名、
path commands、源码及元数据。超大布局可临时使用，不因缓存拒收改变显示语义。
强制极小 glyph 预算的 Source/Preview 回归验证像素相同、无需重新布局；
数学布局淘汰后重新测量也保持尺寸和基线。

## 宽内容与性能边界

自动验证覆盖宽表、长代码、Unicode/BiDi/组合符、数学、图片、链接、选区/复制、
列表标记、resize/zoom 重置、滚轮累积、拖动取消及过期 frame。文字绘制使用保守
字体 ink bounds 裁剪离屏 glyph；变体、彩色或 bitmap 字体回退到实际 raster placement，
并与完整绘制逐像素对照。

新增 525,011 bytes 的单行代码压力诊断有一个明确剩余边界：

- 首次构建 4,210.717 ms；25 次横移绘制 p50 16.244 / p95 18.679 ms。
- 追加归因复测首次构建 4,633.921 ms；横移 p95 20.441 ms。
- 同一字体系统、源码、15 px 字号/776 px 宽的组件对照：WordOrGlyph 整形
  5,015.186 ms，None 整形 5,188.263 ms。
- 推断：主要耗时在两种方式共有的整形路径，横向显示不是解决极端单行首次排版的手段。
  这不是原版完整应用的该 fixture 对照，也没有定位到字体库内部某个具体算法。
- 横移只保留少于 160 个可见命中盒，但仍扫描该行 glyph；未新增复杂行内索引、
  分片排版或字体缓存框架。极端单行仍可能延迟首屏，不能称为所有支持范围内输入
  都达到了 2 s 构建目标。若实际使用频繁出现此类内容，应单独评估整形归因与简洁降级，
  不以放宽性能门或本轮绿色测试掩盖该边界。

普通 5,000 行代码垂直滚动 p95 为 0.914 ms。既有标准文档 warm p95：
20 KiB / 100 KiB / 1 MiB 分别为 7.831 / 21.222 / 127.591 ms；
相应数学文档为 4.084 / 9.710 / 136.252 ms，均通过既有 Release 基准门。
标准文档冷构建另记为 337.651 / 472.350 / 760.506 ms；它包含首次字体初始化，
不是上述 warm gate 的结果，不将冷/热数值混写成同一通过结论。

## 验证与复现

本轮实际执行：

| 检查 | 结果 |
| --- | --- |
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets --locked -- -D warnings | PASS |
| cargo test --workspace --locked | 1,206 passed，50 ignored，0 failed |
| render library Release ignored tests，串行 | 17 passed |
| 极端单行附加对照，Release 串行 | 1 passed；延迟边界如上 |
| Windows Release source edit during math build | 1 passed；100 次 DocumentState.edit p95 1.6 µs，max 648.8 µs |

最后一项测量 canonical edit 操作与后台数学构建隔离；不是 IME、键盘到屏幕的端到端延迟。
原公式集成测试先前断言全文生成所有 rasters，现改为遍历可见区域后验证公式完整显示；
没有删除 parser、错误隔离或复制检查。新增全局 AC 序列校验范围同步到 AC-041，
新增 glyph 模块加入 qualification 输入指纹登记，避免旧证据错误复用。

现有 Phase 05/06/07/10 PowerShell 入口仅更新说明；测试选择、执行与计时仍由
Rust smoke CLI 持有。主要复现入口：

    cargo test --workspace --locked
    cargo test -p stickymd-render --release --locked --lib -- --ignored --nocapture --test-threads=1
    cargo test -p stickymd-win --release --locked phase6_source_edit_during_math_build_release_baseline -- --ignored --nocapture --test-threads=1
    cargo run -p stickymd-smoke --locked -- phase 00 --json

本地原始日志保留在本工作目录 ignored target/：
resource-before.txt、text-before.txt、text-after.txt、image-before.txt、
cache-retention-before.txt、release-render-validation.txt、workspace-validation.txt、
clippy-validation.txt、long-line-attribution.txt、formula-process-comparison.json、
formula-process-{before,after}-{1..5}.txt、release-worker-validation.txt。
它们不是 promoted candidate evidence。关键日志 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| workspace-validation.txt | EB9FD014BAB4DC8AEB08F01C5F0C2A38DC28B213D460054E56445CCF388BD31F |
| release-render-validation.txt | BC549F59A8C60F5D7E719B1B409E05ACF5C9C64368919B79B5983517388ED64E |
| long-line-attribution.txt | 554B3B507A9566EB8DC3C90C697CD1EAF3DB8B7E15D16759742A2BFCFC32E8E5 |
| formula-process-comparison.json | 7E39D01B5ED4400DB6CF492DD8CBF7FED7343C344A29E1BF92C8A49BF0E3F068 |

## 未覆盖与后续边界

- 真实 Windows 窗口的宽表滑块/指针、300% 密集公式视觉质量未人工验收，
  P05-R03/P06-R03 保持 NOT TESTED。
- 未重新运行全桌面 Source/Preview/Split、隐藏、idle CPU 或 mixed-DPI 的完整资格化。
- 字体数据库总内存、超大编码图片的流式策略、极端单行整形仍分别保留上述测量边界。
- 本次改动不修复或评估另一 session 的 display-math standalone-equals 问题。
- 后续合入主分支前需与另一 session 的变更按共同契约复核；本分支不会自动继承任何
  历史版本的 USER 发布特例。

## Final verification（2026-10-22）

完成本报告和 coverage/task 状态同步后，Phase 00 governance contracts 与 acceptance
readiness 均 PASSED（0.762 s）；该 readiness 仅指文档治理检查，不是发布资格化。
结果保留在 ignored target/governance-validation.txt。实现提交之后没有修改生产代码。
