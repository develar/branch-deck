//! The git that talks to a remote with its progress (`--progress`, read by `transfer_progress`), under the policy of
//! `git_ops::remote_command`: the user's credential helpers without prompts, an abort of a stalled HTTP transfer, the C
//! locale (docs/design/git-operations.md, Key Decision 8).

use crate::transfer_progress::{TransferProgress, TransferProgressReporter};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::{GitCommandExecutor, GitOutput, ProgressStop};
use git_ops::remote_command::{REMOTE_ENV, remote_args};

/// `execute_remote_command_output` with `--progress` (added after the subcommand): `on_progress` gets how far the
/// transfer is as git reports it, throttled by `TransferProgressReporter`; stderr holds the messages without the
/// progress updates
pub fn execute_remote_command_with_progress(
  git_executor: &GitCommandExecutor,
  args: &[&str],
  repository_path: &str,
  ok_exit_codes: &[i32],
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<GitOutput> {
  execute_remote_command_with_progress_until(git_executor, args, repository_path, ok_exit_codes, on_progress, &ProgressStop::NEVER)
}

/// `execute_remote_command_with_progress` that git is killed for when `stop` says so: a stall or a cancel, the error
/// is then a `StoppedCommand`
pub fn execute_remote_command_with_progress_until(
  git_executor: &GitCommandExecutor,
  args: &[&str],
  repository_path: &str,
  ok_exit_codes: &[i32],
  on_progress: &mut dyn FnMut(TransferProgress),
  stop: &ProgressStop<'_>,
) -> Result<GitOutput> {
  with_progress(git_executor, &[], args, repository_path, ok_exit_codes, on_progress, stop)
}

/// `execute_remote_command_with_progress` with `config`, more `-c` pairs after the policy's: the push into the baseline
/// by its URL clears the URLs of its remote, so git takes no configured remote for the URL and moves none of its
/// remote-tracking refs (`baseline_push`)
pub fn execute_remote_command_with_progress_configured(
  git_executor: &GitCommandExecutor,
  config: &[&str],
  args: &[&str],
  repository_path: &str,
  ok_exit_codes: &[i32],
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<GitOutput> {
  with_progress(git_executor, config, args, repository_path, ok_exit_codes, on_progress, &ProgressStop::NEVER)
}

fn with_progress(
  git_executor: &GitCommandExecutor,
  config: &[&str],
  args: &[&str],
  repository_path: &str,
  ok_exit_codes: &[i32],
  on_progress: &mut dyn FnMut(TransferProgress),
  stop: &ProgressStop<'_>,
) -> Result<GitOutput> {
  let (subcommand, rest) = args.split_first().ok_or_else(|| anyhow!("No git subcommand to run"))?;
  let mut progress_args = Vec::with_capacity(args.len() + 1);
  progress_args.extend([*subcommand, "--progress"]);
  progress_args.extend_from_slice(rest);
  let mut reporter = TransferProgressReporter::new(on_progress);
  git_executor
    .cmd(&remote_args(config, &progress_args), repository_path)
    .env(&REMOTE_ENV)
    .accept(ok_exit_codes)
    .progress(
      &mut |line| {
        reporter.on_line(line);
      },
      stop,
    )
}
