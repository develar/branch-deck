//! `git replay` of a range and `merge-tree --write-tree` of three trees: they write objects only (docs/design/git-operations.md).

use crate::copy_commit::CopyCommitError;
use anyhow::anyhow;
use bstr::ByteSlice;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

/// `git replay --ref-action=print` reports the new tip as an update of this ref; the ref itself is never written
const REPLAY_REF: &str = "refs/branch-deck/replay";

/// Replay the commits of `revs` (a revision range of linear history, e.g. `A..B`) onto `onto` with `git replay`;
/// returns the new tip. The replayed commits keep their authors; the committer is the current user. Commits that
/// become empty are dropped. `None` on a conflict.
pub fn replay_range(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, revs: &[&str]) -> Result<Option<String>, CopyCommitError> {
  replay_range_with_env(git_executor, repo_path, onto, revs, &[])
}

/// `replay_range` with extra environment variables, e.g. a fixed `GIT_COMMITTER_DATE` for a replay whose commits are
/// only inspected: the same input then writes the same objects
#[instrument(skip(git_executor))]
pub fn replay_range_with_env(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, revs: &[&str], env: &[(&str, &str)]) -> Result<Option<String>, CopyCommitError> {
  let onto_arg = format!("--onto={onto}");
  let ref_arg = format!("--ref={REPLAY_REF}");
  let mut args = vec!["replay", "--ref-action=print", &onto_arg, &ref_arg];
  args.extend_from_slice(revs);
  let output = git_executor.cmd(&args, repo_path).env(env).status()?;
  match output.code {
    0 => {
      let output = output.into_text();
      // `update <ref> <new> <old>`
      let new_tip = output
        .lines()
        .find_map(|line| line.strip_prefix("update ")?.strip_prefix(REPLAY_REF)?.split_whitespace().next())
        .ok_or_else(|| CopyCommitError::Other(anyhow!("git replay printed no new tip: {output}")))?;
      Ok(Some(new_tip.to_string()))
    }
    1 => Ok(None),
    _ => Err(CopyCommitError::Other(anyhow!("git replay failed: {}", output.stderr))),
  }
}

/// The first commit of a linear range whose replay conflicts, by a binary search over its prefixes (⌈log₂ n⌉ replays:
/// `git replay` stops at the first conflict). `replay(last)` replays the commits up to the index `last` and is `None` on
/// a conflict; the prefix up to `high` conflicts, and every prefix before `low` replays cleanly onto `tip`. Returns the
/// index of the first conflicting commit and the tip of the longest clean prefix (`tip` when there is none)
pub(crate) fn first_conflict(mut low: usize, mut high: usize, mut tip: String, mut replay: impl FnMut(usize) -> anyhow::Result<Option<String>>) -> anyhow::Result<(usize, String)> {
  while low < high {
    let middle = low + (high - low) / 2;
    match replay(middle)? {
      Some(clean) => {
        tip = clean;
        low = middle + 1;
      }
      None => high = middle,
    }
  }
  Ok((low, tip))
}

/// The files whose merge conflicts when `commit` is cherry-picked onto `onto`, each once, in the order of
/// `merge-tree --name-only` (one process)
pub(crate) fn conflicting_files(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, commit: &str) -> anyhow::Result<Vec<String>> {
  let merge_base = format!("--merge-base={commit}^");
  // `<tree>\0<file>\0...`, exit code 1 on conflicts
  let output = git_executor
    .cmd(&["merge-tree", "--write-tree", "--name-only", "-z", "--no-messages", &merge_base, onto, commit], repo_path)
    .accept(&[1])
    .bytes()?;
  let mut files: Vec<String> = output
    .split(|&b| b == 0)
    .skip(1)
    .filter(|file| !file.is_empty())
    .map(|file| file.to_str_lossy().into_owned())
    .collect();
  files.dedup();
  Ok(files)
}

/// Replay the commits of `from..to` onto `onto`, see `replay_range`
pub(crate) fn replay_onto(git_executor: &GitCommandExecutor, repo_path: &str, onto: &str, from: &str, to: &str) -> Result<Option<String>, CopyCommitError> {
  replay_range(git_executor, repo_path, onto, &[&format!("{from}..{to}")])
}

/// Three-way merge of trees without a working tree; `None` on a conflict
pub(crate) fn merge_trees(git_executor: &GitCommandExecutor, repo_path: &str, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, CopyCommitError> {
  let merge_base = format!("--merge-base={base}");
  let (output, exit_code) = git_executor.execute_command_with_status(&["merge-tree", "--write-tree", "--no-messages", &merge_base, ours, theirs], repo_path)?;
  match exit_code {
    0 => Ok(Some(output.lines().next().unwrap_or_default().to_string())),
    1 => Ok(None),
    _ => Err(CopyCommitError::Other(anyhow!("git merge-tree failed: {output}"))),
  }
}
