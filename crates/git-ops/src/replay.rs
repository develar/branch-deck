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
/// `git replay` stops at the first conflict). `replay(first, tip, last)` replays the commits from the index `first` to
/// the index `last` onto `tip` and is `None` on a conflict; each probe starts from the longest clean prefix found so
/// far, so the probes replay about n commits in all, not ⌈log₂ n⌉ × n/2. The prefix up to `high` conflicts, and every
/// prefix before `low` replays cleanly onto `tip`. Returns the index of the first conflicting commit and the tip of the
/// longest clean prefix (`tip` when there is none)
pub(crate) fn first_conflict(
  mut low: usize,
  mut high: usize,
  mut tip: String,
  mut replay: impl FnMut(usize, &str, usize) -> anyhow::Result<Option<String>>,
) -> anyhow::Result<(usize, String)> {
  while low < high {
    let middle = low + (high - low) / 2;
    match replay(low, &tip, middle)? {
      Some(clean) => {
        tip = clean;
        low = middle + 1;
      }
      None => high = middle,
    }
  }
  Ok((low, tip))
}

/// Where the replay of runs of commits ended (`first_conflict_in_runs`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunsReplay {
  /// Every commit replayed onto the tip
  Clean(String),
  /// The commit at `index` conflicts onto `tip`, the commits before it replayed
  Conflict { index: usize, tip: String },
}

/// Replay the commits from the index `from` to `end` (exclusive) onto `tip`, one `replay(first, tip, last)` per run of
/// consecutive commits (`run_starts`, the indices after a commit left out), and find the first conflicting commit of
/// the first run that conflicts (`first_conflict`). With `known_conflict`, the run of `from` is known to conflict up
/// to `end`, so its search starts at once
pub(crate) fn first_conflict_in_runs(
  run_starts: &[usize],
  from: usize,
  end: usize,
  mut tip: String,
  known_conflict: bool,
  mut replay: impl FnMut(usize, &str, usize) -> anyhow::Result<Option<String>>,
) -> anyhow::Result<RunsReplay> {
  let mut first = from;
  while first < end {
    let last = run_end(run_starts, first, end).min(end) - 1;
    // only the run of `from` can be known to conflict: a run that replays goes on to the next
    if !(known_conflict && first == from)
      && let Some(clean) = replay(first, &tip, last)?
    {
      tip = clean;
      first = last + 1;
      continue;
    }
    let (index, tip) = first_conflict(first, last, tip, &mut replay)?;
    return Ok(RunsReplay::Conflict { index, tip });
  }
  Ok(RunsReplay::Clean(tip))
}

/// The replay of the commits from `local[first]` to `local[last]` onto `tip` with `env`, as `first_conflict` and
/// `first_conflict_in_runs` call it; `local` is linear history within each run
pub(crate) fn replay_of<'a>(
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  local: &'a [String],
  env: &'a [(&'a str, &'a str)],
) -> impl FnMut(usize, &str, usize) -> anyhow::Result<Option<String>> + 'a {
  move |first, tip, last| {
    // linear history within a run: `<last> ^<first>^` is exactly the commits from `first` to `last`
    let revs = [local[last].clone(), format!("^{}^", local[first])];
    let revs: Vec<&str> = revs.iter().map(String::as_str).collect();
    Ok(replay_range_with_env(git_executor, repo_path, tip, &revs, env)?)
  }
}

/// The end (exclusive) of the run of consecutive commits that holds the commit at `index` of `len` commits, whose runs
/// start at `run_starts` (the indices after a commit left out)
pub(crate) fn run_end(run_starts: &[usize], index: usize, len: usize) -> usize {
  run_starts.iter().copied().find(|&start| start > index).unwrap_or(len)
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

#[cfg(test)]
#[path = "replay_test.rs"]
mod replay_test;
