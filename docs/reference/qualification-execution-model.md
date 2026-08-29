# Qualification Execution Model

> Reference projection only. Authority remains `docs/plan/11_testing_and_release.md`.

## Three Evidence Layers

1. **GitHub-hosted deterministic CI/package**：fmt、Clippy、tests、deny、package structure、SBOM、
   static/runtime contract smoke。共享 runner 不执行绝对 550 ms startup 或资源门。
2. **Trusted Windows qualification host**：exact copied EXE 的 Runtime、Performance、Resources、
   environment preflight、startup attribution 与串行 G3/G4/G5 exact desktop automation。未来
   优先 pull-based local lab；不得给 public repo 绑定高权限、长期在线的 persistent self-hosted runner。
3. **Human interaction acceptance**：真实 IME、视觉、tray/docking 观感、物理显示拓扑与 Clean VM。

## Failure Propagation

Environment、identity、P0/security/data safety 与 corrupt evidence 是 global blockers。普通 channel
failure 只改变该 channel receipt；independent channels 继续运行。依赖明确不可满足时可以 SKIP，
但必须记录原因，不能写 PASS。

## Evidence Binding

Source Freeze 先绑定 clean source SHA、version、Cargo.lock 与 harness。GitHub `release` workflow 的
ZIP/SHA256SUMS/SBOM 下载并验证后，才晋升为 canonical `dist/exact-candidate/`；最终 candidate 还绑定
run/attempt/artifact id 与实际 EXE/ZIP/SBOM SHA-256。动态 artifact-bound receipt 至少绑定 source SHA、
EXE SHA-256、version、Windows build 与 channel/session，package/manual 还绑定 ZIP SHA-256。manual
observation 只接受显式 human status；automated facts 不能提升人工状态。

本地 `target/release` 只用于 source preflight，不与远端独立 build 比较逐字节等价。Promote 后所有
artifact-bound receipt 必须针对 staged candidate 重建；Source Freeze 未变时 fmt/Clippy/headless/
dependency 等 source-only evidence 可复用。这就是最小 exact 重验，不是完整 Phase 0–14 Campaign。

## Parallelism and Targeted Reruns

- GitHub-hosted deterministic CI 使用隔离 runner 并发 format/lint、headless tests、headless
  Release performance、portable-core 与 release build；Rust CLI 的 shard-union test 防止漏项。
- 日常修复按受影响 Phase 或 Resource module 定向复核；完整 Campaign 只用于候选冻结、发布资格化
  或明确全量请求。定向资源结果不提升完整 candidate Resources 状态。
- 一个交互桌面上的窗口、焦点、物理鼠标、clipboard、tray 和资源采样是共享能力，必须串行。
  多进程同时采样还会污染 CPU、缓存和 working-set 结论，因此不能用“多开 app”换取虚假的速度。
- Docker 不是 StickyMD v1 的交付对象；没有明确消费者前不建立无用途 image job。

## Guided and exact desktop sessions

- G1：Editor / IME / Preview rendering。
- G2：ToolWindow / Tray / Dock / presentation shell。
- G3：本地串行 exact-candidate automation，覆盖 Clipboard / Export / Recovery / user-asset safety；
  UIA 仅为 Windows shell 薄适配，判定与收据由 Rust CLI 持有；`-G3Case` 可单独诊断一个
  case，但其独立收据不参与 release readiness。
- G4：本地串行 exact-candidate automation，覆盖 Tray lifecycle / 主屏 Dock timing / legacy
  shortcuts / math conversion / junction identity；`-G4Case` 只用于单组诊断，独立收据不参与
  release readiness。mixed-DPI Left/Right sensor 仍由 G2 人工观察。
- G5：本地串行 exact-candidate automation，覆盖 ToolWindow shell eligibility、compact 三视图、
  zoom/opacity/theme mechanics 与 rendering stress；`-G5Case` 只用于单组诊断。逐窗口 PNG 的相对
  path/SHA-256 写入 receipt，readiness 会重新读取并校验；截图只提供 visual companion evidence，
  不替代真实 IME、mixed-DPI 或首次人工视觉判断。

G1/G2 guided step 可同时映射多个 case，仅限这些 case 由同一个观察事实直接支持；manual receipt
仍逐 case 保存状态和 observation。G3/G4/G5 不写人工状态，分别写 exact automated receipt，并按
G3 → G4 → G5 串行执行。
