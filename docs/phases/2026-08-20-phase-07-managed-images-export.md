# StickyMD Phase 7 — Managed Images, Clipboard Paste, Asset Transactions, GC & Export

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0–6 已完成。

当前正式系统已经具备：

```text
DocumentState
Source Editor + IME
Portable Persistence
Autosave / Recovery / Conflict
Comrak Owned AST
Native Markdown Preview
RaTeX Native Math
```

USER 已批准进入 Phase 7。

本阶段名称：

> **Phase 7 — Managed Images, Clipboard Paste, Asset Transactions, GC & Export**

---

# 0. Phase 7 的系统目标

本阶段建立完整图片生命周期：

```text
Clipboard Image
      │
      ▼
Clipboard Classification
      │
      ▼
Image Normalization / Preserve Encoding
      │
      ▼
SHA-256 Managed Identity
      │
      ▼
./note/images/stickymd-<hash>.<ext>
      │
      ▼
Markdown Image Reference
      │
      ▼
DocumentState
      │
      ▼
Native Lazy Preview
      │
      ▼
Reference Reconciliation
      │
      ├─ referenced → images/
      └─ unreferenced → .trash/
                         │
                    Undo/Redo
                         │
                         ▼
                    restore/trash
```

并正式实现：

```text
Ctrl+Shift+S
→ 导出 Markdown + 当前引用图片
```

---

# 1. 本阶段最重要的边界

StickyMD **不是附件管理器**。

Asset subsystem 只服务于：

> Markdown 中的图片。

不得扩展成：

```text
PDF attachment
ZIP attachment
generic file attachment
audio
video
document library
asset browser
media manager
```

---

# 2. 本阶段明确不做

禁止正式实现：

```text
Tray final lifecycle
Dock
Auto-hide
Hover reveal
Multi-monitor docking

最终视觉 polish
最终 Theme selector
最终 Opacity selector

图片编辑
图片裁剪
图片压缩设置
图片旋转 UI
图片 caption system
图库
附件面板

remote image download
HTTP cache
network client

animated GIF/WebP playback
image OCR
SVG renderer
PDF renderer
```

---

# 3. Phase 6 条件继承

以下仍保持原状态：

```text
Microsoft Pinyin real verification
WeChat IME real verification
真实 Light/Dark visual
真实 DPI visual
部分人工 math visual
同进程首次公式内存
```

如果仍未执行：

```text
NOT TESTED
```

不得变成 PASS。

Phase 7 不得用图片工作掩盖这些遗留项。

---

# 4. 开始前必须读取

严格执行：

```text
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md
```

至少阅读：

```text
docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/05_document_persistence.md
docs/plan/06_markdown_math_rendering.md
docs/plan/07_editor_and_ime.md
docs/plan/08_assets_and_export.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md

docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md

docs/report/phase-04-portable-persistence.md
docs/report/phase-05-markdown-native-preview.md
docs/report/phase-06-ratex-native-math.md
docs/report/phase-06-dependency-delta.md
```

---

# 5. Phase 6 Gate

必须确认：

```text
APPROVE Phase 7
```

或：

```text
APPROVE Phase 7 WITH CONDITIONS
```

且条件已被 USER 接受。

如果：

```text
STOP — architecture review required
```

停止。

---

# 6. 仓库 Preflight

执行：

```bash
git status --short
git branch --show-current
git log -10 --oneline

cargo metadata --no-deps

cargo tree -p stickymd-core
cargo tree -p stickymd-render
cargo tree -p stickymd-win
```

记录：

```text
starting commit
branch
clean / dirty
```

不得：

```text
reset
clean
rebase
覆盖 USER 修改
```

---

# 7. Phase 7 四层映射

本阶段调用链：

```text
Interaction Shell
      │
      ▼
Paste / Export Intent
      │
      ▼
AssetCoordinator
      │
      ├─ Clipboard capability
      ├─ ManagedAssetStore
      ├─ ImageDecoder
      ├─ ExportCoordinator
      └─ DocumentState mutation
      │
      ▼
Execution Domain
      │
      ├─ Windows Clipboard Adapter
      ├─ Filesystem Asset Adapter
      ├─ Native Image Decoder
      └─ File Save Dialog Adapter
```

---

# 8. Object Plane

正式加入/落实：

```text
asset::managed_image
asset::user_image
asset::trash_entry
asset::reference_set
asset::decoded_image
asset::export_asset
clipboard::image_payload
```

---

# 9. Authority 模型

必须明确：

```text
图片是否应该存在
```

的最终语义来源是：

> 当前 canonical `DocumentState` 中的图片引用状态。

Filesystem 只是存储事实。

---

# 10. Preview 不是 Asset Authority

禁止：

```text
Preview 看不到图片
→ 删除文件
```

---

# 11. images/ 目录也不是引用 Authority

禁止：

```text
images/中存在
→ 认为文档引用
```

---

# 12. Managed vs User Asset

正式定义：

### Managed Asset

由 StickyMD 自己粘贴/创建并具备可验证 ownership。

### User Asset

其它本地图片。

---

# 13. User Asset 绝不能自动删除

Hard invariant：

> StickyMD must never automatically delete a file that it cannot prove it owns.

必须进入代码注释、plan 和测试。

---

# 14. Runtime 目录

Phase 7 后：

```text
<program-dir>/
├─ StickyMD.exe
└─ note/
   ├─ note.md
   ├─ config.toml
   ├─ images/
   │  ├─ stickymd-7c9a0d7f8139e921a3f4.png
   │  ├─ stickymd-7ac2....jpg
   │  └─ user-image.png
   └─ .trash/
      └─ stickymd-....
```

---

# 15. Startup Bootstrap

Phase 7 正式确保：

```text
./note/images/
./note/.trash/
```

存在。

---

# 16. 如果 images 路径存在但不是目录

Persistence/Asset capability failure。

不得覆盖。

---

# 17. `.trash` 同理

---

# 18. Managed filename

默认：

```text
stickymd-<20 lowercase hex>.<canonical-ext>
```

例如：

```text
stickymd-7c9a0d7f8139e921a3f4.png
```

---

# 19. Hash

使用：

```text
SHA-256(final persisted encoded bytes)
```

不是：

```text
decoded pixels
```

---

# 20. 为什么 hash encoded bytes

保证：

```text
identical file bytes
→ identical asset identity
```

同时不需要 decode 才能 deduplicate。

---

# 21. Filename hash prefix

默认：

```text
20 hex characters
```

即 80 bit prefix。

---

# 22. Hash collision必须检测

如果：

```text
images/stickymd-<prefix>.png
```

已存在：

不得只因为 filename相同就假设内容相同。

必须计算完整 SHA-256。

---

# 23. 如果 full hash相同

复用。

---

# 24. 如果 prefix collision

极小概率但必须定义。

依次尝试：

```text
20 hex
32 hex
64 hex
```

---

# 25. Managed filename parser

因此 grammar允许：

```text
stickymd-<20|32|64 hex>.<supported-ext>
```

---

# 26. Managed Ownership Proof

文件只有同时满足：

1. 位于 canonical：

```text
./note/images/
```

或：

```text
./note/.trash/
```

2. 文件名符合 managed grammar。

3. 文件实际 SHA-256 与 filename hash prefix 匹配。

才视为：

```text
OwnershipProvenManagedAsset
```

---

# 27. 仅文件名匹配不够

如果用户手工放：

```text
stickymd-deadbeef....png
```

但 bytes hash 不匹配：

StickyMD不得删除或移动。

按：

```text
untrusted/user file
```

处理。

---

# 28. 自动 destructive operation必须先做 ownership proof

包括：

```text
MoveToTrash
RestoreFromTrash
PermanentDelete
DuplicateCleanup
```

---

# 29. Supported managed extensions

v1 至少：

```text
png
jpg
webp
gif
```

JPEG canonical extension：

```text
.jpg
```

不是：

```text
.jpeg
```

---

# 30. Screenshot

统一：

```text
PNG
```

---

# 31. 其它图片输入

例如：

```text
BMP
ICO
TIFF
```

如果来自 clipboard/file：

允许：

```text
decode
→ PNG
```

前提：

- 当前 image decoder 支持。
- resource limits通过。

---

# 32. 不保留危险/无必要格式

例如：

```text
EXR
AVIF
DDS
HDR
```

v1 默认不需要。

不要为了“image crate支持”全部启用。

---

# 33. image crate Dependency Gate

在修改 Cargo 前重新确认当前 stable。

当前 Prompt 编写时：

```text
image 0.25.10
```

但实际实现以当前 crates.io + Cargo compatibility 为准。

---

# 34. image crate 必须

```toml
default-features = false
```

---

# 35. 最小 codec feature

