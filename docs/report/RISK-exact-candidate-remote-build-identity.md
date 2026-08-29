# RISK — Exact Candidate 与远端独立构建身份不一致

**日期：2026-08-29**  
**状态：APPROVED — Option B implementation authorized**  
**适用阶段：Phase 14 release qualification closure**

## Executive Result

当前 `downloaded-artifact` gate 无法按既有模型关闭。原因不是远端包损坏，而是工具同时要求：

1. 本地构建的 ZIP/EXE 是 exact candidate；
2. GitHub Windows runner 从同一 source commit 独立构建；
3. 下载后的独立构建必须与本地 ZIP/EXE 逐字节相同。

Phase 9 已明确只证明“同一个已构建 EXE 输入下，ZIP 生成确定”，并明确不声称独立
Rust/linker build 可逐字节复现。当前 downloaded verifier 因此把一个未建立的 reproducible-build
前提升级成 release hard gate，和已有报告及实际 Windows toolchain 行为冲突。

不得通过删除哈希比较或把远端包视作与本地 exact artifact 等价来绕过。这样会让本地 GUI、性能、
资源和人工证据绑定一个 EXE，却发布另一个未经相同资格化的 EXE。

## Reproduction

候选身份：

- source commit：`cd9c1c7e1f5aa5df714bf23ded86905a56032483`
- local ZIP：`StickyMD-0.1.0-local-rc-cd9c1c7e1f5a-windows-x64-portable.zip`
- local ZIP SHA-256：`c5ac7b579e31382e0e061410b9b631ee849d849b9fa6b8814c17dcb254db0699`
- local EXE SHA-256：`5c21d46e3831af1511bbb41b325a01524fd6692861a1918770b2b5a5021ad167`

远端执行：

- CI run：`33256817160`，attempt 1，6/6 jobs PASS
- release diagnostic run：`33257534796`，attempt 1，PASS
- artifact id：`9716585230`
- remote ZIP SHA-256：`54cfba66a3e3281fd904e1ce844b1e7986e8a3bedae6ae48acc97ecef189e6b4`
- remote EXE SHA-256：`1dbbad63862688a9d5e084c75b32c07f7ec5d7b726c0dcde6a401ad90e13a07c`

执行：

```powershell
.\tools\smoke\phase-14.ps1 `
  -DownloadedZip dist\remote-release-33257534796\StickyMD-0.1.0-local-rc-cd9c1c7e1f5a-windows-x64-portable.zip
```

结果：

```text
stickymd-smoke: downloaded ZIP hash differs from the exact local candidate
```

## Data

### 远端包自身正确

下载后的远端 ZIP 使用它自己的 `SHA256SUMS.txt`、SBOM 和当前仓库 verifier 通过：

```text
PACKAGE_RUNTIME=PASS (ASCII, space, Chinese, same-directory and different-directory)
PACKAGE_VERIFY=PASS
```

所以该失败不是包结构、便携运行时、同目录单实例或 checksum corruption。

### 成员差异

| Member | Local bytes | Remote bytes | Result |
| --- | ---: | ---: | --- |
| `LICENSE.txt` | 1,086 | 1,086 | byte-identical |
| `README.txt` | 1,038 | 1,038 | byte-identical |
| `THIRD_PARTY_NOTICES.txt` | 1,829,345 | 1,829,345 | byte-identical |
| `StickyMD.exe` | 8,543,744 | 8,535,040 | different |
| `licenses/SIL-OFL-1.1.txt` | 4,068 | 4,153 | different |
| `licenses/KaTeX-fonts-NOTICE.txt` | 447 | 459 | different |

两个 ZIP 的 entry 顺序和固定 `1980-01-01` timestamp 一致。ZIP 层本身的确定性实现有效；差异来自
staging 输入。

### 文本输入差异

两份字体许可文件当前只有 `text=auto`，未冻结 `eol`。本机 `core.autocrlf=true`，而 checkout
环境产生了不同的工作树行尾。因此 `Copy-Item` 把工作树字节直接带入包中。

这是局部、可直接修复的 packaging drift：release tooling 应从 canonical source bytes 生成固定
UTF-8/LF 或明确冻结的 durable bytes，而不应依赖 checkout 的行尾策略。

### PE 输入差异

两个 EXE 均使用 Rust 1.97.1 / LLVM 22.1.6 source toolchain，但 PE header 显示：

| Field | Local | GitHub runner |
| --- | --- | --- |
| MSVC linker version | 14.44 | 14.51 |
| PE timestamp | `2026-08-28 14:32:44` | `2026-08-29 10:50:16` |
| initialized data | `0x272800` | `0x270000` |
| image size | `0x82D000` | `0x82B000` |

因此即使统一 ZIP metadata 和文本行尾，当前两个独立 linker invocation 仍不会产生相同 EXE。
Phase 9 报告对此已经准确标为 `Independent Rust/linker build reproducibility: NOT TESTED`。

## Root Cause

资格化模型缺少两个不同对象：

```text
local preflight build
    用来尽早验证 frozen source/tooling

release exact artifact
    remote workflow 实际生成、下载、随后接受完整 artifact-bound qualification 的唯一发布对象
