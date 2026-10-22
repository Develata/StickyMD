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
