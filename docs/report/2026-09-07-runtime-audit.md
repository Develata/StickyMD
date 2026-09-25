# StickyMD 运行时审查与定向优化（2026-09-07）

本轮已复现并修复五组问题：公式位图预算逃逸、Undo 合并边界、纯文字预览重复布局、长代码块全文字形绘制、缩放与释放时遗留文字位图。Release 局部滚动测量有明显改善；没有证据支持重写主架构或整批迁移 PowerShell。

这是有时间属性的维护证据，不是工程合同、完整安全审计或新版本发布资格证明。

## 基线、范围与授权

- 开始时核实：`main`，HEAD 与本地 `origin/main` 均为 `37fa6e0b6da3ae83c16bb29a42b605ad078ac3c2`，工作树干净。未 fetch，未查询当前远端服务器状态。
- 最近三条提交为 `37fa6e0`、`9642199`、`64690ab`，与交接一致。
- `v0.1.0` exact source 仍是 `64690ab8f86f63f3cbfeabbb0961276978c8f26d`。本轮源码修改与测试不能归属于该已发布 artifact。
- 任务属于既有合同下的普通修复/性能维护，附带验收工具与文档投影维护。没有新增产品能力、依赖、线程、持久格式或网络能力。
- 未 commit、push、创建 tag、dispatch workflow 或发布资产。
- 开始时未发现 `.codegraph/`，按要求使用 `rg` 与源码阅读。后续状态检查发现新出现的 `.codegraph/`，保留其内容，并使用现有索引做只读调用链复核；未执行初始化、重建或清理。其生成来源未核实，不计入本轮交付文件。

权威依据：根目录及相关文档目录 AGENTS，工程宪法与术语，03/04 authority 和状态模型、05 持久化、06 Markdown/math/preview、07 Undo/IME、08 图片、09 Windows 适配、10 性能/内存、11 验证工具与 module-success ledger；结合现有 features、AC-009、Phase 02/05/06/10 及 coverage matrix。公共诊断事实以当前 CONTRIBUTING 的实现说明为证据，同时保留它与 plan 的差异。

## 已核实发现及修复

### P1：公式缓存淘汰后，当前布局仍持有位图，实际像素绕过预算

根因在 `math/engine.rs` 与 `math/cache.rs`：LRU 只扣除 map 中的字节数，`LaidOutDocument` 的 `Arc<MathRaster>` 仍使被淘汰像素存活。缓存账本低于 8 MiB 不能证明实际保留像素低于 8 MiB。

复现将同一预算机制缩为 4,096 字节，连续渲染 40 个不同公式并保留全部返回值：修复前缓存报告 3,292 字节，存活像素却有 57,120 字节，回归断言失败。

修复：

- 在像素分配前复用 painter 的尺寸计算，预检像素、source 和元数据计费。
- 只淘汰 `Arc::strong_count == 1`、没有布局租用的 raster；已有命中继续共享同一分配。
- 没有可释放空间时返回明确预算错误，走现有可复制原公式和 tooltip 的错误投影；不追加 raster 分配。
- 沿用现有“先释放旧布局，再构建新投影”的生命周期。没有增加缓存或放宽 8 MiB 阈值。

修复后同一缩小预算测试保留 3,528 字节像素，账本 3,927 字节；释放租用后可再次接受公式。另有真实 8 MiB、100 个不同长公式、300% 缩放的布局压力测试，验证像素总量、错误 tooltip、完整源文本复制，以及释放后新文档正常绘制。

边界：这里约束 resident raster 与其缓存计费，不能代替进程 Private Working Set/Private Bytes 测量；字体、RaTeX layout、临时 painter 分配也占内存。高密度公式超过预算时会显示原公式错误投影，未新增 viewport-only 数学调度方案。

### P2：Undo 在 750 ms 和删除换行处错误合并

`undo.rs::try_absorb` 原来仅拒绝间隔 `> 750 ms`，与合同 `< 750 ms` 不符；换行防合并只检查插入，Backspace/Delete 的被删除换行可与两侧文字合为一步。

修复前：750 ms 测试得到 1 条而非 2 条记录；删除文字/换行/文字得到 1 条而非 3 条记录。修复后拒绝 `>= 750 ms`，并检查新旧 delta 的 deleted 字段是否含换行。749/750/751 ms 及两个删除方向的回归均通过，既有 Unicode 和撤销/重做 property tests 通过。

