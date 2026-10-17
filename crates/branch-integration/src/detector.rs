use super::cache::{CacheOps, CachedDetection, Recheck, STACK_NOTES_REF};
use super::squash::SquashCheck;
use super::{common, merge, rebase, strategy::DetectionStrategy};
use anyhow::{Result, anyhow};
use git_executor::concurrency::max_parallel_git_processes;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::{BranchKind, strip_branch_namespace};
use git_ops::notes::read_notes;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus};
use sync_types::{ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_number, find_issue_range};
use sync_utils::timestamp::{days_ago, parse_utc_date};
use tracing::{debug, info, instrument, trace, warn};

/// Configuration for integration detection
#[derive(Clone, Copy)]
pub struct DetectConfig<'a> {
  /// Simple names of the virtual branches that still have commits ahead of the baseline
  pub active_branches: &'a HashSet<String>,
  pub progress: &'a dyn ProgressReporter,
  pub strategy: DetectionStrategy,
  pub retention_days: u64,
}

/// A tip to detect: one without a usable cached result, or a `NotIntegrated` one to check again
struct DetectionJob {
  /// The branches with this tip (two archived names can share one), each reported with the one result
  branches: Vec<String>,
  tip: String,
  is_merged: bool,
  /// The commit a stacked branch was built on (its stack note), so only its own commits count
  stack_base: Option<String>,
  /// The cached `NotIntegrated` result and its baseline, an ancestor of the current one: only the baseline commits since
  /// then are compared; `None` for a full detection
  previous: Option<(BranchIntegrationStatus, Recheck)>,
  /// The branch was archived within the retention period (a virtual branch always is): the slow checks of a recheck,
  /// the squash fallback, run only for such a branch
  within_retention: bool,
}

/// What to do with the cached result of a tip for the current baseline
pub enum CachePlan<'a> {
  /// The cached result still holds
  Holds(&'a CachedDetection),
  /// Detect it, incrementally with the cached `NotIntegrated` result and its baseline, or in full with `None`
  Detect(Option<(BranchIntegrationStatus, Recheck)>),
}

/// Whether the baseline moved on from the baselines recorded in `NotIntegrated` notes: one `merge-base --is-ancestor`
/// per distinct recorded baseline
pub struct BaselineAncestry<'a> {
  git: &'a GitCommandExecutor,
  repo: &'a str,
  baseline: &'a str,
  known: HashMap<String, bool>,
}

impl<'a> BaselineAncestry<'a> {
  /// `baseline` is a commit id (`resolve_baseline`)
  pub fn new(git: &'a GitCommandExecutor, repo: &'a str, baseline: &'a str) -> Self {
    Self {
      git,
      repo,
      baseline,
      known: HashMap::new(),
    }
  }

  /// Whether the baseline descends from `since`, so a recheck against the commits since then is complete
  fn moved_on_from(&mut self, since: &str) -> bool {
    if !self.known.contains_key(since) {
      let descends = is_ancestor(self.git, self.repo, since, self.baseline);
      self.known.insert(since.to_string(), descends);
    }
    self.known[since]
  }

  /// What to do with the note `cached` of a tip. An `Integrated` or `Partial` result holds. A `NotIntegrated` one holds
  /// for the baseline it was checked against, is checked again incrementally when the baseline moved on from it, and
  /// otherwise (a note of version 1, a rewound or renamed baseline, one gone in a gc) is detected in full once, but only
  /// `within_retention`, because a full scan of an old archive is slow; an older such note holds as it is.
  pub fn plan<'c>(&mut self, cached: Option<&'c CachedDetection>, within_retention: bool) -> CachePlan<'c> {
    let Some(cached) = cached else {
      return CachePlan::Detect(None);
    };
    match (&cached.info.status, &cached.check) {
      (BranchIntegrationStatus::NotIntegrated { .. }, Some(check)) if check.baseline == self.baseline => CachePlan::Holds(cached),
      (BranchIntegrationStatus::NotIntegrated { .. }, Some(check)) if self.moved_on_from(&check.baseline) => CachePlan::Detect(Some((cached.info.status.clone(), check.clone()))),
      (BranchIntegrationStatus::NotIntegrated { .. }, _) if within_retention => CachePlan::Detect(None),
      _ => CachePlan::Holds(cached),
    }
  }
}

