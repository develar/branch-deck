//! The baseline's remote branch: which one the baseline tracks (`resolve_baseline_upstream`), its fetch without an
//! update (`fetch_baseline`), and the check of the checked out branch against its last fetched state, without the
//! network (`check_baseline_update`, `check_known_baseline_update` of the state a sync read), which every sync runs so
//! that the header shows whether an update would apply cleanly. Update Project (`update_project`), `brd pick`, the lease
//! pool and the reapply read the upstream from here (docs/design/git-operations.md, "Update project").

use crate::config_snapshot::ConfigSnapshot;
use crate::fetch_lock::{FetchTurn, take_fetch_turn};
use crate::landed::find_landing;
use crate::remote_command::execute_remote_command_with_progress;
use crate::repo_facts::{Baseline, Remotes, read_baseline, read_remotes};
use crate::transfer_progress::TransferProgress;
use anyhow::Result;
use domain_types::landed::LandedBranch;
use domain_types::pull_request::MergedPullRequests;
use domain_types::update_project::UpdateCheck;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{current_branch, known_dirs};
use git_ops::update_branch::{CheckPurpose, CheckedHead, LandedLookup, UpdatePlan, UpstreamRef, check_update, check_update_of, with_attribute_files};
use git_ops::update_resolve::LostBindingRule;
use git_ops::word_resolve::ResolveMode;
use history_rewrite::baseline::NoBaseline;
use history_rewrite::commit_grouper::GroupingInputs;
use history_rewrite::snapshot::read_grouping_inputs;
use std::path::Path;
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
pub fn resolve_baseline_upstream(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineUpstream>> {
  Ok(resolve_baseline_upstream_with(git_executor, repo_path, baseline_branch)?.0)
}

/// `resolve_baseline_upstream` with the remotes it read (`read_remotes`: `remote -v`, the one process `remote` would
/// cost), whose URLs tell the remote of the baseline (`patronus::required_target_branch`)
#[instrument(skip(git_executor))]
pub fn resolve_baseline_upstream_with(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<(Option<BaselineUpstream>, Remotes)> {
  let remotes = read_remotes(git_executor, repo_path)?;
  if !remotes.names.iter().any(|remote| remote_of(baseline_branch, remote).is_some()) {
    return Ok((None, remotes));
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
  Ok((baseline_upstream_of(&remotes.names, baseline_branch, tracked), remotes))
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
/// The fetch holds the fetch lock of the repository (`fetch_lock`): beside another fetch of the same tracking ref, of
/// any process (`brd update` in a lease beside the app's Update Project or Fetch: one common dir, one
/// `refs/remotes/origin/master`), it waits while that one's holder lives, `on_progress` naming the holder, and takes its
/// result when that one ended well or the ref moved meanwhile: one transfer for both. `canceled` stops the wait
/// (`LockWaitCanceled`): the app's operation that the user stopped
#[instrument(skip(git_executor, on_progress, canceled))]
pub fn fetch_baseline(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  upstream: &BaselineUpstream,
  on_progress: &mut dyn FnMut(TransferProgress),
  canceled: &dyn Fn() -> bool,
) -> Result<Option<String>> {
  let own = match take_fetch_turn(git_executor, repo_path, upstream, on_progress, canceled)? {
    FetchTurn::Joined { previous } => return Ok(previous),
    FetchTurn::Own(own) => own,
  };
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
  let output = execute_remote_command_with_progress(git_executor, &args, repo_path, &[], on_progress)?.into_text();
  own.fetched();
  let previous = previous_tip_of_forced_update(&output, &upstream.tracking_ref);
  // the remote rewrote the baseline: a history unrelated to HEAD defers the next sync (`SyncDeferred::UnrelatedHistory`)
  if let Some(previous) = &previous {
    warn!(previous, tracking_ref = %upstream.tracking_ref, "the baseline was force-pushed");
  }
  Ok(previous)
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

/// Check the checked out branch against the last fetched state of its baseline, without the network, and without the
/// lost bindings of its replay (`check_baseline_update_checking`).
/// `None` for a local baseline: there is nothing to update from.
pub fn check_baseline_update(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Option<BaselineCheck>> {
  check_baseline_update_checking(git_executor, repo_path, baseline_branch, None)
}

/// `check_baseline_update` with the lost-binding rule (`brd update --dry-run`): a clean rebase whose replay loses a
/// binding is a conflict, as the update answers it
pub fn check_baseline_update_checking(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  baseline_branch: &str,
  lost_bindings: Option<&'static dyn LostBindingRule>,
) -> Result<Option<BaselineCheck>> {
  let config = ConfigSnapshot::read(git_executor, repo_path)?;
  let grouping = read_grouping_inputs(git_executor, repo_path)?;
  let git_executor = with_attribute_files(git_executor, repo_path, config.attributes_file());
  let inputs = CheckInputs {
    grouping: &grouping,
    resolve: config.resolve_mode()?,
    lost_bindings,
    merged: None,
  };
  check_baseline_update_with(&git_executor, repo_path, baseline_branch, inputs)
}

/// What the update check of a sync takes besides the state: the issue groups, the resolve by words, the lost-binding
/// rule, and the merged pull requests its pairing asks after a conflict (`LandedLookup::by_key_with`)
#[derive(Clone, Copy)]
pub struct CheckInputs<'a> {
  pub grouping: &'a GroupingInputs,
  pub resolve: ResolveMode,
  pub lost_bindings: Option<&'static dyn LostBindingRule>,
  /// The merged pull requests of the session (`SyncOptions::merged_pull_requests`); `None` pairs without them
  pub merged: Option<&'a dyn MergedPullRequests>,
}

/// The inputs of an update check from one read of the config: the issue groups, and the resolve by words (`branchdeck.resolveByWords`)
pub fn read_check_inputs(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<(GroupingInputs, ResolveMode)> {
  let config = ConfigSnapshot::read(git_executor, repo_path)?;
  Ok((read_grouping_inputs(git_executor, repo_path)?, config.resolve_mode()?))
}

/// `check_baseline_update` with the issue groups and the resolve mode already read, and the files of attributes on
/// `git_executor` (`with_attribute_files`) for its probe to answer in the next check
#[instrument(skip(git_executor, inputs))]
pub fn check_baseline_update_with(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str, inputs: CheckInputs<'_>) -> Result<Option<BaselineCheck>> {
  let Some(upstream) = resolve_baseline_upstream(git_executor, repo_path, baseline_branch)? else {
    return Ok(None);
  };
  let upstream_ref = UpstreamRef {
    tracking_ref: &upstream.tracking_ref,
    name: &upstream.name,
    previous: None,
    local: false,
  };
  let landed = LandedLookup::by_key_with(inputs.merged);
  let mut plan = check_update(git_executor, repo_path, upstream_ref, CheckPurpose::Preview, landed, inputs.resolve)?;
  plan.check.fetched_at = last_fetch_time(git_executor, repo_path);
  check_lost_bindings(git_executor, repo_path, &mut plan, inputs.lost_bindings);
  Ok(Some(with_landing(git_executor, repo_path, plan, inputs.grouping)))
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
  /// The files of the lease of this worktree, with its landed state, which only `lease_state` writes
  pub(crate) lease: crate::lease_state::LeasePaths,
}

/// `check_baseline_update_with` of the state a sync read: no guard, branch, upstream or commit is read again
#[instrument(skip(git_executor, inputs))]
pub fn check_known_baseline_update(git_executor: &GitCommandExecutor, repo_path: &str, known: &KnownBaseline, inputs: CheckInputs<'_>) -> Result<BaselineCheck> {
  let upstream_ref = UpstreamRef {
    tracking_ref: &known.upstream.tracking_ref,
    name: &known.upstream.name,
    previous: None,
    local: false,
  };
  let checked = CheckedHead {
    branch: &known.branch,
    head: &known.head,
    upstream_head: &known.upstream_head,
  };
  let landed = LandedLookup::by_key_with(inputs.merged);
  let mut plan = check_update_of(git_executor, repo_path, checked, upstream_ref, CheckPurpose::Preview, landed, inputs.resolve)?;
  plan.check.fetched_at = known.fetched_at;
  check_lost_bindings(git_executor, repo_path, &mut plan, inputs.lost_bindings);
  Ok(with_landing(git_executor, repo_path, plan, inputs.grouping))
}

/// The lost bindings of the replay of a preview (`UpdatePlan::check_lost_bindings`): a finding makes it the conflict the
/// update would answer. Like the landing, a detail of the check that must not fail it: a failure is logged, the check
/// stays as the replay found it
fn check_lost_bindings(git_executor: &GitCommandExecutor, repo_path: &str, plan: &mut UpdatePlan, rule: Option<&'static dyn LostBindingRule>) {
  let Some(rule) = rule else { return };
  if let Err(e) = plan.check_lost_bindings(git_executor, repo_path, rule, &[]) {
    warn!(error = %format!("{e:#}"), "can't read the lost bindings of the update");
  }
}

/// The check of `plan` with the local commits the upstream already has
fn with_landing(git_executor: &GitCommandExecutor, repo_path: &str, plan: UpdatePlan, grouping: &GroupingInputs) -> BaselineCheck {
  // the landing is a detail of the check: it must not fail it
  let landing = find_landing(git_executor, repo_path, &plan, grouping).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't find the landed commits");
    Vec::new()
  });
  BaselineCheck { check: plan.check, landing }
}

/// The target's baseline and its remote-tracking ref; `None` for no baseline, the ref `None` for a local baseline
pub fn baseline_tracking_ref(git_executor: &GitCommandExecutor, target: &str) -> Result<(Option<String>, Option<String>)> {
  match read_baseline(git_executor, target) {
    Ok(Baseline { name, upstream, .. }) => Ok((Some(name), upstream.map(|upstream| upstream.tracking_ref))),
    Err(e) if e.downcast_ref::<NoBaseline>().is_some() => Ok((None, None)),
    Err(e) => Err(e),
  }
}
