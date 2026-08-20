# Phase 0–3 Architecture Convergence Audit

- `Date`: 2026-08-20
- `Scope`: Phase 0 governance through Phase 3 native Source/IME slice
- `Authority`: dated audit evidence; `docs/plan/` remains governing
- `Starting commit`: `3094dbb1f48febf5e2906c03d74f9a1c797c1240`

## Executive Result

The retained implementation now has a clean architectural backbone for its implemented scope:

- `DocumentState` is the only mutable runtime text authority.
- Interaction, typed intent, coordination, execution adapters, and projection are distinct.
- Core/render remain platform-independent and forbid unsafe code.
- The ordinary key path carries a generation-tagged `TextDelta`; it does not clone the complete
  document before and after every key.
- Source projection rebuilds only the affected logical lines, including line splits/merges and
  terminal empty lines, and fails closed to an explicit snapshot resync on desynchronization.
- Production responsibilities that crossed the repository's cohesion threshold were separated.

Phase 0 and Phase 2 automated contracts pass. The retained parts of Phase 1 and the automated part
of Phase 3 pass with the qualifications recorded below. Phase 3 is **not complete** until Microsoft
Pinyin and WeChat Input Method are exercised on Windows using the manual checklist.

## Requirement-to-Evidence Ledger

| Phase | Required contract | Current evidence | Result |
| --- | --- | --- | --- |
| 0 | plan authority, projections, acceptance mapping, governance | constitution/plan tree, coverage matrix, governance revalidation | PASS |
| 1 | minimal workspace and bounded risk spikes | clean core/render boundary; rebuilt Markdown/math and persistence spikes; dependency/API/performance reports | CONDITIONAL |
| 2 | canonical document, UTF-8 deltas, monotonic generation, bounded undo, snapshots | 35 core tests plus Release benchmark | PASS |
| 3 | typed native editor flow, projection-only shaping, IME state contract, input latency | 35 render/app ordinary tests, one Release pipeline benchmark, manual checklist | AUTOMATED PASS / MANUAL OPEN |

Phase 1 remains conditional because real IME, full DPI/window opacity behavior, and the production
RaTeX painter path are not established by its portable automated spikes. Those gaps are evidence
limits, not permission to substitute another architecture.

## Correctness Defects Removed

1. **Hot-path duplicate authority/copy pressure.** The shell formerly requested owned document
   snapshots around ordinary edits. Effects now carry `generation + TextDelta`; UI reads use a
   short-lived borrowed view and snapshots occur only at worker/resync boundaries.
2. **Quadratic script segmentation.** Neutral characters previously scanned the remaining text for
   every character. Font-run segmentation now performs one initial script lookup and one linear
   pass; a 64 KiB neutral fixture guards the case.
3. **Unjustified IME duplicate suppression.** A same-content ordinary keyboard event after commit
   could be legitimate input. The heuristic was removed after checking winit's Windows event path;
   commit remains the sole composition mutation and later ordinary input is preserved.
4. **Generation-zero persistence receipt.** The first successful save of a newly created empty note
   now establishes `base_disk_hash` even though both generations are zero.
5. **Boundary safety.** Navigation clamps an unexpected byte inside a UTF-8 code point instead of
   slicing and panicking; cursor arithmetic and framebuffer sizing fail closed on overflow.
6. **IME visual position.** Candidate placement follows the cursor inside preedit text, including a
   selected preedit range, without making preedit canonical.
7. **Line-structure projection.** Newline insertion/deletion and a terminal synthetic empty line
   now splice only affected cosmic-text lines while preserving later scroll identity.
8. **Evidence drift.** The stale direct `arrayref` constraint and yanked-package claim were removed
   after a refreshed registry index and isolated fresh-lock verification disproved them.
9. **Missing dependency policy.** `deny.toml` now limits the graph to the v1 Windows target, permits
   only reviewed licenses/sources, rejects wildcard and forbidden framework dependencies, and keeps
   the exact transitive `ttf-parser` maintenance advisory visible through a documented exception.

