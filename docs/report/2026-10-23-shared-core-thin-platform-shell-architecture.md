# 2026-10-23 — 三端共享核心与平台薄壳架构提案

- 状态：USER APPROVED（2026-10-31，见文末 Resolution）；跨模型审查六轮，第六轮 AGREE、无阻断项（见第 13 节）。
- 基线：`1d943497b91b704a34e32b3150a923d5113a5f9d`；复核基线 `259e95000ffbbcc6c747530819d2a2878d3a9616`（第 2 节事实在此仍成立）。
- 用户目标：面向 Windows、Debian、macOS，把可共享实现集中维护；当前先整理共享部分和 Windows 壳，另外两端在对应平台实现。
- 当前工作树原有未跟踪文件：`docs/report/RISK-display-math-standalone-equals.md`；本提案不修改它。
- 本文是有时间属性的分析证据。批准后须把相关决定写入 `docs/plan/`，本文不替代工程合同。

## 1. 结论与授权边界

建议采用“领域核心 + 共享应用逻辑 + 共享渲染 + 共享桌面交互 + 平台宿主/适配器”。
这里的共享核心是一组职责明确的模块。现有 `stickymd-core` 继续只持有平台无关领域模型；
应用协调与窗口呈现分别有自己的共享位置。三端共享规则、流程、绘制与通用交互，平台部分只提供真实系统能力。

用户已明确三端方向、先抽共享实现、后做另外两端壳的顺序。本报告将这一方向作为既定目标。
本轮拟新增共享应用入口、平台能力接口与跨 crate 的所有权边界，属于需先评审的关键接口结构调整。
依 [根 AGENTS](../../AGENTS.md) 与 [工程宪法 10.1.3](../plan/00_engineering_constitution.md)，
当前交付具体提案；批准后更新合同并实施。Windows 当前产品行为与数据布局作为迁移基线。

## 2. 已核实的现状

| 证据 | 当前事实 | 对提取的意义 |
| --- | --- | --- |
| [领域核心](../../crates/stickymd-core/src/lib.rs) | DocumentState、generation、undo、持久化事实类型已独立；禁止 unsafe | 保留其领域边界，不引入窗口或文件系统执行 |
| [渲染库](../../crates/stickymd-render/src/lib.rs) | Source、Preview、数学、图片与滚动计算已独立；禁止 unsafe | 继续复用渲染与投影，平台字体策略另行注入 |
| [现有应用入口](../../apps/stickymd-win/src/main.rs) | 大部分应用模块受 `cfg(windows)` 控制，非 Windows 入口只输出平台限制 | 非 Windows 可编译入口不能证明共享应用已可运行 |
| [协调层](../../apps/stickymd-win/src/flow/mod.rs) | 已有 Editor、Persistence、Preview、Recovery、Save、Window coordinator 与 ClipboardPort | 可以迁移成熟职责和测试，不必重写这些状态机 |
| [保存回执处理](../../apps/stickymd-win/src/app/persistence_runtime.rs) | 回执校验、冲突、再次保存、恢复与退出屏障和窗口操作仍在同一运行时对象里 | 必须拆开应用裁决与窗口呈现，单纯移动目录不够 |
| [视图切换](../../apps/stickymd-win/src/app/preview_runtime.rs) | Search 的组字守卫、配置提交、预览调度与投影刷新混合组织 | 应用规则与共享呈现分别归位 |
| [文件持久化](../../apps/stickymd-win/src/persistence/storage.rs)、[资产存储](../../apps/stickymd-win/src/assets/storage.rs) | Windows 文件身份、元数据与受保护操作不只出现在 `platform/windows` 目录 | 提取边界不能机械地按现有目录划线 |
| [字体策略](../../crates/stickymd-render/src/source/fonts.rs) | 现有候选字体侧重 Windows 字体；预览代码字体另有回退 | 可共享字体选择算法，各平台给出默认字体策略 |
| [CI](../../.github/workflows/ci.yml)、[治理检查](../../tools/stickymd-smoke/src/governance.rs) | 已有 Linux core/render 构建测试配置；部分检查硬编码 Windows 应用源路径：`forbid(unsafe_code)` 只查 core/render 的 `lib.rs`，窗口协调的平台类型禁令只扫 `apps/stickymd-win/src/flow/window` | 必须随每批移动同步迁移检查规则与任务选择，不能保留失效路径或绕过检查 |
| [窗口隐藏](../../apps/stickymd-win/src/flow/window/lifecycle.rs)、[窗口效果执行](../../apps/stickymd-win/src/app/window_runtime.rs) | `complete_hide` 先把状态设为 `HiddenToTray`，再发出 `SetVisible(false)` 与释放缓存；执行端调用 `set_visible` 后没有失败回执 | 状态机不能原样搬迁：共享层须区分“已请求”与“已执行”，否则无法表达第 5 节第 6 条的失败语义 |
| [I/O 回执](../../apps/stickymd-win/src/persistence/worker.rs) | `IoCompletion::External`、`TemporaryRemoved`、`CanonicalPreserved` 不带 generation 或操作身份；正确性依赖当前串行执行与状态守卫 | 与 [AGENTS](../../AGENTS.md) 的 generation 禁令不符的既有偏差；在 Windows 上单独修复，迁移时不能把隐含依赖当作已有版本校验 |
| [帧缓冲呈现](../../apps/stickymd-win/src/surface.rs) | 局部更新假定 Win32 softbuffer 的单个持久 DIB：只在 `age == 0` 时全量填充，其余情况只写 damage 区 | 共享前须改为只在 `age == 1` 时局部更新；Wayland 双缓冲（`age == 2`）与 macOS（每帧新缓冲、忽略 damage）不满足该假设 |
| [工作区依赖](../../Cargo.toml) | winit 与 softbuffer 关闭默认 features（winit 仅启用 `rwh_06`），依赖只在 Windows target 下引入 | 共享 desktop 在 Linux 上需显式选择 X11/Wayland backend features，否则无法编译 |

