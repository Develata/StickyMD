# 09_windows_shell.md - Windows 壳层与平台边界合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-20
- `Scope`: 窗口生命周期、托盘、置顶、透明度、主题、dock 状态机、多显示器、DPI、单实例唤醒；Windows 实现细节与核心行为契约分离

---

<a id="windows-shell-purpose"></a>
## Purpose

定义 StickyMD 与 Windows 11 桌面环境的交互契约，并把
**Windows 实现细节**与**核心行为契约**严格分离：
核心契约在本章与 `04`；具体 Win32 手段只允许存在于平台 adapter。

<a id="platform-adapter-boundary"></a>
## Boundary

- 业务层不得直接调用 Win32；一切平台能力经 adapter。
- 优先使用跨平台 Rust API（winit）：窗口、键盘/鼠标、IME、drag、resize、
  cursor、redraw、window level。只有 winit 不足时才进入 Win32 层。

## Owned Objects

`window::placement`、`config::runtime`（窗口相关字段）。

---

## 核心行为契约（平台无关部分）

### 窗口与视图

- 单主窗口，三互斥视图：Source / Split / Preview（默认 Source）。
- 默认尺寸 520×680 DIP；最小 360×240 DIP；分栏推荐宽度 900 DIP；
  顶部控制区 34 DIP；分栏每栏不得窄于 240 DIP。
- 进入分栏窗口过窄时可向屏幕内侧临时扩展；离开时恢复先前宽度。
- 顶部图标控件：模式切换×3、Always on top、主题、透明度、手动收起、关闭到托盘。
  控件仅在鼠标位于窗口内/窗口聚焦/正在交互时明显显示，其余状态淡化但可发现。

### 主题

- 三态：Light / System / Dark；三态滑块 ☀ / ▣ / ☾。
- 首次默认 Light；System 跟随 Windows 应用主题且运行时变化立即响应。
- 写入 config；不提供主题编辑器/自定义颜色/主题文件。

### 透明度

- 作用整窗（背景、文字、公式、图片、控件、阴影）。
- 70–100 整数 slider（步长 1）+ 整数输入框；默认 96；越界 clamp；非整数不提交。
- 拖动实时预览；仅在松开滑块、Enter、输入框失焦时写配置（不逐步写盘）。

### Dock 与自动隐藏

- 支持边缘：左、右、上。**不支持底部**（避免任务栏冲突）。
- 吸附：松手时窗口边缘距工作区边缘 ≤ 12 DIP → dock；保留 3 DIP 感应条
  （顶部吸附时条宽=窗宽；左右吸附时条高=窗高）。
- 拖离边缘 > 16 DIP → Floating。
- 焦点规则：获得键盘焦点后禁止自动收起；IME composition 期间绝不收起；
  鼠标临时离开不触发收起。
- 失焦 700 ms → 收起（前提：未重新聚焦、未拖动、无内部弹出控件、无冲突/恢复提示）。
- hover 感应区 100 ms → 展开；仅 hover 未获焦时，鼠标离开 500 ms → 收起。
- Esc 与手动收起按钮：无条件立即收起。
- 动画：展开/收起 140 ms ease-out。

### 托盘生命周期

- 托盘菜单只有：显示/隐藏、置顶、退出。不加新建/打开/最近/设置/关于/更新/同步。
- 纸张窗口关闭按钮 = Hide to tray，不退出进程。
- 真正退出仅来自托盘“退出”：立即保存 → 等待资产事务 → 安全 GC → 保存配置
  → 释放 mutex → 清理已确认无用的临时文件。
- 退出保存失败：显示明确错误、保持运行、不静默退出、不丢内存文本。

### 单实例唤醒

同一 canonical 目录第二个实例：通知第一实例显示并激活，自己立即退出。

### 固定视觉效果（不可配置）

小幅圆角、轻微窗口阴影、140 ms 滑入/滑出动画、Light/Dark 固定纸张背景、
控件 hover 动画、1 DIP 分栏分隔线、适度内边距。
禁止：Acrylic、Mica、毛玻璃、动态背景、主题市场、背景图片。

---

## 多显示器契约

