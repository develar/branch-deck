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

/// How long a command of Git LFS (`git lfs pull`, `git lfs fetch`) may go without a line of its progress before it is
/// killed: ten minutes, past the time git-lfs takes itself to give up a dead transfer (`lfs.activitytimeout` 30 s a
/// try, `lfs.transfer.maxretries` 8, `lfs.transfer.maxretrydelay` 10 s: about five minutes), so only a hang is killed,
/// a helper that never answers or a transfer that never ends, never a slow download, which writes a line as each part
/// arrives. git-lfs does its own HTTP, which `http.lowSpeedLimit` does not reach. No cap on the whole: a large honest
/// download takes what it takes, and the user cancels it
pub const LFS_IDLE: Duration = Duration::from_mins(10);

/// `execute_remote_command` of a command of Git LFS, killed with every process it started when it wrote no line of
/// progress for `idle` (to `GIT_LFS_PROGRESS`, a file of its own, or to stderr) or once `canceled` says so. Where
/// Windows puts git into no job object, a kill reaches git alone, not the git-lfs it started (`sync_utils::process_tree`,
/// which warns of it once): that git-lfs runs on until it ends by itself
///
/// # Errors
/// git failed, made no progress for `idle` (`git lfs reported no progress for …`), or was canceled
pub fn execute_remote_command_while_progressing(
  git_executor: &GitCommandExecutor,
  args: &[&str],
  repository_path: &str,
  idle: Duration,
  canceled: &dyn Fn() -> bool,
) -> Result<()> {
  let progress = tempfile::NamedTempFile::new()?;
  let progress_path = progress.path().to_string_lossy().into_owned();
  let mut env = REMOTE_ENV.to_vec();
  env.push(("GIT_LFS_PROGRESS", progress_path.as_str()));
  // the length of the progress file, and when it or stderr last grew
  let last = std::cell::Cell::new((0u64, std::time::Instant::now()));
  let idled = std::cell::Cell::new(false);
  let stopped = || {
    if canceled() {
      return true;
    }
    let length = std::fs::metadata(progress.path()).map_or(0, |metadata| metadata.len());
    let (seen, since) = last.get();
    if length != seen {
      last.set((length, std::time::Instant::now()));
    } else if since.elapsed() >= idle {
      idled.set(true);
      return true;
    }
    false
  };
  let stop = ProgressStop {
    idle_timeout: None,
    is_cancelled: &stopped,
  };
  let mut on_line = |_: &str| last.set((last.get().0, std::time::Instant::now()));
  let ran = git_executor.cmd(&remote_args(&[], args), repository_path).env(&env).progress(&mut on_line, &stop);
  match ran {
    Ok(_) => Ok(()),
    Err(error) if idled.get() => Err(error.context(format!("git lfs reported no progress for {} s", idle.as_secs()))),
    Err(error) => Err(error),
  }
}

/// `execute_remote_command_accepting` that keeps git's stderr and exit code too: an accepted failure (a `push` whose
/// ref the remote refused) is said in the `remote:` lines of stderr
pub fn execute_remote_command_output(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, ok_exit_codes: &[i32]) -> Result<GitOutput> {
  git_executor.cmd(&remote_args(&[], args), repository_path).env(&REMOTE_ENV).accept(ok_exit_codes).output()
}
