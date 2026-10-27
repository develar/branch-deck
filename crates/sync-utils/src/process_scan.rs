//! The processes whose working directory is inside a directory, for the lease pool (`lease`): a slot with a process in
//! it is busy, a shell or a build daemon there would see its files change. One scan reads every process of the user
//! once: on macOS through libproc (`proc_listallpids`, `proc_pidinfo`), on Linux from `/proc`. Windows has no scan:
//! `ProcessScan::Unavailable`, and a lease never waits for it; it tells the start of a process by its pid
//! (`process_started`), as the other two do. The scan leaves out the session that runs `bd`: this
//! process, its ancestors and every process of their process groups (the `jq` of `bd lease return | jq`, the
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
  /// time `/proc/stat` gives to the second; `None` elsewhere and when unknown
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
/// none of them (`ProcessScan::session`)
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
  "env",
  "nohup",
  "timeout",
  "xargs",
  "time",
  "sudo",
  "bd",
  "branch-deck",
];

/// The processes of one scan
#[derive(Debug, Clone)]
pub enum ProcessScan {
  Done {
    processes: Vec<ProcessInfo>,
    /// The session that runs this process: it, its ancestors and the processes of their groups
    own: HashSet<u32>,
  },
  /// No scan on this platform: nothing is known to be inside a directory
  Unavailable,
}

impl ProcessScan {
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

  /// The process this one works for: the nearest ancestor that is no shell or wrapper (`RUNNERS`), the coding agent or
  /// the terminal; `None` without a scan or when the chain has none below the init process
  pub fn session(&self) -> Option<&ProcessInfo> {
    let Self::Done { processes, .. } = self else { return None };
    let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|process| (process.pid, process)).collect();
    let mut seen = HashSet::new();
    let mut current = by_pid.get(&std::process::id())?.parent;
    while current > 1 && seen.insert(current) {
      let process = by_pid.get(&current)?;
      if !RUNNERS.contains(&process.name.as_str()) {
        return Some(process);
      }
      current = process.parent;
    }
    None
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

/// When the process `pid` started, in a unit of the platform (microseconds on macOS, clock ticks since boot on Linux,
/// 100 ns since 1601 on Windows), so a pid that the system gave to another process since tells apart; `None` when
/// unknown (a gone process, one that exited, one the system won't describe to this process)
pub fn process_started(pid: u32) -> Option<u64> {
  platform::started(pid)
}

/// The pids of the processes that have the file `path` open: `lsof` on macOS, the `fd` links of `/proc` on Linux (the
/// processes of this user); `None` when that can't be told (no `lsof`, its failure, another platform)
pub fn processes_with_open_file(path: &Path) -> Option<Vec<u32>> {
  scan::processes_with_open_file(path)
}

/// Ask the process `pid` to exit with SIGTERM; false when the signal could not be sent (gone, another user's, no
/// signals on this platform)
#[cfg_attr(
  not(any(target_os = "macos", target_os = "linux")),
  expect(clippy::missing_const_for_fn, reason = "const only where there are no signals")
)]
pub fn terminate(pid: u32) -> bool {
  signal(pid, false)
}

/// Kill the process `pid` with SIGKILL, only while it still has the start time `started`: between a scan and the
/// signal the process can exit and the system can give its pid to an unrelated one. A start time is required, since
/// `is_running` without one counts any live process of the pid. False when it was not signalled (gone, another
/// process now, another user's, no signals on this platform)
pub fn kill(pid: u32, started: u64) -> bool {
  is_running(pid, Some(started)) && signal(pid, true)
}

/// Whether the process that had `pid` and started at `started` still runs. A process whose start the system won't tell
/// has exited (a zombie on Linux; on Windows one that a handle still keeps), unless it exists but is hidden from this
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

