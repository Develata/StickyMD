# Phase 03 Acceptance Matrix

> Verification projection for Native Source Editor, IME state modeling and the interaction pipeline.
> Synthetic IME tests never substitute for real input-method acceptance.

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P03-A01 | AC-002 source editing pipeline | Automated | render/app tests through [`phase-03.ps1`](../../tools/smoke/phase-03.ps1) | AUTOMATED PASS |
| P03-A02 | AC-009 end-to-end intent/undo projection subset | Automated | render/app coordinator tests | AUTOMATED PASS |
| P03-A03 | synthetic preedit/commit/cancel and commit-as-one-undo | Automated | interaction/session and editor-flow tests | AUTOMATED PASS |
| P03-A04 | 20 KiB/100 KiB/1 MiB input pipeline baseline | Automated | [`phase-03.ps1 -Performance`](../../tools/smoke/phase-03.ps1) | AUTOMATED PASS |
| P03-A05 | copied Release native-shell startup smoke | Automated | [`phase-03.ps1 -Runtime`](../../tools/smoke/phase-03.ps1) | AUTOMATED PASS |
| P03-M01 | AC-003 Microsoft Pinyin matrix | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P03-M02 | AC-004 WeChat Input Method matrix | Manual | [`phase-03 manual IME checklist`](../report/phase-03-manual-ime-checklist.md) | NOT TESTED |
| P03-M03 | candidate positioning at 100/150/200% DPI | Manual | Current-commit visual receipt required | NOT TESTED |
| P03-M04 | selection/composition navigation, refocus and move/resize | Manual | Current-commit interaction receipt required | NOT TESTED |
| P03-M05 | CJK/Latin fallback, caret, selection and preedit visual quality | Manual | Current-commit visual receipt required | NOT TESTED |

## 2026-09-25 clipboard feedback regression

- Preconditions: Source or Split contains a nonempty selection; also exercise copying a Preview selection and a pending error diagnostic.
- Action: copy with Ctrl+C and Ctrl+Insert, then continue reading, scrolling and editing.
- Expected: the clipboard receives the selected text without a success banner covering the source pane; copying does not replace an existing diagnostic. Clipboard failures remain visible and do not mutate the document.
- Failure signals: a persistent `Clipboard updated` overlay, a success notification hiding an error, incorrect copied text, or mutation on a failed copy.
- Automated entry: `cargo test -p stickymd-win --locked`; existing copy/clipboard coordinator tests cover content, generation and failure atomicity. Source/Preview shortcuts converge on the same `ClipboardWritten` shell effect, which now leaves presentation unchanged on success.
- Visual scope: the supplied screenshots reproduce the original obstruction; real desktop verification of the rebuilt application remains `NOT TESTED` until observed. The historical manual rows above are not upgraded by headless tests.
