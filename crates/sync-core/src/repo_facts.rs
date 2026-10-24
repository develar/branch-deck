//! What a sync reads about the repository once, at its start, for every step that needs it: the baseline with its
//! commit and its upstream, HEAD, and the branches under the prefix (docs/design/git-operations.md, "Virtual branch
//! sync"). Three git processes after the guards, a fourth only when HEAD's branch tracks a branch the listing lacks;
//! none of them reads a commit beyond HEAD and the baseline, so the first rows of a sync never wait for the commits of
//! the archived and the remote branches (on the monorepo 452 refs, 1 to 2 s from a cold pack).

use crate::baseline::{BaselineUpstream, KnownBaseline, TrackedUpstream, baseline_upstream_of, modified_time};
use crate::conflict_chain::CONFLICT_REFS;
use crate::landed::STATE_FILE;
use anyhow::{Result, bail};
use branch_integration::common::{LISTED_BRANCH_ATOMS, ListedBranch};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{STALE_TEMP_REF_AGE, TEMP_REF_PREFIX, is_stale_temp_ref};
use git_ops::revs::{current_branch, known_dirs, resolve_commit};
use history_rewrite::baseline::{NoBaseline, baseline_candidates, no_baseline};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::instrument;

/// The repository as a sync found it at its start; every step of the sync works from these
#[derive(Debug, Clone)]
pub(crate) struct RepoFacts {
  /// The checked out branch; the guards of the sync passed for it
  pub branch: String,
  /// The configured remotes, as `git remote` lists them
  pub remotes: Vec<String>,
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
  /// The landed state of this worktree (`landed::STATE_FILE`)
  pub landed_state: PathBuf,
  /// The branches under the prefix by name: virtual, archived, unapplied and stack (the archive step orders the archived
  /// ones by their tips, `branch_data_of`)
  pub branches: Vec<ListedBranch>,
  /// Full name -> tip of its remote branch on `origin`, of the branches under the prefix
  pub remote_tips: HashMap<String, String>,
  /// Ref -> tip of the refs that keep the copies of the conflicting branches (`conflict_chain`)
  pub conflict_tips: HashMap<String, String>,
  /// `(ref, tip)` of the temporary refs that processes gone left behind (`is_stale_temp_ref`), which the sync deletes
  pub stale_temp_refs: Vec<(String, String)>,
}

impl RepoFacts {
  /// Whether HEAD is the local baseline branch itself (`commit_list::is_on_local_baseline`), from the facts: no process
  pub(crate) fn is_on_local_baseline(&self) -> bool {
    !self.baseline.contains('/') && self.branch == self.baseline
  }

  /// What the update check of the sync needs; `None` for a local baseline, which has nothing to update from
  pub(crate) fn known_baseline(&self) -> Option<KnownBaseline> {
    Some(KnownBaseline {
      branch: self.branch.clone(),
      head: self.head.clone(),
      upstream: self.upstream.clone()?,
      upstream_head: self.baseline_commit.clone(),
      fetched_at: self.fetched_at,
      landed_state: self.landed_state.clone(),
    })
  }
}

/// The ref atoms of the listing: the name, the upstream of a local branch (`TrackedUpstream`, and its short name as
/// `rev-parse --abbrev-ref @{u}` prints it), then the atoms of a `ListedBranch`
const FORMAT_PREFIX: &str = "%(refname)%00%(upstream)%00%(upstream:remotename)%00%(upstream:remoteref)%00%(upstream:short)%00";

/// The baseline and the branches under the prefix, as `detect_baseline_branch` and `list_branches` would read them, from
/// `remote` and one `for-each-ref` (`read_baseline_listing`)
#[derive(Debug, Clone)]
pub(crate) struct BaselineListing {
  /// The configured remotes, as `git remote` lists them
  pub remotes: Vec<String>,
  /// The baseline as `detect_baseline_branch` names it: `origin/master`, `origin/develop` (the upstream), `master`
  pub baseline: String,
  /// The full ref of the baseline: `refs/remotes/origin/master`, `refs/heads/master`
  pub baseline_ref: String,
  /// The remote branch behind the baseline; `None` for a local baseline
  pub upstream: Option<BaselineUpstream>,
  /// The branches under the prefix by name: virtual, archived, unapplied and stack (the archive step orders the archived
  /// ones by their tips, `branch_data_of`)
  pub branches: Vec<ListedBranch>,
  /// Full name -> tip of its remote branch on `origin`, of the branches under the prefix
  pub remote_tips: HashMap<String, String>,
  /// Ref -> tip of the refs that keep the copies of the conflicting branches, when asked for (`conflict_chain`)
  pub conflict_tips: HashMap<String, String>,
  /// `(ref, tip)` of the stale temporary refs, when asked for (`is_stale_temp_ref`)
  pub stale_temp_refs: Vec<(String, String)>,
  /// The commit of the checked out branch, as the listing found it; `None` for a detached HEAD or a branch without commits
  pub head_tip: Option<String>,
  /// The commit of the baseline, as the listing (or the `rev-parse` of an upstream that is no candidate) found it
  pub baseline_tip: Option<String>,
}

