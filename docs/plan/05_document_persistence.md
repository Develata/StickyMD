# 05_document_persistence.md - 文档持久化合同

## Metadata

- `Layer`: Runtime
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-20
- `Scope`: note.md 与 config.toml 的编码、换行、原子保存、崩溃恢复、外部修改处理、可写性与单实例身份

---

## Purpose

保证用户文本与配置在任何失败场景下不被静默丢失、不产生半写文件，
并维持“程序目录即便签身份”的 portable 模型。

## Boundary

- 本章定义持久化契约；线程调度与 coordinator 结构见 `03`/`04`。
- 所有磁盘写入必须经 Execution Domain 的文件 adapter；UI 线程不做文件写入。

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

### 目录可写检查（启动时，创建 UI 之前）

1. 创建 `./note/`。
2. 在其中创建测试文件、写入少量字节、flush、删除。
3. 失败 → 显示“当前目录不可写，请将程序移动到有写权限的文件夹。”并退出。

禁止 fallback 到 `%APPDATA%`、`%LOCALAPPDATA%`、Documents、注册表或任何用户配置目录。

### 单实例目录身份

- 实例身份 = canonical program directory：
  `current_exe()` → 父目录 → 解析 `.`/`..` → 解析 junction/symlink/reparse point
  → 统一大小写语义 → SHA-256 → 本地命名对象。
- 同目录：第一个实例持有 named mutex；第二个实例通知第一个显示并激活，自己退出。
- 不同目录：哈希不同，可并行运行，互不干扰。

---

<a id="text-encoding-newlines"></a>
## 文本编码与换行

| 项 | 规则 |
| --- | --- |
| 磁盘编码 | UTF-8 without BOM |
| 读取 | 兼容 UTF-8 BOM（剥除） |
| 非 UTF-8 文件 | 不允许静默覆盖（见外部修改节） |
| 首次创建换行 | CRLF |
| 已存在文件 | 保留原有主要换行风格 |
| 混合换行 | 按多数风格统一；数量相等用 CRLF |
| 内部表示 | UTF-8 + `\n`，保存时转换 |

---

<a id="atomic-save"></a>
## Atomic Save

### 调度

```text
文本修改 → dirty = true，save_deadline = now + 650 ms（连续输入后移 deadline）
立即保存点：Ctrl+S、窗口失焦、Hide to tray、程序退出、session shutdown、冲突“保留本地”
```

### 步骤（目标文件已存在时）

1. 同目录创建 `note.md.tmp`。
2. 写入完整 UTF-8 内容。
3. flush 用户态 buffer。
4. 调用 `FlushFileBuffers`。
5. `ReplaceFileW` 原子替换。
6. 只有 adapter 将失败分类为“目标不存在”或“ReplaceFileW 不适用于该目标、且 temp 与
   target 已确认仍位于同一目录”时，才允许使用 `MoveFileExW`
   （`REPLACE_EXISTING | WRITE_THROUGH`）。权限、共享冲突、路径身份变化或未知错误不得
   无条件降级，必须保留原文件并上报 typed error。
7. 更新磁盘 hash 与 saved_generation。
8. 删除残留 temp。

### 首次创建（目标不存在）

写入并完整 flush 同目录 temp 后，直接使用同目录、write-through 的原子 rename/move 建立
target；不得先创建空 target，也不得经过 truncate + in-place write。竞争中若 target 突然
出现，adapter 必须重新分类，不能静默覆盖未知外部内容。

### 禁止

- 原地 truncate 后写入。
- 在 UI 线程保存。
- 保存半个文件。
- 因 config 保存失败而损坏 note.md（两者是独立事务）。

### 保存期间的新修改

完成当前保存 → 立即保存最新 generation；中间 generation 可合并；
`saved_generation` 只推进到实际落盘版本。

### 崩溃恢复边界

保证：正常保存不留半文件；原文件在替换前始终存在；启动可检测有效 temp；配置损坏不影响笔记。
不保证：进程被突然终止时最后约 650 ms 的每个字符都已落盘；断电期间未触发 autosave 的输入。

---

## 启动恢复（note.md.tmp）

