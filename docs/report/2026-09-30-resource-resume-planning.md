# 诊断整组续跑、执行计划与身份检查优化

日期：2026-09-30。USER 已批准依次实现这三项；本报告是分析记录，不是独立契约。

## 背景与问题

当前诊断缓存只覆盖 Source/Math/Images 完整场景。Window/Zoom 仍重新运行，固定等待
分别为 1,350 秒与 75 秒，另有压力循环和进程准备。缓存失效只打印泛化 MISS，难以判断
重跑原因。首次缓存读取路径包含四次完整身份采集，存在相邻重复检查。

## 路径与规格影响

1. 扩展为明确的诊断单元：完整五次采样场景，或完整 Window/Zoom 组。后者使用正式组的
   原始观测、统计、硬门、fixture 与压力完成校验；不恢复部分压力循环，清理成功后才缓存。
2. 执行前列出复用/重跑与原因、固定等待预算。计划是可失效的预测，实际执行仍重新验证。
   最新记录索引只用于解释失效原因，不能授权复用，也不能覆盖正式资格化证据。
3. 先记录真实身份采集分项耗时，再合并相邻检查；保留读取前后、采样前后和原子发布前的
   鲜活身份边界。相同命令内共享观测仍须检查身份。

不改产品 runtime、五次采样协议、门槛、正式 Resources 账本或 readiness。缓存版本升级
不导入旧记录；保留旧文件。未知环境完整重跑，运行中身份漂移失败退出。

## 验证安排

针对损坏/过期/缺样本/压力标记缺失/漂移/路径别名增加回归，运行 smoke tests、fmt、
clippy 和 Phase 00。使用短 Zoom 完整组做本地首次运行与命中实测；Window 长组未实测的
部分明确保留缺口。身份优化必须报告实际次数和观测用时，不将静态预算当作实测收益。

## Resolution 1 — 实现与回归（2026-09-30）

三项按序实现并分别提交：`0857d6e` 完整 Window/Zoom 组诊断缓存；`552e152` 续跑计划、
失效原因与分项计时；`cf2c7d6` 去除相邻重复身份采集。契约和 P14-A55/A56/A57 同步更新。
缓存 v2 保留原始观测/统计/硬门/fixture/压力检查，不导入 v1，不从部分组恢复；完整组清理
成功后才发布。计划输出 NOT_RUN，执行重新检查，最新索引只解释失效原因。

完整 smoke 回归：257 单元测试通过、4 个显式 native/profile 测试默认 ignored；16 个集成
测试通过。fmt、strict clippy 与 Phase 00 通过。随后复查发现 PowerShell wrapper 的
`-ResourcePlan` 与资格化 action 混用可能静默选择另一动作，已补显式互斥和独立集成回归。
最后的 17 项集成测试结果单独保存在 `target/acceptance-profiling/resource-round2-integration-final.log`。
无产品 runtime 或外部依赖改动。

## Resolution 2 — 身份检查前后实测（2026-09-30）

显式运行 `native_diagnostic_identity_lookup_profile --ignored --nocapture`；固定读取 clean
隔离 checkout `E:\gitclone\StickyMD-resource-probe-0d89ca5` 的 `552e152`，actual Release
EXE 不变。对照 harness 分别为优化前与优化后的构建；两次都查询缺失的完整 Zoom 记录，
不产生资源样本。首次查询包含一次主机信息采集。

| 路径 | 优化前采集次数 / 秒 | 优化后采集次数 / 秒 |
| --- | --- | --- |
| 首次打开与查询 | 4 / 5.424882 | 3 / 4.964233 |
| 后续单元查询 | 3 / 2.804620 | 2 / 2.024509 |

优化前单次输入 hash 约 0.30–0.49 秒，Git 约 0.24–0.29 秒，是主要重复工作。
记录位于 `target/acceptance-profiling/resource-identity-before.log` 和 `resource-identity-after.log`。
这是一组顺序观测，不是统计显著性证明；采集次数减少是确定行为，用时受主机负载影响。
缓存读前/读后、采样后/原子发布前仍各有鲜活检查；同命令共享另行检查。任一边界注入
身份漂移都会失败且不覆盖旧完整记录，已由回归验证。

## Resolution 3 — Zoom 完整组与续跑实测（2026-09-30）

在同一 clean 隔离 checkout 切至 `cf2c7d65cb62815b937a7351b41b546b686ebcc6` 后，直接运行
预构建 CLI 的 `phase 14 --resources --resource-module=zoom --resource-resume --json`，分别
指定 ignored `target/acceptance-profiling/resource-round2-live-20260930/first.json` 和 `resumed.json`。
checkout 无 `dist/`，不修改主仓库已有正式收据。两次环境均为 VALID，结束后无残留产品进程。

| 观测 | 首次完整组 | 诊断复用 |
| --- | --- | --- |
| 命令 wall time / 秒 | 103.829131 | 10.602892 |
| group.execution_seconds | 100.870438 | 7.556730 |
| 本轮 desktop probe / 秒 | 1.279382 | 1.287338 |
| 新采样 / 历史采样 | 15 / 0 | 0 / 15 |
| 计划固定等待 / 秒 | 75 | 0 |

首次完成 50%/100%/300% 各五轮采样和原压力循环，硬门通过后保存完整组。复用的 15 条
原始观测及所有 gates 与首次一致，每条增加 `diagnostic-cache:` 来源；历史执行耗时
93.541899 秒独立记录。本轮探针仍重新执行。命令用时在这一次对照中减少约 90%，不能
外推为五组正式验收提速比例。

只看计划的入口在首次前显示 MISSING/75 秒，首次后显示 REUSE_IF_VALID/0 秒，并经 hash
确认未修改指定的已有 first.json。临时增加一个 `STICKYMD_` 执行设置后，只看计划明确显示
`IDENTITY_CHANGED: environment` / RUN / 75 秒；设置在该进程内恢复，未执行重测。

身份与证据：

- Release EXE SHA-256：`ba77d77e3d698d31ed5754e3e5802d050e013f689364b2252944e208a1be4165`。
- CLI SHA-256：`5d83a27e1ca7c9df7fda18bf63cb70fe8e91a71b5fa78041f512d583496543ef`。
- cache key：`40e65e115b045870be8a15ee1560bf29bf35481d1f5769951262ab51ff02efe4`。
- first.json SHA-256：`8f653cd7a5626b6229d616c522ecc03fcaa52550018556b2d4145306933dd016`。
- resumed.json SHA-256：`71d15204ae5d89b9f91bde91677e99a767d1c9ff10ce4601acc0446d5b2615f1`。
- 同目录保留两份完整日志、进度文件、首次前/后和环境变更的 plan JSON 与日志。

Window 完整长组现场复用及当前源码的正式五组资源资格化仍为 NOT TESTED；本轮证明的是
诊断工具行为。后续文档/参数保护提交不继承本次精确 source/cache 身份。既有正式发布特例
和 NOT_READY 边界未改变。

最终收尾验证：新增参数互斥保护后，17 项集成测试全部通过；strict clippy、fmt check 和
Phase 00 再次通过。主 checkout 仅提交本轮工具与文档变更，无 push。
