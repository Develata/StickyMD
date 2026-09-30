# 资源验收优化真实桌面诊断（2026-09-29）

## 结论与范围

USER 要求对已完成独立 review 的验收优化“尝试实测一次”，此前允许分批本地 commit。
本轮执行一次完整 Resources 命令，实际运行 4,935.127391 秒（82 分 15 秒），退出码为 1。
Source/Preview/Split 与数学两组 PASSED；图片组在最后一个场景的首次切换中 FAILED；
窗口与缩放组尚未执行，记录为 NOT TESTED。整轮没有通过完整资源验收。

四处等价 cohort 复用均在真实执行中生效，保留 70 份独立实测样本与 20 份标注来源的
复用样本。按既有采样协议，四处复用避免 1,500 秒（25 分钟）的重复固定等待。
这是执行分支和等待预算的核对，不是优化前后完整运行的 A/B 耗时比较。

## 身份、环境与重现

本次测量 source 为 `0d89ca5da0c1ee30ce035db1512ee1d210f8bc8b`，测量时工作树干净。
主 checkout 的 Source Freeze / Promoted Candidate 仍绑定旧 source
`9a0a00a1fe143cf25fa4208a54ec5dfea7f3e7c9`。为遵守
[plan 11](../plan/11_testing_and_release.md#resource-module-qualification)，使用当前 source
的独立 detached checkout 做 LOCAL_DIAGNOSTIC，不把旧候选冒充为本次程序。

- Checkout：`E:/gitclone/StickyMD-resource-probe-0d89ca5`。
- 其 `target` junction 指向主 checkout 的 `target`，复用构建缓存；未并行执行其他构建或测试。
- 该 checkout 不含 `dist/`、Source Freeze 或 Promoted Candidate，程序由本次命令本地构建。
- 测量使用 harness 管理的独占临时 portable 目录和 fixtures，不访问用户便签。
- 完成后主 checkout `dist/evidence/` 未发现运行开始后修改的文件；正式收据与成功账本未更新。
- Harness SHA-256：`fb529a2c69ddfa8d9fafd2546762c1097837af5b63a625b5075fb8d2e6b240eb`。
- 本次 Release EXE SHA-256：`ba77d77e3d698d31ed5754e3e5802d050e013f689364b2252944e208a1be4165`。
  运行后的文件 hash 与诊断 JSON 的 `executable_sha256` 一致；`artifact_sha256` 为空。

命令在上述 checkout 中执行，`STICKYMD_SMOKE_RESOURCE_CASE` 临时清空，没有选择单组或单场景：

```powershell
& E:/gitclone/StickyMD/target/debug/stickymd-smoke.exe phase 14 --resources --json `
  --evidence-file=E:/gitclone/StickyMD/target/acceptance-profiling/live-resources-20260929-193017/resources-diagnostic.json
```

实际命令约在 `2026-09-29T22:32:39.517Z` 启动，`2026-09-29T23:54:54.644Z` 结束。
`invocation.json` 的时间是更早的准备时间，不用于计算执行耗时。
开始前桌面环境为 VALID：交互桌面可用、未锁定、shell 与前台窗口可用、单显示器。
主机有 20 个逻辑处理器、15.797 GiB RAM，运行前可用 3.148 GiB；运行后可用 3.904 GiB。
这些快照不证明整个运行期间始终没有桌面干扰。

## 实测耗时与覆盖

以下组耗时取诊断 JSON 的 `group.execution_seconds`；总耗时还包含治理、环境、构建、
检查点写入和收尾。FAILED 行是停止前耗时，不能当作该组完整耗时。

| 阶段 | 状态 | 秒 | 样本情况 |
| --- | --- | ---: | --- |
| 本地 Release 构建 | PASSED | 193.587768 | 本次构建；不是冷缓存构建基准 |
| Source/Preview/Split | PASSED | 1394.929077 | 15 份独立实测 |
| 数学 | PASSED | 502.934276 | 15 份独立实测 + 15 份复用 |
| 图片 | FAILED | 2834.922052 | 前 9 个 cohort 共 40 份独立实测 + 5 份复用 |
| 窗口 | NOT TESTED | — | 未执行 |
| 缩放 | NOT TESTED | — | 未执行 |
| 整条命令 | FAILED | 4935.127391 | 90 份记录，其中 70 份独立实测 |

图片组中 `source-after-preview-cache-release` 的第 1 次运行在切回 Source 前失败，
该场景耗时 34.138820 秒，尚未形成资源样本；此前 9 个 cohort 的记录均保留。
完整失败详情与样本位于诊断 JSON，不能据此前面场景已完成而把图片整组改为 PASSED。

已有样本的部分指标如下，内存列为五次采样的最大 private working set：

| 场景 | 最大内存（bytes） | 最大空闲 CPU（%） |
| --- | ---: | ---: |
| Source | 14090240 | 0.002604 |
| Preview | 20975616 | 0.003906 |
| Split | 21692416 | 0.002604 |
| Preview 图片缓存饱和 | 36007936 | 0.002604 |
| Split 图片缓存饱和 | 37527552 | 0.002604 |

空闲 CPU 是 harness 按逻辑处理器数归一化的 60 秒区间指标。全部已有样本的最大值为
0.003906%，低于本轮记录的 0.1% 空闲 CPU 门槛；实际停止原因是物理输入路由检查失败。
这些结果不能替代未运行的窗口、缩放或缓存释放检查。

## 已证实的复用与加速边界

| 复用场景 | 原始 cohort | 复用样本 | 避免的固定等待 |
| --- | --- | ---: | ---: |
| `source-20-math-lazy` | `source` | 5 | 450 秒 |
| `preview-20-math` | `preview` | 5 | 450 秒 |
| `split-20-math` | `split` | 5 | 450 秒 |
| `preview-no-images` | `preview-no-math` | 5 | 150 秒 |

三个 CPU 场景分别避免 `5 × (30 + 60)` 秒；无图 Preview 避免 `5 × 30` 秒。
四处 `execution_seconds` 分别为 0.000083、0.000014、0.000009、0.000020 秒。
运行后逐个比对 20 份复用记录，其每次采样的全部 measurements 均与对应原始样本相同，
且保留 `shared_from`；没有把复用记录算成新的独立实测。

本次没有运行优化前版本的同条件对照，也没有获得五组全部完成的耗时，因此不报告整体
加速百分比。跨命令正式 last-success 复用、Runtime/Performance 的 workspace 共享与
远程 CI 并行收益均未实测。资源测试保持串行，不能由此推断同机并行采样同样有效。

## 失败现场、根因边界与可选后续

原始错误为：

```text
refusing activation click outside StickyMD: point=(926,214) observed_root=66128 expected_root=25758202 rect=WindowRect { x: 890, y: 127, width: 780, height: 1020 } visible=true style=WindowStyleFacts { tool_window: true, app_window: false, no_activate: false, transparent: false }
```

执行路径为 `measure_case` 预热后调用 `switch_to_source`，激活窗口时先用
`WindowFromPoint` 校验真实点击目标；目标 HWND 不符，因而在发送点击前返回错误。
这证明输入路由保护生效，失败并非报告中的资源门槛超限。

在 `2026-09-29T20:56:49-03:00` 只读查询同一 `observed_root=66128`，该 HWND 仍存在，
所属进程为 `StartMenuExperienceHost`（PID 15224），窗口类为
`Windows.UI.Core.CoreWindow`。这与开始菜单遮挡目标的解释一致。查询发生在失败后，
没有当时的桌面录像；菜单由谁、何时打开，以及是否存在其他同时干扰，尚不能确定。
本次没有关闭该系统窗口、放宽点击保护，或自动重跑整轮。

下一次可先在无人操作的桌面只跑失败场景定位干扰；这种过滤诊断不能补成完整正式收据。
若要取得 P14-A46 的正式通过证据，应针对新的 Source Freeze / Promoted Candidate，
在独占桌面完成五组资格化，并另外验证跨命令复用与中断重跑。优先消除桌面干扰，
不以缩短采样窗口或忽略路由检查处理本次失败。

本轮不改产品、harness、采样次数、门槛或 plan，不改变既有发布状态。
P14-A46 继续保持 NOT TESTED，LOCAL_DIAGNOSTIC 的通过子组不建立正式候选资格。

## 原始证据与收尾

本地 ignored 证据目录为
`target/acceptance-profiling/live-resources-20260929-193017/`，包含 invocation、运行前环境、
运行前后主机快照、完整诊断 JSON、stderr、completion 与 artifact-audit。
失败命令的 stdout 为空，实际失败详情在 stderr 和诊断 JSON 中。

- `resources-diagnostic.json` SHA-256：
  `edf5c8a7d130ccde489bb99135e5b8e5ac77921fee393e38583fb28448b35edb`。
- `resources.stderr.log` SHA-256：
  `48795e4872c4f614694bab2d8d6b238e1bb824c5e7cd91019d055356f63bbdf1`。
- `completion.json` SHA-256：
  `2a2e2b3f778d532950f3d72df71780990184d6136a3e028c44ce2241e6e92dac`。

命令退出后未发现 StickyMD、stickymd-smoke、cargo 或 rustc 残留进程；主 checkout 与
诊断 checkout 的 Git 工作树在写本报告前均干净。诊断 checkout 和证据保留供追溯，
没有执行 workspace 清理、远程 push 或重新发布。

## Resolution（2026-09-29）

报告新增后的 Phase 00 治理检查 PASSED，证据为同目录的 `report-governance.json`。
本轮源码没有修改；此前独立 review 的测试基线不重复执行，也不以治理通过覆盖上述
真实桌面运行的 FAILED 结果。
