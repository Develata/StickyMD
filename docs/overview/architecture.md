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

## 当前实现切片

Phase 2 已建立 `DocumentState`、checked generation、UTF-8 TextDelta、不可变 snapshot
和有界 Undo/Redo。Phase 3 开发壳实现的实际调用链为：

```text
winit event
  → typed AppIntent
  → EditorCoordinator（唯一 DocumentState mutation gateway）
  → AppEffect + generation-tagged TextDelta
  → cosmic-text SourceProjection
  → tiny-skia / softbuffer
```

IME preedit 只存在于 editor session 和视觉投影，commit 才产生一次 canonical delta。
当前开发壳不持久化，真实微软拼音/微信输入法人工验收仍为 NOT TESTED。

## 技术方向（已批准；按已实现切片分别验证）

```text
Rust · winit · cosmic-text · tiny-skia · softbuffer · Comrak · RaTeX · 少量 Win32 adapter
禁止：WebView / Electron / Tauri / JS / 通用 async runtime / 数据库 / 网络
```

当前生产切片使用前五项中的 Source/窗口链路；Comrak/RaTeX 仍只处于独立 spike。
`arrayref` yanked 风险已由刷新后的 registry index 与 fresh-lock 复核证伪；RaTeX 的
剩余条件风险是正式热路径 painter/API 尚未验证，不得把 PNG spike 等同于生产集成。

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