没有修改 DocumentState、Undo 容量、图片 AssetEffect 顺序或 IME commit 的独立事务规则。

### P2：生产图片 adapter 使纯文字滚动重复全文布局

真实调用链：`app/preview_runtime.rs::ensure_preview_worker` → `PreviewWorker::start_with_image_base` → `LocalImageSource` → `paint_with_image_source`。即使文档没有图片，生产环境仍提供 adapter。

原判断把 adapter 存在等同于文档需要图片 band 更新；滚动跨出 band 就调用全文 `layout_document`。既有大量滚动测试传入 `None`，因此没有覆盖该生产条件。20 KiB 回归滚动三次后布局计数从 1 增至 4。

修复只从新 RenderTree 计算一次“是否存在本地图片节点”，作为私有派生事实；仅实际本地图片需要 band 更新。检查覆盖表格 cell 和已展开的容器；远程图片、行内代码及 fenced code 的图片字样不触发。

新的滚动热路径不遍历全文查图片。无本地图片时新增布局计数为零；已有图片 lazy decode、表格图片、图片 source 切换、缩放及 overscroll 测试继续通过。

### P2：长代码块只裁掉最终像素，仍绘制并缓存全文字形

`paint_document` 已做 block culling，但对可见长 block 调用无限高度的 `Buffer::draw`，会遍历该 block 的全部 layout runs。最终像素裁剪发生得太晚。

复现：首屏只显示重复的 `visible`，200 行以后放置 400 个不同中文字符。修复前首屏已缓存 406 个字形，修复后为 6 个。

修复复用现有视觉行 locator：二分确定可见行，保留两边各一行容纳跨行框的字形墨迹；直接引用 Cosmic Text 已整形的行与 glyph，并调用其公开 `LegacyRenderer` 和 `render_decoration`。没有自建 shaping、字体布局或 glyph geometry 缓存。

`TextLayout` 现在私有持有不可变 Buffer，兄弟模块仅读取首行尺寸；防止行索引建立后被旁路修改。每行只增加一个既有基线值；在 x64 对齐下 locator 从约 32 字节增至 40 字节，5,000 行增加约 39 KiB，换取滚动时不扫描全部字形。算法为 `O(log R + G_visible)`，其中邻接行属于有限 overscan。

回归比较全 Buffer 绘制参考与可见行输出，覆盖中文、emoji/ZWJ、结合符、阿拉伯文、希伯来文、下划线、删除线、自动换行和分数像素边界，在 75/100/125/200% 比例一致。另有既有 50/100/300%、窄宽窗口、主题与混合文档压力测试。它们不等同于实际屏幕字体/交互的人工验收，也不是对所有系统字体墨迹边界的穷尽证明。

### P2：缩放保留失效字号，Preview purge 没有清文字位图

Cosmic Text 的 SwashCache key 包含字号，但不会自动淘汰旧字号。Source/Preview 原先改变比例后继续持有旧条目；Preview 的 release 方法只清公式与图片。

复现：同一文字预览经过 125/150/200/250/300% 再回到 100%，缓存由 42 项增至 229 项；两个 Preview release 路径均未清文字字形，断言失败。

修复：有效比例改变时重置文字 raster cache；窗口仅改大小时保留复用。Preview 现有 raster/document release 路径同时释放文字缓存。修复后同一循环回到 42 项，两个释放路径清空；Source 的相应测试确认文档与 generation 不变。

这消除了已复现的旧字号和隐藏投影驻留，不代表为所有上游字体缓存建立了独立字节预算。固定比例下长期大量不同字符的常驻表现仍需独立压力测量。

## Release 局部测量

环境：Windows 11 家庭中文版 build 26200，Intel Core i7-12700H，物理内存约 15.80 GiB，Rust 1.97.1 / x86_64-pc-windows-msvc，PowerShell 7.6.5；使用仓库 Release opt-level 3、fat LTO、codegen-units 1 和静态 CRT 配置。

渲染基线对应修改前的渲染生产代码，另加入同一合成 fixture；修复后对应 `37fa6e0...` 上本报告所列未提交修改。测试程序是 Cargo Release 单元测试可执行文件，**不是** promoted candidate 或公开 `StickyMD.exe`。

