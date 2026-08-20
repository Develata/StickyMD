# Risk — `ttf-parser` 0.25.1 Is Unmaintained

- `Date`: 2026-08-20
- `Status`: Accepted temporarily for Phase 3 verification; must be rechecked before release
- `Advisory`: `RUSTSEC-2026-0192`
- `Affected path`: `stickymd-render → cosmic-text 0.19.0 → fontdb 0.23.0 → ttf-parser 0.25.1`

## Fact

The current RustSec database marks `ttf-parser` 0.25.1 unmaintained. The advisory reports no safe
upgrade. `cargo deny check` reaches the crate through the approved `cosmic-text` shaping stack; it is
not a direct StickyMD dependency.

This is a maintenance-status advisory, not evidence of a known exploitable vulnerability in the
current StickyMD path. It still matters because font parsing is part of the rendering trust boundary.

## Options Reviewed

1. **Remove or replace cosmic-text now.** Rejected: this would replace an approved architectural
   direction before real IME acceptance and without a verified lower-risk alternative.
2. **Patch/fork ttf-parser.** Rejected: maintaining a font parser would be substantially riskier and
   outside StickyMD's product ontology.
3. **Temporarily acknowledge the exact advisory.** Selected: keep the dependency visible, scope the
   exception to this advisory ID, and require re-evaluation before release and on dependency update.

## Controls

- `deny.toml` ignores only `RUSTSEC-2026-0192`; vulnerabilities and yanked packages remain failures.
- The dependency is transitive and cannot acquire document authority.
- No user-supplied embedded font loading has been added in Phase 3.
- Scheduled/final release dependency review must re-run `cargo deny check` and inspect whether
  cosmic-text/fontdb has migrated away from the unmaintained parser.

## Exit Criteria

Remove the advisory exception when one of the following is verified:

- the approved text stack upgrades to a maintained parser without regressing IME, shaping, memory,
  or license gates; or
- USER approves a documented architecture change after a replacement spike.

If a security vulnerability is published for the locked parser, this temporary acceptance expires
immediately and release work stops for architecture review.
