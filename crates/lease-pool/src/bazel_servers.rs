//! The Bazel servers of a slot where no process scan tells them (Windows), and their stop there. A server writes its
//! pid into `server/server.pid.txt` of its output base, and the `bazel-out` link of its workspace leads into that
//! output base (`<output_base>/execroot/<name>/bazel-out`). The return, the get and the reclaim stop such a server
//! with `bazel shutdown`, the clean stop of Bazel (the server writes its state and drops its locks), as SIGTERM stops
//! a server the scan found on macOS and Linux (`lease::stop_servers`). A workspace whose link is missing or named
//! otherwise (another `--symlink_prefix`) shows no server, and its server keeps running. A `bazel shutdown` that can't
//! run (no `bazel` on the `PATH`) or fails says why, and the caller names it with the server it did not stop.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use sync_utils::child_process::{ChildProcessError, run_child, windowless_command};
use sync_utils::paths::{display_path, without_verbatim};
use sync_utils::process_scan::{ProcessInfo, process_started};
use tracing::debug;

/// The program of `bazel shutdown` when the caller names none: `bazel` of the `PATH` (Bazelisk mostly)
pub(crate) const BAZEL: &str = "bazel";

/// The extensions `cmd` tries for a bare name when the environment has no `PATHEXT`
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// The files that make a directory a Bazel workspace
const WORKSPACE_FILES: [&str; 3] = ["MODULE.bazel", "WORKSPACE.bazel", "WORKSPACE"];

/// How much later than the write of its pid file a server may seem to start: the start of a process is told to the
/// second on Linux. A process that started later still got the pid of a server that is gone
const PID_FILE_SLACK: Duration = Duration::from_secs(2);

/// The Bazel servers of the workspaces in `slot` that run, told by the pid file of their output base, which the
/// `bazel-out` link of the workspace leads to: where no process scan is (Windows). A workspace is the slot or a
/// directory right in it (`community/`) with `MODULE.bazel`, `WORKSPACE.bazel` or `WORKSPACE`; its link is
/// `bazel-out`, or `out/bazel-out` with `--symlink_prefix=out/bazel-`. Each server is a `ProcessInfo` with the
/// `--workspace_directory=` and `--output_base=` a scan would read from its argv, once per output base
pub(crate) fn servers_by_output_base(slot: &Path) -> Vec<ProcessInfo> {
  let Ok(slot) = std::fs::canonicalize(slot) else { return Vec::new() };
  // a link of the slot leads out of it: only real directories are its workspaces
  let children: Vec<PathBuf> = std::fs::read_dir(&slot)
    .into_iter()
    .flatten()
    .flatten()
    .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
    .map(|entry| entry.path())
    .collect();
  let mut servers: Vec<ProcessInfo> = Vec::new();
  for workspace in std::iter::once(slot).chain(children) {
    if !WORKSPACE_FILES.iter().any(|file| workspace.join(file).is_file()) {
      continue;
    }
    for link in [workspace.join("bazel-out"), workspace.join("out").join("bazel-out")] {
      let Some(output_base) = output_base(&link) else { continue };
      let text = display_path(&output_base);
      if servers.iter().any(|server| server.output_base() == Some(text.as_str())) {
        continue;
      }
      if let Some(server) = server(&workspace, &output_base) {
        servers.push(server);
      }
    }
  }
  servers
}

/// The output base that the `bazel-out` link `link` leads to, canonical: the link (a symlink, a junction on Windows;
/// never a directory of that name) names `<output_base>/execroot/<name>/bazel-out`
fn output_base(link: &Path) -> Option<PathBuf> {
  if !std::fs::symlink_metadata(link).ok()?.file_type().is_symlink() {
    return None;
  }
  let target = std::fs::read_link(link).ok()?;
  let target = if target.is_relative() { link.parent()?.join(target) } else { target };
  let execroot = target.parent()?.parent()?;
  if target.file_name()? != "bazel-out" || execroot.file_name()? != "execroot" {
    debug!(link = %link.display(), target = %target.display(), "a bazel-out link that leads to no output base");
    return None;
  }
  std::fs::canonicalize(execroot.parent()?).ok()
}

