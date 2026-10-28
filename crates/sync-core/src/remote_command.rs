//! The policy of every git that talks to a remote (push, fetch): the user's credential helpers without prompts, an
//! abort of a stalled HTTP transfer, the C locale for the `--progress` lines (docs/design/git-operations.md, Key
//! Decision 8).

use crate::transfer_progress::{TransferProgress, TransferProgressReporter};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::{GitCommandExecutor, GitOutput, ProgressStop};

/// Config for every git command that talks to a remote (push, fetch).
/// The user's credential helpers stay in effect (osxkeychain, Git Credential Manager, `gh auth git-credential`):
/// stored credentials are used, but nothing may ask for new ones, since there is no one to answer.
/// - `credential.interactive=false`: git and the helpers that honour it (GCM) never prompt;
///   `GIT_TERMINAL_PROMPT=0` from the executor's base environment covers the terminal prompt as well
/// - `http.lowSpeedLimit`/`http.lowSpeedTime`: an HTTP transfer below 1000 bytes/s for 30 s is aborted instead of
///   hanging on a dead connection
///
/// SSH is left as the user configured it. ssh prompts on the controlling terminal, which the app started from Finder
/// does not have, so a missing key fails rather than hangs; forcing `GIT_SSH_COMMAND="ssh -o BatchMode=yes"` would
/// override `core.sshCommand` and break `SSH_ASKPASS` GUI prompts.
const REMOTE_CONFIG: [&str; 6] = ["-c", "credential.interactive=false", "-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=30"];

/// - `GCM_INTERACTIVE=never`: GCM before `credential.interactive` existed reads only its own variable
/// - `LC_ALL=C`, `LANGUAGE=`: untranslated output, so the `--progress` lines can be read (IDEA does the same for every
///   git command); an unknown locale would fall back to C as well
const REMOTE_ENV: [(&str, &str); 3] = [("GCM_INTERACTIVE", "never"), ("LC_ALL", "C"), ("LANGUAGE", "")];

/// The policy's config, then `config` (more `-c` pairs of the caller), then `args`
fn remote_args<'a>(config: &[&'a str], args: &[&'a str]) -> Vec<&'a str> {
  let mut full_args = Vec::with_capacity(REMOTE_CONFIG.len() + config.len() + args.len());
  full_args.extend(REMOTE_CONFIG);
  full_args.extend_from_slice(config);
  full_args.extend_from_slice(args);
  full_args
}

/// Run a git command that talks to a remote under the shared policy above and return its trimmed stdout.
/// `args` start with the git subcommand (`push`, `fetch`); the policy config goes before it.
pub fn execute_remote_command(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str) -> Result<String> {
  execute_remote_command_accepting(git_executor, args, repository_path, &[])
}

/// `execute_remote_command` for which the given non-zero exit codes are not failures, e.g. 1 for a `push --porcelain`
/// with rejected refs, whose stdout still reports every ref
pub fn execute_remote_command_accepting(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, ok_exit_codes: &[i32]) -> Result<String> {
  git_executor.cmd(&remote_args(&[], args), repository_path).env(&REMOTE_ENV).accept(ok_exit_codes).text()
}

/// `execute_remote_command_accepting` that keeps git's stderr and exit code too: an accepted failure (a `push` whose
/// ref the remote refused) is said in the `remote:` lines of stderr
pub fn execute_remote_command_output(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, ok_exit_codes: &[i32]) -> Result<GitOutput> {
  git_executor.cmd(&remote_args(&[], args), repository_path).env(&REMOTE_ENV).accept(ok_exit_codes).output()
}

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
