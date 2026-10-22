//! Own the exact one- or two-process set declared for a manual scenario.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::{Instance, Launched, Workshop, running_stickymd_processes};
use crate::qualification::manual::flow::Tested;
use crate::qualification::manual_observation::{instances, registry::InstanceRule};

pub(in crate::qualification::manual) struct LaunchedSet {
    processes: Vec<Launched>,
    instances: Vec<Instance>,
}

impl Workshop {
    pub(in crate::qualification::manual) fn launch_set(
        &mut self,
        rule: InstanceRule,
    ) -> Result<LaunchedSet, String> {
        let fixture = match rule {
            InstanceRule::Launched(fixture)
            | InstanceRule::Preserved(fixture)
            | InstanceRule::TwoCopies(fixture) => fixture,
            InstanceRule::FirstRunDeclared => {
                return Err("first run has no recorder launch".to_owned());
            }
        };
        let mut set = LaunchedSet {
            processes: Vec::new(),
            instances: Vec::new(),
        };
        for _ in 0..instances::count(rule) {
            let allowed: Vec<_> = set.instances.iter().map(|instance| instance.pid).collect();
            let launched = self.launch_owned(fixture, &allowed)?;
            set.instances.push(launched.instance.clone());
            set.processes.push(launched);
        }
        // launch_owned always creates a new directory, even for identical fixtures.
        if set.processes.len() == 2 && set.processes[0].program == set.processes[1].program {
            return Err("two copies must use distinct program directories".to_owned());
        }
        instances::validate(rule, &set.instances, Some(&self.exe_sha256))?;
        Ok(set)
    }
}

impl Tested for LaunchedSet {
    fn prepare(
        &mut self,
        item: &crate::qualification::manual::flow::Item,
        input: &mut dyn std::io::BufRead,
        output: &mut dyn std::io::Write,
    ) -> Result<crate::qualification::manual::flow::Preparation, String> {
        super::procedure::prepare(&mut self.processes, item, input, output)
    }

    fn instances(&self) -> &[Instance] {
        &self.instances
    }

    fn startup_error(&self) -> Option<&str> {
        self.processes
            .iter()
            .find_map(|process| process.startup_error.as_deref())
    }

    fn still_exact(&mut self) -> Result<bool, String> {
        for process in &mut self.processes {
            if !process.child.is_running()? {
                return Ok(false);
            }
        }
        let mut expected: Vec<_> = self.instances.iter().map(|instance| instance.pid).collect();
        let mut actual = running_stickymd_processes()?;
        expected.sort_unstable();
        actual.sort_unstable();
        Ok(actual == expected)
    }
}
