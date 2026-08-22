# 07_editor_and_ime.md - 源码编辑器与输入法合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-20
- `Scope`: Source 编辑器职责、IME preedit/commit 语义、字体 run、undo 分组、RichEdit fallback 治理

---

## Purpose

定义源码编辑区的行为契约与输入法正确性要求。中文输入法体验是 v1 一级需求。

## Boundary

- 编辑器后端只产生/消费 TextDelta 与选区状态；文本权威始终是 DocumentState。
- 本合约不覆盖 Preview 的选择/复制（见 `06`）。

## Owned Objects

`doc::text`（经 delta 修改）、caret/selection/preedit 呈现状态（非权威）。

---

<a id="source-editor"></a>
## Source 编辑器职责

- cosmic-text 布局 + 自绘 caret / selection / preedit。
- viewport 滚动（垂直；水平按需）。
- 常规快捷键：编辑、Ctrl+C/X/V、Ctrl+Insert/Shift+Delete/Shift+Insert、
  Ctrl+Z / Ctrl+Y、Ctrl+S、Ctrl+Shift+S。传统剪贴板快捷键必须映射到与常规快捷键
  完全相同的 typed intents；Shift+Insert 必须保留文本/图片剪贴板的同一优先级路径。
- 剪贴板文本（文件列表剪贴板由平台层补充）。
- 字符/脚本级字体 run（见下）。
- 向 Flow Coordination 发出 dirty/autosave 事件。
- 不做完整语法高亮；Markdown marker 不额外着色；当前行可有极轻背景提示。
- Source 基准字号 16 DIP、基准行高 1.55；统一乘以全局 Content Zoom（50–300%）。
  不提供独立 Source 字号、行高或字体配置。
- caret、selection、IME preedit 必须明显可见。

<a id="font-runs"></a>
## 字体 Run 规则

| 内容 | 首选 | fallback |
| --- | --- | --- |
| 中文/CJK 正文 | 仿宋_GB2312 | 仿宋 / FangSong / 系统 CJK |
| Latin 正文 | Times New Roman | 系统 serif |
| 代码 | Consolas | 系统 monospace |
| 数学 | RaTeX KaTeX fonts | RaTeX 内置 fallback |
| Emoji/特殊字符 | 系统 fallback | 系统 emoji/CJK fallback |

- 按字符脚本分段形成多个 run；Markdown 标点跟随相邻正文 run。
- 示例：`这是 Rust 的 trait 示例` → 仿宋 / Times / 仿宋 / Times / 仿宋。

---

<a id="ime-semantics"></a>
## IME 语义

### preedit vs commit

| 项 | preedit（composition 中） | commit |
| --- | --- | --- |
| 进入规范文档 | 否 | 是（一次性 TextDelta） |
| 触发 autosave | 否 | 是 |
| 进入 undo | 否 | 是（整次提交一个 undo entry） |
| 触发资产 reconcile | 否 | 是 |
| 呈现 | 带下划线的临时 run | 正常文本 |
| cancel | 文档保持不变 | — |

- 候选框位置必须通过当前 caret 屏幕坐标更新：`window.set_ime_cursor_area(...)`。
- IME composition 期间：窗口**绝不自动收起**（typing guard，见 `09`）。

### 一级验收输入法

微软拼音、微信输入法。验证项至少包括：

1. 中文连续输入。
2. 中英文混输。
3. 候选框位于 caret 附近。
4. selection 状态下开始 composition。
5. composition 中按左右键。
6. composition 中按 Backspace。
7. commit 后一次 Ctrl+Z 撤销整次提交。
8. composition 取消不污染 undo。
9. 高 DPI 候选框位置正确。
10. 分栏 / 源码 / 吸附展开后都可输入。
11. 整窗透明度 40–100 时输入正常。
12. 失焦/重新聚焦后 composition 状态正确。
13. 输入期间绝不自动收起。

---

<a id="undo-grouping"></a>
## Undo 分组

### 范围与限制

- 仅当前进程；重启清空；不写磁盘；不与 autosave 绑定。
- 最多 256 entries 或 4 MiB undo memory，先达到者淘汰最老 entry。

