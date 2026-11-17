use crate::platform::{Blocker, Notifier, Process};
use limiter_core::{budget::State, rules::Rule};
use std::collections::HashMap;

#[derive(Default)]
pub struct Enforcement {
    notified: bool,
    pending: HashMap<u32, (u64, String, bool)>,
}
impl Enforcement {
    pub fn apply(
        &mut self,
        tick: (u64, &State),
        rule: &Rule,
        processes: &[Process],
        notifier: &mut impl Notifier,
        blocker: &mut impl Blocker,
    ) {
        let (now, state) = tick;
        if matches!(state, State::Ok { .. }) {
            self.notified = false;
            self.pending.clear();
        }
        if let State::Warning { kill_in } = state {
            if !self.notified {
                match notifier.notify(&rule.target.process, *kill_in) {
                    Ok(()) => self.notified = true,
                    Err(error) => eprintln!("notification: {error}"),
                }
            }
        }
        self.service(now, processes, blocker);
        if rule.action.kill && matches!(state, State::Exceeded) {
            for process in processes {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.pending.entry(process.pid)
                {
                    match blocker.terminate(process.pid) {
                        Ok(()) => {
                            entry.insert((now, process.argv.clone(), false));
                        }
                        Err(error) => eprintln!("SIGTERM: {error}"),
                    }
                }
            }
        }
    }
    pub fn next_delay(&self, now: u64) -> Option<u64> {
        self.pending
            .values()
            .filter(|(_, _, killed)| !killed)
            .map(|(sent, _, _)| sent.saturating_add(3).saturating_sub(now))
            .min()
    }
    pub fn service(&mut self, now: u64, processes: &[Process], blocker: &mut impl Blocker) {
        self.pending
            .retain(|pid, (_, argv, _)| processes.iter().any(|p| p.pid == *pid && p.argv == *argv));
        for (&pid, (sent, _, killed)) in &mut self.pending {
            if !*killed && now.saturating_sub(*sent) >= 3 {
                match blocker.kill(pid) {
                    Ok(()) => *killed = true,
                    Err(error) => {
                        eprintln!("SIGKILL: {error}");
                        *sent = now;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ProcessWatcher;
    use std::io;
    fn rule() -> limiter_core::rules::Rule {
        limiter_core::rules::parse(include_str!("../../../config/rules.example.toml"))
            .unwrap()
            .rule
            .remove(0)
    }
    #[derive(Default)]
    struct FakeNotifier(Vec<u64>);
    impl Notifier for FakeNotifier {
        fn notify(&mut self, _: &str, seconds: u64) -> io::Result<()> {
            self.0.push(seconds);
            Ok(())
        }
    }
    #[derive(Default)]
    struct FakeBlocker(Vec<(u32, &'static str)>);
    impl Blocker for FakeBlocker {
        fn terminate(&mut self, pid: u32) -> io::Result<()> {
            self.0.push((pid, "TERM"));
            Ok(())
        }
        fn kill(&mut self, pid: u32) -> io::Result<()> {
            self.0.push((pid, "KILL"));
            Ok(())
        }
    }
    struct FakeWatcher;
    impl ProcessWatcher for FakeWatcher {
        fn processes(&mut self) -> io::Result<Vec<Process>> {
            Ok(vec![Process {
                pid: 42,
                argv: "wine C:\\games\\noita.exe".into(),
            }])
        }
    }
    #[test]
    fn notify_then_term_then_kill() {
        let mut enforcer = Enforcement::default();
        let mut notifier = FakeNotifier::default();
        let mut blocker = FakeBlocker::default();
        let processes = FakeWatcher.processes().unwrap();
        for (now, state) in [
            (0, State::Warning { kill_in: 10 }),
            (2, State::Warning { kill_in: 8 }),
            (10, State::Exceeded),
            (12, State::Exceeded),
        ] {
            enforcer.apply(
                (now, &state),
                &rule(),
                &processes,
                &mut notifier,
                &mut blocker,
            );
        }
        assert_eq!(notifier.0, vec![10]);
        assert_eq!(blocker.0, vec![(42, "TERM")]);
        enforcer.apply(
            (13, &State::Exceeded),
            &rule(),
            &processes,
            &mut notifier,
            &mut blocker,
        );
        assert_eq!(blocker.0, vec![(42, "TERM"), (42, "KILL")]);
    }
    #[test]
    fn exited_or_reused_pid_is_not_killed() {
        let mut enforcer = Enforcement::default();
        let mut notifier = FakeNotifier::default();
        let mut blocker = FakeBlocker::default();
        let processes = FakeWatcher.processes().unwrap();
        enforcer.apply(
            (0, &State::Exceeded),
            &rule(),
            &processes,
            &mut notifier,
            &mut blocker,
        );
        enforcer.apply(
            (3, &State::Exceeded),
            &rule(),
            &[],
            &mut notifier,
            &mut blocker,
        );
        assert_eq!(blocker.0, vec![(42, "TERM")]);
    }
}
