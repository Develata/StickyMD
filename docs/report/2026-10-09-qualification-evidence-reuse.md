# 资格化证据跨版本复用：根因与第一阶段修复（2026-10-09）

## 背景与授权

USER 希望"模块没有被动过，对应测试就不再跑"。2026-08-30 批准的 module success ledger
（[报告](phase-14-module-success-ledger.md)）已实现，但发版时从未生效：v0.1.3 候选
（`972aad4`）的 `qualification readiness --explain` 中 10 个功能模块全部是
`RUN_REQUIRED REASON=NO_LAST_SUCCESS`，每次发版仍需数小时全量重跑。

2026-10-09 USER 批准按以下四个根因修复，完成后交 gpt-6-astra 独立审查。

## 已核实根因

1. **账本位置**：成功记录存于各 worktree 自己被忽略的 `dist/evidence/module-success/`；每次发版新建
   隔离 worktree（`tmp/release-vX-*/source`），账本必然为空。v0.1.2 发版目录没有账本，v0.1.3 只有当天新跑的
   2 项，主工作区的 4 份记录来自更早源码，从未被发版目录读到。
2. **指纹失效面过大**：`Cargo.toml`/`Cargo.lock` 是全局输入，每次改版本号即全部失效；release notes、
   checklist、coverage matrix、README 等未分类路径落入保守兜底 `GLOBAL`。
3. **人工项不在账本**：21 项 Tier A/B 人工收据绑定当前 candidate。
4. **域划分粗**：`EDITOR`/`PREVIEW` 进入几乎所有模块。

此外，代码核对发现两处读取路径会让复用失效：G3/G4/G5 readiness 要求证据 `version` 等于**当前**
candidate；G5 截图只在当前 worktree 的 `dist/evidence/g5-artifacts/` 中查找。

## 第一阶段（本次）实施

按独立审查建议拆成两个单元。本次为 U1，覆盖根因 1、2 和上述两处读取路径：

- **克隆级共享存储**：`<git common dir>/stickymd/qualification-ledger/`，所有 linked worktree 共用；
  记录按 `(module, fingerprint)` 存放，不同指纹并存、按指纹直接查找；每模块保留最近 8 份并保护 24 小时内
  的写入；同指纹重跑替换自身记录。git 不可用时 fail closed。
- **G5 companion 归档**：截图按 SHA-256 校验后归档到共享存储，readiness 从共享存储校验。
- **origin 比较**：G3/G4/G5 复用证据的 version 与记录 origin 比较；当前 candidate 的一致性仍由
  Source Freeze 与 exact-byte 门持有。G3–G5 readiness 不再接收无用的 candidate 参数。
- **指纹 v2**：功能模块对根 manifest 的 workspace version 与 lock 中 workspace member 的 version 做严格
  白名单规范化，语法外写法整体回退原始字节；Source Freeze、workspace-tests identity 与 exact-byte 门保持原始字节。
- **文档分类**：发布说明、README/CHANGELOG 等说明文档、AGENTS 指南、coverage matrix、release checklist、
  `docs/{adr,overview,features}/`、README 图片与许可证文本不使功能模块失效。经检索，这些文件只被
  governance 检查或打包读取，不被任何功能模块执行；`docs/plan/` 与 `docs/acceptance-cases/` 因跨域约束仍对
  全部模块保守传播。

旧的 `dist/evidence/module-success/` 不再是 authority，不导入（指纹算法已变化），仍作为保留路径拒绝写入。
因此**第一次使用新工具的版本仍需一次全量基线**，此后同类发版才开始节省时间。

## 未纳入本次的部分

- **U2：人工项复用**。独立审查给出四项 BLOCKER，须先解决：用例到调用链的依赖闭包（例如
  `app/input.rs` 同时分发窗口、Preview 选择与滚轮，按用例名称收窄域会漏失效）；完整环境身份（含 UBR、IME、
  显示拓扑，不能删掉补丁号）；同指纹后续 `MANUAL_FAIL` 必须优先阻断；M34 Clean VM 保持 exact-artifact。
  同时保留 Tier C 语义、candidate 绑定的 waiver 与当前 candidate 汇总。若采用保守闭包（全部产品域），
  产品代码一改人工项仍全部重测，这一点需如实接受。
- **根因 4：拆细域**。不在没有调用链证据时拆分 `EDITOR`。解锁条件是产品侧按域拆分输入分发
  （例如 `app/input.rs`），再以依赖闭包证明各检查的实际输入。

