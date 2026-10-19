//! The processes whose working directory is inside a directory, for the lease pool (`lease`): a slot with a process in
//! it is busy, a shell or a build daemon there would see its files change. One scan reads every process of the user
//! once: on macOS through libproc (`proc_listallpids`, `proc_pidinfo`), on Linux from `/proc`. Windows has no scan:
//! `ProcessScan::Unavailable`, and a lease never waits for it. The scan leaves out this process and its ancestors (the
//! shell and the agent that run `bd`). It reads the argv of each process too (`KERN_PROCARGS2` on macOS,
//! `/proc/<pid>/cmdline` on Linux): a Bazel server names its workspace there (`--workspace_directory=`), and serves
//! only that one, since Bazel derives its output base from the workspace path.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A process the scan read
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
  pub pid: u32,
  /// The parent's pid; 0 when unknown
  pub parent: u32,
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

  /// Whether the program is git: its argv starts with `git` or a path to it
  fn is_git(&self) -> bool {
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

  /// Whether the process works in `dir` (canonical): its working directory, or the workspace of a Bazel server, is
  /// `dir` or below it, by path components
  fn works_in(&self, dir: &Path) -> bool {
    self.cwd.as_ref().is_some_and(|cwd| cwd.starts_with(dir)) || self.bazel_workspace().is_some_and(|workspace| workspace.starts_with(dir))
  }
}

/// The processes of one scan
#[derive(Debug, Clone)]
pub enum ProcessScan {
  Done {
    processes: Vec<ProcessInfo>,
    /// This process and its ancestors
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

  /// The processes whose working directory, or the workspace of a Bazel server, is `dir` or below it, by path
  /// components, except this process and its ancestors; `dir` is compared canonical, as the kernel names a working
  /// directory
  pub fn inside(&self, dir: &Path) -> Vec<&ProcessInfo> {
    let Self::Done { processes, own } = self else { return Vec::new() };
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    processes.iter().filter(|process| !own.contains(&process.pid) && process.works_in(&dir)).collect()
  }
}

/// `pid` and the chain of its parents among `processes`
fn own_processes(processes: &[ProcessInfo], pid: u32) -> HashSet<u32> {
  let parents: HashMap<u32, u32> = processes.iter().map(|process| (process.pid, process.parent)).collect();
  let mut own = HashSet::new();
  let mut current = pid;
  while current != 0 && own.insert(current) {
    current = parents.get(&current).copied().unwrap_or(0);
  }
  own
}

/// When the process `pid` started, in a unit of the platform (microseconds on macOS, clock ticks since boot on Linux),
/// so a pid that the system gave to another process since tells apart; `None` when unknown (Windows, a gone process)
pub fn process_started(pid: u32) -> Option<u64> {
  platform::started(pid)
}

/// Ask the process `pid` to exit with SIGTERM; false when the signal could not be sent (gone, another user's, no
/// signals on this platform)
pub fn terminate(pid: u32) -> bool {
  platform::terminate(pid)
}

/// Whether the process that had `pid` and started at `started` still runs. Without a start time (Windows) it counts as
/// running: a lease is never taken from a process that may live
pub fn is_running(pid: u32, started: Option<u64>) -> bool {
  match started {
    Some(started) => platform::started(pid) == Some(started),
    None => !platform::SCAN_AVAILABLE || platform::started(pid).is_some(),
  }
}

/// SIGTERM through `kill(2)`
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[expect(unsafe_code, reason = "kill(2), which std offers only as SIGKILL of a child")]
fn unix_terminate(pid: u32) -> bool {
  let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
  // SAFETY: kill(2) takes plain integers and touches no memory of this process
  unsafe { libc::kill(pid, libc::SIGTERM) == 0 }
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

  pub(super) const SCAN_AVAILABLE: bool = true;

  pub(super) fn processes() -> Option<Vec<ProcessInfo>> {
    Some(
      pids()?
        .into_iter()
        .filter_map(|pid| {
          let info = bsd_info(pid)?;
          Some(ProcessInfo {
            pid,
            parent: info.pbi_ppid,
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

  const fn start_time(info: &libc::proc_bsdinfo) -> u64 {
    info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec
  }

  pub(super) fn terminate(pid: u32) -> bool {
    super::unix_terminate(pid)
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

  pub(super) const SCAN_AVAILABLE: bool = true;

  pub(super) fn processes() -> Option<Vec<ProcessInfo>> {
    let entries = std::fs::read_dir("/proc").ok()?;
    Some(
      entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(|pid| {
          let stat = Stat::read(pid)?;
          let dir = Path::new("/proc").join(pid.to_string());
          Some(ProcessInfo {
            pid,
            parent: stat.parent,
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

  pub(super) fn terminate(pid: u32) -> bool {
    super::unix_terminate(pid)
  }

  /// The fields of `/proc/<pid>/stat` the scan needs
  struct Stat {
    name: String,
    parent: u32,
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
        started: fields.get(19)?.parse().ok()?,
        zombie: fields.first() == Some(&"Z"),
      })
    }
  }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
  use super::ProcessInfo;

  pub(super) const SCAN_AVAILABLE: bool = false;

  pub(super) const fn processes() -> Option<Vec<ProcessInfo>> {
    None
  }

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn started(_pid: u32) -> Option<u64> {
    None
  }

  #[expect(clippy::missing_const_for_fn, reason = "the signature of the scans of macOS and Linux")]
  pub(super) fn terminate(_pid: u32) -> bool {
    false
  }
}

#[cfg(test)]
#[path = "process_scan_test.rs"]
mod process_scan_test;
