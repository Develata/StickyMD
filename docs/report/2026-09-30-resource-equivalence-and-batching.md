# 跨命令等价复用、缓存批量校验与失败优先诊断

日期：2026-09-30。USER 已批准按建议继续优化；本报告记录设计和验证，不建立平级契约。

## 背景、数据与问题

当前同命令内会合并等价场景，但持久缓存只按场景名读取。Source/Math/Images 分开运行
时存在四对已验证等价的场景，固定等待合计 1,500 秒。这是协议预算，不是实测提速。
上轮 Zoom 命中命令仍采集五次完整身份，合计 3.927803 秒；初始化和计划开始相邻重复。
固定诊断顺序也会延迟已知失败场景的反馈。

## 已批准路径与边界

1. 先验证请求单元自己的记录，再查同一身份下注册且严格等价的完整场景。候选须分别通过
   校验和、有效期、五次原始观测、统计和硬门。只转换场景标签，保留原单元/创建时间/身份
   和历史耗时，不续期、不重新保存历史观测；Window/Zoom 不与其他单元合并。
2. 合并新 Store 的初始化与计划前检查。资源组全部命中时，在桌面探针后批量读取，批次
   前后各采集一次完整身份；全部通过后才消费结果。部分命中回到逐场景执行，跨新测量
   不沿用旧批次授权。每组仍有自己的探针，正式资格化和真实采样边界不变。
3. 可选诊断失败优先只调整执行顺序。失败提示不持有成功权威，不扩大用户选择范围；
   Window/Zoom 始终作为完整组。提示缺失/过期/损坏时明确回退固定顺序。正式入口拒绝
   此选项，完整执行的样本数和时长不变。

## 验证安排与规格影响

同步 plan 11、Phase 14 投影和薄入口。覆盖等价/不等价、损坏/过期、来源和 formal 拒绝、
批次中漂移/部分命中/失败不漏采、失败提示范围和排序。运行 targeted tests、完整 smoke、
fmt、strict clippy 和 Phase 00；有针对性地实测身份开销。任何合成记录只用于隔离测试，
不注入真实诊断缓存。本轮不改变产品 runtime、正式资源协议或 readiness。

## Resolution — 2026-09-30：三项实现与验证完成

USER 授权的实现已分三批本地提交，未 push：

- `1964017be1f392c09bc261816cf680af4ec5f3ac`：同身份跨命令等价完整场景复用。
- `ba9c37edafc1c756ccd308016c066a9841bc085a`：合并初始化/计划前检查，同组全命中批量读取。
- `78c903519f340830e2c615c4503e5a2bf4269f3b`：可选失败优先、强制新测量与提示生命周期。

等价查找覆盖 source ↔ source-20-math-lazy、preview ↔ preview-20-math、
split ↔ split-20-math、preview-no-math ↔ preview-no-images 四对场景。首先验证请求单元，
再逐候选检查同一完整身份、原始五次采样、统计、硬门、摘要和有效期。仅做标签投影，
`shared_from` 保留原单元、创建时间与身份，不生成新的历史缓存或续期。四对全命中时，
可避免的重复固定等待为 450×3+150=1,500 秒；这是协议计算，未作全矩阵现场计时。

批量读取只消费同组尚未同命令共享、且本批全部验证通过的场景。任一记录缺失或失效，
整批退回逐场景路径；不得跨新测量沿用部分结果。缓存消费前后完整采集身份，探针仍逐组
执行。新增 `cache_batch.execution_seconds` / `case_count`，与场景处理耗时分开。

`-ResourceFailureFirst` / `--resource-failure-first` 要求诊断续跑。为避免旧成功掩盖新失败，
被选中的失败单元明确绕过历史缓存并完整重测。构建、候选和环境前置任务不移动，其他
已选任务不删减，Window/Zoom 保持完整组。24 小时内的校验和有效提示可以跨 source
变更指导排序，但不改变任何成功数据的身份要求。新测量成功清除前会重读比较提示版本；
历史命中不清除。版本比较属于建议性调度，不是跨进程锁。基础/数学/图片组在场景尚未
开始时发生的探针或预检错误，不虚构具体失败场景。

只看计划示例（不启动产品、不写 evidence 或提示）：

```powershell
./tools/smoke/phase-14.ps1 -Resources -ResourceResume -ResourceFailureFirst -ResourcePlan -EvidenceFile target/diagnostics/resource-next.json
```

实际运行使用相同诊断参数并去掉 `-ResourcePlan`；可用 `-ResourceModule` 限定原有资源组。
正式路径和混合 qualification 的薄入口明确拒绝失败优先。

### 身份扫描成本实测

在 clean detached checkout `E:\gitclone\StickyMD-resource-probe-0d89ca5`、
source `ba9c37edafc1c756ccd308016c066a9841bc085a` 上运行
`native_diagnostic_batch_identity_profile`。它读取真实 Git、程序/harness、环境及输入身份；
三条缓存记录是临时测试目录中的合成 fixture（preview-no-math、preview-1-math、
preview-200-unique），没有放入仓库实际诊断缓存，也未启动资源测量。对应日志为
`target/acceptance-profiling/resource-round3-batch-profile.log`。

| 路径 | 真实完整身份采集次数 | 本次耗时 |
| --- | ---: | ---: |
| 三个独立 load | 6 | 7.426306 秒 |
| 一个完整 batch | 2 | 2.969423 秒 |

单次配对测量减少 4.456883 秒，约 60.0%。该数据只支持缓存验证成本下降，不是整体资源
验收耗时、GUI 性能或统计稳定收益证明。日志明确标记 `NOT_RESOURCE_ACCEPTANCE`。
本轮未重复上轮 Zoom 现场实验，旧报告中的 103.829131/10.602892 秒保持旧提交身份。

### 检查、边界与未验证项

- 定向资源回归：49 passed、5 ignored；相关 CLI/PowerShell 集成回归 3 passed。
- 完整 `cargo test -p stickymd-smoke --locked`：269 passed、5 ignored；集成 17 passed。
  覆盖双向等价投影、过期/损坏/漂移、部分批次回退、提示范围/有效期/清理、强制重测、
  前置任务和完整组排序、正式输出保护、只读预览、Windows PowerShell 5.1/7 入口。
- `cargo fmt --all -- --check`、`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`
  通过，Phase 00 治理通过。代码检查在 ba9c37e 加本次差异的提交前 worktree 上完成，
  提交为 78c9035 后另做 clean HEAD Phase 00 检查。
- 日志：`target/acceptance-profiling/resource-round3-full-tests.log`、`resource-round3-clippy.log`、
  `resource-round3-governance.json` 和 `resource-round3-committed-governance.json`（均在同一目录）。
  提交前治理 JSON 如实标记 dirty，不作为 clean-source 或产品资格化证明。
- 本轮没有重跑完整五组现场资格化、Window 全压力复用或跨组别名的长时间原生对照；
  这些现场验证仍为 NOT TESTED。本轮只变更验收工具，不改变产品 runtime 和既有门槛，
  不继承 v0.1.1 exact-artifact 身份或发布特例，技术 readiness 仍为 NOT_READY。