方法：800×300 软件 viewport、scale 1.0，无 GUI；每个场景 33 次 paint，前 3 次 warmup、其后 30 次固定 warm 样本，nearest-rank p95 为排序后的第 29 项。所有保留样本均纳入统计。median 沿用测试中排序后的第 16 项（上中位数）。两个基准串行，测量时本轮未并发构建或运行其他测试；未控制机器上其他进程。

文本 fixture 含中文与 Latin、多短段落，带生产条件下存在的图片 adapter；在 0/4,000 px 之间交替滚动。代码 fixture 是单个 1,000/5,000 行 fenced block，在 5,000/6,000 px 间滚动。计时只包含 pipeline paint 调用与返回 frame，不包含初始 build、Windows 输入/事件队列、桌面合成或屏幕呈现。

物理显示器数量、系统 DPI、Defender 状态未记录；本实验的逻辑 viewport/scale 是固定输入。它不是合同中的完整 OS 性能/资源 cohort，不用于判断端到端输入门或发布 readiness。

单位均为 ms：

| 场景 | 修复前 median / p95 / max | 修复后 median / p95 / max | 新增布局数（前 → 后，33 次） |
| --- | --- | --- | --- |
| 100 KiB 纯文字 + adapter | 64.2665 / 103.8588 / 107.5848 | 0.1788 / 0.1968 / 0.2176 | 32 → 0 |
| 1 MiB 纯文字 + adapter | 286.6074 / 321.8038 / 336.1103 | 0.1582 / 0.2959 / 0.3166 | 32 → 0 |
| 1,000 行代码块 | 10.9309 / 13.4248 / 13.6860 | 0.2343 / 0.2483 / 0.3247 | 不依赖图片 band |
| 5,000 行代码块 | 53.9911 / 63.7841 / 65.0831 | 0.2603 / 0.3273 / 0.3396 | 不依赖图片 band |

复跑最小命令：

```powershell
cargo test -p stickymd-render --release --locked --lib phase5_preview_release_baseline_image_adapter_scroll -- --ignored --nocapture --test-threads=1
cargo test -p stickymd-render --release --locked --lib phase5_preview_release_baseline_long_code_scroll -- --ignored --nocapture --test-threads=1
```

本次先 `cargo test -p stickymd-render --release --locked --lib --no-run`，再直接调用产生的测试 EXE 执行相同 filter。修复后测试 EXE SHA-256：`ee9c9cdf985f0a24264cb2ad53b4a3e3237a5ec64b7e7b3f2272d7f819d57e41`。

基于证据的推断：收益主要来自消除全文工作，而不是更换数据结构或降低视觉质量。不能由这些数字推断任何文档和机器都能达到亚毫秒交互。

## 高内聚、低耦合与后续优先级

已追踪的编辑、预览和保存主链仍符合合同：EditorCoordinator 私有持有 DocumentState；Source/Preview 仅保留投影；Preview worker 持有自己的 pipeline；I/O worker 接受 typed job 并返回 generation/revision 标记的完成事实。没有在本次覆盖路径发现必须重写 canonical authority 的证据。

本轮修改在既有 owner 内完成：UndoManager 决定分组；MathEngine 决定缓存 admission；TextLayout 持有不可变整形与行索引；PreviewPipeline 决定缓存生命周期。`tree_has_local_images` 只由 RenderTree 更新，不是第二文档 authority。未引入跨层文件写入或 worker 直接修改文档。

文件规模检查发现 input 路由、Source projection、window runtime、I/O worker 有超过约 500 手写行的模块。这是审视信号，并非按行数强拆的理由。input 目前主要分派键盘/IME/剪贴板，I/O worker 主要负责 mailbox 和任务生命周期；宜在后续实际变更时按事务/平台 effect 进一步收敛，而非现在搬动大量稳定代码。新可见行 painter 单独放入 TextLayout 子模块，避免把绘制实现继续塞进 pipeline。

后续值得投入的事项：

