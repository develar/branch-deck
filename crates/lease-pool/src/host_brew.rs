//! The process of Homebrew of `host_slots::ensure_host_git` (`host_git`): a file of its own, as the git scan takes every
//! process a file starts beside `GitInfo` for a git (`git_usage_scan_test.rs`)

use anyhow::{Context, Result, bail};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use sync_utils::child_process::windowless_command;

/// `brew <args>` in `/`, its output in a private temporary file, killed at `deadline`: whether it succeeded, and what it
/// said. Each run of Homebrew updates its formulae first, so the upgrade sees the newest git; no cleanup, so a run that
/// still uses the old git keeps the helpers of its `libexec/git-core`
pub(crate) fn run_with_deadline(brew: &Path, args: &[&str], path: &OsStr, deadline: Duration) -> Result<(bool, String)> {
  let output = tempfile::tempfile().context("Cannot make a file for the output of Homebrew")?;
  let mut child = windowless_command(brew)
    .args(args)
    .current_dir("/")
    .env("PATH", path)
    .env("HOMEBREW_AUTO_UPDATE_SECS", "0")
    .env("HOMEBREW_NO_INSTALL_CLEANUP", "1")
    .stdin(Stdio::null())
    .stdout(output.try_clone()?)
    .stderr(output.try_clone()?)
    .spawn()
    .with_context(|| format!("Cannot start {}", brew.display()))?;
  let started = Instant::now();
  let status = loop {
    if let Some(status) = child.try_wait()? {
      break status;
    }
    if started.elapsed() >= deadline {
      let _ = child.kill();
      let _ = child.wait();
      bail!("Homebrew did not end within {} min", deadline.as_secs() / 60);
    }
    std::thread::sleep(Duration::from_millis(100));
  };
  let mut said = Vec::new();
  let mut output = output;
  std::io::Seek::rewind(&mut output)?;
  std::io::Read::read_to_end(&mut output, &mut said).context("Cannot read the output of Homebrew")?;
  Ok((status.success(), format!("{} ({status})", String::from_utf8_lossy(&said).trim())))
}
