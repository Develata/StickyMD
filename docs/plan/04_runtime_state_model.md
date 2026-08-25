# 04_runtime_state_model.md - 运行时状态与权威模型

## Metadata

- `Layer`: Architecture
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-20
- `Scope`: 运行时状态结构、所有权与修改权、authority 划分、generation 语义、核心 invariant

---

## Purpose

定义 StickyMD 运行时所有状态的概念结构、谁拥有谁、谁能修改谁、
runtime authority 与 durable projection 的关系，以及 generation 语义。
使用 Rust-like 伪类型描述；**不是代码契约，不得据此直接生成实现**。

---

## Boundary

- 本章定义状态模型，不定义 UI 控件树、线程模型或具体库调用。
- 状态之间的流程编排见 `03_system_architecture.md` 调用链。
- 每个状态的失败路径细节见对应能力章节（05–09）。

---

## Owned Objects

`doc::text`、`doc::generation`、`preview::owned_ast`、`preview::render_tree`、
`math::display_list`、`asset::managed_image`、`asset::trash_entry`、
`config::runtime`、`window::placement`（权威与投影见下表）。

---

## Inputs

经 Instruction Interface 验证的 typed intent、启动载入/外部 reconcile 输入、带 generation
的后台 capability result。

## Outputs

原子提交后的 state delta、immutable snapshot、保存/预览/资产调度所需的 generation token，
以及 typed failure。

## State Changes

每类状态只能由所有权矩阵指定的 coordinator 或领域 mutation gateway 修改；失败必须保持
相关 authority 原子不变。后台结果在提交前必须经过 generation 或事务状态校验。

---

## AppState 总览

```rust
// 伪类型：概念结构
struct AppState {
    lifecycle:   LifecycleState,
    visibility:  VisibilityState,
    docking:     DockState,
    view_mode:   ViewMode,          // Source | Split | Preview
    document:    DocumentState,
    preview:     PreviewState,
    save:        SaveState,
    conflict:    ConflictState,
    recovery:    RecoveryState,
    assets:      AssetState,
    ime:         ImeState,
    config:      ConfigState,
    window:      WindowState,
}
```

---

## 所有权与修改权矩阵

| 状态 | 拥有者 | 谁能修改 | 谁不能修改 |
| --- | --- | --- | --- |
| DocumentState | 编辑核心（经 Instruction Interface → Flow Coordination） | 仅 edit intent、undo/redo、reconcile/recovery 流程 | Shell、Preview、Execution Domain 后台任务、外部文件事件 |
| PreviewState | PreviewCoordinator | 仅预览调度流程（提交经 generation 校验） | 用户输入直接修改、Preview 自身 |
| SaveState | SaveCoordinator | 仅保存调度与落盘回执 | Shell、Preview |
| ConflictState | ConflictCoordinator | 仅冲突检测与用户选择执行 | 保存流程（冲突期间 autosave 暂停） |
| RecoveryState | RecoveryCoordinator | 仅启动恢复流程与用户选择 | 运行期任何流程 |
| AssetState | AssetCoordinator | 仅资产事务（含 undo/redo 副作用） | Shell、Preview、GC 之外的任何直接文件操作 |
| WindowState / DockState | WindowDockCoordinator + LifecycleCoordinator | 仅窗口/dock 流程 | 编辑与保存流程 |
| ImeState | 编辑器后端（经 Shell 事件） | 仅 IME 事件处理 | 保存、预览流程 |
| ConfigState | 配置管理（经 Set* intent） | 仅 SetTheme/SetOpacity/SetAlwaysOnTop 等 intent 与启动载入 | 后台任务 |

---

<a id="documentstate"></a>
## DocumentState

```rust
struct DocumentState {
    text: StringTextStore,              // 内部 trait TextStore；v1 用 String
    generation: u64,
    saved_generation: u64,
    base_disk_hash: Option<Hash32>,
    dirty: bool,                        // derived: generation != saved_generation；非独立权威
    line_ending: LineEnding,            // CRLF | LF（保存时转换）
    undo: UndoManager,                  // max 256 entries 或 4 MiB
    managed_ref_counts: Map<ManagedAssetName, usize>,
}

trait TextStore {
    fn as_str(&self) -> &str;
    fn apply(&mut self, delta: &TextDelta) -> Result<(), EditError>;
    fn len_bytes(&self) -> usize;
}
```

