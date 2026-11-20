//! The processes whose working directory is inside a directory, for the lease pool (`lease`): a slot with a process in
//! it is busy, a shell or a build daemon there would see its files change. One scan reads every process of the user
//! once: on macOS through libproc (`proc_listallpids`, `proc_pidinfo`), on Linux from `/proc`. Windows has no scan of
//! the working directories: `ProcessScan::Unavailable`, and a lease never waits for it; it tells the start of a process
//! by its pid (`process_started`), as the other two do, and the ancestors of this process from one snapshot of the
//! process list (`CreateToolhelp32Snapshot`: the pid, the parent and the exe of each, no memory of another process), so
//! a lease names the session it was got for there too (`ProcessScan::ancestors`). The scan leaves out the session that
//! runs `brd`: this
//! process, its ancestors and every process of their process groups (the `jq` of `brd lease return | jq`, the
//! `caffeinate` of the agent). It reads the argv of each process too (`KERN_PROCARGS2` on macOS, `/proc/<pid>/cmdline`
//! on Linux): a Bazel server names its workspace there (`--workspace_directory=`), and serves only that one, since Bazel
//! derives its output base from the workspace path. The index lock of a worktree asks who has a file open
//! (`processes_with_open_file`) and when a git started (`ProcessInfo::started_at`): a lock no process owns was left by a
//! killed git.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A process the scan read
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
  pub pid: u32,
  /// The parent's pid; 0 when unknown
  pub parent: u32,
  /// The process group (`pgid`); 0 when unknown
  pub group: u32,
  /// The command name, as `ps -o comm` shows it
  pub name: String,
  /// The working directory, as the kernel resolved it (no symlinks); `None` when it can't be read (another user's)
  pub cwd: Option<PathBuf>,
  /// The arguments, the program first; empty when they can't be read
  pub argv: Vec<String>,
  /// When it started (`process_started`)
  pub started: Option<u64>,
  /// Whether it has a controlling terminal, as `ps -o tty` shows one: an interactive shell, the command of a terminal tab;
  /// a command that a coding agent runs, a daemon and a service have none. `false` on Windows and where unknown
  pub terminal: bool,
}

const WORKSPACE_DIRECTORY: &str = "--workspace_directory=";
const OUTPUT_BASE: &str = "--output_base=";

impl ProcessInfo {
  /// The workspace of a Bazel server: the value of its `--workspace_directory=`, canonical when it exists
  pub fn bazel_workspace(&self) -> Option<PathBuf> {
    let workspace = Path::new(self.argv.iter().find_map(|arg| arg.strip_prefix(WORKSPACE_DIRECTORY))?);
    Some(std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf()))
  }

  /// The `--output_base=` of a Bazel server
  pub fn output_base(&self) -> Option<&str> {
    self.argv.iter().find_map(|arg| arg.strip_prefix(OUTPUT_BASE))
  }

  /// When it started, on the wall clock: to the microsecond on macOS, on Linux by the clock ticks since the boot, whose
  /// time `/proc/stat` gives to the second, on Windows to the 100 ns; `None` elsewhere and when unknown
  pub fn started_at(&self) -> Option<SystemTime> {
    scan::started_at(self.started?)
  }

  /// Whether the program is git: its argv starts with `git` or a path to it
  pub fn is_git(&self) -> bool {
    self
      .argv
      .first()
      .is_some_and(|program| matches!(Path::new(program).file_name().and_then(|name| name.to_str()), Some("git" | "git.exe")))
  }

  /// The subcommand of a git argv: the first argument that is no option, past the value of `-c` and `-C`
  fn git_subcommand(&self) -> Option<&str> {
    let mut args = self.argv.iter().skip(1);
    while let Some(arg) = args.next() {
      match arg.as_str() {
        "-c" | "-C" => {
          args.next();
        }
        option if option.starts_with('-') => {}
        subcommand => return Some(subcommand),
      }
    }
    None
  }

  /// `git maintenance …` or `git gc --auto`
  fn is_git_maintenance_root(&self) -> bool {
    self.is_git()
      && match self.git_subcommand() {
        Some("maintenance") => true,
        Some("gc") => self.argv.iter().any(|arg| arg == "--auto"),
        _ => false,
      }
  }

  /// `git fsmonitor--daemon run`: it watches one worktree for file events and writes only its cookie files in the git
  /// dir. git moves it out of the worktree to `$HOME`, but without `HOME` it stays there. It must outlive the processes
  /// of a run, since its event log lets the next `status` of the worktree skip the walk of every file
  fn is_fsmonitor_daemon(&self) -> bool {
    self.is_git() && self.git_subcommand() == Some("fsmonitor--daemon")
  }

  /// Whether the process works in `dir` (canonical): its working directory, or the workspace of a Bazel server, is
  /// `dir` or below it, by path components
  fn works_in(&self, dir: &Path) -> bool {
    self.cwd.as_ref().is_some_and(|cwd| cwd.starts_with(dir)) || self.bazel_workspace().is_some_and(|workspace| workspace.starts_with(dir))
  }

  /// `caffeinate` holds a power assertion and opens no file, whatever its working directory: Claude Code starts one in
  /// the directory of a session to keep the Mac awake. A command it wraps is a process of its own and still counts
  fn holds_only_its_directory(&self) -> bool {
    self.name == "caffeinate"
  }

  /// The arguments as one line, at most `limit` characters (`…` at the cut); the name without them
  pub fn command_line(&self, limit: usize) -> String {
    let line = if self.argv.is_empty() { self.name.clone() } else { self.argv.join(" ") };
    if line.chars().count() <= limit {
      return line;
    }
    let mut cut: String = line.chars().take(limit.saturating_sub(1)).collect();
    cut.push('…');
    cut
  }
}

/// Shells and wrappers that run a command for someone else: the holder of a lease is the nearest ancestor that is
/// none of them (`Ancestors::session`), compared ignoring case (an exe of Windows, its `.exe` cut). Not `node`: Claude
/// Code installed by npm is that `node.exe` itself, and the terminal above it would outlive every session; not
/// `conhost`, which is a child of the console program, never its parent
const RUNNERS: &[&str] = &[
  "sh",
  "bash",
  "zsh",
  "dash",
  "fish",
  "ksh",
  "tcsh",
  "csh",
  "nu",
  "pwsh",
  "powershell",
  "cmd",
  "env",
  "nohup",
  "timeout",
  "xargs",
  "time",
  "sudo",
  "brd",
  "branch-deck",
];

/// The ancestors of this process, its parent first, each with its start time when the system tells it (not of another
/// user's process or a protected one on Windows)
#[derive(Debug, Clone)]
pub struct Ancestors {
  links: Vec<ProcessInfo>,
  /// Whether the chain reached its end: a parent of pid 1 or below, or one no longer in the list. A chain cut at a pid
  /// seen twice or, on Windows, at a parent that started after its child is not whole: what was above it is unknown
  whole: bool,
}

impl Ancestors {
  /// The process this one works for: the nearest ancestor that is no shell or wrapper (`RUNNERS`), the coding agent or
  /// the terminal; `None` when the chain has none below the init process, or when the system won't tell its start (a
  /// service that runs the command on Windows), since a holder must be told from a process that got its pid later
  pub fn session(&self) -> Option<&ProcessInfo> {
    self
      .links
      .iter()
      .find(|process| !RUNNERS.iter().any(|runner| runner.eq_ignore_ascii_case(&process.name)))
      .filter(|process| process.started.is_some())
  }

