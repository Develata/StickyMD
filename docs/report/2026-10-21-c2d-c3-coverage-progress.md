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

## 2026-10-22 physical diagnostics and harness identity correction

- `18-shell-clean-compile` returned a shared-target test executable whose inventory
  still had only the two first-batch preflights. `19-search-diagnostic` therefore must
  not be attributed to `b884b74`; its printed current-directory HEAD did not establish
  the binary's compile identity. The retained `harness-b884b74.exe` is rejected for
  that attribution. No PASS or exact receipt was produced by that run.
- Rebuilding in the separate `target/c2d-harness-b884b74` directory produced the four
  expected preflights. The frozen `harness-b884b74-isolated.exe` SHA-256 is
  `156c24b213d03dc19fa936f07bc1b69fb7d54a28cf9e0d08925f0e2e1ab08c70`.
  Future preflight code checks the compiled checkout against the run directory and
  emits its own executable SHA-256. Shared build caches are not used for these harnesses.
- `22-search-diagnostic-isolated` reached the replacement scenario. Its failure image
  showed query `alphaX` and replacement `alphaX` with zero matches. This confirms a
  harness clipboard race: the query paste could read the later replacement payload.
  The repair waits for a positive copy of the lower-case Source match before reusing
  an upper-case query clipboard payload; it does not extend sleeps or weaken assertions.
- `16-ime-preflight` failed in the WeType Search scenario after Esc: the following
  replacement guard changed Source text. The effect of the preceding Ctrl+3 and Esc
  still needs separate observation. Optional per-step screenshots were added to the
  harness for this diagnosis; they are not manual acceptance or additional PASS checks.
- `23-tray-preflight` was refused by the native input guard during tray targeting:
  icon geometry changed, then a `#32768` popup occupied the requested point. The case
  made no lifecycle claim. This is retained for the multi-icon targeting work.
- `25-identity-preflight` passed all ten declared G4-05 checks on the retained local
  EXE, including collapsed wake, activation, unchanged note/config and sole primary.
  Input cleanup succeeded and no StickyMD child remained. This is local source/preflight
  evidence, not a promoted-candidate receipt. C2d/C3 remain in progress.

## 2026-10-22 Search confirmation, IME reproduction and portable isolation

- `27-search-ack-preflight` passed all 15 G4-03 checks, using harness `edb3986`
  (EXE SHA-256 `37a643ac4c5ce17a3ba10876db7dea1c848183f7c9e5a0e489b2e7eb80081f18`)
  and the unchanged retained local product EXE. No exact-candidate receipt was produced.
- `28-ime-stage-diagnostic` reproduced the WeType failure. The three retained images
  show composition before Ctrl+3, composition still present after Ctrl+3, and Search
  closed after one Esc. The following Ctrl+Enter then changes Source. This violates
  plan 09's composition-only Esc row. Event ordering remains to be established;
  debug-only input event metadata (without document/IME strings) and opt-in fixture
  stderr capture are being used to diagnose it. Release input behavior is unchanged.
- The tray adapter now enumerates same-name candidates, rereads their geometry after
  hovering, and accepts only a native popup owner matching the target PID plus that
  PID's UIA menu items. It closes other popups only if their PID belongs to the two
  explicitly owned test copies. Unknown owners are rejected without Esc or invocation.
  Single-instance right-clicks remain capped at two; two-copy enumeration is bounded.
- P08-A11 now drives edits and config changes in both copies, compares peer bytes and
  mtimes, exits one via its actual tray menu, and edits the survivor. Its expanded
  acceptance row is NOT TESTED until execution. The phase wrapper interface is unchanged.
- C2d/C3 are incomplete. 尚未做跨模型审查.

## 2026-10-22 native menu and two-copy results

- `36-two-copies-preflight` passed the expanded P08-A11 isolation operation on the
  retained local Release EXE: two-way note/config edits, peer bytes/mtime, target
  tray exit and survivor edit. This targeted preflight does not replace a full
  `WindowShell` runtime receipt or M45 manual observation.
- `34-tray-owner-preflight` successfully opened the target PID's menu, then failed
  because the installed .NET UI Automation assembly has no `LegacyIAccessiblePattern`
  type. The check-state adapter is replaced by read-only, bounded native popup/menu
  queries with owner checks before and after; no unchecked default is accepted.
- `31-tray-candidates-checks` had 266 passing tests and two source-digest failures.
  The decomposition was unchanged; the mistaken raw-file hash is corrected to the
  normalized-lines hash. `35-reviewed-source-digest` then passed all ten coverage
  checks. The failed run remains retained.
- `37-ime-event-trace` refused an absent EXE before desktop input (the Cargo output
  was named `stickymd-win.exe`). `38-ime-event-trace` used the actual frozen debug
  build, SHA-256 `1204a176d97e20ff37dc4d7e5c06af85dcbfdec59f3fb2e40f4c95df1cc1bda7`.
  Microsoft Pinyin's trace shows Esc key-down before empty preedit, with Search
  still open. That run stopped at another harness acknowledgement race: it read
  the clipboard before the Copy request completed. The assertion now waits for
  the exact expected copied term; WeType event ordering is still unverified.

