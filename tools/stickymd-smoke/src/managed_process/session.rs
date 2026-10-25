//! Which Windows session a process runs in, so another sign-in's StickyMD is not this desktop's.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

#[link(name = "kernel32")]
unsafe extern "system" {
    fn ProcessIdToSessionId(process_id: u32, session_id: *mut u32) -> i32;
}

fn session_of(pid: u32) -> Option<u32> {
    let mut session = 0;
    // SAFETY: `session` is writable DWORD storage; the API retains no pointer.
    (unsafe { ProcessIdToSessionId(pid, &raw mut session) } != 0).then_some(session)
}

/// The `pids` not proven to run in another session than this process. The process list is
/// system-wide, but only this session's desktop and tray can be clicked; a session that cannot
/// be read keeps the PID, so nothing unproven is excused.
pub(crate) fn outside_other_sessions(pids: Vec<u32>) -> Vec<u32> {
    keep_unless_other_session(pids, session_of(std::process::id()), session_of)
}

fn keep_unless_other_session(
    pids: Vec<u32>,
    own: Option<u32>,
    session_of: impl Fn(u32) -> Option<u32>,
) -> Vec<u32> {
    let Some(own) = own else {
        return pids;
    };
    pids.into_iter()
        .filter(|&pid| session_of(pid).is_none_or(|session| session == own))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::keep_unless_other_session;

    /// 10 runs here, 20 in another session, 30's session cannot be read.
    #[test]
    fn only_a_process_proven_in_another_session_is_dropped() {
        let session = |pid| match pid {
            10 => Some(1),
            20 => Some(2),
            _ => None,
        };
        assert_eq!(
            keep_unless_other_session(vec![10, 20, 30], Some(1), session),
            vec![10, 30]
        );
        // This process's own session unknown: nothing is proven, everything stays.
        assert_eq!(
            keep_unless_other_session(vec![10, 20, 30], None, session),
            vec![10, 20, 30]
        );
    }

    /// The real call answers for this process.
    #[test]
    fn this_process_is_in_its_own_session() {
        assert!(super::session_of(std::process::id()).is_some());
        assert_eq!(
            super::outside_other_sessions(vec![std::process::id()]),
            vec![std::process::id()]
        );
    }
}
