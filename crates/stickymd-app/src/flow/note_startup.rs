//! What a startup must do about `note.md` and `note.md.tmp`, decided from what the host observed.
//! The host observes and executes inside its own safe operations (plan 03 rule 5); this only
//! decides, so no path or authority crosses into shared code.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence

use std::path::Path;

use stickymd_core::{
    DocumentState, ExternalFileFact, ExternalFileState, LineEnding, PreserveExpectation,
    RecoveryCandidate, RecoveryDisposition, RecoveryInspection, inspect_recovery,
};

#[derive(Debug, Clone)]
pub struct NoteObservation {
    pub state: ExternalFileState,
    pub recovery: RecoveryInspection,
}

impl NoteObservation {
    /// Recovery decoding is meaningful only when the file bytes were actually loaded. An
    /// over-limit file deliberately has no synthetic empty body.
    pub fn recovery_inspection(&self) -> Option<&RecoveryInspection> {
        (!matches!(self.state, ExternalFileState::TooLarge { .. })).then_some(&self.recovery)
    }
}

/// Why the canonical note cannot be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteRefusal {
    InvalidUtf8,
    TooLarge(u64),
    Missing,
}

/// Where the document comes from once `note.md.tmp` needs nothing more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteDocumentPlan {
    Load(ExternalFileFact),
    /// There is no note: the host creates an empty one, without replacing anything that appeared.
    CreateEmpty,
    Refuse(NoteRefusal),
}

/// An over-limit `note.md.tmp` is moved aside before anything else. Without a canonical note the
/// startup stops once it is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OversizedTemporary {
    pub bytes: u64,
    pub canonical_exists: bool,
}

#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "one value per startup, consumed at once"
)]
pub enum NoteStartupStep {
    /// The two files differ: nothing is written, and the user chooses.
    Recover {
        candidate: RecoveryCandidate,
        /// The unusable canonical observation a restore must preserve first (plan 05 recovery 5).
        canonical_to_preserve: Option<PreserveExpectation>,
        document: DocumentState,
    },
    /// `note.md.tmp` repeats the canonical note. An `Err` means the canonical note cannot be
    /// opened, and the temporary is then left where it is.
    RemoveRedundantTemporary(Result<ExternalFileFact, NoteRefusal>),
    /// `note.md.tmp` is not valid UTF-8: the host moves it aside, then follows the plan.
    QuarantineInvalidTemporary(NoteDocumentPlan),
    Open(NoteDocumentPlan),
}

#[derive(Debug)]
pub struct NoteStartupPlan {
    pub oversized_temporary: Option<OversizedTemporary>,
    pub step: NoteStartupStep,
}

/// `new_note` is the line ending of a note this startup creates, which is the host's convention.
pub fn plan_note_startup(
    canonical: Option<&NoteObservation>,
    temporary: Option<&NoteObservation>,
    new_note: LineEnding,
) -> NoteStartupPlan {
    let recovery = inspect_recovery(
        canonical.and_then(NoteObservation::recovery_inspection),
        temporary.and_then(NoteObservation::recovery_inspection),
        new_note,
    );
    let oversized_temporary = temporary.and_then(|observation| match observation.state {
        ExternalFileState::TooLarge { bytes } => Some(OversizedTemporary {
            bytes,
            canonical_exists: canonical.is_some(),
        }),
        _ => None,
    });
    let step = match recovery {
        RecoveryDisposition::Candidate(candidate) => NoteStartupStep::Recover {
            canonical_to_preserve: canonical_to_preserve(canonical),
            document: recovery_document(&candidate, new_note),
            candidate,
        },
        RecoveryDisposition::RedundantTemporary => NoteStartupStep::RemoveRedundantTemporary(
            canonical.ok_or(NoteRefusal::Missing).and_then(present),
        ),
        RecoveryDisposition::InvalidTemporary => {
            NoteStartupStep::QuarantineInvalidTemporary(document_plan(canonical))
        }
        RecoveryDisposition::NoTemporary => NoteStartupStep::Open(document_plan(canonical)),
    };
    NoteStartupPlan {
        oversized_temporary,
        step,
    }
}

/// A too-large canonical is not read, so it is never moved automatically; the user moves it aside
/// (USER decision 2026-10-31).
fn canonical_to_preserve(canonical: Option<&NoteObservation>) -> Option<PreserveExpectation> {
    match canonical.map(|observation| &observation.state) {
        Some(ExternalFileState::InvalidUtf8 { fingerprint }) => {
            Some(PreserveExpectation::Content(*fingerprint))
        }
        Some(ExternalFileState::TooLarge { .. }) => Some(PreserveExpectation::Unverifiable),
        _ => None,
    }
}

fn recovery_document(candidate: &RecoveryCandidate, new_note: LineEnding) -> DocumentState {
    candidate.canonical.as_ref().map_or_else(
        || DocumentState::empty(new_note),
        |loaded| DocumentState::loaded(&loaded.text, loaded.line_ending, Some(loaded.fingerprint)),
    )
}