## 验证

- `cargo test -p stickymd-smoke --locked`：359 + 25 PASS，15 ignored（未计为执行）。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`：PASS；`cargo fmt --all -- --check`：PASS。
- 新增回归：`module_ledger::reuse_tests`（主 worktree 记录全部功能模块 → 版本号/发布文档变更并提交 →
  新建 linked worktree：全部 `REUSED_PASS`；产品代码改动后全部 `RUN REQUIRED`；G5 截图从共享存储校验，
  篡改拒绝登记；不同指纹记录并存）、`fingerprint::normalize::tests`（版本规范化与 9 类 lock、7 类 manifest
  回退写法）。
- 当前指纹批量计算实测约 0.34–0.46 s（debug，5 个模块读取 487 个文件、约 4.5 MB），未做算法重写。
- 未执行：真实候选上的桌面资格化与人工验收。

## 独立审查与修正（gpt-6-astra max，`972aad4..f2ccdcc`）

审查给出 3 项 BLOCKER、2 项 SHOULD_FIX、1 项 NEEDS_VERIFICATION，均已对照源码核实并修正：

1. **规范化在真实仓库不生效**（属实）：`apps/stickymd-win/Cargo.toml` 的
   `[target.'cfg(windows)'.dependencies]` 等带引号表头被拒绝，整体回退原始字节，发版仍全部失效；原测试只用
   简化 manifest。修正：表头支持带引号的 dotted key；新增"真实仓库 manifest 必须在语法内"的守卫测试，以及
   复制真实四个 member manifest 与 lock 做版本升级、断言十个功能指纹不变而原始 workspace identity 改变的回归。
2. **GC 竞态与不完整扫描**（属实）：改为存储级 OS 文件锁（`File::lock`/`lock_shared`，进程退出即释放）。
   归档、发布与清理串行；读取方持共享锁跟随记录到 evidence。去掉 24 小时 mtime 宽限与同指纹立即删除；清理
   遇到任何不可读记录、目录项或 evidence 即停止删除并报告。
3. **诊断可覆盖共享存储**（属实）：整个共享存储（含 `.lock`）加入保留路径，沿用既有别名归一化
   （junction、8.3、`..`），并补充相应测试。
4. **同指纹立即删除旧 evidence**：已由第 2 条的锁与统一清理取代。
5. **plan 10 特例与新合同不一致**（属实）：删除该特例，`docs/plan/` 统一向全部功能模块传播。
6. **readiness 消费链未被证明**：新增从 linked worktree 执行真实 `g5_readiness::check` 的回归，覆盖
   截图只存在于共享存储、篡改记录 origin version 后阻断、删除归档截图后阻断。

未采纳为本次修改：审查提到 readiness 中各模块单独计算指纹（约 11 次），属性能优化且非阻断，留待实测后决定。

修正后：`cargo test -p stickymd-smoke --locked` 368 + 25 PASS（15 ignored 未计为执行）。

## 第二轮审查与修正（`f2ccdcc..52c2b72`）

复审确认上轮第 1、4、5、6 项已解决，第 2、3 项仍不完整，并新增以下问题；均已核实并修正：

1. **G5 截图在共享锁释放后才校验**，且身份与截图各读一次 success（属实）：新增
   `with_compatible_success`，G5 在同一把共享锁、同一份 success 快照内完成身份与全部截图校验。
2. **截图 GC 只认当前四个 case**（属实）：较新工具记录的 `G5-05` 截图会被较旧工具清理。改为从
   `results[].artifacts[]` 结构化提取；条目损坏即拒绝登记或停止清理。
3. **git 查询失败时保留路径检查放行**（属实）：改为在别名解析后的路径上匹配
   `/stickymd/qualification-ledger/` 路径段（忽略大小写），不再依赖 git。
4. **登记时多次读取工作区收据**（属实）：读取一次，对同一份字节完成校验、摘要与归档（新增内存
   SHA-256：Windows CNG，其他平台 `sha256sum -`）；截图同理。
5. **同秒或时钟回拨时可删除刚发布的记录**（属实）：清理显式保留当前记录。
6. **锁无等待上限**（属实）：`try_lock*` 加 120 s 上限，超时返回可诊断错误。
7. **被忽略的真实仓库性能比较失效**（属实）：比较双方都使用相同的规范化语义。

## 第三轮审查与修正（`52c2b72..43cc20a`）

复审确认第二轮 7 项全部解决，并提出以下问题，均已核实并修正：

1. **G5 按固定字符串切分 case**（属实）：合法 JSON 若在对象间加空格，前一个 case 会计入后面 case 的截图。
   改为严格 JSON 解析，每个 case 只统计自身对象内的截图，并要求每个 case 恰好出现一次。新增"带空格、前三项
   零截图"的反例回归。
2. **GC 采信未校验的 evidence**（属实）：清理前严格解析整条记录，并校验 evidence 字节与记录摘要一致；仍是
   合法 JSON 但摘要不符的归档，或不完整的记录，都会让清理停止。
3. **存储根被 junction 重定向时路径段检查失效**（属实，条件性）：在路径段检查之外，从 `.git` 元数据文件
   （`gitdir:` 与 `commondir`，不运行 git）定位存储根并解析 junction，再做包含检查；`.git` 无法解释时拒绝写入。
   新增 Windows junction 回归。
4. **同一 manifest 的并发替换竞态**（属实，属推断机制）：发布在替换失败时有界重试（5 次指数退避），并接受
   已是完全相同内容的 manifest；其他失败照常上报。
5. **临时名测试不确定**：两个命名入口都改为可注入时钟值，新增"相同时钟值仍得到不同名字"的确定性回归。
   历史失败的根因仍属推断。

## 第四轮审查（`43cc20a..b5af3e0`）

结论 **PASS / no blocker**：上轮第 1、2、4、5 项确认解决。第 3 项（共享存储路径保护）另有两个条件性缺口，
已核实并修正：

1. **无效 `.git` 指针被当作有效**（属实）：空 `gitdir:`、不存在的目标或无效 `commondir` 现在均视为无法解释，
   保留路径检查因此拒绝写入。新增空指针、失效目标、空/失效 `commondir` 与非 gitfile 的回归。
2. **存储子目录被 junction 重定向**（属实）：存储在取得读/写锁后检查根以下所有条目，发现 junction 或符号链接
   即拒绝读写；新增 Windows 回归（`evidence/` 被替换为 junction 后读取与登记均失败）。

## 第五轮确认审查（`b5af3e0..3632a88`）

结论 **PASS / no blocker**。核实并处理了三个边界问题：

1. **悬空链接**：`.git` 或 `commondir` 本身是悬空链接时曾被当作"不存在"。现在用 `symlink_metadata` 区分，
   只有目录项确实不存在才回退；新增悬空 `.git` junction 回归。
2. **`.lock` 链接**：打开锁文件会跟随链接。现在打开前先拒绝链接形式的 `.lock`，新增"链接目标不会被创建"的回归。
3. **持锁期间外部替换目录**：路径级 API 无法消除这一竞态。plan 11 明确保证范围为遵守锁协议的进程与静态
   链接；不遵守锁协议的外部进程不在防御范围内。
4. **扫描开销**（NEEDS_VERIFICATION）：新增被忽略的计时 profile，约 900 个条目时每次取锁平均 4.8 ms（debug）。

## 偶发的并发发布测试失败

v0.1.3 预检和本轮完整测试各出现一次
`release::package_publish::tests::concurrent_publication_never_overwrites_a_winner` 失败（同字节并发发布
只成功一次）。之后单独连跑 40 次、发布测试高并发连跑 25 次、完整套件连跑 4 次均未复现。读代码找到两个
确定存在的竞态：`atomic_evidence` 的临时文件名与 `release::temporary` 的临时目录名都只由进程号加纳秒组成，
同一进程内两个线程读到同一时钟值时会生成同名路径，后到的一方在 CreateNew/create_dir 处失败，这与该测试
"同字节两方都应成功"的失败形态一致。按指纹模块已有做法加入进程内自增序号，并新增 8 线程临时名唯一性
回归；测试失败时现在会打印双方结果。根因属于推断（未在失败当次取得错误文本）。

## 对规格的影响

plan 11 `#module-success-ledger` 更新存储位置、按指纹记录、规范化与文档分类、G5 归档和 origin 比较；
phase-14 验收更新 P14-A35 并新增 P14-A67..A69；coverage matrix 与 release checklist 同步。

