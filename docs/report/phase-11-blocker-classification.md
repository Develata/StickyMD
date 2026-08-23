# Phase 11 Blocker Classification

## Scope

本报告按 Phase 11 的三类模型重新审计当前候选。它只分类已经有证据的事实；真实输入法、
物理显示器和人眼视觉项目没有 receipt 时仍为 `NOT TESTED`，不由自动化替代。

## Class A — Non-relaxable

| Area | Current evidence | Result |
| --- | --- | --- |
| canonical text / persistence authority | workspace regression、OCC、recovery 与 failure-injection tests | no known defect |
| managed/user-file safety | managed-name、digest、safe-root、hard-link 与 GC regression | no known defect |
| IME transaction semantics | deterministic composition/Undo tests | automated contract PASS; real IME remains Class C |
| forbidden architecture | dependency/governance scans | no WebView、Tokio、database、runtime network |
| late caret/image defects | off-screen caret 与 bottom-viewport image regressions | fixed and automated |

已知 Class A P0 defect：**0**。这不是对未执行人工路径的 PASS 声明。

## Class B — USER-relaxable Engineering Gates

| Gate | Current evidence | Status |
| --- | --- | --- |
| cold editor-ready p95 <=400 ms | 30 samples, 300.692 ms | PASS under USER-approved gate |
| warm editor-ready p95 <=180 ms | 50 samples, 311.353 ms | FAIL; USER decision required |
| Preview <=100/400/2000 ms | current Release baselines | PASS |
| zoom relayout p95 <=50 ms | 50/100/300% = 2.479/2.365/2.347 ms | PASS |
| memory / idle CPU | full five-process resource matrix | PASS; max idle CPU 0.005% |

Startup 不是 architecture invariant。当前实现不以第二渲染器、持久字体数据库、后台服务或
平行 authority 换取数字；cold 已通过，warm 在 USER 明确批准新门前继续阻断 RC。

## Class C — Environment-dependent Acceptance

以下项目仍无当前候选的人工 receipt：Microsoft Pinyin、WeChat Input Method、taskbar、
Alt+Tab、tray、Top/Left/Right dock、24 DIP capture、nearest-edge、真实 zoom/compact/opacity、
传统剪贴板 producer、原生导出对话框、真实强杀恢复、双显示器、混合 DPI、拔插显示器，以及
Phase 11-B 的真实按钮/Undo/Pin hover timing。

状态统一为 `NOT TESTED`；详见 `docs/report/phase-11-manual-acceptance.md` 与逐 Phase matrix。

## Severity Summary

| Severity | Count | Disposition |
| --- | ---: | --- |
| P0 known correctness/security defects | 0 | none known after automated regression |
| P1 release blockers | 2 classes | warm startup gate FAIL; release-critical manual matrix NOT TESTED |
| P2 follow-up observations | 0 | no new frozen-v1 product issue classified |

P1 未归零，且已明确阻断 release。不得 tag、push 或创建 GitHub Release。
