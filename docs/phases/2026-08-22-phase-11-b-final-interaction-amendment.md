# StickyMD Phase 11-B — Math Delimiter Conversion & Pin/Auto-Hide Orthogonality Amendment

你当前正在完成 StickyMD Phase 11。

**不要重做 Phase 11。**

USER 现在追加两个很小、明确、已批准的 v1 interaction amendments。请把它们作为：

> **Phase 11-B — Final Interaction Amendment**

在当前 Phase 11 基础上完成，然后重新运行所有受影响的 regression/readiness gates。

本补充阶段必须保持极窄 scope。

---

# 0. 允许修改的内容只有两项

## Amendment A — Math Delimiter Conversion

增加一个极小的 toolbar action：

```text
Convert AI math delimiters
```

或 UI 上更紧凑的 icon/label。

正式转换：

```text
\( ... \)
→
$ ... $
```

以及：

```text
\[
...
\]
→
$$
...
$$
```

不做反向转换。

---

## Amendment B — Pin / Auto-hide Orthogonality

正式冻结：

> **Always-on-top / Pin 与 Dock auto-hide 完全独立。**

Pin 只影响 Windows Z-order。

Pin **绝不能**：

- 阻止自动缩起；
- 延长自动缩起时间；
- 取消 focus-loss collapse；
- 改变 sensor reveal；
- 改变 manual collapse；
- 改变 Esc collapse。

---

# 1. 不允许其它功能扩张

Phase 11-B 禁止：

```text
new settings
new parser
new math syntax
regex-based Markdown rewrite framework
new toolbar customization
new docking behavior
new performance subsystem
new dependencies unless absolutely unavoidable
```

理想情况：

```text
new runtime dependencies = 0
```

---

# 2. 开始前

不要丢弃当前 Phase 11 工作。

先执行：

```bash
git status --short
git branch --show-current
git log -8 --oneline
```

记录当前 Phase 11 工作状态。

如果 Phase 11 有未提交修改：

- 不 reset；
- 不 clean；
- 不覆盖；
- 直接在当前工作上追加本补充。

---

# 3. Authority

开始实现前先更新对应 authoritative docs：

```text
docs/plan/07_editor_and_ime.md
docs/plan/09_windows_shell.md
docs/features/00_v1_product_behavior.md
docs/coverage-matrix.md
```

以及当前 Phase 11 task/report/acceptance。

不得先改代码再让 plan 追代码。

---

# 4. Amendment A 的核心原则

**禁止全局 regex/string replace。**

不要：

```rust
text.replace(r"\(", "$")
```

更不能盲目替换：

```text
\(
\)
\[
\]
```

因为这可能误伤：

```markdown
`\(...\)`

```text
\( not math \)
```

文档中讨论 delimiter 本身的普通文本。
```

---

# 5. 必须复用现有 Markdown semantic pipeline

当前已有：

```text
DocumentSnapshot
→ Comrak
→ Owned AST
→ MathNode
→ source ranges
```

因此正确流程：

```text
current DocumentSnapshot
        ↓
existing Comrak semantic parse
        ↓
find math nodes whose original delimiter is:
    \( ... \)
    \[ ... \]
        ↓
construct non-overlapping source replacements
        ↓
apply replacements from back to front
        ↓
single DocumentState transaction
```

Comrak 继续决定：

> 什么是真的数学公式。

StickyMD 只改变 delimiter representation。

---

# 6. 不重新实现 Math Parser

禁止：

```text
regex infer math
custom dollar parser
manual escape parser
manual code-block detector
```

已有 Comrak semantic authority 必须复用。

---

# 7. 必须保留原公式内部 bytes

例如：

```text
\(
  \frac{a}{b}
\)
```

只能改变 delimiter。

内部：

```text
  \frac{a}{b}
```

不得：

- trim；
- normalize；
- format；
- change escapes。

---

# 8. 转换规则

Inline：

```text
\(SOURCE\)
→
$SOURCE$
```

Display：

```text
\[SOURCE\]
→
$$SOURCE$$
```

---

# 9. Display newline preservation

例如：

```text
\[
x+y
\]
```

应变成：

```text
$$
x+y
$$
```

不是：

```text
$$x+y$$
```

如果原 inner source 本身包含换行：

原样保留。

---

# 10. 已是 Dollar Math

以下完全不动：

```text
$x$
$$x$$
```

---

# 11. Selection 行为

如果 Source editor 存在非空 selection：

> 只转换 **整个 MathNode source range 完全位于 selection 内部** 的公式。

不要转换与 selection 仅部分相交的 formula。

---

# 12. 无 Source selection

转换整篇当前 canonical document。

---

# 13. Preview-only Mode

如果当前：