## 最终状态（2026-10-09，`972aad4..7965350`）

本节以追加方式更正前文已被后续修正取代的表述，前文保持原样。

- **范围**：U1 完成，共 9 个提交（8 个修正 + 1 个按职责拆分）。根因 1、2 与两处读取路径已修复；U2（人工项）与
  根因 4 维持"未纳入本次的部分"所述状态，未开始实施。
- **取代"第一阶段"中的保留规则**：24 小时 mtime 保护与同指纹立即删除已在第一轮审查后移除。现行规则是
  存储级 OS 锁下每模块保留最近 8 份记录，并始终保留刚发布的记录；任何记录、目录项或 evidence 不可读或摘要
  不符时，清理停止且不删除。
- **取代"验证"中的计数**：`cargo test -p stickymd-smoke --locked` 380 + 25 PASS，16 ignored（显式计时
  profile 与真实环境用例，未计为执行）；`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`
  PASS；`cargo fmt -p stickymd-smoke --check` PASS；`tools/smoke/phase-00.ps1` PASS。
- **真实 CLI**：主工作区 `cargo run -p stickymd-smoke --locked -- qualification modules` 输出
  `LEDGER_STORE=E:/gitclone/StickyMD/.git\stickymd\qualification-ledger`，10 个功能模块均为
  `RUN_REQUIRED REASON=NO_LAST_SUCCESS`。这是预期结果：存储为空，第一次使用新工具的版本需要一次全量基线。
  共享存储不存在时，该命令不会创建它。
