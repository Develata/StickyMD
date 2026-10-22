# C2d / C3 coverage follow-up — 2026-10-21

## Background and scope

USER approved continuing the coverage plan on a new branch after PR #4 merged.
Branch: `codex/c2d-c3-coverage`, based on main
`5a8f0cad3447f75102f5925667ff0887d076b6d8`.
The main CI run `39638521070` completed successfully (all nine jobs).

This is an implementation/evidence record, not a new contract. Authority remains
plan 07 `source-find-replace` / `ime-semantics`, plan 09 `dock-auto-hide` /
`tray-lifecycle`, and plan 11 `physical-input-injection` / the qualification
and manual-observation contracts. The C2c/C2d choices approved on 2026-10-19
remain in force (see the 2026-10-10 manual-reuse report).

## Work sequence

1. C2d Search / Esc / IME runtime checks: physical shortcuts, Search parking and
   restoration, replacement selection, composition cancellation and native routing.
2. C2d remaining shell cases: opacity commit/cancel and focus transitions, tray
   save failure, collapsed-instance wake, two-copy isolation, scroll routing,
   and deterministic crash/recovery boundaries.
3. Run the C2c desktop checks together with the extended C2d checks, preserving
   each command, result and tested source/EXE identity.
4. C3 recorder: declared instance sets, second-launch preconditions, read-only
   fixture restoration, monitor arrangements, log/bundle schema and failure paths.

## Verification and evidence boundaries

- First verify the new assertions and receipt-label declarations without a GUI.
  Then use serial desktop runs in a clean checkout with owned child processes.
- USER authorized exclusive desktop use for the next eight hours on 2026-10-21;
  recorded at 17:15 UTC (13:15 America/New_York). Stop physical input by
  2026-10-22 01:15 UTC (2026-10-21 21:15 America/New_York).
- Local copied-Release checks establish source/preflight evidence only. Exact
  candidate receipts still require the approved promoted workflow artifact.
  Existing candidate receipts are not current evidence for this branch.
- Manual observations remain `NOT TESTED` until actually recorded. No cross-model
  review of this follow-up has been performed.
- Preserve the sealed merge evidence under
  `target/review-evidence/merge-2026-10-21/`; write new evidence separately.

## Current state

In Progress. The new branch exists; implementation and validation follow below.
No product behavior or architecture change is proposed by this coverage batch.
The current G4 code covers basic Search IME commit/Undo and Left-dock/40% input,
but does not yet exercise the newly merged Search parking/replacement routes or
the full C2d Right/Top and 96/100% matrix.

## 2026-10-22 first implementation and headless verification

- Added seven G4-03 Search checks and sixteen additional G4-06 IME matrix/check
  labels (147 G3/G4 labels in total). Per-profile obligation owners stay exactly
  aligned with the declared IME labels. The remaining Gap entries are retained
  pending runtime verification and the rest of C2d.
- Added explicit ignored local-preflight tests for G4-03/G4-06. They require a
  clean checkout, an explicitly named EXE, an interactive desktop and no existing
  StickyMD; they retain fixture paths and print the EXE hash, harness commit,
  reached checks and input-cleanup status. They never write candidate evidence.
- `cargo test -p stickymd-smoke --locked qualification::exact_checks`: exit 0,
  four tests passed. Full smoke tests: exit 0, 561 unit tests passed / 19 ignored
  (including the two new opt-in desktop tests), and 26 headless integration tests
  passed. Evidence: `target/review-evidence/c2d-c3-2026-10-22/01-exact-checks.*`
  and `02-smoke-tests.*`. These results do not prove physical input delivery.
- At resumption (2026-10-22 04:59 UTC), the eight-hour desktop authorization had
  expired. No physical input was injected during this implementation batch.
- Product source and dependencies are unchanged. Cross-model review is pending.

### First-batch code checkpoint

`cargo clippy --workspace --all-targets --locked -- -D warnings` passed (exit 0,
`03-clippy.*`), including the final clean-checkout preflight guard and first-frame
startup acknowledgement. The seven Search and sixteen IME checks are implemented
and compiled, but still await physical execution. The implementation does not
close the remaining C2d or C3 tasks.

## 2026-10-22 shell implementation and first physical runs

- USER renewed exclusive desktop access for three hours at 05:26 UTC. The conservative
  stop-input deadline is 08:26 UTC. The authorization record is in the new evidence pack.
- The clean detached checkout at `b40bb0d3947b3761dc0b6b7fcb6d4c88bfcc2a5d` built
  successfully. Its retained local EXE SHA-256 is
  `6699300ccf7176da6efe6fbf97f7bc4fe253b1576fe247c7fe0f497824be1db7`.
  Its base-to-commit diff was independently regenerated and matched
  `a606b53d799a29acd8db5105e9276b1e5f667a0e3f130babbde5d0f674e0abed`.
- G4-01 now exercises tray Hide, physical Alt+F4, checked read-only save failure and
  retry, plus tray/toolbar Pin synchronization using native menu check state, HWND
  state and config. G4-05 adds collapsed junction wake, strict activation, same HWND,
  one process and unchanged note/config bytes and timestamps. Fourteen more declared
  labels bring G3/G4 to 161; physical execution remains required.
- Fixture permissions are restored and read back after success and error, with an
  unwind guard. A real filesystem test also makes restoration fail and verifies that
  a successful operation cannot return success in that case. R7-N1 now has a regression
  assertion for partial tray observations surviving into later failure diagnostics.
- Headless qualification tests: 265 passed / 10 ignored (`07-shell-checks`). Governance
  passed (`10-shell-phase00`). The focused follow-up first caught the deliberately
  stale Phase 08 source digest after the new acceptance description; review confirmed
  that all 62 decomposed rows and their clause hashes were unchanged. After updating
  only that document digest, all 20 focused regressions passed (`15-shell-regressions-reviewed-docs`).
- Physical Search preflight first refused a click because Explorer covered the target
  (`12-search-preflight`); a subsequent inspection found that Explorer HWND already gone,
  so the attempted environment adjustment changed no window. The next run reached the
  Search parking scenario but timed out waiting for the first replacement's durable
  text (`14-search-preflight-clear-blocker`). The note remained original and the view
  returned to Source. Root cause is not yet established; this is not a PASS.
- The next harness checkpoint adds failure screenshots and waits for editor-ready
  startup before legacy editor cases. Assertions and deadlines have not been relaxed.
  Product source remains unchanged. Cross-model review is still pending.