根据实际 API审核，优先只启用：

```text
png
jpeg
webp
gif
bmp
ico
```

---

# 36. 不自动启用

```text
avif
avif-native
dav1d
rayon
exr
tiff
hdr
dds
```

除非 USER 之后批准。

---

# 37. Cargo Tree Hard Gate

执行：

```bash
cargo tree -p stickymd-render
```

检查不存在意外：

```text
dav1d
rav1e
rayon
exr
```

等重依赖。

---

# 38. ImageDecoder 所属

`image` crate 应主要进入：

```text
stickymd-render
```

负责：

```text
image format inspection
dimensions
decode
resize
pixel conversion
```

---

# 39. Render crate 不负责打开用户文件

仍遵循壳核/adapter：

```text
filesystem adapter
→ opened reader / bytes
→ image decode capability
```

---

# 40. 不让 render crate hardcode

```text
./note/images
```

---

# 41. Image decoding必须 safe Rust

`stickymd-render` 继续：

```rust
#![forbid(unsafe_code)]
```

---

# 42. 图片 resource limits

冻结工程保护：

```text
MAX_ENCODED_IMAGE_BYTES = 64 MiB
MAX_IMAGE_SIDE = 16_384 px
MAX_IMAGE_PIXELS = 40_000_000
DECODED_IMAGE_CACHE = 16 MiB
```

---

# 43. Pixel multiplication必须 checked

永远：

```text
width.checked_mul(height)
```

再：

```text
pixels.checked_mul(4)
```

---

# 44. 不允许整数溢出

---

# 45. image crate limits

使用库提供的：

```text
max_image_width
max_image_height
```

strict limit。

---

# 46. 不仅依赖 max_alloc

因为该项不一定对所有 decoder 都是严格限制。

StickyMD 自己仍必须先：

```text
inspect dimensions
→ checked pixel count
→ expected RGBA bytes
```

---

# 47. 40 MP 可能带来大瞬时内存

如果实测发现：

```text
40 MP
```

显著违反低内存设计：

不得擅自降低冻结上限。

创建：

```text
docs/report/phase-07-large-image-memory-risk.md
```

交 USER。

---

# 48. Animated Image

GIF/WebP：

v1：

```text
render first frame only
```

不播放动画。

---

# 49. 不建立 animation timer

---

# 50. EXIF orientation

如果 decoder API可可靠提供 orientation：

正式应用。

否则：

记录限制。

不要手写 EXIF parser。

---

# 51. Alpha

PNG/WebP透明：

保持 alpha。

---

# 52. Preview pixel representation

优先转换为：

```text
premultiplied RGBA
```

或 tiny-skia自然需要的等价 representation。

---

# 53. Clipboard Priority

图片粘贴时严格按：

```text
1. File-drop image(s)
2. Native encoded image clipboard format
3. Bitmap/DIB/DIBV5/raw pixels
4. Text clipboard
```

---

# 54. 为什么 File Drop优先

因为可以：

```text
保留原始文件编码
```

---

# 55. File Drop

Windows：

```text
CF_HDROP
```

需要薄 Windows adapter。

---

# 56. 不把 CF_HDROP 放 core

---

# 57. File Drop 只读取 path descriptors

实际文件 read/hash/copy：

放 worker。

---

# 58. 多文件 Clipboard

如果 clipboard 文件列表：

```text
全部都是可处理图片
```

则：

```text
multi-image paste
```

---

# 59. Mixed file list

如果同时有图片和非图片：

默认：

> 不建立“半图片半附件”行为。

按普通文件路径文本处理，或沿 Phase 3 已有 file-path fallback。

必须在 report明确。

---

# 60. 不创建 generic attachment

---

# 61. File Image Preserve Encoding

如果文件实际内容检测为：

```text
PNG
JPEG
WebP
GIF
```

保留原 bytes。

---

# 62. 不盲信扩展名

必须 sniff actual format。

---

# 63. Canonical extension由实际格式决定

例如：

```text
foo.png
```

实际 JPEG：

managed file应：

```text
stickymd-....jpg
```

---

# 64. 原编码校验

保留 encoded bytes前：

至少：

```text
format detection
dimension inspection
resource guards
```

---

# 65. Native Encoded Clipboard

Windows clipboard可能提供注册：

```text
PNG
JFIF
image/png
image/jpeg
```

等。

Agent 必须先实际枚举 Windows 11常见来源：

```text
Snipping Tool
browser
Paint
Photos
```

记录 clipboard formats。

---

# 66. 不凭名称猜完整支持

建立：

```text
docs/report/phase-07-windows-clipboard-formats.md
```

记录真实格式 ID/name。

---

# 67. 如果有可靠 raw PNG clipboard bytes

直接保留。

---

# 68. JPEG/WebP同理

如果实际格式存在且可靠。

---

# 69. Bitmap fallback

否则：

```text
DIB / DIBV5 / raw image pixels
→ RGBA
→ PNG
```

---

# 70. Screenshot路径

最终：

```text
PNG
```

---

# 71. arboard 审计

如果 Phase 3 已使用：

```text
arboard
```

先查看其实际 version/features。

---

# 72. 当前基线

Prompt 编写时：

```text
arboard 3.6.1
```

其 `image-data` feature会引入图片支持。

但 Agent 必须重新核实。

---

# 73. 不假设 arboard保留 encoded bytes

如果其 API只返回：

```text
RGBA/raw pixels
```

它只能作为：

```text
bitmap fallback
```

不能替代 CF_HDROP / encoded format path。

---

# 74. 启用 image-data 前依赖审计

检查是否导致：

```text
duplicate image crate
unwanted platform deps
binary growth
```

---

# 75. 如果现有 Windows adapter更轻

可直接使用 Windows clipboard API。

---

# 76. 平台-specific clipboard是允许的

因为：

> 原始编码格式是 Windows clipboard 事实。

属于合理 platform adapter。

---

# 77. Clipboard unsafe边界

只允许：

```text
apps/stickymd-win/src/platform/windows/clipboard*
```

---

# 78. 每个 unsafe都有 SAFETY

说明：

```text
OpenClipboard lifetime
GlobalLock lifetime
GlobalUnlock
handle ownership
buffer bounds
UTF-16 path ownership
```

---

# 79. Clipboard Resource Guard

读取 clipboard encoded image前：

检查 size。

超过：

```text
64 MiB
```

拒绝。

---

# 80. DIB width/height必须先检查

不能根据恶意 header分配无限内存。

---

# 81. Clipboard Busy

`OpenClipboard` 可能失败。

使用非常有限 retry，例如：

```text
10ms
30ms
80ms
```

然后失败。

---

# 82. 不无限 retry

---

# 83. Clipboard paste failure

DocumentState不变。

---

# 84. Paste Image Intent

正式新增：

```text
PasteClipboard
```

现有 Intent可以继续。

Coordinator判断：

```text
text
image
files
```

---

# 85. Text paste行为不能回归

普通 text仍走 Phase 3路径。

---

# 86. 图片粘贴是异步事务

因为可能涉及：

```text
file read
hash
encode
write
flush
```

不得阻塞 UI thread。

---

# 87. Paste capture

图片粘贴开始时记录：

```text
document generation
selection
```

---

# 88. Async Paste OCC

asset准备完成时：

如果：

```text
current generation != captured generation
```

默认：

```text
abort Markdown insertion
```

---

# 89. 为什么不自动重新定位

避免把图片插到用户随后输入后的错误位置。

---

# 90. Aborted paste asset cleanup

本次新创建、尚未被Document引用的managed files：

```text
move to .trash
```

或安全删除。

推荐：

```text
trash
```

然后由 GC清理。

---

# 91. Paste OCC失败提示

极小：

```text
图片粘贴已取消，因为文本已发生变化。
```

---

# 92. Paste Transaction顺序

必须：

```text
1. Read/validate clipboard image
2. Determine final encoded bytes
3. Hash
4. Ensure managed asset exists in images/
5. Only after asset persistence success
6. Commit Markdown TextDelta
7. Attach managed asset undo effect
```

---

# 93. 禁止反向顺序

不能：

```text
先插Markdown
→ 后写图片
```

否则文件写失败产生 broken reference。

---

# 94. Crash between asset write and Markdown edit

结果：

```text
unreferenced managed image
```

Startup reconciliation会清理。

这是安全 orphan。

---

# 95. Crash after Markdown edit before autosave

磁盘 note可能还没有图片引用。

asset成为 orphan。

下一次启动按 durable note清理。

正确。

---

# 96. Asset file write必须安全

使用：

```text
same-dir temp
→ write
→ flush
→ publish new file
```

---

# 97. 不需要 ReplaceFileW覆盖 content-addressed asset

