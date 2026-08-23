# Phase 14 Guided Manual Campaign

本指南只记录真实人工观察，不会把未执行项目提升为 PASS。开始前必须已有 exact candidate：

```powershell
.\tools\smoke\phase-14.ps1 -Candidate
```

建议在正常 Windows 11 交互桌面中依次执行三个 session，总目标时间约 15–30 分钟。每一步只接受：

- `P`：实际观察满足 Expected；
- `F`：实际观察不满足，必须在 observation 写明现象；
- `N`：环境不可用或未执行，保持 `NOT TESTED`。

## G1 — Editor / IME / Rendering

```powershell
.\tools\smoke\phase-14.ps1 -GuidedSession G1
```

覆盖 Microsoft Pinyin、WeChat Input Method、zoom/opacity、Preview、math、滚动后的 lazy image、
传统剪贴板和数学分隔符转换。真实输入法或视觉观察不可由自动测试替代。

## G2 — ToolWindow / Tray / Dock / Theme

```powershell
.\tools\smoke\phase-14.ps1 -GuidedSession G2
```

覆盖 taskbar/Alt+Tab、焦点恢复、tray、左/右/上 dock、capture/tie、auto-hide、Pin、紧凑三模式、
主题和 tray exit。

## G3 — Clipboard / Export / Recovery / Asset Safety

```powershell
.\tools\smoke\phase-14.ps1 -GuidedSession G3
```

覆盖真实图片剪贴板、native export、复制目录中的 hard-kill recovery、用户图片和伪 managed 文件
安全。强杀测试不得在仓库根或唯一便签副本上执行。

## 查看结果

```powershell
.\tools\smoke\phase-14.ps1 -ManualStatus
.\tools\smoke\phase-14.ps1 -Readiness -Explain
```

`P12-M01..M44` 仍是人工验收 authority；G1..G3 只是缩短操作路径。Tier B/C 未执行项目继续保持
`NOT TESTED`，任何 waiver 必须由 USER 明确授权并绑定 version、source SHA 与 case/group。
