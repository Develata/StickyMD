# 05_document_persistence.md - 文档持久化合同

## Metadata

- `Layer`: Runtime
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-30
- `Scope`: note.md 与 config.toml 的编码、换行、原子保存、崩溃恢复、外部修改处理、可写性与单实例身份

---

## Purpose

保证用户文本与配置在失败场景下不被静默丢失、不因正常保存失败产生半写 canonical 文件，
并维持“程序目录即便签身份”的 portable 模型。

<a id="persistence-authority"></a>
## Boundary

- 本章定义持久化契约；线程调度与 coordinator 结构见 `03`/`04`。
- 所有磁盘写入必须经 Execution Domain 的文件 adapter；UI 线程不做文件写入。
- 运行时 canonical 文本权威始终是 `DocumentState`；`note.md` 是 durable representation。
- watcher 事件只是待验证 hint；它和磁盘 bytes 都不能直接修改 Source projection。
- 保存输入只能来自带 generation 的不可变 Document Snapshot。

## Owned Objects

`file::note_md`、`file::config_toml`、`doc::snapshot`（写入输入）。

---

## 运行时目录模型

```text
<program-dir>/
├─ StickyMD.exe
└─ note/
   ├─ note.md          唯一工作文档
   ├─ config.toml      唯一配置
   ├─ images/          managed + user 图片
   ├─ .trash/          逻辑删除的 managed 图片
   ├─ note.md.tmp      仅在写入事务/崩溃残留中存在
   └─ crash.log        仅在 panic/关键故障时创建
```

<a id="startup-sequence"></a>
### 启动顺序与可写检查

创建 UI 前固定执行：

```text
解析并 canonicalize Program Directory
→ 建立同目录单实例
→ 验证 Program Directory 的 create/write/flush/delete 能力
→ 创建 note/ 及冻结目录布局
→ 加载/验证 config
→ 检查 recovery
→ 加载或安全创建 note.md
→ 建立 Durable Fingerprint
→ 创建 Source Editor
→ 启动 watcher
→ 启用 autosave
```

第二实例必须在任何 durable 文件创建前退出。可写检查在 Program Directory 内以唯一文件名执行
create-new、写 sentinel、flush、删除；删除失败也视为环境不可靠。失败时显示：

> 当前目录不可写，请将程序移动到有写权限的文件夹。

禁止 fallback 到 `%APPDATA%`、`%LOCALAPPDATA%`、Documents、注册表或任何用户配置目录。

<a id="program-directory-identity"></a>
### 单实例目录身份

- 实例身份 = canonical program directory：`current_exe()` → 父目录 → 解析 `.`/`..`
  → 解析 junction/symlink/reparse point → 统一 Windows 大小写语义 → SHA-256 → 本地命名对象。
- 同目录：第一个实例持有 named mutex；第二个实例通知第一个显示并激活，自己退出。
- 不同目录：哈希不同，可并行运行，互不干扰。
- 真实 canonical 路径继续用于 I/O；仅 identity key 归一化大小写，lower-case key 不得反向成为 I/O 路径。

---

<a id="text-encoding-newlines"></a>
## 文本编码与换行

| 项 | 规则 |
| --- | --- |
| 磁盘编码 | UTF-8 without BOM |
| 读取 | 兼容 UTF-8 BOM（剥除）；不得使用 lossy decode |
| 非 UTF-8 文件 | 不允许静默覆盖 |
| 首次创建换行 | CRLF |
| 已存在文件 | 保留原有主要换行风格 |
| 混合换行 | 按多数风格统一；数量相等用 CRLF |
| 孤立 `\r` | 保留为普通字符 |
| 内部表示 | UTF-8 + `\n`，仅在 load/save 边界转换 |

Durable Fingerprint 是实际磁盘 bytes 的 SHA-256，因此 BOM 与 CRLF/LF 的变化都属于 durable
事实变化，即使规范化后的文本相同也不做语义合并。

---

<a id="atomic-save"></a>
## Atomic Save

<a id="autosave-and-save-queue"></a>
### 调度与有界队列

```text
文本修改 → dirty = true → save_deadline = now + 650 ms
连续输入 → 只后移 deadline，不 snapshot
立即保存点 → Ctrl+S、窗口失焦、Hide to tray、正常退出、session shutdown、冲突“保留本地”
```

deadline 到达时才创建最新 Document Snapshot。单一 I/O worker 的 note 保存队列上界为：

```text
1 in-flight + 1 latest pending
```

pending 被更新 generation 覆盖；中间 generation 无需落盘。成功回执只确认实际落盘 generation，
若当前文档已有更新则仍 dirty，并直接调度最新状态。

### 目标文件已存在