/// Whether a branch archived on `date` (`YYYY-MM-DD` of `<prefix>/archived/<date>/<name>`) is within the archive
/// retention period of `retention_days` (`ARCHIVE_RETENTION_DAYS` by default)
pub fn archived_within_retention(date: &str, retention_days: u64) -> bool {
  let Some(archived) = parse_utc_date(date) else {
    return false;
  };
  archived >= days_ago(i64::try_from(retention_days).unwrap_or(i64::MAX))
}

/// The commit `baseline` names now: detection compares with it and records it in a `NotIntegrated` note, so the scanned
/// range and the recorded baseline are the same commit even if a fetch moves the branch meanwhile
pub fn resolve_baseline(git: &GitCommandExecutor, repo: &str, baseline: &str) -> Result<String> {
  let output = git.execute_command(&["rev-parse", "--verify", "--end-of-options", &format!("{baseline}^{{commit}}")], repo)?;
  Ok(output.trim().to_string())
}

/// Whether `ancestor` is an ancestor of `baseline`; false for a commit that no longer exists (one process)
fn is_ancestor(git: &GitCommandExecutor, repo: &str, ancestor: &str, baseline: &str) -> bool {
  match git.execute_command_with_status(&["merge-base", "--is-ancestor", "--end-of-options", ancestor, baseline], repo) {
    Ok((_, 0)) => true,
    Ok(_) => false,
    Err(e) => {
      debug!(ancestor = %ancestor, error = %e, "Cannot check the baseline of a detection note");
      false
    }
  }
}

/// The day an archived branch was archived, from `<prefix>/archived/YYYY-MM-DD/<name>`
fn archive_date(branch: &str, branch_prefix: &str) -> Option<i64> {
  parse_utc_date(strip_branch_namespace(branch, branch_prefix, BranchKind::Archived)?.split('/').next()?)
}

/// The stack bases of the tips that have a stack note and whose base still exists: one `log` for the notes and one
/// `cat-file --batch-check` for the bases. Best effort: a failure only means detecting without the bases.
pub fn read_stack_bases(git: &GitCommandExecutor, repo: &str, tips: &[&str]) -> HashMap<String, String> {
  let read = || -> Result<HashMap<String, String>> {
    let mut bases = read_notes(git, repo, STACK_NOTES_REF, tips)?;
    if bases.is_empty() {
      return Ok(bases);
    }

    let mut check = String::with_capacity(bases.len() * 41);
    for base in bases.values() {
      check.push_str(base);
      check.push('\n');
    }
    let existing: HashSet<String> = git
      .execute_command_with_input(&["cat-file", "--batch-check=%(objectname) %(objecttype)"], repo, &check)?
      .lines()
      .filter_map(|line| line.strip_suffix(" commit"))
      .map(str::to_string)
      .collect();
    bases.retain(|_, base| existing.contains(base));
    Ok(bases)
  };
  read().unwrap_or_else(|e| {
    warn!(error = %e, "Failed to read the stack notes, detecting stacked branches with their parents' commits");
    HashMap::new()
  })
}

/// Summary for issue-named archived branches: the tip subject without the issue reference
fn compute_summary(branch: &str, subject: Option<&str>) -> String {
  let leaf = branch.rsplit('/').next().unwrap_or(branch);
  if !matches!(find_issue_number(leaf), Some(issue) if issue == leaf) {
    return String::new();
  }
  let Some(subject) = subject else {
    return String::new();
  };
  if let Some((_, end)) = find_issue_range(subject) {
    subject[end..].trim_start_matches([' ', ':']).trim().to_string()
  } else {
    subject.trim().to_string()
  }
}

/// Write all detection results with one notes commit
fn write_caches(git_executor: &GitCommandExecutor, repo_path: &str, detected: Vec<Detected>) -> Result<()> {
  let caches_to_write: Vec<(String, CachedDetection)> = detected.into_iter().map(|result| (result.tip, result.detection)).collect();
  if !caches_to_write.is_empty() {
    CacheOps::new(git_executor, repo_path)
      .write_all(&caches_to_write)
      .inspect_err(|e| trace!(error = %e, "Failed to write caches"))?;
    for (branch_tip, cache) in &caches_to_write {
      debug!(branch_tip = %branch_tip, status = ?cache.info.status, "Cached detection result");
    }
  }
  Ok(())
}

