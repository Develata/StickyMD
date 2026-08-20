# Phase 1 Retrospective Verification

本目录只保留仍有独立验证价值、且不会与生产代码形成第二套实现的技术实验。

2026-08-20 重审后，原 Phase 1 spike 被整体移除：

- `text/` 是 774 行单文件、快照式 undo、每次输入全量重建 projection，且真实微软拼音/
  微信输入法均未测试；当前 Source/IME 验证入口已由生产开发壳和
  [`docs/report/phase-03-manual-ime-checklist.md`](../../docs/report/phase-03-manual-ime-checklist.md)
  取代。
- `window/` 的代码路径已经由当前 `stickymd-win` 开发壳覆盖；旧结果只测试 100% DPI，
  没有 150%/200% 或多显示器收据，因此不再保留重复窗口程序。
- 旧 `markdown/` 只有演示输出，没有自动化 contract tests，并包含不必要的全局 allocator。
- 旧 `persistence/` 忽略 recovery mtime，且在任意 `ReplaceFileW` 错误后无条件 fallback。

当前独立实验：

- [`markdown-math/`](markdown-math/)：Comrak Arena → owned projection、四种 delimiter、
  raw HTML literal、RaTeX parse/layout/PNG spike 与可重复基准。
- [`persistence/`](persistence/)：mtime-aware recovery、故障注入、保守原子替换、目录身份、
  named mutex/event。

窗口与 IME 的当前证据直接来自 production dev shell；真实人工输入法矩阵仍是阶段门，
不得因自动化测试通过而写成 PASS。

两个 crate 均声明空 `[workspace]`。删除整个 `experiments/phase-01/` 不影响 production
workspace 构建。