  /// Whether the process `pid`, started at `started` when known, is one of them: `None` when it is not among the links
  /// of a chain that is not whole, since it may be above the cut
  pub fn contains(&self, pid: u32, started: Option<u64>) -> Option<bool> {
    let found = self
      .links
      .iter()
      .any(|process| process.pid == pid && started.is_none_or(|started| process.started == Some(started)));
    (found || self.whole).then_some(found)
  }
}

/// The ancestors of `pid` among `processes`, each with the start time `started` tells, when it does. With
/// `younger_parent_ends`, a parent that started after the nearest child whose start is known ends the chain, not
/// whole: Windows keeps the parent pid of a process after that parent exited, and may have given the pid to a younger
/// process since; macOS and Linux give an orphan to the init process instead, and a wall clock that stepped back must
/// not cut their chain
pub(crate) fn chain(processes: &[ProcessInfo], pid: u32, started: impl Fn(&ProcessInfo) -> Option<u64>, younger_parent_ends: bool) -> Ancestors {
  let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|process| (process.pid, process)).collect();
  let mut links = Vec::new();
  let Some(own) = by_pid.get(&pid) else { return Ancestors { links, whole: false } };
  let mut child_started = started(own);
  let mut seen = HashSet::from([pid]);
  let mut current = own.parent;
  let whole = loop {
    if current <= 1 {
      break true;
    }
    if !seen.insert(current) {
      break false;
    }
    let Some(process) = by_pid.get(&current) else { break true };
    let process_started = started(process);
    if younger_parent_ends && process_started.zip(child_started).is_some_and(|(parent, child)| parent > child) {
      break false;
    }
    links.push(ProcessInfo {
      started: process_started,
      ..(*process).clone()
    });
    child_started = process_started.or(child_started);
    current = process.parent;
  };
  Ancestors { links, whole }
}

/// The processes of one scan
#[derive(Debug, Clone)]
pub enum ProcessScan {
  Done {
    processes: Vec<ProcessInfo>,
    /// The session that runs this process: it, its ancestors and the processes of their groups
    own: HashSet<u32>,
  },
  /// No scan on this platform: nothing is known to be inside a directory; on Windows the ancestors of this process are
  /// still known (`ancestors`)
  Unavailable,
}

impl ProcessScan {
  /// Whether the system of this build lists the processes and their working directories (`scan` gives `Done`): what an
  /// answer tells when it needed no scan
  pub const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "linux"));

  /// Read every process now
  pub fn scan() -> Self {
    match platform::processes() {
      Some(processes) => {
        let own = own_processes(&processes, std::process::id());
        Self::Done { processes, own }
      }
      None => Self::Unavailable,
    }
  }

  pub const fn is_available(&self) -> bool {
    matches!(self, Self::Done { .. })
  }

  /// The pids of git's background maintenance: a `git maintenance` or a `git gc --auto` (a commit starts `git
  /// maintenance run --auto --detach`), and every git below it in the parent chain (`git repack`, `git multi-pack-index
  /// write`). They work on the shared object store and write nothing in a working tree, by their argv
  pub fn git_maintenance(&self) -> HashSet<u32> {
    let Self::Done { processes, .. } = self else { return HashSet::new() };
    let parents: HashMap<u32, u32> = processes.iter().map(|process| (process.pid, process.parent)).collect();
    let roots: HashSet<u32> = processes.iter().filter(|process| process.is_git_maintenance_root()).map(|process| process.pid).collect();
    processes
      .iter()
      .filter(|process| process.is_git())
      .filter(|process| {
        let mut current = process.pid;
        let mut seen = HashSet::new();
        while current != 0 && seen.insert(current) {
          if roots.contains(&current) {
            return true;
          }
          current = parents.get(&current).copied().unwrap_or(0);
        }
        false
      })
      .map(|process| process.pid)
      .collect()
  }

  /// The processes that work in `dir`: their working directory, or the workspace of a Bazel server, is `dir` or below
  /// it, by path components; `dir` is compared canonical, as the kernel names a working directory. Left out: the
  /// session that runs this process (`own`), git's background maintenance (`git_maintenance`: it writes only the shared
  /// object store), git's fsmonitor daemon (`is_fsmonitor_daemon`: it only watches) and `caffeinate`, which holds only
  /// its directory
  pub fn inside(&self, dir: &Path) -> Vec<&ProcessInfo> {
    let maintenance = self.git_maintenance();
    self
      .in_directory(dir)
      .into_iter()
      .filter(|process| !maintenance.contains(&process.pid) && !process.is_fsmonitor_daemon() && !process.holds_only_its_directory())
      .collect()
  }

  /// The pids of git's background maintenance in `dir`, which `inside` leaves out
  pub fn maintenance_inside(&self, dir: &Path) -> Vec<u32> {
    let maintenance = self.git_maintenance();
    self.in_directory(dir).iter().map(|process| process.pid).filter(|pid| maintenance.contains(pid)).collect()
  }

  /// The processes in `dir`, except the session of this process
  fn in_directory(&self, dir: &Path) -> Vec<&ProcessInfo> {
    let Self::Done { processes, own } = self else { return Vec::new() };
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    processes.iter().filter(|process| !own.contains(&process.pid) && process.works_in(&dir)).collect()
  }

  /// The ancestors of this process: from the scan, or where there is none from the list of the processes the platform
  /// gives (Windows: a snapshot of their pids, parents and exes); `None` where the system tells no list
  pub fn ancestors(&self) -> Option<Ancestors> {
    let pid = std::process::id();
    match self {
      Self::Done { processes, .. } => Some(chain(processes, pid, |process| process.started, false)),
      Self::Unavailable => {
        let processes = platform::process_tree()?;
        Some(chain(&processes, pid, |process| process.started.or_else(|| platform::started(process.pid)), cfg!(windows)))
      }
    }
  }

  /// The process this one works for (`Ancestors::session`); `None` without a list of the processes or when the chain
  /// has none below the init process
  pub fn session(&self) -> Option<ProcessInfo> {
    self.ancestors()?.session().cloned()
  }

  /// Whether the process `pid`, started at `started` when known, is an ancestor of this one: the session a lease was got
  /// for is one of the processes above a `brd` it runs, a shell or `cargo` between them or not. `None` when that can't
  /// be told: no list of the processes, or a chain cut below the init process (`Ancestors::contains`)
  pub fn has_ancestor(&self, pid: u32, started: Option<u64>) -> Option<bool> {
    self.ancestors()?.contains(pid, started)
  }

  /// The processes between the process `pid` of the scan and its ancestor `ancestor` (its pid and start), the parent of
  /// `pid` first: empty for a child of `ancestor`. `None` when `pid` runs below no such process, as far as the chain of
  /// its parents tells: another process with the pid of a gone ancestor is none, and a chain cut at a pid seen twice
  /// tells nothing; `None` without a scan too
  pub fn path_to(&self, pid: u32, ancestor: (u32, u64)) -> Option<Vec<&ProcessInfo>> {
    let Self::Done { processes, .. } = self else { return None };
    let ancestors = chain(processes, pid, |process| process.started, false);
    let at = ancestors
      .links
      .iter()
      .position(|process| process.pid == ancestor.0 && process.started == Some(ancestor.1))?;
    let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|process| (process.pid, process)).collect();
    ancestors.links[..at].iter().map(|link| by_pid.get(&link.pid).copied()).collect()
  }
}