/// The baseline of the checked out `branch` (`None` for a detached HEAD, which tracks nothing) and the branches under
/// `branch_prefix`, local and on `origin`: `remote` and one `for-each-ref` of the branch, the baseline candidates and the
/// two namespaces, and with `sync_refs` the refs of the conflicting branches and the temporary refs. An upstream the
/// listing lacks (not a candidate) costs one `rev-parse` more. The listing reads refs only, no commit
/// (`LISTED_BRANCH_ATOMS`)
pub(crate) fn read_baseline_listing(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch: Option<&str>,
  branch_prefix: Option<&str>,
  sync_refs: bool,
) -> Result<BaselineListing> {
  let remotes: Vec<String> = git_executor
    .execute_command_lines(&["remote"], repo_path)?
    .into_iter()
    .map(|remote| remote.trim().to_string())
    .filter(|remote| !remote.is_empty())
    .collect();
  let first_remote = remotes.first().map(String::as_str);
  let candidates = baseline_candidates(first_remote);

  let current_ref = branch.map(|branch| format!("refs/heads/{branch}"));
  // without a prefix no branch of it is listed: an empty prefix matches no ref
  let (local_prefix, remote_prefix) = branch_prefix.map_or_default(|prefix| {
    let prefix = prefix.trim_end_matches('/');
    (format!("refs/heads/{prefix}/"), format!("refs/remotes/origin/{prefix}/"))
  });
  let format = format!("--format={FORMAT_PREFIX}{LISTED_BRANCH_ATOMS}");
  let mut args = vec!["for-each-ref", format.as_str()];
  args.extend(current_ref.as_deref());
  if branch_prefix.is_some() {
    args.extend([local_prefix.as_str(), remote_prefix.as_str()]);
  }
  args.extend(candidates.iter().map(|(full_ref, _)| full_ref.as_str()));
  if sync_refs {
    args.extend([CONFLICT_REFS, TEMP_REF_PREFIX]);
  }
  let lines = git_executor.execute_command_lines(&args, repo_path)?;

  let mut listed: HashMap<&str, &str> = HashMap::with_capacity(lines.len());
  let mut tracked = None;
  let mut tracked_short = "";
  let mut branches = Vec::new();
  let mut remote_tips = HashMap::new();
  let mut conflict_tips = HashMap::new();
  let mut stale_temp_refs = Vec::new();
  for line in &lines {
    let mut fields = line.splitn(6, '\0');
    let (Some(name), Some(upstream), Some(remote), Some(remote_ref), Some(upstream_short), Some(rest)) =
      (fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    else {
      continue;
    };
    let tip = rest.split('\0').next().unwrap_or_default();
    listed.insert(name, tip);
    if current_ref.as_deref() == Some(name) {
      tracked = Some(TrackedUpstream { upstream, remote, remote_ref });
      tracked_short = upstream_short;
    }
    if branch_prefix.is_some() && name.starts_with(&local_prefix) {
      branches.extend(ListedBranch::parse(&format!("{name}\0{rest}")));
    } else if let Some(full_name) = name.strip_prefix("refs/remotes/origin/")
      && branch_prefix.is_some()
      && name.starts_with(&remote_prefix)
    {
      remote_tips.insert(full_name.to_string(), tip.to_string());
    } else if name.starts_with(CONFLICT_REFS) {
      conflict_tips.insert(name.to_string(), tip.to_string());
    } else if is_stale_temp_ref(name, STALE_TEMP_REF_AGE) {
      stale_temp_refs.push((name.to_string(), tip.to_string()));
    }
  }

  // `@{u}` first, as `detect_baseline_branch` takes it: the upstream if it exists, else the first candidate that does
  let mut resolved_upstream = None;
  let mut upstream_exists = |upstream: &str| -> Result<bool> {
    if listed.contains_key(upstream) {
      return Ok(true);
    }
    resolved_upstream = resolve_commit(git_executor, repo_path, upstream)?;
    Ok(resolved_upstream.is_some())
  };
  let tracked_upstream = tracked.map(|tracked| tracked.upstream).filter(|upstream| !upstream.is_empty());
  let (baseline_ref, baseline) = match tracked_upstream {
    Some(upstream) if first_remote.is_some() && !tracked_short.is_empty() && upstream_exists(upstream)? => (upstream.to_string(), tracked_short.to_string()),
    _ => match candidates.iter().find(|(full_ref, _)| listed.contains_key(full_ref.as_str())) {
      Some((full_ref, name)) => (full_ref.clone(), name.clone()),
      None => return Err(no_baseline(git_executor, repo_path, first_remote.is_some(), candidates)?),
    },
  };
  let head_tip = current_ref.as_deref().and_then(|name| listed.get(name)).map(|tip| (*tip).to_string());
  let baseline_tip = listed.get(baseline_ref.as_str()).map(|tip| (*tip).to_string()).or(resolved_upstream);
  Ok(BaselineListing {
    upstream: baseline_upstream_of(&remotes, &baseline, tracked),
    remotes,
    baseline,
    baseline_ref,
    branches,
    remote_tips,
    conflict_tips,
    stale_temp_refs,
    head_tip,
    baseline_tip,
  })
}

/// The baseline of the checked out branch, as `detect_baseline_branch` names it, and the remote branch behind it, as
/// `resolve_baseline_upstream` finds it (`None` for a local baseline)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
  pub name: String,
  pub upstream: Option<BaselineUpstream>,
  /// The branch HEAD is on, as the read found it; `None` for a detached HEAD
  pub branch: Option<String>,
}

