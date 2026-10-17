use crate::baseline::detect_baseline_branch;
use crate::branch_processor::{BranchJob, BranchNotes, BranchSyncContext, BuildOutcome, BuiltBranch, ExistingCommits, RemoteTips, build_branch, finish_branch};
use crate::commit_grouper::{CommitGrouper, GroupedCommits, GroupingInputs, parse_subject};
use crate::commit_key::CommitKey;
use crate::issue_groups::read_grouping_inputs;
use crate::issue_navigation::load_issue_navigation_config;
use crate::landed::record_sync;
use crate::missing_commits::CommitOwners;
use crate::stack::{PendingDetection, describe_block, describe_problem, empty_parent_states, place_branches};
use crate::stack_parents::{drop_integrated_stack_parents, integrated_children};
use crate::update_project::{BaselineCheck, check_baseline_update_with};
use anyhow::{Result, anyhow};
use branch_integration::archive::ARCHIVE_RETENTION_DAYS;
use branch_integration::cache::STACK_NOTES_REF;
use branch_integration::detector::{archive_and_clean_up, detect_archived_branches};
use branch_integration::strategy::DetectionStrategy;
use git_executor::concurrency::max_parallel_git_processes;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_description::read_descriptions;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{Commit, get_commit_list_with_handler};
use git_ops::model::{BranchError, BranchKind, BranchSyncStatus, strip_branch_namespace};
use git_ops::notes::write_commit_notes_with;
use git_ops::rewrite_guard::{find_operation_in_progress, get_current_branch};
use indexmap::IndexMap;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use sync_types::issue_navigation::IssueNavigationConfig;
use sync_types::stack::{BlockReason, BranchStack, StackBlock};
use sync_types::{BranchOrigin, GroupedBranchInfo, ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_number, find_issue_range, has_issue_reference};
use tokio::task::JoinSet;
use tracing::{debug, error, info, instrument, warn};

/// Where the sync takes the issue navigation configuration from
pub enum IssueConfigSource {
  /// Read `.idea/vcs.xml` from the repository
  Load,
  /// Use an already loaded configuration; `None` means the repository has none
  Cached(Option<IssueNavigationConfig>),
}

/// Options for configuring sync behavior
pub struct SyncOptions {
  /// Issue navigation configuration source
  pub issue_config: IssueConfigSource,
  /// Detection strategy for integration detection
  pub detection_strategy: DetectionStrategy,
  /// Archive cleanup retention in days (older fully integrated archived branches are deleted)
  pub archive_retention_days: u64,
  /// Check every mapping note against the tree of its virtual commit, not only by the source it records (`bd --no-cache`)
  pub verify_mapping_notes: bool,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      issue_config: IssueConfigSource::Load,
      detection_strategy: branch_integration::strategy::get_detection_strategy(),
      archive_retention_days: ARCHIVE_RETENTION_DAYS,
      verify_mapping_notes: false,
    }
  }
}

/// The summary of a branch that belongs to an issue: the commit message after the issue reference.
/// Searches in reverse order to skip cleanup commits and find meaningful ones
fn compute_branch_summary(commits: &[Commit]) -> String {
  let mut fallback_summary: Option<String> = None;

  // Search commits in reverse order (oldest first) to find a suitable summary
  for commit in commits.iter().rev() {
    // Strip issue number if present
    let summary = if let Some((_, end)) = find_issue_range(&commit.stripped_subject) {
      commit.stripped_subject[end..].trim_start_matches([' ', ':']).trim()
    } else {
      commit.stripped_subject.as_str()
    };

    // Skip empty summaries
    if summary.is_empty() {
      continue;
    }

    // Check if it's a cleanup/refactor/format commit AFTER stripping issue prefix
    // Use ASCII case-insensitive comparison for efficiency
    // `get` instead of slicing: the byte length of the keyword may fall inside a multi-byte character
    let starts_with_keyword = |keyword: &str| summary.get(..keyword.len()).is_some_and(|start| start.eq_ignore_ascii_case(keyword));
    if starts_with_keyword("cleanup") || starts_with_keyword("refactor") || starts_with_keyword("format") {
      // Save the first non-empty summary as fallback (even if it's cleanup)
      if fallback_summary.is_none() {
        fallback_summary = Some(summary.to_string());
      }
      continue;
    }

    // Found a non-cleanup summary, return it
    return summary.to_string();
  }

  // If all commits are cleanup/refactor/format, use the fallback (first non-empty)
  fallback_summary.unwrap_or_default()
}