/// The session of `pid` among `processes`: it, the chain of its parents, and every process of a group one of them is
/// in (the other commands of its pipeline, what the agent above it runs in its own group). The group of the init
/// process (pid 1) and an unknown group (0) are not counted
pub(crate) fn own_processes(processes: &[ProcessInfo], pid: u32) -> HashSet<u32> {
  let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|process| (process.pid, process)).collect();
  let mut own = HashSet::new();
  let mut groups = HashSet::new();
  let mut current = pid;
  while current != 0 && own.insert(current) {
    let Some(process) = by_pid.get(&current) else { break };
    if process.pid > 1 && process.group > 1 {
      groups.insert(process.group);
    }
    current = process.parent;
  }
  own.extend(processes.iter().filter(|process| groups.contains(&process.group)).map(|process| process.pid));
  own
}

/// A moment in the unit of `process_started` (`now`), with the units in one second (`per_second`): a start compared
/// with it carries no rounding of a conversion to the wall clock, where `started_at` of Linux adds the clock ticks to
/// `btime`, the boot in whole seconds, and so tells every start of a boot early by the fraction of its second
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartClock {
  pub now: u64,
  pub per_second: u64,
}

/// Now in the unit of `process_started`: the microseconds since the epoch on macOS, the clock ticks since the boot of
/// `/proc/uptime` on Linux; `None` elsewhere, and where the system won't tell it
pub fn start_clock() -> Option<StartClock> {
  scan::start_clock()
}

/// The clock ticks since the boot of the text of `/proc/uptime`, whose first field is the seconds since the boot with
/// their decimals; `ticks` is the clock ticks in a second
#[cfg_attr(
  not(any(target_os = "linux", test)),
  expect(dead_code, reason = "only Linux reads /proc/uptime; the tests read the text everywhere")
)]
fn uptime_ticks(uptime: &str, ticks: u64) -> Option<u64> {
  let seconds = uptime.split_whitespace().next()?;
  let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
  let mut from_fraction = 0;
  let mut scale = 1;
  for digit in fraction.chars().take(9) {
    from_fraction = from_fraction * 10 + u64::from(digit.to_digit(10)?);
    scale *= 10;
  }
  whole.parse::<u64>().ok()?.checked_mul(ticks)?.checked_add(from_fraction * ticks / scale)
}

/// When the process `pid` started, in a unit of the platform (microseconds on macOS, clock ticks since boot on Linux,
/// 100 ns since 1601 on Windows), so a pid that the system gave to another process since tells apart; `None` when
/// unknown (a gone process, one that exited, one the system won't describe to this process)
pub fn process_started(pid: u32) -> Option<u64> {
  platform::started(pid)
}

/// The CPU time that the process `pid` used so far, user and system, of all its threads: `proc_taskinfo` of libproc on
/// macOS, `utime` and `stime` of `/proc/<pid>/stat` on Linux; `None` for a gone process, one the system won't describe
/// to this process, and on another platform. The time of its reaped children is not in it
pub fn cpu_time(pid: u32) -> Option<std::time::Duration> {
  platform::cpu_time(pid)
}

/// The CPU time of a process, user and system, and of its reaped children (`cpu_times`)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuTimes {
  /// What the process used itself, of all its threads
  pub own: std::time::Duration,
  /// What its children used, once it reaped them, with what their reaped children used in turn
  pub children: std::time::Duration,
}

impl CpuTimes {
  /// The CPU time of the process and of its reaped children
  pub fn total(self) -> std::time::Duration {
    self.own + self.children
  }
}

/// The CPU time of the process `pid` and of its reaped children: `proc_pid_rusage` (`RUSAGE_INFO_V2`) on macOS, fields
/// 14 to 17 of `/proc/<pid>/stat` on Linux; `None` for a gone process, one the system won't describe to this process,
/// and on another platform. A child counts in it once the process waited for it, so a sampler sees the CPU of a child
/// that started and ended between two looks. On macOS an exec loses the children reaped before it
#[cfg_attr(
  not(any(target_os = "macos", target_os = "linux")),
  expect(clippy::missing_const_for_fn, reason = "const only where the system tells no CPU time")
)]
pub fn cpu_times(pid: u32) -> Option<CpuTimes> {
  #[cfg(any(target_os = "macos", target_os = "linux"))]
  return platform::cpu_times(pid);
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = pid;
    None
  }
}

/// Every process with its parent, its group and its start, without its working directory and its arguments, which take
/// most of the time of a full scan (`ProcessScan::scan`): what a sampler of the CPU of a process group reads often;
/// `None` where the system tells no list
pub fn process_list() -> Option<Vec<ProcessInfo>> {
  platform::process_tree()
}

/// The pids of the processes that have the file `path` open, each once: the file descriptors of each process through
/// libproc on macOS, the `fd` links of `/proc` on Linux (the processes of this user); `None` when that can't be told
/// (the file is gone, a sandbox, another platform)
pub fn processes_with_open_file(path: &Path) -> Option<Vec<u32>> {
  scan::processes_with_open_file(path)
}

/// The name of this host, as the system names it (`gethostname`): a record of a process on another host can't tell by
/// its pid whether it runs. Read once per process; empty when the system names none
pub fn host_name() -> &'static str {
  static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
  HOST.get_or_init(|| gethostname::gethostname().to_string_lossy().into_owned())
}

/// A process below another one (`descendants`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descendant {
  pub pid: u32,
  /// Its start (`process_started`)
  pub started: u64,
  /// The pid of its parent when the scan read it
  pub parent: u32,
}

/// The processes below `pid`, which started at `started`, each with its start and parent: the children by their parent
/// pid, their children, and so on, as one scan of the process list sees them, each after its parent. A process counts
/// as a child only when it started after its parent, so a pid that the system gave again to an older process ends no
/// chain. Not found: a process whose parent exited before the scan (the system gave it to launchd or init, or it
/// daemonized so), and a process forked after the scan (`process_tree` stops what it found and scans again until a pass
/// finds nothing new). The list without working directories and arguments (`process_tree` of the platform); `None`
/// when the system tells none or it could not be read (a churn of processes that no read kept up with), which a kill
/// takes for no scan, never for an empty tree
#[cfg(unix)]
pub fn descendants(pid: u32, started: u64) -> Option<Vec<Descendant>> {
  let processes = platform::process_tree()?;
  let mut found: Vec<Descendant> = Vec::new();
  let mut parents = vec![(pid, started)];
  while let Some((parent, parent_started)) = parents.pop() {
    for process in &processes {
      let Some(process_started) = process.started else { continue };
      if process.parent == parent && process.pid != parent && process_started >= parent_started && !found.iter().any(|seen| seen.pid == process.pid) {
        found.push(Descendant {
          pid: process.pid,
          started: process_started,
          parent,
        });
        parents.push((process.pid, process_started));
      }
    }
  }
  Some(found)
}

/// Interrupt the process `pid` with SIGINT, only while it still has the start time `started`, as `kill` signals. False
/// when it was not signalled (gone, another process now, another user's, no signals on this platform)
pub fn interrupt(pid: u32, started: u64) -> bool {
  is_running(pid, Some(started)) && signal(pid, Signal::Interrupt)
}

/// Ask the process `pid` to exit with SIGTERM; false when the signal could not be sent (gone, another user's, no
/// signals on this platform)
#[cfg_attr(
  not(any(target_os = "macos", target_os = "linux")),
  expect(clippy::missing_const_for_fn, reason = "const only where there are no signals")
)]
pub fn terminate(pid: u32) -> bool {
  signal(pid, Signal::Terminate)
}

/// Kill the process `pid` with SIGKILL, only while it still has the start time `started`: between a scan and the
/// signal the process can exit and the system can give its pid to an unrelated one. A start time is required, since
/// `is_running` without one counts any live process of the pid. False when it was not signalled (gone, another
/// process now, another user's, no signals on this platform)
pub fn kill(pid: u32, started: u64) -> bool {
  is_running(pid, Some(started)) && signal(pid, Signal::Kill)
}