1. 用新的当前候选执行 Preview/Source/Split 与隐藏后的五轮 OS 资源协议，检查 glyph 生命周期修复的实际 PWS/Private Bytes 收益。
2. 测固定比例下大量不同字符、复杂公式的长期 resident footprint；不要以 item count 或一次常见文档低内存宣传覆盖所有输入。
3. 有本地图片的文档跨 band 仍可能全布局，初始 Preview/zoom 也仍有全文整形工作。当前修复聚焦已明确证明的多余工作；进一步增量 layout 需先有对应 fixture、归因和复杂度收益证据。
4. 优先关闭诊断合同缺口，见下文；不为“首版前一次解决所有问题”做无证据的大规模重写。

## PowerShell 职责与 Rust CLI 化判断

本轮重新盘点 Git tracked 文件，共 24 个、1,907 行：

| 类别 | 文件数 / 行数 | 主要职责 | 当前判断 |
| --- | --- | --- | --- |
| `tools/smoke/*.ps1` | 17 / 658 | 参数、定位仓库、启动 Rust CLI、传播退出码 | 已是薄入口，无需再迁移业务逻辑 |
| `tools/release/*.ps1` | 6 / 809 | 可复现 ZIP、身份/校验和、第三方声明、固定版本 Syft/SBOM、PE/manifest/icon/allowlist 与 promoted artifact 检查 | 可迁移，当前保留成熟脚本成本更低 |
| `helpers/windows-uia.ps1` | 1 / 440 | 原生导出对话框、托盘、截图与物理输入的 Windows UIA/Win32 适配 | Rust 可通过 COM/Win32 实现，但平台维护和 unsafe 成本较高 |

Rust `tools/stickymd-smoke` 已负责阶段任务图、断言、进程生命周期、资源/性能门、receipt、readiness 与成功账本；不进入产品包。PowerShell 仍包含具体打包与校验细节，不能宣称“全部逻辑都已经是 Rust”，但当前合同允许成熟 package/GUI helper。

已执行 PowerShell 7.6.5 AST 解析：24 文件、0 syntax errors。没有本轮完整运行 UIA、打包/SBOM 下载、远程 workflow 或 PS 5.1 动态兼容矩阵；语法通过不等于这些场景通过。

**基于证据的建议**：整批 Rust CLI 化技术上可行，当前没有足够必要性。主要收益是减少工具语言种类，不能降低 `StickyMD.exe` 的运行时内存或输入延迟。实际代价包括 ZIP/PE 资源、原子发布、Syft 调用和 UIA/COM 适配重测。若未来明确要求单文件自包含、目标环境无 PowerShell，或出现可复现的 shell 兼容/维护故障，再优先迁移 `package-path` 与 promoted-artifact 身份判断，后处理打包/验证，最后考虑 UIA。

本轮只给现有 Rust Phase 05 性能任务增加串行测试参数，并接入新的同名前缀基准；未复制 verdict，未改 PowerShell、release schema 或发布资产。

## 验证事实与缺口

截至正文落盘，已执行并核实：

- 原始失败：两项 Undo 回归、公式租用预算、生产 adapter 纯文字布局次数、屏幕外字形缓存、旧字号积累、Preview release 未清文字缓存。
- `cargo test -p stickymd-core --locked`：50 个单元测试、5 个 property tests 通过，1 个 opt-in 性能测试未运行。
- `cargo test -p stickymd-render --locked`：133 个单元测试通过、9 个 opt-in 基准跳过；2 个语义、6 个数学、6 个混合渲染压力集成测试通过。
- 修复后 Release render 单元测试：133 通过、9 ignored；两项新滚动基准分别执行并通过。
- `cargo fmt --all --check` 通过；workspace Clippy 曾捕获新增基准的常量 assert 写法，改为显式 Release 运行检查后通过。最终交付前对后续窄改动复核结果追加在 Resolution。
- Rust smoke 的 114 个库测试已通过；CLI 文档链接集成测试发现本报告当时尚未创建，因此首次失败。正文现在已落盘，后续复测结果追加在 Resolution，不把首次失败记为成功。

Qualification Module Registry 的本地查询显示六组模块均 `RUN_REQUIRED`；没有复用未匹配的旧 PASS，也没有刷新或覆盖 last-success。此次未运行 Phase 0–14 完整 campaign。

尚未验证：真实 IME、鼠标选择、屏幕视觉/字体墨迹、多显示器、完整 UIA/发布包、当前产品 EXE 的五轮 PWS/Private Bytes/idle CPU、启动 cohort，以及长时常驻。既有人工状态不升级，旧发布 resource receipt 不当作本轮测试。