/// Archive inactive branches using pre-fetched branch data. Best effort: on a failure (a virtual tip moved meanwhile,
/// e.g. by `bd` in another process) nothing is archived and a later sync archives them. A branch checked out in a
/// worktree is left alone. Returns map of newly archived branch names to their commits
fn archive_inactive_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  branch_data: &common::BranchData,
  active_branches: &HashSet<String>,
) -> HashMap<String, String> {
  // Find inactive virtual branches
  let inactive_branches: Vec<String> = branch_data
    .virtual_commits
    .keys()
    .filter(|full_name| strip_branch_namespace(full_name, branch_prefix, BranchKind::Virtual).is_some_and(|simple_name| !active_branches.contains(simple_name)))
    .filter(|full_name| {
      let checked_out = branch_data.checked_out.contains(*full_name);
      if checked_out {
        debug!(branch = %full_name, "Not archiving a branch checked out in a worktree");
      }
      !checked_out
    })
    .cloned()
    .collect();

  if inactive_branches.is_empty() {
    return HashMap::new();
  }

  debug!(count = inactive_branches.len(), "Found inactive branches to archive");

  // Archive with pre-fetched data
  match super::archive::batch_archive_inactive_branches(
    git_executor,
    repo_path,
    branch_prefix,
    &inactive_branches,
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  ) {
    Ok(newly_archived) => newly_archived,
    Err(e) => {
      warn!(error = %e, "Failed to archive inactive branches");
      HashMap::new()
    }
  }
}

/// The archived branches after the archive step of a sync, which `detect_archived_branches` detects
pub struct ArchivedBranches {
  /// The newly archived branches first, then the earlier ones, newest first
  names: Vec<String>,
  /// Branch -> tip, of the archived ones and more
  tips: HashMap<String, String>,
  /// Tip -> parsed detection note
  notes: HashMap<String, CachedDetection>,
  /// Tip -> subject
  subjects: HashMap<String, String>,
}

/// The ref part of the archive step, fast: delete the archived branches older than `retention_days` that are fully
/// integrated, move the virtual branches not in `active_branches` (simple names) into today's archive, and send
/// `ArchivedBranchesFound`. A sync runs it before it rebuilds the virtual branches, so a rebuilt branch never meets a
/// virtual branch about to be archived on git's directory/file rule (`(foo)` renamed to `(foo/bar)`).
#[instrument(skip(git_executor, active_branches, progress), fields(branch_prefix = %branch_prefix, active_count = active_branches.len()))]
pub fn archive_and_clean_up(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  active_branches: &HashSet<String>,
  retention_days: u64,
  progress: &dyn ProgressReporter,
) -> Result<ArchivedBranches> {
  let branches = common::list_branches(git_executor, repo_path, branch_prefix)?;
  archive_and_clean_up_listed(git_executor, repo_path, branch_prefix, &branches, active_branches, retention_days, progress)
}

