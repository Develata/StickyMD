# phase-01-windows-api-baseline.md - Phase 1 Windows API 基线

- `Date`: 2026-08-19
- `Type`: 阶段基线（Windows API baseline）
- `Status`: Completed（证据来自 1B window / 1C text / 1E persistence spike）

## 背景

plan_ref: docs/plan/09_windows_shell.md（“winit 优先，仅 winit 不足时才进入 Win32 层”）。
本报告记录 Phase 1 实际触达的 Win32 API、**为何 winit 0.30 不足**、以及 unsafe 的
隔离方式。生产 adapter 的取舍在本报告基础上于 Phase 1 评审后裁定。

## 1. winit 覆盖范围（优先层）

以下能力由 winit 0.30.13 直接提供，spike 已验证，**无需进入 Win32**：

- 事件循环、`ControlFlow::Wait`、窗口创建、键盘/修饰键事件。
- `WindowEvent::Ime`（Ime::Enabled / Preedit / Commit / Disabled）——IME 事件可达。
- resize / scale_factor / redraw 调度、窗口句柄经 rwh 0.6.2 暴露。

## 2. 必须进入 Win32 的能力（winit 0.30 不足，已实测）

| 能力 | API | 为何 winit 不足 | spike |
| --- | --- | --- | --- |
| 整窗不透明度 | `SetLayeredWindowAttributes`（GWL_EXSTYLE \| WS_EX_LAYERED + LWA_ALPHA） | winit 0.30 无整窗 alpha | 1B |
| Win11 圆角 | `DwmSetWindowAttribute`（DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND=2） | winit 0.30 无圆角偏好 | 1B |
| 剪贴板读写 | `OpenClipboard`/`GetClipboardData`/`SetClipboardData`/`GlobalAlloc`/`GlobalLock`（CF_UNICODETEXT） | winit 0.30 无剪贴板 | 1C |
| 目录规范路径 | `GetFinalPathNameByHandleW` | std 无 reparse/junction 解析 | 1E |
| 单实例 | `CreateMutexW` + `GetLastError()==ERROR_ALREADY_EXISTS` | 无跨平台等价 | 1E |
| 激活事件 | `CreateEventW`/`OpenEventW`/`SetEvent`/`WaitForSingleObject` | 第二实例唤醒第一实例 | 1E |
| 持久化刷新 | `FlushFileBuffers` | std `sync_all` 底层即此调用，此处显式验证 | 1E |
| 原子替换 | `ReplaceFileW`（主）/ `MoveFileExW`（REPLACE_EXISTING\|WRITE_THROUGH，回退） | std `rename` 在目标存在时语义/原子性不足 | 1E |

## 3. 呈现链路关键 API（softbuffer 0.4 + rwh 0.6.2，非 Win32 但易踩坑）

- softbuffer 0.4 与 0.3 API 不同：`Context::new(display)` → `Surface::new(&context,
  window)` → `surface.resize(w,h)` → `surface.buffer_mut()` → `buffer.present()`。
- rwh 0.6.2 的 `DisplayHandle<'a>` / `WindowHandle<'a>` 为**借用类型且无 `Arc`/`Rc`
  实现**。spike 用本地 `WinRef(Arc<Window>)` newtype 重新实现
  `HasWindowHandle`/`HasDisplayHandle` 解决所有权问题——生产可沿用此模式。

## 4. unsafe 隔离与 SAFETY 契约

- 所有 unsafe 均集中在各 spike 的独立 adapter 模块（`win32.rs` / `win32_clipboard.rs`），
  模块顶部注明“本模块是唯一触碰 Win32 之处”。
- 每个 unsafe fn 带 `# Safety` / SAFETY 契约注释（句柄有效性、配对 open/close、
  剪贴板在成功 SetClipboardData 后接管句柄所有权等）。
- 句柄用 RAII guard（`OwnedHandle`，Drop 时 `CloseHandle`）防止泄漏（1E）。

## 5. 运行时验证摘要

- 1B：不透明度 + 圆角可同时生效，无渲染异常；启动属性应用日志确认。
- 1C：剪贴板 set/get round-trip 成功（进程内自检）。
- 1E：单实例第二实例检测 + 激活事件信号跨进程成功；原子替换后无 temp 残留。

## Resolution

生产 adapter 设计（继续手写薄 Win32 vs. 引入成熟 crate 如 arboard 负责剪贴板）在
Phase 1 评审后裁定。本基线证明「winit 优先 + 少量 Win32 薄适配」路线可行且 unsafe
可被有效隔离。交互式 IME 组合输入、真实 DPI/多显示器为人工项，见 spike 报告 NOT TESTED。