```text
ViewMode::Preview
```

没有 Source selection authority。

因此：

> action 转换整篇 document。

不要使用 Preview rendered selection 决定 Source mutation。

---

# 14. Split

使用 Source pane 当前 selection。

---

# 15. Undo

无论一次转换：

```text
1
10
100
```

个公式：

必须是：

> **一个用户级 Undo step。**

一次：

```text
Ctrl+Z
```

恢复整个 delimiter conversion action。

---

# 16. Redo

一次：

```text
Ctrl+Y
```

重新应用整组 conversion。

---

# 17. No-op

如果：

```text
matches = 0
```

则：

```text
Document text unchanged
generation unchanged
undo unchanged
dirty unchanged
```

---

# 18. Generation

成功转换至少一个公式：

```text
generation += 1
```

只增加一次用户事务。

不要每个 formula 增一次。

---

# 19. Autosave / Preview

转换后正常触发：

```text
dirty
autosave
Preview refresh
asset scanner if normal document mutation path requires
```

全部走已有 canonical mutation pipeline。

---

# 20. 推荐 Intent

类似：

```rust
ConvertLatexMathDelimiters
```

不要让 toolbar 直接 mutate DocumentState。

路径仍：

```text
Interaction Shell
→ typed Intent
→ coordinator
→ semantic conversion
→ DocumentState
```

---

# 21. Toolbar UI

由于最小窗口已是：

```text
220 × 120 DIP
```

不得加入长文字按钮。

使用一个很紧凑的 control。

例如视觉概念：

```text
\( → $
```

或等价小型 vector icon。

---

# 22. Compact UI Gate

220 DIP width 时：

- 不得破坏 Close 可达性；
- 不得严重挤压 ViewMode controls；
- 不得新增 “...” settings menu。

如果现有 toolbar 已无空间：

优先优化 gap / icon geometry。

不要顺手重新设计 toolbar。

---

# 23. Delimiter Tests

至少自动覆盖：

### Inline

```text
\(x\)
→
$x$
```

### Display

```text
\[
x
\]
→
$$
x
$$
```

### Multiple

混合多个 inline/display。

### Existing dollar

保持原样。

### Inline code

```markdown
`\(x\)`
```

不改。

### Code fence

不改。

### Plain discussion

未被 Comrak 识别为 MathNode 时不改。

### Malformed delimiter

不改。

### Unicode

```text
\(中文 + \alpha\)
```

正确。

### Escapes

内部 bytes保持。

---

# 24. Selection Tests

至少：

```text
formula fully inside selection → convert
formula fully outside → unchanged
formula partially intersecting → unchanged
reverse selection → same behavior
```

---

# 25. Undo Tests

多 formula：

```text
convert
→ one undo
→ exact original source
→ one redo
→ converted source
```

---

# 26. Source Preservation Test

除 delimiter bytes 外：

```text
everything else byte-for-byte identical
```

---

# 27. Amendment B — Pin 与 Auto-hide

正式写入 plan invariant：

```text
AlwaysOnTop is orthogonal to Dock reveal/collapse state.
```

---

# 28. Pin 唯一职责

Pin / Always-on-top 只回答：

> 主窗口在 Windows Z-order 中是否保持 topmost。

---

# 29. Auto-hide 决策绝不能读取 configured_topmost

禁止类似：

```rust
if state.always_on_top {
    return KeepExpanded;
}
```

---

# 30. Auto-hide 也不读取 effective topmost

因为 Phase 8 已存在：

```text
configured_topmost
temporary_sensor_topmost
effective_topmost
```

它们都与：

```text
should_auto_collapse
```

正交。

---

# 31. Collapse predicate

其逻辑应只依赖既有条件：

```text
Docked
!focused
!ime_composing
!dragging
!resizing
!popup_active
pointer/timer state
manual/Esc
```

不得包含：

```text
always_on_top
effective_topmost
```

---

# 32. 明确场景

### Scenario 1

```text
Docked Right
Pin = ON
Focused = true
```

保持展开。

原因：

```text
Focused
```

不是 Pin。

---

# 33. Scenario 2

```text
Docked Right
Pin = ON
Focused = false
```

700 ms 后：

```text
collapse
```

---

# 34. Scenario 3

Pin ON + Collapsed：

sensor仍：

```text
100ms hover
→ reveal
```

---

# 35. Scenario 4

Pin ON + hover reveal：

pointer leave：

```text
500ms
→ collapse
```

如果没有focus。

---

# 36. Scenario 5

Pin ON + manual collapse：

立即。

---

# 37. Scenario 6

Pin ON + Docked + Esc：

立即collapse。

---

# 38. Floating

无论 Pin ON/OFF：

