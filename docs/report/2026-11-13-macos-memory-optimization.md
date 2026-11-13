# 分析：macOS 内存的去向与优化方案（2026-11-13）

## 结论

**内存去向。** macOS 宿主的常驻内存主要是整窗大小的像素块，不是字体、文档或预览树。

- Source 可见时 47 MiB，其中约 32 MiB 是 6 块整帧大小的内存。
- 字体数据库每个线程只有约 0.6 MiB 活跃分配（不含启动期瞬时峰值）。

**隐藏后不降的原因。** 隐藏回执后，StickyMD 已经释放了 Source 面板缓冲。但 macOS 的 malloc 把释放的大块保留为 `Malloc Large (empty)` 区，仍计入 `phys_footprint`。`malloc_zone_pressure_relief(NULL, 0)` 在 VM 里实测返回 0 字节，收不回这些页。

**临时构建实测（各 1 次，单位 MiB，不是门证据）：**

| 构建 | Source | Preview | Split | Hidden |
| --- | --- | --- | --- | --- |
| 基线（`4ce8a226…` 正式包） | 46.9 | 50.5 | 50.1 | 52.9 |
| 只加大块分配走 `mmap` | 41.7 | 44.3 | 44.8 | 42.8 |
| 再加隐藏时缩掉帧存储 | 41.7 | 44.3 | 44.8 | **26.3** |

第二行的改动是：宿主进程里 ≥ 1 MiB 的分配直接用 `mmap`/`munmap`，释放即还给系统。第三行再加上：隐藏回执后把整窗画布和图层内容缩成 1×1。

**建议。**

- **现在做方案 A。** A 是宿主大块分配策略加隐藏时释放呈现存储。它把 Hidden 从约 53 MiB 降到约 26 MiB，远低于 48 MiB 的临时门，常驻各态也降 5–6 MiB。
- **方案 B 另起报告。** B 是 macOS 原生呈现器，常驻还能再降约 10 MiB，但它是结构改动，等 A 落地、门定下来以后再评估。
- **两条都要先批准。** 它们都要改 plan，且 `10` 现行规定“不引入自定义 allocator，除非基准证明明确收益”。

**R01 保持 `BLOCKED`，临时门不动。**

## 背景

- **R01 现状。** P16-R01 的 Hidden 在 r9/r10 为 53.0–55.7 MiB，超过 `10` 的 macOS 临时门 48 MiB，见 [VM 验收](2026-11-10-macos-vm-acceptance.md) 的“R01：Hidden 内存”。
- **USER 的要求（2026-11-13）：**
  - “可以问问 codex 怎么优化，再去调研一下看看类似的产品怎么处理内存占用的”；
  - “不只是隐藏时可释放内存，常驻也需要优化改进”。
- **本报告的材料：**
  - VM 里的区域与调用栈拆解；
  - Codex（`gpt-6-astra`，只读）对代码的盘点；
  - 一个子代理对同类产品与 Apple 文档的调研；
  - 三个未提交的临时构建实验。

## 测量方法

**环境与工具。**

- 测试虚拟机 `mac-test`：macOS 27.2，显示 scale 2（2048×1536）。
- 夹具与 `mac-resources` 相同：20 KiB，20 个公式。
- 默认窗口 1040×1360 物理像素，一整帧 5.40 MiB。
- 测量工具是 `stickymd-smoke mac-resources` 的临时探针版（未提交，补丁见证据目录 `mem_probe_patch.py`）。它比正式版多两处：
  - 允许 1 次重复；
  - 多一个 `menushow` 场景：菜单“隐藏”后再“显示”。
- 每次采样后，探针对被测进程运行 `footprint`、`footprint -v`、`vmmap -summary` 和 `heap -s`。
- 归因用两种手段：
  - 不剥符号的同源构建，带 `MallocStackLogging=lite`，再用 `malloc_history -callTree`；
  - 正式包的 `heap` 活跃块大小，对照 `footprint -v` 的逐区域脏页。