以上是当前源码与配置事实，不是本轮测试通过声明。没有运行 Linux/macOS 桌面程序，未验证两端输入法、性能或发布包。

## 3. 目标模块与依赖方向

| 模块 | 拥有的职责 | 边界 |
| --- | --- | --- |
| `stickymd-core`（现有） | 文本、编辑 delta、撤销重做、generation、资产引用等领域状态与纯计算 | 不依赖窗口、渲染、平台接口实现或磁盘执行 |
| `stickymd-render`（现有） | Markdown/数学投影、排版、绘制、命中与选择映射、布局缓存 | 不决定保存、冲突或资产删除；环境读取经执行接口 |
| `stickymd-app`（新增） | typed intents、应用会话、保存/预览/冲突/恢复/资产/生命周期协调、配置状态、能力接口与可共享执行逻辑 | 不直接依赖 winit、softbuffer、Windows API；不暴露可变 DocumentState 给壳 |
| `stickymd-desktop`（新增） | 通用工具栏、Search 控件、选择与滚动交互、窗口事件翻译、Source/Preview/Split 呈现、软件帧缓冲接线 | 可使用现有 winit/softbuffer；不持有保存/冲突/GC 的业务裁决 |
| `apps/stickymd-win`（保留） | Windows 启动与宿主接线、Win32 适配、资源/manifest、平台验证探针接入 | 提供平台事实和执行回执；不复制应用规则 |
| 未来 Linux/macOS 宿主 | 对应平台启动接线与原生能力实现 | 本阶段不创建伪实现或声称可用的应用入口 |

编译依赖方向：

```text
平台宿主 ──→ stickymd-desktop ──→ stickymd-app ──→ stickymd-core
    │               │                  │
    │               └──────→ stickymd-render ──→ stickymd-core
    │                                  ↑
    └── 实现并注入 app 定义的能力接口    └── app 可调用渲染能力
```

图中各共享库不反向依赖平台宿主。能力接口的定义属于共享层，具体实现由宿主提供；
运行时仍遵守 Shell → Instruction → Coordination → Execution 的调用链。
`stickymd-app` 内须区分 instruction、coordination、execution、ports 模块，crate 名称不取代原四层结构。
平台宿主内的启动接线属于 Shell，原生文件/剪贴板/窗口适配属于 Execution Domain；二者不能因同处一个 crate 而越层调用。

共用的 UI 仍属于 Interaction Shell。将它集中维护不会使它成为文档权威，也不会使界面操作获得直接写文件的权限。

## 4. 从当前实现到目标位置

