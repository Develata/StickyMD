# Phase 09 — Pre-Release Convergence Acceptance Matrix

> Historical source baseline. Individual rows preserve Phase 9 evidence and then-open blockers;
> they are not the current `v0.1.0` release verdict. Real GUI, IME, visual, physical-display,
> fault-timing and user-asset observations remain `NOT TESTED` here unless Phase 9 itself recorded
> them. The frozen checklist below is a trace projection of the USER Phase 9 prompt, not a new
> product contract. Current release identity and dispositions are recorded in
> [`../release-notes/0.1.0.md`](../release-notes/0.1.0.md).

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P09-D001 | Feature freeze遵守。 | Automated | Phase 9 task + inherited/blocker reports | AUTOMATED PASS |
| P09-D002 | Phase0–8 inherited conditions完整汇总。 | Automated | Phase 9 task + inherited/blocker reports | AUTOMATED PASS |
| P09-D003 | 所有 release blockers分类。 | Automated | Phase 9 task + inherited/blocker reports | AUTOMATED PASS |
| P09-D004 | Cold startup完整instrumentation。 | Automated | startup instrumentation, copied-Release measurements and startup report | AUTOMATED PASS |
| P09-D005 | Cold startup ≥20 samples。 | Automated | two copied-Release cohorts, each with 20 cold samples | AUTOMATED PASS |
| P09-D006 | Warm startup ≥20 samples。 | Automated | two copied-Release cohorts, each with 20 warm samples | AUTOMATED PASS |
| P09-D007 | Cold startup p95 ≤300ms，或USER WAIVED。 | Automated | final 20-sample copied-Release p95 277.205 ms; original gate PASS, fallback unused | AUTOMATED PASS |
| P09-D008 | Warm startup p95 ≤180ms，或USER WAIVED。 | Automated | final 20-sample copied-Release p95 342.891 ms; no USER waiver exists | BLOCKED |
| P09-D009 | FontSystem瓶颈被实测。 | Automated | two milestone cohorts; see startup hardening report | AUTOMATED PASS |
| P09-D010 | Startup优化没有牺牲CJK/Emoji fallback。 | Automated | unchanged full system font database plus source Unicode regression suite | AUTOMATED PASS |
| P09-D011 | 没有bundle proprietary fonts。 | Automated | dependency/package source scan; no font asset was added | AUTOMATED PASS |
| P09-D012 | Microsoft Pinyin真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D013 | WeChat IME真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D014 | Preview视觉测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D015 | Math视觉测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D016 | Image视觉测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D017 | Light视觉测试。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D018 | Dark视觉测试。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D019 | System theme真实切换。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D020 | Opacity真实测试。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D021 | Tray真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D022 | Left Dock真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D023 | Right Dock真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D024 | Top Dock真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D025 | Hover no-focus真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D026 | 125% DPI真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D027 | 150% DPI真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D028 | 200% DPI真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D029 | dual monitor真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D030 | mixed DPI真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D031 | monitor disconnect真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D032 | sleep/resume真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D033 | RDP真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D034 | Explorer PNG clipboard真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D035 | Explorer JPEG clipboard真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D036 | Snipping Tool真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D037 | browser image clipboard真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D038 | native Export dialog真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D039 | hard-kill recovery真实测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D040 | real junction/symlink测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D041 | Clean Windows 11 VM测试或NOT TESTED。 | Manual | current-commit Phase 9 manual acceptance receipt | NOT TESTED |
| P09-D042 | Atomic save failure matrix PASS。 | Automated | `phase-09-reliability.md`; workspace tests | AUTOMATED PASS |
| P09-D043 | OCC external-race PASS。 | Automated | `phase-09-reliability.md`; guarded create/replace race tests | AUTOMATED PASS |
| P09-D044 | user asset safety PASS。 | Manual | full restart/edit/undo/redo/GC/export/quit receipt required | NOT TESTED |
| P09-D045 | managed-looking fake file safety PASS。 | Manual | full destructive-boundary receipt required | NOT TESTED |
| P09-D046 | raw HTML safety PASS。 | Automated | parser/render/copy literal tests; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D047 | remote image zero-network PASS。 | Automated | remote loader non-invocation + dependency/static audit; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D048 | 4K image transient memory评审。 | Automated | owned decode lifetime + final five-run copied-Release measurement; `phase-09-performance-final.md` | AUTOMATED PASS |
| P09-D049 | Final Source memory测量。 | Automated | final five-run copied-Release resource report | AUTOMATED PASS |
| P09-D050 | Final Preview memory测量。 | Automated | final five-run copied-Release resource report | AUTOMATED PASS |
| P09-D051 | Final Split memory测量。 | Automated | final five-run copied-Release resource report | AUTOMATED PASS |
| P09-D052 | Final Hidden memory测量。 | Automated | final five-run copied-Release resource report | AUTOMATED PASS |
| P09-D053 | Final Idle CPU测量。 | Automated | five independent 60-second samples per mode; Source/Preview/Split/Hidden p95 0.002604/0.001302/0.005208/0.002604% | AUTOMATED PASS |
| P09-D054 | Final input latency测量。 | Automated | current-code Release source pipeline baseline | AUTOMATED PASS |
| P09-D055 | Final Preview latency测量。 | Automated | current-code Release native Preview baseline | AUTOMATED PASS |
| P09-D056 | Final startup测量。 | Automated | final 20 cold + 20 warm copied-Release cohorts | AUTOMATED PASS |
| P09-D057 | Leak stress PASS。 | Automated | 1000 window cycles plus 100 autosave/reload, 100 conflicts and 100 image-decode cycles; private bytes +0.527 MiB, no linear growth | AUTOMATED PASS |
| P09-D058 | Cargo dependency freeze。 | Automated | Cargo.lock hash + supply-chain report | AUTOMATED PASS |
| P09-D059 | cargo deny PASS。 | Automated | final `cargo deny check` | AUTOMATED PASS |
| P09-D060 | unresolved high-severity advisory = 0。 | Automated | deny audit + explicit unmaintained-only risk report | AUTOMATED PASS |
| P09-D061 | third-party licenses完整。 | Automated | exact package includes generated license texts for all 187 locked Windows runtime registry packages | AUTOMATED PASS |
| P09-D062 | proprietary font package scan PASS。 | Automated | package allowlist and source audit | AUTOMATED PASS |
| P09-D063 | `SBOM.spdx.json`生成。 | Automated | exact local RC SPDX 2.3 report | AUTOMATED PASS |
| P09-D064 | SBOM tool/version固定。 | Automated | Syft 1.50.0 archive/checksum pins | AUTOMATED PASS |
| P09-D065 | SBOM checksum。 | Automated | exact local RC SHA256SUMS verification | AUTOMATED PASS |
| P09-D066 | Portable staging allowlist。 | Automated | package/verify scripts and exact local RC | AUTOMATED PASS |
| P09-D067 | Portable ZIP生成。 | Automated | exact local RC ZIP | AUTOMATED PASS |
| P09-D068 | ZIP不含note/。 | Automated | package verifier allowlist | AUTOMATED PASS |
| P09-D069 | ZIP不含user data。 | Automated | package verifier allowlist | AUTOMATED PASS |
| P09-D070 | ZIP不含proprietary fonts。 | Automated | package verifier allowlist | AUTOMATED PASS |
| P09-D071 | ZIP路径安全。 | Automated | traversal/absolute/duplicate/out-of-allowlist rejection | AUTOMATED PASS |
| P09-D072 | SHA256SUMS生成。 | Automated | exact local RC checksums | AUTOMATED PASS |
| P09-D073 | symbols策略经过验证。 | Automated | stripped single-build EXE; no unsupported symbols claim | AUTOMATED PASS |
| P09-D074 | PE x64验证。 | Automated | package verifier PE32+/x64 check | AUTOMATED PASS |
| P09-D075 | PerMonitorV2验证。 | Automated | package verifier manifest check | AUTOMATED PASS |
| P09-D076 | asInvoker验证。 | Automated | package verifier manifest check | AUTOMATED PASS |
| P09-D077 | icon/version resource验证。 | Automated | package verifier resource check | AUTOMATED PASS |
| P09-D078 | package在ASCII path运行。 | Automated | exact local RC copied-runtime smoke | AUTOMATED PASS |
| P09-D079 | package在space path运行。 | Automated | exact local RC copied-runtime smoke | AUTOMATED PASS |
| P09-D080 | package在Chinese path运行。 | Automated | exact local RC copied-runtime smoke | AUTOMATED PASS |
| P09-D081 | same-dir single instance package测试。 | Automated | second exits, first remains, durable files unchanged | AUTOMATED PASS |
| P09-D082 | different-dir instances package测试。 | Automated | independent copied-runtime directories | AUTOMATED PASS |
| P09-D083 | README finalization。 | Automated | checked-in release-facing documentation | AUTOMATED PASS |
| P09-D084 | README.zh同步或创建。 | Automated | checked-in Chinese README | AUTOMATED PASS |
| P09-D085 | CHANGELOG更新为Unreleased。 | Automated | checked-in changelog | AUTOMATED PASS |
| P09-D086 | SECURITY.md完善。 | Automated | checked-in security policy | AUTOMATED PASS |
| P09-D087 | CONTRIBUTING.md完善。 | Automated | checked-in contribution policy | AUTOMATED PASS |
| P09-D088 | release checklist完成。 | Automated | checked-in release checklist | AUTOMATED PASS |
| P09-D089 | candidate 与 promotion release workflows 完成。 | Automated | actionlint + local static audit | AUTOMATED PASS |
| P09-D090 | release workflows actions pin full SHA。 | Automated | governance static audit | AUTOMATED PASS |
| P09-D091 | release workflows 按 operation 使用最小 permissions。 | Automated | governance static audit | AUTOMATED PASS |
| P09-D092 | no pull_request_target release privilege。 | Automated | governance static audit | AUTOMATED PASS |
| P09-D093 | no curl/sh。 | Automated | governance static audit | AUTOMATED PASS |
| P09-D094 | package script是CI/local唯一规则。 | Automated | workflow invokes checked-in scripts | AUTOMATED PASS |
| P09-D095 | release workflow生成checksums。 | Automated | local exact-RC pipeline + workflow audit | AUTOMATED PASS |
| P09-D096 | release workflow生成SBOM。 | Automated | local exact-RC pipeline + workflow audit | AUTOMATED PASS |
| P09-D097 | draft promotion operation 配置 actions/attest 且复用 exact artifact。 | Automated | pinned action and exact run/hash audit | AUTOMATED PASS |
| P09-D098 | candidate workflow 不创建 release；draft operation 只创建/更新 draft。 | Automated | workflow static audit | AUTOMATED PASS |
| P09-D099 | publish 仅由独立显式 operation 执行，不由 candidate/tag/draft 自动触发。 | Automated | workflow static audit | AUTOMATED PASS |
| P09-D100 | release workflow未在Phase9擅自运行远端。 | Automated | no push/tag/release; remote execution NOT EXECUTED | AUTOMATED PASS |
| P09-D101 | Phase9 smoke完成。 | Automated | all code tests pass; final status gate rejects P09-D008 warm-start BLOCKED | BLOCKED |
| P09-D102 | all.ps1 -Ci PASS。 | Automated | all 13 CI tasks run; final ready-status gate rejects P09-D008 | BLOCKED |
| P09-D103 | fmt PASS。 | Automated | final `cargo fmt --all -- --check` | AUTOMATED PASS |
| P09-D104 | clippy PASS。 | Automated | final workspace/all-targets `-D warnings` | AUTOMATED PASS |
| P09-D105 | workspace tests PASS。 | Automated | latest full run: 357 passed, 0 failed, 12 Release performance tests explicitly exercised separately | AUTOMATED PASS |
| P09-D106 | Release build PASS。 | Automated | final locked workspace Release build | AUTOMATED PASS |
| P09-D107 | cargo deny PASS。 | Automated | advisories/licenses/bans/sources PASS | AUTOMATED PASS |
| P09-D108 | git diff --check PASS。 | Automated | final Phase 9 evidence diff validated with zero whitespace errors | AUTOMATED PASS |
| P09-D109 | core unsafe=0。 | Automated | forbid declaration + unsafe scan | AUTOMATED PASS |
| P09-D110 | render unsafe=0。 | Automated | forbid declaration + unsafe scan | AUTOMATED PASS |
| P09-D111 | no WebView。 | Automated | Cargo.lock/source forbidden-architecture scan | AUTOMATED PASS |
| P09-D112 | no Tauri runtime。 | Automated | Cargo.lock/source forbidden-architecture scan | AUTOMATED PASS |
| P09-D113 | no Tokio。 | Automated | Cargo.lock/source forbidden-architecture scan | AUTOMATED PASS |
| P09-D114 | no DB。 | Automated | Cargo.lock/source forbidden-architecture scan | AUTOMATED PASS |
| P09-D115 | no runtime network。 | Automated | dependencies + remote-loader non-invocation | AUTOMATED PASS |
| P09-D116 | no updater。 | Automated | source/workflow scan | AUTOMATED PASS |
| P09-D117 | no telemetry。 | Automated | source/dependency scan | AUTOMATED PASS |
| P09-D118 | AC-001..AC-030 final release matrix完成。 | Automated | final table below and release-readiness report | AUTOMATED PASS |
| P09-D119 | Phase9 task完成。 | Automated | task is implementation-complete, release validation incomplete | AUTOMATED PASS |
| P09-D120 | Phase9 reports完成。 | Automated | startup/performance/reliability/supply-chain/package/readiness reports | AUTOMATED PASS |
| P09-D121 | working tree clean或明确解释。 | Automated | exact RC source was clean; evidence-only report commit follows | AUTOMATED PASS |
| P09-D122 | 未push。 | Automated | local Git audit | AUTOMATED PASS |
| P09-D123 | 未tag。 | Automated | local Git audit | AUTOMATED PASS |
| P09-D124 | 未创建GitHub Release。 | Automated | local/remote workflow audit | AUTOMATED PASS |
| P09-D125 | 未自动开始任何新产品Phase。 | Automated | task and diff scope audit | AUTOMATED PASS |

