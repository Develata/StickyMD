# 03_system_architecture.md - 系统架构：四层调用 + Object Plane

## Metadata

- `Layer`: Architecture
- `Status`: Approved Contract
- `Version`: 0.2.0
- `Last Review`: 2026-10-31
- `Scope`: StickyMD 主骨架：四层调用架构 + Object Plane、层间规则、coordinator 清单、核心调用链、共享 crate 与平台宿主的映射

---

## Purpose

将 StickyMD 的全部行为约束在宪法 5.x 的“四层架构 + 对象层”模型内，
保证壳核分离、单一真相源与失败路径可定义。

---

## Boundary

本章定义层间关系与调用链骨架；各层内部细节由后续章节持有：

- 文档持久化 → `05_document_persistence.md`
- Markdown/数学/预览 → `06_markdown_math_rendering.md`
- 编辑器/IME → `07_editor_and_ime.md`
- 资产/导出 → `08_assets_and_export.md`
- Windows 壳层 → `09_windows_shell.md`

---

## Owned Objects

本章不拥有具体运行时对象；它规定各层对 Object Plane 对象的访问路径与修改权限。

## Inputs

USER action、平台事件、timer、后台结果与 immutable object snapshot。

## Outputs

typed intent、协调后的 capability request、经校验的 state delta，以及供 Shell 呈现的结果。

## State Changes

只有 Instruction Interface 接受的 intent 才能进入 Flow Coordination；协调层调用领域能力
完成并校验 mutation 后，才能提交状态变化。Shell、adapter 与后台 worker 均不得绕过该入口。

---

## 总体结构

```text
User
  ↓
Interaction Shell        （第一层：转译 + 呈现）
  ↓
Instruction Interface    （第二层：action → intent → state delta → capability request）
  ↓
Flow Coordination        （第三层：顺序 / 冲突 / 失败 / 状态推进）
  ↓
Execution Domain         （第四层：具体能力执行，环境依赖经 adapter 进入）
  ↔
Object Plane             （对象层：最小数据元对象，不是第五调用层）
```

---

<a id="shared-crates"></a>
## 共享实现与平台宿主

USER 批准（2026-10-31，[提案](../report/2026-10-23-shared-core-thin-platform-shell-architecture.md)）：
共享实现面向三端集中维护，平台宿主只提供真实系统能力。crate 划分不取代四层结构，每个 crate 内仍按层分模块。

| crate | 拥有的职责 | 对应层 | 边界 |
| --- | --- | --- | --- |
| `stickymd-core` | 文本、编辑 delta、撤销重做、generation、资产引用等领域状态与纯计算 | Execution Domain 的领域能力 + Object Plane 类型 | 不依赖窗口、渲染、Capability Port 实现或磁盘执行 |
| `stickymd-render` | Markdown/数学投影、排版、绘制、命中与选择映射、布局缓存 | Execution Domain | 不决定保存、冲突或资产删除；环境读取经执行接口 |
| `stickymd-app`（Shared Application） | typed intent、应用会话、全部 coordinator、配置状态、Capability Port 定义、可共享执行逻辑（worker 调度与结果协议） | Instruction Interface + Flow Coordination + 共享 Execution | 不依赖 winit、softbuffer 或任何平台 API；不向壳暴露可变 DocumentState |
| `stickymd-desktop`（Shared Desktop） | 通用控件、Search 控件、选择与滚动交互、窗口事件翻译、视图呈现、软件帧缓冲接线 | Interaction Shell（共享部分） | 可使用 winit/softbuffer；不持有保存、冲突、GC、生命周期的业务裁决 |
| Platform Host（当前 `apps/stickymd-win`） | 启动接线、原生 adapter、资源/manifest、平台验证探针 | Shell 启动接线 + Execution Domain 原生 adapter | 实现 Capability Port 并上报 Execution Receipt；不复制应用规则 |

编译依赖方向（共享库不反向依赖平台宿主）：

```text
Platform Host ──→ stickymd-desktop ──→ stickymd-app ──→ stickymd-core
     │                   │                   │
     │                   └────→ stickymd-render ──→ stickymd-core
     └── 实现并注入 stickymd-app 定义的 Capability Port
```

规则：

1. 运行时调用链不变：Shell → Instruction Interface → Flow Coordination → Execution Domain。Platform Host 内的启动
   接线属于 Shell，原生 adapter 属于 Execution Domain；二者不能因同处一个 crate 而越层调用。