**局限。**

- 每个场景只跑 1 次，只是探针，不是 `10` 要求的 ≥ 5 次门证据。
- 带调用栈的那次运行总量偏高（Hidden 71 MiB），是记录本身的开销，只用来归因。

**证据。** 证据在 `dist/evidence/mac-vm-2026-11-13/mem-breakdown/`（git 忽略），`binaries.sha256` 记了各构建的可执行文件哈希。各次运行的 `mac-resources.json` SHA-256 如下：

| 运行 | SHA-256 |
| --- | --- |
| 正式包 | `d1f6cb0b73084c5226c760d856ac8948b2ad7f93092b1538c9b8a17a4ea06e96` |
| 带调用栈 | `0c3ef9044af942b887adb8e8801254fe241c322ebf43a758f2cece19b12a6168` |
| `pressure_relief` 实验 | `4748aa7e4fd28f82e3215b650b666628c87021d5dae9b1e53b04c1c581026764` |
| 只用 `mmap` | `6889dfd709d68f86af70db0b8514c2cbf2cf9178193d26a8c35ad7fadbae31e2` |
| `mmap` 加隐藏缩帧 | `ad28fa77c9a317d37651d17bc0b4f3a80a195ef1e5af73f3aa02cf233362f9f7` |

## 内存构成（已验证）

### 各状态的类别

数据取自正式包，`footprint` 第二次采样，单位 MiB。

| 类别 | Source | Preview | Split | Hidden | 隐藏后再显示 |
| --- | --- | --- | --- | --- | --- |
| 总计 | 47 | 50 | 50 | 53 | 53 |
| Malloc Large | 27.0 | 27.0 | 26.0 | 27.0 | 27.0 |
| Malloc Small | 8.8 | 12.0 | 12.0 | 13.0 | 13.0 |
| CoreAnimation | 5.5 | 5.5 | 5.5 | 5.5 | 5.5 |
| 框架 `__DATA*`/`__AUTH` | 3.1 | 3.1 | 3.1 | 4.5 | 4.5 |
| 其余（页表、栈、元数据等） | 约 2.5 | 约 2.5 | 约 2.5 | 约 3 | 约 3 |

### Source 的六块整帧内存

页大小 16 KiB；每块是 316 或 346 个脏页。

| 持有者 | 大小 | 依据 |
| --- | --- | --- |
| 整窗 `tiny_skia::Pixmap` | 5.40 MiB | 调用栈：`DesktopShell` 启动时 `calloc`；`crates/stickymd-desktop/src/frame/surface.rs:33` |
| Source 面板缓冲 `source_frame` | 4.94 MiB | 调用栈：`preview_runtime::configure_viewports`；`crates/stickymd-desktop/src/shell/preview_runtime.rs:184` |
| softbuffer 当前图像的像素 | 5.40 MiB | 调用栈：`SoftwareSurface::present` → softbuffer 0.4.8 `cg.rs:263` 的 `vec![0; w*h]` |
| CoreAnimation 的再渲染副本 | 5.45 MiB | 调用栈：`CA::Render::copy_image` → `create_image_by_rendering` → `mmap` |
| 已释放、仍计数的大块 ×2 | 10.8 MiB | `heap` 只见 3 个活跃整帧块，`footprint -v` 却有 5 个脏的 Malloc Large 区；`vmmap` 标为 `Malloc Large (empty)` |

**为什么会有已释放的大块。**

- softbuffer 0.4.8 的 CoreGraphics 后端每次 `buffer_mut()` 都新分配一整帧，交给 `CGImage` 当图层内容；上一帧随图像释放。
- 这种逐帧分配让 malloc 留着两块空的整帧。
- softbuffer 上游 master 已改为复用同一块缓冲，但还没有发版：crates.io 最新仍是 0.4.8。

