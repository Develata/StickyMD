# Qualification Execution Model

> Reference projection only. Authority remains `docs/plan/11_testing_and_release.md`.

## Three Evidence Layers

1. **GitHub-hosted deterministic CI/package**：fmt、Clippy、tests、deny、package structure、SBOM、
   static/runtime contract smoke。共享 runner 不执行绝对 550 ms startup 或资源门。
2. **Trusted Windows qualification host**：exact copied EXE 的 Runtime、Performance、Resources、
   environment preflight 与 startup attribution。未来优先 pull-based local lab；不得给 public repo
   绑定高权限、长期在线的 persistent self-hosted runner。
3. **Human interaction acceptance**：真实 IME、视觉、tray/docking、native dialog、物理显示拓扑、
   hard-kill recovery 与 Clean VM。

## Failure Propagation

Environment、identity、P0/security/data safety 与 corrupt evidence 是 global blockers。普通 channel
failure 只改变该 channel receipt；independent channels 继续运行。依赖明确不可满足时可以 SKIP，
但必须记录原因，不能写 PASS。

## Evidence Binding

动态 receipt 至少绑定 source SHA、EXE SHA-256、version、Windows build 与 channel/session。
package/manual 还绑定 ZIP SHA-256。manual observation 只接受显式 human status；automated facts
不能提升人工状态。

## Parallelism and Targeted Reruns

- GitHub-hosted deterministic CI 使用隔离 runner 并发 format/lint、headless tests、headless
  Release performance、portable-core 与 release build；Rust CLI 的 shard-union test 防止漏项。
- 日常修复按受影响 Phase 或 Resource module 定向复核；完整 Campaign 只用于候选冻结、发布资格化
  或明确全量请求。定向资源结果不提升完整 candidate Resources 状态。
- 一个交互桌面上的窗口、焦点、物理鼠标、clipboard、tray 和资源采样是共享能力，必须串行。
  多进程同时采样还会污染 CPU、缓存和 working-set 结论，因此不能用“多开 app”换取虚假的速度。
- Docker 不是 StickyMD v1 的交付对象；没有明确消费者前不建立无用途 image job。

## Manual Guided Sessions

- G1：Editor / IME / Preview rendering。
- G2：ToolWindow / Tray / Dock / presentation shell。
- G3：Clipboard / Export / Recovery / user-asset safety。

Guided step 可同时映射多个 case，仅限这些 case 由同一个观察事实直接支持；receipt 仍逐 case
保存状态和 observation。
