# Phase 14 Acceptance Matrix

本矩阵验证 release policy / qualification tooling，以及 USER 在 candidate freeze 前明确批准的
Split 同步、Source 查找替换和转换控件标识。Promoted Candidate 继续绑定 exact artifact bytes；功能资格化由
module input fingerprint 决定复用或重跑，不按整个 commit/candidate 机械失效。自动项由 std-only Rust CLI
持有并由 CI 调用。人工项和 exact-desktop 动态项在 source matrix 中保持 `NOT TESTED`，只有对应的
ignored exact-candidate receipt 可改变 readiness，不能改写此 Markdown 状态。

因此下表 `Status` 是 tracked source baseline，不是 `v0.1.0` 当前发布 verdict。`v0.1.0` 已从
exact source `64690ab8f86f63f3cbfeabbb0961276978c8f26d` 取得 `READY` 并正式发布；最终身份、
USER waiver 与仍未执行的极端环境项见
[`../release-notes/0.1.0.md`](../release-notes/0.1.0.md) 和
[`../release-checklist.md`](../release-checklist.md)。历史行继续保持原 baseline，避免用发布后的
动态收据重写 source-controlled 证据。

## Automated qualification

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-A01 | Phase 14 stable PowerShell entry、task、report、matrix、guided manual guide 存在 | Automated | `stickymd-smoke phase 14 --ci` | AUTOMATED PASS |
| P14-A02 | Phase 14 加入 deduplicated headless CI graph | Automated | `stickymd-smoke all --ci --json` | AUTOMATED PASS |
| P14-A03 | cold/warm startup 同时报告 180/400/550 ms；正式 warm-cache 固定等待 1000 ms，250 ms rapid-restart 仅作独立诊断；仅 >550 ms hard fail | Automated | qualification/runtime interval + threshold unit tests | AUTOMATED PASS |
| P14-A04 | Performance ordinary failure 不跳过 Resources | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A05 | Resources failure 不抹除 Performance receipt | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A06 | Runtime ordinary failure 仍保留后续独立、安全 receipt | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A07 | invalid environment（含无法写入物理 cursor position）、identity mismatch、P0/data-safety failure 全局停止 | Automated | qualification environment capability probe + campaign policy unit tests | AUTOMATED PASS |
| P14-A08 | startup attribution 使用 per-sample milestone intervals；归因记录不自动断言无需产品优化，门槛结果由 Performance receipt 持有 | Automated | attribution parser/classifier tests | AUTOMATED PASS |
| P14-A09 | Tier A `NOT TESTED` 阻断；PASS 或 explicit waiver 才 eligible | Automated | readiness tests | AUTOMATED PASS |
| P14-A10 | Tier B group waiver 必须绑定 version/source；Tier C NT 仅在 automation PASS 时 nonblocking | Automated | readiness tests | AUTOMATED PASS |
| P14-A11 | v0.1.0 waiver 不适用于 v0.1.1 | Automated | readiness version-binding test | AUTOMATED PASS |
| P14-A12 | manual receipt 绑定 exact source/EXE/ZIP/version/Windows build/session/case | Automated | manual receipt tests | AUTOMATED PASS |
| P14-A13 | unsigned package 明确记录且不伪造 Authenticode signed fields | Automated | release/package contract tests | AUTOMATED PASS |
| P14-A14 | GitHub-hosted CI 不执行 absolute 550 ms/resource qualification | Automated | workflow governance trace | AUTOMATED PASS |
| P14-A15 | USER 授权的 candidate defect correction 与资格化加固保持边界内、无新 runtime dependency 且有命名回归 | Automated | git path/dependency audit + source/preview multiline and grapheme selection isolation、source projection/stale-preview/hard-break/theme-math-cache、missing-canonical recovery/note-ack/Keep-Local force-receipt barrier、fixed-temp/recovery/export evidence ownership、native-drag/direct-redock、zoomed toolbar paint/hit alignment、线性 semantic conversion/search、USER rendering-stress Markdown/RaTeX/layout/lazy-image/source-safety regressions | AUTOMATED PASS |
| P14-A16 | exact candidate Release、headless、Runtime、Performance、Resources receipt 独立绑定；runtime smoke 使用有界 reducer、真实 shell/source projection ready gate，并以 fail-closed physical input 驱动快捷键、toolbar view controls、拖动、失焦、Left -> Top -> Left -> Right dock/auto-hide、Right Pin-ON 正交路径、顶角优先级与真实 compact resize；sensor reveal 显式建立 tracked leave，view-mode/image 压力通道分别证明画面投影与 durable config | Automated | Phase 10 targeted Runtime、Phase 14 qualification campaign、`window-stress` collapse/view-mode parser 与 copied-Release runtime contract tests；动态 receipt 不回写本表 | AUTOMATED PASS |
| P14-A17 | CI tests/performance 分片并发时，任务并集严格等于完整 `all --ci` 且失败日志同时保留 test stdout 与 Cargo stderr | Automated | smoke CLI shard-union/output-capture tests + `.github/workflows/ci.yml` | AUTOMATED PASS |
| P14-A18 | 日常 Resources 可按 source-preview/math/images/window/zoom 定向运行；完整候选仍要求全矩阵 | Automated | `phase-14.ps1 -Resources -ResourceModule <module>` + smoke task-plan tests | AUTOMATED PASS |
| P14-A19 | Portable Release 静态链接 MSVC CRT，Rust CLI 同时检查普通与 delay-load PE imports，CI 在打包前拒绝外置 developer runtime | Automated | `.cargo/config.toml` + `qualification native-runtime` + CI/release workflow trace | AUTOMATED PASS |
| P14-A20 | Split 语义同步默认开启、可持久关闭；双向手势单向映射、stale generation guard、无反馈环且保留独立位置 | Automated | render anchor-index + app reducer/config + Rust CLI regression | AUTOMATED PASS |
| P14-A21 | Source 纯文本查找/替换支持大小写开关、wrap、单次/全部替换、generation invalidation、Unicode boundary 与单事务 Undo；关闭后释放匹配投影且不再扫描，外部重载刷新打开会话，搜索焦点不吞掉全局保存/导出；不含正则 | Automated | interaction/flow/core tests + Rust CLI workspace-test shard regression | AUTOMATED PASS |
| P14-A22 | 数学分隔符转换控件以清晰的 `$` 标识，paint/hit geometry 在 50/100/300% 一致 | Automated | toolbar paint/hit contract + headless CI | AUTOMATED PASS |
| P14-A23 | Release 内存按 Source/Preview/Split/cache 分模块归因；优化决策有实测依据且不放宽既有 hard gate | Automated | targeted resource modules + `docs/report/phase-14-memory-attribution.md` | AUTOMATED PASS |
| P14-A24 | 至少 100 个独立 copied-Release 桌面运行且全部失败仅为已分类输入/调度抖动时，成功率 `≥98%` PASS、`<98%` FAIL；内容/数据安全/崩溃/resource hard gate 永不容错 | Automated | repetition boundary tests + window-stress jitter/blocking classifier tests + `docs/plan/11_testing_and_release.md#desktop-repetition-jitter-policy` | AUTOMATED PASS |
| P14-A25 | G3 exact automation 串行使用隔离候选目录；Rust 持有 clipboard/export/kill/recovery/asset 断言，UIA 只适配原生对话框/tray；receipt 对 source/harness/clean tree/EXE/ZIP/五项结果 fail closed | Automated | `qualification g3` parser/receipt tests + `tools/stickymd-smoke/helpers/windows-uia.ps1` boundary audit；GitHub-hosted CI 只运行无界面子集 | AUTOMATED PASS |
| P14-A26 | G4 exact automation 串行使用隔离候选目录；Rust 持有 tray lifecycle、三边 dock/时序、legacy shortcuts、真实数学转换与 junction 单实例断言；Dock 拖动区分 requested-position 与 application-resolved 终态，DIP 阈值不与固定 physical-pixel 容差混用；tray UIA 对物理右键执行 menu-open acknowledgement、一次有界重试与几何/菜单诊断；receipt 对 source/harness/clean tree/EXE/ZIP/六组结果 fail closed | Automated | `qualification g4` parser/receipt/unit contract tests + 150% DPI snap-normalization regression + UIA adapter governance；UIA 只适配 tray；GitHub-hosted CI 只运行无界面子集 | AUTOMATED PASS |
| P14-A27 | G5 exact automation 串行验证 ToolWindow shell identity、220×120 Source/Preview/Split mechanics、50/100/300% zoom、40 opacity、主题循环以及 Markdown/math/image stress，并把逐候选截图 path/SHA-256 绑定到 receipt；真实 IME、mixed-DPI 与首次视觉判断仍由人工持有 | Automated | `qualification g5` parser/receipt/unit contract tests；UIA 只负责窗口截图，不持有判定；GitHub-hosted CI 只运行无界面子集 | AUTOMATED PASS |
| P14-A28 | Preview 选择保留 Cosmic Text shaping cluster 几何；Times/CJK/Emoji/组合字符/换行/BiDi 的 hit-test、蓝框与 copy range 同源，几何仅缓存当前 viewport，禁止整段比例估算 | Automated | render viewport-cluster geometry unit/integration tests + Phase 14 headless tests shard + Release baseline（5,000 rows；viewport projection p95 19.1 µs；10,000 hits 340.8 µs） | AUTOMATED PASS |
| P14-A29 | 查找/替换使用单一 session；Ctrl+F toggle、Ctrl+H expand、Find-only replacement guard、方向键导航、字段 caret/mouse/IME geometry 与源码 caret 隔离均有回归 | Automated | interaction/render/app unit tests + Phase 14 headless shard | AUTOMATED PASS |
| P14-A30 | exact candidate 使用真实 Microsoft Pinyin 与 WeType profile，以物理键盘验证 Source/Search composition、commit/cancel、selection replace 与一次 Undo；首次 ordinary ASCII 的唯一物理 Shift 纠正后重新确认 profile/route/open/native，再做唯一一次复探针；测试结束恢复原 profile/mode | Automated exact candidate | recovery-plan unit regression + `phase-14.ps1 -G4 -G4Case G4-06`；完整 G4 receipt 必须包含 G4-06 | NOT TESTED |
| P14-A31 | smoke 启动的每个 StickyMD GUI child 都由 RAII owner 持有，普通错误返回或 unwind 会执行 kill + wait；Performance/Resources 在启动前对已遗留的 smoke-owned 测试进程 fail closed，且绝不自动终止用户自己的便签实例 | Automated | `managed_process::tests` + runtime isolation-scenario tests；正式测量前置检查 | AUTOMATED PASS |
| P14-A32 | Local Preflight Build 与 Release Exact Artifact 分离；qualification CLI 按 recorded run/name 自行下载并与用户副本逐字节比对，只有 successful remote artifact 通过 checksum/SBOM/package/runtime 后可 Promote 到 canonical ignored staging；candidate receipt 绑定 run/attempt/artifact id 与实际 ZIP/EXE/SBOM hash | Automated | qualification source-freeze/remote/downloaded/candidate resolver tests | AUTOMATED PASS |
| P14-A33 | Promote 或 candidate identity 改变后 exact-byte evidence fail closed；功能资格化只在相关产品/共享/harness/contract 输入指纹改变时 stale，无法分类的 tracked path 保守失效 | Automated | module registry/fingerprint/readiness evidence-class tests + central candidate resolver regression | AUTOMATED PASS |
| P14-A34 | Candidate workflow 与 publish promotion workflow 分离；tag/draft/publish 只接受显式 source/run/hash/tag 输入并复用同一 artifact，禁止 tag 触发重建或自动选择另一 build | Automated | release workflow governance + operation separation audit | AUTOMATED PASS |
| P14-A35 | 每个 qualification module 只持久化最后一次完整成功记录；相同指纹显示来源 candidate 的 `REUSED PASS`，PASS 原子更新，FAIL/ABORTED/环境中止不创建或覆盖成功记录 | Automated | last-success ledger atomicity、failure non-overwrite、impacted planner 与 origin identity tests | AUTOMATED PASS |
| P14-A36 | 已开始执行的 smoke task 失败或环境中止时仍原子写入本轮 JSON、已收集的 measurements/gates/samples；保持非零退出码、不执行后续任务、不更新成功账本；证据写入失败时同时保留原始错误 | Automated | runner failure-evidence regression + evidence failure/NOT_TESTED ledger-isolation tests | AUTOMATED PASS |
| P14-A37 | Resources 在开始及每个主要场景完成后持久化显式 `INCOMPLETE` 收据，保留已完成测量；最终完整成功才清除该标识；checkpoint 写入失败不得把部分结果登记为成功 | Automated | `runner/resource_progress` 的旧收据替换、测量保留、写入失败和成功账本隔离回归 | AUTOMATED PASS |
| P14-A38 | 资源矩阵的无公式及 1/20 公式样本仅包含指定数量的公式与图片；修复资源 padding 不改变既有 startup fixture bytes；fixture 变化使对应模块指纹失效 | Automated | runtime fixture 数量/UTF-8/尺寸和 startup SHA-256 回归；module fingerprint 实际文件变更回归 | AUTOMATED PASS |
| P14-A39 | G3/G4/G5 的普通 case 失败仍原子写出本轮完整结果，保留已通过项目和原始错误；失败不得覆盖 last-success，证据写入失败同时报告两项错误 | Automated | `exact_desktop/evidence` 失败收据替换与账本隔离回归 | AUTOMATED PASS |
| P14-A40 | 窗口资源压力循环结束后原子恢复原始 20 KiB 文档，等待真实 Source 投影与 clean 状态后才开始隐藏预热；每轮采样后逐字节校验基线并记录文档长度，不一致不得形成完整成功收据 | Automated exact candidate | `phase-14.ps1 -Resources -ResourceModule window` 定向回归；正式证据属于完整 Resources receipt，包含五次 `hidden-to-tray.run_N.fixture_bytes` 与真实 Source projection acknowledgement | NOT TESTED |
| P14-A41 | G5 与 headless 回归共享同一份 PNG/JPEG/WebP/GIF bytes；验证实际产品解码、格式、尺寸、不透明彩色区域与 bytes 保留；图片变化必须使 G5 成功指纹失效 | Automated | render `qualification_images` 集成测试 + module fingerprint 实际文件变更回归 | AUTOMATED PASS |
| P14-A42 | 同一资源命令的 19 个基础/数学/图片场景名称对应 15 份独立五次采样；四个等价别名记录来源，Preview→Source 历史不误合并，失败 cohort 不复用；固定等待差为 1500 秒 | Automated | resource plan、实际 fixture bytes/config 等价性与失败缓存回归；预算不是实测加速 | AUTOMATED PASS |
| P14-A43 | 局部/筛选请求不能写正式 Runtime/Performance/Resources 路径；资源子收据只能由完整入口持有；缺项、重复项、错误次数或单个 sentinel 不能代表完整覆盖 | Automated | smoke scope、resource coverage 与 formal task-plan 回归 | AUTOMATED PASS |
| P14-A44 | 每个完整资源组独立原子保存 last-success，后续失败保留兄弟成功；指纹或 candidate 在测量期间变化拒绝登记；readiness 缺任一组均阻塞，旧 aggregate 不自动导入 | Automated | module ledger、input/candidate drift 与逐组移除 readiness 回归 | AUTOMATED PASS |
| P14-A45 | 共享窗口/进程适配器使所有消费者失效；独立 window/zoom 资源 harness 变化只使对应资源组失效；样本结构在启动等待前预检 | Automated | 实际文件变更 fingerprint 回归 + fixture preflight 回归 | AUTOMATED PASS |
| P14-A46 | 新资源执行器在独占 Windows 桌面完成五组完整资格化，验证真实 CPU/内存、共享 cohort 来源、中断重跑与实际耗时 | Automated exact candidate | 完整 `phase 14 --resources` canonical 入口；必须使用新 Source Freeze / Promoted Candidate | NOT TESTED |
| P14-A47 | Promoted Candidate 测量计划执行独立候选校验并省略未使用的本地 Release build；候选缺失/损坏阻断后续任务，正式覆盖不能用本地 build 冒充候选校验；preflight/package/release/CI 保留原构建 | Automated | candidate input planning、失败中止与 formal task coverage 回归 | AUTOMATED PASS |
| P14-A48 | Resources 的 any/max 硬门在完整样本已超限时立即失败；原始样本、实际次数及门槛进入失败 JSON，不覆盖五类成功账本；阈值相等允许通过，成功仍须五次，startup p95 与 60 秒 CPU 窗口不变 | Automated | resource cohort 边界/首轮与后续轮失败、失败 JSON/账本隔离及指纹传播回归；原生实测仍由 P14-A46 持有 | AUTOMATED PASS |
| P14-A49 | 正式 Runtime/Performance 仅共享同机同输入的完整 workspace tests；绑定 clean Source Freeze、全部仓库 bytes、实际 harness、工具链和执行设置，前后身份变化或失败不登记，未知配置绕过缓存；partial/CI/损坏收据不能冒充完整成功 | Automated | `qualification/workspace_tests` 可控执行器与真实 Git/freeze 身份回归；不替代原生桌面验收 | AUTOMATED PASS |
| P14-A50 | 资源规划只枚举一次文件清单，每组只计算一次指纹供兼容性检查；摘要协议、证据完整性校验不变，复用/登记仍重新读取当前输入 | Automated | planned/fresh digest 一致性、新增文件失效与归档损坏拒绝回归 | AUTOMATED PASS |
| P14-A51 | 任务失败也保留执行用时和已有错误/观测；共享测试的本轮身份核对耗时与历史执行耗时分开记录，资源规划输出总计/逐组用时 | Automated | runner timing、shared prerequisite measurements 与失败留证回归 | AUTOMATED PASS |
| P14-M01 | Microsoft Pinyin / WeType 候选窗位置、遮挡、字体、动画及 DPI 视觉质量 | Guided Manual | exact candidate G1；自动化矩形/截图只能作 companion evidence | NOT TESTED |

