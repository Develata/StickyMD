# 08_assets_and_export.md - 图片资产与导出合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-19
- `Scope`: managed/user 资产边界、命名、编码保留、引用追踪、.trash、undo/redo 副作用、启动 reconcile、安全 GC、remote 图片、导出

---

## Purpose

定义图片资产的所有权、生命周期与导出契约。

## 核心安全 Invariant

```text
StickyMD must never automatically delete a file that it cannot prove it owns.
（StickyMD 永不自动删除它无法证明自己拥有的文件。）
```

## Boundary

- 资产事务由 AssetCoordinator 编排、单一 I/O 串行执行；UI/Preview 不做文件操作。
- 图片渲染呈现见 `06`；剪贴板读取的平台细节见 `09`。

## Owned Objects

`asset::managed_image`、`asset::trash_entry`。

---

## Managed vs User Asset

### Managed asset（程序可自动移动/删除）

文件名匹配：

```text
stickymd-<20-hex>.<supported-ext>
```

且位于 `./note/images/` 或 `./note/.trash/`。两者同时满足才是 managed。

### User asset（用户手工放入）

程序可以显示、可以导出复制；**不自动删除、不移动到 .trash、不重命名**。

### 任意本地引用

```text
![](images/custom.png)            ← 示例引用语法，非仓库内链接
![](../shared/diagram.png)        ← 示例引用语法，非仓库内链接
![](C:/Users/name/Desktop/a.png)  ← 示例引用语法，非仓库内链接
```

只读、不属于 GC、导出时可复制；必须路径规范化，不允许路径穿越写到目标目录外。

---

## 粘贴

### 剪贴板检测优先级（Windows）

1. `CF_HDROP` 图片文件列表。
2. 可直接取得的原编码 PNG/JPEG/WebP。
3. DIB/DIBV5 或普通 bitmap。
4. Unicode text。

### 编码规则

- PNG/JPEG/WebP：保留原始 bytes。
- GIF：保留原始 bytes；预览可只显示首帧。
- 其他稳定且可解码格式：可保留；不适合稳定预览的格式：解码后转 PNG。
- 截图/bitmap：统一编码为 PNG。

### 命名与去重

对最终写盘 bytes 计算 SHA-256，取前 20 个 hex：

```text
images/stickymd-7c9a0d7f8139e921a3f4.png
```

相同 bytes 不重复写文件；复用现有文件；同名文件在 `.trash` 时先恢复。

### Markdown 插入

- 单张：`` `![](images/stickymd-<hash>.<ext>)` ``。
- 多张：每张一个独立图片段落（空行分隔）。
- **图片写入成功后才插入 Markdown**；写入失败则不插入、显示错误、剪贴板文本不受影响。
- 图片写入与文本插入必须是同一个 UndoEntry。

---

## 引用追踪与 GC

### 保守引用扫描

为避免每键全量 parse，managed GC 用保守扫描：

- 扫描当前文本中的 managed 文件名 literal。
- literal 存在即视为引用（即使在 code block 中，宁可暂时保留）。
- 不允许因 parser 边界判断错误而误删图片。
- 完整 AST 只用于 preview 与 export。

### 逻辑删除

引用计数 `1 → 0` → `AssetEffect::MoveToTrash`：

```text
note/images/stickymd-x.png → note/.trash/stickymd-x.png
```

文本变化与 asset move 属于同一个 UndoEntry。这是逻辑删除，不是立即物理删除。

### Undo / Redo 副作用

- Ctrl+Z：恢复文本 → 计数 `0 → 1` → `.trash` 恢复到 `images/` → 更新计数 → preview dirty。
- Ctrl+Y：再次删除引用 → 重新移入 `.trash` → 更新计数 → preview dirty。

### 并发与顺序

每个资产操作携带 `transaction_id / document_generation / asset_name / expected_state`。
undo 在实际 move 前发生：可取消未执行的 move，或按队列顺序先 move 后 restore；
**最终状态必须与最新 DocumentState 一致**。

### 正常退出清理

等待所有资产操作 → 重扫最新内存文本 → 只删除确认无引用的 managed trash →
被引用的 managed 文件恢复到 `images/` → 用户文件永不删除。

### 异常退出后的启动清理

**禁止一启动就清空 `.trash`。** 正确顺序：

1. 加载 note.md。
2. 处理可能存在的恢复 temp。
3. 建立 managed reference set。
4. 被引用文件在 `.trash` 中 → 恢复到 `images/`。
5. 未引用的 managed trash → 永久删除。
6. `images/` 中未引用的 managed 文件 → 移入 `.trash`，再按安全策略清理。
7. 用户文件不动。

---

## Remote 图片

HTTP/HTTPS 图片：不发起网络请求、不下载、不缓存；Preview 显示 alt text + 可点击链接；
导出保留原 URL。程序默认无网络依赖。

## 图片安全限制

| 限制 | 值 |
| --- | --- |
| 最大解码像素 | 40 MP |
| 最大单边尺寸 | 16384 px |
| 最大编码文件 | 64 MiB |
| decoded cache | 16 MiB |
| 超限行为 | 显示占位符，不修改源文件 |

导出可复制超限原文件，但预览不解码。

---

## 导出（Ctrl+Shift+S）

- UI 名称：**导出**（不是“另存为”）。
- 工作文档永远是 `./note/note.md`；导出不切换 active document。
- 目标示例：

```text
D:\Export\my-note.md
D:\Export\my-note-assets\
├─ stickymd-a.png
└─ external-b.jpg
```

- 只复制实际引用的本地图片；remote URL 保留原样。
- 导出副本中的本地图片引用重写为：`` `![](my-note-assets/stickymd-a.png)` ``。
- 文件名冲突用内容 hash 解决。
- raw HTML 原样保留在 Markdown。
- 不导出：配置、`.trash`、未引用 managed 图片。
- 不生成 HTML/PDF。

---

## Inputs / Outputs

- Inputs：PasteClipboard intent、DocumentState 文本、undo/redo 事件、启动扫描。
- Outputs：images/ 文件写入、`.trash` 移动/删除、文本 delta 请求、导出目录。

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| 写入失败 | 不插入引用，显示错误 |
| 不可解码/超限 | 占位符提示，不写文件 |
| move 与 undo 竞争 | 事务顺序保证最终一致 |
| 路径穿越引用 | 规范化拒绝，不写目标目录外 |
| 启动发现被引用文件在 trash | 先恢复再 GC |

## Configuration

Not applicable（无用户配置项；限制为固定保护值）。

## Lifecycle

见“逻辑删除 / 退出清理 / 启动清理”节。

## Extension / Replacement Points

支持的图片格式集合（image crate feature）；编码保留策略。

## Performance Critical Paths

粘贴写盘在 I/O worker；预览解码懒加载且仅限 viewport 附近。

## Verification

- 粘贴/undo/redo、asset 写入与文本保存之间崩溃、启动 referenced-trash 恢复、
  用户图片不删除、remote 零请求、cache ≤ 16 MiB、路径穿越、超限占位符。
- property：任意图片事务最终与引用状态一致。
- 验收：AC-010/011/012/017/018。

## Non-Goals

图片编辑、图片标注、位图复制到剪贴板、云端图库、非图片附件管理。
