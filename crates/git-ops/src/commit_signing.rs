use crate::commit_writer::{NewCommit, NewParent, WrittenCommits, read_raw_commits, write_commits};
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

/// Whether Branch Deck should sign commits it writes onto branches
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Signing {
  /// Do not sign commits
  #[default]
  Off,
  /// Sign commits with git's configured signing key
  On,
}

/// The repository's `commit.gpgsign`; an unset key is off
pub fn commit_signing(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Signing> {
  let (value, exit_code) = git_executor.execute_command_with_status(&["config", "--type=bool", "--get", "commit.gpgsign"], repo_path)?;
  match exit_code {
    0 => match value.trim() {
      "true" => Ok(Signing::On),
      "false" => Ok(Signing::Off),
      value => bail!("git config --type=bool --get commit.gpgsign returned unexpected value {value:?}"),
    },
    1 => Ok(Signing::Off),
    _ => bail!("git config --type=bool --get commit.gpgsign failed with exit code {exit_code}: {}", value.trim()),
  }
}

pub(crate) const PLACEHOLDER: &str = "-----BEGIN PGP SIGNATURE-----\n-----END PGP SIGNATURE-----\n";

pub(crate) const fn signed_commits_arg(signing: Signing) -> &'static str {
  match signing {
    Signing::Off => "--signed-commits=strip",
    Signing::On => "--signed-commits=sign-if-invalid",
  }
}

pub(crate) fn push_signature_placeholder(stream: &mut Vec<u8>, signing: Signing) {
  if signing == Signing::Off {
    return;
  }
  stream.extend_from_slice(format!("gpgsig sha1 openpgp\ndata {}\n", PLACEHOLDER.len()).as_bytes());
  stream.extend_from_slice(PLACEHOLDER.as_bytes());
  stream.push(b'\n');
}

pub(crate) fn map_signing_error(error: anyhow::Error, signing: Signing) -> anyhow::Error {
  match signing {
    Signing::Off => error,
    Signing::On => SigningFailed(format!("{error:#}")).into(),
  }
}

/// Sign the first-parent commits after `base` through `tip`, or all commits through `tip` when `base` is `None`.
/// Already-signed commits at the start are kept; once a commit needs signing, its descendants are rewritten too.
#[instrument(skip(git_executor), fields(signing = ?signing))]
pub fn sign_range(git_executor: &GitCommandExecutor, repo_path: &str, base: Option<&str>, tip: &str, signing: Signing) -> Result<Option<WrittenCommits>> {
  if signing == Signing::Off {
    return Ok(None);
  }
  let range = base.map_or_else(|| tip.to_string(), |base| format!("{base}..{tip}"));
  let ids = git_executor.execute_command_lines(&["rev-list", "--reverse", "--first-parent", "--end-of-options", &range], repo_path)?;
  if ids.is_empty() {
    return Ok(None);
  }
  let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
  let raw = read_raw_commits(git_executor, repo_path, &id_refs)?;
  let first_unsigned = ids.iter().position(|id| !raw[id].signed);
  let Some(first_unsigned) = first_unsigned else {
    return Ok(None);
  };
  let commits: Vec<NewCommit<'_>> = ids[first_unsigned..]
    .iter()
    .enumerate()
    .map(|(index, id)| {
      let commit = &raw[id];
      let parent = if index == 0 {
        commit.parents.first().map(|parent| NewParent::Existing(parent.as_str()))
      } else {
        Some(NewParent::Written(index - 1))
      };
      NewCommit {
        tree: &commit.tree,
        parent,
        author: &commit.author,
        committer: &commit.committer,
        encoding: commit.encoding.as_deref(),
        message: &commit.message,
      }
    })
    .collect();
  if ids[first_unsigned..].iter().any(|id| raw[id].parents.len() > 1) {
    bail!("Cannot sign a range containing a merge commit");
  }
  write_commits(git_executor, repo_path, &commits, signing).map(Some)
}

/// A `fast-import` writer could not sign the commits it was asked to write
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Cannot sign the commits (commit.gpgsign is true): {0}")]
pub struct SigningFailed(pub String);

#[cfg(test)]
#[path = "commit_signing_test.rs"]
mod commit_signing_test;

#[cfg(all(test, windows))]
pub(crate) fn ssh_keygen_available() -> bool {
  sync_utils::child_process::windowless_command("ssh-keygen").arg("-?").output().is_ok()
}
