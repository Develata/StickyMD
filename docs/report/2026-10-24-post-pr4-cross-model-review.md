# PR #4 之后提交的跨模型审查与修复（2026-10-24）

## 范围与方式

- 审查对象：`5a8f0ca..2345ba4`（PR #4 合并之后直接进入 `main` 的 55 个提交，含 v0.1.3 的 exact source
  `1d94349` 及其后的发布记录提交）。这些提交由 GPT 编写并推送，此前没有跨模型审查。
- 方式：Claude 分五个方向只读审查（Preview 排版与绘制；公式与图片资源；应用层；smoke 与资格化工具；
  plan、验收与发布文档），每条发现由主会话对照源码复核后才采纳。审查不构成 exact-artifact 或发布资格证明。
- 结论：没有 BLOCKER。下列 SHOULD_FIX 在分支 `fix/post-pr4-review` 修复；需要 USER 决定的事项单列。

## 已修复（附回归测试；关键修复做了变异检查，原实现或去掉修复时测试失败；本节经五轮 Codex 审查后修订）

| # | 位置 | 问题 | 修复与测试 |
|---|---|---|---|
| 1 | `stickymd-render` `preview/paint.rs` | 公式改为"排版只测量、绘制时才栅格化"后，painter 错误（如字体加载失败）在绘制时以 `?` 让整帧失败，Preview 停在旧帧，违反 plan 06 `math-raster-lifecycle`"单独传播" | 绘制失败的公式在其框上画 math-error 标记，失败原因按该公式原子框的选区范围（`LaidOutMath::selection`，不按几何）写入其 tooltip，其余照常绘制、源码仍可复制；之后重绘会再试；`a_painter_error_marks_its_formula_and_the_frame_still_paints`、`a_painter_error_names_only_the_formula_that_failed`（相邻两公式只有失败者带原因；测试接缝 `fail_draws_for_test`，仅 `cfg(test)`） |
| 2 | `stickymd-win` `app/preview_runtime.rs` | 文档变化调用 `cancel_scrollbar_drag()`，本区间起它同时结束竖向拖动；plan 09 的竖向拖动结束条件只有松开、失焦、切换视图、resize | 文档变化只结束横向拖动（plan 06 重置横向偏移），本身不结束竖向拖动：Source 拖动继续滚动；变化使 Preview 旧帧失效、不显示滑块（`stale-frame-no-thumb`），其拖动在下一次移动时结束且不滚动旧帧；`runtime_an_edit_keeps_a_source_drag_and_a_stale_preview_ends_its_own`（真实编辑推进 generation；先断言编辑前 Preview 帧为当前，去掉 generation 校验时该测试失败；并断言窗口协调器实际收到的 guard，去掉 `DragUpdate::Lost` 的 guard 刷新时该测试失败） |
| 3 | `stickymd-smoke` `runtime/portable_isolation.rs` | WindowShell/Phase 10 runtime 的托盘退出先逐个右键同名图标、点击后才判断归属，且该 runtime 没有独占桌面前置检查；USER 自己的 StickyMD 运行时会被右键 | `exit_owned_copy` 在触碰托盘之前、以及每次右键之前按进程证明归属（`Foreign::CheckBeforeEachClick`；G4 已有独占桌面前置，用 `Excluded`）：映像名为 StickyMD 的进程都必须是本次启动的副本；拥有 `tray-icon` 图标宿主窗口（类名 `tray_icon_app`，图标注册前创建、注册失败也保留；用 `EnumWindows` 枚举，隐藏窗口一并枚举，枚举或任一窗口查询失败即视为无法证明，不使用失败前取得的部分名单）的外来进程，须同时由两种证据排除：其映像版本资源的 ProductName 不是 `StickyMD`（版本资源存在却找不到 ProductName、翻译表缺失或格式异常，都算无法读取而非"没有"），且该进程没有标题以 `StickyMD` 开头的顶层窗口（含隐藏窗口；StickyMD 先以该标题建窗、后建托盘，改名副本保留它；映像路径上的文件在启动后被移走或替换时，ProductName 证据失效，由窗口证据补足）；任一证据为 StickyMD 或读不到即拒绝；没有版本资源或其他 ProductName、且无 StickyMD 窗口的程序（本机实测 Clash Verge）放行；被点击的目标必须拥有该窗口（类名随依赖升级失效时拒绝，而非空集放行）；拒绝信息列出外来进程的路径或 ProductName。图标计数不能证明归属（第三轮反例：一自有图标在关闭的溢出区、改名副本图标可见时计数相等），已删除。残余：非 `tray-icon` 程序把图标命名为 StickyMD、正在退出（窗口已销毁）且映像路径已被替换的改名副本、枚举开始至点击之间新建的宿主窗口或新启动的副本（枚举不是原子快照），其弹出菜单在调用任何菜单项前被 `owned` 拒绝；`window_control::tray_icon_owner_pids`、`window_control::has_stickymd_window`、`managed_process::process_product`、`multiple::exclusivity` 及单元测试（含第三、五轮反例、`kernel32.dll` 的版本资源读取） |
| 4 | `stickymd-smoke` G4 `opacity.rs` / `ime.rs` / `escape.rs` | 四处"不应变化"的断言在注入后立刻读取（或只等 20–100 ms），被测行为稍慢即误记 PASS | 统一等待 `NEGATIVE_SETTLE`（800 ms，长于 dock 动画）后再断言，缩小误判窗口；它不是异步工作的完成上界（config 写入与其他 I/O 共用 worker），单次末端采样也看不到出现后又恢复的错误状态，因此仍需桌面证据（NEEDS_VERIFICATION）；需独占桌面运行，未在本机执行 |
| 5 | `stickymd-render` `preview/table_layout.rs` | 列宽为"测得内宽 + 2·padding"，最终换行宽度由"列宽 − 2·padding"反推，`f32` 下可少一个 ulp，宽表格最宽格的末词被挤到下一行 | 换行宽度取不小于测得内宽（`cell_wrap_width`）；`a_cell_wraps_at_no_less_than_its_measured_line`（扫描中确有反推变小的取值） |
| 6 | `stickymd-render` `preview/paint.rs`、`paint/drawing.rs` | 每个横向块每帧整视口清零并重填遮罩两次（装饰与内容各一次） | `BlockMask` 只清上一块填过的像素；同一矩形重复请求不做事；只有随块滚动的装饰才建遮罩；`a_reused_mask_matches_a_fresh_one_for_each_block` |
| 7 | 测试质量 | `runtime_opacity_key_priority_*` 用 Ctrl+1（默认已是 Source）与 Ctrl+A（只改选区）测不出泄漏；Enter 重复测试的 Search 查询为空；`table_math_pipes` 的横向扫掠可能不执行；`rendering_stress` 不断言见到横向块 | 加 Ctrl+2/3 与选区断言；Search 用有匹配的查询并断言匹配与选区不变；表格在 360 px 下必溢出并断言目标存在；断言扫掠见到横向块 |
| 8 | 文档 | release checklist 写"自动资格化已满足"而 P14-A83 仍 `NOT TESTED`；README 写 0.1.3"尚未发布"；CHANGELOG 0.1.3 仍在 Unreleased；P14-A80 标签数 194（实为 236）；phase-05 任务正文仍称在等 P05-R03/P06-R03 | 逐项更正（phase-05 任务保持 In Progress：本次维护及其人工补验完成，但阶段内其他行如 P05-M01..M06 仍为 `NOT TESTED`）；候选报告以追加勘误方式更正，原文保留 |

