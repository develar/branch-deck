use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;

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

fn remote_args<'a>(args: &[&'a str]) -> Vec<&'a str> {
  let mut full_args = Vec::with_capacity(REMOTE_CONFIG.len() + args.len());
  full_args.extend(REMOTE_CONFIG);
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
  git_executor.execute_command_accepting_with_env(&remote_args(args), repository_path, &REMOTE_ENV, ok_exit_codes)
}

/// `execute_remote_command` for a command run with `--progress`: `on_line` gets every stderr line as it arrives
pub fn execute_remote_command_with_progress(git_executor: &GitCommandExecutor, args: &[&str], repository_path: &str, on_line: &mut dyn FnMut(&str)) -> Result<String> {
  git_executor.execute_command_with_progress(&remote_args(args), repository_path, &REMOTE_ENV, on_line)
}