1. 同目录创建/截断 `note.md.tmp`；canonical `note.md` 从不原地 truncate。
2. 写入完整 UTF-8 without BOM bytes。
3. flush Rust 用户态 buffer。
4. 在 temp 文件句柄上调用 `FlushFileBuffers`，然后关闭句柄。
5. 在 publish 前紧邻执行 guarded fingerprint 检查。
6. 调用 `ReplaceFileW(replaced=note.md, replacement=note.md.tmp, flags=0)`。
7. 成功后以写入的 durable bytes 更新 Durable Fingerprint，并确认对应 generation。

`REPLACEFILE_WRITE_THROUGH` 在 Windows API 中明确不受支持，禁止使用。`ReplaceFileW` 失败
必须分类并保留证据，不做 blanket `MoveFileExW` fallback：

| 错误 | 合同 |
| --- | --- |
| 1175 `ERROR_UNABLE_TO_REMOVE_REPLACED` | 检查 target/temp 状态并 fail closed |
| 1176 `ERROR_UNABLE_TO_MOVE_REPLACEMENT` | target 可能已不存在、temp 可能仍在；保留恢复证据 |
| 1177 `ERROR_UNABLE_TO_MOVE_REPLACEMENT_2` | partial/ambiguous mutation；禁止二次写入 |
| 其他错误 | 记录 target/temp 是否存在并返回 typed failure |

### 首次创建（目标不存在）

写入并完整 flush 同目录 temp 后，使用 `MoveFileExW(MOVEFILE_WRITE_THROUGH)` publish，
不带 `MOVEFILE_REPLACE_EXISTING`。不得先创建空 target。竞争中若 target 突然出现，必须
返回 conflict，不能覆盖未知外部内容。

### 乐观并发控制（OCC）

普通保存携带 expected Durable Fingerprint。I/O worker 在 temp 已写完并 flush 后、publish 前
重新读取 target：只有当前 fingerprint 等于 expected（或首次创建时仍不存在）才能写盘。
不匹配时返回 conflict，canonical target 不变。只有用户在冲突 UI 明确选择“保留本地”时，
`ForceOverwrite` 才能绕过 expected-base guard。检查与 replace 间仍有无法完全消除的极小
TOCTOU 窗口；v1 不通过长期独占文件锁破坏外部编辑能力。

### 禁止

- 原地 truncate canonical 文件后写入。
- 在 UI 线程保存或从 cosmic-text projection 取保存文本。
- 所有 `ReplaceFileW` 错误都 fallback 到 `MoveFileExW`。
- 无界保存队列或每按键一次全文 snapshot。
- 因 config 保存失败而损坏/停止 note.md 保存（两者是独立事务）。

### 崩溃与掉电边界

普通失败不会通过 truncate 产生半个 canonical 文件；publish 前的完整 temp 可被启动流程发现。
对于 Windows 报告为 partial/ambiguous 的替换失败，程序保留现场并 fail closed，不宣称 target
一定仍在原名。配置损坏不影响笔记。

本合同不保证突然终止时最后约 650 ms 的字符已经落盘，也不承诺所有硬件、驱动、文件系统
组合下的绝对掉电事务语义；Windows 没有在此被虚构成具备 Unix directory fsync 的等价保证。

---

<a id="recovery"></a>
## 启动恢复（note.md.tmp）

1. 在启动 watcher/autosave 前读取 `note.md` 与 `note.md.tmp` 的 existence、bytes、mtime、hash、UTF-8 状态。
2. temp 与 canonical hash 相同：canonical 成功加载后才删除冗余 temp。
3. valid temp 内容不同：产生 `RecoveryCandidate`，显示“恢复临时内容 / 使用当前文件”。
4. canonical 不存在但 temp valid：仍由用户决定，不自动恢复。
5. canonical invalid/过大且 temp valid：不得静默覆盖；恢复前先保留 canonical 证据。
6. temp invalid/过大：不得解码成空内容；在创建或复用固定事务 temp 前，先将证据隔离为
   `note.invalid-tmp-<timestamp>`，再明确警告。隔离失败则阻止启动，不能让后续 autosave
   truncate 该证据。
7. 用户选择前禁用 autosave/watcher effects并阻止普通编辑。
8. 选择恢复后，恢复文本在内存中保持 dirty，使用候选检测时的 canonical fingerprint 做
   guarded publish；只有真实成功回执才能 mark clean 并删除 temp。等待用户选择期间出现的
   外部修改必须转为 conflict，不能以恢复为名 ForceOverwrite。
9. 选择当前文件时，只有 canonical 成功加载后才删除 temp。

---

<a id="external-file-watch"></a>
## 外部文件监听

`notify` Windows backend 只非递归监听 `note/`；callback 只上报 `NoteFsHint` 或 watcher failure。
相关 raw events 以约 150 ms 合并，再重新读取 bytes 与 hash。event 与 mtime 都不是内容事实。

watcher 初始化/运行时失败可进入明确 degraded mode，但保存仍可用；publish 前 OCC guard 仍会
阻止静默覆盖外部修改。因此 watcher 是低延迟 UX 能力，不是唯一 correctness gate。

