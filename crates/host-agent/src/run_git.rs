//! The git of the host, in process (`lease_pool::host_git`, `lease_pool::host_slots::ensure_host_git`): the git that the
//! runs use is the one of Homebrew before `/usr/bin`, and below the floor of a host Homebrew installs or upgrades it once
//! no slot is lent. The lends queue meanwhile, and refuse with `old_git` while the git stays below the floor.
//!
//! - The keeper of `serve` brings the git to its floor for each repository at each tick ([`ensure`]). The call blocks
//!   while the pool waits for the lends and while Homebrew runs, so it runs on the thread of the keeper.
//! - A run puts the directory of that git first in the `PATH` of its command, before `/usr/bin`, and names its version
//!   in the line before the command ([`for_run`])

use crate::repo::Repository;
use host_link::refusal::{Code, Refusal};
use lease_pool::host_git::{find_git, min_version, run_path};
use lease_pool::host_slots::{EnsuredGit, ensure_host_git};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The git of the host that a run uses
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HostGit {
  pub(crate) path: PathBuf,
  /// The directory of the git, which a run puts first in its `PATH`
  pub(crate) dir: Option<PathBuf>,
  pub(crate) version: String,
}

/// The `PATH` of the agent, which the git of the host is looked up in
fn agent_path() -> OsString {
  std::env::var_os("PATH").unwrap_or_default()
}

/// The git of the host for a run. A git below the floor, or no git, refuses the run with `old_git`
pub(crate) fn for_run() -> Result<HostGit, Refusal> {
  let floor = min_version();
  let found = find_git(&run_path(&agent_path()).0).ok_or_else(|| Refusal::new(Code::OldGit, "the PATH of the host finds no git"))?;
  if !found.is_new_enough(floor) {
    return Err(Refusal::new(
      Code::OldGit,
      format!(
        "the git of the host, {} {}, is below the floor {floor}; the keeper of the agent upgrades it with Homebrew",
        found.path.display(),
        found.text
      ),
    ));
  }
  Ok(HostGit {
    dir: found.path.parent().filter(|dir| !dir.as_os_str().is_empty()).map(Path::to_path_buf),
    version: found.version.map_or(found.text, |version| version.to_string()),
    path: found.path,
  })
}

/// The git of the host, upgraded with Homebrew when it is below the floor. Each line of the wait and of Homebrew goes to
/// `progress`
pub(crate) fn ensure(repo: &Repository, progress: &dyn Fn(&str)) -> anyhow::Result<EnsuredGit> {
  ensure_host_git(&repo.params(), &agent_path(), progress)
}

/// A warning for a command that starts a login shell of bash or sh on macOS, such as `bash -lc '…'`: that shell runs
/// `/etc/profile`, whose `path_helper` puts `/usr/bin` first, so the command gets the Apple Git of `/usr/bin` and not
/// `git`, the one of the line before the command. zsh runs `path_helper` too, but `~/.zprofile` of a Homebrew user puts
/// Homebrew first again. `None` for any other command, and elsewhere than on macOS
pub(crate) fn login_shell_warning(argv: &[String], git: &str, macos: bool) -> Option<String> {
  let (program, arguments) = argv.split_first()?;
  let name = Path::new(program).file_name()?.to_str()?;
  if !macos || !matches!(name, "bash" | "sh") {
    return None;
  }
  let login = arguments
    .iter()
    .take_while(|argument| argument.starts_with('-'))
    .any(|argument| argument == "--login" || (!argument.starts_with("--") && argument.contains('l')));
  login.then(|| {
    format!("`{name} -l` runs /etc/profile, whose path_helper puts /usr/bin first: the command can get the Apple Git of /usr/bin, not {git}. Use `zsh -lc` or no login shell")
  })
}

#[cfg(test)]
#[path = "run_git_test.rs"]
mod run_git_test;