- 内部文本统一为 **UTF-8 + `\n`**；保存时转换为记录的换行风格。
- `TextDelta.range` 必须落在 UTF-8 char boundary。
- 一次 IME commit / 一次图片粘贴 = 一个 delta。
- v1 使用 String 存储；若 1 MiB 性能 gate 不达标，可在不改上层 API 的前提下替换为 rope。**禁止在基准测试前提前引入 rope。**

### Mutation gateway

- UI 只能提交 expected generation、range、inserted text 与 cursor/edit metadata；
  `deleted` 必须由 DocumentState 从 canonical text 派生。
- 不公开 mutable text 引用；edit / undo / redo / reconcile / recovery replacement 是唯一 mutation gateway。
- stale expected generation、range 错误与 history apply 错误均 fail closed，且不产生部分 mutation。

<a id="document-snapshot"></a>
### Document Snapshot

`doc::snapshot` 是 `Arc<str> + generation + line ending` 的 immutable projection，供 worker
使用；它不是 authority，也不得持有 `&mut DocumentState`。

### Runtime authority

程序运行期间，DocumentState 是文档内容的**唯一权威**。
磁盘 `note.md` 是其 durable projection；外部磁盘变化是 External File Fact，
必须经 reconcile/conflict 流程才能进入 DocumentState。

---

## PreviewState

```rust
enum PreviewState {
    Empty,
    Clean     { generation: u64, document: Arc<LaidOutDocument> },
    Dirty     { generation: u64 },
    Scheduled { generation: u64, deadline: Instant },
    Rendering { generation: u64 },
    Failed    { generation: u64, error: PreviewError },
}
```

- Preview 永远是 DocumentState snapshot 的派生 projection，不得反写 source。
- `result.generation != document.generation` → 结果立即丢弃。

## SaveState

```rust
enum SaveState {
    Clean     { hash: Hash32 },
    Dirty     { generation: u64 },
    Scheduled { generation: u64, deadline: Instant },
    Saving    { generation: u64 },
    Conflict  { .. },                 // 与 ConflictState 联动
    Failed    { error: SaveError },
}
```

- `saved_generation` 只更新到实际落盘的 generation。
- 保存失败不静默：必须进入 Failed 并呈现给用户，内存文本保留。

## ConflictState

```rust
enum ConflictState {
    None,
    ExternalModified { autosave_paused: true },   // buffer dirty 时外部变化
    InvalidUtf8      { autosave_paused: true },   // 外部内容非合法 UTF-8
}
```

- 用户选择“载入外部”：丢弃未保存 buffer、载入、清空 undo、更新 hash、reconcile assets。
- 用户选择“保留本地”：原子覆盖外部文件、更新 hash、保留 undo、恢复 autosave。
- 冲突期间允许继续输入，但 autosave 暂停。

## RecoveryState

```rust
enum RecoveryState {
    None,
    Candidate { temp_content_valid: bool, temp_newer: bool },
}
```

- 用户选择前：不覆盖任何文件、暂停 autosave、内存只保留最小必要数据。

## AssetState

```rust
struct AssetState {
    managed_ref_counts: Map<ManagedAssetName, usize>,  // 镜像自 DocumentState 扫描
    trash: Set<ManagedAssetName>,
    pending_ops: Queue<AssetOp>,   // 单 I/O 串行；带 transaction_id / generation / expected_state
}
```

- 引用计数真相来自对 authoritative DocumentState 文本的保守扫描
  （managed 文件名 literal 存在即视为引用，宁保留不误删）。
- AssetEffect：`MoveToTrash` / `RestoreFromTrash` / `CreateManaged`，与文本同属一个 UndoEntry。

## WindowState / DockState

```rust
struct WindowState {
    placement: WindowPlacement,   // DIP size + monitor identity + ratio
    opacity: u8,                  // 40–100
    always_on_top: bool,
}

enum VisibilityState {
    HiddenToTray,
    Floating,
    DockedExpanded(DockEdge),
    DockedCollapsed(DockEdge),
    Animating { from, to, end, final_state },
}

struct DockState {
    edge: Option<DockEdge>,       // Left | Right | Top（无 Bottom）
    monitor_id: Option<MonitorId>,
    offset_ratio: f32,
    manually_hidden: bool,
    hover_revealed: bool,
    focus_guard: bool,            // 键盘焦点 / IME composition 期间为真
}
```