P14-A40 Preconditions：隔离 portable 目录、固定资源 fixture、独占交互桌面。
Action：运行完整窗口资源模块，包括首轮持久化/图片压力循环和五轮隐藏采样。
Expected：五轮隐藏采样均使用原始 20,480 bytes，首轮必须先观察到恢复后的编辑器内容；
恢复或校验失败时返回非零，不写完整成功账本。
Failure Signals：首轮遗留压力文档、仅恢复磁盘而未等待投影、样本不一致仍报告通过。
该工具回归通过不替代新候选的完整 Resources 资格化。

P14-A41 Preconditions：G5 使用的共享图片文件及锁定的产品解码器。
Action：执行 render 图片样本集成测试及模块指纹变更回归。
Expected：四种格式均完整解码为 96×64 不透明彩色图；仅改变图片 bytes 使 G5 需要重跑。
Failure Signals：只检查文件头、允许损坏图片作正向样本、解码为空白、变更后错误复用 G5。

P14-A33 并发回归补充：同一个时钟值下并行申请指纹流和测试输入目录时，路径仍须唯一；
指纹流使用 exclusive create，创建失败不得截断或清理其他调用持有的文件。
固定时间戳回归与既有模块输入变更回归共同验证这一要求。

P14-A47 Preconditions：相同测量参数，分别使用无 freeze 的本地 preflight、正式候选请求，
以及缺失/损坏候选的隔离 fixture。
Action：检查实际任务计划并执行候选失败路径。
Expected：正式请求先验证候选，不执行本地 EXE 构建；失败返回非零且不执行后续任务。
Failure Signals：默默回退到本地 EXE、缺少候选校验仍可登记正式完整成功、影响 source-only 构建。