- **结构**：`module_ledger.rs` 拆为记录与查找权威（424 行）、`paths.rs`（路径身份与保留路径，202 行）、
  `status.rs`（只读状态报告，73 行），另有 `store.rs`（387 行）和 `fingerprint/normalize.rs`（415 行），均低于
  约 500 行的审视线。调用方通过 re-export 不变。
- **审查**：gpt-6-astra max 共五轮，第四、五轮结论 PASS / no blocker；拆分提交为纯移动，未单独送审。
- **未验证**：
  - 分支未推送，Windows CI 未运行。
  - junction 回归依赖 runner 上的 `cmd /C mklink /J`。
  - 并发发布偶发失败的根因仍属推断。
  - 真实候选上的桌面资格化与人工验收均未执行。

## 第六轮：整体架构与性能审计（gpt-6-astra max，`972aad4..3c01f5d`）

针对 USER 的质量标准（高内聚低耦合、性能、内存、算法、架构整洁）做整体审计。结论 **DOES-NOT-MEET**：
1 项 BLOCKER、5 项 SHOULD_FIX、1 项 NICE_TO_HAVE、1 项 NEEDS_VERIFICATION。逐项对照源码核实，均属实，
修正如下（`3c01f5d..eab875d`）：

1. **BLOCKER：成功判定使用文本扫描器**（属实）。G3/G4/G5 的 case 状态、`worktree_dirty` 与账本记录按
   `"key":` 首次出现读取：合法 JSON `"status" : "FAILED"` 旁边嵌套一个 `"status":"PASSED"` 可以放行；截断的
   账本记录仍能读出全部字段。修正（`3fdeabb`）：新增 `module_evidence.rs`，按模块对整份文档严格解析，每个值
   只从所属对象读取；`module_ledger/record.rs` 以固定字段集严格解析记录，查找与清理共用。
2. **复用与 readiness 标准不同**（属实）。归档截图缺失时 readiness 阻断，正式 G5 命令却继续跳过重跑。修正
   （`3fdeabb`）：查找结果分为 `Missing`/`Invalid`/`Valid`，在同一把共享锁内校验记录、evidence 与全部
   companion；无效时报告 `INVALID_LAST_SUCCESS` 并允许重跑，重跑登记替换记录并补回归档。
3. **规范化没有整体回退**（属实）。lock 不在语法内时根 manifest 仍被规范化，单独改 workspace version 可保持
   指纹不变；lock 重复键被接受。修正（`86e47a1`）：root、member manifest 与 lock 在规划时作为一组判定，任一
   失败两份都用原始字节；只有与判定时逐字节相同的内容才规范化；重复键拒绝。新增指纹级回归。
4. **模块依赖环**（属实）。`module_ledger → g5_readiness → module_ledger`、`module_ledger → smoke_scope →
   module_ledger`。修正：`module_registry.rs` 与 `path_identity.rs` 移出账本（`8e31d61`）；evidence 合同移入
   中立模块，`exact_readiness`、`g3/g4/g5_readiness` 删除，store 改为账本私有（`3fdeabb`）。
5. **重复计算**（属实）。readiness 逐模块计算指纹后状态输出又全部重算。修正：readiness 与状态报告共用一次
   `lookup_all` 快照（`3fdeabb`）；资源登记的指纹计算从 3 次降到 2 次（`8e09cdb`）。
