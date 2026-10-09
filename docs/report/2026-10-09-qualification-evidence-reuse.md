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

## 对规格的影响

plan 11 `#module-success-ledger` 更新存储位置、按指纹记录、规范化与文档分类、G5 归档和 origin 比较；
phase-14 验收更新 P14-A35 并新增 P14-A67..A69；coverage matrix 与 release checklist 同步。