1. 发现 `note.md.tmp` → 校验合法 UTF-8。
2. 比较 mtime 与 hash。
3. temp 更新且内容不同 → 显示薄恢复提示：

   ```text
   发现未完成保存的内容
   [恢复临时内容] [使用当前文件]
   ```

4. 用户选择前：不覆盖任何文件、暂停 autosave、内存仅保留最小必要数据。

---

## 外部文件修改与冲突

### 识别自身写入

watcher 会观测到程序自己的原子替换。通过 `last_saved_hash` + 保存 generation +
短期 write token 识别；hash 相同 → 忽略。

### Buffer 干净时外部变化

读取新内容 → 校验 UTF-8 → 更新 DocumentState → 清空 undo → 更新 base hash →
reconcile 图片 → preview dirty。不弹阻塞对话框。

### Buffer 脏时外部变化（Conflict）

```text
SaveState → Conflict；autosave 暂停
banner：文件已在外部修改 [载入外部] [保留本地]
```

- 载入外部：丢弃未保存 buffer、载入、清空 undo、更新 hash、reconcile assets。
- 保留本地：原子覆盖外部文件、更新 hash、保留 undo、解除冲突、恢复 autosave。
- 冲突期间允许继续输入。

### 外部删除

不清空内存、不用空白文件覆盖内存；用当前内存内容原子恢复 canonical note.md；
显示短暂非阻塞提示。

### 无效 UTF-8（外部）

不载入、不自动覆盖；进入冲突状态，提示“外部文件不是有效 UTF-8”；
允许“保留本地覆盖”；用户也可先导出保存内存内容。

---

## Config 持久化

```text
config.toml.tmp → flush → atomic replace
```

- 未知字段忽略（向前兼容）；缺字段用默认值。
- 解析失败：`config.toml` → `config.invalid-<timestamp>.toml`，默认配置启动，
  短暂提示，不影响 note.md，不覆盖损坏文件。

---

## Failure Paths

| 场景 | 行为 |
| --- | --- |
| 目录不可写 | 启动提示并退出，不 fallback |
| 磁盘写失败 | SaveState::Failed，明确报错，保持运行，内存文本不丢 |
| 替换失败 | 安全条件下 MoveFileEx 回退；仍失败则报错 |
| 崩溃残留 temp | 启动恢复提示，用户决定 |
| 外部修改 + dirty | Conflict，autosave 暂停，等待用户 |
| 外部删除 | 内存内容原子恢复 |
| 外部无效 UTF-8 | 冲突态，不载入不覆盖 |
| config 损坏 | 改名保留，默认启动 |
| 读取到非 UTF-8 note.md | 不允许静默覆盖；按冲突类处理并提示 |

---

## Inputs

DocumentState immutable snapshot（带 generation）、外部文件事件、启动扫描与用户的冲突/
恢复决策。

## Outputs

磁盘 `note.md` / `config.toml` 原子更新、带实际 persisted generation/hash 的保存回执、
冲突/恢复事件或 typed failure。

## State Changes

保存回执只推进其实际落盘 generation；若当前文档已继续编辑则仍保持 dirty。外部事实必须
先进入 reload/conflict/recovery 协调流程，不能由 watcher 或 adapter 直接改写 DocumentState。

## Configuration

保存 debounce（约 650 ms）、换行风格检测均为内部固定行为；用户可见配置见 ConfigState。

## Lifecycle

启动序列与退出序列见 `04_runtime_state_model.md#Lifecycle`；
退出时保存失败必须显示明确错误并保持程序运行，不静默退出。

## Extension / Replacement Points

原子替换的平台实现位于 adapter；核心契约（temp → flush → replace）不变。

## Performance Critical Paths

保存在 I/O worker 串行执行；不得阻塞 UI 线程；watcher 回调只发送轻量事件。

## Verification

- 故障注入测试：写失败、替换失败、kill 进程后 temp 恢复、config 损坏、外部删除、
  无效 UTF-8、双实例、无写权限。
- 验收：AC-001/005/006/007/008/026/027/030。

## Non-Goals

- 版本历史 / 快照浏览。
- 跨目录同步或备份策略。
- note.md 之外的文档持久化。
