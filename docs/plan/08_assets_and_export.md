# 08_assets_and_export.md - 图片资产与导出合同

## Metadata

- `Layer`: Capability
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-30
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

<a id="managed-vs-user-asset"></a>
## Managed vs User Asset

### Managed asset（程序可自动移动/删除）

文件名匹配：

```text
stickymd-<20|32|64-lowercase-hex>.<png|jpg|webp|gif>
```

并且必须同时满足：

1. 文件位于 canonical `./note/images/` 或 `./note/.trash/` 直属目录；
2. 目录与文件均不是 reparse point，文件是普通文件；
3. 对文件实际 bytes 计算的完整 SHA-256 以前述文件名 hash 部分开头。

四项共同构成 managed ownership proof。名称相似、位置相同但内容 hash 不匹配的
文件一律视为用户/不可信文件，StickyMD 不得自动移动或删除。

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

<a id="paste"></a>
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

对最终写盘 bytes 计算 SHA-256，优先取前 20 个 hex：

```text
images/stickymd-7c9a0d7f8139e921a3f4.png
```

相同 bytes 不重复写文件；复用现有文件；同名且 ownership proof 成立的文件在
`.trash` 时先恢复。若 20 hex 名称已被不同内容占用，依次扩展为 32、64 hex；
不得覆盖碰撞文件。

### Markdown 插入

- 单张：`![](images/stickymd-<hash>.<ext>)`。
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
- 每次文本 delta 只重扫编辑点两侧一个最大 managed 名称长度的窗口，更新
  DocumentState 内的保守计数；不在每次按键全量扫描文档。

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

资产操作与物理删除必须再满足以下时序约束：

- 后台 request 使用独立单调 `request_id`；`generation` 只表达文档版本，不能代替
  request identity。旧回执不得满足同 generation 的新退出/清理请求。
- 运行期 reconcile 只允许 active/trash 之间的可逆移动，不永久删除。
- 启动完成前与正常退出时才形成 destructive safe boundary。此时必须持有 canonical
  `note.md` 的稳定只读句柄，验证实际 durable bytes 指纹仍等于协调层给出的 base
  fingerprint，并以 durable 文本与最新 runtime 文本引用集合的并集执行保守 GC。
- 指纹未知、不匹配、文件被替换或不能取得稳定句柄时，物理删除必须降级为 deferred；
  只做非破坏性 reconcile，不得把不确定性解释为“无引用”。
- 一个较新的普通 runtime reconcile 会使尚未执行的旧 safe-boundary 请求失效；只有
  最新显式 safe-boundary request 可以物理删除。

### 正常退出清理

等待所有粘贴与资产操作 → 保存并确认最新文档 durable fingerprint → 在上述 safe boundary
重扫 durable 与最新内存文本 → 只删除确认无引用的 managed trash →
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

目录根与每个 managed source 必须分别证明：`note/`、`images/`、`.trash/` 是 canonical
note 下的普通目录且不是 reparse point；源文件通过打开的普通文件句柄校验 bytes 与完整
SHA-256，并由该句柄执行受约束 rename/delete。目标根在操作期间保持稳定句柄，任何 root
替换、reparse 或 full-digest 不一致都 fail closed。相同 hash prefix 但完整 digest 不同的
active/trash 文件不得互相覆盖或删除。

---

<a id="local-image-read-boundary"></a>
## 本地图片只读边界

Preview 与 Export 只通过 app execution-domain resolver 解释 Comrak 给出的 local image
destination。相对路径以 `note/` 为基准；绝对路径、`../`、percent-encoded 路径及 `file:`
路径只用于显式读取，绝不因此获得 managed ownership 或写入权限。Preview 的全局布局阶段
只打开 seekable reader 读取格式与尺寸元数据；只有图片进入 viewport 上下 300 DIP 邻域后，
才读取完整 bounded encoded bytes、计算内容哈希并解码缩放 raster。

## Remote 图片

HTTP/HTTPS 图片：不发起网络请求、不下载、不缓存；Preview 显示 alt text + 可点击链接；
导出保留原 URL。程序默认无网络依赖。

<a id="image-safety-limits"></a>
## 图片安全限制

| 限制 | 值 |
| --- | --- |
| 最大解码像素 | 40 MP |
| 最大单边尺寸 | 16384 px |
| 最大编码文件 | 64 MiB |
| decoded cache | 16 MiB |
| 超限行为 | 显示占位符，不修改源文件 |

导出可复制超限原文件，但预览不解码。

所有尺寸乘法必须 checked；解码前先检查 encoded size 与 metadata dimensions。
解码 cache 只保存 viewport 及上下 300 DIP 邻域需要的缩放 raster，按实际 RGBA
bytes + 保守固定 entry metadata 估值计费，并以 16 MiB LRU 与 512-entry 双重上限
约束内存。被当前 layout chunk 引用的 raster 仍属于 live cache bytes：LRU 不能只移除
map entry 后让外部 `Arc` 逃逸预算；预算不足时后续图片保持 placeholder，直到旧 layout
释放并允许淘汰。替换 layout 前先释放旧 raster leases。
远程图片只产生占位符与安全链接，
任何 Preview 路径均不得发起网络请求。

---

<a id="export"></a>
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
- 导出副本中的本地图片引用重写为：`![](my-note-assets/stickymd-a.png)`。
- 文件名冲突用内容 hash 解决。
- raw HTML 原样保留在 Markdown。
- 不导出：配置、`.trash`、未引用 managed 图片。
- 不生成 HTML/PDF。
- 导出引用集合只来自 Comrak/Owned AST 的真实 Image 节点；code/raw HTML 中的
  类似文本不算图片引用。Markdown 不做整篇重新序列化，只对 Image 节点 source
  range 做互不重叠、逆序应用的局部替换。reference-style 图片 occurrence 可以
  规范化为 inline image，但共享 reference definition 与普通链接不得被改写。
- 先在 StickyMD 独占 staging 目录构建并验证 assets，再发布 assets 目录，最后
  原子发布 Markdown；清理只限本次拥有的 staging。若目标 assets 目录已存在，
  选择 `-2` 等不冲突名称，不覆盖用户目录。

---

## Inputs

PasteClipboard intent、authoritative DocumentState snapshot、undo/redo 事件、启动扫描与
images/.trash 存储事实。

## Outputs

managed 文件、TextDelta+AssetEffect 协调结果、`.trash` 事务、预览位图、导出目录或
typed failure。

<a id="state-changes"></a>
## State Changes

资产写入成功后才允许提交 Markdown 引用；引用从 1→0 只产生受约束的 managed trash
事务。undo/redo 通过同一事务顺序恢复/重放，最终文件状态必须与最新 DocumentState 引用一致。

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
