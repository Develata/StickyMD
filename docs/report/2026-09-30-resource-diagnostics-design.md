# 资源诊断探针、检查点与续跑设计（2026-09-30）

## 背景与授权

USER 已明确要求按顺序实施桌面交互探针、场景进度与检查点、诊断场景续跑，并允许分批
本地 commit。本设计属于开发验证工具；不改产品对象、采样协议或正式资格化的五组边界。
实现顺序为 plan → 验收投影 → Rust CLI，PowerShell 继续只传递参数。

上一轮在 82 分 15 秒后因点击路由被遮挡停止；图片组此前已完成 9 个 cohort，整组用时
47 分 15 秒。开始菜单归属是失败后的查询结果，不能证明其出现原因。完整矩阵固定等待
预算为 6,400 秒，故优先减少无效长跑与中断后的重复成本，不削减五次采样或 CPU 窗口。

## 1. 独立交互探针

每个实际执行的资源组在测量前使用独占临时程序目录，验证真实 Source/Preview/Split
点击及持久化模式确认。探针不产生资源样本；结束后终止其子进程，正式样本另起进程。
探针失败立即进入既有失败收尾；不关闭其他应用、不向被遮挡的窗口发送点击。
输入路由错误在现场记录 observed HWND、PID、可查询的可执行文件名和窗口类；未知项
显式标未知，不记录窗口标题或完整进程路径，也不以事后查询替换当时事实。

## 2. 场景进度与检查点

保留整组/整命令的证据所有权，以 observer 把场景完成通知交给 runner 持久化。
每个完整场景结束后保存当前组已有样本，显式标为 INCOMPLETE/NOT_TESTED；窗口压力
过程与缩放组仍保持原完整性边界。进度显示组、场景、轮次、预热/转换/CPU/压力阶段
和剩余固定等待预算。进度不是完成标记，写入失败仍非零退出并保留原始失败。
持久化在采样窗口外进行；不添加占用 CPU 的后台计时线程。

## 3. 显式诊断续跑

采用显式 opt-in 诊断选项和独立 ignored 存储；正式 canonical 请求拒绝该选项。
仅复用已完整通过五次采样的单个基础/数学/图片场景，不复用部分轮次，不恢复窗口压力
中段或缩放组中段。失败场景下次从头开始。现有历史 JSON 不自动迁移成续跑缓存。

缓存绑定实际 EXE、正在执行的 harness、源码/fixture/协议输入、主机/交互 session、
显示与环境身份；复用前及记录前重新核对。未知、损坏、过期、缺样本、硬门失败或身份
变化的记录不复用。来源和原始观测随记录保存，明确区分历史观测与本轮执行耗时。
诊断结果不能进入正式 last-success 或 readiness，完整组的正式复用协议保持不变。

## 方案取舍、失败路径与验证

不采用自动点击其他窗口、忽略遮挡、缩短采样或把部分轮次拼成五次 PASS 的方案。
先实现可定位的短探针和原子检查点，再实现复用，降低把状态/身份缺陷隐藏在缓存中的风险。
测试覆盖探针短路、隐私字段、进度状态、失败证据、缓存输入变化/损坏/缺项/来源、正式入口
拒绝诊断恢复、写入失败和原成功账本保护。真实桌面短探针单独验证；长矩阵与正式候选
资格未实际完成时保持 NOT TESTED，不用无界面测试替代。

完成后的验证结果在文末追加 Resolution；收益需后续实测，当前不报告新的加速比例。

## Resolution 1（2026-09-30）：前置探针

探针短路与未知窗口诊断定向测试通过。显式 native probe 使用本地诊断 checkout
`StickyMD-resource-probe-0d89ca5` 的 EXE（SHA-256
`ba77d77e3d698d31ed5754e3e5802d050e013f689364b2252944e208a1be4165`），真实三种视图
切换与持久化确认通过，测试过程 1.70 秒。探针不产生资源样本，不构成正式五组验收；
本轮未人为弹出开始菜单复现遮挡。未知 case 在任何探针/GUI 操作前拒绝。

Stage 1 的资源 fixture/缓存定向回归 5 passed、路由诊断 1 passed；严格 Clippy、fmt 与
Phase 00 治理通过。治理首次发现编号上限未同步，更新 A52 注册范围后复查通过。

## Resolution 2（2026-09-30）：场景检查点与进度

场景完成后由 runner 保存当前组原始样本、门槛和统计，强制保留 INCOMPLETE/NOT_TESTED；
窗口只在完整轮次之间保存诊断快照，压力过程未拆分。进度 sidecar 在新命令开始时清除
旧运行状态，在阶段转换时报告轮次与固定预算；整命令结束/失败显式终结进度。它始终不是
资格化收据。CPU 窗口内不新增写盘或计时线程。

资源相关回归为 25 passed、2 ignored（原有规划计时及需显式桌面的 native probe）；
其中原子检查点、旧进度重置、剩余预算、失败状态、写入失败与成功账本隔离通过。
严格 Clippy、fmt 和 Phase 00 治理通过；native 矩阵进度将在诊断续跑集成后定向验证。

## Resolution 3（2026-09-30）：诊断场景续跑实现与回归