不会因普通focus loss进行 edge auto-hide。

---

# 39. Temporary Sensor Topmost

继续保留。

它只是：

> collapsed sensor accessibility mechanism。

---

# 40. configured topmost

与 temporary topmost 可以共同决定：

```text
effective Windows Z-order
```

但绝不能决定：

```text
DockRevealState
```

---

# 41. Pin Tests

必须自动参数化测试：

```text
Pin OFF
Pin ON
```

在同样 auto-hide input 下：

结果完全一致。

---

# 42. 推荐 property

对于所有 auto-hide transition：

```text
transition(state with topmost=false, event)
==
transition(state with topmost=true, event)
```

除了：

> platform Z-order effect output

之外。

---

# 43. 测试矩阵

至少：

```text
focus loss
focus regain
IME composing
manual collapse
Esc
sensor hover
hover leave
drag
resize
popup
```

Pin ON/OFF均相同 reveal/collapse state。

---

# 44. 代码审计

搜索：

```bash
rg "topmost|always_on_top|effective_topmost" apps/stickymd-win/src
```

逐个确认：

> 是否错误进入 auto-hide predicate。

---

# 45. 不为了正交性大重构 reducer

如果当前已经正确：

只：

```text
add invariant docs
add tests
```

不要为了“形式更漂亮”重写 Phase 8 window state machine。

---

# 46. Acceptance

为 Phase 11-B 增加 acceptance，例如：

```text
P11B-A01 semantic inline delimiter conversion
P11B-A02 semantic display delimiter conversion
P11B-A03 code/literal safety
P11B-A04 selection-scoped conversion
P11B-A05 one-step undo
P11B-A06 Pin/auto-hide orthogonality
```

遵循当前 acceptance naming convention。

---

# 47. Manual acceptance

建议新增：

### Math Conversion

在真实 Source：

```text
\(x^2\)
\[
\frac{a}{b}
\]
```

点一次按钮。

确认：

```text
$x^2$
$$
\frac{a}{b}
$$
```

---

# 48. Manual Undo

一次 Ctrl+Z全部恢复。

---

# 49. Manual Literal Safety

文档中同时存在：

```markdown
`\(example\)`

\(real\)
```

只有 real math改变。

---

# 50. Manual Pin / Dock

真实：

```text
Dock Right
Pin ON
click another app
```

仍应约700ms后collapse。

---

# 51. 同时测试

```text
Pin OFF
```

行为相同。

---

# 52. Pin 不影响 Sensor

ON/OFF均：

```text
100ms reveal
500ms hover-leave collapse
```

---

# 53. 不要把 manual 未执行写 PASS

---

# 54. Phase 11 原任务不要重开

本补充结束后：

更新当前：

```text
Phase 11 task
Phase 11 report
Phase 11 acceptance
```

并创建一个很短：

```text
docs/tasks/phase-11-b-final-interaction-amendment.md
```

作为 amendment receipt。

---

# 55. 推荐 task 内容

```text
Status
USER Amendment
Scope
Delimiter Conversion
Pin/Auto-hide Invariant
Tests
Regression
Result
```

---

# 56. Plan authority

必须将两项正式写入：

```text
docs/plan/07_editor_and_ime.md
docs/plan/09_windows_shell.md
docs/features/00_v1_product_behavior.md
```

---

# 57. 不修改 Engineering Constitution

不需要。

---

# 58. Performance

这两个改动不应产生显著性能影响。

只需要 smoke：

```text
semantic conversion 1MiB note
```

---

# 59. Conversion benchmark

构造：

```text
1 MiB note
1000 math nodes
```

Release测试转换。

目标不是极端微秒。

只要求：

```text
明显低于交互可感知阻塞
```

建议 engineering check：

```text
p95 < 50 ms
```

---

# 60. 如果超过50ms

先检查是否：

```text
重复 parse
重复 clone
每formula单独Document mutation
```

不要为了这个功能引入incremental Markdown parser。

---

# 61. 一次 parse

推荐：

```text
one snapshot
one semantic parse
one replacement collection
one batch mutation
```

---

# 62. 不需要后台 worker

这个 action是显式用户command。

在常规note大小下应足够快。

---

# 63. 如果1MiB parse已有现成 Preview pipeline成本

可以复用相同 semantic conversion utility。

但：

> 不得依赖 stale Preview AST。

必须基于当前 generation snapshot。

---

# 64. Stale protection

如果 semantic conversion需要异步执行：

必须：

```text
capture generation
→ build conversion
→ generation still same
→ commit
```

否则cancel。

但优先同步/simple。

---

# 65. Dependency

理想：

```text
new runtime dependency = 0
```

---

# 66. Core/Render Safety

仍必须：

