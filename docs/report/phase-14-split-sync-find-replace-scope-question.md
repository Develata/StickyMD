# Phase 14 Split Sync and Find/Replace Scope Question

## Status

Awaiting USER contract decision. No product implementation has started.

## Request Classification

USER requested two additions while qualifying the exact candidate:

1. synchronized Source/Preview scrolling in Split;
2. find and replace within the current Source document.

Both directly serve the single-note editing/reading ontology, are same-domain extensions, and do not
require a new runtime, database, network capability, parser, or document authority. They pass the
engineering constitution's ontology and heterogeneous-intrusion tests. They are nevertheless product
scope changes: the approved Split contract explicitly says that both panes retain independent scroll
positions and do not synchronize, while Source find/replace is absent from the frozen v1 scope.

## Recommended Split Synchronization

Do not bind raw scrollbar percentages. Markdown projection height differs from Source height because
headings, wrapping, tables, formulas and images expand by different amounts; ratio synchronization will
drift precisely on the documents where alignment is most useful.

Use a generation-bound semantic anchor instead:

```text
active pane scroll gesture
-> top-visible source byte + intra-block fraction
-> sorted source-anchor index
-> binary search nearest current-generation anchor in the other pane
-> update the other pane once per redraw
```

- Preview already projects source ranges; Source can expose line-start/source-byte anchors without
  changing canonical text ownership.
- The pane receiving the current wheel/drag gesture is the one-way owner. A sync token suppresses the
  reciprocal scroll callback and therefore prevents feedback loops.
- Missing source ranges (image, formula, raw block) fall back to the nearest preceding/next stable block
  plus an intra-block fraction.
- Stale Preview generations do not synchronize. The last independent positions remain intact until a
  current generation is available.
- Building an anchor table is O(blocks) during existing layout. Each scroll mapping is O(log blocks),
  O(1) auxiliary work, coalesced to at most one target update per redraw.

Recommended interaction: a compact Split-only sync toggle. Keep it disabled by default for v0.1.0 so
the existing independent-inspection behavior remains available; enabling it aligns both panes. If USER
wants sync enabled by default, that choice must be stated in the plan/config contract.

## Recommended Source Find/Replace

Scope the capability to the one canonical `note.md`; do not add global search, file search or regex in
the first implementation.

```text
Ctrl+F / Ctrl+H shell overlay
-> generation-tagged literal query over DocumentState read projection
-> UTF-8 byte match ranges
-> Replace / Replace All typed intent
-> DocumentState mutation gateway
```

- Query, active match and options are Editor Session state, not durable document authority.
- Any canonical generation change invalidates/recomputes match ranges before mutation; stale byte ranges
  are never applied.
- Next/Previous may use Rust's standard substring search and a sorted match list. Navigation is O(log k)
  after an O(n) scan.
- Replace Current submits one normal `EditRequest` and one Undo entry.
- Replace All validates non-overlapping ranges, then copies unchanged slices and replacement text in one
  forward pass. It is one canonical mutation and one Undo entry, O(n + output bytes), instead of repeated
  `String::replace_range` with O(n * matches) movement.
- Initial controls should be literal query, case-sensitive toggle, wrap-around Next/Previous, Replace and
  Replace All. Regex would add semantics, dependency/failure surface and resource limits without being
  necessary for a scratchpad.

## Boundary and Failure Review

| Concern | Required rule |
| --- | --- |
| Canonical authority | `DocumentState` remains the only mutable text authority |
| Preview authority | source-range anchors are immutable projection only |
| IME | opening/searching must not commit or corrupt preedit; replace is disabled during composition |
| Undo | Replace Current and Replace All use ordinary typed document transactions |
| Stale work | both search matches and sync anchors are generation-bound |
| Memory | no duplicate document authority; match ranges and anchor tables are bounded by current document |
| Dependencies | no runtime dependency is required |
| Verification | Rust CLI/unit coverage plus Phase acceptance matrix; visual/IME behavior remains manual |

## Required USER Decisions

1. Split sync default: **off (recommended)** or on.
2. Find/replace v1 scope: **literal + case toggle (recommended)** or regex as well.
3. Whether both additions are authorized before v0.1.0 candidate freeze, accepting that they require a
   new implementation, full automated qualification, and fresh manual acceptance.

Until these decisions are approved, no authoritative plan, feature projection, acceptance matrix or
runtime code will be changed for either capability.