## 需要 USER 决定（未改动）

1. **公式内存合同的放宽**：plan 06 与 plan 10 由"严格预算 ≤ 8 MiB"改为"驻留 ≤ 8 MiB + 单公式临时位图 + painter"，数学峰值可超过 8 MiB；进程级 Preview 硬门未变。批准依据只有转述（"按 USER 批准计划"），没有原话或明确指向这一项。
2. **IME 声明的按键一律丢弃**：`app/input.rs` 对 `ime_owned`（`VK_PROCESSKEY`）的按键无条件丢弃，不论是否在组字；注释只描述 WeType 组字结束后的多余 Esc。若输入法在开启但未组字时也声明按键，Esc / F6 / F7 / Ctrl+S 会失效，需真实 profile 确认。该改动在 `test:` 提交中引入。
3. **D3 的改绑**：coverage table 把 `plan09:scroll-4 drag-exit end=release/blur` 由"G4 持有"的 gap 改绑到单元 owner；D3（2026-10-19）写明由 G4 持有，未找到取代它的 USER 决定。
4. **发布例外机制**：plan 11 要求 TAG 前 readiness 为 `READY`；v0.1.1–v0.1.3 连续三版靠合同外的 USER 例外发布（各有逐字记录且限定当版）。若要长期保留，应走 skeleton 级 plan 修改。

