//! The git side of Patronus: which Space repository (`git.jetbrains.team`) the virtual branches are pushed to. The HTTP side is
//! the `patronus` crate.

use crate::remote_url::{parse_remote_url, split_owner_and_name};
use crate::update_project::resolve_baseline_upstream;
use git_executor::git_command_executor::GitCommandExecutor;

/// The git host of Space
pub const SPACE_GIT_HOST: &str = "git.jetbrains.team";

/// A repository on Space, e.g. `ij/ultimate`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceRepository {
  /// e.g. `ij`
  pub project_key: String,
  /// e.g. `ultimate`; Patronus names the repository by it
  pub name: String,
}

/// The Space repository of a remote URL: `ssh://git@git.jetbrains.team/ij/ultimate.git`,
/// `https://git.jetbrains.team/ij/ultimate.git`; None for another host or a local path
pub fn parse_space_repository(url: &str) -> Option<SpaceRepository> {
  let remote = parse_remote_url(url)?;
  if !remote.host.eq_ignore_ascii_case(SPACE_GIT_HOST) {
    return None;
  }
  let (project_key, name) = split_owner_and_name(remote.path)?;
  Some(SpaceRepository {
    project_key: project_key.to_string(),
    name: name.to_string(),
  })
}

/// The Space repository `origin` pushes to (`push_branch` pushes to origin); None when it is not on Space
pub fn find_space_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<Option<SpaceRepository>> {
  let (url, exit_code) = git_executor.execute_command_with_status(&["remote", "get-url", "--push", "origin"], repository_path)?;
  Ok((exit_code == 0).then(|| parse_space_repository(&url)).flatten())
}

/// The branch a robot of the repository pushes to: the branch of `origin` the baseline tracks, e.g. `master` for
/// `origin/master`; None for a local baseline or one on another remote (Patronus works on the push remote)
pub fn find_target_branch(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> anyhow::Result<Option<String>> {
  let Some(upstream) = resolve_baseline_upstream(git_executor, repository_path, baseline_branch)? else {
    return Ok(None);
  };
  if upstream.remote != "origin" {
    return Ok(None);
  }
  Ok(upstream.remote_ref.strip_prefix("refs/heads/").map(str::to_string))
}

/// A commit of a virtual branch as the code-owner review check of Patronus reads it: who changed which paths
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCommit {
  pub hash: String,
  pub is_merge: bool,
  pub message: String,
  pub author_name: String,
  pub author_email: String,
  /// RFC 3339, as git prints it (`%aI`)
  pub author_date: String,
  pub committer_name: String,
  pub committer_email: String,
  pub committer_date: String,
  /// Every path the commit touches, a deleted one too: the robot checks the owners of all of them
  pub changed_paths: Vec<String>,
}

/// The IDE does not check branches with this many commits either: the request would be huge
pub const MAX_REVIEW_COMMITS: usize = 500;

/// The format of one commit: a record separator, then NUL-separated fields, the message last
const REVIEW_COMMIT_FORMAT: &str = "--format=%x1e%H%x00%P%x00%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%B";

/// The commits of `branch_ref` that `baseline_branch` lacks, newest first, with their changed paths, in one `git log`.
/// None for `MAX_REVIEW_COMMITS` or more. Renames are a delete and an add (no rename detection on large commits);
/// `--no-relative` ignores a `diff.relative` setting of the user.
pub fn read_review_commits(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, branch_ref: &str) -> anyhow::Result<Option<Vec<ReviewCommit>>> {
  let max_count = format!("--max-count={}", MAX_REVIEW_COMMITS + 1);
  let exclusion = format!("^{baseline_branch}");
  let output = git_executor.execute_command_raw(
    &[
      "log",
      "-z",
      "--no-renames",
      "--no-relative",
      "--name-only",
      &max_count,
      REVIEW_COMMIT_FORMAT,
      &exclusion,
      branch_ref,
      "--",
    ],
    repository_path,
  )?;
  let commits = parse_review_commits(&output);
  Ok((commits.len() <= MAX_REVIEW_COMMITS).then_some(commits))
}

/// `git log -z --name-only` with `REVIEW_COMMIT_FORMAT`: `\x1e` + 9 fields, a NUL, then `\n` and the NUL-terminated paths
fn parse_review_commits(output: &str) -> Vec<ReviewCommit> {
  output
    .split('\x1e')
    .filter(|record| !record.is_empty())
    .filter_map(|record| {
      let mut fields = record.split('\0');
      let mut next = || fields.next().map(str::to_string);
      let hash = next()?;
      let parents = next()?;
      let commit = ReviewCommit {
        hash,
        is_merge: parents.split_whitespace().count() > 1,
        author_name: next()?,
        author_email: next()?,
        author_date: next()?,
        committer_name: next()?,
        committer_email: next()?,
        committer_date: next()?,
        message: next()?.trim_end().to_string(),
        changed_paths: Vec::new(),
      };
      let changed_paths = fields
        .map(|path| path.trim_start_matches('\n'))
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect();
      Some(ReviewCommit { changed_paths, ..commit })
    })
    .collect()
}