增加显式 `--resource-resume` / `-ResourceResume`，要求 Phase 14 Resources 和 `target/`
下 ignored JSON 输出，避免检查点自身改变工作树指纹。正式路径及其目录别名在执行前
拒绝续跑；内部缓存独立放在 `target/resource-diagnostics/v1/`，不调用成功账本登记。

完整五次场景经原始观测、统计、采样协议和硬门核对后才缓存。记录绑定实际 EXE、
正在执行的 harness、全部 tracked/nonignored untracked bytes 与源码提交，以及
主机/启动/登录、单显示器 DPI/尺寸/work area、电源状态和执行设置。24 小时有效期、
完整记录 SHA-256、读取与采样前后身份核对、原子发布前复查共同约束复用。
身份无法识别时完整执行；运行期间漂移则停止并保留已有诊断数据。

复用的原始样本保留 `diagnostic-cache:` 来源，跨组别名共享也不覆盖该标记；
`DIAGNOSTIC_REUSED` 和 `origin_execution_seconds` 区分历史观测、本次检查耗时。
窗口压力和缩放过程没有增加中段恢复。该实现不导入既有历史 JSON。

本轮完整 smoke 回归为 250 passed、3 ignored，headless 集成为 15 passed；严格 Clippy、
fmt 和 Phase 00 治理通过。回归覆盖损坏/过期/未来时间、源/EXE/harness/执行输入漂移、
部分/失败/重复样本、错误统计与硬门、写入失败、目录别名和内部存储越界。
结果位于 ignored `target/acceptance-profiling/resource-resume-smoke-tests.log` 及
`resource-resume-governance.json`。未以这些无界面测试代替真实资源资格化。

显式 native 身份稳定性测试已通过，耗时 4.32 秒。首次发现本机 .NET `Groups` 不提供
登录 SID，因此改为原生 `TOKEN_STATISTICS.AuthenticationId`，没有降级为仅核对用户名或
可复用的 Windows session number；依据为 [Microsoft TOKEN_STATISTICS 文档](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-token_statistics)。
短场景首次采样与跨进程复用的耗时对照将在下文另行追加，当前尚不报告实测加速。

## Resolution 4（2026-09-30）：真实短场景与跨进程续跑对照

复用本会话已有的隔离诊断 checkout `E:\gitclone\StickyMD-resource-probe-0d89ca5`，在确认
工作树干净且无遗留原生 smoke 进程后切换至
`1006f6c91c532f1753daf5ccb803d6084c65c067`。该 checkout 无 `dist/`/Source Freeze，
只使用本地 Release，不改变主工作树中的正式收据。两次测试均显式设置
`STICKYMD_SMOKE_RESOURCE_CASE=preview-no-math`，运行
`phase 14 --resources --resource-module=math --resource-resume --json`，输出各自独立的
ignored `first.json`、`resumed.json`，结束后恢复调用者筛选变量。

| 本次对照范围 | 首次完整场景 | 续跑 |
| --- | ---: | ---: |
| 整条 CLI 命令，秒 | 170.629 | 11.792 |
| 筛选后的 Math 组，秒 | 163.679840 | 7.754740 |
| 场景执行及缓存处理，秒 | 157.171382 | 1.960537 |
| 本轮新采样数 | 5 | 0 |
| 历史复用样本数 | 0 | 5 |
| 实际固定预热等待，秒 | 150 | 0 |

两次命令均零退出、工作树 clean、交互环境 VALID，且均重新完成真实桌面探针。
首次显示 MISS 并保存完整场景；续跑显示 `DIAGNOSTIC_REUSED`。逐项比对五次原始观测、
run、cohort 与 gates 一致，续跑的每个样本明确保留历史来源。缓存的原采样执行耗时
`153.328576` 秒独立记录为 `origin_execution_seconds`，没有当作续跑本轮耗时。
现场读取进度文件确认预热阶段、轮次和剩余固定预算；两次最终 sidecar 的 stage 均为
`command-finished`，status 仍是诊断性 `INCOMPLETE`，不是正式收据。

来源与 ignored 证据：

- EXE SHA-256：`ba77d77e3d698d31ed5754e3e5802d050e013f689364b2252944e208a1be4165`。
- harness SHA-256：`ad7e465066f5509b506b7dd998747556447e2e48ef285374b806c4e8db951e14`。
- 目录：`target/acceptance-profiling/resource-resume-live-20260930/`，保存两次 JSON、完整日志与进度文件。
- `first.json` SHA-256：`35a4eb682335b9c88faaf8078efefcbac3000382b7a698d0d7643942656033a2`。
- `resumed.json` SHA-256：`2e2daf3b5a030c558598ee39b40e71693ef065c916b15bb7c73363f7e94a60af`。
- 缓存 key：`10a8a2ec8a8c0e01a55c257b742f25625538b298a330be58bbe6baac13aacef8`，
  created Unix 秒 `1790775445`；独立核对记录 body 的 SHA-256 与保存值一致。

这是一次无公式预览场景的真实采样/复用对照，未执行 CPU 采样、图片长组或完整五组矩阵，
未故意制造中途菜单遮挡或中断。测试结束没有遗留原生进程，两个工作树均干净。
本结果只证明本次诊断续跑避开了 150 秒固定等待，不能推算完整正式验收的加速比例。
P14-A46、人工缺口及既有发布的技术 readiness 不由本次诊断更新。
