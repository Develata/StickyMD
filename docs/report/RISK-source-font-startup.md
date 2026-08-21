# RISK — Source Font Initialization and Cold Startup

## Status

Open performance-hardening condition for Phase 9. This is not an architecture stop.

## Observed Result

The Phase 8 copied-Release resource route measured five independent launches from a portable
directory on 2026-08-21:

```text
startup samples         = 1429.905 / 325.825 / 435.860 / 393.911 / 381.413 ms
startup-to-paper median = 393.911 ms
startup-to-paper max    = 1429.905 ms
v1 hard gate            = 300 ms cold-start p95
```

The current sample does not calculate p95 from only five observations, but every observation was
above 300 ms. The first observation followed the Release rebuild and was retained rather than treated
as an outlier. The implementation therefore cannot claim the cold-start gate.

## Environment

- Windows 11 Home Chinese, build 26200.
- Intel Core i7-12700H, 20 logical processors.
- 16,962,281,472 bytes RAM.
- ZHITAI TiPlus7100 NVMe, NTFS.
- Rust/Cargo 1.97.1, Release profile.
- Microsoft Defender real-time and antivirus services reported disabled.

## Current Evidence and Hypothesis

`SourceProjection::new` constructs the process's single source-editor `cosmic_text::FontSystem` before
the hidden window becomes interactive. cosmic-text 0.19 documents that `FontSystem::new()` scans and
parses installed system fonts and can take up to one second in Release. The observed stable memory,
near-zero idle CPU and sub-millisecond window algorithms do not indicate a shell-state or redraw
problem. System-font enumeration is therefore the leading hypothesis, not yet a proven stage profile.

## Rejected Shortcut

Do not replace the system database with a hand-picked FangSong/Times/Consolas-only list merely to
make this number green. That would weaken emoji, rare-script and fallback correctness, and would move
platform policy into `stickymd-render` without evidence that the smaller database covers the v1 font
contract.

A measured experiment also prepared the complete `FontSystem` on a temporary thread and transferred
it to the UI owner. It did not improve the copied-executable time-to-input sample and made the
resource receipt less stable, so the experiment was fully reverted. Thread overlap without changing
the readiness dependency cannot shorten the critical path; retaining that extra lifecycle would only
increase complexity.

## Phase 9 Work

1. Instrument cold startup by stage: bootstrap, window/surface, system font database, source shaping,
   shell/tray initialization and first present.
2. Measure at least five runs before and after each candidate change; use the frozen startup definition
   rather than a synthetic constructor microbenchmark as the release gate.
3. Evaluate a Windows font-database adapter that supplies the approved primary and robust fallback
   families to a platform-neutral render constructor. Preserve full CJK/emoji/special-character
   fallback and keep one `FontSystem` authority.
4. Consider background preparation only if the editor cannot accept input until the resulting font
   system is atomically installed; a cosmetic early paper window is not equivalent to “可输入”.
5. If no correct solution reaches the hard gate, report measured trade-offs to the USER before RC.

## Impact

Phase 8 window state, docking, tray lifecycle, memory and idle CPU can proceed. Phase 9 approval should
carry the condition that cold-start performance remains unresolved and must not be advertised as PASS.