/// A branch is an issue branch only while no commit names it with an explicit prefix and every commit carries its
/// name as the key, read as the grouper reads it (after the autosquash markers); a commit of another key came in
/// through the issue groups
fn branch_origin(branch_name: &str, commits: &[Commit]) -> BranchOrigin {
  if commits.iter().any(|c| parse_subject(&c.subject).prefix.is_some()) {
    BranchOrigin::Prefix
  } else if commits.iter().any(|c| find_issue_number(parse_subject(&c.subject).rest) != Some(branch_name)) {
    BranchOrigin::IssueGroup
  } else {
    BranchOrigin::Issue
  }
}

/// The issue of an issue branch is its name, as of a prefix branch named like an issue (`(ABC-123) Fix`); another
/// prefix branch belongs to an issue when all its commits carry the same key
fn branch_issue_key(branch_name: &str, origin: BranchOrigin, commits: &[Commit]) -> Option<String> {
  if origin == BranchOrigin::Issue || find_issue_number(branch_name) == Some(branch_name) {
    return Some(branch_name.to_string());
  }
  let (first, rest) = commits.split_first()?;
  let key = find_issue_number(&first.stripped_subject)?;
  rest.iter().all(|c| find_issue_number(&c.stripped_subject) == Some(key)).then(|| key.to_string())
}

/// Prepare grouped commits for UI display with sorting and metadata; `stacks` places the branches with stacked
/// branches on, `description_titles` (branch -> title) are the titles of their descriptions
pub(crate) fn prepare_branches_for_ui(
  grouped_commits: &IndexMap<String, Vec<Commit>>,
  branch_emails: &HashMap<String, Option<String>>,
  stacks: Option<&IndexMap<String, BranchStack>>,
  description_titles: &HashMap<String, String>,
) -> Vec<GroupedBranchInfo> {
  let mut grouped_branches_for_ui = Vec::with_capacity(grouped_commits.len());

  for (branch_name, commits) in grouped_commits {
    let origin = branch_origin(branch_name, commits);
    let issue_key = branch_issue_key(branch_name, origin, commits);
    let summary = if issue_key.is_some() || has_issue_reference(branch_name) {
      compute_branch_summary(commits)
    } else {
      String::new()
    };

    // Single pass to find latest commit time only (author emails are pre-computed)
    let mut latest_commit_time = 0u32;
    for c in commits {
      // Track latest committer timestamp
      if c.committer_timestamp > latest_commit_time {
        latest_commit_time = c.committer_timestamp;
      }
    }

    // Get pre-computed most frequent author email for this branch
    let branch_my_email = branch_emails.get(branch_name).cloned().flatten();

    grouped_branches_for_ui.push(GroupedBranchInfo {
      name: branch_name.clone(),
      latest_commit_time,
      summary,
      description_title: description_titles.get(branch_name).cloned(),
      all_commits_have_issue_references: {
        if commits.is_empty() {
          false
        } else if origin == BranchOrigin::Issue {
          // If branch is grouped by issue reference, all commits likely have issue references
          // (they were grouped for this reason, so this is a strong heuristic)
          true
        } else {
          // For non-issue branches (like "(feature-auth)"), check each commit
          commits.iter().all(|c| has_issue_reference(&c.stripped_subject))
        }
      },
      my_email: branch_my_email,
      origin,
      issue_key,
      stack: stacks.and_then(|stacks| stacks.get(branch_name)).cloned(),
      commits: commits
        .iter()
        .rev() // Reverse to show newest commits first within branch
        .cloned()
        .collect(),
    });
  }

  // Sort branches by latest commit time (newest first). If committer times are equal (e.g.,
  // after a rebase that rewrites many commits at once), tie-break by latest author time.
  // Finally, use name for a stable ordering. The key is computed once per branch.
  grouped_branches_for_ui.sort_by_cached_key(|branch| {
    let latest_author_time = branch.commits.iter().map(|c| c.author_timestamp).max().unwrap_or(0);
    (Reverse(branch.latest_commit_time), Reverse(latest_author_time), branch.name.clone())
  });

  grouped_branches_for_ui
}