/// What only the scans of macOS and Linux tell, where a platform has none (Windows and the rest): no wall clock of a
/// start without a `ProcessInfo`, no holders of a file (no signals either: `signal`)
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unscanned {
  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn started_at(_started: u64) -> Option<std::time::SystemTime> {
    None
  }

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn processes_with_open_file(_path: &std::path::Path) -> Option<Vec<u32>> {
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

/// SIGTERM, or with `kill` SIGKILL, through `kill(2)`
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[expect(unsafe_code, reason = "kill(2), which std offers only as SIGKILL of a child")]
fn signal(pid: u32, kill: bool) -> bool {
  let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
  let signal = if kill { libc::SIGKILL } else { libc::SIGTERM };
  // SAFETY: kill(2) takes plain integers and touches no memory of this process
  unsafe { libc::kill(pid, signal) == 0 }
}

/// No signals elsewhere
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const fn signal(_pid: u32, _kill: bool) -> bool {
  false
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "libproc: the pids, the parents, the start times and the working directories of the processes")]
mod platform {
  use super::ProcessInfo;
  use std::ffi::CStr;
  use std::mem::MaybeUninit;
  use std::os::raw::{c_char, c_int};
  use std::os::unix::ffi::OsStrExt;
  use std::path::PathBuf;

  pub(super) fn processes() -> Option<Vec<ProcessInfo>> {
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
            cwd: cwd(pid),
            argv: argv(pid),
            started: Some(start_time(&info)),
          })
        })
        .collect(),
    )
  }

  pub(super) fn started(pid: u32) -> Option<u64> {
    bsd_info(pid).map(|info| start_time(&info))
  }

  /// This process, or one that still exists, where libproc gives no process its info (a sandbox denies it)
  pub(super) fn runs_unseen(pid: u32) -> bool {
    super::process_info_denied() && (pid == std::process::id() || super::unix_exists(pid))
  }

  /// `started` is microseconds since the epoch (`start_time`)
  pub(super) fn started_at(started: u64) -> Option<std::time::SystemTime> {
    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_micros(started))
  }

  /// `lsof -t -w -- <path>`: the pids on exit 0; exit 1 without a message is no process
  pub(super) fn processes_with_open_file(path: &std::path::Path) -> Option<Vec<u32>> {
    let mut command = crate::child_process::windowless_command("/usr/sbin/lsof");
    command.args(["-t", "-w", "--"]).arg(path);
    let output = crate::child_process::run_child(&mut command, None, std::time::Duration::from_secs(10), &|| false).ok()?;
    match output.status.code() {
      Some(0) => Some(String::from_utf8_lossy(&output.stdout).lines().filter_map(|line| line.trim().parse().ok()).collect()),
      Some(1) if output.stdout.is_empty() && output.stderr.is_empty() => Some(Vec::new()),
      _ => None,
    }
  }

  const fn start_time(info: &libc::proc_bsdinfo) -> u64 {
    info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec
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

  fn pids() -> Option<Vec<u32>> {
    // SAFETY: a null buffer of size 0 asks only for the number of pids
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let count = usize::try_from(count).ok().filter(|count| *count > 0)?;
    // room for the processes started since the count
    let mut pids: Vec<libc::pid_t> = vec![0; count + 64];
    let size = c_int::try_from(pids.len() * size_of::<libc::pid_t>()).ok()?;
    // SAFETY: the buffer holds `size` bytes of pids; the call writes at most that many and returns how many pids it wrote
    let written = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
    pids.truncate(usize::try_from(written).ok()?);
    Some(pids.into_iter().filter_map(|pid| u32::try_from(pid).ok()).filter(|pid| *pid != 0).collect())
  }

  /// `proc_pidinfo` of `flavor` into a zeroed `T`; `None` unless it filled the whole struct
  fn pid_info<T>(pid: u32, flavor: c_int) -> Option<T> {
    let mut info = MaybeUninit::<T>::zeroed();
    let size = c_int::try_from(size_of::<T>()).ok()?;
    // SAFETY: the buffer is a `T` of `size` bytes, which the call writes at most
    let written = unsafe { libc::proc_pidinfo(c_int::try_from(pid).ok()?, flavor, 0, info.as_mut_ptr().cast(), size) };
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
      pids()?
        .filter_map(|pid| {
          let stat = Stat::read(pid)?;
          let dir = Path::new("/proc").join(pid.to_string());
          Some(ProcessInfo {
            pid,
            parent: stat.parent,
            group: stat.group,
            name: stat.name,
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
            started: Some(stat.started),
          })
        })
        .collect(),
    )
  }

  /// A zombie has exited: it waits only for its parent to reap it
  pub(super) fn started(pid: u32) -> Option<u64> {
    Stat::read(pid).filter(|stat| !stat.zombie).map(|stat| stat.started)
  }

  /// `started` is clock ticks since the boot: `btime` of `/proc/stat` (seconds since the epoch) plus the ticks
  pub(super) fn started_at(started: u64) -> Option<std::time::SystemTime> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let boot: u64 = stat.lines().find_map(|line| line.strip_prefix("btime "))?.trim().parse().ok()?;
    let ticks = clock_ticks()?;
    let since_boot = std::time::Duration::from_secs(started / ticks) + std::time::Duration::from_nanos((started % ticks) * 1_000_000_000 / ticks);
    std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(boot))?.checked_add(since_boot)
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
    /// Field 3, `state`, is `Z`
    zombie: bool,
  }

  impl Stat {
    fn read(pid: u32) -> Option<Self> {
      let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
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
        zombie: fields.first() == Some(&"Z"),
      })
    }
  }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32: the start time of a process (OpenProcess, GetProcessTimes, GetExitCodeProcess)")]
mod platform {
  use super::ProcessInfo;
  use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
  use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, FILETIME, STILL_ACTIVE};
  use windows_sys::Win32::System::Threading::{GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

  /// No scan of the working directories: reading another process's needs its memory
  pub(super) const fn processes() -> Option<Vec<ProcessInfo>> {
    None
  }

  /// The creation time of the process, while it runs: a process that exited keeps its pid as long as a handle to it is
  /// open, with its exit code in place of `STILL_ACTIVE`
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

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn started(_pid: u32) -> Option<u64> {
    None
  }

  /// No start of any process: every one counts as running
  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn runs_unseen(_pid: u32) -> bool {
    true
  }
}

#[cfg(test)]
#[path = "process_scan_test.rs"]
mod process_scan_test;
