# Phase 1E Spike — Portable Persistence（目录身份 / 单实例 / 原子保存 / 崩溃恢复 / 冲突规则）

> 实验性代码。本目录不属于生产 workspace，可随时删除。
> plan_ref: docs/plan/05_document_persistence.md ; docs/plan/05_document_persistence.md#atomic-save

## 1. 验证目标（来自 Phase 1 任务 1E）

- canonical program directory → SHA-256 身份（解析 junction/symlink/reparse，大小写归一）。
- 命名 mutex + 命名 event 的单实例模型：第二实例检测到已存在并激活第一实例。
- 启动可写性检查（create → write → flush → delete）。
- 原子保存：`note.md.tmp → flush → FlushFileBuffers → ReplaceFileW`（失败回退
  `MoveFileExW(REPLACE_EXISTING|WRITE_THROUGH)`），保存后无 temp 残留、无半文件。
- 崩溃恢复候选检测（合法 UTF-8 / 相同 / 无效 / 缺失）。
- 外部修改识别自身写入 / clean 重载 / dirty 冲突 的规则。
- 换行风格检测与保存转换（内部 `\n`，磁盘按多数风格）。

## 2. 环境

| 项 | 值 |
| --- | --- |
| OS | Windows 11 x64 |
| 工具链 | Rust 1.97.1（MSVC） |
| 依赖 | sha2 0.10 / windows 0.62.2（Foundation/Security/Storage_FileSystem/System_Threading/System_IO） |
| 构建/测试 | `cargo build --release`、`cargo test --release`（独立 crate，`[workspace]` 空） |

## 3. 结果

### 3.1 目录身份（canonical → SHA-256）：PASS

```
canonical(base) = \\?\C:\Users\QQ\AppData\Local\Temp\stickymd-spike-demo-<pid>-<nanos>
id(base)        = Local\StickyMD-f3a29d8a…c521   (64-hex)
stable(repeat)  = true      # 同一目录重复解析哈希一致
distinct(subdir)= true      # base 与 base/note 哈希不同
```