/// Result of the sequential first phase of a sync
struct PreparedSync {
  baseline_branch: String,
  branches: IndexMap<String, Vec<Commit>>,
  branch_emails: HashMap<String, Option<String>>,
  /// Parent of the oldest commit ahead of the baseline; `None` without commits (or when that commit is a root)
  first_parent_id: Option<String>,
  /// Full name -> tip of every branch under the branch prefix (virtual, archived, unapplied); empty without commits
  branch_refs: HashMap<String, String>,
  /// The remote tips and the baseline commit for the remote statuses; `None` without branches
  remote_tips: Option<RemoteTips>,
  /// With stacked branches on, where each branch goes
  stacks: Option<IndexMap<String, BranchStack>>,
  /// With stacked branches on, the children of the stack parents that no branch on this HEAD reaches: the detection of
  /// the archived branches drops the lines of those that are integrated
  idle_stack_parents: Vec<String>,
  /// The update check, started once the baseline is known; sends `BaselineStatus` whenever it is done
  update_check: tokio::task::JoinHandle<Result<()>>,
}

/// Phase 1 (blocking): issue config, baseline, commit grouping. Sends `IssueNavigationConfig`, `UnassignedCommits`
/// and then `BranchesGrouped` (also when empty, so the UI drops stale branches) before any per-branch event can exist.
/// The update check runs beside it and the rest of the sync, from the moment the baseline is known: its
/// `BaselineStatus` can come at any point of the event stream.
/// A sync waits while HEAD is not the branch the user works on: git in the middle of a rebase, merge, cherry-pick or
/// revert, or a detached HEAD. The history of HEAD then lacks the commits of groups, and the sync would archive their
/// virtual branches (docs/design/git-operations.md, "Guards"). Nothing is changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncDeferred {
  #[error("Not synced: {0} is in progress")]
  OperationInProgress(&'static str),
  #[error("Not synced: HEAD is detached")]
  DetachedHead,
}

/// Refuses a sync with `SyncDeferred` while HEAD is not on a branch the user works on; two git processes
pub fn check_sync_guards(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<()> {
  if let Some(operation) = find_operation_in_progress(git_executor, repository_path)? {
    return Err(SyncDeferred::OperationInProgress(operation).into());
  }
  if get_current_branch(git_executor, repository_path).is_err() {
    return Err(SyncDeferred::DetachedHead.into());
  }
  Ok(())
}

fn prepare_sync<P: ProgressReporter + Clone + 'static>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  issue_config: IssueConfigSource,
  detection_strategy: DetectionStrategy,
  archive_retention_days: u64,
  progress: &P,
) -> Result<PreparedSync> {
  // before any event: a deferred sync reports nothing
  check_sync_guards(git_executor, repository_path)?;
  // Use cached issue config if available (including a cached "none"), otherwise load it
  let issue_config = match issue_config {
    IssueConfigSource::Cached(cached) => {
      debug!("Using cached issue navigation config");
      cached
    }
    IssueConfigSource::Load => {
      debug!("Loading issue navigation config");
      load_issue_navigation_config(repository_path)
    }
  };
  progress.send(SyncEvent::IssueNavigationConfig { config: issue_config })?;

  // Detect the baseline branch (origin/master, origin/main, or local master/main)
  let baseline_branch = detect_baseline_branch(git_executor, repository_path, "master")?;
  // the stack setting and the issue groups; the update check groups the landed commits with them too
  let grouping = read_grouping_inputs(git_executor, repository_path)?;
  // on the blocking pool of the runtime this phase runs on
  let update_check = tokio::runtime::Handle::current().spawn_blocking({
    let git_executor = git_executor.clone();
    let repository_path = repository_path.to_string();
    let baseline_branch = baseline_branch.clone();
    let grouping = grouping.clone();
    let progress = progress.clone();
    move || send_baseline_status(&git_executor, &repository_path, baseline_branch, &grouping, &progress)
  });

  // Use streaming commit processing
  let mut grouper = CommitGrouper::new_with(&grouping);
  get_commit_list_with_handler(git_executor, repository_path, &baseline_branch, |commit| {
    grouper.add_commit(commit);
    Ok(())
  })?;

  // The virtual branches are rebuilt on top of the parent of the oldest commit
  let first_parent_id = grouper.first_parent_id.take();
  let commit_count = grouper.commit_count;
  let (
    GroupedCommits {
      branches,
      unassigned,
      branch_emails,
      idle_stack_parents,
      ..
    },
    forest,
  ) = grouper.finish_with_stacks();
  info!(commit_count, total_branches = branches.len(), "Fetched and grouped commits");

  // Always send unassigned commits (even if empty) to ensure the frontend updates; newest first
  progress.send(SyncEvent::UnassignedCommits {
    commits: unassigned.into_iter().rev().collect(),
  })?;

  let (branch_refs, remote_tips, description_titles) = if branches.is_empty() {
    (HashMap::new(), None, HashMap::new())
  } else {
    let (branch_refs, remote) = read_branch_refs(git_executor, repository_path, branch_prefix)?;
    let remote_tips = resolve_baseline(git_executor, repository_path, &baseline_branch)?.map(|baseline| RemoteTips { baseline, remote });
    (branch_refs, remote_tips, read_description_titles(git_executor, repository_path, branch_prefix)?)
  };
  let stacks = match forest {
    Some(forest) => {
      let detection = PendingDetection {
        baseline_branch: &baseline_branch,
        strategy: detection_strategy,
        cache: true,
        retention_days: archive_retention_days,
      };
      let empty_parents = empty_parent_states(git_executor, repository_path, branch_prefix, &branch_refs, &forest, detection)?;
      Some(place_branches(&forest, &empty_parents))
    }
    None => None,
  };

  progress.send(SyncEvent::BranchesGrouped {
    branches: prepare_branches_for_ui(&branches, &branch_emails, stacks.as_ref(), &description_titles),
    baseline_branch: baseline_branch.clone(),
    stacks: stacks.is_some(),
    issue_groups: grouping.issue_groups.entries,
    stack_parents: if grouping.stacks { grouping.stack_parents.entries } else { BTreeMap::new() },
  })?;

  Ok(PreparedSync {
    baseline_branch,
    branches,
    branch_emails,
    first_parent_id,
    branch_refs,
    remote_tips,
    stacks,
    idle_stack_parents,
    update_check,
  })
}

