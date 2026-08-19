# StickyMD 架构概览

> 这是可读投影。权威契约在 [`docs/plan/`](../plan/AGENTS.md)；
> 术语见 [`01_terminology.md`](../plan/01_terminology.md)。

---

## 一句话

StickyMD 是 Windows 11 x64 上的单张 Markdown 草稿纸：
一个程序目录 = 一张便签，永远只编辑 `note/note.md`。

## 核心模型

```text
程序目录（身份） → note/note.md（唯一文档）
运行时 DocumentState 是唯一权威；
磁盘文件、Preview、UI 都是它的投影。
```

## 四层调用 + Object Plane

```text
User
  ↓
Interaction Shell        窗口/渲染/输入捕获（只转译 + 呈现）
  ↓
Instruction Interface    action → typed intent
  ↓
Flow Coordination        Save / Preview / Asset / Conflict / Recovery / Dock / Lifecycle
  ↓
Execution Domain         Comrak、RaTeX、cosmic-text、tiny-skia、文件 I/O、剪贴板、平台 adapter
  ↔
Object Plane             doc::text、preview::render_tree、asset::managed_image、window::placement …
```

详见 [`03_system_architecture.md`](../plan/03_system_architecture.md)。

## 关键安全承诺

- 保存永远原子替换，不产生半写文件；崩溃后可恢复。
- 外部修改遇到未保存内容时显式冲突，绝不偷偷覆盖。
- 只自动删除程序自己创建且确认无引用的图片；用户文件永不自动删除。
- 无网络：远程图片不下载；raw HTML 不执行。
- 预览是只读投影，永远不会反写文档。

## 技术方向（已批准，版本待技术验证阶段锁定）

```text
Rust · winit · cosmic-text · tiny-skia · softbuffer · Comrak · RaTeX · 少量 Win32 adapter
禁止：WebView / Electron / Tauri / JS / 通用 async runtime / 数据库 / 网络
```

## 文档导航

| 想了解 | 看这里 |
| --- | --- |
| 工程最高约束 | [工程宪法](../plan/00_engineering_constitution.md) |
| 状态与权威 | [04_runtime_state_model.md](../plan/04_runtime_state_model.md) |
| 保存与冲突 | [05_document_persistence.md](../plan/05_document_persistence.md) |
| Markdown/数学 | [06_markdown_math_rendering.md](../plan/06_markdown_math_rendering.md) |
| 编辑器与输入法 | [07_editor_and_ime.md](../plan/07_editor_and_ime.md) |
| 图片与导出 | [08_assets_and_export.md](../plan/08_assets_and_export.md) |
| 窗口与托盘 | [09_windows_shell.md](../plan/09_windows_shell.md) |
| 用户行为投影 | [00_v1_product_behavior.md](../features/00_v1_product_behavior.md) |
| 验收案例 | [00_v1_acceptance.md](../acceptance-cases/00_v1_acceptance.md) |