### Hidden

- 隐藏回执后，`ReleaseHiddenCaches`（`crates/stickymd-desktop/src/shell/window_runtime.rs:213`）确实丢掉了 `source_frame`：`heap` 中那块 4.94 MiB 的活跃块消失了。
- 但对应的区域变成了 `Malloc Large (empty)`。Hidden 时这类空区共 15.9 MiB。
- 整窗画布、softbuffer 图像和 CoreAnimation 副本仍然活着，因为隐藏不触及它们。

### 菜单

- `menushow` 与 Hidden 在每个类别上都相同。经辅助功能打开过菜单后，比 Source 多约 6 MiB：
  - Malloc Small 多 4.25 MiB；
  - 框架 `__DATA*`/`__AUTH` 多 1.4 MiB。
- 带调用栈的运行里，`NSStatusBarButtonCell popUpStatusBarMenu` 一支有 2.0 MiB 活跃分配，另有 0.5 MiB 在辅助功能 `PerformAction` 的自动释放池里。
- 这条路径由测量驱动经辅助功能触发。用鼠标点开时是多少，仍未分开测。
- 这些分配属于 AppKit 首次使用菜单的成本，不在 StickyMD 自己的数据里，不建议为它改产品菜单。
- 早先的“只开菜单”探针其实是两次“置顶”切换，见 VM 验收报告；本次“隐藏后再显示”是更干净的对照。

### 字体与预览

- **字体：** 每个 `FontSystem` 的 `fontdb` 扫描约 0.6 MiB 活跃分配。Source 一个；Preview/Split 在 worker 上再一个。这只是活跃字节；Source 启动峰值 52 MiB 中的瞬时分配没有归因。
- **Preview 的 worker：** 持有预览面板像素 4.9 MiB 和排版 2.5 MiB。排版主要是 cosmic-text 的整形结果。

### 已排除

- **`malloc_zone_pressure_relief(NULL, 0)`：** 临时构建在隐藏回执 3 秒后调用，两次都返回 0 字节，Hidden 仍是 53.0 MiB。
- **其他候选：** 以下都不是本次超门的来源，理由见 Codex 盘点与下表：字体数据库共享、换 mimalloc、缩撤销预算或线程栈、改菜单。

## 方案

### A：宿主大块分配策略加隐藏时释放呈现存储（建议现在做）

**A1 大块分配走 `mmap`（仅 macOS 宿主）。**

- `apps/stickymd-mac` 设全局分配器：≥ 1 MiB 且对齐不超过页大小的分配用 `mmap`，释放用 `munmap`；其余照旧交给系统 malloc。
- 这样释放的整帧立即离开 footprint，也顺带去掉 softbuffer 逐帧分配留下的空区。
- `unsafe` 只在 `apps/stickymd-mac/src/platform/` 下，带 SAFETY 说明。

**A2 隐藏回执后释放呈现存储（仅 macOS 策略）。**

- 在已有的 `ReleaseHiddenCaches` 里，把整窗画布与 softbuffer 表面缩到 1×1 并提交一次。这会替换图层内容，让 softbuffer 图像和 CoreAnimation 副本一起释放。
- 显示时按真实尺寸重建并全量重绘，再让窗口可见。
- 不要直接丢弃 softbuffer 表面：0.4.8 的析构不摘除子图层，见 Codex 盘点。

**实测：** A1+A2 的临时构建 Hidden 26.3 MiB；只有 A1 时常驻 41.7/44.3/44.8 MiB。

- 隐藏后 CoreAnimation 降到 0.09 MiB。
- 整帧的 `mmap` 区降到 0.33 MiB。
- 空闲 CPU 与基线同量级（Source 0.0005–0.0011%）。

**还剩的 Hidden 26 MiB：**

- 菜单与框架约 6 MiB；
- Malloc Large 里一块 5.4 MiB 的系统侧分配，来源未归因，推断在 CoreGraphics/CoreAnimation 一侧；
- 其余是 Source 投影、字体与框架数据。