/// Stop the process `pid` with SIGSTOP, only while it still has the start time `started`, as `kill` signals: a stopped
/// process starts no other one until SIGCONT or its SIGKILL. False when it was not stopped (gone, another process now,
/// another user's, no signals on this platform)
pub fn stop(pid: u32, started: u64) -> bool {
  is_running(pid, Some(started)) && signal(pid, Signal::Stop)
}

/// Whether the process `pid` starts no other process any more: each of its threads has stopped (`stop` took effect: a
/// thread in the middle of a call, a fork, runs on until the call returns, while the system may call the process
/// stopped at once), or it has exited. `None` where the system tells nothing of `pid` (gone, a sandbox)
#[cfg(unix)]
pub fn halted(pid: u32) -> Option<bool> {
  platform::halted(pid)
}

/// Whether the process that had `pid` and started at `started` has exited: it is gone, another process has the pid now,
/// or it is a zombie, which waits only for its parent to reap it and whose start neither platform tells (`is_running`).
/// Unlike `halted`, false for a stopped process, and on macOS false for one that is still exiting: libproc describes a
/// process no more from the start of its exit, while the kernel has yet to give its children to launchd and to count
/// its process group again, so a kill of its parent then (`process_tree`) made the group of the caller orphaned with a
/// stopped member, and macOS sent it SIGHUP (`platform::exited`)
#[cfg(unix)]
pub fn exited(pid: u32, started: u64) -> bool {
  #[cfg(target_os = "macos")]
  if let Some(exited) = platform::exited(pid, started) {
    return exited;
  }
  !is_running(pid, Some(started))
}

/// The parent of the process `pid` now, as the list of the platform tells it, for what a test says of a process that
/// outlived a kill; `None` where it lists none or `pid` is gone
#[cfg(test)]
pub(crate) fn parent_of(pid: u32) -> Option<u32> {
  platform::process_tree()?.into_iter().find(|process| process.pid == pid).map(|process| process.parent)
}

/// The signals of `signal`
#[derive(Clone, Copy)]
enum Signal {
  Interrupt,
  Terminate,
  Kill,
  Stop,
}

/// Whether the process that had `pid` and started at `started` still runs. A process whose start the system won't tell
/// has exited (a zombie: libproc of macOS describes none, measured on macOS 27, and the start of `/proc` is read only of
/// one that is no zombie, `platform::started`; on Windows one that a handle still keeps), unless it exists but is hidden from this
/// process (`runs_unseen`): where the system tells no process its start, not even this one's (a sandbox that denies the
/// process info, macOS), this process runs and another one while it still exists to the kernel (`kill(pid, 0)`), and
/// on Windows a process that won't open for this user (another user's, a protected one) runs. The start time then
/// can't tell a reused pid apart, but a gone process never counts as running. A platform that tells no start at all
/// counts every process as running: a lease is never taken from a process that may live
pub fn is_running(pid: u32, started: Option<u64>) -> bool {
  match (started, platform::started(pid)) {
    (Some(started), Some(now)) => now == started,
    (None, Some(_)) => true,
    (_, None) => platform::runs_unseen(pid),
  }
}

/// The questions only a scan answers: the platform's own on macOS and Linux, none elsewhere
#[cfg(any(target_os = "macos", target_os = "linux"))]
use platform as scan;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use unscanned as scan;

/// What only the scans of macOS and Linux tell, where a platform has none (Windows and the rest): no holders of a file
/// (no signals either: `signal`), and the wall clock of a start only on Windows, whose start is a time already
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unscanned {
  /// The 100 ns since 1601 of `GetProcessTimes`, on the wall clock
  #[cfg(windows)]
  pub(super) fn started_at(started: u64) -> Option<std::time::SystemTime> {
    // the seconds from 1601 to 1970
    const UNIX_EPOCH_SINCE_1601: u64 = 11_644_473_600;
    let since_1601 = std::time::Duration::from_nanos(started.checked_mul(100)?);
    std::time::UNIX_EPOCH.checked_add(since_1601.checked_sub(std::time::Duration::from_secs(UNIX_EPOCH_SINCE_1601))?)
  }

  #[cfg(not(windows))]
  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn started_at(_started: u64) -> Option<std::time::SystemTime> {
    None
  }

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn processes_with_open_file(_path: &std::path::Path) -> Option<Vec<u32>> {
    None
  }

  /// No signals here, so no moment to compare a start with
  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn start_clock() -> Option<super::StartClock> {
    None
  }
}

/// Whether the system tells this process not even its own start: a sandbox denies the process info
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn process_info_denied() -> bool {
  static DENIED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
  *DENIED.get_or_init(|| platform::started(std::process::id()).is_none())
}

/// Whether `pid` exists to the kernel, as `kill(pid, 0)` answers: it does, or it does but belongs to another user
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "kill(2) with the signal 0, which asks whether the process exists")]
fn unix_exists(pid: u32) -> bool {
  let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
  // SAFETY: kill(2) takes plain integers and touches no memory of this process; the signal 0 sends nothing
  unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// SIGTERM, SIGKILL or SIGSTOP through `kill(2)`
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[expect(unsafe_code, reason = "kill(2), which std offers only as SIGKILL of a child")]
fn signal(pid: u32, signal: Signal) -> bool {
  let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
  let signal = match signal {
    Signal::Interrupt => libc::SIGINT,
    Signal::Terminate => libc::SIGTERM,
    Signal::Kill => libc::SIGKILL,
    Signal::Stop => libc::SIGSTOP,
  };
  // SAFETY: kill(2) takes plain integers and touches no memory of this process
  unsafe { libc::kill(pid, signal) == 0 }
}

/// No signals elsewhere
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const fn signal(_pid: u32, _signal: Signal) -> bool {
  false
}

#[cfg(target_os = "macos")]
#[expect(
  unsafe_code,
  reason = "libproc: the pids, the parents, the start times, the threads and the working directories of the processes"
)]
mod platform {
  use super::ProcessInfo;
  use std::ffi::CStr;
  use std::mem::MaybeUninit;
  use std::os::raw::{c_char, c_int};
  use std::os::unix::ffi::OsStrExt;
  use std::path::PathBuf;

  pub(super) fn processes() -> Option<Vec<ProcessInfo>> {
    Some(
      process_tree()?
        .into_iter()
        .map(|process| ProcessInfo {
          cwd: cwd(process.pid),
          argv: argv(process.pid),
          ..process
        })
        .collect(),
    )
  }

  /// Every process with its parent, group, name and start, one `proc_pidinfo` each, without the working directories and
  /// the arguments of `processes`, which take most of its time: the list of a kill (`descendants`) and of the ancestors
  pub(super) fn process_tree() -> Option<Vec<ProcessInfo>> {
    Some(
      pids()?
        .into_iter()
        .filter_map(|pid| {
          let info = bsd_info(pid)?;
          Some(ProcessInfo {
            pid,
            parent: info.pbi_ppid,
            group: info.pbi_pgid,
            name: name(&info),
            cwd: None,
            argv: Vec::new(),
            started: Some(start_time(&info)),
            terminal: has_terminal(&info),
          })
        })
        .collect(),
    )
  }

  pub(super) fn started(pid: u32) -> Option<u64> {
    bsd_info(pid).map(|info| start_time(&info))
  }

  /// `e_tdev`, the device of the controlling terminal of its session, is one: `NODEV` (all bits set) when it has none
  const fn has_terminal(info: &libc::proc_bsdinfo) -> bool {
    info.e_tdev != u32::MAX && info.e_tdev != 0
  }