## Algorithm and Performance Review

| Area | Algorithm / complexity | Decision |
| --- | --- | --- |
| canonical text | `String`, O(n) worst-case middle move | retain: measured common 1 MiB core edit p95 12.1 µs; abstraction permits later replacement |
| ordinary projection | validate generation/content, reshape affected logical lines | retain: 1 MiB ordinary command worst p95 1.567 ms; newline p95 1.534 ms |
| line offsets | adjust suffix starts after single-line byte-length change, O(lines after edit) | retain: start/middle 1 MiB measurements remain far below the 50 ms gate |
| line splits/merges | rebuild changed block and splice line objects, O(changed text + following line offsets) | added: avoids O(document) Enter path without a rope/tree abstraction |
| fallback resync | explicit O(n) snapshot + full reshape | retain only for recovery; 1 MiB p95 53.227 ms is recorded, not hidden |
| script runs | initial explicit-script discovery + one pass, O(n) | corrected from O(n²) neutral lookahead |
| mouse hit test | cosmic-text hit plus grapheme search on the hit logical line | corrected from document-wide grapheme scan |
| undo/redo | bounded deques, deterministic 256-entry / 4 MiB combined budget | retain; no unbounded history or generic transaction framework |

A rope, Fenwick tree, piece table, async runtime, or generic event bus would add material complexity
without current evidence of user-visible benefit. The present measured paths justify keeping the
smaller design. The full-resync number should be revisited if recovery becomes frequent or future
Preview integration exposes a comparable whole-document hot path.

## Cohesion and Coupling Review

- `app/input.rs` translates window events; `app.rs` owns lifecycle/presentation.
- `flow/editor.rs` owns ordering and failure atomicity; it does not paint or call Win32.
- `source/projection.rs`, `source/geometry.rs`, and `source/rendering.rs` own projection mutation,
  geometry/hit testing, and painting separately.
- Platform clipboard and framebuffer details stay under the Windows app adapter boundary.
- No production file crosses the approximately 500 handwritten-line hard review threshold.
- No WebView, Tauri, Tokio, network client, database, plugin mechanism, or generalized framework was
  introduced.

## Verification Evidence

- Workspace ordinary tests: 73 passed, 0 failed, 1 Release-only benchmark ignored.
- Release Source pipeline: 1 passed; ordinary 1 MiB command p95 at most 1.567 ms, newline p95
  1.534 ms, and exceptional full resync p95 53.227 ms.
- Fixed-seed randomized Source test keeps text, generation, selection boundaries, and projection
  synchronized after every operation.
- Unicode coverage includes CJK, combining marks, emoji and ZWJ grapheme clusters.

Final fmt, clippy, release build, Phase 1 spike tests, dependency/unsafe searches, link checks, and
diff checks are recorded in the completion response and repository status for this audit.

## Remaining Gates

The following must not be reported as complete:

- Microsoft Pinyin and WeChat Input Method composition/candidate behavior at 100/150/200% DPI.
- Real Windows clipboard, focus, move/resize, and transparency interaction smoke tests.
- Later phases: production persistence/autosave/conflict, Preview/RaTeX painter, assets/export,
  tray/docking/multi-monitor lifecycle.
- Re-evaluate `RUSTSEC-2026-0192` before release or any text-stack upgrade; the scoped exception is
  governed by `RISK-ttf-parser-unmaintained.md`.

Use `phase-03-manual-ime-checklist.md` for the immediate blocking verification. RichEdit remains an
approved contingency only after the documented pure-Rust repair attempts and USER review.

## Recommendation

`STOP — complete the Phase 3 manual IME gate before Phase 4.`

No architectural rewrite is currently justified for the implemented slice. Continue long-term work
through the existing contracts, keeping exceptional O(n) paths measured and preventing projections
or platform adapters from acquiring business authority.
