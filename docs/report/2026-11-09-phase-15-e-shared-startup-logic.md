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

## Resolution

2026-11-09，独立跨模型审查指出两个阻断项，核实后成立，设计按下面修订，以本节为准。

**阻断项。** 把 `bootstrap` 的“观察、裁决、按路径删除或隔离”原样放进共享层，就成了 `03` 规则 5 禁止的形式：共享层根据先前的观察裁决，宿主再凭路径执行。导出同理，哈希之后按路径改名，`ExportPlan` 不能成为这一步的授权。

**修订。**

| 项 | 修订后 |
| --- | --- |
| E2 启动恢复 | 不用 `StartupStorage` trait。共享层只提供纯函数 `plan_note_startup`：输入两份观察与新笔记的换行符，输出要做的步骤。观察、隔离、删除、首次创建都由宿主在自己的流程内执行，操作顺序不变，首次创建继续用 `Guarded { expected: None }` |
| E4 导出规划 | `ExportPlan` 只返回命名与复用决定，不持有文件。源键由宿主解析后提供，共享层不做路径规范化或大小写折叠。暂存、复制并哈希、改名、发布、清理、工作笔记目标检查、资产目录选择都留在宿主，顺序不变。扩展名的纯命名规则移入共享层 |
| E2 的 `BootstrapOutcome` | 留在宿主；`into_session` 构造 Windows 剪贴板，不进入共享层 |
| E5 | 只移动百分号解码；`file:///` 处理、解码失败回退和路径校验留在宿主 |
| E1、E3 | 不变。E3 的错误类型保留全部现有变体，文件解码失败仍归 `MixedOrUnsupportedFiles` |

**验证补充。**

- E1：读取失败、解析失败、版本更新、保留失败。
- E2：每种观察组合的裁决表，CRLF 与 LF 两组换行。
- E3：每种输入分支、失败台账、完整错误文本。
- E4：同源复用、同内容不同扩展名的命名、路径段编码。
- Windows 现有测试保留原位且断言不变，用来约束宿主的执行顺序。

**已有问题，本批不改。** Windows 的 `remove_temporary`、`quarantine_temporary` 与导出暂存文件的改名都按路径执行，观察之后被其他进程替换的窗口今天就存在。修订后这些代码仍在宿主内，行为不变。

macOS 上的具体保证（`AlreadyExists` 的映射、不覆盖改名等）不由本设计断言，按 `05` 在建立宿主时单独批准。

移动量随之减少到约 400 行手写代码。