  /// `pti_total_user` and `pti_total_system` of `PROC_PIDTASKINFO`, in the ticks of the Mach clock, which
  /// `mach_timebase_info` turns into nanoseconds (1:1 on Intel, 125:3 on Apple silicon)
  pub(super) fn cpu_time(pid: u32) -> Option<std::time::Duration> {
    let task: libc::proc_taskinfo = pid_info(pid, libc::PROC_PIDTASKINFO)?;
    let ticks = u128::from(task.pti_total_user) + u128::from(task.pti_total_system);
    let (numer, denom) = timebase()?;
    u64::try_from(ticks * u128::from(numer) / u128::from(denom)).ok().map(std::time::Duration::from_nanos)
  }

  /// `ri_user_time` and `ri_system_time` of `proc_pid_rusage` and the two of the reaped children, in the ticks of the
  /// Mach clock as the task info
  #[expect(unsafe_code, reason = "proc_pid_rusage(3) of libproc, which std does not offer")]
  pub(super) fn cpu_times(pid: u32) -> Option<super::CpuTimes> {
    let mut info = MaybeUninit::<libc::rusage_info_v2>::zeroed();
    // SAFETY: the buffer is one `rusage_info_v2`, which the call writes for the flavor `RUSAGE_INFO_V2`
    let read = unsafe { libc::proc_pid_rusage(c_int::try_from(pid).ok()?, libc::RUSAGE_INFO_V2, info.as_mut_ptr().cast()) };
    if read != 0 {
      return None;
    }
    // SAFETY: the call succeeded and filled it; every bit pattern is valid for the plain C struct
    let info = unsafe { info.assume_init() };
    let (numer, denom) = timebase()?;
    let time = |ticks: u64| {
      u64::try_from(u128::from(ticks) * u128::from(numer) / u128::from(denom))
        .ok()
        .map(std::time::Duration::from_nanos)
    };
    Some(super::CpuTimes {
      own: time(info.ri_user_time.saturating_add(info.ri_system_time))?,
      children: time(info.ri_child_user_time.saturating_add(info.ri_child_system_time))?,
    })
  }

  /// The ratio of the Mach clock to nanoseconds, read once
  #[expect(unsafe_code, reason = "mach_timebase_info(3), which std does not offer")]
  #[expect(deprecated, reason = "libc points to the mach2 crate for mach_timebase_info, which this crate does not depend on")]
  fn timebase() -> Option<(u32, u32)> {
    static TIMEBASE: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();
    *TIMEBASE.get_or_init(|| {
      let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
      // SAFETY: the call writes the two fields of `info`, which lives across it
      let status = unsafe { libc::mach_timebase_info(&raw mut info) };
      (status == 0 && info.denom != 0).then_some((info.numer, info.denom))
    })
  }

  /// `PROC_PIDLISTTHREADS` of `sys/proc_info.h`: the flavor of `proc_pidinfo` that lists the thread handles of a process
  const PROC_PIDLISTTHREADS: c_int = 6;

  /// Stopped (`SSTOP`) with each thread suspended, asleep in an interruptible wait (it suspends as its call returns) or
  /// ending. The signal sets `SSTOP` at once, while a thread runs on to the end of its call: a kill read a git that had
  /// `SSTOP` and its thread running in a fork, and missed the ssh that the fork started. `None` for a zombie, which
  /// libproc does not describe, as for a gone process
  pub(super) fn halted(pid: u32) -> Option<bool> {
    let info = bsd_info(pid)?;
    if info.pbi_status != libc::SSTOP {
      return Some(false);
    }
    let stopped = [libc::TH_STATE_STOPPED, libc::TH_STATE_WAITING, libc::TH_STATE_HALTED];
    // a thread that ended since the list tells nothing and starts nothing
    Some(
      threads(pid)?
        .into_iter()
        .all(|thread| thread_info(pid, thread).is_none_or(|info| stopped.contains(&info.pth_run_state))),
    )
  }

  /// The thread handles of the process `pid` (`PROC_PIDLISTTHREADS`); `None` when it tells none
  fn threads(pid: u32) -> Option<Vec<u64>> {
    let task: libc::proc_taskinfo = pid_info(pid, libc::PROC_PIDTASKINFO)?;
    // room for the threads started since the count
    let mut threads = vec![0u64; usize::try_from(task.pti_threadnum).ok()? + 16];
    let size = c_int::try_from(threads.len() * size_of::<u64>()).ok()?;
    // SAFETY: the buffer holds `size` bytes of handles; the call writes at most that many and returns how many bytes
    let written = unsafe { libc::proc_pidinfo(c_int::try_from(pid).ok()?, PROC_PIDLISTTHREADS, 0, threads.as_mut_ptr().cast(), size) };
    threads.truncate(usize::try_from(written).ok().filter(|written| *written > 0)? / size_of::<u64>());
    Some(threads)
  }

  /// The info of the thread `thread` of the process `pid` (`PROC_PIDTHREADINFO`)
  fn thread_info(pid: u32, thread: u64) -> Option<libc::proc_threadinfo> {
    pid_info_of(pid, libc::PROC_PIDTHREADINFO, thread)
  }

  /// This process, or one that still exists, where libproc gives no process its info (a sandbox denies it)
  pub(super) fn runs_unseen(pid: u32) -> bool {
    super::process_info_denied() && (pid == std::process::id() || super::unix_exists(pid))
  }