- Follow-up: `42-tray-governance` rejected the Automated/NOT TESTED row pair. After
  the targeted P08-A11 operation passed, the source matrix uses AUTOMATED PASS with
  that explicitly limited receipt and still says the complete runtime receipt is pending.
- `43-ime-ack-event-trace` refused its first click because the debug window was not
  yet the hit-test target. IME startup now waits for the existing editor-ready event,
  also when stderr capture is enabled; there is no new product test channel.
- `44-tray-native-state-preflight` got past native menu state, then timed out while
  config said Pin ON but the HWND lacked topmost. The adapter now names the exact
  transition in that failure; its root cause is not established.
- The desktop probe now preserves input-injector refusal errors. Previously the
  actual abort stub error was discarded into a generic desktop-unavailable boolean;
  the new error allows a real abort build run to distinguish the two paths.

- Native Pin failure analysis found a harness interference: physical toolbar
  routing temporarily raised a non-topmost window, then restored NOTOPMOST after
  the Pin click had legitimately enabled topmost. The routing-only state is now
  restored before the click; the immediate hit-test still refuses a covered target.
  A fresh physical run is required to confirm this repair.

## 2026-10-22 real WeType ordering and abort build

- `51-ime-ready-event-trace` reached both profiles. In WeType the trace records empty
  preedit, empty commit and IME Disabled at 16637-16638 ms, then Escape key-down at
  16640 ms with composition already false; that event closes Search. Microsoft
  Pinyin delivered Escape before ending composition. A further debug-only boolean
  identifies whether the delayed key is the IME-owned logical Process key; the
  physical scancode alone may not identify who owns the key.
- `52-actual-abort-refusal` ran the real optimized panic=abort smoke CLI via
  `qualification g4 --case=G4-01`, bypassing receipt reuse. It reached the explicit
  injection refusal stub after opening the input desktop and reading its cursor.
  Exit 1 is the expected rejection; it is not G4 qualification failure or a PASS
  receipt. The abort EXE SHA-256 is
  `9f2de3947c730652f6e4b1848431693d3450eeb4955e662f217c6d5e3fedf451`.
  The ERROR object's false/zero environment fields are defaults on that error path,
  not observations that the live desktop disappeared. Full P14-A83 remains open.
- `50-tray-governance-corrected` found the tray opening marker had moved to the new
  child module. Governance now checks the owning files separately, retaining the
  two-attempt constant, menu acknowledgement, native owner and input-call guards.

- `53-tray-pin-route-preflight` passed the previously failing Pin transition and
  reached dock restoration. The next physical target had x=-493 (collapsed HWND).
  Reading the tray menu can lose focus and legitimately collapse the dock; the
  harness now reveals and focuses it again before the final undock drag. This is
  a test-sequencing repair, pending a fresh physical run.

## 2026-10-22 IME key ownership repair

- `57-ime-process-key-trace` reproduced the WeType failure with debug EXE SHA-256
  `94ab0dc0dd545daa72f4026c4c163bc83f6590fac0a954d4e71f80dc22c1cbfa`.
  At 16793-16797 ms composition ended; the 16804 ms physical Escape had logical
  `NamedKey::Process`, and the next event found Search closed. Winit 0.30.13's
  Windows adapter maps `VK_PROCESSKEY` to this logical key independently of its
  physical scancode. The key route now refuses IME-owned presses before any
  physical-key command dispatch. Existing ordinary key-release ownership stays
  intact. This implements plan 07 composition ownership without a new timer,
  platform hook or product contract. Fresh real-IME validation is still required.
- `58-phase00-tray-module-check` caught an injection API name in a Search test
  comment. The comment now describes input insertion without naming the API;
  the governance prohibition itself is unchanged.

- `59-tray-reveal-restoration` passed all 19 declared G4-01 labels on the frozen
  b40bb0d Release EXE, including native Pin, physical Alt+F4 and the read-only Quit
  failure/retry. `60-ime-routing-unit-checks` passed 81 app tests (2 ignored).
  `64-ime-owned-key-fix-preflight` passed all 46 then-declared real Pinyin/WeType
  labels on the fixed debug EXE. The fixed clean Release build (`61`) completed;
  its SHA-256 is `0c9babcac3e6ac2121cbdaf7aeee412fccee079457a9ff97b45fce70b67a70d6`.
- Added the approved D1 filesystem milestones, negative control and recovery Esc
  guard/positive control to G3-03; added docked composition Esc to G4-06. Check labels
  are declared before coverage can be credited. No ownership gap is closed merely
  because this code compiles. `67-crash-and-docked-ime-review` passed clippy; physical
  execution of these new paths is pending. No runtime crash hook was introduced.