/// The server of `output_base`, of the workspace `workspace`, while it runs: the pid of its `server/server.pid.txt` is a
/// process that started before the server wrote the file. A pid file a killed server left names a pid that is gone, or
/// one the system gave to a later process
fn server(workspace: &Path, output_base: &Path) -> Option<ProcessInfo> {
  let pid_file = output_base.join("server").join("server.pid.txt");
  let pid = std::fs::read_to_string(&pid_file).ok()?.trim().parse::<u32>().ok().filter(|&pid| pid != 0)?;
  let started = process_started(pid)?;
  let written: SystemTime = std::fs::metadata(&pid_file).and_then(|metadata| metadata.modified()).ok()?;
  let server = ProcessInfo {
    pid,
    parent: 0,
    group: 0,
    name: BAZEL.to_string(),
    cwd: None,
    argv: vec![
      BAZEL.to_string(),
      format!("--workspace_directory={}", display_path(workspace)),
      format!("--output_base={}", display_path(output_base)),
    ],
    started: Some(started),
  };
  if server.started_at().is_none_or(|started_at| started_at > written + PID_FILE_SLACK) {
    debug!(pid, pid_file = %pid_file.display(), "the pid of a Bazel pid file is a process that started later");
    return None;
  }
  Some(server)
}

/// `bazel --output_base=<its output base> shutdown` in the workspace of `server` (whose `.bazelrc` gives the startup
/// options the server has), through `program` (`bazel` when `None`, `resolve_program`), killed after `timeout`: `Ok`
/// when it exited with 0, else why it could not run or failed, naming the program. The pid is never signalled: the
/// client asks the server of that output base to exit, and the caller waits for the pid
pub(crate) fn shutdown(program: Option<&OsStr>, server: &ProcessInfo, timeout: Duration) -> Result<(), String> {
  let (Some(workspace), Some(output_base)) = (server.bazel_workspace(), server.output_base()) else {
    return Err("the server names no workspace or output base".to_string());
  };
  let program = resolve_program(program)?;
  let mut command = windowless_command(&program);
  command
    .arg(format!("--output_base={output_base}"))
    .arg("shutdown")
    .current_dir(without_verbatim(&workspace));
  let shown = format!("`{} shutdown`", program.display());
  match run_child(&mut command, None, timeout, &|| false) {
    Ok(output) if output.status.success() => Ok(()),
    Ok(output) => Err(format!("{shown} failed: {}", output.failure_message(&shown))),
    Err(ChildProcessError::NotFound) => Err(format!("cannot run {shown}: the program is not found")),
    Err(ChildProcessError::Spawn(error) | ChildProcessError::Wait(error)) => Err(format!("cannot run {shown}: {error}")),
    Err(ChildProcessError::TimedOut | ChildProcessError::Superseded) => Err(format!("{shown} did not finish in time and was killed")),
  }
}

/// The program to run for `program` (`bazel` when `None`): a bare name without an extension is looked up in the `PATH`
/// as `cmd` looks it up, on Windows with each extension of `PATHEXT` (`bazel.exe`, a `bazel.cmd` of an npm Bazelisk, a
/// `bazel.bat` of a wrapper), of which `CreateProcess` tries `.exe` only; a path or a name with an extension runs as it
/// is. A bare name that is nowhere is the error
fn resolve_program(program: Option<&OsStr>) -> Result<PathBuf, String> {
  let program = Path::new(program.unwrap_or_else(|| OsStr::new(BAZEL)));
  if program.components().count() != 1 || program.extension().is_some() {
    return Ok(program.to_path_buf());
  }
  let path = std::env::var_os("PATH").unwrap_or_default();
  let extensions = if cfg!(windows) {
    std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_string())
  } else {
    String::new()
  };
  find_program(program.as_os_str(), &path, &extensions).ok_or_else(|| {
    let tried = if extensions.is_empty() {
      String::new()
    } else {
      format!(" with the extensions {extensions}")
    };
    format!("cannot run `{} shutdown`: no {} on the PATH{tried}", program.display(), program.display())
  })
}

/// The first file `<dir>/<name><extension>`, by the directories of `path` and in each by the `;`-separated extensions
/// `extensions` (`PATHEXT`, in lower case; `name` alone when empty)
pub(crate) fn find_program(name: &OsStr, path: &OsStr, extensions: &str) -> Option<PathBuf> {
  let extensions: Vec<String> = if extensions.is_empty() {
    vec![String::new()]
  } else {
    extensions.split(';').filter(|extension| !extension.is_empty()).map(str::to_ascii_lowercase).collect()
  };
  std::env::split_paths(path).filter(|dir| !dir.as_os_str().is_empty()).find_map(|dir| {
    extensions
      .iter()
      .map(|extension| {
        let mut file = OsString::from(name);
        file.push(extension);
        dir.join(file)
      })
      .find(|candidate| candidate.is_file())
  })
}

#[cfg(test)]
#[path = "bazel_servers_test.rs"]
mod bazel_servers_test;