**风险与代价（推断，实施时要测）：**

- **呈现耗时：** 每次呈现多一次 `mmap`/`munmap`，外加约 346 个零页缺页。要用 macOS 输入延迟与 `present` 耗时验证。逐帧分配本来就要清零，差别可能不大，但没测。
- **显示耗时：** 显示要多一次分配和全量重绘，按 `10` 的预览构建与启动目标测“显示到首帧正确”。
- **失败路径：** 显示失败、快速隐藏/显示、隐藏期间的 DPI 或尺寸变化、未完成的预览任务、IME 恢复都要回归。

**Windows：** 不变。A1 只在 macOS 二进制里；A2 由宿主能力或 `cfg` 限定为 macOS 策略，Windows 的 DIB 与 `age == 1` 局部提交路径不动。

**需要改的 plan（须 USER 批准）：**

- **`10` 结构性内存策略：** 为 macOS 宿主写明大块分配策略的例外与证据，因为现行条款是“不引入自定义 allocator，除非基准证明明确收益”。同时补“Hidden 释放整窗呈现存储”。
- **`09` 的 macOS 适配映射：** 写明隐藏与显示时呈现存储的生命周期。

**实施时的验证：**

- 每项按 `10` 做 before/patch/after 与回归测试；
- 重跑 `mac-resources` 5 次；
- 补 `mac-gui` 的隐藏/显示回归；
- 测输入延迟与显示耗时。

### B：macOS 原生呈现器（建议 A 之后另起报告）

**做法：** 用 IOSurface 作图层内容，复用缓冲，替代 softbuffer 的 CoreGraphics 路径。整窗画布也可以由它持有。

**预期（推断）：** 常驻再省约 5.4 MiB 的 CoreAnimation 副本。若那块系统侧 5.4 MiB 也来自同一路径，再省约 5.4 MiB，常驻合计约 −10 MiB。

**影响：**

- 涉及 `03`/`06`/`09` 的呈现边界；
- 是结构改动，风险中高：生命周期、通道顺序、色彩空间、撕裂；
- Windows 不受影响。

### C：小项（随 A 或之后做）

| 项 | 节省 | 说明 |
| --- | --- | --- |
| 从 Source/Split 切到 Preview 时丢掉不显示的 `source_frame` | 4.9 / 2.4 MiB（只在切换后） | 低风险；Codex 发现切到 Preview 时没有对称清理。限 macOS 策略以免改 Windows |
| Source 直接画进整窗帧，去掉 `source_frame` | 常驻 4.9（Source）/ 4.7（Split）MiB | 改共享绘制接口，要改 plan；Windows 保留现有缓存与光标局部提交 |
| 隐藏时清 Source 的字形缓存 | 0.1–1 MiB（推断） | 低风险，收益小 |

### 不建议

- **`malloc_zone_pressure_relief`：** 实测 0。
- **字体数据库共享或丢弃：** 每个约 0.6 MiB，且 `10` 要求字体缓存各线程独占。
- **换 mimalloc 或 jemalloc：** 唯一找到的 macOS 公开报告（单个应用的使用反馈，不是对照基准）里 footprint 反而更高。
- **改菜单或绕开菜单：** 是框架首次使用的成本，且菜单行为由 `09` 规定。

## 同类产品与资料

**同类产品的做法：** 没有找到同类编辑器或菜单栏应用公布的“隐藏后释放”前后数字。可借鉴的一手做法：

- **WebKit：** 释放自己的缓存后再调用 `malloc_zone_pressure_relief`。本机实测它收不回 `Malloc Large (empty)`。
- **Zed：** 窗口被遮挡时停掉 display link，这是省电措施，不是省内存。它还去掉了一个约 600 MB 的预分配缓冲。

**Apple 文档：**