fn document_plan(canonical: Option<&NoteObservation>) -> NoteDocumentPlan {
    match canonical.map(present) {
        Some(Ok(fact)) => NoteDocumentPlan::Load(fact),
        Some(Err(refusal)) => NoteDocumentPlan::Refuse(refusal),
        None => NoteDocumentPlan::CreateEmpty,
    }
}

fn present(observation: &NoteObservation) -> Result<ExternalFileFact, NoteRefusal> {
    match &observation.state {
        ExternalFileState::Present(fact) => Ok(fact.clone()),
        ExternalFileState::InvalidUtf8 { .. } => Err(NoteRefusal::InvalidUtf8),
        ExternalFileState::TooLarge { bytes } => Err(NoteRefusal::TooLarge(*bytes)),
        ExternalFileState::Missing => Err(NoteRefusal::Missing),
    }
}

pub fn document_from_fact(fact: ExternalFileFact) -> DocumentState {
    DocumentState::loaded(&fact.text, fact.line_ending, Some(fact.fingerprint))
}

pub fn oversized_temporary_warning(preserved: &Path) -> String {
    format!(
        "note.md.tmp 超过安全载入上限；已隔离保留为 {}。",
        preserved.display()
    )
}

pub fn invalid_temporary_warning(preserved: &Path) -> String {
    format!(
        "note.md.tmp 不是有效 UTF-8；已隔离保留为 {}。",
        preserved.display()
    )
}

#[cfg(test)]
mod tests {
    use stickymd_core::{DiskFingerprint, LoadedDocument, hash_bytes};

    use super::*;

    fn fingerprint(bytes: &[u8]) -> DiskFingerprint {
        hash_bytes(bytes)
    }

    fn valid(text: &str) -> NoteObservation {
        let loaded = LoadedDocument::from_durable_bytes(text.as_bytes(), LineEnding::Crlf).unwrap();
        NoteObservation {
            state: ExternalFileState::Present(ExternalFileFact {
                fingerprint: loaded.fingerprint,
                text: loaded.text.clone(),
                line_ending: loaded.line_ending,
                durable_len: text.len(),
            }),
            recovery: RecoveryInspection {
                bytes: text.as_bytes().to_vec(),
                modified: None,
            },
        }
    }

    fn invalid(bytes: &[u8]) -> NoteObservation {
        NoteObservation {
            state: ExternalFileState::InvalidUtf8 {
                fingerprint: fingerprint(bytes),
            },
            recovery: RecoveryInspection {
                bytes: bytes.to_vec(),
                modified: None,
            },
        }
    }

    fn too_large(bytes: u64) -> NoteObservation {
        NoteObservation {
            state: ExternalFileState::TooLarge { bytes },
            recovery: RecoveryInspection {
                bytes: Vec::new(),
                modified: None,
            },
        }
    }

    fn plan(
        canonical: Option<&NoteObservation>,
        temporary: Option<&NoteObservation>,
    ) -> NoteStartupPlan {
        plan_note_startup(canonical, temporary, LineEnding::Crlf)
    }