- `GetFinalPathNameByHandleW` 返回 `\\?\` 规范 NT 路径；`identity_name` 先 `to_ascii_lowercase`
  再 SHA-256，保证大小写不敏感（单测 `identity_is_case_insensitive_and_stable` 验证）。
- ⚠ 说明：本 spike 在**普通临时目录**上验证；真实 junction/symlink 的解析路径代码已写
  （`GetFinalPathNameByHandleW` 本身即负责展开 reparse point），但**未用真实 junction 实测**。

### 3.2 单实例（命名 mutex + 激活 event）：PASS

```
first instance acquired mutex
child: PROBE=SECOND_INSTANCE_DETECTED
child: ACTIVATE_EVENT_SIGNALED
first instance observed activate signal = PASS
```

- 主进程持有 `CreateMutexW`（名字 = 身份哈希 + 后缀），并 `CreateEventW` 建激活事件。
- 以子进程方式启动自身（`--second-instance-probe`）：子进程 `CreateMutexW` 得到
  `ERROR_ALREADY_EXISTS` → 判定为第二实例 → `OpenEventW + SetEvent` 通知第一实例 → 退出。
- 第一实例 `WaitForSingleObject(event, 1000ms)` 观测到信号。
- 单测 `single_instance_second_is_detected` 另证：同进程内第二次 `try_acquire_instance` 亦返回 Err。

### 3.3 可写性检查：PASS

```
writable(note/)   = PASS                     # create→write→flush→delete 成功
writable(blocked) = PASS (rejected as expected)  # 失败注入：路径位于一个文件之下，无法 create_dir
```

- ⚠ 说明：失败注入用「目录路径被一个普通文件占据 → create_dir 失败」模拟；**未用真实
  权限拒绝（ACL/只读卷）实测**，因为该场景需特定权限环境。契约行为（失败→退出、不 fallback）
  由生产启动流程承担，本 spike 仅证明检测函数能区分可写/不可写。

### 3.4 原子保存：PASS

```
v1 landed, no temp leftover = true     # 首次保存（target 不存在 → MoveFileExW 分支建立）
v2 replaced v1 atomically    = true     # 二次保存（target 存在 → ReplaceFileW 分支）
newline style of saved v2    = Crlf     # 内部 \n → 磁盘 CRLF 转换生效
```

- 流程：`File::create(tmp) → write_all → FlushFileBuffers → ReplaceFileW(target,tmp)`，
  ReplaceFileW 失败时回退 `MoveFileExW(REPLACE_EXISTING|WRITE_THROUGH)`，最后确保无 temp 残留。
- 保存后 `sha256(note.md)` 与写入内容一致，`note.md.tmp` 不存在（无半文件、无残留）。
- ⚠ 说明：首次保存走 MoveFileExW 回退分支（因 target 尚不存在，ReplaceFileW 必然失败），
  二次保存走 ReplaceFileW 主分支——两条路径均被覆盖；但**未在运行中打印具体走了哪条分支**，
  仅由「target 是否存在」推断。断电级持久性（FlushFileBuffers 后掉电）**无法在本机验证**。

### 3.5 崩溃恢复检测：PASS（逻辑 + 落盘候选）

```
temp differs        -> OfferRecovery   # 合法 UTF-8 且与当前不同 → 提示用户（不自动覆盖）
temp identical      -> CleanStale      # 与当前相同 → 直接清理
temp invalid utf-8  -> DiscardTemp     # 非 UTF-8 → 丢弃
no temp             -> None
```

- ⚠ 说明：本 spike 用「手工遗留 temp」模拟崩溃残留并验证决策逻辑；**未通过真实 kill 进程
  在原子保存中途制造残留**来端到端验证。决策逻辑本身有单测覆盖（`recovery_decisions` 等）。

### 3.6 外部修改 / 冲突规则：PASS

```
own write echoed   -> Ignore    # 观测哈希 == 上次保存哈希 → 自身写入回显，忽略
external + clean   -> Reload    # 外部变化且 buffer 干净 → 静默重载
external + dirty   -> Conflict  # 外部变化且 buffer 脏 → 冲突，暂停 autosave
```

- 纯逻辑（`logic::decide_external_change`），单测覆盖三种分支 + None-hash 场景。

### 3.7 测试与静态检查

- `cargo test --release`：**18 passed; 0 failed**（含逻辑单测 + 文件系统/单实例集成测试）。
- `cargo clippy --release --all-targets`：**0 warnings**。

## 4. 结论

| 项 | 判定 |
| --- | --- |
| canonical dir → SHA-256 身份（大小写归一、稳定、可区分） | **PASS** |
| junction/symlink 真实解析 | 代码就绪，**NOT TESTED（需真实 junction）** |
| 单实例 mutex + 激活 event（跨进程） | **PASS** |
| 可写性检查（成功 + 失败检测） | **PASS** |
| 权限拒绝（ACL/只读卷）路径 | **NOT TESTED（需特定权限环境）** |
| 原子保存（无残留、无半文件、CRLF 转换） | **PASS** |
| FlushFileBuffers 断电持久性 | **NOT TESTED（需硬件级验证）** |
| 崩溃恢复决策逻辑 | **PASS（逻辑）**；真实 kill 中途残留 **NOT TESTED** |
| 外部修改 / 冲突规则 | **PASS** |

判定：**PASS（附 4 项环境/硬件限制，均为 NOT TESTED 而非 FAIL）**。
持久化原语（身份、单实例、原子保存、恢复/冲突规则）可作为生产 Execution Domain 文件
adapter 的基础；生产化时需：真实 junction 复测、ACL 拒绝复测、kill-mid-save 端到端、
以及与 watcher 集成的 write-token 去抖。

## 5. 复现

```powershell
cd experiments/phase-01/persistence
cargo run --release     # 端到端演示（自动清理临时目录）
cargo test --release    # 18 项单测/集成测试
cargo clippy --release --all-targets
```