正常 managed asset：

final path应不可变。

---

# 98. 如果 final file不存在

publish。

---

# 99. 如果存在

验证 full hash。

一致：

复用。

---

# 100. 不一致

prefix collision / corruption路径。

使用更长 hash filename。

---

# 101. Managed assets immutable

一旦：

```text
stickymd-<hash>.png
```

建立：

StickyMD不得原地修改其 bytes。

---

# 102. 图片内容变化意味着新 asset

---

# 103. 多图片 paste atomicity

Document层应：

> all-or-nothing。

---

# 104. 所有图片先准备

如果任意失败：

```text
不插入任何 Markdown
```

---

# 105. 已创建新 assets

转 `.trash`。

---

# 106. Multi-image Markdown

推荐：

```markdown
![](images/stickymd-a.png)

![](images/stickymd-b.jpg)

![](images/stickymd-c.webp)
```

---

# 107. Alt text

StickyMD自动粘贴统一：

```text
empty alt
```

即：

```markdown
![](images/...)
```

保持极简和一致。

---

# 108. 不把本地原文件名泄漏到 Markdown alt

---

# 109. Managed path syntax

必须统一：

```text
images/stickymd-....
```

使用：

```text
/
```

不是 Windows `\`。

---

# 110. AssetReferenceScanner

GC不能每个键都重新完整 Comrak parse。

建立极小、保守：

```text
ManagedAssetReferenceScanner
```

---

# 111. Scanner 目的

只回答：

> 某个 managed filename literal 是否仍可能被 Document 引用。

---

# 112. Scanner策略必须保守

可以把：

```text
code block
HTML comment
plain text
```

中的 managed filename也视为“仍被引用”。

---

# 113. False Positive允许

结果：

```text
多留一张图
```

可接受。

---

# 114. False Negative绝不允许

结果可能：

```text
误删图片
```

不可接受。

---

# 115. Scanner不是 Markdown parser

不要重新实现 Markdown。

---

# 116. Scanner只识别 managed filename grammar

例如搜索：

```text
stickymd-<20/32/64hex>.<ext>
```

---

# 117. Ref count

建立：

```text
ManagedAssetId → count
```

---

# 118. 同图引用多次

例如3处：

```text
count = 3
```

删一处：

```text
count = 2
```

不能 trash。

---

# 119. 只有：

```text
count > 0
→ 0
```

触发 logical delete。

---

# 120. 0→1

如果 asset在 `.trash`：

需要恢复。

---

# 121. Reference Tracker input

唯一：

```text
canonical DocumentState text
```

---

# 122. Preview AST不是 GC input

---

# 123. UndoEntry 正式扩展

Phase 2 已刻意为此留 private extension point。

本阶段可以扩：

```rust
struct UndoEntry {
    text: TextDelta,
    asset_effects: Vec<AssetUndoEffect>,
    ...
}
```

---

# 124. AssetUndoEffect 是纯领域值

不得包含：

```text
PathBuf
File handle
Windows API
callback
closure
```

---

# 125. 建议模型

例如：

```rust
struct AssetUndoEffect {
    asset: ManagedAssetId,
    forward: ManagedAssetState,
    reverse: ManagedAssetState,
}
```

状态：

```rust
enum ManagedAssetState {
    Active,
    Trashed,
}
```

---

# 126. Core可以知道 ManagedAssetId

这是 Object Plane值对象。

---

# 127. Core不能移动文件

hard invariant。

---

# 128. 普通 edit后的 reconcile

流程：

```text
DocumentState edit success
→ current generation
→ scan new canonical text
→ compare previous managed ref state
→ derive AssetUndoEffects
→ attach to current undo entry
→ AssetCoordinator schedules desired filesystem state
```

---

# 129. attach effect必须 generation-safe

例如：

```rust
attach_asset_effects(
    expected_generation,
    effects
)
```

如果 generation不匹配：

拒绝。

---

# 130. 如果 edit被Undo grouping合并

effect附着到最终 grouped UndoEntry。

---

# 131. 如果 edit没有记录 Undo

例如 oversized edit：

asset reconcile仍必须执行。

只是不具有Undo side effect。

---

# 132. 不为了asset拒绝合法文本 edit

优先级：

```text
canonical text correctness
>
asset convenience
```

---

# 133. Normal delete

用户删除最后图片引用：

立即：

```text
desired state = Trashed
```

---

# 134. 文件移动异步

可以出现短暂：

```text
Markdown已无引用
file仍在 images/
```

这是安全状态。

---

# 135. MoveToTrash失败

不得回滚文本。

记录：

```text
AssetReconcilePending / Failed
```

并可稍后 retry。

---

# 136. 因为：

```text
多留文件
```

比：

```text
回滚用户编辑
```

安全。

---

# 137. Undo

Ctrl+Z：

DocumentState先恢复文本并返回：

```text
asset reverse effects
```

---

# 138. AssetCoordinator

随后：

```text
Trashed → Active
```

---

# 139. Undo Restore失败

文本 Undo仍保持。

必须：

```text
显示图片缺失/恢复失败状态
```

不得把文本再undo回去。

---

# 140. Redo

再：

```text
Active → Trashed
```

---

# 141. Undo/Redo最终一致性

当 I/O 成功后：

```text
Document reference state
==
managed asset desired state
```

---

# 142. Pending operations 必须串行

复用 Phase 4 single I/O worker。

---

# 143. 不增加 generic transaction engine

---

# 144. Asset request

例如：

```rust
enum AssetIoRequest {
    EnsureManaged(...),
    MoveToTrash(...),
    Restore(...),
    DeleteTrash(...),
}
```

可以整合现有 `IoRequest`。

---

# 145. Note Save优先级

Asset GC不能饿死：

```text
note save
```

---

# 146. 建议优先级

```text
Shutdown Save
Manual Note Save
Autosave Note
Paste Asset Ensure
Asset Restore
Trash Move
GC Delete
```

无需 priority queue，只需明确 dispatch策略。

---

# 147. File watcher

Phase 4 watcher：

不要因为 images/变化触发 note conflict。

---

# 148. Watch filter必须仍只把 note.md变化视为文档 external fact

---

# 149. Preview 图片支持

Phase 5 image placeholder正式升级：

```text
ImageNode
→ ResolvedImage
→ ImageLayoutBox
→ Lazy Raster
```

---

# 150. Remote image保持 placeholder

Phase 7也：

```text
不请求网络
```

---

# 151. Local images

允许：

```text
relative
absolute
../
```

只读显示。

---

# 152. Relative base

仍：

```text
./note/
```

---

# 153. Managed local path

```text
images/stickymd-...
```

---

# 154. User image

例如：

```text
images/custom.png
../shared/foo.jpg
C:/Users/.../foo.png
```

全部可读显示。

---

# 155. 但 GC只管 OwnershipProvenManagedAsset

---

# 156. Filesystem read adapter

Preview decode不得让 render crate自己解析 runtime path identity。

App Execution Domain：

```text
resolve local image path
→ open reader
→ pass reader to render image decoder
```

---

# 157. Preview build first pass

需要知道 intrinsic dimensions。

不要全量 decode。

---

# 158. Dimension inspection

使用 decoder metadata / `into_dimensions()` 等成熟 API。

---

# 159. 维度检查也必须 resource guard

---

# 160. Layout box

建议：

```rust
struct ImageBox {
    source: ResolvedImageSource,
    intrinsic_width: u32,
    intrinsic_height: u32,
    display_width: f32,
    display_height: f32,
}
```

---

# 161. Display Size

自然：

```text
1 image pixel ≈ 1 DIP at 100%
```

并：

```text
display_width <= preview content width
```

---

# 162. Preserve aspect ratio

hard invariant。

---

# 163. 不自动 upscale 小图

建议：

```text
display_width = min(intrinsic_width_dip, content_width)
```

---

# 164. 高 DPI只影响 raster presentation

不要错误双倍逻辑尺寸。

---

# 165. Lazy decode

只有进入：

```text
viewport
+
prefetch margin
```

的图片才 decode。

---

# 166. Prefetch margin

建议：

```text
300 DIP
```

---

# 167. Preview opening 100张图

不能一次 decode 100张。

---

# 168. Image decode job

复用：

```text
Preview Worker
```

优先。

原因：

- CPU decode不能阻塞 Phase4 I/O worker。
- Preview worker已有单线程。
- 无需新增 thread pool。

---

# 169. Preview Worker jobs

可以扩：

```text
BuildPreview
DecodeImage
```

---

# 170. Build Preview 优先

如果有新 generation：

stale image decode可以丢弃/降低优先级。

---

# 171. 不创建每图片一个thread

---

# 172. Image Decode Result

immutable：

```rust
Arc<DecodedImage>
```

---

# 173. Cache

冻结：

```text
DecodedImageCache <= 16 MiB
```

---

# 174. Cache accounting

按：

```text
actual pixel bytes
```

+ 小 metadata。

---

# 175. Cache key：managed

Managed：

```text
ManagedAssetId
```

足够，因为 content-addressed immutable。

---

# 176. User/local mutable image key

至少：

```text
canonical/resolved path
file size
modified timestamp
```

---

# 177. 不需要内容hash每帧

---

# 178. 外部 user image 变化

v1 不要求实时 watcher。

下一次：

```text
Preview rebuild
mode refresh
```

重新检查。

明确记录。

---

# 179. Cache eviction

LRU/recency。

---

# 180. 单 decoded display image >16 MiB

可以：

- transient使用不进cache；
- 或先缩放到Preview目标尺寸后缓存。

优先：

```text
resize to actual display need
→ cache scaled raster
```

---

# 181. Peak memory仍需测量

因为 decode原图可能需要完整buffer。

---

# 182. Viewport downscale

如果原图：

```text
8000×8000
```

Preview只需要：

```text
500px
```

应尽早缩小后释放 full decoded image。

---

# 183. 不保留 full-resolution decode

除非显示确实需要。

---

# 184. Image decode error

局部：

```text
[图片无法显示]
alt
path
```

---

# 185. Missing local image

同样 placeholder。

---

# 186. 不使整个 Preview失败

---

# 187. Unsupported image

placeholder。

---

# 188. Image selection

Phase 5 preview selection规则：

选中图片：

```text
copy alt text
```

继续保持。

---

# 189. 不把bitmap复制到clipboard

本阶段没有这个产品要求。

---

# 190. Image hit test

不需要点击打开图片。

---

# 191. Remote placeholder URL

仍可作为 safe link。

---

# 192. Startup Asset Reconciliation

必须在：

```text
Recovery resolution
+
note.md load
```

之后执行。

---

# 193. 不能先清 `.trash`

hard invariant。

---

# 194. Startup顺序扩展

```text
Resolve Program Directory
→ Single Instance
→ Writable
→ Recovery
→ Load note.md
→ Build DocumentState
→ Ensure images/.trash
→ Scan canonical managed refs
→ Asset Reconciliation
→ Start Editor
→ Watcher
→ Autosave
```

---

# 195. Startup referenced asset in images/

保持。

---

# 196. referenced asset only in .trash/

如果 ownership proof通过：

```text
restore to images/
```

---

# 197. referenced asset missing both

记录：

```text
MissingManagedAsset
```

不修改 Markdown。

---

# 198. unreferenced managed in images/

移动：

```text
images/
→ .trash/
```

---

# 199. unreferenced managed in .trash/

可以永久删除。

---

# 200. 但必须 ownership proof

---

# 201. User file in images/

永不动。

---

# 202. User file in .trash/

如果没有 ownership proof：

永不动。

---

# 203. managed-looking corrupt file

hash不匹配：

永不自动删除。

---

# 204. Duplicate active+trash

同一 valid managed asset同时存在：

优先 active。

如果 bytes相同：

trash duplicate可安全删除。

---

# 205. 如果 bytes不同

不自动删。

记录：

```text
ManagedAssetCollisionState
```

---

# 206. Reconciliation错误

不阻止 document编辑。

---

# 207. 但不要假装asset完整

显示 warning。

---

# 208. Startup GC failure

多留文件即可。

---

# 209. Normal Runtime Logical Delete

不永久 delete。

只 trash。

---

# 210. Normal Exit GC

正确顺序：

```text
1. final note save succeeds
2. wait pending asset transitions
3. rescan authoritative latest DocumentState
4. restore any referenced managed trash
5. permanently delete only proven unreferenced managed trash
6. config save
7. exit
```

---

# 211. Final note save失败

不得执行 destructive GC。

---

# 212. GC failure

不应阻止退出。

因为：

```text
留下额外trash
```

是安全状态。

记录。

---

# 213. Process crash

下次 startup reconciliation处理。

---

# 214. Asset rename

使用同volume rename。

---

# 215. Asset move失败

保留源文件。

---

# 216. 不 copy+delete模拟rename除非明确需要

同目录树应可 rename。

---

# 217. Export

正式启用：

```text
Ctrl+Shift+S
```

UI名称：

```text
导出
```

---

# 218. 禁止名称

不要：

```text
另存为
Save As
```

---

# 219. Export不改变 active document

完成后：

```text
working note
=
./note/note.md
```

---

# 220. Export source

必须：

```text
current immutable DocumentSnapshot
```

即使 dirty也可导出。

---

# 221. Conflict期间 Export

允许。

导出的是：

```text
当前 local DocumentState
```

这还能作为数据救援。

---

# 222. Export不要求先Autosave

---

# 223. File Dialog

需要原生 Windows save dialog。

---

# 224. 实现方案 Gate

先检查现有依赖。

优先顺序：

1. 已批准且已经存在的 native dialog abstraction；
2. 小型、合理依赖的 `rfd`;
3. 薄 Windows `IFileSaveDialog` adapter。

---

# 225. 不为了“跨平台”强制加入重依赖

v1是 Windows 11。

---

# 226. 如果选择 rfd

重新核实 current stable和 dependency tree。

当前 Prompt基线：

```text
rfd 0.17.2
```

但不得盲用。

---

# 227. rfd 使用 synchronous save dialog即可

不引入 async runtime。

---

# 228. 如果 rfd dependency明显过重

实现 Windows-specific adapter。

这是合法 platform dependency。

---

# 229. File Dialog只能返回路径

Export业务逻辑不进入 dialog adapter。

---

# 230. Export output

用户选择：

```text
D:\Export\my-note.md
```

生成：

```text
D:\Export\
├─ my-note.md
└─ my-note-assets\
   ├─ stickymd-....png
   ├─ asset-....jpg
   └─ ...
