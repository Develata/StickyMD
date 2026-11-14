**建议首个 macOS 版本选 A：只支持左右贴边。PASS / no blocker——仅针对这个设计方向，不代表 macOS 实现或发布验收通过。** 顶部仍可摆放普通浮动窗口。

理由是保留“贴边即隐、迅速出现”的核心用途，同时延续一个主窗口和现有收起模型。B 改善命中率，但没有消除菜单栏下方缺乏指针止挡的问题；C 已有本机可行性证据，但引入双窗口协调和硬件降级；D 虽然维持单窗口，却需要改变几何、最小尺寸和窗口层级，未必比 C 简单。

以下事实来自指定 HEAD 的文件；额外的未跟踪 notch 探针单独标注。全程未修改文件。

**已核实事实与发现**

- **SHOULD_FIX | 收起后恢复焦点会撤销感应条置顶。** [reducer.rs:203](/Users/develata/gitclone/StickyMD/crates/stickymd-app/src/flow/window/reducer.rs:203) 对任何 `false→true` 焦点变化撤销 temporary topmost；[Spaces 日志:57](/Users/develata/gitclone/StickyMD/experiments/macos-probes/results/spaces_probe-run2.txt:57) 记录细条失焦、切回后再次获焦。**机制／影响：** 手动收起状态保留，但用户未开启 Pin 时，感应条失去覆盖普通窗口的保障。**最小修复：** 在共享 reducer 按呈现状态决定撤销条件，详见下文。注意，[plan 09:111](/Users/develata/gitclone/StickyMD/docs/plan/09_windows_shell.md:111) 本身规定“获得焦点必须撤销”，因此这是需要澄清契约的缺口，不能只称实现偏离。

- **SHOULD_FIX | A 不能只在宿主忽略 Top 请求。** [geometry.rs:121](/Users/develata/gitclone/StickyMD/crates/stickymd-app/src/flow/window/geometry.rs:121) 在固定三个候选中选最近边；[reducer.rs:530](/Users/develata/gitclone/StickyMD/crates/stickymd-app/src/flow/window/reducer.rs:530) 还可能保留旧边。**机制／影响：** 先选 Top 再过滤，会漏掉同样合格的 Left；旧 Top placement 也可能继续恢复。**最小修复：** 宿主提供 `supported_dock_edges`，共享层在求最近值**之前**过滤，并覆盖启动、拖放、显示器恢复；不支持的旧 Top 恢复成工作区内可见的 Floating。Windows 保持三边。

- **SHOULD_FIX | C／D 不是当前端口下的局部几何替换。** [collapsed_frame:186](/Users/develata/gitclone/StickyMD/crates/stickymd-app/src/flow/window/geometry.rs:186) 保持主窗口宽高，只移动位置；[WindowHost:62](/Users/develata/gitclone/StickyMD/crates/stickymd-app/src/ports/window.rs:62) 的显示、定位、置顶均针对同一个窗口。**机制／影响：** 宿主暗中换成面板或缩小窗口，会让共享层的目标矩形、可见性与回执含义失真。**最小修复：** 明确增加收起呈现协议，而非从离屏坐标猜测业务状态。

- **NEEDS_VERIFICATION | 探针证明事件可达，尚未证明产品时序。** 左右与顶部命中记录成立；但 [hover_probe.rs:354](/Users/develata/gitclone/StickyMD/experiments/macos-probes/src/bin/hover_probe.rs:354) 是任一检测器命中后阻塞等待 250 ms，再展开，并未验证“连续悬停 100 ms、提前离开取消”。额外的 [notch_guided.txt:7](/Users/develata/gitclone/StickyMD/experiments/macos-probes/results/notch_guided.txt:7) 记录三次不激活应用的展开，但其 [实现:198](/Users/develata/gitclone/StickyMD/experiments/macos-probes/src/bin/notch_probe.rs:198) 在进入时立即展开。**影响：** 不能把这些日志当作完整 hover 验收。**最小补证：** 用真实 reducer 测短暂穿越、停留、展开后的跨区域移动，以及全屏中的肉眼可见和实际命中。

**刘海方案的边界**

补充事实：未跟踪的 [notch_auto.txt:1](/Users/develata/gitclone/StickyMD/experiments/macos-probes/results/notch_auto.txt:1) 显示本机刘海区域约 **185×32 pt**，level 27 小窗口无需覆写夹取方法即可到顶。因此不能断言 C 技术不可行。