另一个已核实但未实现的合同缺口是 `crash.log`：plan 承诺存在，而当前实现与公共诊断指南明确没有。详细证据、记录字段/容量/故障协议需要确定的原因，以及两条可选路径见 [诊断风险报告](RISK-2026-09-07-crash-diagnostics-contract.md)。本轮没有偷偷修改 plan 或增加新的日志数据写入边界。

## Resolution — 2026-09-07

最终验证已完成：

- 最新 `cargo test -p stickymd-render --tests --locked --quiet`：133 个单元、2 个语义、6 个数学、6 个压力测试全部通过，9 个 opt-in 基准按默认跳过；最新不可变 Buffer 封装在本次编译范围内。
- `cargo test -p stickymd-smoke --test cli_exit --locked --quiet`：2/2 通过，先前指向尚未创建报告的链接失败已关闭；同轮先前已完成的 smoke 库测试为 114/114。
- `cargo test -p stickymd-win --locked --quiet`：244 通过、8 个 opt-in 测试跳过。
- 加上 core 55 项，常规自动测试合计 562 项通过。Release render 的 133 项与 debug 同名测试不重复计数；两项 Release 滚动基准另列，已实际运行。
- 最新 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`git diff --check` 通过。
- `cargo run -p stickymd-smoke --locked -- phase 00 --json`：`governance contracts` 与 `acceptance readiness` 均为 `PASSED`。回执明确 `worktree_dirty=true`、artifact/executable SHA 为 null，不能当作 candidate 资格化。
- 最终 branch、HEAD 和本地 `origin/main` 仍为交接基线；本轮改动未 stage/commit，保留另行出现的 `.codegraph/`。

对上文复杂度符号作精确限定：`G_visible` 指选中视觉行（含邻接 overscan 行）的 glyph 总数，不是裁剪后可见像素数；单条不换行的超长横向行仍遍历整行 glyph。本轮证明并消除的是纵向长 block 的全文扫描，未声称实现横向 glyph culling。

诊断合同风险仍待维护者确定记录字段、容量/保留策略和失败协议；真实 GUI、物理显示环境、当前产品 EXE 的 OS 内存与启动验收仍保持未验证。没有发布操作或新版本 readiness 结论。

## Resolution — 2026-09-25 review

本次复审处理此前保留的 16 个修改文件与 4 个新增项目文件。源码基线是
`c5606fe6d62cf89d7bb15bdb77ef72178b7710bb` 加本报告覆盖的未提交维护补丁；测试时工作树为 dirty。
恢复到 `main` 前后核对原始补丁身份，CLI 收尾产生的重叠修改已保留。独立的
`.codegraph/.gitignore` 已作为 `c5606fe` 推送，忽略本机数据库、PID、socket 和日志。

正文的 PowerShell 清单、迁移建议、测试数量和未提交状态只描述 2026-09-07。
发布工具 Rust CLI 收尾后来已合并到 `main`；本次没有据旧报告回退这些工具改动，也不复用旧测试数作为当前结论。

### 新复现并修复的问题

1. **P2：固定邻接行裁剪漏画堆叠组合符号。** 用 80 个上方重音组合符号构造跨越数行的墨迹，
   将视口放在其所属行上方，对照 Cosmic Text 完整绘制，原实现得到 684 个不同的 RGBA 通道值。
   根因是字形的整形偏移可超过一行，固定一行 overscan 不能覆盖。
   现在在排版完成后对 Cosmic Text 的 glyph offset 做一次 O(glyphs) 归约，只保留一个绘制 margin；
   绘制时先扩展候选范围，再通过原 row locator 二分定位。行 locator 的构造仍为 O(visual rows)，
   没有增加逐字形几何副本或在每次滚动时扫描全文。
   新测试包含向上和向下堆叠的组合符号，以及位于原行两侧的视口；既有多文字系统、装饰与分数边界对照也保留。
   这取代正文“各保留一行即可”的实现假设。极端组合符号会扩大必须绘制的行范围；
   不声称该输入下仍有固定行数的 overscan，也不把本测试当作所有字体的穷尽证明。