- 收起优先级：`Quit > ManualHide/Esc > ActiveDrag > Focused/IME composing > Conflict/Recovery 交互 > Auto-hide timers > Hover reveal`。
- 焦点或 IME composition 期间：**禁止自动收起**；鼠标临时离开不触发收起。

## ImeState

```rust
enum ImeState {
    Disabled,
    Enabled,
    Preediting { text: String, selection: Option<Range<usize>>, anchor: CursorSnapshot },
}
```

- preedit 不写入规范文档、不触发 autosave、不进 undo、不触发资产 reconcile。
- commit 一次性产生 TextDelta；cancel 后文档保持不变。

## ConfigState

```rust
struct ConfigState {
    version: u32,             // = 1
    theme: ThemeMode,         // light | system | dark
    opacity: u8,              // 40–100，默认 96
    content_zoom_percent: u16,// 50–300，默认 100；仅缩放内容投影
    split_scroll_sync: bool,  // 默认 true；仅控制 Split 语义滚动同步
    always_on_top: bool,
    view_mode: ViewMode,
    window: WindowConfig,     // width/height_dip、monitor_id、dock_edge、ratios
}
```

- 运行时 ConfigState 是配置权威；`config.toml` 是 durable projection。
- Content Zoom 不属于 DocumentState、WindowState 或任一 projection；其提交不推进 Document generation，
  不触发 Markdown 重新解析，也不缩放窗口 Shell/控件/边框。
- Split Scroll Sync 只是一项 Runtime Config 偏好；切换它不改变 Document generation、两侧已保存
  scroll position 或 Preview generation，也不把任一 projection 提升为 authority。
- 只在明确提交点写盘（原子替换）；未知字段忽略，缺字段用默认值；损坏则改名保留并以默认启动。

---

<a id="generation"></a>
## Generation Semantics（统一规则）

1. 每次 canonical 文本修改（edit / undo / redo / external reload / recovery replacement）
   使用 checked increment；溢出 fail closed，不允许 wrapping。
2. selection/caret、IME preedit、预览刷新、主题、窗口变化与 persisted acknowledgement
   不递增 generation。
3. 所有后台任务（preview、保存、资产扫描）接收带 generation 的快照。
4. 任务结果携带来源 generation；提交前必须校验 `result.generation == 当前 generation`
   （保存例外：允许合并保存最新 generation，但 saved_generation 只推进到实际落盘版本）。
5. 不匹配 → 结果直接丢弃，不产生任何副作用。

---

<a id="core-invariants"></a>
## 核心 Invariant

以下不变量在任何实现中必须恒成立：

```text
1.  Preview never becomes document authority.
2.  External disk change never mutates DocumentState without reconciliation.
3.  Managed asset GC never deletes a user asset.
4.  Stale preview generation never commits.
5.  Autosave cannot overwrite unresolved external conflict.
6.  IME preedit is not canonical document text.
7.  saved_generation never exceeds the generation actually persisted.
8.  No file write path exists outside atomic replace.
9.  No background task may hold a mutable reference to DocumentState.
10. window::placement 恢复后窗口必须完全位于至少一个可见工作区内。
```

---

## Failure Paths

- delta apply 失败（非 char boundary 等）：状态回滚，输入被拒绝并可见。
- 保存失败：SaveState::Failed，用户可见错误，内存文本不丢。
- 后台任务 panic/超限：结果丢弃，主流程不受影响（保护性限制见各能力章节）。
- 配置解析失败：默认配置启动，损坏文件改名保留。

## Configuration

见 ConfigState 定义；持久化细节见 `05_document_persistence.md`。

## Lifecycle

- 启动：解析 canonical Program Directory → 单实例 → Writable check → 载入 config →
  载入 note.md / recovery → asset reconciliation → 窗口与托盘。第二实例在 durable bootstrap 前退出。
- 退出：立即保存 → 等待资产事务 → 安全 GC → 保存配置 → 释放 mutex → 清理临时文件。

## Extension / Replacement Points

TextStore（String → rope）、EditorBackend（cosmic → RichEdit fallback，见 07）。

## Performance Critical Paths

按键 delta apply；generation 校验开销必须为 O(1)。

## Verification

- invariant 1–10 每一条都必须有对应单元测试或验收案例（见 coverage matrix）。
- property test：任意 Unicode delta 不破坏 UTF-8；undo/redo roundtrip。

## Non-Goals

- 不定义多文档状态。
- 不定义持久化 undo。
- 不定义协同/同步状态。
