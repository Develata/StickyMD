//! Bounded cleanup of LaunchServices processes in this private fixture only.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::{collections::BTreeMap, path::Path, time::Instant};

pub(super) trait Processes {
    fn now(&self) -> u64;
    fn wait(&mut self);
    fn matching(&mut self) -> Result<Vec<u32>, String>;
    fn signal(&mut self, pid: u32, signal: i32) -> Result<(), String>;
}

pub(super) struct Native<'a> {
    pub executable: &'a Path,
    pub started: Instant,
}
impl Processes for Native<'_> {
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn wait(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    fn matching(&mut self) -> Result<Vec<u32>, String> {
        super::native::matching_processes(self.executable)
    }
    fn signal(&mut self, pid: u32, signal: i32) -> Result<(), String> {
        super::native::signal_matching(pid, self.executable, signal)
    }
}

pub(super) fn drain(p: &mut impl Processes, owned: u32) -> Result<Vec<u32>, String> {
    let start = p.now();
    let mut seen = BTreeMap::new();
    loop {
        let foreign = p
            .matching()?
            .into_iter()
            .filter(|pid| *pid != owned)
            .collect::<Vec<_>>();
        let now = p.now();
        for pid in &foreign {
            let (first, killed) = seen.entry(*pid).or_insert_with(|| (now, false));
            if now == *first {
                p.signal(*pid, 15)?;
            } else if now - *first >= 3000 && !*killed {
                p.signal(*pid, 9)?;
                *killed = true;
            }
            if now - *first >= 6000 {
                return Err(format!(
                    "foreign fixture PID {pid} not confirmed gone after SIGTERM/SIGKILL"
                ));
            }
        }
        // open may return before LaunchServices creates the process. Observe for
        // three seconds even when the first enumeration is empty; cleanup audits again.
        if foreign.is_empty() && now - start >= 3000 {
            return Ok(seen.into_keys().collect());
        }
        if now - start >= 9000 {
            return Err("foreign fixture process audit exceeded 9s".into());
        }
        p.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        now: u64,
        gone: bool,
        resist: bool,
        error: bool,
        late: u64,
        signals: Vec<(u64, u32, i32)>,
    }
    impl Processes for Fake {
        fn now(&self) -> u64 {
            self.now
        }
        fn wait(&mut self) {
            self.now += 100;
        }
        fn matching(&mut self) -> Result<Vec<u32>, String> {
            if self.error {
                return Err("enumeration incomplete".into());
            }
            Ok(if self.gone || self.now < self.late {
                vec![42]
            } else {
                vec![42, 99]
            })
        }
        fn signal(&mut self, pid: u32, signal: i32) -> Result<(), String> {
            self.signals.push((self.now, pid, signal));
            if signal == 9 && !self.resist {
                self.gone = true;
            }
            Ok(())
        }
    }
    fn fake() -> Fake {
        Fake {
            now: 0,
            gone: false,
            resist: false,
            error: false,
            late: 0,
            signals: Vec::new(),
        }
    }
    #[test]
    fn foreign_only_gets_term_then_kill_after_three_seconds() {
        let mut p = fake();
        assert_eq!(drain(&mut p, 42).unwrap(), [99]);
        assert_eq!(p.signals, [(0, 99, 15), (3000, 99, 9)]);
        assert!(p.gone);
    }
    #[test]
    fn late_launch_is_observed_and_unconfirmed_exit_or_enumeration_fails() {
        let mut p = fake();
        p.late = 1000;
        assert_eq!(drain(&mut p, 42).unwrap(), [99]);
        assert_eq!(p.signals, [(1000, 99, 15), (4000, 99, 9)]);
        let mut p = fake();
        p.resist = true;
        assert!(
            drain(&mut p, 42)
                .unwrap_err()
                .contains("not confirmed gone")
        );
        let mut p = fake();
        p.error = true;
        assert!(drain(&mut p, 42).unwrap_err().contains("incomplete"));
        assert!(p.signals.is_empty());
    }
}