2. **P3：LRU 计数器溢出会丢失仍被租用的位图账本。** 单元测试将 recency counter 直接置于 `u64`
   边界；原 `next_stamp` 清空 map 与 bytes，使保留中的租用不再计账，查找回归失败。
   现在只按既有顺序重新编号，不删除条目或字节计费。回归覆盖 lookup/insert 两条溢出路径、
   只淘汰未租用条目，以及租用释放后可重新腾出空间。这是注入的极端边界，未观察到实际使用中的自然溢出。

其余已审查修改继续采用既有 owner：Undo 分组、数学 raster admission、语义本地图片判定、
Source/Preview 字形缓存生命周期，以及 Phase 05 基准串行执行。无需修改 plan、公共接口、
文档 authority、持久化边界或依赖。

### 当前自动验证

- 修复前的 core/render 基线：210 项常规测试通过，10 项 opt-in 测试未运行。
- 两个新问题均先建立失败回归；修复后的可见像素对照与 ByteLru 定向测试通过。
- `cargo fmt --all --check` 与 `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过。
- `cargo test --workspace --locked`：637 项通过、0 失败、18 项 opt-in 测试按默认跳过。
  其中 core 55、render 157、smoke 181、Windows 244；没有把定向重复执行另行计数。
- `.codegraph/.gitignore` 所在远端提交的 [CI run 36177936404](https://github.com/Develata/StickyMD/actions/runs/36177936404)
  已通过；它不包含本次运行时补丁，不能作为这些补丁的远端测试证明。

本次仍未做实际桌面视觉、IME、物理多显示器、当前产品 EXE 的 OS 资源 cohort 或新版本 exact-candidate
资格化。`crash.log` 合同缺口已再次核实，仍按独立风险报告保留为设计事项，本次未增加日志写入路径。

### 2026-09-25 验证闭环与 Release 测量

`phase 00 --json` 的 governance contracts、acceptance readiness 均为 PASSED；receipt 明确
`worktree_dirty=true`，artifact/executable SHA 为 null。Qualification Module Registry 的六组模块
查询仍为 RUN_REQUIRED，本次未运行完整资格化 campaign、未更新 last-success，也未引用旧模块 PASS。

Release 命令：

```powershell
cargo test -p stickymd-render --release --locked --lib phase5_preview_release_baseline -- --ignored --nocapture --test-threads=1
```

三项测试全部通过，包含原有 preview 构建基准与两项滚动测量。环境为 Windows 11 家庭中文版
build 26200、i7-12700H、15.8 GiB RAM、Rust 1.97.1（8bab26f4f）、Windows MSVC target。
测量进程查询到 1 个活动显示器、system DPI 96；Defender AntivirusEnabled 和 RealTimeProtectionEnabled
均为 false。本次只使用固定逻辑 viewport/scale 的 headless 渲染，没有实际窗口启动样本。

单位为 ms；每个滚动场景预热 3 次后取 30 个样本，测试串行执行：

| 场景 | median | p95 | max | 新增布局 |
| --- | --- | --- | --- | --- |
| 100 KiB 纯文字 + adapter | 0.1807 | 0.2704 | 0.3048 | 0 |
| 1 MiB 纯文字 + adapter | 0.1857 | 0.2241 | 0.3616 | 0 |
| 1,000 行代码块 | 0.3104 | 0.5662 | 0.6742 | 不依赖图片 band |
| 5,000 行代码块 | 0.4333 | 0.5807 | 0.6241 | 不依赖图片 band |

既有构建基准的 20 次 warm total median/p95/max 分别为：20 KiB `8.7612/10.7989/344.2293`、
100 KiB `34.2706/39.8751/357.2962`、1 MiB `325.8321/388.1691/904.0634`。
对应单次 cold 为 `388.6676/567.3867/989.2950`；未裁掉这些慢样本，既有 warm-p95 断言通过。
这些诊断数值不证明冷启动、任意字体输入、OS 常驻资源或已发布产品的端到端体验。

本次日志与原始改动备份保存在本机临时证据目录 `stickymd-runtime-review-28214c98`，
包含 `baseline-core-render.log`、`final-checks.log`、`release-preview.log`、`phase00.json` 和环境记录。
测试源码已分批提交；运行时维护提交仅保留本地，独立 `.codegraph/.gitignore` 提交已推送。
