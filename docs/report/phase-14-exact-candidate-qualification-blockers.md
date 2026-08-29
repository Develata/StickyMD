# Phase 14 Exact-Candidate Qualification Blockers

## Scope

2026-08-29 的 exact candidate `05d5772b78ce6ca51c070cb6cff354caa9771a11` 已从成功的
GitHub Actions run `33263325541` 下载并提升；候选 EXE SHA-256 为
`4c4f7bc195d845da7e89c55117ca2a90777aab71b839544d3f3ad7323afdbf73`。本报告只记录本轮
资格化暴露的 verification-plane blocker，不改变产品 runtime、Document authority 或依赖图。

## Results

| Lane | Result | Evidence |
| --- | --- | --- |
| Remote CI | PASS | run `33262609457`, attempt 1, exact source SHA |
| Remote Release | PASS | run `33263325541`, attempt 1, artifact id `9718190712` |
| Headless | 17/17 PASS | source-only exact receipt |
| Runtime | 12/12 PASS | exact EXE/ZIP receipt |
| Resources | 14/14 PASS | exact EXE/ZIP receipt |
| G3 | 5/5 PASS | exact EXE/ZIP receipt |
| G4 | 4/6 FAIL | G4-02 and G4-06 failed |
| G5 | 4/4 PASS | exact EXE/ZIP receipt |
| Performance | FAIL | cold p95 882.67 ms; warm p95 1002.13 ms; hard gate 550 ms |

任一 FAIL 都使该 candidate 不可 tag。targeted diagnostic receipt 只用于定位，不能覆盖正式 G4 或
Performance receipt。

## G4-02 Root Cause

物理 drag helper 在释放鼠标后统一要求窗口最终 outer position 与请求坐标相差不超过固定 24 physical
pixels。Dock capture 的请求坐标是 24 DIP 边界内的位置，而产品在 release 后会正确归一化到工作区边缘。
150% DPI 下 24 DIP 等于 36 physical pixels，因此“requested x=36，completed x=0”的正确 Left Dock 被
harness 误报；96 DPI 下两者差值恰为 24，只是偶然掩盖了单位错误。

修复边界：Floating move 仍验证 requested-position；Dock/corner/capture move 验证 application-resolved
terminal，并由调用者继续检查稳定 HWND geometry、dock edge/config 与时间状态机。不得全局删除物理 drag
完成检查。

## G4-06 Root Cause

正式全组的 Microsoft Pinyin 首次 probe 在一次物理 Shift 模式纠正后仍写入 ordinary ASCII；相同 exact EXE
的独立 targeted G4-06 随后通过。当前 helper 的 Shift 路径只发送物理键并立即复探针，没有像普通 profile
reassertion 路径那样重新激活目标 TSF profile、route HWND 并设置及回读 open/native compatibility state。
这使全组第一次 profile normalization 对桌面先前状态敏感。

修复边界：仍只允许一次物理 Shift；随后执行一次有界 profile/route/open/native reassertion，再执行唯一
一次行为复探针。第二次仍为 ASCII 必须 FAIL；不得通过追加 sleep、无限重试或把 ASCII 推断成 composition
来放宽判定。

## Performance Disposition

正式 30 cold + 50 warm run 是有效 FAIL，不按单点 jitter 忽略。该 run 的多个阶段整体抬高，尚不足以把
根因归到产品代码或 Windows 调度。由于 harness 修复会产生新 source freeze 和新 remote artifact，下一
candidate 必须重新生成正式 Performance receipt；在此之前不得修改 550 ms release hard boundary，也不得
继续 startup 优化。

## Required Requalification

1. 先以当前 product candidate 执行 G4-02、G4-06 和完整 G4 的 dirty-harness diagnostics，证明工具修复。
2. targeted tooling tests、strict Clippy 与 fmt 必须通过。
3. 提交后生成新的 clean source freeze；旧 exact receipts 全部失效。
4. 新 SHA 经 USER 授权 push，并从唯一成功 remote artifact 建立新 candidate。
5. 按 Option B 最小 exact 集合重建 artifact-bound receipts；Readiness 只有全部 required lane PASS 才能 READY。

## Architecture Drift

None. Planned changes are confined to `tools/stickymd-smoke` and verification documentation.