    fn fact_of(observation: &NoteObservation) -> ExternalFileFact {
        match &observation.state {
            ExternalFileState::Present(fact) => fact.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn with_no_temporary_the_canonical_note_opens_or_a_new_one_is_created() {
        let canonical = valid("text");
        let step = plan(Some(&canonical), None).step;
        assert!(
            matches!(&step, NoteStartupStep::Open(NoteDocumentPlan::Load(fact)) if *fact == fact_of(&canonical)),
            "{step:?}"
        );
        let step = plan(None, None).step;
        assert!(
            matches!(step, NoteStartupStep::Open(NoteDocumentPlan::CreateEmpty)),
            "{step:?}"
        );
    }

    #[test]
    fn an_unusable_canonical_note_is_refused_and_never_replaced() {
        let step = plan(Some(&invalid(&[0xff])), None).step;
        assert!(
            matches!(
                step,
                NoteStartupStep::Open(NoteDocumentPlan::Refuse(NoteRefusal::InvalidUtf8))
            ),
            "{step:?}"
        );
        let step = plan(Some(&too_large(9)), None).step;
        assert!(
            matches!(
                step,
                NoteStartupStep::Open(NoteDocumentPlan::Refuse(NoteRefusal::TooLarge(9)))
            ),
            "{step:?}"
        );
    }

    #[test]
    fn an_identical_temporary_is_redundant() {
        let canonical = valid("same");
        let step = plan(Some(&canonical), Some(&valid("same"))).step;
        assert!(
            matches!(&step, NoteStartupStep::RemoveRedundantTemporary(Ok(fact)) if *fact == fact_of(&canonical)),
            "{step:?}"
        );
    }

    #[test]
    fn an_empty_temporary_beside_a_usable_note_is_redundant() {
        let canonical = valid("text");
        let step = plan(Some(&canonical), Some(&valid(""))).step;
        assert!(
            matches!(&step, NoteStartupStep::RemoveRedundantTemporary(Ok(fact)) if *fact == fact_of(&canonical)),
            "{step:?}"
        );
    }

    /// The observations disagree, which only a change between two reads can do. The canonical
    /// note cannot be opened, so the temporary must stay.
    #[test]
    fn a_redundant_temporary_is_kept_when_the_canonical_note_cannot_be_opened() {
        let mut canonical = valid("same");
        canonical.state = ExternalFileState::InvalidUtf8 {
            fingerprint: fingerprint(b"same"),
        };
        let step = plan(Some(&canonical), Some(&valid("same"))).step;
        assert!(
            matches!(
                step,
                NoteStartupStep::RemoveRedundantTemporary(Err(NoteRefusal::InvalidUtf8))
            ),
            "{step:?}"
        );
    }

    #[test]
    fn a_different_temporary_is_a_choice_for_the_user() {
        let step = plan(Some(&valid("old")), Some(&valid("new"))).step;
        let NoteStartupStep::Recover {
            canonical_to_preserve,
            document,
            ..
        } = step
        else {
            panic!("{step:?}");
        };
        assert_eq!(canonical_to_preserve, None);
        assert_eq!(document.snapshot().text.as_ref(), "old");
    }

    #[test]
    fn a_restore_must_preserve_an_unusable_canonical_note_first() {
        let bytes = [0xff];
        let step = plan(Some(&invalid(&bytes)), Some(&valid("new"))).step;
        let NoteStartupStep::Recover {
            canonical_to_preserve,
            ..
        } = step
        else {
            panic!("{step:?}");
        };
        assert_eq!(
            canonical_to_preserve,
            Some(PreserveExpectation::Content(fingerprint(&bytes)))
        );

        let step = plan(Some(&too_large(9)), Some(&valid("new"))).step;
        let NoteStartupStep::Recover {
            canonical_to_preserve,
            ..
        } = step
        else {
            panic!("{step:?}");
        };
        assert_eq!(
            canonical_to_preserve,
            Some(PreserveExpectation::Unverifiable)
        );
    }

    /// The only place a startup makes up a document's line ending without a file to read it from.
    #[test]
    fn a_recovery_with_no_canonical_note_uses_the_hosts_line_ending() {
        for line_ending in [LineEnding::Crlf, LineEnding::Lf] {
            let step = plan_note_startup(None, Some(&valid("new")), line_ending).step;
            let NoteStartupStep::Recover { document, .. } = step else {
                panic!("{step:?}");
            };
            assert_eq!(document.snapshot().line_ending, line_ending);
            assert_eq!(document.snapshot().text.as_ref(), "");
        }
    }

    #[test]
    fn an_invalid_temporary_is_moved_aside_then_the_note_opens_as_usual() {
        let canonical = valid("text");
        let step = plan(Some(&canonical), Some(&invalid(&[0xff]))).step;
        assert!(
            matches!(&step, NoteStartupStep::QuarantineInvalidTemporary(NoteDocumentPlan::Load(fact)) if *fact == fact_of(&canonical)),
            "{step:?}"
        );
        let step = plan(None, Some(&invalid(&[0xff]))).step;
        assert!(
            matches!(
                step,
                NoteStartupStep::QuarantineInvalidTemporary(NoteDocumentPlan::CreateEmpty)
            ),
            "{step:?}"
        );
    }

    #[test]
    fn an_invalid_temporary_is_moved_aside_even_when_the_note_is_then_refused() {
        let step = plan(Some(&too_large(9)), Some(&invalid(&[0xff]))).step;
        assert!(
            matches!(
                step,
                NoteStartupStep::QuarantineInvalidTemporary(NoteDocumentPlan::Refuse(
                    NoteRefusal::TooLarge(9)
                ))
            ),
            "{step:?}"
        );
    }

    #[test]
    fn an_oversized_temporary_is_moved_aside_before_the_note_is_opened() {
        let canonical = valid("text");
        let with_note = plan(Some(&canonical), Some(&too_large(9)));
        assert_eq!(
            with_note.oversized_temporary,
            Some(OversizedTemporary {
                bytes: 9,
                canonical_exists: true
            })
        );
        assert!(
            matches!(&with_note.step, NoteStartupStep::Open(NoteDocumentPlan::Load(fact)) if *fact == fact_of(&canonical)),
            "{:?}",
            with_note.step
        );

        let without_note = plan(None, Some(&too_large(9)));
        assert_eq!(
            without_note.oversized_temporary,
            Some(OversizedTemporary {
                bytes: 9,
                canonical_exists: false
            })
        );
    }

    #[test]
    fn the_warnings_name_where_the_temporary_went() {
        let kept = Path::new("kept.tmp");
        assert_eq!(
            oversized_temporary_warning(kept),
            "note.md.tmp 超过安全载入上限；已隔离保留为 kept.tmp。"
        );
        assert_eq!(
            invalid_temporary_warning(kept),
            "note.md.tmp 不是有效 UTF-8；已隔离保留为 kept.tmp。"
        );
    }
}