P14-A48 Preconditions：可控的五轮 CPU/内存样本和已有成功账本。
Action：分别注入第一轮、中间轮、最后一轮超限、阈值相等及负的缩放内存增长。
Expected：完整超限样本立即返回失败并保留该样本；此前样本不裁剪，实际次数不补成五次；
所有成功账本保持不变。未超限路径完整执行五次，负增长不被误判失败。
Failure Signals：超限仍继续等待、丢失失败样本、部分样本被标成完整通过、失败覆盖成功账本。

P14-A49 Preconditions：隔离临时 Git 仓库与有效 Source Freeze，或可控的输入身份/执行器；
正式 Runtime/Performance 请求以及诊断、partial、CI 对照请求。
Action：依次运行两通道的共享前置任务，改变源码/执行身份，注入测试失败、损坏收据、
记录写入失败和不支持的配置；真实 Git fixture 在冻结后再次编辑源码。
Expected：相同身份只执行一次完整命令并明确输出复用来源；变化时重跑或拒绝登记，
未知配置每次完整执行且不更新缓存；dirty tree 拒绝开始，旧成功不被失败覆盖。
Failure Signals：筛选测试可登记、身份变化仍跳过、失败形成成功、来源或证据类型混淆。

P14-A50 Preconditions：具有 tracked/untracked 输入和已有归档成功的隔离仓库。
Action：比较批量规划与旧字节协议摘要，覆盖空/二进制/中文路径及跨缓冲区输入；规划后
修改/新增/删除输入，并注入读取失败、临时路径冲突及归档损坏。
Expected：每个共享输入本批只读取一次，各组摘要与原协议一致；新的独立检查发现输入变化；
失败不返回部分成功、不遗留本次临时流、不改动冲突路径；预计算摘要不能绕过归档校验。
Failure Signals：修改指纹协议、把规划快照用于最终登记、只凭摘要字符串信任损坏证据、
失败遗留临时输入或清理他人文件。