## 记录但未修（NICE_TO_HAVE / 需测量）

- `title.rs` 词法守卫只在 `about_to_wait..user_event` 区段计数 `settle_event_pass(`。
- debug 构建逐键记录物理键码到 stderr（release 不受影响），建议改为显式环境变量开启。
- `fixture_readonly::with_readonly` 无路径守卫（现有调用方都在 nonce temp root 下）。
- 公式 `ByteLru` 在 `entry_limit` 路径不检查 `can_evict`（当前 raster 使用 `usize::MAX` 条目，未触发）。
- Glyph 缓存单帧超过 8 MiB 时可能每帧清空；各缓存上限之和未对 52 MiB Preview 硬门做实测。
- 文本裁剪与遮罩裁剪在分数缩放下取整方式不同，可能差 1 px；表格两遍排版约 2× 成本。
- `pipeline.rs` 非测试部分约 527 行、`layout.rs` 约 501 行，超过 500 行审视线。
- 新增的 debug 诊断、`ImageRequests` 尺寸不同的缓存缺失分支缺测试。
- P08-A11 行内"完整 WindowShell 收据待补"与 `AUTOMATED PASS` 并存（phase-08 矩阵受摘要保护，未改）。
- 托盘候选点击（`multiple::open`）在 hover 后重新枚举、等待弹出菜单归属、`owned(..)?` 等错误出口直接返回，未把本次 attempt 记入历史；溢出区展开与光标位置也不恢复。
- 托盘排他性检查的进程名单（Toolhelp）不按会话过滤，另一登录会话中的 StickyMD 也会使检查拒绝（偏保守，不会误点）。
- 公式绘制失败时重建文本索引为 O(原子框数 × 失败数) 并重新排序；持续失败会每帧付出，未实测。

## 验证

见本分支提交前的命令输出：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、
`all --ci --ci-shard=tests`（含 owner 核验）与 Phase 0 governance。G4 修改需独占桌面运行，未在本机执行。

## 后续（2026-10-24，PR #5 第二批）

### USER 决定

- **决定 1（公式内存合同的放宽）已由 USER 确认**，原话："为了数学公式放宽要求那个是我定的，因为首要任务是正常显示，另外8mb也太小了，那个放宽我认为是合理的"。
  plan 06 / plan 10 现行文本保持不变；上文"需要 USER 决定"第 1 项就此关闭。第 2–4 项仍待 USER 决定。

### 第六轮审查与上文"记录但未修"中已处理的项