  /// `started` is microseconds since the epoch (`start_time`)
  pub(super) fn started_at(started: u64) -> Option<std::time::SystemTime> {
    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_micros(started))
  }

  /// Now in microseconds since the epoch, the unit of `start_time`
  pub(super) fn start_clock() -> Option<super::StartClock> {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(super::StartClock {
      now: u64::try_from(since.as_micros()).ok()?,
      per_second: 1_000_000,
    })
  }

  /// `struct proc_fileinfo` of `sys/proc_info.h`, which libc lacks
  #[repr(C)]
  struct FileInfo {
    open_flags: u32,
    status: u32,
    offset: libc::off_t,
    kind: i32,
    guard_flags: u32,
  }

  /// `struct vnode_fdinfo` of `sys/proc_info.h`: the file of a descriptor and its vnode
  #[repr(C)]
  struct VnodeFdInfo {
    file: FileInfo,
    vnode: libc::vnode_info,
  }

  /// `PROC_PIDFDVNODEINFO` of `sys/proc_info.h`: the flavor of `proc_pidfdinfo` that answers a `VnodeFdInfo`
  const PROC_PIDFDVNODEINFO: c_int = 1;

  /// The processes with a descriptor of the file at `path` (its device and inode): one libproc read per process and
  /// per vnode descriptor, in this process (35 ms for 1000 processes; a spawned `lsof` took 160 ms, and on a loaded
  /// machine more than its limit of 10 s). A process of another user, or one that exited meanwhile, tells nothing and
  /// is left out, as the `/proc` of Linux leaves it out. `None` when the file is gone, this process may read no process
  /// info (a sandbox), or the kernel describes a vnode in a larger struct than this build knows: "nobody" would be a
  /// guess then, and the index lock removes a lock that nobody has open
  pub(super) fn processes_with_open_file(path: &std::path::Path) -> Option<Vec<u32>> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path).ok()?;
    if super::process_info_denied() {
      return None;
    }
    let mut holders = Vec::new();
    for pid in pids()? {
      for fd in vnode_fds(pid) {
        match vnode_of(pid, fd) {
          Vnode::Of(vnode) if u64::from(vnode.vi_stat.vst_dev) == metadata.dev() && vnode.vi_stat.vst_ino == metadata.ino() => {
            holders.push(pid);
            break;
          }
          Vnode::Of(_) | Vnode::Gone => {}
          Vnode::Unknown => return None,
        }
      }
    }
    Some(holders)
  }

  /// The descriptors of `pid` that are vnodes (files, directories); none when they can't be read
  fn vnode_fds(pid: u32) -> Vec<c_int> {
    let Ok(pid) = c_int::try_from(pid) else { return Vec::new() };
    // SAFETY: a null buffer of size 0 asks only for the size of the list
    let size = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    let Ok(size) = usize::try_from(size) else { return Vec::new() };
    // room for the descriptors opened since the size
    let mut fds: Vec<libc::proc_fdinfo> = Vec::with_capacity(size / size_of::<libc::proc_fdinfo>() + 16);
    let Ok(capacity) = c_int::try_from(fds.capacity() * size_of::<libc::proc_fdinfo>()) else {
      return Vec::new();
    };
    // SAFETY: the buffer has room for `capacity` bytes, which the call writes at most; it returns how many it wrote
    let written = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fds.as_mut_ptr().cast(), capacity) };
    let Ok(written) = usize::try_from(written) else { return Vec::new() };
    // SAFETY: the call wrote `written` bytes of whole `proc_fdinfo` structs, plain integers, within the capacity
    unsafe { fds.set_len((written / size_of::<libc::proc_fdinfo>()).min(fds.capacity())) };
    fds
      .into_iter()
      .filter(|fd| c_int::try_from(fd.proc_fdtype) == Ok(libc::PROX_FDTYPE_VNODE))
      .map(|fd| fd.proc_fd)
      .collect()
  }

  /// What `proc_pidfdinfo` tells of a vnode descriptor
  enum Vnode {
    Of(libc::vnode_info),
    /// The descriptor closed, or the process exited, meanwhile
    Gone,
    /// The kernel describes a vnode in a larger struct than this build knows (`ENOMEM`, or a size of its own)
    Unknown,
  }

  /// The vnode of the descriptor `fd` of `pid`
  fn vnode_of(pid: u32, fd: c_int) -> Vnode {
    let (Ok(pid), Ok(size)) = (c_int::try_from(pid), c_int::try_from(size_of::<VnodeFdInfo>())) else {
      return Vnode::Gone;
    };
    let mut info = MaybeUninit::<VnodeFdInfo>::zeroed();
    // SAFETY: the buffer is a `VnodeFdInfo` of `size` bytes, which the call writes at most
    let written = unsafe { libc::proc_pidfdinfo(pid, fd, PROC_PIDFDVNODEINFO, info.as_mut_ptr().cast(), size) };
    if written == size {
      // SAFETY: the call filled all of it, and every bit pattern is valid for these plain C structs
      return Vnode::Of(unsafe { info.assume_init() }.vnode);
    }
    if written > 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOMEM) {
      Vnode::Unknown
    } else {
      Vnode::Gone
    }
  }

  const fn start_time(info: &libc::proc_bsdinfo) -> u64 {
    info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec
  }

  /// The size of `struct kinfo_proc` of `sys/sysctl.h` on 64-bit macOS, which libc does not declare: its `kp_proc`
  /// (`struct extern_proc` of `sys/proc.h`) begins with the start of the process (`p_starttime`, a `timeval`) and has
  /// its state (`p_stat`) at byte 36
  const KINFO_PROC_SIZE: usize = 648;
  const P_STAT_OFFSET: usize = 36;

  /// Whether the process that had `pid` and started at `started` has exited, from `sysctl(KERN_PROC_PID)`, which also
  /// lists a process in its exit and a zombie: a zombie (`SZOMB`) or a process of another start has, one in its exit has
  /// not. libproc describes neither (`bsd_info`): it ends at the start of the exit, before the kernel has given the
  /// children of the process to launchd, a window that a probe of 300 kills of a child of a stopped parent saw in 296.
  /// `None` where the call fails or lists no process (gone, or hidden by a sandbox): the caller asks libproc then
  pub(super) fn exited(pid: u32, started: u64) -> Option<bool> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, c_int::try_from(pid).ok()?];
    let mut info = [0u64; KINFO_PROC_SIZE / size_of::<u64>()];
    let mut size: libc::size_t = KINFO_PROC_SIZE;
    // SAFETY: the buffer holds `size` bytes, aligned for the struct, which the call writes at most, then the size it wrote
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 4, info.as_mut_ptr().cast(), &raw mut size, std::ptr::null_mut(), 0) } != 0 || size != KINFO_PROC_SIZE {
      return None;
    }
    let bytes: Vec<u8> = info.iter().flat_map(|word| word.to_ne_bytes()).collect();
    let seconds = u64::from_ne_bytes(bytes[0..8].try_into().ok()?);
    let micros = u64::from(u32::from_ne_bytes(bytes[8..12].try_into().ok()?));
    let zombie = u32::from(bytes[P_STAT_OFFSET]) == libc::SZOMB;
    Some(zombie || seconds * 1_000_000 + micros != started)
  }

  /// The arguments of `pid` from `sysctl(KERN_PROCARGS2)`: `argc`, the executable path, padding, then `argc` strings,
  /// each ending with a NUL (the environment follows, never read)
  fn argv(pid: u32) -> Vec<String> {
    let Ok(pid) = c_int::try_from(pid) else { return Vec::new() };
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    // SAFETY: a null buffer asks for the size, which the call writes to `size`
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &raw mut size, std::ptr::null_mut(), 0) } != 0 {
      return Vec::new();
    }
    let mut buffer = vec![0u8; size];
    // SAFETY: the buffer holds `size` bytes, which the call writes at most, then the size it wrote
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buffer.as_mut_ptr().cast(), &raw mut size, std::ptr::null_mut(), 0) } != 0 {
      return Vec::new();
    }
    buffer.truncate(size);
    parse_procargs(&buffer)
  }

  /// The arguments in a `KERN_PROCARGS2` buffer: a native `int` argc, the executable path and its NUL padding, then
  /// `argc` strings that end with a NUL
  pub(super) fn parse_procargs(buffer: &[u8]) -> Vec<String> {
    let Some((count, rest)) = buffer.split_first_chunk::<4>() else { return Vec::new() };
    let count = usize::try_from(i32::from_ne_bytes(*count)).unwrap_or(0);
    let Some(executable_end) = rest.iter().position(|&byte| byte == 0) else {
      return Vec::new();
    };
    let args = &rest[executable_end..];
    let start = args.iter().position(|&byte| byte != 0).unwrap_or(args.len());
    args[start..]
      .split(|&byte| byte == 0)
      .take(count)
      .map(|arg| String::from_utf8_lossy(arg).into_owned())
      .collect()
  }

  /// The pids of every process, newest first, then the zombies, as `proc_listallpids` lists them. A list that fills its
  /// buffer may lack the oldest processes and the zombies (under a churn of processes the count read first fell short
  /// by more than its room): it is read again into twice the room, until one does not fill it
  fn pids() -> Option<Vec<u32>> {
    // SAFETY: a null buffer of size 0 asks only for the number of pids
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let mut room = usize::try_from(count).ok().filter(|count| *count > 0)? + PID_ROOM;
    for _ in 0..PID_READS {
      let mut pids: Vec<libc::pid_t> = vec![0; room];
      let size = c_int::try_from(pids.len() * size_of::<libc::pid_t>()).ok()?;
      // SAFETY: the buffer holds `size` bytes of pids; the call writes at most that many and returns how many it wrote
      let written = usize::try_from(unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) }).ok()?;
      if written < room {
        pids.truncate(written);
        return Some(pids.into_iter().filter_map(|pid| u32::try_from(pid).ok()).filter(|pid| *pid != 0).collect());
      }
      room *= 2;
    }
    None
  }

  /// The room for the processes started between the count and the list
  const PID_ROOM: usize = 64;
  /// The reads of the list, each into twice the room of the one before: a churn no read keeps up with tells no list
  const PID_READS: usize = 8;

  /// `proc_pidinfo` of `flavor` into a zeroed `T`; `None` unless it filled the whole struct
  fn pid_info<T>(pid: u32, flavor: c_int) -> Option<T> {
    pid_info_of(pid, flavor, 0)
  }

  /// `pid_info` of the item `arg` of the flavor (a thread)
  fn pid_info_of<T>(pid: u32, flavor: c_int, arg: u64) -> Option<T> {
    let mut info = MaybeUninit::<T>::zeroed();
    let size = c_int::try_from(size_of::<T>()).ok()?;
    // SAFETY: the buffer is a `T` of `size` bytes, which the call writes at most
    let written = unsafe { libc::proc_pidinfo(c_int::try_from(pid).ok()?, flavor, arg, info.as_mut_ptr().cast(), size) };
    // SAFETY: the call filled all of it, and every bit pattern is valid for these plain C structs
    (written == size).then(|| unsafe { info.assume_init() })
  }

  fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
    pid_info(pid, libc::PROC_PIDTBSDINFO)
  }

  fn cwd(pid: u32) -> Option<PathBuf> {
    let info: libc::proc_vnodepathinfo = pid_info(pid, libc::PROC_PIDVNODEPATHINFO)?;
    let path: Vec<u8> = info.pvi_cdir.vip_path.iter().flatten().map(|&c| c.cast_unsigned()).take_while(|&byte| byte != 0).collect();
    (!path.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(&path)))
  }

  fn name(info: &libc::proc_bsdinfo) -> String {
    let text = |chars: &[c_char]| {
      let bytes: Vec<u8> = chars.iter().map(|&c| c.cast_unsigned()).take_while(|&byte| byte != 0).chain(std::iter::once(0)).collect();
      CStr::from_bytes_with_nul(&bytes).map_or_default(|name| name.to_string_lossy().into_owned())
    };
    let name = text(&info.pbi_name);
    if name.is_empty() { text(&info.pbi_comm) } else { name }
  }
}

