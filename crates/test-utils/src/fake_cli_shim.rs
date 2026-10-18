//! The program of a fake CLI on Windows (`test_utils::fake_executable::fake_executable`). Windows runs no `#!` script,
//! so a fake there is `<name>.exe`, a copy of this program, beside its script `<name>.sh`: it runs the script with the
//! sh named on the script's first line (`#!C:\Program Files\Git\usr\bin\sh.exe`), with the same stdin, stdout, stderr
//! and environment, and exits with its code. The directory of that sh (`usr\bin`, with `cat`, `sleep` and the other
//! tools of a script) comes first on `PATH`, as in the programs git runs: the launcher of Git for Windows puts it there
//! for git, while a test may run with only `Git\cmd` on `PATH`.
//!
//! The arguments go in the environment, `FAKE_CLI_ARGC` and `FAKE_CLI_ARG_<n>` from 1, and the second line of the
//! script sets them back as `"$@"`: the sh of Git for Windows splits its Windows command line by rules of its own (it
//! globs, expands braces and unquotes), so a JSON argument or an empty one would not reach the script as given.
//!
//! Built by `build.rs` for a Windows target only, with std only: a test binary of another crate can't ask cargo for a
//! binary of this package.

use std::path::Path;
use std::process::Command;

fn main() {
  let exe = std::env::current_exe().expect("the path of the fake");
  let script = exe.with_extension("sh");
  let text = std::fs::read_to_string(&script).unwrap_or_else(|e| panic!("cannot read the script of the fake {}: {e}", script.display()));
  let sh = text
    .lines()
    .next()
    .and_then(|line| line.strip_prefix("#!"))
    .unwrap_or_else(|| panic!("the script of the fake {} names no sh on its first line", script.display()));

  #[cfg(windows)]
  job::end_with_this_process();

  let mut command = Command::new(sh);
  command.arg(sh_path(&script));
  if let Some(tools) = Path::new(sh).parent() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Ok(path) = std::env::join_paths(std::iter::once(tools.to_path_buf()).chain(std::env::split_paths(&path))) {
      command.env("PATH", path);
    }
  }
  let args: Vec<_> = std::env::args_os().skip(1).collect();
  command.env("FAKE_CLI_ARGC", args.len().to_string());
  for (index, arg) in args.iter().enumerate() {
    command.env(format!("FAKE_CLI_ARG_{}", index + 1), arg);
  }
  let status = command.status().unwrap_or_else(|e| panic!("cannot run {sh}: {e}"));
  std::process::exit(status.code().unwrap_or(1));
}

/// `path` as the script's `$0`: with `/` separators, as sh reads `\` as an escape (`dirname "$0"`), and without the
/// `\\?\` prefix of a long path, which sh does not know
fn sh_path(path: &Path) -> String {
  let path = path.to_string_lossy();
  path.strip_prefix(r"\\?\").unwrap_or(&path).replace('\\', "/")
}

/// A job object that ends every process of the fake with it. A caller that kills the fake after a timeout, or because
/// a newer request superseded it, kills only this process; without the job, its sh and the `sleep 30` of a hanging
/// fake would live on, holding the output pipes and the temp dir of the test.
#[cfg(windows)]
mod job {
  use std::ffi::c_void;

  type Handle = *mut c_void;

  /// `JobObjectExtendedLimitInformation` of `JOBOBJECTINFOCLASS`
  const EXTENDED_LIMIT_INFORMATION: i32 = 9;
  const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

  /// `JOBOBJECT_BASIC_LIMIT_INFORMATION`
  #[repr(C)]
  #[derive(Default)]
  struct BasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
  }

  /// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`
  #[repr(C)]
  #[derive(Default)]
  struct ExtendedLimitInformation {
    basic_limit_information: BasicLimitInformation,
    /// `IO_COUNTERS`: six `ULONGLONG`s, the operation and transfer counts of reads, writes and others
    io_info: [u64; 6],
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
  }

  #[link(name = "kernel32")]
  unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, information: *const c_void, length: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn GetCurrentProcess() -> Handle;
  }

  /// Puts this process into a new job that kills its processes when its last handle closes: the handle is never
  /// closed and is not inheritable, so the job ends when this process does, by exit or by a kill. The children of the
  /// fake (sh, and what sh runs) are in the job too, as the job allows no breakaway. Best effort: a fake that can't
  /// join a job still runs.
  pub(super) fn end_with_this_process() {
    let mut information = ExtendedLimitInformation::default();
    information.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let length = u32::try_from(size_of::<ExtendedLimitInformation>()).expect("the size of a job limit");
    // SAFETY: kernel32 calls with null optional pointers, a struct of the layout and length the class names that
    // outlives the call, and the pseudo handle of this process; the job handle stays open for the life of the process
    unsafe {
      let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
      if !job.is_null() && SetInformationJobObject(job, EXTENDED_LIMIT_INFORMATION, (&raw const information).cast(), length) != 0 {
        AssignProcessToJobObject(job, GetCurrentProcess());
      }
    }
  }
}
