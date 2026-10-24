//! The baseline's remote branch: which one the baseline tracks (`resolve_baseline_upstream`), its fetch without an
//! update (`fetch_baseline`), and the check of the checked out branch against its last fetched state, without the
//! network (`check_baseline_update`, `check_known_baseline_update` of the state a sync read), which every sync runs so
//! that the header shows whether an update would apply cleanly. Update Project (`update_project`), `bd pick`, the lease
//! pool and the reapply read the upstream from here (docs/design/git-operations.md, "Update project").

use crate::config_snapshot::ConfigSnapshot;
use crate::landed::{find_landing, landed_candidates};
use crate::remote_command::execute_remote_command_with_progress;
use crate::repo_facts::{Baseline, read_baseline};
use crate::transfer_progress::TransferProgress;
use anyhow::Result;
use domain_types::landed::LandedBranch;
use domain_types::update_project::UpdateCheck;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{current_branch, known_dirs};
use git_ops::update_branch::{CheckPurpose, CheckedHead, LandedScope, UpdatePlan, UpstreamRef, check_update, check_update_of};
use git_ops::word_resolve::ResolveMode;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::commit_grouper::GroupingInputs;
use history_rewrite::snapshot::read_grouping_inputs_with;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tracing::{instrument, warn};

/// The remote branch the baseline tracks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineUpstream {
  pub remote: String,
  /// The branch on the remote, e.g. `refs/heads/master`
  pub remote_ref: String,
  /// The local remote-tracking ref, e.g. `refs/remotes/origin/master`
  pub tracking_ref: String,
  /// The baseline as it is shown, e.g. `origin/master`
  pub name: String,
}

/// The remote branch behind `baseline_branch` (as `detect_baseline_branch` returns it); `None` for a local baseline.
/// The upstream configuration of the checked out branch gives the remote and the remote branch name when it tracks
/// the baseline; otherwise `<remote>/<branch>` is read with the default refspec.
#[instrument(skip(git_executor))]
pub fn resolve_baseline_upstream(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineUpstream>> {
  let remotes = git_executor.execute_command_lines(&["remote"], repo_path)?;
  if !remotes.iter().any(|remote| remote_of(baseline_branch, remote).is_some()) {
    return Ok(None);
  }
  let tracked = match current_branch(git_executor, repo_path)? {
    Some(branch) => git_executor.execute_command(
      &[
        "for-each-ref",
        "--format=%(upstream)%00%(upstream:remotename)%00%(upstream:remoteref)",
        &format!("refs/heads/{branch}"),
      ],
      repo_path,
    )?,
    None => String::new(),
  };
  let mut fields = tracked.split('\0');
  let tracked = match (fields.next(), fields.next(), fields.next()) {
    (Some(upstream), Some(remote), Some(remote_ref)) => Some(TrackedUpstream { upstream, remote, remote_ref }),
    _ => None,
  };
  Ok(baseline_upstream_of(&remotes, baseline_branch, tracked))
}

/// The upstream configuration of the checked out branch: `%(upstream)`, `%(upstream:remotename)` and
/// `%(upstream:remoteref)` of `for-each-ref`
#[derive(Debug, Clone, Copy)]
pub(crate) struct TrackedUpstream<'a> {
  /// The remote-tracking ref, e.g. `refs/remotes/origin/master`
  pub upstream: &'a str,
  pub remote: &'a str,
  /// The branch on the remote, e.g. `refs/heads/master`
  pub remote_ref: &'a str,
}

/// `resolve_baseline_upstream` of what it read: the remotes and the upstream configuration of the checked out branch
pub(crate) fn baseline_upstream_of(remotes: &[String], baseline_branch: &str, tracked: Option<TrackedUpstream<'_>>) -> Option<BaselineUpstream> {
  // the longest remote name that prefixes the baseline: remote names may contain slashes
  let remote = remotes
    .iter()
    .filter(|remote| remote_of(baseline_branch, remote).is_some())
    .max_by_key(|remote| remote.len())?;
  let tracking_ref = format!("refs/remotes/{baseline_branch}");
  if let Some(tracked) = tracked
    && tracked.upstream == tracking_ref
    && !tracked.remote.is_empty()
    && !tracked.remote_ref.is_empty()
  {
    return Some(BaselineUpstream {
      remote: tracked.remote.to_string(),
      remote_ref: tracked.remote_ref.to_string(),
      tracking_ref,
      name: baseline_branch.to_string(),
    });
  }
  Some(BaselineUpstream {
    remote: remote.clone(),
    remote_ref: format!("refs/heads/{}", &baseline_branch[remote.len() + 1..]),
    tracking_ref,
    name: baseline_branch.to_string(),
  })
}

