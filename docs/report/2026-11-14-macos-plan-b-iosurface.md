# 评估：macOS 方案 B，用 IOSurface 直接呈现（2026-11-14）

## 结论

**建议批准方案 B 进入正式实现。** 原型在 VM 里同时改善了延迟和内存：

- **按键延迟：** 持续输入时每帧绘制的中位数从约 8.7 ms 降到约 3.5 ms；空闲后首键从 13–18 ms 降到约 5–6 ms。
- **常驻内存：** Source/Preview/Split 每种约少 10.7 MiB，Hidden 不变。
- **回归：** `mac-gui` 自动化行与上一轮相同。

**前提：** 原型只在 VM 里测过，像素是否正确（颜色、方向、是否撕裂）还没有人看过。需要 USER 先在本机看一眼原型（见文末）。

**性质：** 这是呈现边界的改动，属于骨架级，须 USER 批准后才进入正式实现。正式实现要改 `03`、`06`、`09`、`10` 四章。

## 背景

[内存报告](2026-11-13-macos-memory-optimization.md) 的末两节已经说明：

- 每键绘制在 VM 里 66–70% 的时间花在 CoreAnimation 把 softbuffer 0.4.8 交来的 CGImage 整帧再渲染、复制一遍上；
- 这份复制同时留下一份常驻的 5.4 MiB 副本；
- softbuffer 每次 present 还要新分配一块整帧缓冲；
- 把 IOSurface 直接作为图层内容，系统就不必复制。上游 softbuffer 的 IOSurface PR #329 尚未合并。

USER 2026-11-13 要求“做原型和报告”。

## 原型

**位置：** 分支 `proto/iosurface-presenter`，提交 `3a3f7ef`，基于 `feat/macos-host` 的 `4f811b1`。这是评估用的原型，不进产品分支。

**做法：**

- **宿主自己呈现。** 共享层的 `Host` 加了一个默认返回 `None` 的 `present_frame`。macOS 宿主实现它，Windows 走原来的 softbuffer。
- **IOSurface 轮换。** macOS 宿主在 `platform/macos/iosurface.rs` 里维护最多 4 个 BGRA 格式的 IOSurface：
  - 每帧挑一个系统没在用（`IOSurfaceIsInUse` 为假）、也不是当前图层内容的；
  - 加锁，把 tiny-skia 画布的 RGBA 转成 BGRA 写进去，解锁；
  - 在禁用动画的事务里把它设为一个子图层的内容。
- **图层几何沿用 softbuffer 的设置：** 锚点 (0,0)，`geometryFlipped`，左上对齐，不透明；每帧同步根图层的 bounds 和缩放。
- **绕开 softbuffer：** 不再调用 softbuffer 的 present，它的每帧分配和 CoreAnimation 的复制随之消失。
- **隐藏：** 隐藏回执后释放全部 IOSurface，并清空图层内容。

**没做的：**

- IOSurface 的色彩空间：没有设置，softbuffer 用的是 deviceRGB；
- 局部提交；
- 直接画进 IOSurface（画布仍是一份独立的 5.4 MiB）；
- 4 个都在用时，原型会复用最旧的一个，可能撕裂，也没有计数。

## 实测（VM，macOS 27.2，scale 2）

**证据目录：** `dist/evidence/mac-vm-2026-11-14/planb-prototype-3a3f7ef/`，`binaries.sha256` 记了三个可执行文件：

- 原型 app：`d3fac171…`
- 基线 app：`fdbd90ef…`，即 `feat/macos-host` 的 `70d1aa9`–`4f811b1`，与 A/A 基线是同一个二进制
- 延迟驱动：`498fc33f…`

### 延迟（`mac-latency`，基线与原型交替 5 轮，40 次运行全部有效）

证据 `latency/mac-latency.json`，SHA-256 `56e3ff46d1ce2e9cb7b9751b75e0986e9eb5c736d2c572f31833608a1922f99f`。单位 ms，按键合并的 p50 / p95。

| 场景 | 基线：总延迟 | 原型：总延迟 | 基线：绘制 | 原型：绘制 |
| --- | --- | --- | --- | --- |
| 100 KiB 持续输入 | 9.86 / 12.68 | **5.08 / 7.08** | 8.75 / 10.80 | **3.38 / 4.70** |
| 1 MiB 持续输入 | 9.92 / 12.88 | **5.90 / 8.42** | 8.54 / 10.84 | **3.74 / 5.26** |
| 100 KiB 空闲后首键 | 17.03 / 22.97 | **8.65 / 20.36** | 14.03 / 17.79 | **5.28 / 7.34** |
| 1 MiB 空闲后首键 | 21.02 / 31.49 | **9.56 / 25.80** | 16.83 / 20.34 | **5.82 / 8.02** |

**逐轮看：** 原型每一轮的绘制 p50 都低于基线的每一轮。

| 场景 | 原型绘制 p50 | 基线绘制 p50 |
| --- | --- | --- |
| 100 KiB 持续输入 | 3.1–3.6 ms | 8.5–9.1 ms |
| 1 MiB 持续输入 | 3.3–3.9 ms | 4.4–9.3 ms |
| 空闲后首键 | 4.6–6.2 ms | 12.5–18.5 ms |

A/A 测出的运行间波动约 ±2–4 ms，这个差别远在其外。两边的等待段相近，空闲后首键的 p95 仍有 20–26 ms，那是等重绘调度，不在绘制段里。

### 内存（`mac-resources`，r24 驱动，每种 5 次，全部有效）