```text
core unsafe = 0
render unsafe = 0
```

---

# 67. Forbidden architecture

仍无：

```text
WebView
Tokio
network
DB
new parser
```

---

# 68. Regression

本补充完成后至少重新运行：

```bash
cargo fmt --check

cargo clippy \
  --workspace \
  --all-targets \
  -- -D warnings

cargo test \
  --workspace \
  --locked

cargo build \
  --workspace \
  --release \
  --locked

cargo test -p stickymd-core --release --locked
cargo test -p stickymd-render --release --locked
cargo test -p stickymd-win --release --locked

cargo deny check

git diff --check
```

---

# 69. Smoke

运行现有：

```text
Phase 11 automated suites
Phase 10 UX regression
all --ci
```

以及新增 Phase 11-B smoke。

---

# 70. Phase 11 Readiness

如果本补充完成时 Phase 11 其它工作已经结束：

重新运行当前：

```text
readiness
```

不要使用补充前旧结果。

---

# 71. Artifact

如果 Phase 11 已生成 local RC：

本补充代码变更会使旧 artifact：

```text
superseded
```

需要重新：

```text
build
package
hash
SBOM
verify
```

---

# 72. 不 Push / Tag / Release

仍：

```text
push = no
tag = no
release = no
```

---

# 73. Commit 建议

如果当前 Phase11已有未提交工作：

不要机械拆提交。

如果适合独立 commit：

```text
feat(editor): add semantic AI math delimiter conversion

test(shell): enforce topmost and auto-hide orthogonality
```

或一个 cohesive：

```text
feat(rc): apply final Phase 11 interaction amendments
```

---

# 74. 最终回复追加格式

完成后，在原 Phase 11 Result 中增加：

# Phase 11-B Amendment Result

## Math Delimiter Conversion

```text
Inline:
Display:
Selection:
Literal/code safety:
One-step undo:
Performance:
```

## Toolbar

说明 compact layout 是否正常。

## Pin / Auto-hide Orthogonality

```text
Pin OFF:
Pin ON:
Focus loss:
Manual:
Esc:
Sensor:
Hover leave:
```

## Authority

```text
Delimiter detection authority: Comrak
Mutation authority: DocumentState
Topmost authority: WindowShellState/config
Auto-hide dependency on topmost: NONE
```

## Tests

新增数量和结果。

## Regression

列完整 gate。

## Artifact

如果重新生成：

```text
commit
EXE SHA-256
ZIP SHA-256
SBOM SHA-256
```

## Architecture Drift

```text
None.
```

## Git

```text
commit(s)
push = no
tag = no
release = no
```

最后：

> Continue/complete the existing Phase 11 readiness decision. Do not start Phase 12 automatically.

---

# 75. Phase 11-B Definition of Done

- [ ] USER amendment写入plan。
- [ ] Math conversion button/action实现。
- [ ] 不使用global regex替换。
- [ ] Comrak决定真实math nodes。
- [ ] `\(...\)` → `$...$`。
- [ ] `\[...\]` → `$$...$$`。
- [ ] dollar math不变。
- [ ] inline code不误改。
- [ ] fenced code不误改。
- [ ] non-math literal不误改。
- [ ] formula body byte-preserved。
- [ ] Source selection只转换fully-contained math。
- [ ] Preview-only转换整篇。
- [ ] Split使用Source selection。
- [ ] 整批转换一个Undo step。
- [ ] Redo正确。
- [ ] 0 matches no-op。
- [ ] conversion正常触发autosave/preview。
- [ ] compact toolbar仍适配220 DIP。
- [ ] Pin正式与auto-hide正交。
- [ ] auto-hide predicate不读取configured topmost。
- [ ] auto-hide predicate不读取effective topmost。
- [ ] Pin ON focus loss仍700ms collapse。
- [ ] Pin ON manual仍collapse。
- [ ] Pin ON Esc仍collapse。
- [ ] Pin ON sensor仍100ms reveal。
- [ ] Pin ON hover leave仍500ms collapse。
- [ ] Floating Pin ON仍不进行edge auto-hide。
- [ ] temporary sensor topmost逻辑保留。
- [ ] Pin ON/OFF reducer transition property测试。
- [ ] no architecture rewrite。
- [ ] new runtime deps ideally 0。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] fmt PASS。
- [ ] clippy PASS。
- [ ] tests PASS。
- [ ] Release build PASS。
- [ ] cargo deny PASS。
- [ ] full existing smoke重新运行。
- [ ] Phase 11 readiness重新评估。
- [ ] 旧artifact若受影响则标记superseded。
- [ ] 新artifact若需要则重新生成。
- [ ] 未push。
- [ ] 未tag。
- [ ] 未release。

完成后停止。