/// `archive_and_clean_up` of the branches under the prefix the caller listed (`common::list_branches`, newest first;
/// the sync lists them with its other refs): only the detection notes are read before the ref writes
#[instrument(skip(git_executor, branches, active_branches, progress), fields(branch_prefix = %branch_prefix, active_count = active_branches.len()))]
pub fn archive_and_clean_up_listed(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  branches: &[common::ListedBranch],
  active_branches: &HashSet<String>,
  retention_days: u64,
  progress: &dyn ProgressReporter,
) -> Result<ArchivedBranches> {
  let branch_prefix = branch_prefix.trim_end_matches('/');
  // Step 0: Get ALL branch data including parsed cached notes
  let mut branch_data = common::branch_data_of(git_executor, repo_path, branch_prefix, branches);

  // Step 1: Clean up old archived branches, but only those fully integrated
  let cutoff_date = days_ago(i64::try_from(retention_days).unwrap_or(i64::MAX));

  // Build list of archived branches to delete: older than cutoff AND cache status Integrated, with their tips
  let mut branches_to_delete: Vec<(String, String)> = Vec::new();
  for branch in &branch_data.archived_all {
    if archive_date(branch, branch_prefix).is_some_and(|date| date < cutoff_date)
      // a branch checked out in a worktree stays
      && !branch_data.checked_out.contains(branch)
      // Check detection cache status on the branch tip commit
      && let Some(tip_commit) = branch_data.all_branch_commits.get(branch)
      && let Some(cache) = branch_data.branch_notes.get(tip_commit)
      && matches!(cache.info.status, BranchIntegrationStatus::Integrated { .. })
    {
      branches_to_delete.push((branch.clone(), tip_commit.clone()));
    }
  }

  if !branches_to_delete.is_empty() {
    // Cleanup is best effort: a failure must not fail detection, and nothing is deleted then (one transaction)
    match super::archive::batch_delete_archived_branches(git_executor, repo_path, &branches_to_delete) {
      Ok(deleted) => {
        info!(deleted_count = deleted, retention_days, "Cleaned up old fully integrated archived branches");

        // Deleted branches must not be reported or re-detected
        let deleted_set: HashSet<&str> = branches_to_delete.iter().map(|(branch, _)| branch.as_str()).collect();
        branch_data.archived_all.retain(|branch| !deleted_set.contains(branch.as_str()));
        branch_data.all_branch_commits.retain(|branch, _| !deleted_set.contains(branch.as_str()));
      }
      Err(e) => warn!(error = %e, "Failed to delete old archived branches"),
    }
  }

  // Step 2: Archive inactive branches using the pre-fetched data
  let newly_archived = archive_inactive_branches(git_executor, repo_path, branch_prefix, &branch_data, active_branches);

  // Step 3: Combine newly archived with previously archived, and complete the branch -> tip map.
  // Don't sort by archive date - it has no correlation with integration date;
  // the frontend sorts by actual integration date once detection completes.
  let common::BranchData {
    archived_all,
    branch_notes,
    mut all_branch_commits,
    commit_subjects,
    ..
  } = branch_data;
  let newly_archived_names: Vec<String> = newly_archived.keys().cloned().collect();
  let mut names: Vec<String> = Vec::with_capacity(archived_all.len() + newly_archived.len());
  names.extend(newly_archived_names.iter().cloned());
  names.extend(archived_all);
  all_branch_commits.extend(newly_archived);

  // Send event with archived branch names (even if empty) to update UI
  progress.send(SyncEvent::ArchivedBranchesFound {
    branch_names: names.clone(),
    newly_archived: newly_archived_names,
  })?;

  Ok(ArchivedBranches {
    names,
    tips: all_branch_commits,
    notes: branch_notes,
    subjects: commit_subjects,
  })
}

/// Detect integrated and not-integrated branches with a specific detection strategy: the archive step
/// (`archive_and_clean_up`), then the detection of the archived branches (`detect_archived_branches`).
/// Blocking: runs git processes and must be called from a blocking context.
#[instrument(skip(git_executor, config), fields(branch_prefix = %branch_prefix, baseline_branch = %baseline_branch, active_count = config.active_branches.len(), strategy = ?config.strategy))]
pub fn detect_integrated_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  baseline_branch: &str,
  config: DetectConfig<'_>,
) -> Result<Vec<(String, bool)>> {
  let archived = archive_and_clean_up(git_executor, repo_path, branch_prefix, config.active_branches, config.retention_days, config.progress)?;
  // If no archived branches exist, nothing to detect
  if archived.names.is_empty() {
    return Ok(Vec::new());
  }
  let baseline = resolve_baseline(git_executor, repo_path, baseline_branch)?;
  detect_archived_branches(
    git_executor,
    repo_path,
    branch_prefix,
    &baseline,
    archived,
    config.strategy,
    config.retention_days,
    config.progress,
  )
}