v1 一级需求，必须覆盖：左右排列、上下排列、负坐标、混合 DPI、主显示器切换、
运行中拔显示器、睡眠/唤醒重枚举、远程桌面显示配置变化。

### 配置保存（不只保存绝对坐标）

```text
monitor_identity、dock_edge、offset_ratio、width_dip、height_dip、
floating_x_ratio、floating_y_ratio
```

### 目标显示器不存在时

1. 恢复到主显示器。
2. 保证窗口完全位于工作区内。
3. 保留原尺寸，除非大于新工作区。
4. 原 dock 状态在主屏相同边缘恢复。
5. 不允许窗口留在不可见坐标。

### DPI

- manifest 声明 PerMonitorV2。
- 跨显示器移动重算 scale；公式 cache key 含 DPI；图片按设备像素绘制；
  IME 候选 rect 用物理坐标；docking 存 DIP；3 DIP 感应条按显示器 scale 转换。

---

<a id="windows-adapter-mapping"></a>
## Windows 实现映射（adapter 内细节，可替换）

| 契约能力 | 参考实现手段（非契约） |
| --- | --- |
| 整窗透明度 | `WS_EX_LAYERED` + `SetLayeredWindowAttributes`（alpha 0–255） |
| 窗口圆角 | DWM 窗口属性 |
| 显示器身份 | `QueryDisplayConfig` + `DisplayConfigGetDeviceInfo` 设备路径稳定哈希 |
| 单实例 | 本地命名 mutex / event：`Local\StickyMD.Mutex.<dir-hash>`、`Local\StickyMD.Show.<dir-hash>` |
| 原子替换 | `ReplaceFileW`，回退 `MoveFileExW`；`FlushFileBuffers` |
| 文件剪贴板 | `CF_HDROP` |
| 链接打开 | Windows Shell |
| 系统主题变化 | 主题变化事件/注册表监听 |
| session shutdown | 系统会话通知 |
| 外部文件监听 | notify 的 Windows backend；回调只发轻量事件 |

这些手段是实现选择；更换实现不改核心契约。

---

## Inputs

窗口/键鼠/焦点/显示器事件、托盘菜单动作、Set* intent 与 coordinator capability request。

## Outputs

平台事实/回执、window::placement/VisibilityState 的协调结果、配置提交请求与第二实例唤醒
信号；adapter 自身不得决定业务状态。

## State Changes

WindowDockCoordinator/LifecycleCoordinator 是窗口业务状态的 mutation owner；Shell 只提交
事件并呈现结果，Windows adapter 只执行已批准的窗口/IPC/monitor 操作并返回结果。

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| 显示器消失 | 主显示器恢复 + 完全可见 |
| 动画中断 | 收敛到最终状态 |
| 托盘创建失败 | 记录错误，保持窗口可用（退出路径必须仍可达） |
| 唤醒 IPC 失败 | 第二实例安全退出，不启动重复会话 |
| DPI 异常 | 按 100% 兜底，窗口仍完全可见 |

## Configuration

窗口/主题/透明度/置顶 → ConfigState（见 `04`）；时间常量（12/3/100/700/500/140 ms）为固定内部参数。

## Lifecycle

启动：Writable check → 单实例 → 载入 config → 载入 note → reconcile → 窗口+托盘。
运行：Close→HiddenToTray；Tray Show→恢复；Tray Quit→清理退出。

## Extension / Replacement Points

平台 adapter 整体可替换；winit 与 Win32 的职责分界在实现阶段按“winit 优先”裁定。

## Performance Critical Paths

空闲时 event loop 进入 wait（见 `10`）；动画期间短暂 request redraw。

## Verification

- 手工矩阵：dock×3、hover、失焦、typing guard、Esc、手动、topmost、
  透明度 70/85/96/100、Light/System/Dark、运行时主题切换、close to tray、tray quit、
  双显示器（同 DPI/混合 DPI/左侧/上方）、拔线、sleep/resume、RDP。
- property：任意窗口几何变化后窗口仍在至少一个工作区内。
- 验收：AC-019..AC-029。

## Non-Goals

底部 dock、多窗口、全局快捷键 v1、开机启动 v1、窗口管理增强（贴角、四分区等）。