/// Full name -> tip of every branch under `{branch_prefix}/`, and full name -> tip of its remote branch on `origin`,
/// with one `for-each-ref`
fn read_branch_refs(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<(HashMap<String, String>, HashMap<String, String>)> {
  let prefix = branch_prefix.trim_end_matches('/');
  let lines = git_executor.execute_command_lines(
    &[
      "for-each-ref",
      "--format=%(objectname) %(refname)",
      &format!("refs/heads/{prefix}/"),
      &format!("refs/remotes/origin/{prefix}/"),
    ],
    repository_path,
  )?;
  let (mut local, mut remote) = (HashMap::new(), HashMap::new());
  for (tip, name) in lines.iter().filter_map(|line| line.split_once(' ')) {
    if let Some(name) = name.strip_prefix("refs/heads/") {
      local.insert(name.to_string(), tip.to_string());
    } else if let Some(name) = name.strip_prefix("refs/remotes/origin/") {
      remote.insert(name.to_string(), tip.to_string());
    }
  }
  Ok((local, remote))
}

/// The commit of the baseline, for the remote statuses (`RemoteTips`); `None` if it does not resolve
fn resolve_baseline(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> Result<Option<String>> {
  let (commit, exit_code) = git_executor.execute_command_with_status(
    &["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{baseline_branch}^{{commit}}")],
    repository_path,
  )?;
  Ok((exit_code == 0).then_some(commit))
}

/// Branch -> the title of its description (`bd describe`), for each virtual branch under `{branch_prefix}/` that has
/// one, with one `git config --get-regexp`
fn read_description_titles(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str) -> Result<HashMap<String, String>> {
  Ok(
    read_descriptions(git_executor, repository_path)?
      .into_iter()
      .filter_map(|(reference, description)| {
        let name = strip_branch_namespace(reference.strip_prefix("refs/heads/")?, branch_prefix, BranchKind::Virtual)?;
        (!description.title.is_empty()).then(|| (name.to_string(), description.title))
      })
      .collect(),
  )
}

/// What a branch task of `process_branches` returns
enum TaskOutput {
  Built(String, Result<BuildOutcome>),
  Finished(String, Result<BranchNotes>),
}

/// The branches of one sync and what they wait for, as `process_branches` schedules them
struct Schedule {
  /// Not started yet, by branch index
  jobs: HashMap<usize, BranchJob>,
  /// Branch -> the indexes of the branches built on its new head
  children: HashMap<String, Vec<usize>>,
  /// Branch index -> length of the longest chain of branches waiting on it, the branch included
  chain_lengths: HashMap<usize, usize>,
  /// Started first: the longest chain, then the oldest branch
  ready: BinaryHeap<(usize, Reverse<usize>)>,
}

impl Schedule {
  fn ready(&mut self, index: usize) {
    self.ready.push((self.chain_lengths[&index], Reverse(index)));
  }

  /// The children of `branch`, with its new `head` as their base, are ready
  fn release_children(&mut self, branch: &str, head: &str) {
    for index in self.children.remove(branch).unwrap_or_default() {
      if let Some(job) = self.jobs.get_mut(&index) {
        head.clone_into(&mut job.base_commit);
        job.on_parent = true;
      }
      self.ready(index);
    }
  }

  /// The descendants of `branch` are not rebuilt; returns them with what blocks each one
  fn block_descendants(&mut self, branch: &str, reason: BlockReason) -> Vec<(BranchJob, StackBlock)> {
    let mut blocked = Vec::new();
    let mut pending = vec![(branch.to_string(), reason)];
    while let Some((parent, reason)) = pending.pop() {
      for index in self.children.remove(&parent).unwrap_or_default() {
        if let Some(job) = self.jobs.remove(&index) {
          pending.push((job.branch_name.clone(), BlockReason::Blocked));
          blocked.push((job, StackBlock { parent: parent.clone(), reason }));
        }
      }
    }
    blocked
  }
}

/// The identities of the commits of the branches each stacked branch is built on (its effective base, then that one's,
/// down to the baseline), for its remote status: a remote commit of the branch with one of them is an older version of
/// an ancestor's commit, restacked, not dropped. Only branches with an effective base are in the map. From the commits
/// the sync grouped, so no git process.
fn ancestor_keys(branches: &IndexMap<String, Vec<Commit>>, stacks: Option<&IndexMap<String, BranchStack>>) -> HashMap<String, HashSet<CommitKey>> {
  let Some(stacks) = stacks else { return HashMap::new() };
  let base_of = |name: &str| stacks.get(name).and_then(|stack| stack.effective_base.as_deref());
  let mut result = HashMap::new();
  for name in branches.keys() {
    let mut parent = base_of(name);
    if parent.is_none() {
      continue;
    }
    let mut keys = HashSet::new();
    let mut seen = HashSet::from([name.as_str()]);
    while let Some(ancestor) = parent
      && seen.insert(ancestor)
    {
      let commits = branches.get(ancestor).map_or(&[][..], Vec::as_slice);
      keys.extend(
        commits
          .iter()
          .map(|commit| CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message)),
      );
      parent = base_of(ancestor);
    }
    result.insert(name.clone(), keys);
  }
  result
}