| 场景 | 已核实事实 → 风险与建议 |
|---|---|
| 无刘海显示器 | Apple 明确说明顶部无遮挡时辅助区域返回 `nil`／零矩形；不存在可复用的刘海空区。应降级左右贴边或可见 Floating，不能虚构一个“刘海”。[Apple](https://developer.apple.com/documentation/AppKit/NSScreen/auxiliaryTopLeftArea-uglc) |
| 合盖使用外屏 | **推断：** 内屏消失，原感应位置失效；外屏若无刘海，必须同时撤除感应面板并恢复主窗口。仅迁移主窗口会留下不可达入口。此路径未测。 |
| 全屏 Spaces | `fullScreenAuxiliary` 允许同处全屏 Space，并不证明具体层级下一定可见、可命中。C 的感应窗和主窗都须验证；D 须验证展开时的层级转换。现有日志还出现 `occlusion_visible=false`。[Apple](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/fullscreenauxiliary)、[日志:64](/Users/develata/gitclone/StickyMD/experiments/macos-probes/results/spaces_probe-run2.txt:64) |
| 自动隐藏菜单栏 | 顶缘也是系统显示菜单栏的手势区域。“没有菜单项”不等于“没有系统行为”。是否同时触发、遮挡或丢失跟踪须实测；不能固定使用 33 pt，`visibleFrame` 会随设置变化。[Apple 菜单栏](https://support.apple.com/en-sg/guide/mac-help/mchlp1446/mac)、[visibleFrame](https://developer.apple.com/documentation/appkit/nsscreen/visibleframe?changes=_8) |

C 是否违反单窗口合同：**plan 09:44 只写“单主窗口”，单看这一句不能断言禁止辅助原生窗口；但 [plan 02:220](/Users/develata/gitclone/StickyMD/docs/plan/02_positioning_and_scope.md:220) 明确是“一个窗口”，[验收投影:125](/Users/develata/gitclone/StickyMD/docs/acceptance-cases/phase-08.md:125) 也明确主窗口兼作 sensor。** 因此 C 至少需要获批的辅助感应窗例外，不能解释为现有合同已涵盖。它不必增加第二份 Note 或编辑会话，但增加了真实的生命周期职责。

若以后采用：

- **C：** 宿主上报当前屏幕的可用感应几何；端口增加显式的“主窗／感应面呈现”请求及可区分的回执；[HostSignal:43](/Users/develata/gitclone/StickyMD/crates/stickymd-desktop/src/host.rs:43) 增加感应面进入／离开事件。共享层统一管理 100／500 ms，区分两个区域的指针状态，不能把从感应面移入正文当作离开整个交互区域。隐藏到托盘、退出、显示器消失必须处理两者；创建失败时保持主窗可见。真正使用 `NSPanel` 时还须关闭默认的失活隐藏行为。[Apple](https://developer.apple.com/documentation/appkit/nswindow/hidesondeactivate?changes=_4)
- **D：** 独立保存展开尺寸与收起矩形，明确收起时的最小尺寸例外，并处理 resize 引发的排版与渲染。185×32 小于当前 220×120 下限。还需显式切换感应态和编辑态的层级；现有 winit `AlwaysOnTop` 映射到 floating level，并不表达“高于菜单栏”。[winit 源码:1531](/Users/develata/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.13/src/platform_impl/macos/window_delegate.rs:1531)

**更简单的补充方案**

顶部保持普通 Floating，收起使用已有菜单栏“隐藏／显示”入口；不新增“顶部固定 dock”状态。这可作为 A 的自然补足。若顶部自动隐藏确实是首版必需，再优先验证 B，而不是直接引入刘海依赖；8 pt 目前只是候选值，不是已验证答案。

**焦点缺口的具体修法**

保留真实焦点事件，**不要由宿主吞掉 `Focused(true)`**。共享层应保证：

- 手动收起状态及其收起动画期间，恢复焦点不撤销 sensor topmost，也不因此自动展开。
- 进入展开路径后，已聚焦则撤销；未聚焦的 hover 展开继续保留。
- 拖动、隐藏到托盘继续撤销，用户 Pin 与配置不变，沿用现有请求／回执机制。

现有 [start_expand:471](/Users/develata/gitclone/StickyMD/crates/stickymd-app/src/flow/window/reducer.rs:471) 已处理展开时的撤销，大部分修复可限于焦点分支及契约措辞。补测完整 `手动收起→失焦→重新获焦→hover 展开`，覆盖动画中／动画后、Pin 开／关及迟到回执。**选 A 不改变这个修复；左右细条同样需要它。**

未覆盖：未运行测试或 GUI 复现，未验证外屏、合盖、菜单栏自动隐藏及真实全屏命中。左右方案在侧置系统 Dock、相邻显示器接缝处也不能直接继承“指针会停在边缘”的单屏结论。