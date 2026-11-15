//! The write of a companion (openspec/specs/branch/declared-needs/spec.md, "Companion repositories"): the add of the
//! repository at a path as the companion `name` of a primary, or its removal; `branchdeck.companion.<name>.path` in the
//! local config of the primary, `branchdeck.primary` in the local config of the companion, both the paths of the main
//! worktrees. A companion has one primary, a primary is no companion, a repository is no companion of itself.
//!
//! An operation of two configs is one write: it holds the companion lock of both repositories
//! (`branch-deck/companions.lock` in the common git directory of each, `lock_companions`), taken in the order of the
//! directories, across every read and write of both configs, so two operations of one repository never interleave,
//! whether of the app or of `brd`, and two that need each other's lock never wait for each other. The app also holds
//! the writers of both configs for the whole write (`app_api::app::repository_config::write_companion`). The check that a
//! companion has one primary and the write of its side are then one step: of two primaries that add one companion at
//! once, the second is refused.
//!
//! A half link, one side of a link the other side lacks (an add or a removal cut short, a `git config` of the user), is
//! a problem of the link that Settings and `brd companion list` show on the side that has it
//! (`companions::half_link_problem`, `companions::primary_link_problem`), and the next add or removal of the pair
//! repairs it: an add of a companion that names this primary already writes the key of the primary, an add of a
//! companion whose primary can be read and does not list it takes it over, a removal unsets the side of the companion
//! only while it names this primary. A primary that can not be read (its path gone, a disk not mounted now, a config
//! that fails to read) is unknown, not absent: an add of its companion elsewhere is refused with how to unlink it, and
//! the companion unlinks it on its side (`unlink_primary`, Unlink of its Settings, `brd companion unlink`).

use crate::companions::{directory_name, same_path};
use crate::config_snapshot::ConfigSnapshot;
use crate::repository_config::{PRIMARY_CONFIG_KEY, companion_problem, run_config_write};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::git_common_dir;
use history_rewrite::repository_lock::{RepositoryLock, lock_file};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, instrument, warn};

/// The lock of the companion operations of a repository, in its common git directory
pub(crate) const COMPANION_LOCK: &str = "branch-deck/companions.lock";

/// How long a write waits for another that holds a companion lock: the other one reads and writes two configs
const COMPANION_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// How often a write asks again which companion it touches, when the one a removal names changed while it took the
/// locks (`Written::Moved`)
pub const COMPANION_TRIES: usize = 3;

/// Adds the repository at `path` as the companion `name` of the repository at `repository_path`, or removes it (`None`),
/// under the companion locks of both repositories (the module's header)
///
/// # Errors
/// A refusal of the write (a companion of a second primary, a companion of a companion, a companion of itself, a name of
/// other characters, a name of another companion), a lock not taken in time (`RepositoryLocked`), a failed read or
/// write of a config
#[instrument(skip(git))]
pub fn write_companion(git: &GitCommandExecutor, repository_path: &str, name: &str, path: Option<&str>) -> Result<()> {
  for _ in 0..COMPANION_TRIES {
    let companion = companion_of_write(git, repository_path, name, path)?;
    if write_companion_under(git, repository_path, name, path, companion.as_deref(), &|| debug!("waiting for the lock of a companion"))? == Written::Done {
      return Ok(());
    }
  }
  Err(companion_moved(repository_path, name))
}

/// The error of a write whose companion changed on each of its `COMPANION_TRIES`
#[must_use]
pub fn companion_moved(repository_path: &str, name: &str) -> anyhow::Error {
  anyhow!("The companion {name} of {repository_path} changed while it was written: ask again")
}

/// The repository a write of a companion touches beside the primary, the main worktree each names the other by: the
/// one at `path` of an add, the one the primary names as `name` of a removal; none for a removal of a name that is no
/// companion, or of a companion whose path can not be read
///
/// # Errors
/// A failed read of the config of the primary, or of the repository at `path`
pub fn companion_of_write(git: &GitCommandExecutor, repository_path: &str, name: &str, path: Option<&str>) -> Result<Option<String>> {
  match path.map(str::trim).filter(|path| !path.is_empty()) {
    Some(path) => Ok(companion_problem(path).is_none().then(|| main_worktree(git, path)).transpose()?),
    None => Ok(
      ConfigSnapshot::read(git, repository_path)?
        .companions()
        .get(name)
        .filter(|path| companion_problem(path).is_none())
        .cloned(),
    ),
  }
}

