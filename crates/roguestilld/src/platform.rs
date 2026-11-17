use std::{io, process::Command};

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub argv: String,
}
pub trait ProcessWatcher {
    fn processes(&mut self) -> io::Result<Vec<Process>>;
}
pub trait Notifier {
    fn notify(&mut self, target: &str, seconds: u64) -> io::Result<()>;
}
pub trait Blocker {
    fn terminate(&mut self, pid: u32) -> io::Result<()>;
    fn kill(&mut self, pid: u32) -> io::Result<()>;
}
pub struct MacOs;
fn success(mut command: Command) -> io::Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("command failed: {status}")))
    }
}
impl ProcessWatcher for MacOs {
    fn processes(&mut self) -> io::Result<Vec<Process>> {
        let output = Command::new("ps")
            .args(["-axo", "pid=,command="])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("ps failed"));
        }
        Ok(parse_processes(&String::from_utf8_lossy(&output.stdout)))
    }
}
pub fn parse_processes(output: &str) -> Vec<Process> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let split = line.find(char::is_whitespace)?;
            Some(Process {
                pid: line[..split].parse().ok()?,
                argv: line[split..].trim().to_owned(),
            })
        })
        .collect()
}
fn stem(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    for suffix in [".exe", ".app"] {
        if let Some(stripped) = lower.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    lower
}
pub fn matches(process: &Process, target: &str) -> bool {
    let wanted = stem(target);
    process.argv.split([' ', '\t', '"', '\'']).any(|part| {
        part.rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| !wanted.is_empty() && stem(name) == wanted)
    })
}
pub fn matches_any(process: &Process, targets: &[String]) -> bool {
    targets.iter().any(|target| matches(process, target))
}
impl Notifier for MacOs {
    fn notify(&mut self, target: &str, seconds: u64) -> io::Result<()> {
        let message = format!("{target}: daily budget exhausted. Closing in {seconds} seconds.");
        let mut command = Command::new("osascript");
        command.args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 1 of argv) with title \"roguestill\"",
            "-e",
            "end run",
            &message,
        ]);
        success(command)
    }
}
impl Blocker for MacOs {
    fn terminate(&mut self, pid: u32) -> io::Result<()> {
        signal(pid, "-TERM")
    }
    fn kill(&mut self, pid: u32) -> io::Result<()> {
        signal(pid, "-KILL")
    }
}
fn signal(pid: u32, signal: &str) -> io::Result<()> {
    let mut command = Command::new("/bin/kill");
    command.args([signal, &pid.to_string()]);
    success(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wine_full_argv_matching() {
        let processes = parse_processes(
            " 12 /wine C:\\games\\noita.exe -arg\n 13 /wine noita.exe.bak\n 14 /games/NOITA.EXE\n",
        );
        assert_eq!(processes.len(), 3);
        assert!(matches(&processes[0], "noita.exe"));
        assert!(!matches(&processes[1], "noita.exe"));
        assert!(matches(&processes[2], "noita.exe"));
    }
}

#[cfg(test)]
mod match_tests {
    use super::*;
    fn process(argv: &str) -> Process {
        Process {
            pid: 1,
            argv: argv.into(),
        }
    }
    #[test]
    fn friendly_names_match_executables() {
        let wine = process("C:\\Program Files\\Noita\\noita.exe CX_GRAPHICS=1");
        assert!(matches(&wine, "noita.exe"));
        assert!(matches(&wine, "Noita"));
        assert!(!matches(&wine, "Noit"));
        let app = process("/Applications/Reddit.app/Contents/MacOS/Reddit");
        assert!(matches(&app, "reddit"));
        assert!(matches(&app, "Reddit.app"));
        assert!(!matches(&app, "Redd"));
    }
}