/// Report the archived branches whose cached result still holds and detect the rest, sending
/// `BranchIntegrationDetected` per branch as soon as it is known. `baseline` is a commit id (`resolve_baseline`; a sync
/// passes the one it read at its start), compared with and recorded in a `NotIntegrated` note. Returns `(full name,
/// integrated)` of every archived branch it reported; a branch whose detection failed is left out. Blocking.
#[expect(clippy::too_many_arguments, reason = "the inputs of the detection phase of a sync")]
#[instrument(skip_all, fields(branch_prefix = %branch_prefix, baseline = %baseline, archived_count = archived.names.len(), strategy = ?strategy))]
pub fn detect_archived_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  baseline: &str,
  archived: ArchivedBranches,
  strategy: DetectionStrategy,
  retention_days: u64,
  progress: &dyn ProgressReporter,
) -> Result<Vec<(String, bool)>> {
  let branch_prefix = branch_prefix.trim_end_matches('/');
  let ArchivedBranches {
    names: all_archived_branches,
    tips: mut all_branch_commits,
    notes: branch_notes,
    subjects: commit_subjects,
  } = archived;
  let cutoff_date = days_ago(i64::try_from(retention_days).unwrap_or(i64::MAX));

  // If no archived branches exist, nothing to detect
  if all_archived_branches.is_empty() {
    return Ok(Vec::new());
  }

  // Branches merged into baseline per git's native detection; only useful for merge-based workflows
  let merged_branches: HashSet<String> = if matches!(strategy, DetectionStrategy::Merge | DetectionStrategy::All) {
    // only Branch Deck's own branches, not every branch of the repository
    let merged = format!("--merged={baseline}");
    let own_branches = format!("refs/heads/{branch_prefix}/");
    let merged_branches: HashSet<String> = git_executor
      .execute_command_lines(&["for-each-ref", &merged, "--format=%(refname:lstrip=2)", &own_branches], repo_path)?
      .into_iter()
      .collect();
    debug!(baseline = %baseline, count = merged_branches.len(), "Found branches merged into baseline");
    merged_branches
  } else {
    HashSet::new()
  };

  // Step 4: Report the results that still hold; queue the rest, one job per tip (two archived names can share one)
  let mut names_by_tip: IndexMap<String, Vec<String>> = IndexMap::new();
  for branch in all_archived_branches {
    let Some(tip) = all_branch_commits.remove(&branch) else {
      warn!(branch = %branch, "Could not resolve branch tip");
      continue;
    };
    names_by_tip.entry(tip).or_default().push(branch);
  }
  let mut ancestry = BaselineAncestry::new(git_executor, repo_path, baseline);
  let mut jobs: Vec<DetectionJob> = Vec::new();
  let mut reported: Vec<(String, bool)> = Vec::new();
  for (tip, branches) in names_by_tip {
    // one name within the retention period is enough: the names of one tip get one result
    let within_retention = branches.iter().any(|branch| archive_date(branch, branch_prefix).is_some_and(|date| date >= cutoff_date));
    match ancestry.plan(branch_notes.get(&tip), within_retention) {
      CachePlan::Holds(cached) => {
        let integrated = matches!(cached.info.status, BranchIntegrationStatus::Integrated { .. });
        for branch in branches {
          let mut info = cached.info.clone();
          info.name.clone_from(&branch);
          progress.send(SyncEvent::BranchIntegrationDetected { info })?;
          reported.push((branch, integrated));
        }
      }
      CachePlan::Detect(previous) => jobs.push(DetectionJob {
        is_merged: branches.iter().any(|branch| merged_branches.contains(branch)),
        branches,
        tip,
        stack_base: None,
        previous,
        within_retention,
      }),
    }
  }
  if !jobs.is_empty() {
    let tips: Vec<&str> = jobs.iter().map(|job| job.tip.as_str()).collect();
    let mut stack_bases = read_stack_bases(git_executor, repo_path, &tips);
    for job in &mut jobs {
      job.stack_base = stack_bases.remove(&job.tip);
    }
  }

  let detected = detect_in_parallel(git_executor, repo_path, baseline, strategy, Some(progress), jobs, &commit_subjects)?;
  for result in &detected {
    let integrated = matches!(result.detection.info.status, BranchIntegrationStatus::Integrated { .. });
    reported.extend(result.branches.iter().map(|branch| (branch.clone(), integrated)));
  }

  // Write all caches sequentially to avoid notes ref races
  write_caches(git_executor, repo_path, detected)?;
  Ok(reported)
}