/// What a write under the locks of a pair did
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
  Done,
  /// The companion the write touches is another one than the one it locked: it asks again (`companion_of_write`)
  Moved,
}

/// The write of `write_companion` under the companion locks of the primary at `repository_path` and of `companion`, the
/// one `companion_of_write` answered, `on_wait` once while another holds one of them. `Moved` when the write touches
/// another companion now, which it then did not write
///
/// # Errors
/// As `write_companion`
pub fn write_companion_under(git: &GitCommandExecutor, repository_path: &str, name: &str, path: Option<&str>, companion: Option<&str>, on_wait: &dyn Fn()) -> Result<Written> {
  validate_companion_name(name)?;
  let _locks = lock_companions(git, &[Some(repository_path), companion], on_wait)?;
  if companion_of_write(git, repository_path, name, path)?.as_deref() != companion {
    return Ok(Written::Moved);
  }
  let key = format!("branchdeck.companion.{name}.path");
  let primary = main_worktree(git, repository_path)?;
  let own = ConfigSnapshot::read(git, repository_path)?;
  let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
    if !own.companions().contains_key(name) {
      bail!("{name} is no companion of {primary}");
    }
    run_config_write(git, repository_path, "--local", &key, None)?;
    // the side of the companion goes only while it names this primary: of a half link, only the key of the primary
    if let Some(companion) = companion
      && names(git, companion, &primary)?
    {
      run_config_write(git, companion, "--local", PRIMARY_CONFIG_KEY, None)?;
    }
    return Ok(Written::Done);
  };
  if let Some(of) = own.primary() {
    bail!("{primary} is a companion of {of}: only a primary has companions");
  }
  if let Some(problem) = companion_problem(path) {
    bail!("{problem}");
  }
  let Some(companion) = companion else {
    bail!("{path} cannot be read");
  };
  if same_path(Path::new(companion), Path::new(&primary)) {
    bail!("{primary} cannot be a companion of itself");
  }
  // one name, one companion: a name of another path is removed first, a companion listed under another name keeps it
  if let Some(listed) = own.companions().get(name)
    && !same_path(Path::new(listed), Path::new(companion))
  {
    bail!("{name} is the companion {listed} of {primary} already: remove it first");
  }
  if let Some((listed, _)) = own
    .companions()
    .iter()
    .find(|(listed, path)| *listed != name && same_path(Path::new(path), Path::new(companion)))
  {
    bail!("{companion} is the companion {listed} of {primary} already");
  }
  let before = ConfigSnapshot::read(git, companion)?.primary().map(str::to_owned);
  let named_already = before.as_deref().is_some_and(|other| same_path(Path::new(other), Path::new(&primary)));
  match before.as_deref() {
    Some(_) if named_already => {}
    Some(other) => match primary_says(git, other, companion) {
      PrimarySays::Lists => bail!("{companion} is a companion of {other} already: a companion has one primary"),
      // an unmounted disk, a config that fails to read: the link there may stand, and a take-over would lose it
      PrimarySays::Unknown(why) => bail!(
        "{companion} names {other} as its primary, which cannot be read ({why}): remove it there once {other} can be read, or Unlink it in Settings → Landing of {} (`brd companion unlink` there)",
        directory_name(companion)
      ),
      // a half link of a primary that does not list the companion: taken over
      PrimarySays::Unlisted => run_config_write(git, companion, "--local", PRIMARY_CONFIG_KEY, Some(&primary))?,
    },
    None => run_config_write(git, companion, "--local", PRIMARY_CONFIG_KEY, Some(&primary))?,
  }
  if let Err(error) = run_config_write(git, repository_path, "--local", &key, Some(companion)) {
    // the side named just now goes back to what it was
    if !named_already && let Err(undo) = run_config_write(git, companion, "--local", PRIMARY_CONFIG_KEY, before.as_deref()) {
      warn!("cannot take back the primary of {companion}: {undo:#}");
    }
    return Err(error);
  }
  Ok(Written::Done)
}

/// Whether the repository at `companion` names the primary at `primary` as its primary
fn names(git: &GitCommandExecutor, companion: &str, primary: &str) -> Result<bool> {
  Ok(
    ConfigSnapshot::read(git, companion)?
      .primary()
      .is_some_and(|named| same_path(Path::new(named), Path::new(primary))),
  )
}