| 项 | 处理 | 测试 |
|---|---|---|
| 第六轮 NEEDS_VERIFICATION：`has_stickymd_window` 无原生正向测试 | 在测试线程建隐藏的 `STATIC` 窗口（标题 `StickyMD * — 外部修改冲突`），断言找到；无关标题、空标题为否，销毁后为否；已销毁句柄读标题报错而非空串。变异：让匹配恒为否时该测试失败（已恢复）。同进程窗口足以抓住"恒答否"；跨进程读取由第六轮审查对照 API 合同确认，并经本机只读探针观察。该测试与 `the_enumeration_completes` 共用测试专用锁：并行测试中窗口在枚举取得句柄后被销毁，枚举会（按设计）报错（第七轮） | `a_hidden_stickymd_titled_window_is_found_and_others_are_not` |
| 第六轮 NICE：残余说明"正在退出、主窗口已销毁"不准确 | `multiple::exclusivity` 注释改为：产品退出先释放托盘再结束事件循环（`WindowEffect::ExitProcess`），该状态不在当前正常退出路径上，异常生命周期未验证。上文第 3 行的残余说明以本条为准 | — |
| 托盘排他性检查不按会话过滤 | 新增 `managed_process::session`：仅去掉经 `ProcessIdToSessionId` 证明属于其他会话的映像名 StickyMD；会话读不到则保留（仍拒绝）。托盘宿主窗口的枚举本就只在当前桌面 | `only_a_process_proven_in_another_session_is_dropped`、`this_process_is_in_its_own_session`（断言本进程会话查询成功） |
| 托盘候选点击在错误出口丢失 attempt | 单个候选抽为 `multiple::candidate`，返回 `Opened / Next / Reenumerate`；任何错误出口都把本次 attempt（几何、光标读回、是否已发右击）记入历史后再返回 | 仅编译与 clippy；需独占桌面运行，未在本机执行 |
| 公式绘制失败时重建文本索引 | `PreviewTextIndex::set_atomic_tooltips` 就地写入新建、唯一持有的索引：不克隆、不重新排序、不重建命中行；按选区范围二分查找，O(框数 · log 失败数) | `atomic_tooltips_go_to_their_own_box_in_place`，以及原有两项公式失败测试 |

仍未处理：溢出区展开与光标位置不恢复（需新增 helper 命令并在桌面验证）；G4 `NEGATIVE_SETTLE` 仍为 NEEDS_VERIFICATION；上文其余"记录但未修"条目不变。

### USER 决定（2026-10-25）

USER 对上文"需要 USER 决定"第 2–4 项采纳本会话提出的建议，原话："按你的建议推送pr吧"，确认范围："这三项也可以按你的建议来"。

- **决定 2（IME 声明的按键一律丢弃）：保留现行为**，不改 `app/input.rs`。未验证的风险（输入法开启但未组字时也声明按键，使 Esc / F6 / F7 / Ctrl+S 失效）
  在下次人工验收时确认，已加入 [release checklist](../release-checklist.md) 第 4 节；若观察到失效，再改为只在组字结束后的短窗口内丢弃。
- **决定 3（D3 改绑）：接受**。`plan09:scroll-4 drag-exit end=release/blur` 继续由单元测试
  `runtime_drag_ends_on_release_blur_view_switch_and_resize_for_both_panes` 持有。覆盖表的模型每条义务只有一个 owner（`Target` 为单一枚举值），
  不能同时登记两个；真实接线的补充证据是 G4-02 的 `scroll-drag-stops-on-release` / `scroll-drag-stops-on-blur` 与
  `preview-scroll-stops-on-{release,blur}:mode={preview,split}` 标签，它们登记在 `plan09:scroll-4 drag-outside-window` 下，覆盖拖出窗口后的松开与失焦。
  这取代 2026-10-19 D3 中"松开 / 失焦由 G4 持有"的安排；为一条义务登记多个 owner 需要改覆盖模型，本次不做。
- **决定 4（发布例外机制）：以后发布走 plan 11 已有的逐项 waiver**（Tier A case/group、Tier B group，绑定版本与 exact source，计入 `READY`
  并注明"符合批准的发布政策"），不再在 `NOT_READY` 下以合同外的整版例外发布；不改 plan。已加入 release checklist 第 5 节。
  上文第 4 项"若要长期保留，应走 skeleton 级 plan 修改"的表述不完整：plan 11 已有该 waiver 机制，v0.1.1–v0.1.3 未使用它，是因为人工目标声明与逐场景记录未齐，而非缺少机制。

## 后续（2026-10-31，桌面项）

### 修改

