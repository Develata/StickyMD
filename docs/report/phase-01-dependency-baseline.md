# phase-01-dependency-baseline.md - Phase 1 依赖基线与禁用项审计

- `Date`: 2026-08-19
- `Type`: 阶段基线（dependency baseline）
- `Status`: Completed（证据来自 Phase 1A–1E 各 spike 的 Cargo.lock 与 cargo tree）

## 背景

Phase 1 以独立实验 crate 验证冻结技术栈。本报告记录**实际解析到的版本**（取自各
spike `Cargo.lock`），并对「无浏览器运行时 / 无 WebView / 无 JS 引擎」做禁用项审计。
报告不是契约；版本冻结的最终裁定以 `docs/plan` 为准。

## 1. 冻结依赖 → 实际解析版本

| 依赖 | 声明 | 解析版本 | 出现于 | 用途 |
| --- | --- | --- | --- | --- |
| winit | 0.30.13 | **0.30.13** | window / text | 事件循环 / 窗口 / 输入 / IME |
| softbuffer | 0.4.8 | **0.4.8** | window / text | CPU 帧缓冲呈现 |
| tiny-skia | 0.12.0 | **0.12.0** | window（直接） | 2D 光栅化 |
| cosmic-text | 0.19.0 | **0.19.0** | text | 文本塑形 / 布局 / 绘制 |
| raw-window-handle | 0.6.2 | **0.6.2** | window / text | 原生句柄抽象 |
| windows | 0.62.2 | **0.62.2** | window / text / persistence | Win32 适配 |
| comrak | 0.54.0 | **0.54.0** | markdown | CommonMark+GFM+math 解析 |
| ratex-parser | 0.1.14 | **0.1.14** | markdown | 数学解析 |
| ratex-layout | 0.1.14 | **0.1.14** | markdown | 数学排版 |
| ratex-render | 0.1.14 | **0.1.14**（`embed-fonts`） | markdown | 数学光栅化（PNG） |
| sha2 | 0.10 | **0.10.9** | persistence | 目录身份 / 磁盘哈希 |

### 1.1 需注意的次级版本共存

- `tiny-skia 0.11.4` 也出现在 window/text 的 lockfile 中，但 `cargo tree -i
  tiny-skia@0.11.4 --target x86_64-pc-windows-msvc` 返回空——它是**平台门控的传递
  依赖**（cosmic-text 等内部使用），**不进入 Windows 构建图**。window spike 实际渲染
  路径用的是直接的 `tiny-skia 0.12.0`。
- 结论：无版本冲突进入 Windows 目标；两版本共存仅为 lockfile 记录。

## 2. 禁用项审计（no browser runtime / no WebView / no JS）

plan_ref: docs/plan/10_performance_reliability.md（第 38 行：无浏览器运行时）。

### 2.1 扫描方法

```powershell
# 全 lockfile 关键字扫描（wry/tao/webview/web-sys/js-sys/wasm-bindgen/v8/cef/egui/iced/...）
grep -rniE "wry|tao|webview|web-sys|js-sys|wasm-bindgen|v8|cef|egui|iced|slint|gpui|dioxus|makepad" --include=Cargo.lock .
# 目标过滤树（只看真正为 Windows 编译的图）
cargo tree --target x86_64-pc-windows-msvc | grep -iE "wasm-bindgen|webview|wry|web-sys|js-sys"
```

### 2.2 结果

- lockfile 中命中 `wasm-bindgen*`（window / text / markdown 的 winit 传递依赖）。
- **但** 以 `--target x86_64-pc-windows-msvc` 过滤后，四个 spike 的构建图中
  `wasm-bindgen` / `web-sys` / `js-sys` / WebView / wry / tao **均为 0 命中**。
- 即：`wasm-bindgen` 仅为 winit 的 wasm32 平台条件依赖，**不会在 Windows 上编译**。
- 无任何 WebView/JS 引擎/浏览器运行时 crate 进入实际构建。

### 2.3 判定

**PASS**：无禁用 GUI/浏览器运行时 crate 被实际编译；技术栈保持「纯 Rust 自绘 + Win32
薄适配」。`wasm-bindgen` 的存在属 lockfile 平台条件项，不构成风险，已在 §1.1 说明。

## 3. 生产 workspace 现状

- 生产 workspace（`crates/stickymd-core` / `crates/stickymd-render` /
  `apps/stickymd-win`）当前仅依赖 `thiserror 2.0.20`（core），其余为空骨架。
- `experiments/phase-01/*` 各自声明空 `[workspace]`，**不属于生产 workspace**。
- 已验证：`cargo build --workspace`（生产）成功，`cargo tree`（生产）不含任何实验
  依赖 → **删除 experiments/ 不影响生产构建**（可删除性成立）。

## Resolution

待 Phase 1 技术评审与 USER 批准后，可据此把冻结版本写入生产 workspace 依赖；
在此之前生产 workspace 保持最小依赖。