P14-A51 Preconditions：带原始错误/观测的失败任务，以及实际执行和复用两种前置任务。
Action：记录任务用时并检查 JSON 测量项，检查资源指纹批次总计与逐组兼容性检查输出。
Expected：失败详情与观测保持原样；历史运行用时仅标为 origin，本轮用时单独记录。
Failure Signals：计时覆盖错误或样本、把历史时长当成本轮测量、宣称未经实测的整体提速。

P14-A42/A43 独立 review 回归补充：每个资源子收据必须携带每个 cohort 的五份原始观测与
既有硬门。共享观测保留 `shared_from`，缺样本/硬门、重复 run、错误单位/统计、非等价来源、
混用新测与复用来源或既有门超限均拒绝登记。Windows verbatim、短路径与 junction 别名
及尚未创建的目标文件不能绕过正式/内部路径保护，失败诊断不得覆盖旧成功。

P14-A49 独立 review 回归补充：从仓库子目录启动并使用相对 `CARGO_HOME`，修改实际
Cargo 工作目录下的配置必须改变缓存身份；不支持的配置禁止复用，真实 Cargo 失败仍保留。
空 `CARGO_HOME` 必须捕获用户目录的回退配置；Windows `C:cache` 等带盘符的相对路径
禁用共享，仍执行完整测试。

