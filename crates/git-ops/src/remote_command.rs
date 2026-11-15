//! The policy of every git that talks to a remote (push, fetch, `git lfs fetch`): the user's credential helpers without
//! prompts, an abort of a stalled HTTP transfer, the C locale for the `--progress` lines (docs/design/git-operations.md,
//! Key Decision 8). The variants with progress are `sync_core::remote_command`'s, on these constants.

use anyhow::Result;
use git_executor::git_command_executor::{GitCommandExecutor, GitOutput, ProgressStop};
use std::time::Duration;

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
pub const REMOTE_ENV: [(&str, &str); 3] = [("GCM_INTERACTIVE", "never"), ("LC_ALL", "C"), ("LANGUAGE", "")];

/// The policy's config, then `config` (more `-c` pairs of the caller), then `args`
pub fn remote_args<'a>(config: &[&'a str], args: &[&'a str]) -> Vec<&'a str> {
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

/// The longest a git of the remote may run that does its HTTP outside git (`git lfs pull`, `git lfs fetch`): git-lfs has
/// its own client, which `http.lowSpeedLimit` does not reach, so a transfer that trickles or a helper that never answers
/// would hold it without end. Past it git is killed with every process it started (`StoppedCommand::TimedOut`)
pub const LFS_DEADLINE: Duration = Duration::from_mins(30);

/// `execute_remote_command` killed after `deadline` (`LFS_DEADLINE`)
pub fn execute_remote_command_within(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, deadline: Duration) -> Result<String> {
  git_executor.cmd(&remote_args(&[], args), repository_path).env(&REMOTE_ENV).timeout(deadline).text()
}

/// `execute_remote_command_within` killed as well once `canceled` says so (`StoppedCommand::Cancelled`)
pub fn execute_remote_command_until(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, deadline: Duration, canceled: &dyn Fn() -> bool) -> Result<()> {
  let stop = ProgressStop {
    idle_timeout: None,
    is_cancelled: canceled,
  };
  git_executor
    .cmd(&remote_args(&[], args), repository_path)
    .env(&REMOTE_ENV)
    .timeout(deadline)
    .progress(&mut |_| {}, &stop)?;
  Ok(())
}

/// `execute_remote_command_accepting` that keeps git's stderr and exit code too: an accepted failure (a `push` whose
/// ref the remote refused) is said in the `remote:` lines of stderr
pub fn execute_remote_command_output(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, ok_exit_codes: &[i32]) -> Result<GitOutput> {
  git_executor.cmd(&remote_args(&[], args), repository_path).env(&REMOTE_ENV).accept(ok_exit_codes).output()
}
