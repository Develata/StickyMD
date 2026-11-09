# Debian 13 平台决定

日期：2026-11-09。基线：`main` `edda2b9`。状态：等待 USER 批准；批准前不建 Debian 宿主，不改 `docs/plan`。

## 背景

`02` 的平台节要求，建立非 Windows 宿主前须单独批准：桌面环境与 X11/Wayland 矩阵、便签目录与实例身份、缺少贴边定位、隐藏或置顶能力时的产品行为、默认字体、输入法验收矩阵、资源与性能门。`09` 另要求批准托盘的事件循环接入与不可达时的行为。

Phase 15 已把共享实现合入 `main`。新宿主只需实现 `stickymd_desktop::host::Host` 及其端口，并提供存储、单实例、监视与启动接线。`stickymd-app` 与 `stickymd-desktop` 的测试已在 Linux 运行。

## 证据等级

事实来自锁定版本的 crate 源码（winit 0.30.13、softbuffer 0.4.8、arboard 3.6.1、tray-icon 0.24.2）、Mutter 48.7 与 gnome-shell 48.7 源码、Debian trixie 包页面。**没有在 Debian 桌面上运行过任何东西。** 标“未实测”的项由下文探针定案。

## 事实

Debian 13 的 GNOME 是 48，默认 Wayland 会话；`gnome-session` 依赖 `xwayland`。GNOME X11 会话以 Recommends 提供。KDE Plasma 是 6.3，上游默认 Wayland，同样带 XWayland。

| 能力 | X11 后端（Wayland 会话下经 XWayland） | Wayland 原生后端 |
| --- | --- | --- |
| 设定窗口位置 | 可以，Mutter 接受 ConfigureRequest | 不可能，xdg-shell 没有这个请求，winit 为空操作 |
| 读回窗口位置 | 可以 | 不可能（`NotSupported`），定位回执无法确认 |
| 隐藏与显示 | 可以（map/unmap） | 不可能，`set_visible` 为空操作 |
| 唤醒与聚焦 | `_NET_ACTIVE_WINDOW`；Mutter 是否放行未实测 | `focus_window` 为空操作，不能解除最小化 |
| 置顶 | `_NET_WM_STATE_ABOVE`，Mutter 处理 | 不可能，Mutter 没有相应协议 |
| 整窗透明度 | 宿主自设 `_NET_WM_WINDOW_OPACITY`，Mutter 读取 | 不可能，Mutter 48 没有 alpha-modifier，softbuffer 用 `Xrgb8888` |
| 拖动与缩放 | 可以 | 可以 |
| 贴边收起到感应条 | 普通窗口至少留 `clamp(size/4, 10, 75)` 像素在屏内，3 DIP 感应条做不到（读 Mutter 源码，未实测） | 不可能 |
| 输入法 | XIM；winit 注释称 preedit callback 下候选框可能在窗口左下，未实测 | text-input-v3，Mutter 转发光标矩形，gnome-shell 按它绘制 IBus 候选框 |
| 分数缩放下的清晰度 | 未实测 | 未实测 |
| 剪贴板 | arboard X11 后端，Mutter 桥接 X11 与 Wayland 选区；图片端到端未实测 | Mutter 48 没有 data-control 协议，只能回退 X11 |

其他事实：

- **托盘：** gnome-shell 48.7 没有 StatusNotifier 宿主，`gnome-shell-extension-appindicator` 不在默认任务里。`tray-icon` 在 Linux 要 GTK3 与 appindicator，并在创建线程运行 GTK 循环。`ksni` 用 zbus，后端是 tokio 或 async-io。两条路线都碰到 `AGENTS.md` 的禁令（GTK 或通用 async runtime），需要新 ADR。
- **保存对话框：** `rfd` 的 `xdg-portal` feature 经 dlopen 的 `libdbus-1` 调用 portal，用 `pollster::block_on` 等待，失败回退 `zenity`，不引入 Tokio。
- **字体：** `source/fonts.rs` 的候选里没有 Linux 字体。`fonts-noto-cjk` 由 `task-chinese-s-desktop` 推荐，GNOME 任务是否带上未确认。
- **共享层现状：** 无托盘路径已经存在，`tray_available = false` 时不隐藏，关闭即安全保存并退出。启动时透明度、置顶失败会继续运行，但 `Unsupported` 目前也按失败在状态行提示，对长期不具备该能力的宿主不合适。
- **窗口回执：** 定位按观察确认（`PLACEMENT_TOLERANCE_PX = 0`），读不回位置的后端无法完成确认，需要能力分支。

## 问题

便签的卖点（贴边即隐、迅速出现、置顶）依赖窗口定位、隐藏、层级，在 Wayland 原生后端上协议层面就做不到。而项目的第一优先级（输入法正确性）在 X11 后端上风险最大，在 Wayland 原生后端上最好。两者不能同时取最优，要由实测决定。

## 可选路径

| 方案 | 能得到 | 失去或风险 |
| --- | --- | --- |
| A：X11 单后端，用 `with_x11` 强制 | 位置、隐藏、唤醒、置顶、透明度都有实现路径；所有会话一套行为、一套测试 | 输入法候选框位置与分数缩放清晰度未知；依赖 XWayland 长期存在 |
| B：Wayland 原生单后端 | 输入法路径最好；缩放原生；面向未来 | 隐藏、唤醒、置顶、透明度、定位都没有，只剩一张普通窗口的便签 |
| C：按会话选后端 | 理论上各取所长 | 两套后端、两套验收矩阵，能力随会话变化，与单一实现的目标相反 |

## 建议