| 当前区域 | 目标归属 | 迁移方式 |
| --- | --- | --- |
| `instruction/`、`flow/editor.rs`、`flow/save.rs`、`flow/persistence.rs`、`flow/recovery.rs`、`flow/reconciliation.rs` | `stickymd-app` | 保留 intent、generation、失败语义和既有测试 |
| `flow/preview.rs`、预览任务版本裁决 | `stickymd-app` | 保留 debounce、旧结果丢弃、缓存生命周期规则 |
| `config/runtime.rs`、`config/coordinator.rs` | `stickymd-app` | 运行时配置只有一份权威；TOML 编解码可共享，真实写入走存储能力 |
| `source_search.rs`、`interaction/search.rs`、`interaction/session.rs` 中的纯会话逻辑 | `stickymd-app` | Search、preedit、selection 会话不依赖原生事件类型；遵守 04/07 的原有所有权 |
| `flow/window/` 的几何计算、守卫和状态协调 | `stickymd-app` | 共享平台中立规则；接收可用能力和显示器事实，不导入 HWND 或系统句柄；B 阶段增加窗口动作的受理/完成/失败转换（如隐藏只在执行回执成功后进入 `HiddenToTray`、释放缓存），由 app 提交或恢复状态 |
| `app/*_runtime.rs` 的保存、恢复、资产、退出与预览裁决 | `stickymd-app` | 逐条调用链提取；把重绘、窗口动作和原生错误呈现分开 |
| `app/controls/`、工具栏绘制、滚动条、选择呈现、Search 绘制、`surface.rs` | `stickymd-desktop` | 共用绘制与命中逻辑；原生光标叠加优化保留在 Windows adapter；`present_damage` 只在 `age == 1` 时局部更新，其他 age 全量填充并完整呈现，以 `age = 0/1/2` 测试覆盖 |
| `app/input.rs`、`interaction/navigation.rs`、视图切换与 IME 接线 | 两个共享模块按职责拆分 | 平台事件/修饰键映射在 desktop；文档与会话语义在 app；字体与系统约定由宿主提供 |
| `persistence/worker.rs`、`preview/worker.rs` | `stickymd-app` 的执行模块 | 提取队列、调度与结果协议；具体存储和唤醒由接口注入；保留 std 线程与有界队列；只提取已带 generation/操作身份的协议；第 2 节三类无版本回执的修复是 B 的入口依赖（见第 8 节），并保留“有效旧保存结果被接受、较新编辑仍 dirty”的现有语义 |
| `persistence/storage.rs`、`assets/storage.rs`、`assets/safe_boundary.rs`、`config/storage.rs`、导出执行 | 共享策略/编解码 + 平台存储适配 | Windows 文件身份与受保护操作仍由 Windows 实现；按事务边界拆分，避免拆散安全证明 |
| `startup/bootstrap.rs` | 共享启动流程 + 平台环境解析 | 共享恢复与载入顺序；程序目录、实例身份、实际路径与单实例唤醒由宿主提供 |
| `platform/windows/`、`build.rs`、Windows manifest 与 native diagnostics | Windows 宿主 | 原生能力保留集中管理，不移动到领域核心 |

具体移动顺序以实际依赖为准。共享层内保留现有 cohesive modules，不把所有状态、端口与处理分支塞进一个大型 Application 文件。

## 5. 能力接口必须保留的语义

接口按当前业务需要定义，不建立包罗所有系统调用的 Platform 大接口，也不把每个文件操作包装为一个虚函数。
下列是拟定职责，尚不是冻结的 Rust 签名：

| 能力组 | 共享层请求/使用什么 | 平台实现负责什么 |
| --- | --- | --- |
| Note 与 Config 存储 | 带版本/预期指纹的保存、载入、观察与恢复请求 | 原子发布、真实文件身份、冲突检测和准确的失败回执 |
| Managed Asset 存储 | 有事务身份的写入、恢复、移动与安全清理请求 | 目录与文件稳定性、所有权证明、无覆盖操作、按实际文件执行 |
| 本地图片读取与导出目标 | 受限的只读流、尺寸探测、导出目的地与事务结果 | 平台路径解释、原生对话框、安全打开与写入 |
| Clipboard 与 Shell launch | 规范化文本/图片/文件列表，以及已获准的打开请求 | 原生格式转换、系统调用、失败报告；不决定 canonical edit |
| Window 与 Monitor | 几何/焦点/可见性/显示器事实，以及经过协调的动作 | 原生实现、能力可用性、实际执行结果 |
| Host、单实例与唤醒 | 已解析环境、实例归属、第二实例通知和工作结果唤醒 | 系统目录与命名规则、IPC、原生事件循环接入 |

共同约束：