## Guided manual sessions

下列 session 是交互记录入口，不替代 `phase-12.md` 的 P12-M01..M44 authority。

| ID | Scope | Mode | Underlying cases / Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-G1 | Editor / IME / rendering | Guided Manual | P12-M01,M02,M21,M22,M24..M26 | NOT TESTED |
| P14-G2 | focus recovery / mixed-DPI dock / compact visual / theme | Guided Manual | P12-M05,M11,M12,M18..M20,M23；可复核 G5 截图以减少重复操作 | NOT TESTED |

## Exact-candidate automated desktop session

| ID | Scope | Mode | Underlying cases / Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-G3 | clipboard / native export / process-kill recovery / asset safety | Automated exact candidate | `phase-14.ps1 -G3`; P12-M28..M30,M32,M33；独立 `g3-exact-qualification.json` | NOT TESTED |
| P14-G4 | tray lifecycle / dock timing / legacy shortcuts / math conversion / junction identity / real IME functional matrix | Automated exact candidate | `phase-14.ps1 -G4`; P12-M06..M10,M13..M17,M27,M31,M44 + P14-A30；独立 `g4-exact-qualification.json` | NOT TESTED |
| P14-G5 | shell identity / compact / presentation / rendering mechanics | Automated exact candidate | `phase-14.ps1 -G5`; P12-M03,M04，并为 M05/M18..M26 提供候选绑定的机械与截图 companion evidence；独立 `g5-exact-qualification.json` | NOT TESTED |

