//! What a sync reads about the repository once, at its start, for every step that needs it: the baseline with its
//! commit and its upstream, HEAD, and the branches under the prefix (docs/design/git-operations.md, "Virtual branch
//! sync"). Three git processes after the guards, a fourth only when HEAD's branch tracks a branch the listing lacks.

use crate::baseline::{NoBaseline, baseline_candidates, no_baseline};
use crate::update_project::{BaselineUpstream, KnownBaseline, TrackedUpstream, baseline_upstream_of, modified_time};
use anyhow::{Result, bail};
use branch_integration::common::{LISTED_BRANCH_ATOMS, ListedBranch};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::Path;
use tracing::instrument;

/// The repository as a sync found it at its start; every step of the sync works from these
#[derive(Debug, Clone)]
pub(crate) struct RepoFacts {
  /// The checked out branch; the guards of the sync passed for it
  pub branch: String,
  /// The baseline as `detect_baseline_branch` names it: `origin/master`, `origin/develop` (the upstream), `master`
  pub baseline: String,
  /// The commit of the baseline: the remote statuses, the placement of stacked branches, the integration detection and
  /// the update check all compare with this one, also when a fetch moves the baseline during the sync
  pub baseline_commit: String,
  /// The remote branch behind the baseline; `None` for a local baseline
  pub upstream: Option<BaselineUpstream>,
  pub head: String,
  /// When the repository was last fetched (the time of `FETCH_HEAD`)
  pub fetched_at: Option<u32>,
  /// The branches under the prefix, newest commit first: virtual, archived and unapplied
  pub branches: Vec<ListedBranch>,
  /// Full name -> tip of its remote branch on `origin`, of the branches under the prefix
  pub remote_tips: HashMap<String, String>,
}

impl RepoFacts {
  /// What the update check of the sync needs; `None` for a local baseline, which has nothing to update from
  pub(crate) fn known_baseline(&self) -> Option<KnownBaseline> {
    Some(KnownBaseline {
      branch: self.branch.clone(),
      head: self.head.clone(),
      upstream: self.upstream.clone()?,
      upstream_head: self.baseline_commit.clone(),
      fetched_at: self.fetched_at,
    })
  }
}

/// The ref atoms of the listing: the name, the upstream of a local branch (`TrackedUpstream`, and its short name as
/// `rev-parse --abbrev-ref @{u}` prints it), then the atoms of a `ListedBranch`
const FORMAT_PREFIX: &str = "%(refname)%00%(upstream)%00%(upstream:remotename)%00%(upstream:remoteref)%00%(upstream:short)%00";

/// Read the facts of a sync of `branch_prefix` on the checked out `branch`, with the baseline found as
/// `detect_baseline_branch` finds it (`preferred_branch` first): `remote`, one `for-each-ref` of the checked out
/// branch, the baseline candidates and the branches under the prefix, local and on `origin`, and one `rev-parse` of
/// HEAD, the baseline and `FETCH_HEAD`. An upstream the listing lacks (not a candidate) costs one `rev-parse` more
#[instrument(skip(git_executor))]
pub(crate) fn read_repo_facts(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str, branch_prefix: &str, preferred_branch: &str) -> Result<RepoFacts> {
  let remotes: Vec<String> = git_executor
    .execute_command_lines(&["remote"], repo_path)?
    .into_iter()
    .map(|remote| remote.trim().to_string())
    .filter(|remote| !remote.is_empty())
    .collect();
  let first_remote = remotes.first().map(String::as_str);
  let candidates = baseline_candidates(first_remote, preferred_branch);

  let prefix = branch_prefix.trim_end_matches('/');
  let current_ref = format!("refs/heads/{branch}");
  let local_prefix = format!("refs/heads/{prefix}/");
  let remote_prefix = format!("refs/remotes/origin/{prefix}/");
  let format = format!("--format={FORMAT_PREFIX}{LISTED_BRANCH_ATOMS}");
  // newest first, for the archived branches (`list_branches`)
  let mut args = vec![
    "for-each-ref",
    format.as_str(),
    "--sort=-committerdate",
    current_ref.as_str(),
    local_prefix.as_str(),
    remote_prefix.as_str(),
  ];
  args.extend(candidates.iter().map(|(full_ref, _)| full_ref.as_str()));
  let lines = git_executor.execute_command_lines(&args, repo_path)?;

  let mut listed: HashMap<&str, &str> = HashMap::with_capacity(lines.len());
  let mut tracked = None;
  let mut tracked_short = "";
  let mut branches = Vec::new();
  let mut remote_tips = HashMap::new();
  for line in &lines {
    let mut fields = line.splitn(6, '\0');
    let (Some(name), Some(upstream), Some(remote), Some(remote_ref), Some(upstream_short), Some(rest)) =
      (fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    else {
      continue;
    };
    let tip = rest.split('\0').next().unwrap_or_default();
    listed.insert(name, tip);
    if name == current_ref {
      tracked = Some(TrackedUpstream { upstream, remote, remote_ref });
      tracked_short = upstream_short;
    }
    if name.starts_with(&local_prefix) {
      branches.extend(ListedBranch::parse(&format!("{name}\0{rest}")));
    } else if let Some(full_name) = name.strip_prefix("refs/remotes/origin/")
      && name.starts_with(&remote_prefix)
    {
      remote_tips.insert(full_name.to_string(), tip.to_string());
    }
  }

  // `@{u}` first, as `detect_baseline_branch` takes it: the upstream if it exists, else the first candidate that does
  let upstream_exists = |upstream: &str| -> Result<bool> {
    if listed.contains_key(upstream) {
      return Ok(true);
    }
    let (_, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{upstream}^{{commit}}")], repo_path)?;
    Ok(exit_code == 0)
  };
  let tracked_upstream = tracked.map(|tracked| tracked.upstream).filter(|upstream| !upstream.is_empty());
  let (baseline_ref, baseline) = match tracked_upstream {
    Some(upstream) if first_remote.is_some() && !tracked_short.is_empty() && upstream_exists(upstream)? => (upstream.to_string(), tracked_short.to_string()),
    _ => match candidates.iter().find(|(full_ref, _)| listed.contains_key(full_ref.as_str())) {
      Some((full_ref, name)) => (full_ref.clone(), name.clone()),
      None => return Err(no_baseline(git_executor, repo_path, first_remote.is_some(), candidates)?),
    },
  };

  let baseline_rev = format!("{baseline_ref}^{{commit}}");
  let (output, exit_code) = git_executor.execute_command_with_status(
    &["rev-parse", "--path-format=absolute", "--git-path", "FETCH_HEAD", "HEAD^{commit}", &baseline_rev],
    repo_path,
  )?;
  let mut lines = output.lines();
  let (Some(fetch_head), Some(head), Some(baseline_commit), 0) = (lines.next(), lines.next(), lines.next(), exit_code) else {
    // HEAD of a repository without commits doesn't resolve
    let (_, head_exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "HEAD"], repo_path)?;
    if head_exit_code != 0 {
      return Err(NoBaseline::NoCommits.into());
    }
    bail!("Cannot resolve HEAD and {baseline}: {}", output.trim());
  };

  Ok(RepoFacts {
    branch: branch.to_string(),
    upstream: baseline_upstream_of(&remotes, &baseline, tracked),
    baseline,
    baseline_commit: baseline_commit.to_string(),
    head: head.to_string(),
    fetched_at: modified_time(Path::new(fetch_head)),
    branches,
    remote_tips,
  })
}

#[cfg(test)]
#[path = "repo_facts_test.rs"]
mod repo_facts_test;