1. DocumentState、ConfigState 与各 coordinator 的权威保持单一。平台回调只上报事实，不能改 canonical text。
2. 文档任务携带 generation；资产/清理任务还保留独立 request_id / transaction_id。配置继续使用配置 revision。
3. 保存请求要完整携带预期指纹与操作模式。普通保存不能被宿主自动升级成强制覆盖。
4. 文件接口必须覆盖完整的安全操作边界。不能让共享层先检查路径，再让平台层凭字符串删除文件，产生新的检查与使用间隙。
5. 不能确认文件所有权或稳定性时，清理返回 deferred/失败，用户文件保持原状。平台错误码可作诊断，但共享逻辑依据语义化错误分类裁决。
6. `Unsupported`、暂时不可用、执行失败与成功分别表达。平台声明支持某能力不等于某次请求已经成功；失败时不能提交虚假的已保存/已隐藏状态。
7. 主线程限定的窗口、菜单和输入法能力不强加 `Send + Sync`。I/O worker 只获得允许在线程间传递的执行对象与不可变请求。
8. 共享调度使用注入的单调时间输入或可控时钟，沿用已有毫秒时间与有界队列语义。平台宿主负责唤醒，不能复制 debounce 与退出顺序。
9. 共享 crate 继续禁止 unsafe。第三方库可以持有经批准的平台后端，项目手写原生调用仍集中在 adapter；不要求依赖图中所有第三方后端都消失。
10. 平台默认字体、快捷键映射与原生菜单约定属于宿主配置/翻译。通用绘制、编辑意图和文档语义共用；Windows 现有默认值保持一致。
11. 窗口动作（显示、隐藏、置顶、定位）分为请求、执行回执与提交三步：协调状态只在回执成功后推进，失败或 `Unsupported` 时恢复或保持原状态，并有“隐藏被拒绝”的验证场景。
12. 呈现成本属于平台：共享绘制只给出 damage，是否局部呈现由 desktop 依 buffer age 与平台后端决定；不以 Windows 的局部更新成本代表其他平台。
13. 原生输入法的范围替换、删除周边文本等操作若需支持，由 adapter 转成带范围的 typed intent，文档修改权威不变；通用 preedit/commit 事件不承诺覆盖这些操作。

## 6. 用完整调用链约束提取

### 保存成功、失败与退出

编辑 intent → 共享 DocumentState 更新 → 共享 SaveCoordinator 调度 → 共享 worker 请求平台存储 →
带 generation 与指纹的结果 → 共享应用校验并推进 saved generation / conflict / recovery / quit barrier →
共享 desktop 更新状态呈现，平台执行已批准的隐藏或退出动作。

过期任务、保存失败、外部冲突、强制覆盖的重提交与最新 generation 追赶仍由共享应用裁决。
失败不解除必要的保存屏障，不释放未保存的内存文本，不由壳自行关闭进程。

### 输入法、Search 与 Preview

原生输入 → desktop 翻译成规范事件 → 共享 Editor/Search Session → typed intent → canonical edit →
共享保存/预览调度。普通组字流程中 Preedit 只更新会话投影，commit 才产生一次文档编辑与撤销记录。
输入法的范围替换与删除周边文本（第 5 节第 13 条）另成一条路径：adapter 把同一输入法事务（如 Wayland 一次 `done`
内的删除与提交）合成一个带范围的 typed intent，形成一次 canonical edit 与一条撤销记录；只有删除、没有字符串提交的事务
同样产生一次编辑。验证须含“只有删除、没有提交”的场景。
候选窗定位留给平台接线；Search 组字期间的视图切换守卫、停放/恢复与替换边界必须只有一份实现。
预览旧 generation 的结果丢弃，不能通过宿主回调覆盖新文档或新视图状态。

### 资产清理与窗口隐藏

共享应用先裁定 safe boundary、最新请求和引用集合，再请求平台证明并执行受限操作。
状态机根据焦点、组字、拖动、冲突等事实决定能否收起；原生窗口操作是否可执行由平台报告。
共享算法可以要求某种能力，但没有该能力的平台不能获得假成功，也不能悄悄改写产品行为。

## 7. 本阶段与未来平台的分界

本阶段保持单便签本体、Windows portable 目录身份、保存格式、换行兼容、配置字段和既有快捷键行为。
不改写已有 note/config/images，不增加网络、插件、WebView 或通用 async runtime，也不附带 parser 迁移。

未来平台需要明确，但本阶段不替用户定案的事项：

