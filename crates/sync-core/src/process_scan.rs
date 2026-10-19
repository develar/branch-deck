//! The processes whose working directory is inside a directory, for the lease pool (`lease`): a slot with a process in
//! it is busy, a shell or a build daemon there would see its files change. One scan reads every process of the user
//! once: on macOS through libproc (`proc_listallpids`, `proc_pidinfo`), on Linux from `/proc`. Windows has no scan:
//! `ProcessScan::Unavailable`, and a lease never waits for it. The scan leaves out this process and its ancestors (the
//! shell and the agent that run `bd`).

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

  /// The processes whose working directory is `dir` or below it, by path components, except this process and its
  /// ancestors; `dir` is compared canonical, as the kernel names a working directory
  pub fn inside(&self, dir: &Path) -> Vec<&ProcessInfo> {
    let Self::Done { processes, own } = self else { return Vec::new() };
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    processes
      .iter()
      .filter(|process| !own.contains(&process.pid) && process.cwd.as_ref().is_some_and(|cwd| cwd.starts_with(&dir)))
      .collect()
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

/// Whether the process that had `pid` and started at `started` still runs. Without a start time (Windows) it counts as
/// running: a lease is never taken from a process that may live
pub fn is_running(pid: u32, started: Option<u64>) -> bool {
  match started {
    Some(started) => platform::started(pid) == Some(started),
    None => !platform::SCAN_AVAILABLE || platform::started(pid).is_some(),
  }
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
          })
        })
        .collect(),
    )
  }

  pub(super) fn started(pid: u32) -> Option<u64> {
    let info = bsd_info(pid)?;
    Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
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
          Some(ProcessInfo {
            pid,
            parent: stat.parent,
            name: stat.name,
            cwd: std::fs::read_link(Path::new("/proc").join(pid.to_string()).join("cwd")).ok(),
          })
        })
        .collect(),
    )
  }

  pub(super) fn started(pid: u32) -> Option<u64> {
    Stat::read(pid).map(|stat| stat.started)
  }

  /// The fields of `/proc/<pid>/stat` the scan needs
  struct Stat {
    name: String,
    parent: u32,
    /// Field 22, `starttime`: clock ticks since boot
    started: u64,
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
}

#[cfg(test)]
#[path = "process_scan_test.rs"]
mod process_scan_test;