/// The baseline of the checked out branch and its upstream from one read: `symbolic-ref`, `remote` and one
/// `for-each-ref` (`read_baseline_listing`), a `rev-parse` more when HEAD's branch tracks a branch that is no candidate.
/// `detect_baseline_branch` and `resolve_baseline_upstream` one after the other read the remotes twice, the branch, its
/// upstream and the candidates in six processes
///
/// # Errors
/// A `NoBaseline` error when no baseline exists, as `detect_baseline_branch`'s
#[instrument(skip(git_executor))]
pub fn read_baseline(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Baseline> {
  let branch = current_branch(git_executor, repo_path)?;
  let listing = read_baseline_listing(git_executor, repo_path, branch.as_deref(), None, false)?;
  Ok(Baseline {
    name: listing.baseline,
    upstream: listing.upstream,
    branch,
  })
}

/// Read the facts of a sync of `branch_prefix` on the checked out `branch`: the baseline and the branches of
/// `read_baseline_listing`; HEAD and the baseline are the commits it listed, `FETCH_HEAD` and the landed state paths of
/// the directories the operation read (`known_dirs`). Without them, one `rev-parse` of HEAD, the baseline,
/// `FETCH_HEAD` and the landed state
#[instrument(skip(git_executor))]
pub(crate) fn read_repo_facts(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str, branch_prefix: &str) -> Result<RepoFacts> {
  let BaselineListing {
    remotes,
    baseline,
    baseline_ref,
    upstream,
    branches,
    remote_tips,
    conflict_tips,
    stale_temp_refs,
    head_tip,
    baseline_tip,
  } = read_baseline_listing(git_executor, repo_path, Some(branch), Some(branch_prefix), true)?;
  let (fetch_head, landed_state, head, baseline_commit) = match (known_dirs(git_executor, repo_path), head_tip, baseline_tip) {
    (Some(dirs), Some(head), Some(baseline_commit)) => (dirs.git_path("FETCH_HEAD"), dirs.git_path(STATE_FILE), head, baseline_commit),
    _ => read_head_and_baseline(git_executor, repo_path, &baseline, &baseline_ref)?,
  };

  Ok(RepoFacts {
    branch: branch.to_string(),
    upstream,
    remotes,
    baseline,
    baseline_commit,
    head,
    fetched_at: modified_time(&fetch_head),
    landed_state,
    branches,
    remote_tips,
    conflict_tips,
    stale_temp_refs,
  })
}

/// `FETCH_HEAD`, the landed state, HEAD and the baseline from one `rev-parse`: `NoBaseline::NoCommits` for a
/// repository without commits
fn read_head_and_baseline(git_executor: &GitCommandExecutor, repo_path: &str, baseline: &str, baseline_ref: &str) -> Result<(PathBuf, PathBuf, String, String)> {
  let baseline_rev = format!("{baseline_ref}^{{commit}}");
  let (output, exit_code) = git_executor.execute_command_with_status(
    &[
      "rev-parse",
      "--path-format=absolute",
      "--git-path",
      "FETCH_HEAD",
      "--git-path",
      STATE_FILE,
      "HEAD^{commit}",
      &baseline_rev,
    ],
    repo_path,
  )?;
  let mut lines = output.lines();
  let (Some(fetch_head), Some(landed_state), Some(head), Some(baseline_commit), 0) = (lines.next(), lines.next(), lines.next(), lines.next(), exit_code) else {
    // HEAD of a repository without commits doesn't resolve
    let (_, head_exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "HEAD"], repo_path)?;
    if head_exit_code != 0 {
      return Err(NoBaseline::NoCommits.into());
    }
    bail!("Cannot resolve HEAD and {baseline}: {}", output.trim());
  };
  Ok((PathBuf::from(fetch_head), PathBuf::from(landed_state), head.to_string(), baseline_commit.to_string()))
}

#[cfg(test)]
#[path = "repo_facts_test.rs"]
mod repo_facts_test;