```

---

# 231. 只导出实际 Markdown image nodes

这与 GC scanner不同。

Export必须使用：

```text
Comrak / Owned AST
```

识别真正图片。

---

# 232. Code block中的文件名

不导出。

---

# 233. Raw text中的图片路径

不导出。

---

# 234. Remote images

保持原URL。

不复制、不下载。

---

# 235. Local managed images

复制当前 active asset。

---

# 236. User local images

也复制。

包括：

```text
./note/images/custom.png
../shared/foo.jpg
C:/...
```

---

# 237. Export只读外部图片

不修改源。

---

# 238. Export missing local image

推荐 fail-before-publish：

```text
导出失败：一个或多个本地图片不存在。
```

---

# 239. 不生成已知 broken export

---

# 240. Export preparation必须先完整 resolve

在写最终文件前：

```text
collect image nodes
resolve paths
validate readable
validate sizes
assign export names
```

---

# 241. Export asset naming

Managed：

可以保留原 managed basename。

User/external：

建议：

```text
asset-<20hex>.<canonical-ext>
```

---

# 242. Export hash

对原文件 bytes SHA-256。

---

# 243. Export dedup

多个 image node指向相同bytes：

只复制一次。

---

# 244. Collision

与managed同样完整hash核验。

---

# 245. Export Assets Directory 已存在

**绝不能删除它。**

---

# 246. 不覆盖已有assets目录

自动找：

```text
my-note-assets
my-note-assets-2
my-note-assets-3
...
```

第一个不存在的。

---

# 247. 用户已有文件绝不能被Export cleanup删除

---

# 248. Existing my-note.md

由 native save dialog负责 overwrite confirmation。

---

# 249. Export Markdown路径重写

所有 local image destination改为：

```text
my-note-assets/<export-name>
```

或实际 suffix dir。

---

# 250. Remote destinations不改

---

# 251. 不要重新序列化整个 Markdown AST

否则可能改变：

```text
formatting
spacing
delimiter
raw HTML
code fences
```

---

# 252. Export必须 source-preserving

原则：

> 除 local image syntax 必要重写外，其它 Markdown bytes/文本语义保持原样。

---

# 253. Image Rewrite

使用 Phase 5 source ranges。

---

# 254. Inline image

可以只替换destination range。

如果 Comrak/Phase5已经有精确destination range：

优先。

---

# 255. 如果没有 precise destination range

可以对：

```text
已由Comrak确认的单个 image node source range
```

做局部 source-preserving rewrite。

---

# 256. 禁止重新实现整个 Markdown parser

---

# 257. Reference-style image

例如：

```markdown
![alt][img]