### 合并条件（连续输入可合并为一条）

相邻位置 + 同一输入类型 + 间隔 < 750 ms + 中间无 selection 替换、
无换行、无粘贴、无 IME commit。

### 必须独立成 entry

IME commit、粘贴、图片粘贴、删除 selection、Enter。
外部 reload 不进 undo（清空 undo）；程序化恢复不进普通 undo。

### 图片联动

图片引用变化的 AssetEffect 与文本 delta 同属一个 UndoEntry：
Ctrl+Z 恢复文本并把图片从 `.trash` 恢复；Ctrl+Y 重新移入 `.trash`（见 `08`）。

---

## EditorBackend 抽象与 RichEdit Fallback 治理

```text
trait EditorBackend {
    set_text / apply_delta / selection / set_selection / handle_event / draw
}
实现：CosmicEditorBackend（默认）、RichEditBackend（受控 fallback）
```

### RichEdit fallback 的定位

RichEdit fallback 是 **approved contingency，not default architecture**。

启用审批条件（全部满足才允许）：

1. 已完成至少两轮纯 Rust IME 修复。
2. 微软拼音或微信输入法仍存在阻塞性问题。
3. 问题有可复现步骤。
4. `docs/report/DESIGN_RISK_IME.md` 已记录。
5. fallback 被 feature flag 隔离：`richedit-fallback`。

### fallback 启用后仍必须保持 Rust 实现

DocumentState、Undo/Redo 外层事务、Markdown、数学、Preview、文件系统、
图片、窗口、托盘、Docking。RichEdit 只负责源码输入区。

---

<a id="selection-caret"></a>
## Selection / Caret

selection/caret 是 Editor Session 的非权威 byte-position projection；移动它们不递增
Document generation。grapheme/visual navigation 在 editor 层计算，DocumentState 只校验
最终 mutation range 的 UTF-8 char boundary。

## Inputs

键盘/IME/鼠标事件、DocumentState read projection、主题与 DPI。

## Outputs

EditText/Undo/Redo typed intent、dirty 调度结果、caret 坐标（供 IME 候选框）；不得输出
UI 自称的 deleted text 或 mutable document buffer。

## State Changes

preedit 只改变 ImeState；commit 通过单一 mutation gateway 产生一个 TextDelta。selection/
caret 变化只改变 Editor Session；成功 canonical edit 才推进 generation 和 dirty 状态。

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| delta 非 char boundary | 拒绝并保持状态不变 |
| composition cancel | 文档与 undo 不变 |
| 输入法候选框定位失败 | 回退到窗口左上角附近，不阻断输入 |
| IME 阻塞性问题 | 走 fallback 审批流程，不私自切换架构 |

## Configuration

基准字号/行高为固定 token；全局 `content_zoom_percent` 是唯一内容缩放配置。
键盘 `Ctrl++`/`Ctrl+=`/数字键盘 `+` 与 `Ctrl+-`/数字键盘 `-` 每次 ±10%，
`Ctrl+0` 恢复 100%；`Ctrl+Wheel` 每个完整 notch ±5%，高分辨率滚轮增量先在
Interaction Session 中累计。缩放提交经 ConfigCoordinator 合并写入；普通滚轮仍只滚动。

## Lifecycle

随 Source/Split 视图存在；纯 Preview 模式下编辑器不接收输入。

## Extension / Replacement Points

EditorBackend 平级实现；TextStore String→rope（见 `04`）。

## Performance Critical Paths

按键 → delta apply → 局部重绘；100 KiB / 1 MiB 文档输入延迟见 `10`。

## Verification

- 手工矩阵：微软拼音 + 微信输入法 × 上述 13 项 × 100/150/200% DPI。
- 单元/property：UTF-8 byte range、CJK、emoji、combining mark、selection 替换、
  undo grouping、256/4 MiB 限制、commit 一次撤销。
- 验收：AC-002/003/004/009/022。

## Non-Goals

语法高亮、多光标、宏、命令面板、Vim/Emacs 模式、WYSIWYG/Typora 模式、LSP。