<a id="external-change-conflict"></a>
## 外部文件修改与冲突

### 自身写入

自己的原子替换也会产生 watcher events。重新读取 durable bytes；hash 与已知 Durable
Fingerprint 相同则忽略。禁止以固定时间窗口无条件吞掉文件事件。

### Buffer 干净

读取 → 校验 UTF-8/大小 → 与 known fingerprint 比较 → `DocumentState::load_external`
→ 清空 undo/redo → projection 全量 resync → 更新 Durable Fingerprint。外部 reload 是
reconciliation，不进入用户 Undo。

### Buffer 脏（Conflict）

进入一级 `FileConflict` 状态并暂停 autosave；冲突期间允许继续输入。后续外部变化更新
conflict 中的最新 external fact，但不改变 DocumentState。

- 载入外部：载入当前最新 external fact、清空 undo、更新 fingerprint、全量 resync、解除冲突。
- 保留本地：在用户明确选择时 snapshot 当前最新 generation，以 ForceOverwrite 原子保存；
  只有成功回执后才解除冲突。
- 无效 UTF-8/过大 external 不提供载入；只允许用户明确“保留本地”后覆盖。

### 外部删除

不清空内存。以 `expected=None` 的 guarded atomic publish 恢复 canonical note；若 target 在 publish
前重新出现则转为 conflict，不覆盖它。

### 短暂读取失败

sharing violation 等错误可有限重试 50/150/300 ms；之后进入明确读取失败状态，不无限重试。

---

<a id="config-persistence"></a>
## Config 持久化

```text
config.toml.tmp → write/flush/FlushFileBuffers → atomic publish
```

- schema `version = 1`；未知字段忽略，缺字段使用 default，非法枚举/值视为损坏。
- 解析失败：改名为 `config.invalid-<timestamp>.toml` 后使用 defaults；改名失败时保留原文件且不覆盖。
- version 高于当前理解范围时保留原文件、使用 defaults 并警告。
- 不提前建立 migration framework；对 version 做显式 match 即可。
- note 与 config 是独立事务；config 失败不得影响 note。
- config 外部变化 v1 不热重载。

---

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| 目录不可写 | 启动提示并退出，不 fallback |
| 磁盘写/flush 失败 | 明确 SaveFailed，保持运行和内存文本；temp 可保留 |
| `ReplaceFileW` 失败 | 分类 1175/1176/1177 与现场；保留证据，禁止 blanket fallback |
| guarded fingerprint mismatch | 不写 target，进入 conflict |
| 崩溃残留 temp | 启动恢复提示，用户决定 |
| 外部修改 + dirty | Conflict，autosave 暂停 |
| 外部删除 | 内存内容 guarded atomic 恢复 |
| 外部无效 UTF-8/过大 | 冲突态，不载入不自动覆盖 |
| config 损坏 | 尽力改名保留，defaults 启动，note 不受影响 |
| watcher 失败 | 明确 degraded；OCC guard 仍保护保存 |
| 第二实例唤醒失败 | 第二实例仍退出，不启动重复编辑会话 |

---

## Inputs

Document Snapshot（带 generation）、startup observations、watcher hint、I/O completion 与用户的
冲突/恢复决策。

## Outputs

durable note/config 更新、带实际 generation/fingerprint 的保存回执、external fact、冲突/恢复
状态或 typed failure。

## State Changes

所有重要状态只在主 coordination thread 串行提交。I/O worker 不持有 `&mut DocumentState`；
watcher/adapter 不修改 editor。保存回执只推进实际落盘 generation。

## Configuration

autosave 650 ms、external check 150 ms、换行检测与 16 MiB 自动载入安全上限是内部工程规则；
后者不是面向未来版本的不可变产品承诺。

## Lifecycle

正常退出若 dirty：请求立即保存并等待 worker；失败时取消退出并保留内存文本。worker 正常关闭
前排空已接受的 note/config 请求。最终 Close-to-tray 生命周期留给后续 Window/Tray phase。

## Extension / Replacement Points

原子替换、single-instance、watcher 是 Windows adapter；core 只理解 fingerprint、loaded document、
recovery candidate、external fact 与 conflict 等纯值模型。

## Performance Critical Paths

save encode/hash/write 在 I/O worker；UI 只做 deadline、snapshot handoff 与 typed result commit。
worker 栈与 snapshot 生命周期有界；成功/失败后释放旧 snapshot，不缓存历史保存文本。

## Verification

- 单元：encoding/newline、recovery、autosave virtual time、queue coalescing、OCC、conflict reducer、config。
- Windows integration：atomic replace、named mutex/event、notify hint、read-only/failure injection。
- portable/manual：AC-001/005/006/007/008/026/027/030、kill/recovery、Notepad external edit。

## Non-Goals

- 版本历史、WAL、数据库、持久化 undo。
- 跨目录同步或备份策略。
- note.md 之外的工作文档。
- 文件长期独占锁与自动语义 merge。