[img]: images/foo.png
```

必须正确导出。

---

# 258. 推荐行为

可以把该单个 image occurrence正规化成：

```markdown
![alt](my-note-assets/foo.png)
```

同时保留其它源文本。

---

# 259. 只允许 image node syntax局部 normalization

必须在文档中明确：

> Export preserves document source except that local image references may be normalized to inline image syntax.

---

# 260. Link reference不能被误改

如果同一个reference definition也被普通link使用：

不得全局修改definition导致link目标变化。

---

# 261. 因此 reference image occurrence局部 inline化更安全

---

# 262. Alt escaping

实现 Markdown-safe escaping。

---

# 263. Title

如果原 image有 title：

保留。

---

# 264. 不丢 alt

---

# 265. Replacements

收集：

```text
byte-range replacements
```

---

# 266. 必须 non-overlapping

---

# 267. 从后往前应用

避免 offset shift。

---

# 268. Export source UTF-8

仍：

```text
UTF-8 without BOM
```

---

# 269. Export line ending

继承当前 DocumentState：

```text
LineEnding
```

---

# 270. Export Staging

不要边复制边创建半成品最终结构。

建议：

```text
destination parent/
└─ .stickymd-export-<pid>-<nonce>/
```

---

# 271. Staging内容

```text
my-note.md
my-note-assets/
```

---

# 272. 全部准备成功后publish

---

# 273. Assets dir publish

final assets dir必须预先不存在。

rename staging assets dir。

---

# 274. Markdown publish

最后进行。

---

# 275. 为什么 Markdown最后

如果assets还未ready：

不允许MD出现broken refs。

---

# 276. Existing MD

可以用 Phase 4 atomic file replace能力。

这是用户明确选择的Export overwrite。

---

# 277. Export不是 guarded note save

不要拿工作 note的 OCC fingerprint。

---

# 278. Export failure before publish

删除自己的 staging目录。

---

# 279. Staging cleanup只能删：

> 本次Export自己创建并可证明ownership的 staging path。

---

# 280. Publish assets成功、MD失败

可以尝试删除本次刚创建且仍可验证owner的final assets dir。

---

# 281. 清理失败

留下：

```text
orphan export assets directory
```

比误删用户文件安全。

报告错误。

---

# 282. Export成功后

不更改：

```text
DocumentState
generation
saved_generation
working path
undo
```

---

# 283. Export不进入Undo

---

# 284. Export不改变dirty

---

# 285. Image Preview layout

正式替换 Phase 5 placeholder only。

---

# 286. 但是 remote仍placeholder

---

# 287. Managed Image Preview

active path：

```text
./note/images/<name>
```

---

# 288. 如果引用存在但file在trash

runtime reconcile应恢复。

Preview resolver可以触发：

```text
asset restore request
```

但不要直接move。

---

# 289. Image metadata缓存

可以与 decoded cache分开。

小型：

```text
path → dimensions/status
```

---

# 290. Metadata cache必须失效于Preview rebuild / changed metadata

---

# 291. Decoded cache 16MiB

Hard。

---

# 292. Source mode

切回Source：

建议：

```text
clear decoded image cache
```

---

# 293. Preview/Split再打开

lazy decode。

---

# 294. Math cache不受影响

图片和数学 cache独立。

---

# 295. 不建统一GenericResourceCache

除非现有架构已经自然统一。

默认不要。

---

# 296. Preview image memory gate

测：

```text
1 small image
20 small images
1 4K image
20 mixed images
```

---

# 297. Cache内部必须始终：

```text
<=16 MiB
```

---

# 298. Working Set可高于cache

因为：

```text
decoder
framebuffer
allocator
```

需要单独测量。

---

# 299. Image decode performance

Release测：

```text
PNG
JPEG
WebP
GIF first frame
screenshot PNG
```

至少 small/large。

---

# 300. First image cold/warm

分开。

---

# 301. Scroll lazy decode test

100 image文档。

首次Preview顶部：

只decode顶部附近。

---

# 302. instrument：

```text
images_total
metadata_inspected
decode_requested
decode_completed
cache_hits
cache_misses
cache_bytes
evictions
```

---

# 303. Scroll到下方

才decode后续。

---

# 304. Scroll回顶部

如果仍cache：

hit。

否则重新decode。

---

# 305. Resize

改变image layout size。

如果已有足够分辨率raster：

可以复用。

如果新的显示尺寸大于cached raster：

重新decode/resize。

---

# 306. DPI change

scaled preview可能需要重新raster。

---

# 307. 不改变Document generation

图片decode、resize、cache均是projection。

---

# 308. Asset Move不改变Document generation

文件位置 reconcile是side effect。

---

# 309. Paste Markdown insertion才改变generation

---

# 310. GC不改变generation

---

# 311. Export不改变generation

---

# 312. External note reload仍按Phase4

---

# 313. External note添加managed reference

如果 referenced managed file在trash：

reconcile restore。

---

# 314. External note删除managed reference

logical trash。

---

# 315. 外部文件编辑后的Undo

Phase4 external reload清Undo。

因此 asset effects也清。

---

# 316. External edit不能让old asset undo effect残留

测试。

---

# 317. Conflict状态

直到Load External前：

asset reference authority仍是local DocumentState。

---

# 318. 不根据 conflict external snapshot提前GC

---

# 319. Keep Local

local asset refs继续。

---

# 320. Load External

重新 reconcile assets。

---

# 321. Crash Recovery

恢复temp document之后：

asset reconcile基于最终选择的canonical text。

---

# 322. 不基于 discarded temp执行GC

---

# 323. Managed ImageRefScanner 初始化

startup document load后扫描。

---

# 324. Session state

保存：

```text
current managed ref counts
```

---

# 325. 每个edit后增量？

可以重新扫描完整文本。

---

# 326. 典型≤1MiB

完整literal scan成本很低。

简单优先。

---

# 327. Benchmark

1MiB managed scanner：

目标：

```text
p95 <5 ms
```

如果超：

分析。

---

# 328. 不为scanner引入复杂incremental parser

---

# 329. Multiple identical refs

count正确。

---

# 330. scanner false-positive fixture

例如 code block：

```markdown
`images/stickymd-abc....png`
```

必须：

```text
retain
```

---

# 331. scanner malformed Markdown

仍保守retain filename literal。

---

# 332. Asset filename path traversal

ManagedAssetId只生成basename。

不接受：

```text
../
/
\
:
```

---

# 333. 从Markdown解析出来的managed filename

先只提取 basename pattern。

所有 destructive op再canonical path验证。

---

# 334. Symlink attack

如果用户把：

```text
note/images
```

或managed file替换为symlink/reparse point：

自动 destructive操作必须保守。

---

# 335. Windows symlink/reparse安全

在删除/移动前：

确认 final target仍位于 canonical images/.trash boundary。

---

# 336. 不追随reparse point去删除目录外文件

hard security invariant。

---

# 337. 如果无法证明

不删除。

---

# 338. Directory junction同理

---

# 339. User external image read可以follow symlink

因为只是显式Markdown read。

---

# 340. 但写/delete必须boundary-safe

---

# 341. File overwrite

Managed store不得覆盖现有user file。

---

# 342. Hash collision fallback使用不同basename

---

# 343. Clipboard original file

StickyMD copy bytes。

不move原文件。

---

# 344. 不修改原file metadata

---

# 345. Image Read Error

Paste失败，不改变Document。

---

# 346. Export external read error

Export失败，不改变Document。

---

# 347. Tests — Managed Identity

至少：

```text
same bytes → same id
different bytes → different id
extension canonicalization
20hex
collision extension to32/64
filename parse
invalid names
hash mismatch ownership rejection
```

---

# 348. Tests — Ownership

```text
valid managed active
valid managed trash
managed-looking wrong hash
user file
symlink outside
junction/reparse outside
```

---

# 349. Tests — Scanner

至少：

```text
zero refs
one ref
three refs
remove one
0→1
1→0
code block conservative
raw text conservative
malformed Markdown conservative
```

---

# 350. Tests — Clipboard

使用 mock ClipboardImagePort。

至少：

```text
PNG file
JPEG file
WebP file
GIF file
bitmap→PNG
multiple images
mixed files
oversize
bad format
clipboard unavailable
```

---

# 351. Windows Clipboard Integration

真实：

```text
Snipping Tool
Paint
File Explorer PNG
File Explorer JPEG
browser copied image
```

如果环境允许。

---

# 352. 每种真实来源记录

```text
available clipboard formats
selected path
persisted extension
hash behavior
```

---

# 353. 真实项不能用mock冒充

没有条件：

```text
NOT TESTED
```

---

# 354. Tests — Paste OCC

```text
capture gen10
asset prepared
current gen11
```

Expected：

```text
no Markdown insertion
asset cleanup/trash
```

---

# 355. Tests — Paste failure

asset write fail：

```text
Document unchanged
generation unchanged
undo unchanged
```

---

# 356. Tests — Multi Paste partial fail

第二张失败：

```text
no Markdown insertion
first newly-created asset not left active
```

---

# 357. Tests — Undo/Redo

核心：

```text
paste
→ asset active
→ markdown ref
→ undo
→ ref gone
→ trash
→ redo
→ ref restored
→ active
```

---

# 358. Normal delete

```text
existing image ref
→ delete source syntax
→ trash
→ undo
→ restore
```

---

# 359. Multiple refs

删一个：

不trash。

删最后一个：

trash。

---

# 360. Undo restore I/O fail

Document text正确。

asset warning明确。

---

# 361. Redo trash fail

Document text正确。

多留active file。

---

# 362. Tests — Startup Reconcile

至少：

```text
referenced active
referenced trash
referenced missing
unreferenced active
unreferenced trash
user active
user trash
hash mismatch
duplicate active+trash same
duplicate active+trash mismatch
```

---

# 363. Tests — Normal Exit

final save成功：

trash cleanup。

final save失败：

不执行permanent GC。

---

# 364. Crash test

在：

```text
asset write完成
Markdown未save
```

模拟强杀。

重启应清orphan。

---

# 365. Crash between trash move andnote save

按durable note决定恢复/清理。

---

# 366. Tests — Image Decode

至少：

```text
PNG alpha
JPEG
WebP
GIF first frame
BMP
ICO
corrupt
oversized dimension
pixel overflow
encoded >64MiB
```

---

# 367. Cache test

超过16MiB：

evict。

---

# 368. Lazy decode test

100图，只decode viewport附近。

---

# 369. Preview missing/error isolation

单图错误不使Preview失败。

---

# 370. Remote zero network

Cargo tree + runtime test。

---

# 371. Tests — Export

至少：

```text
no images
one managed
duplicate managed refs
user relative image
external absolute image
remote image
missing local
reference-style image
image with title
image with alt
same content different source path
asset-dir collision
existing export md
export during dirty
export during conflict
```

---

# 372. Export Snapshot Consistency

导出开始后用户继续编辑：

输出应对应：

```text
captured generation
```

---

# 373. Export不追逐最新generation

这是普通snapshot operation。

---

# 374. 导出完成不改变dirty

测试。

---

# 375. 导出完成不改变undo

---

# 376. Export remote no network

---

# 377. Export path rewrite test

只local images重写。

---

# 378. Source preserving test

没有图片区域：

```text
byte-for-byte unchanged
```

---

# 379. Image syntax normalization test

只有必须的image nodes变化。

---

# 380. Reference-style shared ref test

普通link不能被误改。

---

# 381. Export asset dir existing

不删除existing。

生成suffix。

---

# 382. Export failure cleanup

只清自己 staging。

---

# 383. Performance — Managed Scan

20KiB/100KiB/1MiB。

---

# 384. Performance — Paste

分别：

```text
PNG 1MiB
JPEG 5MiB
screenshot 4K
```

拆：

```text
clipboard capture
inspect
hash
encode if needed
write
Document insert
```

---

# 385. UI thread latency

文件读取/PNG encode不得在UI thread。

---

# 386. Clipboard capture自身可能有短copy

必须测量。

---

# 387. Performance — Decode

记录：

```text
dimensions inspect
decode
resize
cache insert
```

---

# 388. Performance — Export

```text
10 images
50 images
```

---

# 389. Memory Baseline

必须补测：

```text
Source no images
Preview no images
Preview 1 small
Preview 20 small
Preview 1 4K
Preview image-cache saturated
Split image-cache saturated
Source after Preview
```

---

# 390. Image Cache memory hard invariant

内部：

```text
<=16MiB
```

---

# 391. Overall Working Set

继续观察原规格：

```text
Preview typical hard ≤52MiB
Split typical hard ≤64MiB
```

---

# 392. 4K image可能造成 transient peak

必须记录：

```text
steady
peak
```

---

# 393. 如果 peak巨大

分析decoder。

不得隐藏。

---

# 394. Idle CPU

图片Preview稳定后：

```text
<0.1%
```

---

# 395. 不做图片animation

因此idle不应增加。

---

# 396. Binary Size

记录：

```text
Phase6
Phase7
delta
```

---

# 397. Dependency delta review trigger

如果：

```text
+5MiB以上
```

分析 codec features。

---

# 398. `image` 默认feature误启用会是首要嫌疑

---

# 399. arboard image feature也审计

---

# 400. No Network dependency

执行：

```bash
cargo tree | rg \
"reqwest|hyper|ureq|curl"
```

预期无。

---

# 401. No heavyweight codec

执行类似：

```bash
cargo tree | rg \
"dav1d|rav1e|rayon|exr"
```

除非明确批准。

---

# 402. Image dependency report

创建：

```text
docs/report/phase-07-dependency-delta.md
```

---

# 403. 至少记录

```text
image
arboard changes
rfd if added
any codec crates
Windows APIs
```

---

# 404. Windows APIs

可能新增：

```text
OpenClipboard
CloseClipboard
EnumClipboardFormats
GetClipboardData
RegisterClipboardFormatW
DragQueryFileW
GlobalLock
GlobalUnlock
IFileSaveDialog / COM
```

以实际使用为准。

---

# 405. 不要为“可能用”启用全部 windows crate features

最小化。

---

# 406. Unsafe audit

```text
stickymd-core = 0
stickymd-render = 0
```

继续必须。

---

# 407. Clipboard unsafe

只能windows adapter。

---

# 408. File dialog unsafe

只能windows adapter或外部crate。

---

# 409. Asset core model不unsafe

---

# 410. Privacy

日志不得记录：

```text
image bytes
clipboard image
完整用户路径（release默认）
full Markdown
```

---

# 411. 可以记录：

```text
format
byte size
dimensions
hash prefix
managed/user classification
generation
```

---

# 412. 外部路径

Debug必要时可以脱敏。

---

# 413. Image Hash不算秘密

但只打印短prefix。

---

# 414. Security

Remote image零网络仍是hard invariant。

---

# 415. Image decoder面对不可信bytes

必须：

```text
limits
checked allocation
error isolation
```

---

# 416. 不使用 unsafe decoder自研

---

# 417. Path Write Boundary

StickyMD自动写入图片只能：

```text
./note/images/
./note/.trash/
export destination selected by USER
```

---

# 418. 不允许Markdown path控制写目标

---

# 419. Markdown local image只能被read

---

# 420. Managed basename由StickyMD生成

---

# 421. Export basename由StickyMD生成

---

# 422. Path traversal test

malicious Markdown：

```markdown
![](../../foo.png)
```

Preview可以read。

Export可以explicit copy。

但绝不能：

```text
write ../../...
```

---

# 423. Export destination sanitize

只用生成basename。

---

# 424. Symlink Export Destination

如果 destination parent存在复杂reparse：

按正常 USER-selected filesystem语义。

但 cleanup只删除自己可证明创建的staging。

---

# 425. Phase 7 Task

创建：

```text
docs/tasks/phase-07-managed-images-export.md
```

至少：

```text
Status
Prerequisites
Inherited Conditions
Scope
Out of Scope
Asset Authority
Ownership Proof
Clipboard Pipeline
Managed Persistence
Reference Scanner
Undo/Redo Effects
Preview Image Pipeline
Cache
Startup Reconciliation
GC
Export
Security
Performance
Manual Verification
Risks
Result
```

---

# 426. Phase 7 Report

创建：

```text
docs/report/phase-07-managed-images-export.md
```

---

# 427. Executive Result

必须：

```text
Managed Asset Identity:
PASS / CONDITIONAL / FAIL