#[cfg(target_os = "linux")]
mod platform {
  use super::ProcessInfo;
  use std::path::Path;

  /// The pids of `/proc`
  fn pids() -> Option<impl Iterator<Item = u32>> {
    let entries = std::fs::read_dir("/proc").ok()?;
    Some(entries.flatten().filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok()))
  }

  pub(super) fn processes() -> Option<Vec<ProcessInfo>> {
    Some(
      process_tree()?
        .into_iter()
        .map(|process| {
          let dir = Path::new("/proc").join(process.pid.to_string());
          ProcessInfo {
            cwd: std::fs::read_link(dir.join("cwd")).ok(),
            argv: std::fs::read(dir.join("cmdline")).map_or_else(
              |_| Vec::new(),
              |cmdline| {
                cmdline
                  .split(|&byte| byte == 0)
                  .filter(|arg| !arg.is_empty())
                  .map(|arg| String::from_utf8_lossy(arg).into_owned())
                  .collect()
              },
            ),
            ..process
          }
        })
        .collect(),
    )
  }

  /// Every process with its parent, group, name and start, from its `stat`, without the working directories and the
  /// arguments of `processes`: the list of a kill (`descendants`) and of the ancestors
  pub(super) fn process_tree() -> Option<Vec<ProcessInfo>> {
    Some(
      pids()?
        .filter_map(|pid| {
          let stat = Stat::read(pid)?;
          Some(ProcessInfo {
            pid,
            parent: stat.parent,
            group: stat.group,
            name: stat.name,
            cwd: None,
            argv: Vec::new(),
            started: Some(stat.started),
            terminal: stat.terminal,
          })
        })
        .collect(),
    )
  }

  /// A zombie has exited: it waits only for its parent to reap it
  pub(super) fn started(pid: u32) -> Option<u64> {
    Stat::read(pid).filter(|stat| !stat.zombie).map(|stat| stat.started)
  }

  /// `utime` and `stime` of `/proc/<pid>/stat`, in clock ticks
  pub(super) fn cpu_time(pid: u32) -> Option<std::time::Duration> {
    let stat = Stat::read(pid)?;
    let ticks = clock_ticks()?;
    Some(std::time::Duration::from_nanos((stat.cpu_ticks.saturating_mul(1_000_000_000)) / ticks))
  }

  pub(super) fn cpu_times(pid: u32) -> Option<super::CpuTimes> {
    let stat = Stat::read(pid)?;
    let ticks = clock_ticks()?;
    let time = |cpu: u64| std::time::Duration::from_nanos(cpu.saturating_mul(1_000_000_000) / ticks);
    Some(super::CpuTimes {
      own: time(stat.cpu_ticks),
      children: time(stat.children_ticks),
    })
  }

  /// Each thread stopped (`T`, or `t` under a tracer), a zombie (`Z`) or dead (`X`): `/proc/<pid>/stat` has the state of
  /// the main thread, and another one runs on to the end of its call (a fork)
  pub(super) fn halted(pid: u32) -> Option<bool> {
    let tasks = std::fs::read_dir(format!("/proc/{pid}/task")).ok()?;
    // a thread that ended since the list tells nothing and starts nothing
    Some(tasks.flatten().all(|task| Stat::at(&task.path().join("stat")).is_none_or(|stat| stat.halted)))
  }

  /// `started` is clock ticks since the boot: `btime` of `/proc/stat` (seconds since the epoch) plus the ticks
  pub(super) fn started_at(started: u64) -> Option<std::time::SystemTime> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let boot: u64 = stat.lines().find_map(|line| line.strip_prefix("btime "))?.trim().parse().ok()?;
    let ticks = clock_ticks()?;
    let since_boot = std::time::Duration::from_secs(started / ticks) + std::time::Duration::from_nanos((started % ticks) * 1_000_000_000 / ticks);
    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(boot))?.checked_add(since_boot)
  }

  /// Now in clock ticks since the boot, the unit of `starttime` of `/proc/<pid>/stat`: the first field of `/proc/uptime`,
  /// both on the clock of the boot with the time of a suspend, and neither rounded to the second of `btime`
  pub(super) fn start_clock() -> Option<super::StartClock> {
    let ticks = clock_ticks()?;
    let uptime = std::fs::read_to_string("/proc/uptime").ok()?;
    Some(super::StartClock {
      now: super::uptime_ticks(&uptime, ticks)?,
      per_second: ticks,
    })
  }

  /// `sysconf(_SC_CLK_TCK)`: the clock ticks per second of `/proc/<pid>/stat`
  #[expect(unsafe_code, reason = "sysconf(3), which std does not offer")]
  fn clock_ticks() -> Option<u64> {
    // SAFETY: sysconf takes a plain integer and touches no memory of this process
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    u64::try_from(ticks).ok().filter(|&ticks| ticks > 0)
  }

  /// The processes of this user with a `/proc/<pid>/fd/<n>` link to `path`
  pub(super) fn processes_with_open_file(path: &Path) -> Option<Vec<u32>> {
    let path = std::fs::canonicalize(path).ok()?;
    Some(
      pids()?
        .filter(|pid| {
          std::fs::read_dir(format!("/proc/{pid}/fd"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|target| target == path))
        })
        .collect(),
    )
  }

  /// `/proc` tells a zombie, which `kill(pid, 0)` would count, from a running process: what it doesn't show is gone,
  /// but this process runs where a sandbox hides `/proc` from it
  pub(super) fn runs_unseen(pid: u32) -> bool {
    super::process_info_denied() && pid == std::process::id()
  }

  /// The fields of `/proc/<pid>/stat` the scan needs
  struct Stat {
    name: String,
    parent: u32,
    /// Field 5, `pgrp`
    group: u32,
    /// Field 22, `starttime`: clock ticks since boot
    started: u64,
    /// Field 7, `tty_nr`, names a device: the process has a controlling terminal
    terminal: bool,
    /// Fields 14 and 15, `utime` and `stime`: clock ticks of CPU
    cpu_ticks: u64,
    /// Fields 16 and 17, `cutime` and `cstime`: clock ticks of CPU of the reaped children
    children_ticks: u64,
    /// Field 3, `state`, is `Z`
    zombie: bool,
    /// Field 3, `state`, is `T`, `t`, `Z` or `X`: the thread forks no more
    halted: bool,
  }

  impl Stat {
    fn read(pid: u32) -> Option<Self> {
      Self::at(Path::new(&format!("/proc/{pid}/stat")))
    }

    /// The `stat` file of a process, or of a thread in `/proc/<pid>/task/<tid>/stat`
    fn at(path: &Path) -> Option<Self> {
      let text = std::fs::read_to_string(path).ok()?;
      // `pid (comm) state ppid …`: the name may hold spaces and parentheses, so the fields start after the last `)`
      let (head, rest) = text.rsplit_once(')')?;
      let name = head.split_once('(')?.1.to_string();
      let fields: Vec<&str> = rest.split_whitespace().collect();
      // fields[0] is field 3, `state`
      Some(Self {
        name,
        parent: fields.get(1)?.parse().ok()?,
        group: fields.get(2)?.parse().ok()?,
        started: fields.get(19)?.parse().ok()?,
        terminal: fields.get(4)?.parse::<i64>().ok()? != 0,
        cpu_ticks: fields.get(11)?.parse::<u64>().ok()?.saturating_add(fields.get(12)?.parse::<u64>().ok()?),
        children_ticks: fields.get(13)?.parse::<u64>().ok()?.saturating_add(fields.get(14)?.parse::<u64>().ok()?),
        zombie: fields.first() == Some(&"Z"),
        halted: fields.first().is_some_and(|state| ["T", "t", "Z", "X"].contains(state)),
      })
    }
  }
}