/// The branch of `baseline_branch` on `remote` (`origin/master` -> `master`), if the baseline is one
fn remote_of<'a>(baseline_branch: &'a str, remote: &str) -> Option<&'a str> {
  baseline_branch.strip_prefix(remote)?.strip_prefix('/').filter(|branch| !branch.is_empty())
}

/// Fetch only the baseline's remote branch into its tracking ref. Other tracking refs stay where they are, so the
/// push lease of the virtual branches (`--force-with-lease`) still compares against what the last push saw.
/// Returns the tip before the fetch if the fetch was a forced update (the upstream was force-pushed).
/// `on_progress` gets how far the fetch is.
///
/// Another fetch of the same tracking ref (`bd update` in a lease beside the app's Update Project or Fetch: one common
/// dir, one `refs/remotes/origin/master`) may hold the ref's lock when this one writes it, and git fails with `cannot
/// lock ref` (C locale, `remote_command`). The other fetch moved the ref by then, so this one is run once more, which
/// finds it up to date. The `fetch.lock` of the operations registry (s10 of `rust-domain-authority.md`) replaces the
/// repeat with a wait.
#[instrument(skip(git_executor, on_progress))]
pub fn fetch_baseline(git_executor: &GitCommandExecutor, repo_path: &str, upstream: &BaselineUpstream, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<Option<String>> {
  let refspec = format!("+{}:{}", upstream.remote_ref, upstream.tracking_ref);
  let args = [
    "fetch",
    "--porcelain",
    "--no-tags",
    "--no-write-fetch-head",
    "--no-recurse-submodules",
    "--no-auto-maintenance",
    &upstream.remote,
    &refspec,
  ];
  let output = match execute_remote_command_with_progress(git_executor, &args, repo_path, &[], on_progress) {
    Err(e) if is_ref_lock_failure(&e) => {
      warn!(error = %format!("{e:#}"), "another fetch held the tracking ref, fetching once more");
      execute_remote_command_with_progress(git_executor, &args, repo_path, &[], on_progress)?
    }
    output => output?,
  };
  Ok(previous_tip_of_forced_update(&output, &upstream.tracking_ref))
}

/// A fetch that failed because another process held the lock of a ref it writes (git's `cannot lock ref`, untranslated
/// under the C locale of every remote command)
fn is_ref_lock_failure(error: &anyhow::Error) -> bool {
  format!("{error:#}").contains("cannot lock ref")
}

/// The old id from the `fetch --porcelain` line of `tracking_ref` if that line is a forced update:
/// `<flag> <old> <new> <ref>`, flag `+` for forced, ` ` for a fast-forward, `*` for a new ref; nothing is printed for a
/// ref that is up to date
pub fn previous_tip_of_forced_update(output: &str, tracking_ref: &str) -> Option<String> {
  output.lines().find_map(|line| {
    let rest = line.strip_prefix("+ ")?;
    let mut fields = rest.split(' ');
    let (old, _new, local_ref) = (fields.next()?, fields.next()?, fields.next()?);
    (local_ref == tracking_ref && !old.bytes().all(|b| b == b'0')).then(|| old.to_string())
  })
}

/// When the repository was last fetched, from the time git last wrote `FETCH_HEAD` (Branch Deck's own fetch writes no
/// `FETCH_HEAD`, its time is returned with the update result)
pub fn last_fetch_time(git_executor: &GitCommandExecutor, repo_path: &str) -> Option<u32> {
  if let Some(dirs) = known_dirs(git_executor, repo_path) {
    return modified_time(&dirs.git_path("FETCH_HEAD"));
  }
  let path = git_executor
    .execute_command(&["rev-parse", "--path-format=absolute", "--git-path", "FETCH_HEAD"], repo_path)
    .ok()?;
  modified_time(Path::new(&path))
}

/// The modification time of a file, Unix seconds; for `FETCH_HEAD`, when the repository was last fetched
pub(crate) fn modified_time(path: &Path) -> Option<u32> {
  let modified = std::fs::metadata(path).and_then(|metadata| metadata.modified()).ok()?;
  u32::try_from(modified.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()
}

/// The check of the checked out branch against the last fetched baseline
#[derive(Debug)]
pub struct BaselineCheck {
  pub check: UpdateCheck,
  /// The local commits the upstream already has: the update drops them
  pub landing: Vec<LandedBranch>,
}

/// Check the checked out branch against the last fetched state of its baseline, without the network.
/// `None` for a local baseline: there is nothing to update from.
pub fn check_baseline_update(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineCheck>> {
  let (grouping, resolve) = read_check_inputs(git_executor, repo_path)?;
  check_baseline_update_with(git_executor, repo_path, baseline_branch, &grouping, resolve)
}

/// The inputs of an update check from one read of the config: the stack setting with the issue groups, and the resolve
/// by words (`branchdeck.resolveByWords`)
pub fn read_check_inputs(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<(GroupingInputs, ResolveMode)> {
  let config = ConfigSnapshot::read(git_executor, repo_path)?;
  Ok((read_grouping_inputs_with(git_executor, repo_path, config.stacks()?)?, config.resolve_mode()?))
}

/// `check_baseline_update` with the stack setting, the issue groups and the resolve mode already read
#[instrument(skip(git_executor, grouping))]
pub fn check_baseline_update_with(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  baseline_branch: &str,
  grouping: &GroupingInputs,
  resolve: ResolveMode,
) -> Result<Option<BaselineCheck>> {
  let Some(upstream) = resolve_baseline_upstream(git_executor, repo_path, baseline_branch)? else {
    return Ok(None);
  };
  let upstream_ref = UpstreamRef {
    tracking_ref: &upstream.tracking_ref,
    name: &upstream.name,
    previous: None,
  };
  let candidates = |scope: LandedScope<'_>| landed_candidates(git_executor, repo_path, scope, grouping);
  let mut plan = check_update(git_executor, repo_path, upstream_ref, CheckPurpose::Preview, &candidates, resolve)?;
  plan.check.fetched_at = last_fetch_time(git_executor, repo_path);
  Ok(Some(with_landing(git_executor, repo_path, plan, grouping)))
}

/// What a sync read at its start for its update check (`RepoFacts`): its guards passed for `branch`, and HEAD and the
/// upstream were at these commits
#[derive(Debug, Clone)]
pub struct KnownBaseline {
  pub branch: String,
  pub head: String,
  pub upstream: BaselineUpstream,
  pub upstream_head: String,
  /// When the repository was last fetched
  pub fetched_at: Option<u32>,
  /// The landed state of this worktree (`landed::STATE_FILE`)
  pub landed_state: PathBuf,
}

/// `check_baseline_update_with` of the state a sync read: no guard, branch, upstream or commit is read again
#[instrument(skip(git_executor, grouping))]
pub fn check_known_baseline_update(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  known: &KnownBaseline,
  grouping: &GroupingInputs,
  resolve: ResolveMode,
) -> Result<BaselineCheck> {
  let upstream_ref = UpstreamRef {
    tracking_ref: &known.upstream.tracking_ref,
    name: &known.upstream.name,
    previous: None,
  };
  let checked = CheckedHead {
    branch: &known.branch,
    head: &known.head,
    upstream_head: &known.upstream_head,
  };
  let candidates = |scope: LandedScope<'_>| landed_candidates(git_executor, repo_path, scope, grouping);
  let mut plan = check_update_of(git_executor, repo_path, checked, upstream_ref, CheckPurpose::Preview, &candidates, resolve)?;
  plan.check.fetched_at = known.fetched_at;
  Ok(with_landing(git_executor, repo_path, plan, grouping))
}

/// The check of `plan` with the local commits the upstream already has
fn with_landing(git_executor: &GitCommandExecutor, repo_path: &str, plan: UpdatePlan, grouping: &GroupingInputs) -> BaselineCheck {
  // the landing is a detail of the check: it must not fail it
  let landing = find_landing(git_executor, repo_path, &plan.check, plan.excluded(), plan.replayed_head(), grouping).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't find the landed commits");
    Vec::new()
  });
  BaselineCheck { check: plan.check, landing }
}

/// The target's baseline and its remote-tracking ref; `None` for no baseline, the ref `None` for a local baseline
pub fn baseline_tracking_ref(git_executor: &GitCommandExecutor, target: &str) -> Result<(Option<String>, Option<String>)> {
  match read_baseline(git_executor, target) {
    Ok(Baseline { name, upstream }) => Ok((Some(name), upstream.map(|upstream| upstream.tracking_ref))),
    Err(e) if e.downcast_ref::<NoBaseline>().is_some() => Ok((None, None)),
    Err(e) => Err(e),
  }
}