Ownership Safety:
PASS / FAIL

Clipboard File Images:
PASS / CONDITIONAL / FAIL / NOT TESTED

Clipboard Screenshot:
PASS / CONDITIONAL / FAIL / NOT TESTED

Managed Paste:
PASS / CONDITIONAL / FAIL

Undo/Redo Asset Transaction:
PASS / CONDITIONAL / FAIL

Startup Reconciliation:
PASS / CONDITIONAL / FAIL

GC:
PASS / CONDITIONAL / FAIL

Native Image Preview:
PASS / CONDITIONAL / FAIL

Lazy Decode:
PASS / CONDITIONAL / FAIL

Decoded Cache:
PASS / CONDITIONAL / FAIL

Remote Zero-Network:
PASS / FAIL

Export:
PASS / CONDITIONAL / FAIL

Memory:
PASS / CONDITIONAL / FAIL

Idle CPU:
PASS / CONDITIONAL / FAIL

Visual:
PASS / CONDITIONAL / FAIL / NOT TESTED
```

---

# 428. Clipboard Evidence

列真实来源：

| Source | Available Formats | Selected Path | Stored Format | Result |
|---|---|---|---|---|
| Explorer PNG | | | | |
| Explorer JPEG | | | | |
| Snipping Tool | | | | |
| Paint | | | | |
| Browser | | | | |

没有真实测试必须写：

```text
NOT TESTED
```

---

# 429. Ownership Evidence

表：

```text
filename
hash match
location
classification
auto-delete allowed?
```

---

# 430. Undo Evidence

完整：

```text
paste
undo
redo
normal delete
undo restore
```

---

# 431. Reconciliation Evidence

startup matrix。

---

# 432. Preview Evidence

```text
formats
lazy decode
missing
corrupt
remote
large
```

---

# 433. Cache Evidence

```text
max bytes
hits
misses
evictions
```

---

# 434. Export Evidence

至少：

```text
snapshot generation
assets directory
rewritten paths
dedup
remote preservation
reference-style
failure cleanup
```

---

# 435. Performance Table

图片 paste：

| Input | Capture | Inspect | Hash/Encode | Persist | Document Insert |
|---|---:|---:|---:|---:|---:|

---

# 436. Decode Table

| Format/Size | Inspect | Decode | Resize | Cache |
|---|---:|---:|---:|---:|

---

# 437. Memory Table

完整。

---

# 438. Binary Delta

```text
Phase6:
Phase7:
Delta:
```

---

# 439. Unsafe

列所有 Windows adapter unsafe。

---

# 440. Architecture Authority

必须回答：

```text
Who owns Markdown text?
Who determines managed reference?
Who proves asset ownership?
Who performs file movement?
Can Preview delete assets?
Can ImageDecoder mutate Document?
Can GC use Preview AST as authority?
Can Export switch active document?
```

正确核心：

```text
DocumentState owns text
reference tracker derives from canonical text
ownership proof derives from path+name+content hash
I/O adapter moves files
Preview cannot delete
decoder cannot mutate
GC does not trust Preview
Export never switches active doc
```

---

# 441. Acceptance

Phase 7 应正式推进：

```text
AC-010 Image Paste
AC-011 Managed Image Undo
AC-012 User Image Safety
AC-017 Remote Image No Network
AC-018 Export
```

---

# 442. AC-010

图片 paste + managed file + Markdown insertion。

---

# 443. AC-011

完整 Undo/Redo。

---

# 444. AC-012

必须完整 PASS。

误删用户文件是 release blocker。

---

# 445. AC-017

继续完整 PASS。

---

# 446. AC-018

完整 Export workflow。

---

# 447. Phase 7 Visual Matrix

至少：

```text
IMG-VIS-001 PNG
IMG-VIS-002 JPEG
IMG-VIS-003 transparent PNG
IMG-VIS-004 screenshot
IMG-VIS-005 wide image
IMG-VIS-006 tall image
IMG-VIS-007 missing image
IMG-VIS-008 corrupt image
IMG-VIS-009 remote placeholder
IMG-VIS-010 Split with images
IMG-VIS-011 125% DPI
IMG-VIS-012 150% DPI
IMG-VIS-013 200% DPI
```

---

# 448. 无视觉能力

必须：

```text
NOT TESTED
```

---

# 449. Phase7 Architecture Review

必须回答：

1. Asset是不是已经变成“附件系统”？
2. Managed ownership是否真的可证明？
3. User file有没有任何自动删除路径？
4. GC是否错误依赖Preview？
5. Scanner是否保守？
6. Paste是否先写asset再改Document？
7. Paste stale generation是否安全？
8. Undo失败时会不会丢文本？
9. MoveToTrash失败时是否安全？
10. Startup是否先load document再GC？
11. normal exit是否先save再GC？
12. image decode是否在UI thread？
13. image decode cache是否bounded？
14. remote是否完全零网络？
15. Export是否不改变working document？
16. Export是否重新序列化整篇Markdown？
17. reference-style是否不会误改普通link？
18. staging cleanup是否可能删用户目录？
19. `image`是否启用了过多codec？
20. 有没有新增thread pool？
21. 是否破坏Phase6 Math memory？
22. 是否破坏Phase3 IME？
23. 是否破坏Phase4 Autosave/Conflict？
24. core/render unsafe是否仍为0？

---

# 450. Review Subagents

如果支持，最多3个。

### Reviewer 1

```text
Asset ownership / GC / Undo / crash consistency
```

### Reviewer 2

```text
Clipboard / image decoder / memory / security
```

### Reviewer 3

```text
Export / source rewrite / architecture boundaries
```

---

# 451. Risk Report Conditions

以下任一必须停止相关扩张并写 report：

### R1

无法在不误删user file情况下可靠证明managed ownership。

### R2

Image decode典型场景导致不可接受内存峰值。

### R3

Windows clipboard无法可靠区分 file-image / bitmap，导致原编码目标无法实现。

### R4

Undo asset side effect需要破坏Document public model。

### R5

Export image rewrite需要重写整个Markdown并产生明显格式损失。

### R6

Preview lazy image decode必须阻塞UI。

---

# 452. Risk File

例如：

```text
docs/report/phase-07-image-memory-risk.md
docs/report/phase-07-export-rewrite-risk.md
```

---

# 453. Automated Baseline

至少：

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

cargo test \
  -p stickymd-core \
  --release \
  --locked

cargo test \
  -p stickymd-render \
  --release \
  --locked

cargo test \
  -p stickymd-win \
  --release \
  --locked

cargo deny check

git diff --check
```

