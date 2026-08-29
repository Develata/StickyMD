# Phase 14 Real IME Functional Automation Design

## Status

USER approved on 2026-08-28. Contract and implementation repaired on 2026-08-29; clean exact-candidate evidence pending.

## Problem and Evidence Boundary

既有自动化已经证明 `Ime::Preedit` / commit / cancel reducer、Document mutation、Undo 和 SearchSession
不变量，但 synthetic event 不能证明 Windows 真实 TSF 输入法经过 winit 到达 StickyMD。原人工矩阵又把
可客观验证的 commit/Undo/cancel 与主观的候选窗视觉混在同一行，导致大量功能事实只能靠人工重复。

本轮按以下边界重新分类：

- **自动化事实**：真实 profile 被激活；物理按键产生 composition；preedit 不进入 durable note；commit
  产生合法 Unicode canonical text；一次 Undo 撤销完整 commit；cancel 不修改 canonical text；selection
  replacement、composition 内 Left/Right/Backspace、refocus、Source 与 Search 输入链路均可继续工作。
- **人工视觉**：候选窗是否实际显示、与 caret 的距离是否自然、是否遮挡、字体/动画/透明度与不同 DPI
  下的主观观感。自动化读取到的矩形或截图只能作为 companion evidence，不能把视觉项改成 PASS。

## Authority and Architecture

```text
G4 exact-candidate harness
    -> TSF profile guard (test-only Windows adapter)
         capture active keyboard profile
         validate requested profile is installed/enabled
         activate for current input desktop
         route the target HWND with substitute or current same-language HKL
         apply best-effort open/native compatibility preconditions
    -> behavior acknowledgement from physical romanization
         preedit: canonical text remains clean
         direct ASCII: rollback, one physical Shift correction, retry
    -> verified foreground StickyMD HWND
    -> balanced physical key input
    -> durable note / clipboard projection assertions
    -> restore captured profile
```

产品 runtime 不增加测试接口、环境变量、IPC、日志内容或依赖。`DocumentState` 仍是唯一 canonical
authority；测试只观察 exact executable 的既有用户入口和 durable/clipboard projection。TSF COM、
profile GUID、物理键盘与恢复逻辑只存在于 `stickymd-smoke` 的 Windows adapter。

## Profiles

v0.1.0 的一级输入法固定为：

- Microsoft Pinyin：TIP `{81D4E9C9-1D3B-41BC-9E6C-4B40BF79E35E}`，profile
  `{FA550B04-5AD7-411F-A5AC-CA038EC515D7}`。
- WeChat Input Method / WeType：TIP `{86598FB9-66A2-463E-B9C2-AEB906D477AD}`，profile
  `{607FDF85-FCC8-4DBD-A365-41296F980C9C}`。

测试不得安装、启用、注册或修改默认输入法。任一 profile 不存在、未启用、无法激活或无法确认当前
active profile 时，exact case fail closed 为 environment unavailable，不得 fallback 到 `Win+Space` 猜测，
也不得用 Microsoft Pinyin 结果冒充 WeType。

## Lifecycle and Failure Paths

1. 在独占、解锁、可控的交互桌面为每个 profile 复制一份独立 portable candidate；不得让前一 profile
   的 opacity、view mode 或 dock config 成为后一 profile 的 startup state。
2. 创建 TSF manager，读取并保存当前 `GUID_TFCAT_TIP_KEYBOARD` active profile。
3. 验证目标 profile，使用 session-scoped activation；再次读取 active profile确认一致。
4. 只有 StickyMD HWND 同时 foreground、active、focused 时才允许物理键盘注入。
5. 每次 composition 前重新激活并回读同一 profile，再 route 目标 HWND；不能因为窗口已经 focused 就跳过。
6. 每次 composition 后从 `note.md` 或 clipboard 读取客观结果；不依赖候选文字像素或固定候选词。若 profile
   尚未成功 composition 且首次输入落成普通 ASCII，先 cancel + Undo，再允许一次物理 `Shift` 模式纠正；若
   已有成功 composition，ordinary ASCII 只允许重新激活 profile，不允许反向 Shift。第二次仍为 ASCII 立即失败。
7. 若测试执行过模式纠正，必须在 child teardown 和 profile restore 之前对称执行一次物理 `Shift` 恢复。
8. 正常路径显式恢复输入模式与原 profile；unwind/drop 路径 best-effort 恢复并释放 COM。恢复失败必须使 case 失败，
   不能留下 PASS receipt。

测试进程异常终止可能使当前桌面暂时停留在被测 profile，这是 verification-plane 的残余风险；它不修改
profile registration/default list，USER 可通过正常输入法切换恢复。G4 全组必须串行，不能与其它 GUI、
clipboard、dock、tray 或 resource measurement 并发。

## Automated Matrix

每个真实 profile 至少执行：