2. 共享 UI 仍属于 Interaction Shell：集中维护不使它成为文档权威，也不使界面操作获得直接写文件的权限。
3. Capability Port 按当前业务需要定义（存储、资产、本地图片与导出目标、剪贴板与 Shell launch、窗口与显示器、
   宿主/单实例/唤醒），不建立包罗所有系统调用的大接口，也不把每个文件操作包装成一个虚函数。
4. 每次请求的结果是 Execution Receipt，分别表达成功、执行失败、暂时不可用与 `Unsupported`，并携带判定过期所需的身份
   （身份要求及豁免按 `04` Generation 规则 6）。平台声明支持某能力不等于某次请求已经成功；失败时不得提交虚假的已保存/已隐藏状态。
5. 文件接口覆盖完整的安全操作边界：不能由共享层先检查路径、再由平台层凭字符串删除或移动文件。
6. 主线程限定的窗口、菜单与输入法能力不强加 `Send + Sync`；I/O worker 只获得允许跨线程传递的执行对象与不可变请求。
7. 共享调度使用注入的单调时间或可控时钟，沿用毫秒时间与有界队列语义；Platform Host 负责唤醒，不复制 debounce 与退出顺序。
8. 共享 crate 禁止 `unsafe`。第三方库可以持有经批准的平台后端；项目手写原生调用仍集中在 Platform Host 的 adapter。
9. 平台默认字体、快捷键映射与原生菜单约定属于宿主配置/翻译；Windows 现有默认值保持不变。

迁移（Phase 15 A–D，`11` Phase 15 节）期间，尚未迁出的代码仍在 `apps/stickymd-win`，但每批只保留一个生效实现，
必要转发不保存状态；本节是目标契约，未迁移部分按本节判定为待收敛的 implementation drift，而不是第二套权威。

---

<a id="interaction-shell"></a>
## 第一层：Interaction Shell

### 职责

- 窗口创建与呈现（Source / Preview / Split）。
- Keyboard / mouse / IME 事件捕获。
- Tray 呈现。
- Theme / opacity 控件呈现。
- 视觉选择、滚动、dock 动画呈现。
- 将平台事件转译为标准指令，将状态投影呈现给用户。

### 唯一职责

```text
转译 + 呈现
```

### 禁止

- 文件写入决策。
- asset GC 判断。
- Markdown 业务判断。
- save conflict 决策。
- lifecycle business state 决策。
- 直接调用 Execution Domain（必须经过第二、三层）。

---

<a id="instruction-interface"></a>
## 第二层：Instruction Interface

### 职责

把 UI action 转为 typed intent，校验合法性，映射为状态变化请求：

```text
action → intent
intent → state delta
state delta → capability requests
```

### v1 预期 intent 类别（contract，非代码）

```text
EditText
Undo
Redo
SaveNow
Export
SetViewMode
SetTheme
SetOpacity
SetAlwaysOnTop
RequestCollapse
RequestShow
RequestQuit
ResolveFileConflict
PasteClipboard
```

### 禁止

- 直接修改 DocumentState 或触碰文件系统。
- 合并或吞掉 intent（每个 intent 必须有明确的状态结果或拒绝理由）。

---

<a id="flow-coordination"></a>
## 第三层：Flow Coordination

### Coordinator 清单

```text
SaveCoordinator          保存调度：debounce、立即保存点、失败传播
PreviewCoordinator       预览调度：debounce、generation 校验、stale 丢弃
AssetCoordinator         图片事务：引用扫描、move/restore、GC、undo 副作用
ConflictCoordinator      外部修改冲突：检测、banner、用户选择执行
RecoveryCoordinator      启动恢复：temp 检测、用户选择、恢复执行
WindowDockCoordinator    停靠：吸附、收起/展开、hover/失焦计时、typing guard
LifecycleCoordinator     生命周期：启动序列、隐藏到托盘、退出清理、单实例
```

### 职责

- 任务拆分与调用顺序规划。
- 前后依赖协调（例如：图片写入成功才允许插入引用 delta）。
- 状态推进与错误回滚/中止。
- 后台任务分发与结果接收（generation 校验在此层完成裁决）。

### 禁止

- 绕过 Execution Domain 直接操作 filesystem、parser、剪贴板等具体能力。
- 直接接触 Object Plane 对象做持久化（宪法 5.4：第三层不直接接触对象层）。

---

## 第四层：Execution Domain

### v1 执行能力