#[cfg(windows)]
#[expect(
  unsafe_code,
  reason = "Win32: the start time of a process (OpenProcess, GetProcessTimes, GetExitCodeProcess) and the snapshot of the process list (CreateToolhelp32Snapshot)"
)]
mod platform {
  use super::ProcessInfo;
  use std::os::windows::io::{AsRawHandle, FromRawHandle, HandleOrInvalid, OwnedHandle};
  use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, FILETIME, STILL_ACTIVE};
  use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
  use windows_sys::Win32::System::Threading::{GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

  /// No scan of the working directories: reading another process's needs its memory
  pub(super) const fn processes() -> Option<Vec<ProcessInfo>> {
    None
  }

  /// Every process of the system with its parent and its exe (`.exe` cut), from one snapshot of the process list, which
  /// reads no memory of another process; no start times (`started` reads those of the links of a chain), working
  /// directories, arguments or groups. The parent is the pid the process was started by, which the system may have
  /// given to another process once that one exited (`super::chain`)
  pub(super) fn process_tree() -> Option<Vec<ProcessInfo>> {
    // SAFETY: plain arguments; the result is a handle nothing else owns, or `INVALID_HANDLE_VALUE`
    let snapshot = unsafe { HandleOrInvalid::from_raw_handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)) };
    let snapshot = OwnedHandle::try_from(snapshot).ok()?;
    let mut entry = PROCESSENTRY32W {
      dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).ok()?,
      ..PROCESSENTRY32W::default()
    };
    let mut processes = Vec::new();
    // SAFETY: the snapshot is open while `snapshot` lives, and the entry is a local whose `dwSize` names its size
    let mut read = unsafe { Process32FirstW(snapshot.as_raw_handle(), &raw mut entry) };
    while read != 0 {
      processes.push(ProcessInfo {
        pid: entry.th32ProcessID,
        parent: entry.th32ParentProcessID,
        group: 0,
        name: exe_name(&entry.szExeFile),
        cwd: None,
        argv: Vec::new(),
        started: None,
        terminal: false,
      });
      // SAFETY: as for the first entry
      read = unsafe { Process32NextW(snapshot.as_raw_handle(), &raw mut entry) };
    }
    Some(processes)
  }

  /// The exe of an entry up to its NUL, without its `.exe`, whatever its case
  fn exe_name(exe: &[u16]) -> String {
    let length = exe.iter().position(|&unit| unit == 0).unwrap_or(exe.len());
    let name = String::from_utf16_lossy(&exe[..length]);
    match name.len().checked_sub(4) {
      Some(stem) if name.is_char_boundary(stem) && name[stem..].eq_ignore_ascii_case(".exe") => name[..stem].to_string(),
      _ => name,
    }
  }

  /// The creation time of the process, while it runs: a process that exited keeps its pid as long as a handle to it is
  /// open, with its exit code in place of `STILL_ACTIVE`
  /// No CPU time: no run of brd-host samples one here
  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn cpu_time(_pid: u32) -> Option<std::time::Duration> {
    None
  }

  pub(super) fn started(pid: u32) -> Option<u64> {
    let process = open(pid).ok()?;
    let mut exit_code = 0;
    // SAFETY: the handle is open while `process` lives, and the code goes into a local
    if unsafe { GetExitCodeProcess(process.as_raw_handle(), &raw mut exit_code) } == 0 || exit_code != STILL_ACTIVE.cast_unsigned() {
      return None;
    }
    let (mut created, mut exited, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: the handle is open while `process` lives, and the four times go into locals
    let read = unsafe { GetProcessTimes(process.as_raw_handle(), &raw mut created, &raw mut exited, &raw mut kernel, &raw mut user) };
    (read != 0).then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
  }

  /// A process that won't open for this user (another user's, a protected one): it exists, its start can't be read
  pub(super) fn runs_unseen(pid: u32) -> bool {
    open(pid).is_err_and(|error| error.raw_os_error() == Some(ERROR_ACCESS_DENIED.cast_signed()))
  }

  /// The process `pid`, opened only to query it; an error when it is gone (`ERROR_INVALID_PARAMETER`) or won't open
  fn open(pid: u32) -> std::io::Result<OwnedHandle> {
    // SAFETY: plain arguments; a non-null result is a handle that nothing else owns
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
      return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the handle just opened, owned from here on and closed when dropped
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
  }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod platform {
  use super::ProcessInfo;

  pub(super) const fn processes() -> Option<Vec<ProcessInfo>> {
    None
  }

  pub(super) const fn process_tree() -> Option<Vec<ProcessInfo>> {
    None
  }

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn started(_pid: u32) -> Option<u64> {
    None
  }

  /// No start of any process: every one counts as running
  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn runs_unseen(_pid: u32) -> bool {
    true
  }

  pub(super) const fn halted(_pid: u32) -> Option<bool> {
    None
  }

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn cpu_time(_pid: u32) -> Option<std::time::Duration> {
    None
  }
}

#[cfg(test)]
#[path = "process_scan_test.rs"]
mod process_scan_test;
