# Phase 14 G3 Exact-Candidate Automation

## Background

原 G3-01..05 被列为 guided manual，但其判定对象分别是 Windows clipboard format、原生导出结果、
进程强杀后的完整文件、user asset ownership 和 managed-looking fake file。它们主要是可重复的
状态/文件事实，并不依赖视觉审美或真实 IME 判断。持续人工执行既慢，也无法形成稳定回归。

## Approved Boundary

USER 明确批准把 G3 自动化。产品 runtime、Document authority、资产 ownership 与发布包均不改变。
验证面采用：

```text
Rust CLI
  -> exact ZIP/EXE identity
  -> isolated candidate directory per case
  -> physical shortcut / Windows clipboard producer
  -> file, hash, process and recovery assertions
  -> exact JSON receipt

PowerShell UIA adapter
  -> select native export path
  -> invoke tray Exit
```

UIA adapter 不读取或判断 note、资产、hash、Undo、恢复状态，也不生成 PASS。一个交互桌面上的五项
严格串行，避免 clipboard、foreground、tray 与物理输入互相污染。

## Automated Cases

| Case | Exact executable path | Hard assertions |
| --- | --- | --- |
| G3-01 | CF_HDROP、CF_DIB、registered PNG + competing text | image priority；managed publication；Markdown；Undo/Redo active/trash convergence |
| G3-02 | Ctrl+Shift+S + native `IFileSaveDialog` | local-only rewrite；exact asset copy；remote URL unchanged；canonical note unchanged |
| G3-03 | Ctrl+S 后 0/15/75/250 ms process kill | canonical/tmp valid UTF-8；only complete old/new states；restart/recovery usable |
| G3-04 | user image edit/undo/redo/export/tray quit/restart | path and SHA-256 unchanged；never moved to managed trash |
| G3-05 | wrong-hash managed-looking filename + tray quit/restart | unowned file path and SHA-256 unchanged；never moved/deleted |

数据安全、恢复、文件内容、receipt identity 均为 one-strike FAIL，不适用 98% desktop jitter policy。

## Evidence Binding

`g3-exact-qualification.json` 同时绑定 candidate source、harness commit、clean worktree、EXE SHA-256、
ZIP SHA-256 和五个有序 case。readiness 只有在全部字段与当前 candidate 精确一致且所有 case 为
`PASSED` 时，才允许 P12-M28..M30/M32/M33 不再要求人工 receipt。开发期 dirty receipt、旧 ZIP、
缺项、重复/乱序或任意失败均 fail closed。

## CI Boundary

GitHub-hosted Windows runner 不具备可作为证据的独占交互桌面，因此只通过现有 `all --ci` tests
shard 运行 CLI parser、receipt schema/identity 和纯 fixture tests。真正 G3 exact lane 仅在解锁的
可信 Windows qualification host 上显式运行：

```powershell
.\tools\smoke\phase-14.ps1 -G3
```

开发期可用 `-G3Case G3-01..G3-05` 只复测一个模块；CLI 将收据写入独立 case 后缀文件，
readiness 仍只接受包含五项有序 `PASSED` 的默认完整收据。

托盘图标由 Explorer 承载，UIA 无法给出对应 StickyMD PID。Harness 因此在开始前拒绝任何已有
StickyMD 进程，并在 G3-04/G3-05 托盘退出前再次验证目标 PID 是唯一 StickyMD；Explorer 重建
overflow island 导致的 stale rectangle 只做有界重取，绝不猜测或退出其它实例。

## Development Evidence

2026-08-28 在旧 exact candidate `eba0a18e3b34300be99c17c6faea76d9f69696b0` 上完成开发期
端到端验证；加入单案例入口、Explorer 元素有界重取与唯一进程保护后，先单独运行 G3-04，随后
完整运行 G3-01..05，六个执行结果均为 `PASSED`，运行后无 StickyMD 残留进程。完整收据明确记录
`worktree_dirty=true`，因此只证明当前 harness 能安全驱动既有候选，不参与 release readiness。
新 source freeze 后必须生成新 candidate 并重新收集 clean exact receipt。

### 2026-08-30 display-topology follow-up

在双屏扩展切换到单屏后，G3-04 可重复停在 `movable StickyMD tray icon`，而同一 exact candidate 的
`NotifyItemIcon` 已在 Windows 11 `TopLevelWindowForOverflowXamlIsland` 中以可见、启用、40x40 且位于
当前 virtual desktop 的 rectangle 出现。根因位于 adapter：旧 `Find-TrayIcon` 遍历所有顶层 UIA provider
的完整 descendants，并返回第一个同名节点；它既可能选中显示拓扑切换后残留的隐藏/屏幕外节点，也可能
被无关 provider 无限阻塞。纠正边界为：只枚举 `Shell_TrayWnd` 与 overflow island，先快照并验证节点几何，
没有可用目标时确保 overflow 打开后有界重取；Rust 对整个 PowerShell helper 施加硬超时并负责 kill + wait。
产品 tray runtime、菜单协议与候选 EXE 不变。

## Architecture Drift

None. 验证 authority 仍在 std-only Rust CLI，PowerShell 只承担成熟 Windows GUI adapter；产品
runtime dependency delta 为零。