```text
Markdown parsing（Comrak）
Math parsing/layout（RaTeX）
Text shaping（cosmic-text）
Rasterization（tiny-skia）
File I/O + Atomic replace
Asset move/delete
Clipboard read/write
File watch
Monitor query
Window platform adaptation（winit + Win32 adapter）
Shell launch（链接/文件交给系统）
Config serialization（TOML）
```

### 规则

- 所有环境依赖通过 **adapter** 进入：Win32、剪贴板、显示器枚举、文件系统原子替换。
- 平台无关 crate（含 Shared Application、Shared Desktop）禁止 `unsafe`；平台代码集中在 Platform Host 的 adapter 目录。
- 能力是被动的：被 Flow Coordination 请求后才执行，不自行决定业务流程。

---

<a id="object-plane"></a>
## Object Plane

对象层定义系统实际操作的最小数据元对象（宪法 5.6）。它不是第五调用层。

```text
doc::text                DocumentState 中的规范文本（UTF-8 + \n）
doc::snapshot            供后台任务使用的只读文本快照（带 generation）
doc::generation          文档版本号
preview::owned_ast       Comrak Arena 转换后的自有 AST
preview::render_tree     布局后的可绘制文档树
math::display_list       RaTeX 排版输出
asset::managed_image     managed 图片文件及其引用计数
asset::trash_entry       .trash 中的逻辑删除项
config::runtime          运行时配置状态
window::placement        窗口位置/尺寸/停靠（DIP + ratio + monitor identity）
file::note_md            磁盘上的 note.md（durable projection）
file::config_toml        磁盘上的 config.toml（durable projection）
```

每个对象的权威与生命周期在 `04_runtime_state_model.md` 中定义。

---

## 禁止的跨层关系

1. Shell 直接调用 Execution Domain（必须经 Instruction Interface + Flow Coordination）。
2. Flow Coordination 直接读写 Object Plane 持久对象（必须经 Execution Domain）。
3. Execution Domain 反向调用 Shell（结果只能经事件/回执上行）。
4. 任何层绕过 ConflictCoordinator 直接写 `note.md`。
5. Shell 或 Execution Domain 自行决定 asset 删除（只有 AssetCoordinator 可裁决）。
6. Preview 结果反写 DocumentState。

---

## 核心调用链（5 例）

### 1. 输入文字

```text
keyboard/IME commit
→ Shell: winit 输入事件
→ Instruction Interface: EditText intent（expected generation + range + inserted + cursor/meta）
→ Flow Coordination: 调用 document capability；成功后调度保存/预览/managed 引用扫描
→ Execution Domain: DocumentState 从 canonical text 派生 deleted text，校验并原子 apply delta
→ Object Plane: doc::text 更新，doc::generation +1
```

- **Input**：IME commit 文本或键盘编辑动作。
- **State change**：DocumentState 文本与 generation；SaveState → Dirty/Scheduled；PreviewState → Dirty；undo entry 追加。
- **Failure**：stale generation、越界或非 char boundary → 拒绝并保持 document/history/generation
  全部不变；undo 超限 → 按有界策略淘汰或不记录超大 entry，但 canonical edit 仍成功。
- **Authority**：DocumentState 是文本权威；IME preedit 不是文档内容。
- **Output**：重绘请求（dirty 区域）；保存/预览调度被触发。

### 2. Autosave

```text
Document dirty（generation G）
→ SaveCoordinator: 调度（debounce 650 ms / 立即保存点）
→ Execution Domain: atomic persistence（temp → flush → atomic replace）
→ Object Plane: file::note_md 更新；saved_generation = G
```

- **Input**：Dirty 状态 + 待保存 generation 的最新文本快照。
- **State change**：SaveState: Scheduled → Saving → Clean；`last_saved_hash` 更新；write token 生效（供 watcher 识别自身写入）。
- **Failure**：磁盘写入失败 → SaveState = Failed，明确报错，保留内存文本，不静默退出；保存期间又有新修改 → 完成当前保存后立即保存最新 generation。
- **Authority**：saved_generation 只由实际落盘回执推进。
- **Output**：磁盘 note.md 原子更新；UI 保存状态指示。

### 3. Preview 构建

```text
dirty generation G
→ PreviewCoordinator: debounce 1000 ms（split）/ 立即（纯 preview）
→ Execution Domain: Comrak parse → owned AST → RaTeX math → layout（后台线程）
→ Object Plane: preview::owned_ast / preview::render_tree
→ PreviewCoordinator: generation 校验
→ Shell: 原子替换呈现
```