1. **支持矩阵：** Debian 13 x86_64 的 GNOME 48 Wayland 会话是唯一验收目标；KDE、GNOME X11 保持 `NOT TESTED`。
2. **后端：** 方案 A。探针 P1 或 P2 不通过时改用方案 B，因为输入法正确性优先于窗口特性。
3. **缺少能力时的规则：**
   - `Unsupported` 是宿主的常态，不是失败：不提示，对应控件隐藏或禁用。启动路径要区分它与真正的失败。
   - 不能隐藏时，不提供隐藏到托盘，关闭即安全保存并退出（现有路径）。
   - 不能读回位置时，不提供贴边，位置交给窗口管理器，只记住大小。
4. **贴边：** 首版关闭贴边吸附与自动收起。普通窗口不能完全离屏，要做就得改成“缩成细条窗口”，这是共享状态机的变化，单独决定。
5. **托盘：** 首版不做，走现有无托盘路径。标准 GNOME 上本来没有托盘，两条实现路线都要 ADR。第二阶段单独决定。
6. **便签目录与实例身份：** 与 Windows 相同，`<program-dir>/note/note.md`，以 `.tar.gz` 解压分发。不做 `.deb` 与 AppImage（程序目录只读），不回退到 XDG 目录，不可写时明确提示后退出。路径规范化不做大小写折叠。实例用程序目录内的 `flock` 加 `$XDG_RUNTIME_DIR` 下按路径哈希命名的 Unix socket。
7. **字体：** 说明依赖 `fonts-noto-cjk`，候选加入 `Noto Serif CJK SC`；缺字体时状态行提示，不阻止启动。
8. **输入法矩阵：** GNOME 上的 fcitx5 拼音与 IBus 拼音，验收组字、提交、取消、候选位置。范围替换与删除周边文本 winit 会丢弃，记为不支持。
9. **剪贴板与保存对话框：** arboard X11 后端，文本与图片；`rfd` 的 `xdg-portal`，失败回退 `zenity`。保存对话框阻塞主线程，与 Windows 相同。`pollster` 只等待单个 future，不视为通用 async runtime，这一点请确认。
10. **性能与资源门：** 先沿用 Windows 的数值作为目标，P9 实测基线后再定。

## 探针

在真实的 Debian 13 GNOME Wayland 会话上，用一次性程序验证（`experiments/`，不进产品）。

| ID | 内容 | 通过条件 | 不通过时 |
| --- | --- | --- | --- |
| P1 | X11 后端下 fcitx5 与 IBus 中文组字 | 组字与提交正确，候选框贴近光标 | 改方案 B |
| P2 | 150% 分数缩放下 XWayland 文字 | 不明显发糊 | 改方案 B |
| P3 | 设定位置并读回 | 读回与目标一致，否则记录偏差并定容差 | 定位改为尽力而为 |
| P4 | 置顶 | 压在普通窗口之上，不抢焦点 | 该控件禁用 |
| P5 | 透明度 | 生效且不影响绘制 | 该控件禁用 |
| P6 | 第二实例唤醒 | 窗口前置并取得键盘焦点 | 改为只提醒 |
| P7 | 向屏外移动 | 记录实际可离屏的量 | 供第 4 条参考 |
| P8 | 与 Wayland 应用互相复制文本和图片 | 内容一致 | 记录差异，图片单独决定 |
| P9 | 空闲内存、击键延迟、每帧复制量 | 得到基线 | 无 |

## 对规格的影响

批准后：`02` 平台节写入矩阵与目录身份，`09` 增加能力缺失的行为，`11` 增加 Debian 宿主的阶段与验收表。第 3 条的 `Unsupported` 规则与能力分支会改共享层，按 plan 先行。第 4、5 条关闭的能力不进入首版验收。

## Resolution

2026-11-09，USER 的答复：

- 后端倾向 Wayland 原生，不采用建议 2 的 X11。
- 托盘到 Debian 实机上再看。
- 贴边与其余建议没有表态，视为未批准，`docs/plan` 不变。
- Debian 排在 macOS 之后。

USER 追问能否靠“缩成极窄窗口”或自行原生实现得到这些功能。对照 Mutter 48.7 的协议列表与 winit 0.30.13 源码（均未在 Debian 上运行）：

- 缩小窗口可行（`request_inner_size`），但客户端不能设定或读取位置，也不能压在其他窗口之上，所以细条贴不到屏幕边，也可能被遮住。
- 自行实现能解决隐藏与唤醒：隐藏靠销毁并重建窗口，位置由合成器重新决定；唤醒靠 xdg-activation，Mutter 实现了该协议，而 winit 的 `focus_window` 是空操作。这两点由接口推断，未实现过。
- 指定位置、置顶、整窗透明度与贴边，Mutter 48.7 没有对应协议，客户端算法无法补足。能做到的路线是运行在合成器内的 GNOME Shell 扩展，需要单独安装、跟随 GNOME 版本适配，并因 JavaScript 需要 ADR。

USER 继续追问能否写一个盖在 Wayland 之上、不依赖 Wayland 的高优先级框：

- 绕过合成器不可行。Wayland 会话里屏幕由合成器独占，普通程序没有另一条路把画面放到屏幕上。这是 Linux 图形栈的一般事实，这次没有实测。
- 效果上最接近的是 XWayland 窗口。Mutter 仍然合成它，但接受它的定位与 `_NET_WM_STATE_ABOVE`（读 Mutter 源码）。收起时把窗口缩成贴边细条，窗口完全在屏内，“至少留一部分在屏内”的约束就不再限制感应条。
- 这条路线的风险仍是探针 P1、P2（输入法候选框、分数缩放），所以 Debian 恢复时先跑这两项，再决定后端。
