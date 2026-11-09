# Phase 15 E：把不依赖文件语义的逻辑移到共享层

日期：2026-11-09。基线：`688a077`。USER 同日批准“只抽不依赖文件语义的部分”。

## 背景

macOS 宿主需要的启动恢复、配置规则、粘贴批次和导出规划，现在都在 `apps/stickymd-win`。不移动的话，macOS 要重写一份，以后两边还要同步修。

## 问题

最初的设想是把 `atomic_file`、`managed_file`、`file_identity` 收成统一的文件原语端口，macOS 只实现原语。清点后这条路走不通：

- Windows 的存储安全依赖三件事：打开时拒绝其他写者（共享模式）、按句柄改名、按句柄删除。POSIX 没有对等物，`flock` 只是建议锁，`rename` 和 `unlink` 都按路径。
- `03` 的规则 5 禁止共享层先检查路径、平台层再凭字符串操作。`05` 要求非 Windows 存储的保证在建立宿主时单独批准。

所以存储事务本身留在现有的 Capability Port 后面，由各宿主按自己的语义实现，用现有的 `storage_conformance!` 校验。可以共享的是这些事务之上的裁决。

## 设计

原则：共享层只做决定，碰磁盘的每一步由宿主用自己的句柄完成。Windows 的操作顺序不变。

| 项 | 进入 `stickymd-app` | 留在宿主 |
| --- | --- | --- |
| E1 配置 | `config.toml` 的解析、版本与语义裁决、序列化、保留文件的命名规则、加载结果与警告类型 | 读文件、不覆盖地改名、原子发布 |
| E2 启动恢复 | `bootstrap` 的全部裁决与 `BootstrapOutcome`，通过 `StartupStorage` trait 调用；`NoteObservation` | trait 实现；`StartupStorage` 还给出新笔记的换行符（Windows 为 CRLF） |
| E3 粘贴批次 | `prepare_and_store_paste` 的批次上限、失败时的回滚台账、Markdown 生成，通过 `PasteTarget` trait 调用 | 有界读取用户文件、发布受管资产 |
| E4 导出规划 | `ExportPlan`：同源与同内容去重、资产命名、引用改写、路径段编码 | 暂存、复制并哈希、发布、清理 |
| E5 路径解码 | 百分号解码 | Windows 路径语法校验 |

`StartupStorage` 与 `PasteTarget` 的每个方法都对应一次现有的完整操作，文件身份留在宿主内部，符合规则 5。错误用关联类型表达，`Display` 文本逐字不变。

新增依赖：`stickymd-app` 增加 `toml`（已是工作区依赖，纯解析）。共享 crate 不直接访问文件系统，治理规则不变。

不移动：

- `persistence/storage.rs`、`assets/storage.rs`、`safe_boundary.rs`：事务本身。
- `persistence/adapter.rs` 的失败分类：输入是各宿主自己的错误类型。
- `preview/image_source.rs`：约 40 行，依赖按平台打开文件，留待 macOS 再定。

合计移动约 550 行手写代码，少于最初估计的 1000 行，因为失败分类和路径校验都绑定平台。

## 验证

- Windows 现有测试保留原位，断言不改，经薄包装调用共享逻辑，用来证明行为没有漂移。
- 共享逻辑增加用内存假实现的裁决测试，Linux 也运行。
- fmt、clippy、全工作区测试、依赖检查、`phase-00`、`aarch64-apple-darwin` 的 `cargo check`。
- 独立跨模型审查；全量本地桌面预检；`config.toml` 字节不变，回退重开。

## 可选路径

| 方案 | 结果 |
| --- | --- |
| 本设计 | Windows 保证不降，macOS 复用裁决 |
| 不移动 | 约 550 行规则要在 macOS 重写，且没有 Windows 回归把关 |
| 统一文件原语端口 | macOS 上检查与操作之间出现窗口，并要改 `03` 规则 5，不采用 |

## 对规格的影响

`03` 的 Shared Application 职责增加这些裁决；`11` 的 Phase 15 里程碑增加 E；`phase-15` 验收表增加对应行。不改变存储合同，也不改变磁盘格式。
