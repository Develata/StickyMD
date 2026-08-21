# Phase 09 — Pre-Release Convergence Acceptance Matrix

> Status: In Progress. Automated rows remain `BLOCKED` until the named current-commit Phase 9 route passes. Real GUI, IME, visual, physical-display, fault-timing and Clean-VM observations remain `NOT TESTED` without a checked-in receipt. The frozen checklist below is a trace projection of the USER Phase 9 prompt, not a new product contract.

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P09-D001 | Feature freeze遵守。 | Automated | Phase 9 task + inherited/blocker reports | AUTOMATED PASS |
| P09-D002 | Phase0–8 inherited conditions完整汇总。 | Automated | Phase 9 task + inherited/blocker reports | AUTOMATED PASS |
| P09-D003 | 所有 release blockers分类。 | Automated | Phase 9 task + inherited/blocker reports | AUTOMATED PASS |
| P09-D004 | Cold startup完整instrumentation。 | Automated | startup instrumentation, copied-Release measurements and startup report | AUTOMATED PASS |
| P09-D005 | Cold startup ≥20 samples。 | Automated | two copied-Release cohorts, each with 20 cold samples | AUTOMATED PASS |
| P09-D006 | Warm startup ≥20 samples。 | Automated | two copied-Release cohorts, each with 20 warm samples | AUTOMATED PASS |
| P09-D007 | Cold startup p95 ≤300ms，或USER WAIVED。 | Automated | measured FAIL; USER explicitly approved 400 ms fallback and the waiver is recorded in the startup reports | AUTOMATED PASS |
| P09-D008 | Warm startup p95 ≤180ms，或USER WAIVED。 | Automated | startup instrumentation, copied-Release measurements and startup report | BLOCKED |
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
| P09-D044 | user asset safety PASS。 | Automated | asset storage/safe-boundary/export tests; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D045 | managed-looking fake file safety PASS。 | Automated | wrong-hash and full-digest collision tests; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D046 | raw HTML safety PASS。 | Automated | parser/render/copy literal tests; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D047 | remote image zero-network PASS。 | Automated | remote loader non-invocation + dependency/static audit; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D048 | 4K image transient memory评审。 | Automated | owned decode buffer lifetime + five-run copied-Release measurement; `phase-09-reliability.md` | AUTOMATED PASS |
| P09-D049 | Final Source memory测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D050 | Final Preview memory测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D051 | Final Split memory测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D052 | Final Hidden memory测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D053 | Final Idle CPU测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D054 | Final input latency测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D055 | Final Preview latency测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D056 | Final startup测量。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D057 | Leak stress PASS。 | Automated | Phase 9 final performance/resource report | BLOCKED |
| P09-D058 | Cargo dependency freeze。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D059 | cargo deny PASS。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D060 | unresolved high-severity advisory = 0。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D061 | third-party licenses完整。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D062 | proprietary font package scan PASS。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D063 | `SBOM.spdx.json`生成。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D064 | SBOM tool/version固定。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D065 | SBOM checksum。 | Automated | dependency, advisory, license and SBOM supply-chain report | BLOCKED |
| P09-D066 | Portable staging allowlist。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D067 | Portable ZIP生成。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D068 | ZIP不含note/。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D069 | ZIP不含user data。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D070 | ZIP不含proprietary fonts。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D071 | ZIP路径安全。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D072 | SHA256SUMS生成。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D073 | symbols策略经过验证。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D074 | PE x64验证。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D075 | PerMonitorV2验证。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D076 | asInvoker验证。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D077 | icon/version resource验证。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D078 | package在ASCII path运行。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D079 | package在space path运行。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D080 | package在Chinese path运行。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D081 | same-dir single instance package测试。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D082 | different-dir instances package测试。 | Automated | package/verify scripts and copied-package Rust smoke | BLOCKED |
| P09-D083 | README finalization。 | Automated | checked-in release-facing documentation | BLOCKED |
| P09-D084 | README.zh同步或创建。 | Automated | checked-in release-facing documentation | BLOCKED |
| P09-D085 | CHANGELOG更新为Unreleased。 | Automated | checked-in release-facing documentation | BLOCKED |
| P09-D086 | SECURITY.md完善。 | Automated | checked-in release-facing documentation | BLOCKED |
| P09-D087 | CONTRIBUTING.md完善。 | Automated | checked-in release-facing documentation | BLOCKED |
| P09-D088 | release checklist完成。 | Automated | checked-in release-facing documentation | BLOCKED |
| P09-D089 | `.github/workflows/release.yml`完成。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D090 | release workflow actions pin full SHA。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D091 | release workflow最小permissions。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D092 | no pull_request_target release privilege。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D093 | no curl/sh。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D094 | package script是CI/local唯一规则。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D095 | release workflow生成checksums。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D096 | release workflow生成SBOM。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D097 | release workflow配置actions/attest。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D098 | release workflow只创建draft release。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D099 | release workflow不自动stable publish。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D100 | release workflow未在Phase9擅自运行远端。 | Automated | release workflow static audit and local-equivalent validation | BLOCKED |
| P09-D101 | Phase9 smoke完成。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D102 | all.ps1 -Ci PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D103 | fmt PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D104 | clippy PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D105 | workspace tests PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D106 | Release build PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D107 | cargo deny PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D108 | git diff --check PASS。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D109 | core unsafe=0。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D110 | render unsafe=0。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D111 | no WebView。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D112 | no Tauri runtime。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D113 | no Tokio。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D114 | no DB。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D115 | no runtime network。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D116 | no updater。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D117 | no telemetry。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D118 | AC-001..AC-030 final release matrix完成。 | Automated | Phase 9 Rust smoke, CI graph and final baseline commands | BLOCKED |
| P09-D119 | Phase9 task完成。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |
| P09-D120 | Phase9 reports完成。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |
| P09-D121 | working tree clean或明确解释。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |
| P09-D122 | 未push。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |
| P09-D123 | 未tag。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |
| P09-D124 | 未创建GitHub Release。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |
| P09-D125 | 未自动开始任何新产品Phase。 | Automated | Phase 9 task/report and Git-state audit | BLOCKED |

## Final AC-001..AC-030 Matrix

The release-level AC matrix will be populated during Phase 9J from the same local RC artifact. Until then, the existing per-phase automated evidence remains available, while every inherited manual requirement remains open in [the inherited-condition report](../report/phase-09-inherited-conditions.md).

## Manual Receipt Policy

A manual row can become `MANUAL PASS` only with a checked-in current-RC receipt containing environment, artifact hash, steps, expected/actual results and failure evidence. Automated substitutes, prior-commit reports and one-off terminal output cannot advance a manual row.