6. **非 Windows SHA 子进程未回收**（属实）。修正（`eab875d`）：写入线程与有界等待，超时或出错时终止并回收，
   停止失败一并报告。

语义变化需要说明：登记现在与 readiness 使用同一合同，原先能登记、只在 readiness 被拒的不完整或身份不符的
收据，现在登记即失败（`exact_readiness` 原测试改为断言登记被拒）。`smoke_scope::validate_task_coverage` 并入
evidence 合同。

另外，Linux CI 首次回放发现 `readers_and_writers_exclude_each_other_through_the_os_lock` 偶发失败：Unix 的
flock 属于打开文件描述，并行测试派生的子进程在 exec 前持有写锁描述符的副本，释放会短暂滞后。测试改为有界
重试（`117ade0`）；生产路径本就有 120 s 等待。

### 实测（Windows，debug）

| 项目 | 逐模块 | 批量/快照 |
| --- | --- | --- |
| 真实仓库 10 个模块指纹 | 2.76–3.97 s，读 4469 次 / 36.8 MB | 0.39–0.57 s，读 492 次 / 4.04 MB |
| 10 个模块全部有记录时的查找（fixture） | 1.61–1.84 s | 0.21–0.29 s |

前文"约 0.34–0.46 s（5 个模块）"来自只覆盖资源组的旧 profile，以本表为准。

### 验证

- Windows：`cargo test -p stickymd-smoke --locked` 390 + 25 PASS，17 ignored（显式 profile 与真实环境用例，未计为
  执行）；clippy `-D warnings`、fmt、`tools/smoke/phase-00.ps1` PASS。
- Linux（WSL Debian，与 CI "Linux smoke CLI" 相同命令）：clippy 无警告；测试 320 + 19 PASS，连跑 3 次。

### 未处理

- 仍使用文本扫描器的读取面：`manual_readiness`、`decisions`、`source_freeze` 与 candidate 收据解析
  （`qualification/json.rs`）。不属于账本成功判定，本轮未改，留待审查评估。
- `governance.rs` 约 1182 行（既有问题，NICE_TO_HAVE），未拆分。
- 待下一轮讨论：`tools/stickymd-smoke/src/` 下的任何文件都归入 `ALL_HARNESS` 或 `GLOBAL`，工具本身的改动会使
  全部十个模块失效；工具几乎每个版本都会改，这决定复用在实际中能否生效。

## 第七轮：复审（gpt-6-astra max，`3c01f5d..13886dd`）

第六轮的六项中，四项判定 RESOLVED（账本严格解析、复用与 readiness 统一、整组规范化、重复计算），两项
PARTIALLY RESOLVED（依赖环、SHA 子进程回收）。另有 1 项 BLOCKER、2 项 SHOULD_FIX、2 项 NICE_TO_HAVE，
均已核实，处理如下（`13886dd..` 本轮提交）：

1. **BLOCKER：release readiness 仍用文本扫描器**（属实）。decision 收据里重新加空格的 `"status" : "USER
   REJECTED"` 旁嵌套一个 `"USER APPROVED"`，会被读成批准；downloaded 与 manual 收据同理。修正（`ffb3b70`）：
   `qualification/json.rs` 只保留整份严格解析与"从给定对象读字段"；candidate、Source Freeze、decision、
   remote/downloaded、manual 与 startup attribution 全部改为从所属对象读取。startup attribution 所需的测量名
   必须恰好出现一次；真实收据中 `task.execution_seconds` 每个任务各有一份，因此不能一律拒绝重复名。回归测试
   都以完整收据为基线，一次只改一个字段。
2. **headless 收据只要完成标记即可通过**（属实）。修正（`92a1cda`）：结果 id 必须按顺序等于 runner 为
   `all --ci` 规划的任务加完成标记。该计划与 v0.1.3 候选的真实收据逐项一致（19/19）。
3. **SHA 子进程的异常出口**（属实，条件性）。修正（`f3ee7b4`）：内存字节写入独占创建的私有临时文件，交给
   `sha256sum` 读取，不再需要管道和写入线程；每次外部摘要都有上限等待，超时或等待出错时终止并回收，失败
   的每一步都报告。新增"挂起、失败、成功"三种命令的测试。
4. **指纹分类**（第七轮问题 C）。修正（`f1405ba`）：`module_ledger/status.rs` 与 `qualification/readiness.rs`
   只汇报或汇总已有判定，不再是功能模块输入；evidence 接受规则（`module_evidence`、注册表、账本、记录格式）
   仍为全局。