- WWDC21 10180 定义 footprint 为脏页加压缩页，干净的文件映射不计。所以 `fontdb` 的 `memmap` 映射不是负担。
- 遮挡状态的文档只谈停止工作省电。
- 状态栏应用在菜单栏可见时整体仍算“可见”，所以隐藏信号要用自己的隐藏回执，不能用应用级遮挡。

## 需要 USER 决定

**是否批准方案 A。** 批准包含两件事：

- 在 `10` 写入 macOS 宿主大块分配策略的例外与 Hidden 释放呈现存储，在 `09` 写入对应的生命周期；
- 按上面的验证实施。

方案 B、C 的去留，以及 `10` macOS 内存门的最终值，等 A 落地并重测 5 次后再定。

## 补充与更正（同日复核）

1. **两块空的整帧，来源不同。** 上文把 Source 的两块 `Malloc Large (empty)`（10.8 MiB，正式包 Source 的 `vmmap` 已确认这个标记）都归给 softbuffer 的逐帧分配，不准确。
   - 只用 `mmap` 的临时构建把 Rust 侧的大块都移出了 malloc，常驻只降了 5.3 MiB。
   - 那次运行里仍剩一块 5.4 MiB 的 `Malloc Large (empty)`。
   - 所以两块里一块来自 softbuffer 的逐帧分配，另一块是系统侧（推断为 CoreGraphics/CoreAnimation）释放后留下的。后者就是 A 节“还剩的 Hidden”里那块未归因的 5.4 MiB。
2. **C 表 Split 的数字写错了。** 去掉 `source_frame` 在 Split 里省一个面板，约 2.4 MiB，不是 4.7 MiB。
3. **`10` 的优化顺序。** `10` 不只要求“不引入自定义 allocator，除非基准证明明确收益”，还在 Performance Critical Paths 里固定了顺序：先查持续 redraw，再查重复分配，再查无界缓存，最后才考虑 allocator。A1 跳过了“重复分配”，即 softbuffer 0.4.8 每次呈现的 `vec!`。理由如下：
   - **只修重复分配，常驻能省的和 A1 一样。** 复用 softbuffer 缓冲（自写呈现路径，或等上游发版）同样去掉那块约 5.3 MiB 的空区。
   - **但 Hidden 降不下来。** 隐藏时释放的整窗画布和 softbuffer 缓冲仍经 malloc，仍会变成计数的空区。
   - **只有 A2、没有 A1 的推断。** 这时只有 `mmap` 出来的 CoreAnimation 副本真正离开。Hidden 约 53 − 5.5 ≈ 47.5 MiB，贴着 48 MiB 的门，没有余量。
   - **结论。** 要让隐藏时的释放体现在读数上，帧存储必须不经 malloc：要么用 A1 的分配策略，要么交给 B 的原生呈现器。
4. **显示时恢复没有验证。**
   - 临时构建只在隐藏时缩到 1×1，没有实现显示时恢复。
   - 那次运行的“隐藏后再显示”场景在隐藏之前就因辅助功能报错（`AXExtrasMenuBar absent`）中止了。
   - 所以“显示时重建并全量重绘”目前只是设计，没有观察支持。
5. **常驻的收益有限。** A 对常驻只省 5–6 MiB。常驻的大头在 B（约 −10 MiB，推断）和 C 的“直接画进整窗帧”（Source 约 −4.9 MiB）。这两项都是结构改动，按原建议放在 A 之后评估。
6. **批准 A 的附加条件。** 按工程宪法，性能排在内存之前。A1 让每次呈现多一次 `mmap`/`munmap` 和约 346 次缺页。实施时须先测 macOS 的输入延迟与 `present` 耗时：
   - 不回退，才重跑 5 次门测量；
   - 回退，就退回只修重复分配，并回到 USER 重新决定。

## 实施结果（2026-11-13）