证据 `res/mac-resources.json`，SHA-256 `ca749244c3e8b9d7e3bf3e13e35575786b2a57bd832e39e213a88d79d4431bf7`。单位 MiB，中位 / 最大。

| 场景 | 原型 | 方案 A（r11，`7ba67fa2…`） | 临时门 |
| --- | --- | --- | --- |
| Source | **36.5 / 36.6** | 47.2 / 47.3 | 72 |
| Preview | **39.1 / 39.3** | 50.1 / 50.1 | 96 |
| Split | **39.7 / 39.7** | 50.4 / 50.5 | 128 |
| Hidden | 26.6 / 26.7 | 26.7 / 26.7 | 48 |

r11 的 app 是 `5f72d97`；基线 `fdbd90ef` 只多了关闭状态的延迟记录器，不影响内存。

**分区拆解**（探针驱动 1 次，证据 `probe/`）：

- **不再出现的：** CoreAnimation 副本，以及 Malloc Large。
- **新增的：** IOSurface。Preview/Split 为 11 MB，即 2 个；探针那次 Source 为 16 MB，即 3 个，读数 42.1 MiB。
- **其余不变：** Untagged 约 11 MB 仍是画布和 Source 面板。
- **结论：** IOSurface 的数量由合成器何时放手决定，2 个或 3 个都可能，所以 Source 的常驻在约 36.5 到 42 MiB 之间。
- **Hidden：** 不保留 IOSurface，但有约 5.5 MiB 计为“已拥有、未映射”。推断是合成器还拿着最后一块，总量与方案 A 相同。

### 回归（`mac-gui`，r24 驱动）

PASS 8、FAIL 0、NOT_TESTED 12，逐行与 r25 相同，证据 SHA-256 `832e832f924d4f0f133822cf93ce06c41a2aeec82f98b7129658d9848df0cfe4`。

- **W04 又是未定：** 离开起点在 521–595 ms，采样间隔偏大。起点的下界与历次相同，按 r25 的分析属于 VM 采样抖动。
- **隐藏后再显示能工作：** 探针的“隐藏后再显示”场景有效，窗口回到屏幕上。

## 未验证与风险

- **像素正确性：** 颜色、上下方向、打字和滚动时有没有撕裂或闪烁，都没有看过。VM 里没有屏幕录制权限，`mac-gui` 只查窗口状态和笔记内容。
- **色彩空间：** 原型的 IOSurface 没有设色彩空间，与现在的 deviceRGB 可能有细微色差。
- **撕裂：** 4 个都在用时原型会写其中最旧的一个，可能撕裂，且没有计数。
- **缓冲个数不定：** 内存因此在 2 到 3 帧之间变动。
- **未覆盖的环境：** 多显示器与混合 DPI、拖动缩放、真机（VM 的图形是半虚拟化的）都没有测。
- **这是未公开用法：** 用 IOSurface 作 `CALayer.contents` 并没有公开文档，但浏览器、GTK 和 softbuffer 的 PR #329 都这样用。

## 正式实现的设计（待批准）

1. **呈现边界。**
   - 共享层的 `Host` 增加宿主呈现能力：宿主拿到完成的画布，自己负责提交；返回 `None` 时仍走 softbuffer。
   - macOS 宿主实现 IOSurface 呈现器，原生调用只在 `platform/macos/` 里。
   - Windows 不实现这个能力，行为不变。
   - 共享层的绘制流程（`render.rs`）有改动，需要 Windows 回归。
2. **缓冲。**
   - 默认 3 个，用 `IOSurfaceIsInUse` 判断能否写；
   - 都在用时不写在用的缓冲：本帧跳过，下一次事件循环再画，并计数；
   - 上限写进 plan。
3. **色彩空间。** 设为与现在一致的色彩空间，先比较 deviceRGB 与 sRGB 的效果，再定。
4. **隐藏与显示。** 沿用方案 A：隐藏回执后释放全部 IOSurface，显示前重建并提交一帧。
5. **macOS 上不再创建 softbuffer 的表面。** 省掉它的子图层和逐帧分配；画布改由共享层或呈现器持有。
6. **可以另行评估的后续：**
   - tiny-skia 直接画进 IOSurface（若 RGBA 格式的 IOSurface 可用），省掉 5.4 MiB 画布和一次转换；
   - 只提交变化区域；
   - 分配器的备用块：softbuffer 不再逐帧分配后，它主要服务于其他大块分配，可以重新评估是否保留。
7. **plan 修改：**
   - `03`：呈现所有权；
   - `06` frame-presentation：macOS 的呈现路径；
   - `09` macOS 映射：图层与缓冲生命周期；
   - `10` macOS 门：内存策略与延迟诊断的结论。
8. **验证：**
   - 单元测试，覆盖缓冲选择与释放；
   - `mac-latency` 的 A/B；
   - `mac-resources` 跑 5 次；
   - `mac-gui`；
   - USER 目视；
   - Windows 桌面回归；
   - codex-reviewer 审查。

## 需要 USER 决定

1. **是否批准按上面的设计正式实现方案 B。** 这包括修改上述四章 plan。
2. **在本机目视检查原型。** 运行 `/Users/develata/gitclone/StickyMD/dist/proto-iosurface/StickyMD.app`（可执行文件 SHA-256 `d3fac171…`，ad-hoc 签名），检查以下几项：
   - 文字颜色与背景是否正常；
   - 画面是否上下颠倒；
   - 打字、滚动、拖动缩放、切换 Source/Preview/Split 时有没有撕裂、闪烁或残影。

   它的笔记写在 `dist/proto-iosurface/note/`，与正式包分开。