5. **畸形 JSON 测试基线本身无效**（NICE，属实）。修正（`f1405ba`）：基线改为完整六项 G4 收据。

未处理：

- **`module_evidence → runner` 依赖**（NICE）：只调用纯规划函数，没有运行时递归。把任务规划移到中立模块会
  牵动 runner 主体，留到下一轮评估。
- **`governance.rs` 约 1182 行**：既有问题，未拆分。
- **完整 readiness 的耗时与峰值内存**（NEEDS_VERIFICATION）：需要真实候选与十个模块的归档，本轮无法测量。

### 验证

- Windows：`cargo test -p stickymd-smoke --locked` 394 + 25 PASS，17 ignored；clippy `-D warnings`、fmt、
  phase-00 PASS。
- Linux（WSL Debian，CI "Linux smoke CLI" 命令）：clippy 无警告；测试 324 + 19 PASS，连跑 3 次。

## 第八轮：第三次复审（gpt-6-astra max，`13886dd..504bdb6`）

第一次尝试在约 10 分钟后以退出码 1 结束，没有给出最终结论，按规则视为未审查。它在中途指出两处线索，核实
属实后修正（`504bdb6`）：续录人工验收时未逐 case 校验来源与 EXE，重复的 case 会互相覆盖；非 Windows 的哈希
快照改为仅本人可读写。重试的结论是 **PASS / no blocker**：第七轮的 JSON 读取、headless 完整性、测试基线和
人工 case 身份四项判定 RESOLVED，SHA 出口判定 PARTIALLY RESOLVED，规划依赖环未处理（可延期）。另有 3 项
SHOULD_FIX 要求在推送前修正，均已核实：

1. **续录与 readiness 校验不一致**（属实，既有问题）：schema 未知或 Windows 版本为 `UNKNOWN` 的收据，readiness
   会拒绝，续录却能载入并重新生成这些元信息。修正（`4635fe3`）：两条路径都只通过
   `manual_receipt::validated_cases` 接受收据，并删除 `manual_readiness` 中重复的 tier 模型与 case 解析。
2. **SHA 超时后的回收没有期限**（属实）：修正（`138469d`）：终止后的回收有独立上限，超时报告"未回收"而不阻塞。
3. **快照名可能撞上崩溃残留**（属实）：修正（`138469d`）：名称加入进程启动 nonce，遇到已存在的名称跳过并换名，
   残留文件不被覆盖；新增针对该路径的测试。

同时处理了两项 NICE：`sha256sum` 的 stderr 改为继承；startup attribution 只接受有限、非负的毫秒值，溢出为
无穷大的 JSON 数值被拒绝。

仍未处理：`module_evidence → runner` 的纯规划依赖（审查确认可延期）、`governance.rs` 体量，以及完整 readiness
的端到端耗时与峰值内存（需要真实候选与十个模块的归档）。

### 验证

- Windows：393 + 25 PASS，17 ignored；clippy、fmt、phase-00 PASS。
- Linux（WSL Debian，CI 命令）：clippy 无警告；324 + 19 PASS，连跑 3 次。

## 第九轮：确认审查（gpt-6-astra max，`504bdb6..6887e05`）

结论 **PASS / no blocker**：SHA 回收上限与快照换名判定 RESOLVED；人工收据统一校验判定 PARTIALLY RESOLVED。
推送前唯一要求是下面第 1 项，已核实并修正（`cb122c6`）：

1. **Windows build 可以没有构建号**（属实，既有问题）：`ver` 的输出不是 UTF-8 时（GBK 控制台默认如此，本机
   实测为 `[版本 10.0.26200.9457]` 的 GBK 字节），记录值回退为 `OS` 变量 `Windows_NT`；校验只拒绝空值与
   `UNKNOWN`。修正：新增 `qualification/windows_build.rs`，对 `ver` 有损解码后只保留 ASCII 版本号
   （`Microsoft Windows 10.0.26200.9457`），读不到即记为 `UNKNOWN`；人工收据与 exact evidence 只接受含版本与
   构建号的值。本机测试确认能读出已知构建号。
2. **逐 case 身份测试到不了逐 case 检查**（NICE，属实）：修正为顶层身份保持合法，只改单个 case 的 source 或
   EXE，并断言具体的 case 错误。

验证：Windows 395 + 25 PASS，17 ignored；Linux（CI 命令）clippy 无警告，325 + 19 PASS，连跑 3 次；fmt、phase-00 PASS。