## Final AC-001..AC-030 Matrix

`AUTOMATED PASS` means the complete acceptance contract is covered by deterministic tests or
copied-executable automation. `NOT TESTED` means a required human/physical-OS observation is still
missing even when lower-level reducers or adapters pass.

| AC | Name | Automated evidence | Release status |
| --- | --- | --- | --- |
| AC-001 | Portable First Launch | exact-RC ASCII/space/Chinese copied launch, bootstrap and no-fallback tests | AUTOMATED PASS |
| AC-002 | Source Editing | source pipeline latency and command tests pass; real interaction matrix absent | NOT TESTED |
| AC-003 | Microsoft Pinyin | IME state/reducer tests only | NOT TESTED |
| AC-004 | WeChat IME | IME state/reducer tests only | NOT TESTED |
| AC-005 | Autosave | virtual-time debounce, worker/OCC and hide/focus barriers | AUTOMATED PASS |
| AC-006 | Manual Save | typed intent, immediate scheduling, success/failure authority tests | AUTOMATED PASS |
| AC-007 | External Clean Reload | watcher/OCC/reconciliation integration tests | AUTOMATED PASS |
| AC-008 | External Dirty Conflict | conflict, load-external and keep-local state tests | AUTOMATED PASS |
| AC-009 | Undo Redo | bounded Unicode/property and asset transaction tests | AUTOMATED PASS |
| AC-010 | Image Paste | transaction tests pass; real clipboard producers absent | NOT TESTED |
| AC-011 | Managed Image Undo | paste/undo/redo/restart reconciliation tests | AUTOMATED PASS |
| AC-012 | User Image Safety | lower-level ownership tests pass; required full user-file chain absent | NOT TESTED |
| AC-013 | Markdown Preview | semantic/layout/selection tests pass; visual interaction absent | NOT TESTED |
| AC-014 | Math Delimiters | Comrak delimiter + RaTeX layout corpus | AUTOMATED PASS |
| AC-015 | Math Error | failure isolation tests pass; visual error affordance absent | NOT TESTED |
| AC-016 | Raw HTML Safety | literal AST/render/copy tests and no DOM/runtime | AUTOMATED PASS |
| AC-017 | Remote Image No Network | loader non-invocation, URL preservation and dependency audit | AUTOMATED PASS |
| AC-018 | Export | deterministic export tests pass; native dialog/Explorer flow absent | NOT TESTED |
| AC-019 | Left Dock | geometry/reducer/runtime automation exists; real interaction absent | NOT TESTED |
| AC-020 | Right Dock | geometry/reducer/runtime automation exists; real interaction absent | NOT TESTED |
| AC-021 | Top Dock | geometry/reducer/runtime automation exists; real interaction absent | NOT TESTED |
| AC-022 | Input Focus Guard | reducer tests pass; real IME+dock interaction absent | NOT TESTED |
| AC-023 | Tray Lifecycle | lifecycle/runtime automation passes; real tray menu observation absent | NOT TESTED |
| AC-024 | Opacity | reducer/Win32 adapter tests pass; whole-window visual observation absent | NOT TESTED |
| AC-025 | Theme | config/reducer tests pass; live Windows theme visual observation absent | NOT TESTED |
| AC-026 | Same Directory Single Instance | exact-RC second-process wake/exit and durable-file check | AUTOMATED PASS |
| AC-027 | Different Directory Multi Instance | exact-RC independent-directory process/file check | AUTOMATED PASS |
| AC-028 | Monitor Disconnect | synthetic topology properties pass; physical disconnect absent | NOT TESTED |
| AC-029 | Mixed DPI | geometry/cache-key tests pass; physical mixed-DPI/IME observation absent | NOT TESTED |
| AC-030 | Crash Recovery | recovery/failure injection passes; real forced-process timing absent | NOT TESTED |

Release-level totals: 12 `AUTOMATED PASS`, 18 `NOT TESTED`, 0 `MANUAL PASS`, 0 `USER WAIVED`,
0 observed failures. Warm startup is a separate automated release-gate failure.

## Manual Receipt Policy

A manual row can become `MANUAL PASS` only with a checked-in current-RC receipt containing environment, artifact hash, steps, expected/actual results and failure evidence. Automated substitutes, prior-commit reports and one-off terminal output cannot advance a manual row.