---

# 454. Smoke

建立：

```text
tools/smoke/phase-07.ps1
```

遵循现有 smoke 风格。

建议支持：

```powershell
tools/smoke/phase-07.ps1
tools/smoke/phase-07.ps1 -Performance
tools/smoke/phase-07.ps1 -Runtime
```

---

# 455. Full CI Smoke

```powershell
tools/smoke/all.ps1 -Ci
```

必须仍 PASS。

---

# 456. Dependency Scan

检查：

```bash
cargo tree | rg \
"reqwest|hyper|ureq|curl|dav1d|rav1e|rayon|exr"
```

根据已批准依赖人工判断。

---

# 457. Web Architecture Scan

```bash
cargo tree | rg \
"tauri|wry|webview|cef|chromium|tokio|wgpu"
```

不得新增。

---

# 458. Unsafe Scan

```bash
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
```

应：

```text
runtime unsafe = 0
```

---

# 459. Asset Destructive Call Audit

搜索：

```bash
rg \
"remove_file|remove_dir|rename|MoveFile|DeleteFile" \
apps crates
```

逐个确认：

> 是否有 ownership proof + boundary check。

---

# 460. 不允许裸 remove_file(managed-looking-path)

---

# 461. File IO Boundary Audit

UI modules不得：

```text
直接写 images/
直接delete .trash/
```

---

# 462. Runtime Manual Smoke

使用独立 portable目录。

至少：

1. 打开 StickyMD。
2. 粘贴 Snipping Tool截图。
3. 确认 PNG进入 `note/images/`。
4. 确认 Markdown自动插入。
5. Preview显示。
6. Undo。
7. 图片进入 `.trash`。
8. Redo。
9. 图片恢复。
10. 删除图片Markdown最后引用。
11. 图片进入trash。
12. 退出。
13. 重开。
14. 确认未引用trash已清理。
15. 手工放 `images/my-photo.png`。
16. 不引用。
17. 重启。
18. 确认它仍存在。
19. 添加对它的Markdown引用。
20. Preview显示。
21. 添加remote image URL。
22. 确认只placeholder。
23. Ctrl+Shift+S导出。
24. 确认 `.md + assets/`。
25. 原 `./note/note.md`仍active。

---

# 463. User Safety Smoke

必须人工做：

```text
images/user-important.png
```

然后经过：

```text
edit
undo
redo
restart
GC
export
```

最后确认：

```text
user-important.png
```

未被自动删/移。

这是 Phase7最重要人工安全测试之一。

---

# 464. Crash Smoke

图片刚paste后强杀。

重开。

根据durable note引用状态：

```text
restore or cleanup
```

不能产生错误永久删除。

---

# 465. Export Smoke

用：

```text
managed
user relative
external absolute
remote
reference-style
```

混合文档。

验证。

---

# 466. README

更新真实状态：

```text
Native Markdown/math preview, managed image paste and portable export are implemented in development.
Desktop docking/tray polish is still pending.
```

不得声称v1完成。

---

# 467. Plan Updates

主要：

```text
docs/plan/08_assets_and_export.md
```

补已验证实现。

必要时：

```text
04_runtime_state_model
05_document_persistence
06_markdown_math_rendering
```

只做真实 contract连接。

---

# 468. Terminology

确保正式术语：