- Debian 的桌面环境、X11/Wayland 会话与支持矩阵。
- macOS 最低版本、CPU 架构、应用身份、菜单栏/窗口行为与发布签名。
- 非 Windows 的便签目录与实例身份。共享层接受已解析的环境事实，不沿用 Windows 大小写归一化或假设可执行文件旁一定可写。
- 不具备贴边定位、隐藏或置顶能力时的产品行为。当前阶段只建立明确的能力与失败边界，不批准新的降级交互。
- 两端默认字体、输入法验收矩阵、原生资源指标与性能门。输入法矩阵须含范围替换（macOS `replacementRange`）与删除周边文本（Wayland `DeleteSurroundingText`）；锁定的 winit 0.30.13 在这两处丢弃信息。
- 托盘接线与可达性：Linux 需接入 GTK 事件循环并确认存在 StatusNotifierHost，macOS 须在主线程、事件循环启动后创建；无托盘宿主或宿主重启时不得允许隐藏到不可恢复状态。
- 剪贴板 backend：arboard 默认走 X11/XWayland，纯 Wayland 需 `wayland-data-control` 且依赖 compositor 协议；Linux 剪贴板内容依赖所有者存活；macOS 使用 pasteboard types，不能复用 CF_HDROP/DIB 接线。验证须含多种格式同时存在时的格式优先级（与 Windows 现有优先级一致或经批准的差异）。
- macOS 呈现成本：softbuffer 0.4.8 的 AppKit 后端每次 `buffer_mut()` 分配并清零 `width × height` 像素、`age()` 恒为 0、忽略 damage，光标闪烁即为 `O(WH)` 全窗提交；验收须给出物理窗口尺寸、DPI、每帧分配/复制量与光标闪烁基线。

