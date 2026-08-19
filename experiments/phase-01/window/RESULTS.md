# Phase 1B Spike — Window / Framebuffer（winit 0.30 + softbuffer + tiny-skia + Win32 adapter）

> 实验性代码。本目录不属于生产 workspace，可随时删除。
> plan_ref: docs/plan/09_windows_shell.md ; docs/plan/00_engineering_constitution.md#§5-idle-behavior

## 1. 验证目标（来自 Phase 1 任务 1B）

- 单一测试窗口，winit 0.30.13 事件循环 + softbuffer 0.4.8 CPU 帧缓冲 + tiny-skia 0.12.0 绘制。
- 静止（idle）时不得持续 `request_redraw()`；只在 dirty 事件（resize / 按键）时重绘。
- Win32 独占能力放入薄 adapter（`src/win32.rs`）：整窗不透明度（WS_EX_LAYERED +
  SetLayeredWindowAttributes）、Win11 圆角（DwmSetWindowAttribute, DWMWCP_ROUND）。
- 记录重绘次数、pixmap 重分配次数、DPI/scale 变化。

## 2. 环境

| 项 | 值 |
| --- | --- |
| OS | Windows 11 x64（构建机 20 逻辑核） |
| 工具链 | Rust 1.97.1（rust-toolchain.toml 锁定），MSVC |
| 依赖 | winit 0.30.13 / softbuffer 0.4.8 / tiny-skia 0.12.0 / raw-window-handle 0.6.2 / windows 0.62.2 |
| 构建 | `cargo build --release`（独立 crate，`[workspace]` 空） |

## 3. 结果

### 3.1 呈现链路：PASS

- 窗口创建成功（820×560 物理像素，scale_factor=1.0）。
- tiny-skia 渲染到复用的 `Pixmap`，逐像素转换为 softbuffer `u32` 缓冲并 `present()`。
- 关键 API 记录（softbuffer 0.4 与 0.3 不同）：`Context::new(display)` →
  `Surface::new(&context, window)` → `surface.resize(w,h)` → `surface.buffer_mut()` →
  `buffer.present()`。rwh 0.6.2 的 `DisplayHandle<'a>`/`WindowHandle<'a>` 为借用类型且
  无 `Arc`/`Rc` 实现，spike 用本地 `WinRef(Arc<Window>)` newtype 重新实现
  `HasWindowHandle`/`HasDisplayHandle` 解决所有权问题（生产代码沿用此模式即可）。

### 3.2 空闲行为：PASS（无持续重绘）

`ControlFlow::Wait` + dirty-only 重绘。进程关闭时打印的统计（release 构建，运行 8.06 s）：

```
uptime_s           : 8.06
redraw_count       : 4        # 1 次初始绘制 + 3 次启动期 Resized
pixmap_reallocs    : 3
seconds_since_draw : 7.95     # 关闭前约 8 秒内零重绘
final_size         : 820x560
```

### 3.3 空闲 CPU / 内存（PowerShell，release，窗口静止 ≥12 s 后采样，2 s 间隔 × 5 次）

| 采样 | CPU（占单核 %） | Working Set | Private Memory |
| --- | --- | --- | --- |
| 1 | 3.906（含启动余波） | 18.31 MB | 5.96 MB |
| 2 | 0 | 18.31 MB | 5.96 MB |
| 3 | 0 | 18.31 MB | 5.96 MB |
| 4 | 0 | 18.31 MB | 5.96 MB |
| 5 | 0 | 18.31 MB | 5.96 MB |
| 中位数 | **0%** | **18.31 MB** | **5.96 MB** |
| 最大 | 3.906% | 18.31 MB | 5.96 MB |

20 核机器上整机 CPU 占用 ≈ 0.000%。内存恒定（无泄漏迹象，pixmap 无重复分配）。

### 3.4 Win32 adapter：PASS（编译 + 运行时）

- `set_opacity_percent`（GWL_EXSTYLE | WS_EX_LAYERED + LWA_ALPHA）与
  `enable_rounded_corners`（DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND=2）在启动时
  调用成功，日志确认 `win32 attributes applied: opacity=100%, corners=true`。
- WS_EX_LAYERED 与 DWM 圆角可同时生效，未观察到渲染异常。
- unsafe 全部隔离在 `src/win32.rs`，带 SAFETY 契约注释。

### 3.5 交互功能（O=不透明度循环 70/85/96/100，R=圆角开关，Esc=退出）

- 代码已实现（按键 → adapter → 重绘）。
- 自动化验证仅覆盖：启动属性应用、resize 重绘、WM_CLOSE 正常退出（exit stats 打印成功）。
- 按键交互本身需人工验证：**NOT TESTED（人工）**——见 §5。

## 4. 观察与风险记录

1. 启动期出现一次意外的 `Resized 1064x1824`→恢复 820x560 事件（系统/显示器放置导致），
   证明 resize 路径稳健（重新分配 pixmap 并重绘一次即静止）。
2. DPI 变化（ScaleFactorChanged）代码路径已实现并记录 scale，但本机单显示器 scale=1.0，
   **DPI 切换场景 NOT TESTED**。
3. 全屏/最小化（size=0）已做守卫（跳过 resize/render），未实测。
4. softbuffer buffer 长度与窗口尺寸一致性有运行时校验，未触发。

## 5. 结论

| 项 | 判定 |
| --- | --- |
| winit+softbuffer+tiny-skia 呈现链路 | **PASS** |
| 空闲零重绘 / CPU≈0 / 内存稳定 | **PASS** |
| Win32 薄 adapter（不透明度+圆角） | **PASS** |
| 按键交互（O/R/Esc） | 已实现，**NOT TESTED（人工）** |
| DPI 切换 / 多显示器 | **NOT TESTED** |

判定：**PASS（附 2 项人工/环境限制）**。该呈现链路可作为生产 Interaction Shell 的基础；
生产化时需要：damage/局部重绘评估、IME 区域配合（交给 1C）、真实 DPI 环境复测。

## 6. 复现

```powershell
cd experiments/phase-01/window
cargo run --release
# O: 不透明度循环  R: 圆角开关  Esc: 退出（打印统计）
```

空闲 CPU/内存采样（5 次 × 2 s）：

```powershell
$p = Start-Process .\target\release\spike-window.exe -PassThru
Start-Sleep 12
1..5 | ForEach-Object {
  $p.Refresh(); $c1 = $p.TotalProcessorTime.TotalMilliseconds
  Start-Sleep 2; $p.Refresh(); $c2 = $p.TotalProcessorTime.TotalMilliseconds
  "cpu(one-core)%=$([math]::Round(($c2-$c1)/2000*100,3)) ws=$([math]::Round($p.WorkingSet64/1MB,2))MB pm=$([math]::Round($p.PrivateMemorySize64/1MB,2))MB"
}
```