/// A branch of a stacked parent without commits to detect before the branches are placed: its virtual branch, which
/// the sync archives right after, or its newest archived branch
pub struct PendingBranch<'a> {
  /// The branch name without `refs/heads/`
  pub branch: &'a str,
  pub tip: &'a str,
  /// The subject of the tip, for the summary of an issue-named branch
  pub subject: &'a str,
  /// The cached `NotIntegrated` result the baseline moved on from (`CachePlan::Detect`); `None` for a full detection
  pub previous: Option<(BranchIntegrationStatus, Recheck)>,
}

/// Detect `branches` now, as the archive step of the sync would: a stacked branch is labelled by whether its parent is
/// integrated, before the parent is archived (spec/branch/stacked-branches.spec.md). Sends no events. `baseline` is a
/// commit id (`resolve_baseline`), recorded in a `NotIntegrated` note. With `cache`, the results are written as
/// detection notes, so the archived branch is reported from its note; without, nothing is written. Returns the status
/// per tip; a branch whose detection fails is left out.
#[instrument(skip(git, branches), fields(count = branches.len()))]
pub fn detect_pending_branches(
  git: &GitCommandExecutor,
  repo: &str,
  baseline: &str,
  strategy: DetectionStrategy,
  branches: &[PendingBranch<'_>],
  cache: bool,
) -> Result<HashMap<String, BranchIntegrationStatus>> {
  if branches.is_empty() {
    return Ok(HashMap::new());
  }
  let merged: HashSet<String> = if matches!(strategy, DetectionStrategy::Merge | DetectionStrategy::All) {
    let mut args = vec![format!("--merged={baseline}"), "--format=%(refname:lstrip=2)".to_string()];
    args.extend(branches.iter().map(|branch| format!("refs/heads/{}", branch.branch)));
    let args: Vec<&str> = std::iter::once("for-each-ref").chain(args.iter().map(String::as_str)).collect();
    git.execute_command_lines(&args, repo)?.into_iter().collect()
  } else {
    HashSet::new()
  };
  let tips: Vec<&str> = branches.iter().map(|branch| branch.tip).collect();
  let mut stack_bases = read_stack_bases(git, repo, &tips);
  let jobs = branches
    .iter()
    .map(|branch| DetectionJob {
      branches: vec![branch.branch.to_string()],
      tip: branch.tip.to_string(),
      is_merged: merged.contains(branch.branch),
      stack_base: stack_bases.remove(branch.tip),
      previous: branch.previous.clone(),
      // a virtual branch is current, and an archived parent is detected only within the retention period
      within_retention: true,
    })
    .collect();
  let subjects: HashMap<String, String> = branches.iter().map(|branch| (branch.tip.to_string(), branch.subject.to_string())).collect();

  let detected = detect_in_parallel(git, repo, baseline, strategy, None, jobs, &subjects)?;
  let statuses = detected.iter().map(|result| (result.tip.clone(), result.detection.info.status.clone())).collect();
  if cache {
    write_caches(git, repo, detected)?;
  }
  Ok(statuses)
}

/// The result of a detection, cached on the tip: a stacked branch's too, because its stack base keeps its parent's
/// commits out and a `NotIntegrated` result is checked again when the baseline moves
struct Detected {
  tip: String,
  /// The info of the first branch of the job
  detection: CachedDetection,
  /// Every branch of the job: the names of the tip
  branches: Vec<String>,
}

/// Run detection for `jobs` on a bounded pool of scoped threads, sending each result to `progress` as soon as it is
/// known.
#[instrument(skip_all, fields(job_count = jobs.len()))]
fn detect_in_parallel(
  git: &GitCommandExecutor,
  repo: &str,
  baseline: &str,
  strategy: DetectionStrategy,
  progress: Option<&dyn ProgressReporter>,
  jobs: Vec<DetectionJob>,
  commit_subjects: &HashMap<String, String>,
) -> Result<Vec<Detected>> {
  if jobs.is_empty() {
    return Ok(Vec::new());
  }

  let worker_count = max_parallel_git_processes().min(jobs.len());
  let queue = Mutex::new(jobs.into_iter());
  let results = Mutex::new(Vec::new());
  let squash = SquashCheck::new(git, repo, baseline);

  let worker = || -> Result<()> {
    loop {
      let Some(job) = queue.lock().unwrap_or_else(PoisonError::into_inner).next() else {
        return Ok(());
      };
      let subject = commit_subjects.get(&job.tip).map(String::as_str);
      let (status, check) = match perform_detection(git, repo, &job, baseline, strategy, &squash, subject) {
        Ok(result) => result,
        Err(e) => {
          warn!(branch = %job.branches[0], error = %e, "Integration detection failed for branch");
          continue;
        }
      };
      let infos: Vec<BranchIntegrationInfo> = job
        .branches
        .iter()
        .map(|branch| BranchIntegrationInfo {
          summary: compute_summary(branch, subject),
          name: branch.clone(),
          status: status.clone(),
        })
        .collect();
      // Send unified event immediately as each branch completes detection
      if let Some(progress) = progress {
        for info in &infos {
          progress.send(SyncEvent::BranchIntegrationDetected { info: info.clone() })?;
        }
      }
      let info = infos.into_iter().next().expect("a job has a branch");
      results.lock().unwrap_or_else(PoisonError::into_inner).push(Detected {
        tip: job.tip,
        detection: CachedDetection { info, check },
        branches: job.branches,
      });
    }
  };

  std::thread::scope(|scope| {
    (0..worker_count)
      .map(|_| scope.spawn(worker))
      .try_for_each(|handle| handle.join().unwrap_or_else(|_| Err(anyhow!("integration detection worker panicked"))))
  })?;

  Ok(results.into_inner().unwrap_or_else(PoisonError::into_inner))
}

/// Detect one job with the specified strategy: in full, or for a cached `NotIntegrated` result against the baseline
/// commits since its baseline. `subject` is the subject of the tip. Returns the status and, for a `NotIntegrated` one,
/// what its note records for the next check.
fn perform_detection(
  git: &GitCommandExecutor,
  repo: &str,
  job: &DetectionJob,
  baseline: &str,
  strategy: DetectionStrategy,
  squash: &SquashCheck<'_>,
  subject: Option<&str>,
) -> Result<(BranchIntegrationStatus, Option<Recheck>)> {
  // git gets the tip the job was listed with, never the name: a tag of the same name wins when git resolves it, and
  // the branch may have moved since; the name is for the messages only
  let branch_name = job.branches[0].as_str();
  let tip = job.tip.as_str();
  let stack_base = job.stack_base.as_deref();
  // 1) Merge detection first if enabled and branch is known merged
  if (strategy == DetectionStrategy::Merge || strategy == DetectionStrategy::All)
    && job.is_merged
    && let Some(merge_integrated) = merge::detect_merge_status(git, repo, branch_name, tip, baseline, stack_base)?
  {
    return Ok((merge_integrated, None));
  }

  // 2) Rebase/cherry-pick detection via marker scan, incremental for a recheck
  let (mut status, orphaned) = match &job.previous {
    None => {
      let marks = rebase::scan_cherry_marks(git, repo, baseline, tip, stack_base, None)?;
      let status = rebase::detect_rebase_status_with_marks(git, repo, branch_name, tip, &marks)?;
      (status, marks.right.orphaned)
    }
    Some((previous, check)) => rebase::recheck_rebase_status(git, repo, branch_name, tip, baseline, stack_base, previous, check)?,
  };

  // 3) Squash detection fallback for branches with no integrated commits. It compares the whole branch with the baseline,
  //    so a recheck runs it only for a branch archived within the retention period
  if matches!(status, BranchIntegrationStatus::NotIntegrated { integrated_count: 0, .. })
    && (strategy == DetectionStrategy::Squash || strategy == DetectionStrategy::All)
    && (job.previous.is_none() || job.within_retention)
    && let Some(squash_integrated) = squash.detect(branch_name, tip, subject, stack_base, orphaned.len())
  {
    status = squash_integrated;
  }

  let check = match &status {
    BranchIntegrationStatus::NotIntegrated { .. } => Some(Recheck {
      baseline: baseline.to_string(),
      orphaned,
    }),
    _ => None,
  };
  Ok((status, check))
}