| 项 | 处理 | 自动化测试 |
|---|---|---|
| 托盘步骤失败后不恢复溢出区与光标 | helper 在 `tray-targets` 自己展开溢出区时输出 `UIA_TRAY_OVERFLOW_OPENED`（自身失败时先收起再报错）；`multiple::open` 开始前用 `GetCursorPos` 记下光标，并记下本次是否展开了溢出区（`Restore`）。只在失败路径恢复：菜单清理之后，若菜单已观察到关闭，经新 helper 命令 `tray-overflow-close`（UIA `InvokePattern`，非输入注入）收起本次展开的溢出区，再经注入模块把光标放回并读回核对；结果写入该步骤的错误。成功路径行为不变（光标仍停在图标上，溢出区按原样），因为既有 G4 case 以此资格化。不额外调用 helper，不增加成功路径耗时 | 解析 `UIA_TRAY_OVERFLOW_OPENED` 的单元断言 |
| G4 负向检查的 800 ms 单次采样 | `cases::hold`：离散采样，从输入后立即开始，每次采样返回后再隔 50 ms（实际间隔还要加上检查自身耗时，`assert_edge` 约多 20 ms），并在 `NEGATIVE_SETTLE`（800 ms）结束时或之后再采一次，任一次不成立即失败。仍存在于某次采样时的错误状态都会失败，包括原单次末端读取漏掉的"出现后又恢复"；短于采样间隔、落在两次采样之间的瞬态，以及窗口之后才开始的错误仍看不到（异步工作没有完成凭据）。opacity、IME（Ctrl+3）与 escape 两处共四个检查改用它；config 读取遇到原子替换中的短暂失败会重试，不当作"未变" | 假时钟测试 `hold_tests`（检查不耗时）：0–800 ms 采样 17 次、间隔 50 ms；晚出现与跨越采样点的瞬态错误都失败；两次采样之间的瞬态与窗口后的错误不可见（已知限制） |

### 独占桌面预检（本地 EXE，非 exact-candidate 证据）

在临时干净 worktree（`a0b3983` + 本批改动的临时提交；Rust 文件逐字节相同，helper 仅行尾为 CRLF）中构建 release `stickymd-win.exe` 与 smoke 测试，经 `STICKYMD_PREFLIGHT_EXE` 逐项串行运行；预检开始前 USER 关闭了自己的 StickyMD，Clash Verge 保持运行。

| 预检 | 结果 |
|---|---|
| `c2d_tray_failure_restores_local_preflight`（新增：打开菜单后请求不存在的项） | PASS：错误记录 "the menu is observed closed; restore: the overflow flyout this step opened is closed; the cursor is back at (945,334)"，读回光标与步骤前一致；本机图标在溢出区，溢出区路径实际执行 |
| `c2d_dock_local_preflight`（G4-02，含 opacity 与 escape 的持续检查） | 第一次 FAIL：在 opacity 段末尾 Ctrl+S 后等待笔记恢复原文超时，现场笔记为 `:G4 docking integration`（行首多一个半角冒号），该次总耗时 332 s；原因未查明（本批改动不发送按键）。立即重跑 PASS（54 项检查，128 s）。记为未解释的偶发失败，不计为通过证据 |
| `c2d_ime_local_preflight`（G4-06，含 Ctrl+3 的持续检查） | 第一次在首个 source 提交步骤因微软拼音处于英文模式而停止（环境）；USER 切回中文后 PASS（73 项检查，含两种输入法的 `ime-search-composition-blocks-preview`） |
| `c2d_tray_local_preflight`（G4-01） | PASS（19 项检查） |
| `c2d_two_copies_local_preflight`（P08-A11，`exit_owned_copy` 与排他性证明） | PASS；Clash Verge（`tray-icon` 宿主窗口的外来 owner）运行中被正确豁免 |

运行间 E: 盘一度被其他程序占满（与本批无关），磁盘闸拦截构建；USER 释放空间后继续。G4 `NEGATIVE_SETTLE` 从"单次采样"收窄为"窗口内持续观察"，窗口之后才发生的错误仍是已知限制。

第九轮审查 DISAGREE，5 项 SHOULD_FIX 均已核实并修复，修复后的桌面预检见下：
- helper 的溢出区观察改为三态（可见 / 观察到隐藏 / 无法观察），只有观察到隐藏才报告收起，无法观察不再当作已收起；
- helper 自身失败路径有界确认收起，并把结果并入原错误；
- 没有活动弹出菜单所有者时，先由 helper 观察到菜单关闭才收起溢出区；
- "本次展开了溢出区"在候选校验之前读取，候选被拒绝时仍会恢复；
- `hold` 的说明改为离散采样（上表已改；原表述"窗口内出现的错误状态都会失败"夸大）。