/// Report a branch that is not rebuilt this sync: its status and its commits as blocked
fn report_not_built(progress: &dyn ProgressReporter, job: &BranchJob, status: BranchSyncStatus, reason: String) {
  let _ = progress.send(SyncEvent::CommitsBlocked {
    branch_name: job.branch_name.clone(),
    blocked_commit_hashes: job.commits.iter().map(|commit| commit.id.clone()).collect(),
  });
  let _ = progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: job.branch_name.clone(),
    status,
    error: Some(BranchError::Generic(reason)),
  });
}

/// Rebuild every virtual branch on the blocking pool, at most `max_parallel_git_processes()` tasks at a time, then
/// write the mapping notes and the stack notes of all of them in one `fast-import` run. A stacked branch starts once its parent's head is
/// written, built on that head; a branch waiting for its parent holds no permit. A parent that fails blocks its
/// descendants. Returns the first failure, naming its branch.
async fn process_branches<P: ProgressReporter + Clone + 'static>(
  context: BranchSyncContext<P>,
  branches: IndexMap<String, Vec<Commit>>,
  mut branch_emails: HashMap<String, Option<String>>,
  base_commit: String,
  stacks: Option<IndexMap<String, BranchStack>>,
) -> Result<()> {
  let context = Arc::new(context);
  let max_running = max_parallel_git_processes();
  let mut schedule = Schedule {
    jobs: HashMap::with_capacity(branches.len()),
    children: HashMap::new(),
    chain_lengths: HashMap::with_capacity(branches.len()),
    ready: BinaryHeap::new(),
  };

  let mut waiting_on: HashMap<usize, String> = HashMap::new();
  for (branch_idx, (branch_name, commits)) in branches.into_iter().enumerate() {
    let stack = stacks.as_ref().and_then(|stacks| stacks.get(&branch_name));
    let job = BranchJob {
      my_email: branch_emails.remove(&branch_name).flatten(),
      branch_name,
      commits,
      branch_idx,
      base_commit: base_commit.clone(),
      on_parent: false,
    };
    if let Some(stack) = stack {
      if let Some(problem) = stack.problems.first() {
        report_not_built(&context.progress, &job, BranchSyncStatus::Error, describe_problem(problem));
        continue;
      }
      if let Some(block) = &stack.blocked_by {
        report_not_built(&context.progress, &job, BranchSyncStatus::Blocked, describe_block(block));
        continue;
      }
      if let Some(parent) = &stack.effective_base {
        waiting_on.insert(branch_idx, parent.clone());
        schedule.children.entry(parent.clone()).or_default().push(branch_idx);
      }
    }
    schedule.jobs.insert(branch_idx, job);
  }

  // the chain lengths, leaves first; a branch's children all come after it in the stack order
  let index_of: HashMap<String, usize> = schedule.jobs.iter().map(|(index, job)| (job.branch_name.clone(), *index)).collect();
  let mut by_depth: Vec<usize> = schedule.jobs.keys().copied().collect();
  let depth = |mut index: usize| {
    let mut depth = 0;
    while let Some(parent) = waiting_on.get(&index).and_then(|parent| index_of.get(parent)) {
      depth += 1;
      index = *parent;
    }
    depth
  };
  by_depth.sort_by_key(|index| Reverse(depth(*index)));
  for index in by_depth {
    let length = 1
      + schedule.children.get(&schedule.jobs[&index].branch_name).map_or(0, |children| {
        children.iter().map(|child| schedule.chain_lengths.get(child).copied().unwrap_or(1)).max().unwrap_or(0)
      });
    schedule.chain_lengths.insert(index, length);
  }
  let roots: Vec<usize> = schedule.jobs.keys().copied().filter(|index| !waiting_on.contains_key(index)).collect();
  for index in roots {
    schedule.ready(index);
  }

  let mut tasks = JoinSet::new();
  let mut to_finish: Vec<BuiltBranch> = Vec::new();
  let mut first_error = None;
  let mut notes = Vec::new();
  let mut stack_notes = Vec::new();
  loop {
    // builds first: branches wait for their parents' heads, not for their refs
    while tasks.len() < max_running {
      if let Some((_, Reverse(index))) = schedule.ready.pop() {
        let Some(job) = schedule.jobs.remove(&index) else { continue };
        let context = Arc::clone(&context);
        tasks.spawn_blocking(move || {
          let branch_name = job.branch_name.clone();
          TaskOutput::Built(branch_name, build_branch(&context, job))
        });
      } else if let Some(built) = to_finish.pop() {
        let context = Arc::clone(&context);
        tasks.spawn_blocking(move || {
          let branch_name = built.branch_name().to_string();
          TaskOutput::Finished(branch_name, finish_branch(&context, built))
        });
      } else {
        break;
      }
    }

    let Some(joined) = tasks.join_next().await else { break };
    let failed = match joined {
      Ok(TaskOutput::Built(branch_name, Ok(BuildOutcome::Built(built)))) => {
        schedule.release_children(&branch_name, &built.head);
        to_finish.push(*built);
        None
      }
      Ok(TaskOutput::Built(branch_name, Ok(BuildOutcome::Failed(reason)))) => Some((branch_name, reason)),
      Ok(TaskOutput::Built(branch_name, Err(e)) | TaskOutput::Finished(branch_name, Err(e))) => {
        error!(branch = %branch_name, error = ?e, "Branch processing failed");
        first_error.get_or_insert_with(|| e.context(format!("Failed to sync branch {branch_name}")));
        Some((branch_name, BlockReason::Error))
      }
      Ok(TaskOutput::Finished(_, Ok(branch_notes))) => {
        notes.extend(branch_notes.mappings);
        stack_notes.extend(branch_notes.stack_note);
        None
      }
      Err(e) => {
        error!(error = %e, "Branch processing task failed");
        first_error.get_or_insert_with(|| anyhow!("Branch processing task failed: {e}"));
        None
      }
    };
    if let Some((branch_name, reason)) = failed {
      for (job, block) in schedule.block_descendants(&branch_name, reason) {
        report_not_built(&context.progress, &job, BranchSyncStatus::Blocked, describe_block(&block));
      }
    }
  }
  // a branch whose parent's task panicked is still waiting
  for job in schedule.jobs.values() {
    report_not_built(&context.progress, job, BranchSyncStatus::Blocked, "Blocked: the parent branch failed".to_string());
  }

  // the notes only save copying the commits again; the copies are deterministic, so a lost note costs time, not ids
  // (a copy older than the deterministic dates is kept by checking it against its tree, see `copy_commit`)
  // a stacked branch's base lets the integration detection of its archived branch look at its own commits only
  let notes_result = tokio::task::spawn_blocking(move || write_commit_notes_with(&context.git_executor, &context.repository_path, &notes, Some((STACK_NOTES_REF, &stack_notes))))
    .await
    .map_err(|e| anyhow!("Writing the commit notes failed: {e}"))?;
  match first_error {
    Some(e) => Err(e),
    None => notes_result.map_err(|e| e.context("Failed to write commit notes")),
  }
}

