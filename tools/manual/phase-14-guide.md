# Phase 14 Guided Manual Campaign

本指南只记录真实人工观察，不会把未执行项目提升为 PASS。开始前必须已有 exact candidate：

```powershell
.\tools\smoke\phase-14.ps1 -Candidate
```

建议在正常 Windows 11 交互桌面中依次执行两个人工 session。每一步只接受：

- `P`：实际观察满足 Expected；
- `F`：实际观察不满足，必须在 observation 写明现象；
- `N`：环境不可用或未执行，保持 `NOT TESTED`。

## G1 — Editor / IME / Rendering

```powershell
.\tools\smoke\phase-14.ps1 -GuidedSession G1
```

Microsoft Pinyin / WeType 的 composition、commit/cancel、selection replace、Undo 与 Search 功能由
G4-06 自动化；G1 只观察候选窗位置、遮挡、字体、动画、DPI 视觉，以及 zoom/opacity、Preview、math
与滚动后的 lazy image 主观质量。
真实输入法或视觉观察不可由自动测试替代。

## G2 — ToolWindow / mixed-DPI Dock / Compact Window / Theme

```powershell
.\tools\smoke\phase-14.ps1 -GuidedSession G2
```

覆盖 taskbar/Alt+Tab、焦点恢复、mixed-DPI Left/Right sensor、紧凑三模式与主题。主屏三边 dock、
capture/tie、auto-hide、Pin 与 tray lifecycle 由 G4 exact automation 持有。

## G3 — Exact-candidate automated desktop qualification

```powershell
.\tools\smoke\phase-14.ps1 -G3
```

单项调试可使用：

```powershell
.\tools\smoke\phase-14.ps1 -G3 -G3Case G3-05
```

Rust CLI 在独立候选副本中串行覆盖 Windows file-drop、DIB、PNG+text clipboard、native export、
多个时点 hard-kill recovery、用户图片和伪 managed 文件安全。UIA helper 只操作 native dialog/tray，
所有文件、hash、Undo/Redo、恢复与候选 identity 断言均由 Rust 持有。强杀不会作用于仓库根或用户
唯一便签。需要指定候选时使用 `-G3Zip <path>`；动态证据默认写入
`dist/evidence/g3-exact-qualification.json`。
单项运行写入带 case 后缀的诊断收据，不能替代五项完整收据，也不能解除 readiness blocker。
运行前必须从托盘正常退出其它 StickyMD 实例；托盘 UIA 元素没有产品 PID，CLI 因此在已有实例
存在或目标 PID 不是唯一 StickyMD 时 fail closed，绝不替用户退出其它便签。

## G4 — Exact-candidate shell/editor compatibility qualification

```powershell
.\tools\smoke\phase-14.ps1 -G4
```

单组调试可使用 `-G4Case G4-01..G4-06`。六组依次覆盖 tray lifecycle、主屏三边 dock/精确时序、
legacy clipboard shortcuts、真实 toolbar 数学分隔符转换、junction 单实例与 Microsoft Pinyin/WeType
真实 IME 功能矩阵。G4 与 G3 共用 exact
candidate/clean harness/isolated-copy 收据合同，但分别写入 `g4-exact-qualification.json`；单组诊断
收据同样不能解除 readiness blocker。G3/G4 必须依次串行，不能并发争抢 clipboard、tray、窗口
焦点或鼠标。

## 查看结果

```powershell
.\tools\smoke\phase-14.ps1 -ManualStatus
.\tools\smoke\phase-14.ps1 -Readiness -Explain
```

剩余 26 项人工事实由 G1/G2 与 Tier B/C receipt 持有；其中 mixed-DPI M11/M12 不因 G4 主屏
自动化而被替代。G3/G4 对应的 18 项只有各自 exact receipt 满足 source/harness/clean/EXE/ZIP
绑定后才解除 `NOT_TESTED` blocker。Tier B/C 未执行项目继续保持
`NOT TESTED`，任何 waiver 必须由 USER 明确授权并绑定 version、source SHA 与 case/group。