/// What the primary at `primary` says of the repository at `companion`
#[derive(Debug, PartialEq, Eq)]
enum PrimarySays {
  /// It lists it among its companions: the link stands on both sides
  Lists,
  /// It can be read and lists it not: a half link of the companion's side
  Unlisted,
  /// It can not be read, with why: its path is gone (a disk not mounted now) or holds no git repository, or its config
  /// fails to read. What it lists is unknown, not absent
  Unknown(String),
}

/// What the primary at `primary` says of the repository at `companion` (`PrimarySays`)
fn primary_says(git: &GitCommandExecutor, primary: &str, companion: &str) -> PrimarySays {
  if let Some(problem) = companion_problem(primary) {
    return PrimarySays::Unknown(problem);
  }
  match ConfigSnapshot::read(git, primary) {
    Ok(config) if config.companions().values().any(|path| same_path(Path::new(path), Path::new(companion))) => PrimarySays::Lists,
    Ok(_) => PrimarySays::Unlisted,
    Err(error) => PrimarySays::Unknown(format!("{error:#}")),
  }
}

/// Unlinks the repository at `repository_path` from the primary its config names (`branchdeck.primary`, unset), the
/// side a half link leaves on a companion whose primary does not list it or can not be read; under the companion locks
/// of both repositories, asked again under them. Refused while the primary lists it: a link of both sides goes by a
/// removal there
///
/// # Errors
/// It names no primary, its primary lists it, a lock not taken in time, a failed read or write of its config
#[instrument(skip(git))]
pub fn unlink_primary(git: &GitCommandExecutor, repository_path: &str) -> Result<()> {
  unlink_primary_with(git, repository_path, &|| debug!("waiting for the lock of a companion"))
}

/// `unlink_primary`, `on_wait` once while another holds a companion lock
pub(crate) fn unlink_primary_with(git: &GitCommandExecutor, repository_path: &str, on_wait: &dyn Fn()) -> Result<()> {
  for _ in 0..COMPANION_TRIES {
    let Some(primary) = ConfigSnapshot::read(git, repository_path)?.primary().map(str::to_owned) else {
      bail!("{repository_path} names no primary");
    };
    let _locks = lock_companions(git, &[Some(repository_path), Some(&primary)], on_wait)?;
    if ConfigSnapshot::read(git, repository_path)?.primary() != Some(primary.as_str()) {
      continue;
    }
    let own = main_worktree(git, repository_path)?;
    if primary_says(git, &primary, &own) == PrimarySays::Lists {
      bail!("{primary} lists {own} as its companion: remove it there");
    }
    return run_config_write(git, repository_path, "--local", PRIMARY_CONFIG_KEY, None);
  }
  bail!("The primary of {repository_path} changed while it was unlinked: ask again")
}

/// The companion locks of the repositories of `repositories`, each the one of its common git directory, taken in the
/// order of those directories, one per directory: a repository that can not be read takes none. `on_wait` once while
/// another holds one
fn lock_companions(git: &GitCommandExecutor, repositories: &[Option<&str>], on_wait: &dyn Fn()) -> Result<Vec<RepositoryLock>> {
  let mut dirs: Vec<PathBuf> = repositories
    .iter()
    .flatten()
    .filter_map(|repository| git_common_dir(git, repository).ok())
    .map(|dir| std::fs::canonicalize(&dir).unwrap_or_else(|_| PathBuf::from(dir)))
    .collect();
  dirs.sort();
  dirs.dedup();
  let mut locks = Vec::with_capacity(dirs.len());
  for dir in dirs {
    let path = dir.join(COMPANION_LOCK);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    locks.push(lock_file(path, COMPANION_LOCK_TIMEOUT, on_wait)?);
  }
  Ok(locks)
}

/// A name of a companion: a need writes it before `:`, git config keeps it as a subsection
fn validate_companion_name(name: &str) -> Result<()> {
  if name.is_empty() || !name.chars().all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | '.')) {
    bail!("{name:?} is no name of a companion: use letters, digits, -, _ and .");
  }
  Ok(())
}

/// The main worktree of the repository at `path` (the parent of its common git dir), the path both sides of a
/// companion name each other by: a lease and a linked worktree name their repository
pub(crate) fn main_worktree(git: &GitCommandExecutor, path: &str) -> Result<String> {
  let common = git_common_dir(git, path)?;
  let common = Path::new(&common);
  let main = if common.file_name().is_some_and(|name| name == ".git") {
    common.parent().unwrap_or(common)
  } else {
    common
  };
  Ok(main.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "companion_write_test.rs"]
mod companion_write_test;