前一轮已核对的外部限制继续作为风险输入：锁定的 winit 0.30.13 对 Wayland 的
[主动定位与可见性控制](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html)、
[窗口层级](https://docs.rs/winit/0.30.13/winit/window/enum.WindowLevel.html)存在不支持项；
Apple 要求[签名 bundle 视为只读](https://developer.apple.com/library/archive/technotes/tn2206/_index.html)。
这些限制应进入未来壳层设计，不能因共享库可编译而被视为解决。

## 8. 分阶段实施与回退

建议把这次重构登记为 Phase 15，按 A–D 小步完成；当前仓库的正式 Phase 入口与矩阵到 14。

| 阶段 | 交付 | 验证与回退点 |
| --- | --- | --- |
| A：合同与共享应用基础 | 更新批准的 plan/projection；建立 `stickymd-app` 并同时登记其 `forbid(unsafe_code)` 治理、CLI 任务、Linux/Windows 测试选择与资格化指纹归属；先迁移 intent、纯 coordinator、配置模型、Search 与会话逻辑 | 原有语义测试随责任迁移且在新 package 下实际执行；Windows 用新库，避免生产代码保留双份权威；失败则回退该批代码移动 |
| B：执行接口与应用运行时 | 入口依赖：三类无版本 I/O 回执的 Windows 修复已合并，完成证据为身份失配时不产生副作用的单元测试与“有效旧保存回执被接受”的既有测试（修复在 `fix/io-receipt-identity` 分支，`1ca792f`，待审查）。按保存/恢复、预览、资产/导出、窗口/退出顺序接入接口，提取目前夹在 app runtime 中的业务裁决；窗口动作增加受理/完成/失败转换 | 每条链条完成后执行成功/失败/过期回执测试与“隐藏被拒绝”场景；保留 Windows 已有安全实现与资源边界 |
| C：共享桌面交互 | 建立 `stickymd-desktop` 并同时登记其治理、任务与指纹归属；明确各 target 的 winit/softbuffer backend features；迁移通用控件、绘制、命中和事件翻译（含 buffer age 修正）；Windows 保留宿主、原生适配和构建资源 | `stickymd-desktop` 在 Windows 与 Linux CI 上真实构建；Windows 桌面定向回归；字体、IME、可达性与帧缓冲必须实测；出现差异回退该链条 |
| D：边界和验证收口 | 迁移剩余业务分支；删除仅为迁移保留的冗余转发；最终审查治理、CI 选择与文档 | 完成依赖与平台泄漏审查、适用 baseline、Windows 验证；未测平台保持 NOT TESTED |

每批移动的出口（A–D 都适用，不推迟到 D）：

1. **治理随代码移动：** 同一提交内更新受影响的硬编码路径检查（unsafe 禁令、窗口协调的平台类型禁令等），新增业务 module 带 `plan_ref`；旧路径检查不能因代码迁出而空转。
2. **测试选择与指纹：** 验证本批新增、移动、删除路径的任务选择结果与全量任务并集；新路径缺规则或旧规则无命中即失败。旧收据保留历史身份，有效性按新指纹重新判断，纯搬迁不视为旧证据仍有效。
3. **数据字节兼容与回退：** 指定前一已验证检查点；在数据副本上执行无操作启动、保存与恢复，比对 note/config 的路径与预期字节、图片未被改写，再用回退到检查点的版本重新打开同一副本。受影响的 Windows 行为断言列为该批出口。
4. **热路径基线：** 实施前写明输入、delta、快照与帧的借用/移动边界，并固定基线 SHA、fixture 与测量口径；迁移后与同一基线比较，新增成本与既有行重建、帧复制分开计量。判定规则：
   - 硬门：[plan 10](../plan/10_performance_reliability.md) 既有的输入延迟与内存失败线（不是 Target 列）；每键全文快照次数、每帧复制字节与每次编辑触发的后台（预览/保存）构建次数不得超过基线。
   - 诊断项：每键分配次数与队列容量；增加时须在该批记录中说明来源，不单独阻断。
   - 缺少基线或测量证据时该批不得通过。阈值属于本提案的默认建议，批准时可由 USER 调整。

这是同一重构的四个里程碑。只完成 A 时不得宣称 Windows 已成为完整薄壳。
首次实施 Phase 15 时同时建立 `tools/smoke/phase-15.ps1` 与 `docs/acceptance-cases/phase-15.md`，
自动化任务、结果与证据仍由 Rust CLI 持有。既有 Phase 入口和历史资格化记录继续保留。

回退只针对本阶段代码/构建/文档变更，不执行用户数据迁移，不删除便签或图片，不改写发布 tag 与历史收据。
每批只保留一个实际生效的实现；必要转发不保存状态。若原生安全要求无法由拟议接口表达，收窄或修订接口后再继续。

## 9. 可验证的完成标准

1. `stickymd-core` 继续不依赖 app/render/desktop；app 不直接依赖 winit/softbuffer/Win32，桌面库不依赖平台宿主。
2. 编辑、Search、配置、保存、预览、冲突、恢复、资产与退出规则各有一个共享所有者；平台目录不出现第二套业务实现。
3. 共用的工具栏、Search、滚动、选择与命中逻辑只有一份。原生优化与能力差异有明确的 adapter 位置。
4. 新共享应用在 Windows 与 Linux CI 上执行真实构建和语义测试，`stickymd-desktop` 在两者上以明确的 backend features 真实构建，不用 `cfg(windows)` 把主体或测试排除后宣称跨平台。macOS 编译结果单独记录。
5. 以同一组测试覆盖保存失败、过期预览、旧资产回执、恢复冲突、退出屏障与能力不可用。测试替身仅证明接口和共享语义，不证明原生实现正确。
6. Windows 重新验证实际文件替换/冲突/图片安全、IME、托盘可达性、透明度、置顶、贴边、多屏与剪贴板/导出；自动化与人工结果分别记录。
   剪贴板须验证多格式同时存在时的选择，按 [plan 08](../plan/08_assets_and_export.md) 的检测优先级与当前实现顺序
   `CF_HDROP → 原编码 PNG/JPEG/WebP（保留编码格式别名顺序）→ CF_DIBV5 → CF_DIB → CF_BITMAP → Unicode text`：
   基线实现为 `259e950:apps/stickymd-win/src/platform/windows/clipboard.rs:70–108`（编码格式别名顺序
   `PNG → image/png → JFIF → JPEG → image/jpeg → WebP → image/webp`）。至少覆盖下列五组 fixture，各用可区分的 payload
   或选中格式记录断言来源，而不是只断言“产生了图片”：
   每组写明“显式发布的格式”与“预期读取的格式”（Windows 会在 `CF_DIB`、`CF_DIBV5`、`CF_BITMAP` 之间合成格式，
   发布集合不等于可用集合）：
   - 发布合法非空图片文件列表 + PNG + 文本 → 读取 `CF_HDROP` 文件列表；
   - 发布 PNG + DIB + 文本 → 读取原编码 PNG；
   - 发布 DIBV5 + DIB + 文本 → 读取显式 `CF_DIBV5`；
   - 只发布 DIB + 文本（扩展现有只发布 `CF_DIB` 的 G3-01 snipping 场景，追加 Unicode text 并保留文本未被插入的断言）→ 读取合成的 `CF_DIBV5`，并校验图片内容有效；
   - 只发布有效 `CF_BITMAP`（真实 `HBITMAP`）+ 文本 → 读取合成的 `CF_DIBV5`，并校验图片内容有效。
   “图片内容有效”用已知非对称图案检查最终 PNG 的尺寸、像素位置与颜色（防翻转、通道与透明度错误），不要求合成缓冲区与源字节相等。
   现有 G3-01 exact 检查（目前只证明 PNG 优先于文本）保留并在迁移后重跑，其 DIB 场景补实际读取格式断言。
7. 每批更新 `plan_ref`、治理扫描、CI impacted selection、模块指纹输入与覆盖矩阵（第 8 节出口 1–2）。搬目录不能让测试漏选，也不能取消旧的安全检查。
8. 依变更执行 fmt、clippy、targeted tests 与适用的性能/资源基线；按第 8 节出口 4 的硬门与诊断项判定每键分配、全文快照、帧缓冲复制、队列容量与后台重复构建。
9. 新抽象不承诺统一所有系统行为。不支持能力具有明确结果；现有 Windows 成功与失败路径保持合同语义。
10. Linux/macOS 原生壳未实现、未验收时保持 NOT TESTED。共享层完成、Windows 回归、跨平台应用可用与发布 readiness 分别报告。
11. 每批的数据字节兼容与回退重开检查（第 8 节出口 3）通过；`present_damage` 的 `age = 0/1/2` 测试与“隐藏被拒绝”场景通过。

## 10. 代价、风险与备选路径

- 新增两个共享 crate 会增加跨模块接线和少量 public API。收益是应用逻辑与共用 UI 都不再被 Windows 可执行包困住；用最小可见性和明确 DTO 控制 API 面积。
- 最大回归风险在保存/恢复/退出与资产安全，不在目录移动。每批保持请求身份、指纹和错误分类，不能同时简化安全协议。
- 过早为未知平台设计完整接口会形成错误抽象。接口以当前真实调用与已知平台限制为基础；未来实机发现差异时按合同演进，不靠静默默认实现填洞。
- 效果/事件抽象可能引入复制、分配和排队延迟。保留短借用、Arc 快照、有界队列和局部绘制；不引入字符串消息总线或通用插件框架。局部绘制只在 Windows 等保留缓冲的后端降低呈现成本，macOS 的全窗呈现成本是平台事实，单独测量。
- 只提取纯 coordinator 是成本更小的中间路径，但应用回执裁决和 UI 仍绑在 Windows 包中，未完成用户的薄壳目标。
- 把所有代码塞进现有 core 会扩大领域核心依赖并混淆呈现与权威，因此推荐按稳定职责分成共享模块。
- 维持现状会让未来宿主重复接线并可能复制保存、退出、Search 等规则，增加后续行为分叉和验收成本。

## 11. 批准后需要同步的合同

| 合同区域 | 修订内容 |
| --- | --- |
| [01 术语](../plan/01_terminology.md)、[02 范围](../plan/02_positioning_and_scope.md) | 区分“共享实现面向三端”与“已交付 Windows 产品”；未来非 Windows 的便签身份留待专门批准 |
| [03 架构](../plan/03_system_architecture.md)、[04 状态](../plan/04_runtime_state_model.md) | 明确共享库与四层的映射、应用会话入口、能力接口和事件/回执的修改权限；保持单一文档权威 |
| [05 持久化](../plan/05_document_persistence.md)、[08 资产](../plan/08_assets_and_export.md) | 分开安全语义与 Windows 实现，冻结请求/回执/所有权证明边界；本阶段不降低任何 Windows 安全门 |
| [06 渲染](../plan/06_markdown_math_rendering.md)、[07 编辑](../plan/07_editor_and_ime.md)、[09 壳层](../plan/09_windows_shell.md) | 标注共用绘制、会话、输入翻译与原生执行的边界；保留 Windows 行为配置 |
| [10 性能](../plan/10_performance_reliability.md)、[11 验证](../plan/11_testing_and_release.md) | 新共享库的 unsafe/依赖边界、迁移基线、Phase 15 和平台证据分层；不转移旧发布特例 |
| features / acceptance / overview / coverage | 从上述批准合同同步职责位置与验证映射，不由投影反向定义架构 |

## 12. 本轮工作与待确认事项

已执行：工作树与基线核对、合同与投影阅读、CodeGraph 调用链检查、平台耦合及治理路径盘点，形成以上职责划分与迁移方案。
已尝试的分析路径：检验了只抽纯逻辑、全部并入 core、共享应用加共享桌面三个方案，推荐第三个。
未执行：代码迁移、plan 修订、构建/测试、跨平台原型与实机验收；没有提交或推送。

待 USER 批准的具体范围：保留现有 core/render，新增 app/desktop 两个共享模块，按 A–D 提取并接回 Windows；
维持当前 Windows 产品行为与数据格式，另外两端只预留明确能力边界，待在对应平台实现壳时再批准其行为差异。

## 13. 第一轮跨模型审查与处置（2026-10-31）

审查：Codex gpt-6-astra，max effort，只读，分三次（A：现状与边界；B：平台可行性与热路径；C：迁移与 USER 决定），
基于 `259e950`。三次均为 DISAGREE、无 BLOCKER；每条 SHOULD_FIX 已对照源码或锁定版本的依赖源码核实。

| 审查项 | 核实依据 | 本文处置 |
| --- | --- | --- |
| A1 窗口隐藏先提交状态、无失败回执 | `flow/window/lifecycle.rs` `complete_hide`；`app/window_runtime.rs` `SetVisible` | 第 2 节新增行；第 4 节窗口行与第 5 节第 11 条；B 阶段验证 |
| A2 三类 I/O 回执无 generation | `persistence/worker.rs` `IoCompletion` | 第 2 节新增行；第 4 节 worker 行；第 8 节列为 B 的入口依赖（第二轮审查后补） |
| B1 局部更新假定 Win32 单缓冲 | `surface.rs` `present_damage`；softbuffer 0.4.8 Wayland 后端双缓冲 age | 第 2、4 节；第 9 节第 11 条 |
| B2 Linux 未覆盖共享 desktop 的 backend features | `Cargo.toml` winit/softbuffer `default-features = false` | 第 2 节；C 阶段；第 9 节第 4 条 |
| B3 macOS 呈现为全窗成本 | softbuffer 0.4.8 `backends/cg.rs` `buffer_mut`/`age`/`present_with_damage` | 第 5 节第 12 条；第 7 节；第 10 节 |
| C1 治理迁移不能留到 D | `governance.rs` unsafe 禁令与 `flow/window` 扫描的硬编码路径 | 第 8 节出口 1；A/C 阶段 |
| C2 新 crate 测试调度与指纹须随 A/C 建立 | `ci.yml` 任务来源；plan 11 模块指纹规则 | 第 8 节出口 2；A/C 阶段 |
| C3 字节兼容与回退缺可判定条件 | 第 7、8 节原文只有语义测试 | 第 8 节出口 3；第 9 节第 11 条 |

NEEDS_VERIFICATION 项（热路径增量、IME 范围操作、托盘事件循环与可达性、剪贴板 backend 与所有者生命周期）
已写入第 5 节、第 7 节与第 8 节出口 4，需实现或实机后才能判定。

第二轮审查（同日，单次运行）：一轮 8 项 SHOULD_FIX 中 7 项判为已补齐，A2 部分补齐；另提出 3 项 SHOULD_FIX，均已核实并修订：
A2 的实施归属改为 B 的入口依赖并写明完成证据；输入法范围替换/删除与“commit 才编辑”的普通组字流程分开，
补“只有删除、没有提交”的验证场景；第 8 节出口 4 补固定基线与硬门/诊断项判定规则。剪贴板一项补多格式优先级验证。
第三轮审查（同日）：第二轮 3 项判为已补齐；剪贴板优先级在第 9 节第 6 条补具体顺序与三组 fixture，后台重复构建次数归入出口 4 的硬门。
第四轮审查（同日）：后台构建计数的硬门判为已补齐；剪贴板一项补基线实现引用与三组逐项预期选择及来源断言方式。
第五轮审查（同日）：三组预期与来源断言判为已补齐；补 Windows 合成格式：每组区分发布与读取格式，增加只发布 DIB、只发布 BITMAP 两组。
第六轮审查（同日）：AGREE，无 BLOCKER/SHOULD_FIX；两项 NICE（fixture 数量表述、snipping 场景描述）已改；NEEDS_VERIFICATION（合成图片保真）以非对称图案检查写入第 9 节第 6 条，需实测。

## Resolution（2026-10-31）

USER 批准第 12 节所列范围：保留 core/render，新增 `stickymd-app` 与 `stickymd-desktop`，按 A–D 提取并接回 Windows，
维持 Windows 产品行为与数据格式，Linux/macOS 只预留能力边界。第 8 节出口 4 的默认阈值按本文采用。
B 阶段入口依赖为三类 I/O 回执的 Windows 修复（`fix/io-receipt-identity`）。下一步按第 11 节修订 `docs/plan/` 与投影，
并经跨模型审查；本文不替代工程合同。