另删除 helper 中不可达的 `tray-overflow-state` 分支。首次 G4-02 失败（行首冒号）仍为 NEEDS_VERIFICATION：未找到本批改动导致它的机制，也无法排除时序影响。

第九轮修复后，在新临时 worktree（Rust 文件与本批逐字节相同；helper 只因 `eol=crlf` 检出为 CRLF，归一行尾后相同）上重跑受 helper 改动影响的托盘预检（USER 再次关闭自己的 StickyMD，Clash Verge 运行中）：`c2d_tray_failure_restores_local_preflight` PASS（溢出区路径实际执行，"the overflow flyout this step opened is closed; the cursor is back at (1242,1158)"），`c2d_tray_local_preflight`（G4-01）PASS，`c2d_two_copies_local_preflight` PASS。G4-02 / G4-06 此轮只改注释，未重跑。

第十轮审查（DISAGREE，3 项 SHOULD_FIX，均已核实并修复）：没有活动弹出菜单所有者时，逐个观察全部自有副本的菜单关闭（不只目标），任一未观察到关闭即不收起溢出区，原因写入错误；菜单关闭观察的原始错误不再丢弃（`menus closed=...`）；溢出区状态改为 `Untouched / Opened / Unknown`：helper 调用失败或超时（可能已展开却来不及输出标记）记为未知，未知时只报告、不切换；helper 的 `Open-TrayOverflow` 移入自身清理范围。另把"代码逐字节相同"改为"Rust 文件逐字节相同、helper 仅行尾不同"。

第十一轮审查（DISAGREE，1 项 SHOULD_FIX，已核实并修复）：helper 曾先读一次状态判定"本次是否展开"，展开时又独立再读，首读无法观察、次读已可见时会误报"本次已展开"，失败时 Rust 会收起原本就展开的溢出区。现由 `Open-TrayOverflowObserved` 以同一次观察同时决定是否切换与返回值（观察到隐藏才切换并返回 opened；观察到可见返回 already-open；持续无法观察则短暂重试后报错、不切换），并以加载 helper 源码、脚本化 UIA 读取的 PowerShell 测试覆盖"首读无法观察→次读可见"等五种序列。

第十二轮审查（DISAGREE，1 项 SHOULD_FIX 与 1 项 NEEDS_VERIFICATION，均已处理）：`Close-TrayOverflowObserved` 原来只在轮询前看一次是否可见，首读无法观察时之后即使一直可见也只空等到超时；现改为在期限内重试，首次观察到可见时切换一次、无法观察时从不切换。`Open-TrayOverflowObserved` 在切换前设置"已尝试打开"标记，新函数 `Open-TrayOverflowUntil` 在之后的任何失败（包括切换生效后才抛错）时观察并收起溢出区，再带上结果重新抛错；从未观察到状态时不切换。PowerShell 测试（加载 helper 源码、脚本化读取与切换）覆盖八种序列；两处变异（回到"只在轮询前切换"、标记移到切换之后）均使测试失败，已恢复。

第十三轮审查 AGREE（无 BLOCKER / SHOULD_FIX；NICE：新 PowerShell 测试未覆盖 `Write-TrayTargets` 对目标结果的完整消费；NEEDS_VERIFICATION：真实 Explorer 上 Invoke 延迟或部分生效，以及首次 G4-02 的行首冒号）。审查后在 Rust 文件逐字节相同（helper 仅行尾为 CRLF）的临时 worktree 上重跑托盘预检，USER 关闭自己的 StickyMD、Clash Verge 运行中：`c2d_tray_failure_restores_local_preflight` PASS（这次还经过一次"悬停后候选变化、重新枚举"，溢出区收起、光标回到 (1407,931)），`c2d_tray_local_preflight`（G4-01）PASS，`c2d_two_copies_local_preflight` PASS。
