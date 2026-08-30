# Phase 14 Module Success Ledger Architecture Report

## Status

`APPROVED BY USER — 2026-08-30`

## Background

Phase 14 原合同把 Runtime、Performance、Resources、G3、G4、G5 与 manual evidence
全部绑定到一个 Promoted Candidate。任何 candidate identity 变化都会让这些收据整体 stale；
同一通道的新运行又会覆盖固定路径上的旧文件。该模型安全但粒度过粗：只修改一个隔离的
qualification harness 或产品模块，也会要求重跑全部桌面、资源与人工矩阵；用户移动物理鼠标
导致的中止还可能覆盖一份已成功的结果。

## Observed Problem

1. candidate identity 表达“拟发布的确切 ZIP/EXE/SBOM 字节”，不能回答某个功能模块的相关输入
   是否变化。
2. fixed receipt path 同时承担 last attempt 与 last successful authority，失败或环境中止会破坏
   已有成功证据。
3. 按 source commit 整体失效无法利用现有高内聚测试模块，重复 GUI、Performance 与 Resources
   资格化耗时很高。
4. 若简单忽略所有失败并无条件复用旧 PASS，又会让旧代码或旧 harness 的结果掩盖当前输入变化。

## Alternatives

### A. 保持 candidate 全量失效

正确但成本最高；每次无关变化都机械重跑全部 artifact-bound 通道。

### B. 永远使用最近一次 PASS，不校验输入

实现最简单，但会把旧模块、旧 harness 或旧验收合同的 PASS 冒充当前事实，拒绝采用。

### C. 模块输入指纹 + last-success ledger

每个稳定测试模块声明产品输入、共享依赖、harness 与验收合同输入。Rust CLI 对排序后的实际
内容计算稳定 SHA-256；只有指纹相同的成功记录才可复用。每个模块只持久化一份最后成功记录，
成功后原子替换；失败或用户/环境中止不覆盖成功记录。USER 批准采用本方案。

## Approved Skeleton

### Authorities

- Promoted Candidate 继续拥有当前拟发布 ZIP/EXE/SBOM 的精确字节身份。
- Qualification Module Registry 是模块边界、输入集合、共享失效关系和 evidence class 的唯一权威。
- Last Successful Module Receipt 是该模块最近一次成功验证的唯一 durable evidence。
- Readiness 只消费当前输入指纹可兼容的成功记录；最近一次尝试不是 authority。

### Fingerprints

模块指纹至少包含：

```text
module id
+ sorted product input paths and bytes
+ sorted shared input paths and bytes
+ module harness paths and bytes
+ authoritative acceptance/plan inputs
+ applicable toolchain/manifest/lock inputs
+ exact artifact hashes when the evidence class requires exact bytes
```

文件时间、提交时间、目录枚举顺序和“最新 candidate”不进入指纹。无法分类的 tracked path
必须保守命中共享/全局失效集合，禁止默认忽略。

### State Transitions

```text
current fingerprint == last successful fingerprint
    -> REUSED PASS; do not launch the module

fingerprint differs or success is absent
    -> RUN REQUIRED
       -> PASS: archive evidence by content hash, then atomically replace last-success pointer
       -> FAIL / ABORTED: return non-zero; leave last-success receipt unchanged
```

失败和中止可以输出到当前命令的终端/临时诊断，但不得成为正式成功收据，也不得覆盖成功
ledger。成功 evidence 使用内容 hash 的不可变归档，ledger 原子切换后再清理旧归档，避免 ledger
更新失败时连旧成功证据也一并丢失。部分 Resources 结果仍可用于当前进程诊断，但不能替换完整模块成功记录。

### Exact-byte Boundary

package、checksum、SBOM、PE/native-runtime 与 downloaded/promoted identity 直接验证当前发布字节，
其模块指纹必须包含当前 ZIP/EXE/SBOM hash，因此新 artifact 必然要求重新验证。功能行为模块可以
跨 candidate 复用，但只能在其声明的产品、共享、harness 与 contract 输入指纹完全相同时复用，
并保留成功来源 candidate。

### Selected Candidate

Readiness 显式读取当前 Promoted Candidate；它不再要求每个功能模块的成功运行都发生在该
candidate 被 promote 之后。成功记录必须说明 origin candidate/source，并通过当前模块指纹兼容性
校验。tag/draft/publish 仍只使用当前 Promoted Candidate 的精确 artifact，不受模块复用影响。

## Failure Paths

- registry 缺少模块、输入路径不存在、指纹不可计算、ledger schema 损坏：fail closed。
- tracked path 无法分类：使保守共享集合 stale，不静默复用。
- last-success 指纹不同：显示 `RUN REQUIRED`，不能把旧 PASS 提升为当前 PASS。
- 运行 FAIL 或用户/环境 ABORTED：非零退出；成功记录保持原子不变。
- 写入新成功记录失败：本次不能报告 durable PASS，旧记录保持不变。
- exact-byte gate 的 candidate hash 不同：必须重跑，不允许 source-equivalence 转移。

## Migration And Rollback

- 旧固定收据只有在其状态完整为 PASS、身份合法，并能从记录的 source/harness 重建相同模块指纹时，
  才可显式导入；否则保持未资格化。
- 新 ledger 写入 ignored `dist/evidence/`，不修改产品配置或发布 ZIP。
- 回滚工具实现只需删除新 ledger projection；Promoted Candidate 与旧收据文件保持不变。

## Performance

指纹只在资格化计划/收据更新边界计算，不进入产品 runtime。实现必须按文件流式 hash，不把全部
仓库内容同时载入内存；同一计划内共享输入 digest 可缓存。相比启动 GUI、Performance 或 Resources，
该成本应可忽略，并通过 CLI 单元基准/测试约束为有界工作。

## Verification

- 未变输入复用最后成功且不启动 runner。
- 产品、共享、harness、contract、manifest/lock 任一适用输入变化会要求重跑。
- 无关模块变化不使本模块 stale。
- 未分类 tracked path 保守失效。
- PASS 原子更新；FAIL、ABORTED 与 unwind 不覆盖成功记录。
- exact-byte evidence 在 EXE/ZIP/SBOM hash 变化时永不复用。
- readiness 展示每个模块的 `RAN PASS` / `REUSED PASS`、origin candidate 与 fingerprint。

2026-08-30 实现验证：

- `cargo test -p stickymd-smoke --locked`：113 unit + 2 CLI tests PASS。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`：PASS。
- `cargo test --workspace --locked`：workspace PASS；Release-only ignored baselines 未在本次工具变更中重跑。
- `tools/smoke/phase-00.ps1`：governance PASS。
- `qualification modules`：六个新 ledger 均显示 `RUN_REQUIRED`，符合不隐式导入旧固定收据的迁移合同。

## Consequence Of Not Changing

任何 harness/docs 或隔离模块修复都会继续触发数小时的无关桌面与资源重验；物理桌面干扰会
反复破坏已有证据，最终鼓励人工绕过而不是可审计复用。

## Resolution

2026-08-30 USER 批准方案 C，并进一步确认：正式记录只保存每个模块最后一次成功运行；失败或
中止不覆盖，下一次成功自动更新。实现必须以模块相关输入指纹决定复用或重跑。