/// Sync virtual branches: group the commits ahead of the baseline by prefix, rebuild one virtual branch per group,
/// and detect integration of archived branches concurrently. All git work runs on the blocking pool.
#[instrument(skip(git_executor, progress, options), fields(repository_path = %repository_path, branch_prefix = %branch_prefix, cached_issue_config = matches!(options.issue_config, IssueConfigSource::Cached(_))))]
pub async fn sync_branches<P: ProgressReporter + Clone + 'static>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  progress: P,
  options: SyncOptions,
) -> Result<()> {
  let SyncOptions {
    issue_config,
    detection_strategy,
    archive_retention_days,
    verify_mapping_notes,
  } = options;
  // every git process of the sync, its tasks included, is counted; the summary is logged at its end
  let git_executor = &git_executor.with_process_stats("sync");

  // Phase 1: sequential, BranchesGrouped is on the wire before any per-branch task starts
  let PreparedSync {
    baseline_branch,
    branches,
    branch_emails,
    first_parent_id,
    branch_refs,
    remote_tips,
    stacks,
    idle_stack_parents,
    update_check,
  } = tokio::task::spawn_blocking({
    let git_executor = git_executor.clone();
    let repository_path = repository_path.to_string();
    let branch_prefix = branch_prefix.to_string();
    let progress = progress.clone();
    move || {
      prepare_sync(
        &git_executor,
        &repository_path,
        &branch_prefix,
        issue_config,
        detection_strategy,
        archive_retention_days,
        &progress,
      )
    }
  })
  .await
  .map_err(|e| anyhow!("Sync preparation task failed: {e}"))??;

  // The archive step before any virtual branch is rebuilt: a rebuilt `(foo/bar)` can't be created while the virtual
  // branch of the former `(foo)` still exists (git's directory/file rule). Only ref writes, fast. Its failure fails
  // the detection only, the branches are still rebuilt.
  let active_branches: HashSet<String> = branches.keys().cloned().collect();
  let archived = tokio::task::spawn_blocking({
    let git_executor = git_executor.clone();
    let repository_path = repository_path.to_string();
    let branch_prefix = branch_prefix.to_string();
    let progress = progress.clone();
    move || archive_and_clean_up(&git_executor, &repository_path, &branch_prefix, &active_branches, archive_retention_days, &progress)
  })
  .await
  .map_err(|e| anyhow!("Archive task failed: {e}"))
  .flatten();

  // Phase 2: integration detection and branch processing run concurrently, beside the update check
  let detection = tokio::task::spawn_blocking({
    let git_executor = git_executor.clone();
    let repository_path = repository_path.to_string();
    let branch_prefix = branch_prefix.to_string();
    let baseline_branch = baseline_branch.clone();
    let progress = progress.clone();
    move || {
      let reported = detect_archived_branches(
        &git_executor,
        &repository_path,
        &branch_prefix,
        &baseline_branch,
        archived?,
        detection_strategy,
        archive_retention_days,
        &progress,
      )?;
      // an integrated branch stacks nothing anywhere any more
      let integrated = integrated_children(&idle_stack_parents, &reported, &branch_prefix);
      drop_integrated_stack_parents(&git_executor, &repository_path, &integrated);
      Ok(())
    }
  });

  let processing = async {
    if branches.is_empty() {
      info!("No commits ahead of baseline, only checking for integrated branches");
      return Ok(());
    }
    let parent_commit_hash = first_parent_id.ok_or_else(|| anyhow!("The oldest commit ahead of the baseline has no parent"))?;
    let existing_tips = virtual_tips(branch_refs, branch_prefix);
    let context = BranchSyncContext {
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      baseline_branch,
      total_branches: branches.len(),
      progress,
      git_executor: git_executor.clone(),
      tree_id_cache: seeded_tree_cache(TreeIdCache::new(), branches.values().flatten()),
      existing_commits: Some(ExistingCommits::new(&parent_commit_hash, existing_tips.values().cloned())),
      existing_tips,
      remote_tips,
      ancestor_keys: ancestor_keys(&branches, stacks.as_ref()),
      verify_mapping_notes,
      owners: CommitOwners::new(&branches, |name| {
        stacks.as_ref().and_then(|stacks| stacks.get(name)).and_then(|stack| stack.effective_base.as_deref())
      }),
    };
    process_branches(context, branches, branch_emails, parent_commit_hash, stacks).await
  };

  // join (not try_join) so a failure on one side doesn't orphan the other
  let (processing_result, detection_result, update_check_result) = tokio::join!(processing, detection, update_check);
  processing_result?;
  update_check_result.map_err(|e| anyhow!("Update check task failed: {e}"))??;
  detection_result.map_err(|e| anyhow!("Integration detection task failed: {e}"))?
}