## Readiness interpretation

- Tier A manual facts require exact PASS or explicit case/group waiver。
- Tier B requires exact PASS or version/source-bound USER disposition。
- Tier C `NOT TESTED` is nonblocking only while corresponding automated coverage is PASS；FAIL blocks。
- PUSH、TAG、DRAFT-RELEASE、PUBLISH 均不由本矩阵授权。

## 2026-09-22 release CLI maintenance verification

本节是 plan 11 既有发布与验证合同的工具维护投影，关联 P09-D061/D066..D082、
P14-A19/A32/A34，不修改上面的历史 source baseline 或人工状态。

Preconditions：当前工作树、锁定 Cargo 依赖、隔离 fixture 与本次新构建的本地验证包。
Action：执行 smoke crate tests、对应 PowerShell 薄入口及一次串行 package runtime 检查。
PowerShell 5.1/7 的 headless wrapper 回归可作为两个 Rust tests 并发：共享只读仓库与已编译 CLI，
各自持有带 edition 标识的临时输入/输出目录和独立子进程环境/控制台；同一 edition 内的状态变更案例
仍依序执行。两版均须保留成功/失败、Unicode 路径和调用者状态断言；5.1 必须存在，7 缺失显式记为
`NOT_TESTED`。这不允许并发执行 package runtime、GUI 或资源测量。
Expected：合法输入保持输出语义；下列错误返回非零，失败不写候选/资格化收据；
notices 拒绝覆盖，SBOM/manifest 的输出失败边界由 REL-CLI-08/09 明确。
Failure Signals：接受缺失/冲突来源、哈希不符、重复 manifest、危险路径、缺失许可证；状态恢复失败；
把局部工具通过描述为 exact candidate 或人工验收通过。

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| REL-CLI-01 | full source SHA、tag/version、预期 ZIP/SBOM hash 与唯一 README 来源绑定 | Automated | Rust `release::identity/promoted` + `release_wrappers` | AUTOMATED PASS |
| REL-CLI-02 | checksum 恰好绑定不同名称的 ZIP 与 SBOM，拒绝角色重名、重复、缺失、危险名称和错误 hash；空文件与含十六进制名称的路径按实际 bytes 哈希；candidate receipt 复用同一实现 | Automated | Rust `integrity::tests` + `cli_exit` + PowerShell 两版本失败路径 | AUTOMATED PASS |
| REL-CLI-03 | allowlist、路径安全、30 MiB 边界、explicit ZIP 与 manifest 所指文件一致、PE 与原生资源验证 | Automated | Rust `release::package/package_rules`、`pe_dependencies` + 新本地包检查 | AUTOMATED PASS |
| REL-CLI-04 | Cargo normal-edge 闭包、build/dev 分类、本地依赖传递、循环终止、稳定排序与许可证选择/拒绝 | Automated | Rust `release::notices` + 迁移前后同一锁图输出逐字节比较 | AUTOMATED PASS |
| REL-CLI-05 | PowerShell 5.1/7 保留输出、失败退出码、Unicode/空格路径、CWD 和编码恢复；相对路径按调用者实际目录解释，兼容 8.3 TEMP 别名；子进程不继承不兼容模块路径且父进程环境不变；notices 拒绝覆盖 | Automated | `tests/release_wrappers.rs` + `atomic_evidence` 并发新文件测试 | AUTOMATED PASS |
| REL-CLI-06 | ASCII/空格/中文隔离启动、同目录第二实例退出且 durable files 不变、不同目录进程独立存活；遗留 package-test child 阻断后续测量且不被自动终止 | Automated local | 本次新构建包的 `verify-package.ps1 -Runtime` + `managed_process::tests`；结果见维护报告 | AUTOMATED PASS |
| REL-CLI-07 | package naming/dirty/tag/exact 策略在 Rust 单点实现，计划不生成验收收据；版本只取 workspace.package，兼容赋值空白并拒绝缺失/歧义 | Automated | Rust `release::package_inputs`、`repository` 与现有 `package_path` 回归 | AUTOMATED PASS |
| REL-CLI-08 | Syft 先写隔离临时文件；失败、非法 UTF-8/JSON、错误 SPDX 版本、空 packages 或缺失必需文件均不得替换既有 SBOM/manifest；包验证复用同一结构/覆盖规则，即使 hash 正确也拒绝非法 SBOM | Automated | Rust `release::sbom::tests`、`cli_exit` + PowerShell 5.1/7 `release_outputs.ps1` 行为回归 | AUTOMATED PASS |
| REL-CLI-09 | checksum 生成复用严格名称/hash 规则；拒绝输入输出别名和非法目标；每个输出原子替换且 manifest 最后写入；manifest 替换失败返回非零，不匹配的文件组合不能通过验证 | Automated | Rust `release::checksums::tests`、`release::sbom::tests` 的已知摘要、已有输出保护、Windows 文件锁与残留临时文件回归 | AUTOMATED PASS |
| REL-CLI-10 | ZIP 中三份受控许可证文本均为非空 UTF-8、无 BOM、LF；保留 checksum/SBOM 输出接口、Unicode/空格路径及 Syft 环境恢复 | Automated | `release_wrappers` 在 PowerShell 5.1/7 实际打包并读取 ZIP 成员和生成输出，不依赖脚本函数名/调用次数 | AUTOMATED PASS |

详细运行环境、数据及未验证项见 [维护报告](../report/2026-09-22-release-cli-migration.md)。
SBOM 与 checksum 收尾记录见 [输出维护报告](../report/2026-09-25-release-output-finalization.md)。

REL-CLI-05 notices 输出预检：Given 已存在的目标文件/目录或缺失的父目录，且当前 fixture
没有可用的 Cargo manifest，调用 notices 生成。Expect 在依赖读取之前返回对应输出路径错误，
原文件字节和目录保持不变，不生成临时输出；合法目标仍须通过完整依赖和许可证检查。
最终发布继续使用原子 no-replace 操作，预检后的并发创建不得被覆盖。
Failure Signals：先报 Cargo/许可证错误、生成部分文件、创建缺失父目录或覆盖已有目标。