- `66-ime-fix-release-preflight` passed the 46 existing real-IME checks on the
  fixed Release bytes. `70-crash-milestones-preflight` passed all 17 G3-03 checks;
  `71-docked-ime-cancel-release` passed the expanded 52 IME checks, including both
  profiles' Esc cancellation on all three dock edges. `69-phase00-and-labels`
  passed governance (the command checks governance; label tests are separate).
- G4-02 now declares physical opacity/Esc/covered-sensor checks. A single owned
  key registration permits bounded repeated key-downs and still releases once;
  short repeat, unwind and release-failure paths are covered by the 22 passing
  injection tests in `73`. The cover is a separate owned native window with no
  StickyMD mouse-message reinforcement. Native FFI signature consistency was
  corrected after `72`; a fresh desktop run of this extension remains required.

- `76-native-controls-preflight` passed the initial physical slider commit, then
  refused the field press because the adapter expected an I-beam. The product's
  `shell_cursor_icon` deliberately uses Pointer for the entire popup; the adapter
  now requires that cursor while retaining the HWND/foreground/focus checks.
- Added the approved D3 fixed-viewport-line observations for Source drag outside,
  release and blur, plus note/selection invariants. Other scrollbar obligations
  remain explicit gaps; this small real-input test does not substitute for them.

### C2d / C3 continuation checkpoint (2026-10-22, logs 81–95)

- The Preview selection probe now waits for a copied rendered frame rather than treating
  the durable view-mode setting as layout readiness. G4-05 now calls the same portable
  isolation scenario as Runtime from a separate fresh copy; this adds one exact check.
- G4-02 remains incomplete at this checkpoint. Logs 82/85 exposed the cover setup;
  87/90/94 refuse when the owned cover cannot be read back as the hit target. A temporary
  always-on-top cover had incorrectly required global cursor polling, which plan 09 does
  not require: the contracted mechanism is temporary sensor topmost over ordinary windows.
  The acceptance projection now states that scope. No failed run is a qualification PASS.
- Log 84 saw a file-read error after explicit Save. That probe now waits for acknowledged
  canonical bytes using the existing bounded note reader instead of reading during publication.
- C3 first checkpoint implements the approved observation log schema 3 / bundle schema 2:
  zero instances for M34, two distinct PIDs for M45, one otherwise, and every EXE digest is
  checked by the same predicate. Existing schema files are not converted or rewritten.
  The recorder owns separate portable directories and rechecks the exact live PID set under
  the commit lock. Partial preparation drops owned processes; startup failure can be recorded
  as FAIL, never PASS. The native Job test now checks that both owned children stop.
- Logs 91 and 95: 96 targeted manual/observation/readiness tests pass. Log 92: smoke CLI
  all-target clippy passes. These are implementation tests; no manual answers or candidate
  receipts have been created. M05 special procedures and M35 layouts are still pending.
- 尚未做跨模型审查. No push, tag or release operation was performed.

### C2d / C3 continuation checkpoint (2026-10-22, logs 96–117)

- The ordinary cover now uses the owning thread's physical DPI context and briefly
  raises its own window before returning to the non-topmost band. Both its actual hit
  target and the sensor's temporary topmost target are checked before native hover.
  Failed cover probes 97/101 and independent native diagnostics 102–104 are retained.
- Log 106 reached scroll blur after the earlier controls passed. Its assertion wrongly
  required the middle viewport to survive focus loss, although focus legitimately reveals
  the existing caret. The corrected probe reads only the fixture-owned thumb column after
  blur and compares it after a held physical move, with no intervening focus/click.
  Log 114 passes all 44 G4-02 labels with clean harness commit `1c10d99` and Release EXE
  SHA-256 `0c9babcac3e6ac2121cbdaf7aeee412fccee079457a9ff97b45fce70b67a70d6`.
- On that same Release, log 108 passes Search's 15 labels and log 110 passes all 11
  identity/portable-isolation labels. Log 109 safely refuses changed tray coordinates;
  a new run after settling (111) passes all 19 tray labels. A successful case alone
  does not establish both normal and overflow tray routes; P14-A83 remains incomplete.
- C3 implements sampled M05 wake evidence, observer continuity/order attestations,
  read-only failed-Quit preparation with restoration, and M35 physical monitor rectangles.
  Procedure facts are checked by the codec and readiness. Missing evidence stays unrecorded;
  genuine product failures can be reported by the observer. No human answers were supplied
  by this implementation work and no manual PASS was created.
- Logs 112/116 pass 101 targeted tests. The added reset regression includes a legacy PASS
  lacking new procedure fields alongside a FAIL: only the approved reset compatibility
  reader accepts its structure, retains the FAIL, and archives the exact original bytes.
  Ordinary readers still reject legacy schema 2. The legacy PASS is not trusted.
- The isolated optimized abort build in 115 is frozen independently of the unwind test
  harness. Log 117 reaches the explicit injection-refusal stub and exits nonzero before
  physical injection. This diagnostic does not create an exact-candidate receipt.
- 尚未做跨模型审查. Local implementation/tests do not change NOT_READY or manual NOT TESTED.