- **Input**：doc::snapshot（带 generation）。
- **State change**：PreviewState: Dirty → Scheduled → Rendering → Clean(G)；失败则 Failed(G)。
- **Failure**：结果 generation 落后 → 直接丢弃；单个公式错误 → 该公式显示原文 + 错误提示，不整体失败；超限（公式大小/数量）→ 显示原文 + 提示。
- **Authority**：DocumentState；Preview 永远只是 projection。
- **Output**：新的 LaidOutDocument 原子替换旧预览。

### 4. 图片粘贴

```text
clipboard 粘贴动作
→ Shell: 粘贴事件
→ Instruction Interface: PasteClipboard intent
→ Flow Coordination: AssetCoordinator（读取剪贴板 → 定编码 → SHA-256 命名）
→ Execution Domain: image persistence（写 images/，去重/trash 恢复）
→ 写入成功后：EditText intent 插入 Markdown 引用
→ Object Plane: asset::managed_image 建立，引用计数 = 1，与文本同属一个 UndoEntry
```

- **Input**：剪贴板内容（CF_HDROP 文件列表 / 编码图 / bitmap / 文本，按优先级）。
- **State change**：images/ 新增 managed 文件；doc::text 插入引用；单一 UndoEntry 同时包含文本与 AssetEffect。
- **Failure**：写入失败 → 不插入引用，显示错误，剪贴板文本不受影响；格式不可解码/超限 → 显示占位符提示，不写文件。
- **Authority**：引用状态的真相来自 DocumentState；文件是否存在只是存储事实。
- **Output**：文档中出现 `![](images/stickymd-<hash>.<ext>)`。

### 5. Dock 隐藏（失焦收起）

```text
focus state 变化 / 计时器到期
→ Shell: 焦点与 hover 事件
→ Instruction Interface / timer: RequestCollapse intent
→ Flow Coordination: WindowDockCoordinator（检查 guard：焦点/IME/拖动/弹出/冲突）
→ Execution Domain: platform adapter（窗口几何 + 动画）
→ Object Plane: window::placement 更新为 DockedCollapsed(edge)
```

- **Input**：失焦事件 + 700 ms 计时，或 Esc / 手动收起按钮。
- **State change**：WindowState: DockedExpanded → Animating → DockedCollapsed；config 的 dock 状态投影更新。
- **Failure**：guard 命中（正在输入/IME composition/拖动/有冲突 banner）→ 取消收起；动画被打断 → 以最终目标状态收敛，不卡在中间态。
- **Authority**：WindowState；typing guard 规则高于自动收起计时器。
- **Output**：窗口收起为 3 DIP 感应条。

---

## Failure Paths

- 任何后台结果：generation 不匹配即丢弃，不得覆盖新状态。
- 任何文件操作失败：错误必须上行到用户可见层，不得静默吞掉。
- 任何 coordinator 异常中止：DocumentState 与磁盘文件保持一致性或进入显式 Conflict/Recovery 状态，绝不允许半写文件。

## Configuration

本章自身无运行时配置项；窗口与 dock 相关配置见 `09_windows_shell.md`，保存相关见 `05_document_persistence.md`。

## Lifecycle

架构骨架生命周期 = 项目生命周期；修改需走宪法 10.1.3 审批。

## Extension / Replacement Points

- 编辑器后端：cosmic-text 后端与 RichEdit fallback 为平级实现（见 `07_editor_and_ime.md`）。
- 文本存储：String 实现可替换为 rope，接口不变（见 `04_runtime_state_model.md`）。
- 数学渲染桥接：上游 API 或项目内薄 painter（见 `06_markdown_math_rendering.md`）。
- 平台 adapter：Windows 实现可整体替换，核心层不变；新的 Platform Host 实现同一组 Capability Port（见共享实现节），
  其行为差异须按 `02` 平台节另行批准。

## Performance Critical Paths

- 按键 → delta apply → 重绘（UI 线程，必须最轻）。
- Preview 全量 parse/layout（后台，debounce 保护）。
- 原子保存（I/O worker，不阻塞 UI）。

## Verification

- 每条调用链必须可在代码中以“单一入口、逐层向下”被 review 验证。
- 跨层调用在 CI review 与 `plan_ref` 审查中拒绝。
- 共享实现节的依赖方向、`unsafe` 禁令与平台类型禁令由治理检查覆盖，检查路径随每批迁移同步更新（`11` Phase 15 节）。

## Non-Goals

- 不定义任何 UI 框架或控件树细节。
- 不规定线程数量与调度实现（由后续实现阶段在宪法内决定）。