```

当前 `Candidate` 把 `target/release/stickymd-win.exe` 和本地 ZIP 固定为唯一候选，同时
`remote::verify_downloaded` 又要求远端独立构建与它逐字节相同。该设计只有在完整 Windows linker、
SDK、resource compiler、checkout bytes 和 deterministic-link flags 全部被冻结并被证明可复现时才成立。
仓库没有建立这些前提，并且已有报告明确没有作此声明。

## Options

### A. 冻结完整 Windows reproducible-build toolchain

工作包括：

- 固定 linker、Windows SDK、resource compiler 和其安装来源；
- 加入并验证 deterministic PE/link flags；
- 处理 checkout path、PE/PDB metadata、resource COFF 与行尾；
- 在至少本机和 GitHub runner 上证明 clean independent builds bit-identical；
- 继续保留当前 downloaded exact-hash gate。

优点：获得真正的可复现构建。  
缺点：工作量和供应链维护成本高；MSVC/Windows SDK pinning 脆弱；这不是当前 v0.1.0 已批准的
release contract，不能假设短期可完成。

### B. 远端产物晋升为唯一 release exact candidate（推荐）

建议资格化顺序：

```text
source freeze
→ local/source preflight
→ push + CI
→ release workflow build/package/verify
→ download artifact
→ verify checksum/SBOM/package/runtime
→ promote downloaded ZIP + extracted EXE as exact candidate
→ Runtime / Performance / Resources / G3 / G4 / G5
→ human acceptance
→ readiness
```

要求：

- 新增明确的 `RemoteArtifactCandidate`/等价值对象，不把 `target/release` Cargo 输出冒充远端 artifact；
- ignored staging 使用固定位置，例如 `dist/exact-candidate/`，receipt 记录 source SHA、workflow
  run/attempt、artifact id、ZIP/EXE/SBOM SHA-256；不得记录机器绝对路径；
- 所有 artifact-bound runner 从同一个 candidate resolver 取得 EXE/ZIP，禁止各模块硬编码
  `target/release/stickymd-win.exe`；
- local preflight receipt 只证明 source/tooling，不声称验证最终发布字节；
- downloaded artifact 通过 package/runtime verifier 后才能晋升；
- 晋升后重新生成并运行所有 EXE/ZIP-bound receipts；旧本地 EXE receipts 必须变 stale；
- G3/G4/G5、人工观察、performance/resources 最终都运行将要发布的远端 EXE；
- 远端 artifact 仍只来自已批准 source commit 的 read-only workflow_dispatch；不创建 tag、draft 或 publish。

优点：不虚构 independent build reproducibility；所有最终证据真正绑定发布字节；供应链 provenance
清楚。  
缺点：资格化顺序变长；每次 source/tooling 变化都必须先远端构建，再在本机完成 artifact-bound
验收；需要重构 smoke 中多个 `target/release` 硬编码入口。

### C. 只比较 source commit，允许远端 EXE 与本地 EXE 不同

实现最简单，但会把不同字节的 EXE 当作同一 exact candidate。此方案破坏 exact-artifact evidence
的核心不变量，**拒绝**。

## Recommended Contract Change

建议批准 Option B，并同时做以下小范围确定性修复：

1. package staging 对两份字体许可文本使用冻结的 canonical bytes，消除 checkout EOL 漂移；
2. 将“source preflight build”和“release exact artifact”在术语、receipt schema 与路径 resolver 中分离；
3. `remote-workflow.json` 绑定 source/run/artifact，不再复制尚未存在的 final EXE hash；
4. downloaded verify 先验证 workflow checksum、SBOM、package/runtime，再以观察到的 hash 晋升候选；
5. readiness 只接受晋升后的 candidate 及其后生成的 artifact-bound receipts；
6. 不声称 bit-for-bit reproducible build；该能力以后若实现，应作为独立 supply-chain gate 加入。

## Scope Impact

这是 release qualification authority 和生命周期顺序的骨架调整，涉及：

- `docs/plan/11_testing_and_release.md`；
- Phase 12/14 acceptance projection；
- candidate / remote / downloaded / readiness receipt schema；
- Phase 14 campaign 顺序；
- runtime/performance/resources 与 evidence 的 executable resolver；
- release tooling 的文本行尾确定性。

不涉及产品 runtime、DocumentState、编辑器、Preview、IME、持久化或 Windows shell 行为。

## Current Disposition

- local Release/Headless/Runtime/Performance/Resources：当前本地候选均 PASS，但不能证明远端 EXE；
- G3/G4/G5：当前本地候选均 PASS，但不能证明远端 EXE；
- CI run `33256817160`：PASS；
- release diagnostic run `33257534796`：PASS；
- remote package self-verification/runtime：PASS；
- downloaded exact-artifact gate：FAIL；
- readiness：BLOCKED；
- tag/draft/publish：未执行且仍未授权。

未经 USER 批准 Option B 或其它明确路径，不修改权威 plan，不继续生成虚假的 downloaded PASS，
不声称 `READY`。

## Resolution

2026-08-29 USER 明确批准 Option B，并要求采用“最小 exact 重验”，不再运行完整 Phase 0–14 Campaign。

批准后的约束为：

1. GitHub `release` workflow artifact 是唯一 Release Exact Artifact 来源；
2. 本地 build 降级为 Source Freeze 下的 Local Preflight Build，不参与远端 EXE/ZIP 等价判断；
3. 下载产物只有在 checksum、SBOM、package、native runtime 与 runtime smoke 全部通过后才能 Promote；
4. Runtime、Performance、Resources、G3、G4、G5 与人工收据必须在 Promote 后针对相同 staged candidate 重建；
5. 若 Source Freeze 未变化，source-only CI/headless evidence 可以复用；不重跑 Phase 0–14 全量 Campaign；
6. 不建立或声称 bit-for-bit reproducible Windows build；
7. 本批准不授权 tag、draft release 或 publish。