1. Source preedit 跨过 650 ms autosave boundary 时 durable note 不变。
2. composition 中 Left/Right/Backspace 后可继续输入并 commit。
3. commit 得到非 ASCII 合法 Unicode 文本；一次 Undo 完整恢复 commit 前文本；同一 profile 下再输入
   `rust`，若 TIP 保持 composition，则用 Enter 提交原始罗马字并验证一次 Undo。
4. Escape cancel 不改变 canonical/durable text。
5. selection replacement 的 commit 是一次独立 Undo。
6. Search query/replacement 接受真实 IME commit；Up/Down 导航命中，关闭面板后 clipboard selection
   与 query 相同；Find-only 下 replacement command 仍无效。
7. Source/Split/Docked-expanded、40% opacity 与失焦重聚焦后可继续真实 composition/commit/Undo；
   composition 期间 durable note 与自动收起均受 guard 保护。

候选词具体字面量不是测试 authority，避免用户词频和输入法版本改变导致脆弱断言；测试只接受非空、
合法 Unicode、包含 CJK 且不含未提交拼音字母的 commit。Search fixture 由同一 profile 在 Source 中先做
一次无组合编辑的干净 commit、读取实际 durable term、Undo，再在 Search 中复现同一输入；不得把带
Left/Right/Backspace 编辑的首个候选错误复用于干净 Search composition。

## Diagnostic Resolution

- 2026-08-29 在同一 exact executable 上稳定复现 Microsoft Pinyin ordinary-ASCII failure。TSF 返回的目标
  profile 已通过 active-profile 回读，但该 profile 的 `substitute_layout` 与 `keyboard_layout` 均为 `0`，目标线程
  只能观察到共享的简体中文 HKL `0x8040804`。因此 LANGID 无法确认 Microsoft Pinyin 的“中/英”转换子模式；
  `WM_IME_CONTROL` open/native 回读为成功时，真实物理拼音仍连续两次成为 canonical ASCII，证明该兼容窗口
  不是现代 TSF 子模式的可靠 acknowledgement。跨进程 `ImmGetContext(candidate_hwnd)` 又以
  `ERROR_INVALID_HANDLE` 失败，verification process 不能合法持有 candidate thread 的 HIMC。
- 修复合同改为黑盒行为 acknowledgement：只有“物理拼音未污染 canonical text”才证明 composition 已开始；
  首次 direct ASCII 先事务性回滚，再执行一次所测 Microsoft Pinyin / WeType 的用户等价物理 `Shift` 纠正。
  工具记录是否纠正，并在结束或 unwind 时对称恢复。该方案不新增产品 test API，不跨进程读取 TSF 私有状态，
  不用固定 sleep 冒充状态确认。Windows 官方说明 `WM_INPUTLANGCHANGEREQUEST` 本来就是 posted message，
  `GetKeyboardLayout` 的低字仅是语言 ID；TSF conversion flag 的 authority 是
  `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION`，而不是 smoke 进程自己的兼容 IME window。
  参考：<https://learn.microsoft.com/windows/win32/winmsg/wm-inputlangchangerequest>、
  <https://learn.microsoft.com/windows/win32/winmsg/wm-inputlangchange>、
  <https://learn.microsoft.com/windows/win32/tsf/predefined-compartments>。
- 后续诊断证明 refocus 后会在本会话已多次成功 composition 的情况下短暂落成 ASCII；此时 `Shift` 会把正确模式
  反向切走。根因是旧 harness 仅在 `focus_window()` 实际改变 focus 时 route profile，错误地把“已经 focused”
  当成“TSF profile 仍绑定”。每次 preedit 现改为无条件 session-scoped profile reactivation + active-profile 回读 +
  HWND route；只有尚无任何成功 composition 的初始状态才允许一次物理模式纠正。对于 TSF 返回
  `substitute_layout=0` 的 profile，route 使用目标线程已有的同语言完整 HKL post
  `WM_INPUTLANGCHANGEREQUEST`，迫使已运行窗口重新经过 `WM_INPUTLANGCHANGE`；若线程语言不同且没有
  substitute layout 则继续 fail closed。
- active preedit 的失焦复测还暴露了第二个时序缺口：`GetForegroundWindow()!=candidate` 只证明 native
  foreground owner 已改变，不证明 winit event loop 已消费 `Focused(false)` 并完成
  `set_ime_allowed(false)`。立即请求 foreground back 会合并/越过这一 reducer 边界，随后新拼音成为 ASCII。
  Refocus fixture 先从 docked-expanded 回到 floating，避免 700 ms auto-hide 与 IME focus 形成复合场景；在
  shell foreground acknowledgement 后保留唯一 100 ms target-event-loop dispatch interval，再用 bounded
  native foreground acknowledgement 返回 candidate。该等待没有可用跨进程 reducer projection，且远小于
  dock collapse boundary；不用于普通输入、不得扩大成通用 sleep。

