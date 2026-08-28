# Phase 14 Real IME Functional Automation Design

## Status

USER approved on 2026-08-28. Contract and implementation complete; exact-candidate evidence pending.

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

1. 在独占、解锁、可控的交互桌面启动 isolated exact candidate。
2. 创建 TSF manager，读取并保存当前 `GUID_TFCAT_TIP_KEYBOARD` active profile。
3. 验证目标 profile，使用 session-scoped activation；再次读取 active profile确认一致。
4. 只有 StickyMD HWND 同时 foreground、active、focused 时才允许物理键盘注入。
5. 每次 composition 后从 `note.md` 或 clipboard 读取客观结果；不依赖候选文字像素或固定候选词。
6. 正常路径显式恢复原 profile；unwind/drop 路径 best-effort 恢复并释放 COM。恢复失败必须使 case 失败，
   不能留下 PASS receipt。

测试进程异常终止可能使当前桌面暂时停留在被测 profile，这是 verification-plane 的残余风险；它不修改
profile registration/default list，USER 可通过正常输入法切换恢复。G4 全组必须串行，不能与其它 GUI、
clipboard、dock、tray 或 resource measurement 并发。

## Automated Matrix

每个真实 profile 至少执行：

1. Source preedit 跨过 650 ms autosave boundary 时 durable note 不变。
2. composition 中 Left/Right/Backspace 后可继续输入并 commit。
3. commit 得到非 ASCII 合法 Unicode 文本；一次 Undo 完整恢复 commit 前文本。
4. Escape cancel 不改变 canonical/durable text。
5. selection replacement 的 commit 是一次独立 Undo。
6. Search query/replacement 接受真实 IME commit；Up/Down 导航命中，关闭面板后 clipboard selection
   与 query 相同；Find-only 下 replacement command 仍无效。
7. Source/Split/Docked-expanded、40% opacity 与失焦重聚焦后可继续真实 composition/commit/Undo；
   composition 期间 durable note 与自动收起均受 guard 保护。

候选词具体字面量不是测试 authority，避免用户词频和输入法版本改变导致脆弱断言；测试只接受非空、
合法 Unicode、包含 CJK 且不含未提交拼音字母的 commit，并使用实际 commit 作为后续 Search fixture。

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
  session-scoped 激活、active profile 回读、显式恢复与 Drop best-effort 清理；不修改 profile 注册或默认值。
- `qualification/g4/cases/ime.rs`：每个 profile 使用独立 exact-candidate 副本和物理键盘，覆盖 Source
  preedit/commit/cancel/selection/Undo、中英混输、Search query/replacement、Up/Down、Find-only guard、
  Source/Split、40% opacity、左侧 Docked-expanded 与 refocus。
- G4 parser、PowerShell ValidateSet、六项 receipt/readiness 和治理合同均已更新；旧五项 G4 receipt 不再
  能解除 readiness blocker。
- `cargo test -p stickymd-smoke --locked --no-fail-fast`：94 passed、0 failed（92 unit + 2 CLI）。
- `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings`：PASS。
- `stickymd-smoke all --ci --ci-shard=tests --json`：governance、Markdown/math、persistence、workspace
  tests 与 requested shard 全部 PASS。

未运行：G4-06 exact desktop。原因是当前实现尚未 freeze 为新 exact candidate；P14-A30 与候选窗人工
视觉项继续保持 `NOT TESTED`，不得复用 `ad030008...` 的旧候选收据。