```text
Managed Asset
User Asset
Ownership Proof
Asset Reference
Trash Asset
Asset Reconciliation
Export Snapshot
```

统一。

---

# 469. Overview

加入：

```text
Clipboard
↓
AssetCoordinator
↓
Managed Asset Store
↓
DocumentState reference
↓
Preview Image Resolver

DocumentState
↓
Asset Reference Tracker
↓
desired active/trash state
↓
I/O worker
```

---

# 470. Coverage Matrix

更新实际 code mapping。

---

# 471. Phase 7 Task

完成后：

```text
Status: Completed — awaiting USER review
```

如果真实 clipboard / visual未完成：

```text
Status: Implementation Complete — manual verification incomplete
```

---

# 472. Git Commit建议

如果初始clean：

```text
feat(assets): establish managed image ownership and storage

feat(assets): add clipboard image paste and undo transactions

feat(preview): add lazy native image rendering

feat(assets): add startup reconciliation and safe garbage collection

feat(export): export Markdown with local image assets

test(assets): verify ownership and lifecycle invariants

docs: record phase 7 asset and export results
```

无需机械遵循数量。

---

# 473. 不 Push

```text
push = no
```

除非 USER明确要求。

---

# 474. Phase 7 Definition of Done

只有全部成立才完成：

- [ ] USER批准Phase7。
- [ ] Phase6 inherited conditions保留。
- [ ] `images/` bootstrap。
- [ ] `.trash/` bootstrap。
- [ ] ManagedAssetId正式实现。
- [ ] SHA-256基于final encoded bytes。
- [ ] 20hex默认filename。
- [ ] 32/64 collision fallback。
- [ ] canonical extension。
- [ ] ownership proof包含path + filename + content hash。
- [ ] user file无法通过名字 alone被删除。
- [ ] managed-looking wrong hash不自动操作。
- [ ] reparse/symlink boundary safety。
- [ ] PNG原编码保留。
- [ ] JPEG原编码保留。
- [ ] WebP原编码保留。
- [ ] GIF原编码保留。
- [ ] screenshot→PNG。
- [ ] 其它批准格式→PNG。
- [ ] image crate default features关闭。
- [ ] codec features最小化。
- [ ] 没有AVIF/EXR等无关重codec。
- [ ] encoded size guard。
- [ ] dimensions guard。
- [ ] pixel count guard。
- [ ] checked decoded byte math。
- [ ] Windows CF_HDROP支持。
- [ ] native encoded clipboard路径审计。
- [ ] bitmap fallback。
- [ ] clipboard busy有限retry。
- [ ] paste不阻塞UI做文件读取/encode。
- [ ] paste OCC generation guard。
- [ ] asset先成功persist再插Markdown。
- [ ] paste失败Document不变。
- [ ] multi-image paste all-or-nothing。
- [ ] managed Markdown path使用 `/`。
- [ ] conservative reference scanner。
- [ ] multi-ref count正确。
- [ ] `1→0` logical trash。
- [ ] `0→1` restore。
- [ ] UndoEntry加入pure asset effect。
- [ ] Core不做filesystem。
- [ ] undo asset restore。
- [ ] redo asset trash。
- [ ] asset failure不回滚正确文本。
- [ ] I/O requests有界。
- [ ] note save不会被GC饿死。
- [ ] file watcher不会把images变化当note conflict。
- [ ] native local image preview。
- [ ] remote仍零网络。
- [ ] image metadata inspection。
- [ ] lazy decode。
- [ ] viewport-prefetch。
- [ ] decoded cache≤16MiB。
- [ ] image cache bounded。
- [ ] first-frame-only animation policy。
- [ ] transparent image alpha。
- [ ] aspect ratio保持。
- [ ] no-upscale默认。
- [ ] missing image局部fallback。
- [ ] corrupt image局部fallback。
- [ ] image failure不破坏Preview。
- [ ] startup先load/recovery再asset reconcile。
- [ ] referenced trash restore。
- [ ] unreferenced active→trash。
- [ ] unreferenced trash delete。
- [ ] user active永不删。
- [ ] user trash永不删。
- [ ] corrupt managed-looking永不删。
- [ ] final save失败时不做destructive exit GC。
- [ ] normal exit safe GC。
- [ ] crash后startup reconcile。
- [ ] Ctrl+Shift+S正式导出。
- [ ] UI名称为“导出”。
- [ ] Export不改变working path。
- [ ] Export snapshot current runtime text。
- [ ] dirty状态可导出。
- [ ] conflict状态可导出local text。
- [ ] remote image不下载。
- [ ] actual image nodes才export。
- [ ] local managed copy。
- [ ] local user copy。
- [ ] external absolute copy。
- [ ] dedup。
- [ ] missing local fail-before-publish。
- [ ] export assets dir不覆盖用户existing dir。
- [ ] automatic suffix。
- [ ] source-preserving Markdown rewrite。
- [ ] reference-style image支持。
- [ ] 普通link不被reference rewrite误改。
- [ ] staging。
- [ ] staging failure cleanup仅自己文件。
- [ ] Export成功不改generation。
- [ ] Export成功不改dirty。
- [ ] Export成功不改Undo。
- [ ] 1MiB scanner benchmark。
- [ ] image paste benchmark。
- [ ] decode benchmark。
- [ ] export benchmark。
- [ ] image cache memory测量。
- [ ] 4K image peak memory测量。
- [ ] idle CPU测量。
- [ ] binary delta测量。
- [ ] AC-010推进。
- [ ] AC-011推进。
- [ ] AC-012完整PASS。
- [ ] AC-017继续PASS。
- [ ] AC-018推进。
- [ ] core unsafe=0。
- [ ] render unsafe=0。
- [ ] Windows unsafe全部有SAFETY。
- [ ] 无network client。
- [ ] 无WebView。
- [ ] 无generic attachment system。
- [ ] docs更新。
- [ ] Phase7 task完成。
- [ ] Phase7 report完成。
- [ ] all smoke/CI通过。
- [ ] 人工项完成或诚实NOT TESTED。
- [ ] 未自动进入Phase8。

---

# 475. Final Recommendation

只能：

```text
APPROVE Phase 8
```

或：

```text
APPROVE Phase 8 WITH CONDITIONS
```

或：

```text
STOP — architecture review required
```

---

# 476. Phase 8 预定方向

如果 Phase 7 通过：

下一阶段应进入：

> **Windows Desktop Shell Finalization**

正式完成：

```text
paper window
Always-on-top
Light/System/Dark
70–100 opacity
Tray
Close→Hide to Tray
Left/Right/Top Dock
3 DIP sensor strip
100ms hover reveal
700ms focus-loss collapse
500ms hover-leave collapse
Esc/manual collapse
multi-monitor identity
DPI
monitor disconnect recovery
window persistence
fixed animation/shadow/rounding
```

即真正把已经可靠的 Markdown/Math/Asset 核心变成最终桌面便签行为。

但：

> 不得自动开始 Phase 8。

---

# 477. 最终回复格式

必须：

# Phase 7 Result

## Preconditions

```text
Phase 6 recommendation
USER approval
starting commit
inherited conditions
```

## Repository State Before Work

## Managed Asset Model

```text
filename grammar
hash
ownership proof
managed/user distinction
```

## Clipboard

### File Images

### Encoded Clipboard Images

### Bitmap/Screenshot

### Real Windows Sources

表格，并明确 PASS / NOT TESTED。

## Paste Transaction

```text
clipboard
→ prepare
→ persist
→ document insert
→ undo effect
```

## Undo / Redo

完整结果。

## Reference Tracking

```text
scanner
counts
false-positive safety
```

## Startup Reconciliation

完整状态矩阵。

## GC

```text
runtime logical deletion
exit physical cleanup
failure behavior
```

## Image Preview

```text
formats
lazy decode
dimensions
cache
remote
errors
```

## Export

```text
dialog
snapshot
assets dir
path rewriting
reference-style
dedup
failure cleanup
```

## Security

```text
ownership
symlink/reparse
path boundary
decoder limits
zero network
```

## Acceptance

```text
AC-010
AC-011
AC-012
AC-017
AC-018
```

## Performance

完整表。

## Memory

完整表。

## Idle CPU

## Binary Size

## Dependencies Added

## Windows APIs Added

## Unsafe

```text
core = 0
render = 0
windows adapter = ...
```

## Architecture Authority

明确回答。

## Visual Verification

PASS / FAIL / NOT TESTED。

## Architecture Drift

```text
None
```

或 Risk Report。

## Verification

全部命令。

## Documentation

## Git

```text
commit(s)
push = no
```

## Recommendation

三选一。

最后：

> Awaiting USER review. Do not start Phase 8 automatically.

完成后立即停止。
