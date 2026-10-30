//! The git side of Patronus: which Space repository (`git.jetbrains.team`) the virtual branches are pushed to. The HTTP side is
//! the `patronus` crate.

use crate::baseline::{BaselineUpstream, resolve_baseline_upstream};
use crate::remote_url::{origin_push_url, parse_remote_url, split_owner_and_name};
use domain_types::patronus::{PatronusReviewRequirement, ReviewPathsSource};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::read_commit_files_cached;
use git_ops::ownership::OwnershipRules;
use object_cache::cache::ObjectCache;
use object_cache::commit_files::FileStatus;
use std::collections::{BTreeSet, HashMap, HashSet};

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

/// The Space repository `origin` pushes to (the virtual branches are pushed to origin); None when it is not on Space. One
/// `remote get-url`; a caller that read the URL (`origin_push_url`) parses it (`parse_space_repository`)
pub fn find_space_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<Option<SpaceRepository>> {
  Ok(origin_push_url(git_executor, repository_path)?.as_deref().and_then(parse_space_repository))
}

/// The branch a robot of the repository pushes to: the branch of `origin` the baseline tracks, e.g. `master` for
/// `origin/master`; None for a local baseline or one on another remote (Patronus works on the push remote)
pub fn find_target_branch(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> anyhow::Result<Option<String>> {
  Ok(target_branch_of(resolve_baseline_upstream(git_executor, repository_path, baseline_branch)?.as_ref()))
}

/// `find_target_branch` for an action: a baseline that is no branch of origin is an error
pub fn required_target_branch(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> anyhow::Result<String> {
  required_target_branch_of(baseline_branch, resolve_baseline_upstream(git_executor, repository_path, baseline_branch)?.as_ref())
}

/// `required_target_branch` of the upstream of the baseline a caller read (`Baseline::upstream`): no git process
pub fn required_target_branch_of(baseline_branch: &str, upstream: Option<&BaselineUpstream>) -> anyhow::Result<String> {
  target_branch_of(upstream).ok_or_else(|| anyhow::anyhow!("The baseline {baseline_branch} is not a branch of origin: Patronus pushes into a branch of origin"))
}

/// `find_target_branch` of the upstream of the baseline a caller read (`SyncReport::upstream`): no git process
pub fn target_branch_of(upstream: Option<&BaselineUpstream>) -> Option<String> {
  let upstream = upstream.filter(|upstream| upstream.remote == "origin")?;
  upstream.remote_ref.strip_prefix("refs/heads/").map(str::to_string)
}

/// A commit of a virtual branch as the code-owner review check of Patronus reads it: who changed which paths
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCommit {
  pub hash: String,
  pub parents: Vec<String>,
  pub message: String,
  pub author_name: String,
  pub author_email: String,
  /// RFC 3339, as git prints it (`%aI`)
  pub author_date: String,
  pub committer_name: String,
  pub committer_email: String,
  pub committer_date: String,
  /// Every path the commit touches, a deleted one too: the robot checks the owners of all of them. A merge has none,
  /// as `git log --name-only` lists none for it
  pub changed_paths: Vec<String>,
}

impl ReviewCommit {
  pub const fn is_merge(&self) -> bool {
    self.parents.len() > 1
  }
}

/// The IDE does not check branches with this many commits either: the request would be huge
pub const MAX_REVIEW_COMMITS: usize = 500;

/// The format of one commit: a record separator, then NUL-separated fields, the message last
const REVIEW_COMMIT_FORMAT: &str = "--format=%x1e%H%x00%P%x00%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00%B";

/// The commits of each of `tips` (commit ids, or one revision) that `baseline_branch` lacks, newest first, in one
/// `git log` of all of them; None for a tip with more than `MAX_REVIEW_COMMITS`. The paths come from the object cache (`fill_changed_paths`). A log cut by
/// its limit (several long branches) can't tell which tips it covers whole: then one `git log` per tip.
pub fn read_review_commits(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, tips: &[String]) -> anyhow::Result<Vec<Option<Vec<ReviewCommit>>>> {
  if tips.is_empty() {
    return Ok(Vec::new());
  }
  let limit = (MAX_REVIEW_COMMITS + 1) * tips.len();
  let commits = read_review_log(git_executor, repository_path, baseline_branch, tips, limit)?;
  if commits.len() < limit {
    return Ok(tips.iter().map(|tip| commits_of_tip(&commits, tips, tip)).collect());
  }
  tips
    .iter()
    .map(|tip| {
      let tips = std::slice::from_ref(tip);
      let commits = read_review_log(git_executor, repository_path, baseline_branch, tips, MAX_REVIEW_COMMITS + 1)?;
      Ok(commits_of_tip(&commits, tips, tip))
    })
    .collect()
}

/// `git log` of the commits of `tips` without `baseline_branch`, at most `limit`, children before their parents (equal
/// commit times of several tips leave the date order to chance)
fn read_review_log(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str, tips: &[String], limit: usize) -> anyhow::Result<Vec<ReviewCommit>> {
  let max_count = format!("--max-count={limit}");
  let exclusion = format!("^{baseline_branch}");
  let mut args = vec!["log", "-z", "--topo-order", &max_count, REVIEW_COMMIT_FORMAT, "--end-of-options", &exclusion];
  args.extend(tips.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.cmd(&args, repository_path).bytes()?;
  Ok(parse_review_commits(&String::from_utf8_lossy(&output)))
}

/// The commits of `commits` (a log of `tips`) that `tip` reaches, in the order of the log; None for too many. With
/// several tips each is a commit id; a tip the log doesn't list (merged into the baseline) has none.
fn commits_of_tip(commits: &[ReviewCommit], tips: &[String], tip: &str) -> Option<Vec<ReviewCommit>> {
  if tips.len() == 1 {
    return (commits.len() <= MAX_REVIEW_COMMITS).then(|| commits.to_vec());
  }
  let by_hash: HashMap<&str, &ReviewCommit> = commits.iter().map(|commit| (commit.hash.as_str(), commit)).collect();
  let mut reached: HashSet<&str> = HashSet::new();
  let mut pending: Vec<&str> = by_hash.get_key_value(tip).map(|(hash, _)| *hash).into_iter().collect();
  while let Some(hash) = pending.pop() {
    if reached.insert(hash)
      && let Some(commit) = by_hash.get(hash)
    {
      pending.extend(commit.parents.iter().map(String::as_str).filter(|parent| by_hash.contains_key(parent)));
    }
  }
  (reached.len() <= MAX_REVIEW_COMMITS).then(|| commits.iter().filter(|commit| reached.contains(commit.hash.as_str())).cloned().collect())
}

/// `git log -z` with `REVIEW_COMMIT_FORMAT`: `\x1e` + 9 NUL-separated fields, the message last
fn parse_review_commits(output: &str) -> Vec<ReviewCommit> {
  output
    .split('\x1e')
    .filter(|record| !record.is_empty())
    .filter_map(|record| {
      let mut fields = record.split('\0');
      let mut next = || fields.next().map(str::to_string);
      Some(ReviewCommit {
        hash: next()?,
        parents: next()?.split_whitespace().map(str::to_string).collect(),
        author_name: next()?,
        author_email: next()?,
        author_date: next()?,
        committer_name: next()?,
        committer_email: next()?,
        committer_date: next()?,
        message: next()?.trim_end().to_string(),
        changed_paths: Vec::new(),
      })
    })
    .collect()
}

/// Give each commit the paths it changes against its first parent, as `git log --no-renames --name-only` lists them: a
/// rename is a delete and an add, whose owners both review it; a merge has none. From the `commit_files` store of the
/// object cache, with one `diff-tree --stdin` for the commits it misses (`read_commit_files_cached`).
pub fn fill_changed_paths(git_executor: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, commits: &mut [ReviewCommit]) -> anyhow::Result<()> {
  let ids: Vec<String> = commits.iter().filter(|commit| !commit.is_merge()).map(|commit| commit.hash.clone()).collect();
  let files: HashMap<String, Vec<String>> = read_commit_files_cached(git_executor, repository_path, cache, &ids)?
    .into_iter()
    .map(|files| {
      let mut paths: Vec<String> = files
        .files
        .into_iter()
        .flat_map(|file| match (file.status, file.old_path) {
          (FileStatus::Renamed, Some(old_path)) => vec![old_path, file.path],
          _ => vec![file.path],
        })
        .collect();
      paths.sort_unstable();
      paths.dedup();
      (files.commit_id, paths)
    })
    .collect();
  for commit in commits.iter_mut() {
    commit.changed_paths = files.get(&commit.hash).cloned().unwrap_or_default();
  }
  Ok(())
}

/// Every path the commits change, sorted and distinct: what the code owners of a branch review
pub fn review_paths(commits: &[ReviewCommit]) -> Vec<String> {
  let paths: BTreeSet<&str> = commits.iter().flat_map(|commit| commit.changed_paths.iter().map(String::as_str)).collect();
  paths.into_iter().map(str::to_string).collect()
}

/// Whether some group of the requirement lacks the paths it owes, so the `OWNERSHIP` files are worth reading
pub fn needs_ownership_paths(requirement: &PatronusReviewRequirement) -> bool {
  requirement.groups.iter().any(|group| group.paths_source == ReviewPathsSource::Unknown)
}

/// Give each group Patronus sent without paths the changed paths whose owner by the `OWNERSHIP` files has its name
/// (trimmed, case-insensitive, as Patronus resolves an owner to a group). A group that owns none by name stays
/// `Unknown`: Patronus delegated its review, or `review-rules.yaml` names it as the reviewer of another owner.
pub fn attribute_paths_by_ownership(requirement: &mut PatronusReviewRequirement, rules: &OwnershipRules, paths: &[String]) {
  let owners: Vec<(String, &str)> = paths
    .iter()
    .filter_map(|path| rules.owner_of(path).map(|owner| (owner.owner.trim().to_lowercase(), path.as_str())))
    .collect();
  for group in requirement.groups.iter_mut().filter(|group| group.paths_source == ReviewPathsSource::Unknown) {
    let name = group.name.trim().to_lowercase();
    let owned: Vec<String> = owners.iter().filter(|(owner, _)| *owner == name).map(|(_, path)| (*path).to_string()).collect();
    if !owned.is_empty() {
      group.paths = owned;
      group.paths_source = ReviewPathsSource::Ownership;
    }
  }
}