**USER 的决定。** 批准方案 A（“批准方案 A”）。实施期间 USER 另说“以后可以引入自定义分配器”，`10` 的分配器条款已据此修改。

**实施。** 分三个提交：`ce46c52`，以及审查后的 `5f72d97`、`d6e68e7`。

- **A1：** `stickymd-posix` 的大块分配器。≥ 1 MiB 的分配各自映射，释放即归还。
- **A2：** 隐藏回执后，画布与表面缩为 1×1。隐藏期间不绘制、不提交预览任务；显示前重建并提交一帧；显示失败时再次释放，聚焦失败时不释放。
- **复用槽（原报告没有的取舍）：** 只做 A1 时 VM 里每帧多 0.15–0.3 ms，属于回退。按事先约定的“延迟不回退”，加了一块备用块。代价是 A1 原本带来的约 5 MiB 常驻收益没有了。

**结果（VM，每种 5 次）：**

| 场景 | 读数（MiB） | 与 r10 相比 |
| --- | --- | --- |
| Hidden | 26.7 | 原 55.4，降约 29 |
| Source | 47.2 | 持平 |
| Preview | 50.1 | 持平 |
| Split | 50.4 | 持平 |

每帧分配耗时与系统 malloc 相同。证据与命中率见 [VM 验收](2026-11-10-macos-vm-acceptance.md) 的“方案 A 的 release”。

**常驻仍待改进。** 可选的路线有：

- 去掉备用块：省约 5 MiB，每帧多 0.14–0.24 ms；
- 方案 B，原生呈现器：推算省约 10 MiB；
- C 节的“直接画进整窗帧”：Source 省约 4.9 MiB。

后两项是结构改动，需要另起报告。

**仍未验证：**

- 显示后的首帧像素；
- Windows 桌面回归，需要 Windows 主机；
- 多显示器与混合 DPI。


## 补充：备用块的去留（2026-11-13，codex 建议）

USER 让 codex 评估备用块的去留。codex 的排序是：保留（现状）＞ 空闲后停用复用 ＞ 直接去掉 ＞ 等方案 B。理由有三：

- 宪法把性能排在内存之前；
- 各场景离临时门还有余量；
- 现有证据只有微基准，没有真实输入延迟。

**“空闲后停用复用”的做法。** 最后一次呈现之后过 N 秒，宿主停用复用，并归还备用块；下一次绘制之前再恢复。

- **分层：** 计时放在 shell，复用的开关由宿主转给分配器，分配器本身不建计时器。
- **只停用一次不够：** 必须持续停用到下次绘制。CoreGraphics 的释放回调晚于提交，只清一次槽，晚到的释放会把备用块重新填上。
- **兼容现有流程：** 隐藏时取消这个截止时间，按现有流程释放；显示失败与聚焦失败的处理不变。
- **代价：** 每段输入结束后多一次唤醒；空闲后的第一个按键要承担冷分配。

**承诺之前要补的测量：**

- macOS 的真实输入延迟，分持续输入和空闲后的首键两种情况；
- 缺页的拆分计时；
- 空闲前后的内存，每种场景 5 次；
- 唤醒次数与 CPU，要包含 trim 本身；
- 生命周期交错：晚到的释放回调、隐藏与显示、各种失败路径。

codex 还提了一个变体：空闲时保留映射，用 `MADV_FREE_REUSABLE` 交还内容，复用前再 `MADV_FREE_REUSE`。现在的备用块把页数元数据存在块里，这个变体要先解决元数据怎么保存；codex 没有把它排在前两项之前。

**两处更正：**

- 结论里“`malloc_zone_pressure_relief` 收不回”应限定为本次 VM 与这次调用的观察。Apple 只把它定义为尽力回收，返回 0 的原因没有归因。
- 方案 B 的“约 −10 MiB”不能由“复用自己的缓冲”直接推出。上游 IOSurface 方案用三缓冲，备用块也会被其他大块分配用到。
