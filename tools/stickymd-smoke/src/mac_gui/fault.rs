//! The save fault W11/W12 inject (USER 2026-11-11): the fixture's note directory loses its write
//! permission. The change goes through a descriptor opened without following links, inside the
//! session directory whose identity is checked first, and a guard gives the permission back when it
//! is dropped: an unwinding scenario cannot leave a directory that cleanup cannot remove.
//! Premise: a dedicated test VM with no other writer running as the same user. The session
//! directory is validated and then `note` is opened by path, so a same-user process that replaced
//! the session directory between the two could still redirect the change.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use crate::mac_common::session::Anchor;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};

/// Darwin `<sys/fcntl.h>`; the crate takes no `libc` dependency for two flags.
const O_NOFOLLOW: i32 = 0x0000_0100;
const O_DIRECTORY: i32 = 0x0010_0000;

/// Takes every write permission from the session's `note` directory, or gives the owner's back.
/// A `note` that is a link, or a session directory that was replaced, is refused unchanged; the
/// mode is changed on the opened descriptor, never by path.
pub(super) fn note_writable(session: &Anchor, writable: bool) -> Result<(), String> {
    session.validate()?;
    let note = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW | O_DIRECTORY)
        .open(session.path().join("note"))
        .map_err(|e| format!("note directory: {e}; no permission was changed"))?;
    let mut permissions = note
        .metadata()
        .map_err(|e| format!("note directory: {e}"))?
        .permissions();
    let mode = permissions.mode();
    permissions.set_mode(if writable {
        mode | 0o200
    } else {
        mode & !0o222
    });
    note.set_permissions(permissions)
        .map_err(|e| format!("note directory mode: {e}"))
}

/// Gives the owner's write permission back once, explicitly or when dropped (also on unwind).
/// Declared after the session, it drops first, so the session's cleanup sees a writable tree.
pub(super) struct RestoreOnDrop(Option<Anchor>);

impl RestoreOnDrop {
    /// `None` for scenarios that inject no fault.
    pub fn new(session: Option<Anchor>) -> Self {
        Self(session)
    }

    /// Nothing to restore when the fixture was never created.
    pub fn restore(&mut self) -> Result<(), String> {
        match self.0.take() {
            Some(session) if session.path().join("note").exists() => note_writable(&session, true),
            _ => Ok(()),
        }
    }
}

impl Drop for RestoreOnDrop {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("note directory permission not restored: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac_common::session::Session;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn mode(path: &std::path::Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn an_unwinding_scenario_still_gets_its_writable_tree_back() {
        let mut session = Session::new().unwrap();
        let note = session.path().join("note");
        fs::create_dir(&note).unwrap();
        fs::write(note.join("note.md"), "kept").unwrap();
        let anchor = session.anchor();
        let unwound = std::panic::catch_unwind(|| {
            let _guard = RestoreOnDrop::new(Some(anchor.clone()));
            note_writable(&anchor, false).unwrap();
            assert_eq!(mode(&note) & 0o222, 0);
            panic!("scenario failed after the fault");
        });
        // Only the planned panic counts: an earlier one would mean the fault never took hold.
        let payload = unwound.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"scenario failed after the fault")
        );
        assert_ne!(mode(&note) & 0o200, 0);
        session.cleanup().unwrap();
        assert!(!note.exists());
    }

    #[test]
    fn a_linked_note_or_a_replaced_session_is_refused_unchanged() {
        let mut session = Session::new().unwrap();
        let elsewhere = Session::new().unwrap();
        let before = mode(elsewhere.path());
        symlink(elsewhere.path(), session.path().join("note")).unwrap();
        assert!(note_writable(&session.anchor(), false).is_err());
        assert_eq!(mode(elsewhere.path()), before);

        // A session directory moved away and replaced by another fails the identity check.
        let anchor = session.anchor();
        let moved = anchor.path().with_extension("moved");
        fs::rename(anchor.path(), &moved).unwrap();
        fs::create_dir(anchor.path()).unwrap();
        fs::create_dir(anchor.path().join("note")).unwrap();
        let replacement = mode(&anchor.path().join("note"));
        assert!(note_writable(&anchor, false).is_err());
        assert_eq!(mode(&anchor.path().join("note")), replacement);
        fs::remove_dir_all(anchor.path()).unwrap();
        fs::rename(&moved, anchor.path()).unwrap();
        session.cleanup().unwrap();
    }
}