/// `cache` with the trees the commit list read: the parent of a commit ahead of the baseline is another such commit, so
/// checking a mapping note's source asks git for no tree
pub(crate) fn seeded_tree_cache<'a>(cache: TreeIdCache, commits: impl Iterator<Item = &'a Commit>) -> TreeIdCache {
  for commit in commits {
    if !commit.tree_id.is_empty() {
      cache.insert(&commit.id, &commit.tree_id);
    }
  }
  cache
}

/// The tips of the virtual branches among `branch_refs`
fn virtual_tips(branch_refs: HashMap<String, String>, branch_prefix: &str) -> HashMap<String, String> {
  branch_refs
    .into_iter()
    .filter(|(name, _)| strip_branch_namespace(name, branch_prefix, BranchKind::Virtual).is_some())
    .collect()
}

/// Check the update of the checked out branch onto the baseline, record what landed since the previous sync and send
/// `BaselineStatus`. A failed check is logged and reported in the event, it never fails the sync; only a failure to send
/// the event does.
fn send_baseline_status(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  baseline_branch: String,
  grouping: &GroupingInputs,
  progress: &dyn ProgressReporter,
) -> Result<()> {
  let start = std::time::Instant::now();
  let result = check_baseline_update_with(git_executor, repository_path, &baseline_branch, grouping);
  info!(elapsed_ms = start.elapsed().as_millis(), ok = result.is_ok(), "update check done");
  let (baseline_check, check_error) = match result {
    Ok(baseline_check) => (baseline_check, None),
    Err(e) => {
      warn!(error = %format!("{e:#}"), "update check failed");
      (None, Some(format!("{e:#}")))
    }
  };
  let landed = match &baseline_check {
    Some(BaselineCheck { check, .. }) => record_sync(git_executor, repository_path, check, grouping).unwrap_or_else(|e| {
      warn!(error = %format!("{e:#}"), "can't record the landed commits");
      Vec::new()
    }),
    None => Vec::new(),
  };
  let (update, landing) = baseline_check.map_or((None, Vec::new()), |BaselineCheck { check, landing }| (Some(Box::new(check)), landing));
  progress.send(SyncEvent::BaselineStatus {
    baseline_branch,
    update,
    check_error,
    landing,
    landed,
  })
}

#[cfg(test)]
#[path = "stack_sync_test.rs"]
mod stack_sync_test;
