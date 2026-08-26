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

Phase 11/11-B 已完成 v0.1.0 产品实现收敛；Phase 12 只建立 tools-only exact-artifact
qualification。运行时权威与依赖图不变：candidate、automated、manual、remote 与 downloaded
artifact receipts 写入 ignored `dist/evidence/`，不能反向成为产品状态或架构权威。

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
Phase 4 已把该调用链接入有界 I/O worker、650 ms autosave、guarded atomic publish、
recovery 与外部文件 reconciliation；磁盘仍只是 durable representation，不能绕过
`DocumentState` 成为运行时文本权威。Phase 5 新增的预览链为：

```text
DocumentSnapshot
  → lazy bounded Preview worker
  → transient Comrak Arena
  → OwnedDocumentTree → RenderTree
  → cosmic-text layout → tiny-skia viewport frame
```

预览选择是只读 clipboard projection；链接必须经 typed intent 与 coordinator 校验后才到
Windows Shell adapter。Preview/公式资源矩阵已由 Rust smoke CLI 自动化；真实 Preview
视觉、同进程首次公式内存增量以及微软拼音/微信输入法人工验收仍为 NOT TESTED。

Phase 6 在同一个单线程 Preview worker 内加入：

```text
Comrak math node（delimiter authority）
  → delimiter-free literal
  → RaTeX parse + layout + DisplayList
  → bounded layout/raster/outline caches
  → thin native tiny-skia painter
  → atomic selectable formula object
```

公式 raster 是可释放 projection；`DocumentState` 与原始 delimiter 文本仍是唯一复制/保存
来源。错误公式显示原文与错误装饰，不修改 source，也不阻断同文档其他 block。

Phase 7 在同一个 authority 模型下增加两条受限链路：

```text
Clipboard
  → Asset paste intent
  → single I/O worker preparation
  → content-addressed Managed Asset Store
  → generation-checked DocumentState reference
  → Preview local-image resolver

DocumentState canonical text
  → conservative Asset Reference Tracker
  → desired images/.trash state
  → ownership-proven I/O reconciliation
```

自动 move/delete 必须同时证明 canonical managed 目录、严格文件名、普通非 reparse 文件与
实际内容 SHA-256 前缀。物理删除还要求 durable note 指纹与稳定句柄匹配，并以 durable 与
runtime 引用并集建立 safe boundary；否则只做可逆整理并延后删除。Preview/cache 不参与
GC authority。图片 cache 把 layout 仍持有的 raster 计入 16 MiB live budget，不能通过移除
map entry 隐藏 `Arc` 像素内存。远程图片仍只显示 placeholder，
不会发起网络请求。导出从当前 immutable snapshot 出发，只重写 Comrak 识别的 image node
source range，复制本地资源后最后原子发布 Markdown；它永远不切换工作文档。

## 技术方向（已批准；按已实现切片分别验证）

```text
Rust · winit · cosmic-text · tiny-skia · softbuffer · Comrak · RaTeX · 少量 Win32 adapter
禁止：WebView / Electron / Tauri / JS / 通用 async runtime / 数据库 / 网络
```

当前生产切片已使用 Comrak 建立 owned semantic/native Preview，使用 RaTeX 0.1.14
parser/layout/font crates完成 native math projection。项目采用审计过的薄 DisplayList painter；
`ratex-render`/PNG renderer 不进入生产依赖图。确定性 raster golden 与六场景资源矩阵已
自动化；真实 DPI/主题视觉与同进程首次公式内存增量仍是人工 `NOT TESTED` 条件，不能从
headless 测试推断完成。Phase 7 的 managed image、bounded decode/cache 和 portable export
同样由 Rust smoke CLI 持有；无图片/懒加载/4K/饱和 cache 的进程资源矩阵也由 CLI 运行。
真实 Windows 图片来源、视觉和 native dialog 仍明确
`NOT TESTED`。

Phase 8 将桌面窗口行为收口为一个平台无关 reducer：

```text
Window intent + monotonic time + platform facts
  → WindowShellState（visibility / dock / guards / lifecycle）
  → typed WindowEffect
  → winit / thin Win32 / tray adapters
```

`WindowShellState` 是运行时窗口与 Dock 生命周期权威；`ConfigCoordinator` 是已提交偏好的
唯一 mutable authority，并用 monotonic revision 合并写入。窗口、托盘和 Win32 只执行 effect，
不能读写 `DocumentState`。Close 在 dirty 时先冻结输入并保存最新 generation，再隐藏；Tray Quit
依次等待资产事务、最新 note save、安全 GC 与 config acknowledgement。PerMonitorV2 manifest、
CCD stable display identity、signed `rcWork` geometry 和 40–100 整窗 alpha 已接入。真实托盘菜单、
物理多屏拓扑、混合 DPI、IME 与视觉品质仍以 Phase 8 人工矩阵中的 `NOT TESTED` 为准。

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
