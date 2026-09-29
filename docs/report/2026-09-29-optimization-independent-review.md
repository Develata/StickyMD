# 验收优化独立 review 与修复（2026-09-29）

## 范围、授权与背景

USER 要求派 subagent 对当前修改做完整独立 review，并修复存在的问题；此前已允许分批
本地 commit。本轮初始工作树干净，审查范围为 `c204afa..c6c7c86` 的全部 7 个未推送提交，
共 55 个文件。独立 reviewer 只读审查，主 agent 负责隔离复现、实现和验证；未推送远程。

覆盖资源场景/共享 cohort/硬门、五组成功证据与 readiness、指纹协议及失败清理、workspace
共享身份、候选执行规划、CI Linux job 与聚合、headless 测试隔离、notices 输出预检。
本报告记录工具正确性，不建立真实桌面或 exact-artifact 资格。

## 确认的问题与根因

### P2：规划后变化的成功证据仍被复用

`resource_modules::Campaign` 保存规划时的完整 `CompatibleSuccess`。执行到该组时只
重新核对 candidate 与输入指纹，没有重新读取 ignored ledger/archive。长 campaign 中
文件被删除或损坏，甚至指针已经轮换且旧归档已删除，仍会返回旧的 `REUSED_PASS`。

隔离回归使用真实 Git、Source Freeze、合成且不执行的 PE、完整资源样本和正式 ledger
函数；分别删除/损坏 ledger/archive，并轮换为另一份兼容成功。修复前四种失效均错误
复用，合法轮换也报告旧来源。测试只验证收据与调度，不测量产品资源。

修复后规划只保存是否可复用。复用点重新计算当前输入，再以此摘要读取并核对当前
ledger、归档 hash、完整覆盖和 origin；同一复用点不重复计算输入摘要。失效返回错误，
合法轮换显示刚验证的来源，历史测量不会冒充本轮实测。

### P2：诊断输出可覆盖内部成功账本和归档

公共路径守卫只保护 workspace 和五个资源子收据，遗漏整个 `module-success/` 目录。
在临时仓库预置 marker 后执行：

```text
stickymd-smoke phase 00 --evidence-file=dist/evidence/module-success/resources-window.json
stickymd-smoke phase 00 --evidence-file=dist/evidence/module-success/evidence/resources-window-fixture.json
```

两次命令均因缺少 Cargo.lock 返回 1，但失败 JSON 覆盖了 marker。该入口也影响此前已有
的模块账本。G3/G4/G5 的直接 evidence writer 还绕过了通用输出守卫。

修复保护整个 ledger/archive 目录，使用文件系统身份与目录组件边界识别，保留同名前缀
的普通兄弟诊断文件。G3/G4/G5 在桌面检查前及写入前均执行守卫；保留原始 case 错误。
回归覆盖 canonical、verbatim/短路径与 junction 别名，且确认已有成功 bytes 不变。

### P2：尚不存在的 Windows 正式输出可通过尾随点/空格别名写入

已有文件的普通 Win32 别名能被 canonicalize 识别；不存在的文件只解析父目录，剩余
文件名保留了尾随点/空格。原子 writer 的普通 `MoveFileExW` 路径解释会折叠尾缀，因而
路径检查与实际写入目的地不一致。

临时根内用 `phase 00` 指定不存在的 `resources-qualification.json.`、
`resources/window.json.` 或 `resources/window.json `，修复前均返回 1，但创建了
对应无尾缀的正式文件；已有文件的同类别名作为对照均被正确拒绝。
修复在解析文件系统身份前规范普通 Win32 拼写，verbatim 路径保留字面身份。
CLI 回归覆盖 summary、资源子收据、workspace 收据的未创建目标。

## 方案与规格影响

选择在复用点校验证据、在所有已识别的公共输出边界保护内部路径；没有增加并行度、
削减测试、改变采样次数/阈值或引入依赖。拒绝只信规划缓存或仅在最终 readiness 才发现
无效证据的替代方案：这会让当前命令错误报告通过。普通诊断仍可输出到独立路径。

修复属于 plan 11 的 `module-success-ledger`、`resource-module-qualification` 和
`shared-headless-prerequisite` 已有边界，未改变骨架或产品 runtime。同步 P14-A42/A43/A50、
coverage matrix 和 CLI README；Phase 14 薄入口及任务选择协议不变。

## 已完成的复现与验证

- 修复前 qualification 定向测试：74 passed、5 个预期回归失败、1 ignored。
  失败分别覆盖缓存收据、轮换来源、内部路径、目录别名和 exact writer。
- 修复后的全部 smoke unit tests：237 passed、0 failed、1 ignored。
  ignored 项是显式本机规划计时，不是被跳过的功能验收。
- actionlint 检查 `.github/workflows/ci.yml` 通过。
- 完整 headless 集成与最终 baseline 正在收尾，结果在文末追加。

复现日志与测试日志位于 ignored `target/acceptance-profiling/review-regressions-*.log`；
临时 CLI 复现只使用独占 scratch 根，未修改实际 `dist/` 收据。

## 未覆盖

未执行 Linux runner、远程 GitHub CI、真实候选 GUI/资源性能验收及完整耗时基准。
P14-A46 维持 NOT TESTED；本轮不声称正式资格通过或测得新的整体加速比例。

## Resolution（2026-09-29）

独立 reviewer 对修复、新增真实 Campaign fixture 与全部投影完成复审，未发现新的阻断
问题。Win32 只读 API 查询进一步确认：只有普通路径的最后组件折叠尾随点/空格；
中间目录不得按同样方式归一化。最终实现只折叠 leaf，对普通路径中的歧义目录明确拒绝，
verbatim 保留字面身份，并补充 device prefix、已有父目录和 `.. ` / `...` 的回归。

首轮修复后 unit tests 通过，headless 为 14 passed / 1 failed：治理测试发现 coverage
matrix 先加入了报告链接，而报告文件尚未创建。报告补齐后，加上上述路径收窄改动，
再次执行完整 smoke suite，最终结果如下：

- `cargo test -p stickymd-smoke --locked`：237 unit passed、1 ignored；15 headless passed，
  共 252 passed、0 failed。Windows PowerShell 5.1 与 PowerShell 7 两个包装器均通过。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`：通过。
- `cargo fmt --all --check`、Phase 00 治理、`git diff --check`、CI workflow actionlint：通过。

最终日志为 ignored `target/acceptance-profiling/review-final-tests.log`、
`review-clippy.log`、`review-phase00.json` 与 `review-phase00.log`。工具修复和测试不改变
上述未覆盖范围，不撤回既有发布，也不恢复旧候选资格。