- 2026-08-28 的旧修复曾因 WeType 中英混输删除所有 hot-key fallback；它只覆盖当时自然 composition 的
  桌面状态。2026-08-29 的重复实验进一步证明 Microsoft Pinyin 可以在 profile/open/native 全部回读成功时
  仍处于 direct-ASCII 子模式，因此旧结论被更精确的“行为探针 + 至多一次物理 Shift + 对称恢复”取代。
  WeType 已成功 composition 后不得走该 correction，而是重申 profile；两类恢复仍共享同一有界二次探针。
- 第二次诊断越过混输后，WeType 窗口被前一 Microsoft Pinyin 场景遗留的 40% opacity + left dock config
  恢复到屏幕外，物理 cursor 被夹到桌面边界。每个 profile 改用从未启动的 candidate template 的独立副本，
  消除 verification fixture 的跨 profile 状态泄漏。
- 第三次诊断越过 selection 后，Search 用干净 `zhongguo` 查询，而文档 term 来自带组合编辑的首次提交；
  WeType 两次实际候选不同。Search fixture 改为前述同 profile 干净 commit 捕获，不硬编码候选文字。
- durable note assertion 只在 probe 边界把 CRLF 转为内部 `\n`，不改变文件，也不把孤立 `\r` 归一化。

## Performance and Dependencies

- 新增 runtime dependency：0。
- 新增 product unsafe：0。
- test-only TSF adapter 使用常数大小 COM/profile state；输入和文件轮询均有 bounded timeout。
- 该 case 只在 exact-candidate 本地 Windows 桌面运行，不进入 GitHub-hosted headless CI。

## Verification

- TSF GUID/profile identity 与 CJK commit predicate 的纯单元测试；restore 由真实 G4-06 的 active-profile
  回读与显式恢复结果持有，不能用 fake COM 单元测试冒充桌面事实。
- G4 parser/receipt/readiness 必须包含完整 `G4-06`，targeted 单组不能解除 readiness blocker。
- 本机安装 Microsoft Pinyin 与 WeType 时运行 copied exact/Release executable 的 `G4-06`。
- G1 只保留候选窗、字体、遮挡、动画与 DPI 的主观视觉检查。

## Implementation Evidence

- `window_control/ime_profile.rs`：std-only TSF COM adapter，捕获原 profile、验证目标 installed/enabled、
  session-scoped 激活、active profile 回读，通过 `WM_INPUTLANGCHANGEREQUEST` 把目标 HWND 路由到目标
  substitute layout，并通过候选线程的 default IME window 发送
  `WM_IME_CONTROL` 设置/回读 open status 与 native conversion bit 作为兼容性预置；这些回读不再被当作
  composition acknowledgement。不跨进程持有 HIMC，不修改 profile 注册或默认值。
- `qualification/g4/cases/ime.rs`：每个 profile 使用独立 exact-candidate 副本和物理键盘，覆盖 Source
  preedit/commit/cancel/selection/Undo、中英混输、Search query/replacement、Up/Down、Find-only guard、
  Source/Split、40% opacity、左侧 Docked-expanded 与 refocus；行为探针需要时至多执行一次物理 `Shift`
  模式纠正，并用 RAII 在 child/profile teardown 前对称恢复。
- `window_control.rs`：物理 activation 在点击前要求 `WindowFromPoint` 的 root HWND 就是 candidate，避免
  composition/candidate overlay 或已收起窗口让 smoke 误点用户桌面；refocus 可走 bounded native foreground
  request，再以真实 source click 验证编辑器输入目标。
- G4 parser、PowerShell ValidateSet、六项 receipt/readiness 和治理合同均已更新；旧五项 G4 receipt 不再
  能解除 readiness blocker。
- `cargo test -p stickymd-smoke --locked`：104 passed、0 failed（102 unit + 2 CLI）。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`：PASS。
- `stickymd-smoke all --ci --ci-shard=tests --json`：governance、Markdown/math、persistence、workspace
  tests 与 requested shard 全部 PASS。

诊断验证：旧产品 candidate `f406933d18c1...` + harness commit `80b5c4d...` 的 targeted `G4-06` 已完整
通过 Microsoft Pinyin 与 WeType。该组合只证明 harness/root-cause 修复，不能形成正式候选收据。当前
实现尚未 freeze 为新 exact candidate；P14-A30 与候选窗人工视觉项继续保持 `NOT TESTED`，不得复用任何
旧候选收据。

2026-08-29 回归诊断使用产品 exact executable SHA-256
`5c21d46e3831af1511bbb41b325a01524fd6692861a1918770b2b5a5021ad167` 与 dirty repaired harness 运行 targeted
`G4-06`，Microsoft Pinyin / WeType 完整通过。该结果证明当前修复路径，不是 clean-worktree exact receipt；
提交后仍须生成新 candidate 并重建完整 G4 evidence。

同一 dirty harness 的完整 G4 诊断结果为 G4-02..G4-06 PASS、G4-01 FAIL；G4-01 及其独立重跑均在
Windows UIA 等待 `StickyMD Show` 托盘菜单项时超时。该失败不属于 G4-06 调用路径，不能被本报告的 IME
修复或 targeted PASS 覆盖；它继续作为单独资格化 blocker 等待后续诊断。
