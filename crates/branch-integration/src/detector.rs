use super::cache::{CacheOps, CachedDetection, Recheck, STACK_NOTES_REF};
use super::{common, merge, rebase, squash, strategy::DetectionStrategy};
use anyhow::{Result, anyhow};
use git_executor::concurrency::max_parallel_git_processes;
use git_executor::git_command_executor::GitCommandExecutor;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus};
use sync_types::{ProgressReporter, SyncEvent};
use sync_utils::issue_pattern::{find_issue_number, find_issue_range};
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
}

impl DetectionJob {
  fn new(branch: String, tip: String, is_merged: bool, previous: Option<(BranchIntegrationStatus, Recheck)>) -> Self {
    Self {
      branches: vec![branch],
      tip,
      is_merged,
      stack_base: None,
      previous,
    }
  }
}

/// What step 4 does with the tip of an archived branch
enum Next<'a> {
  /// The cached result still holds
  Report(&'a CachedDetection),
  /// Detect it, incrementally with the cached `NotIntegrated` result and its baseline
  Detect(Option<(BranchIntegrationStatus, Recheck)>),
}

/// The commit `baseline` names now: detection compares with it and records it in a `NotIntegrated` note, so the scanned
/// range and the recorded baseline are the same commit even if a fetch moves the branch meanwhile
pub fn resolve_baseline(git: &GitCommandExecutor, repo: &str, baseline: &str) -> Result<String> {
  let output = git.execute_command(&["rev-parse", "--verify", "--end-of-options", &format!("{baseline}^{{commit}}")], repo)?;
  Ok(output.trim().to_string())
}

/// Whether `ancestor` is an ancestor of `baseline`; false for a commit that no longer exists (one process)
fn is_ancestor(git: &GitCommandExecutor, repo: &str, ancestor: &str, baseline: &str) -> bool {
  match git.execute_command_with_status(&["merge-base", "--is-ancestor", ancestor, baseline], repo) {
    Ok((_, 0)) => true,
    Ok(_) => false,
    Err(e) => {
      debug!(ancestor = %ancestor, error = %e, "Cannot check the baseline of a detection note");
      false
    }
  }
}

/// The day an archived branch was archived, from `<prefix>/archived/YYYY-MM-DD/<name>`
fn archive_date(branch: &str, archive_prefix: &str) -> Option<chrono::DateTime<chrono::Utc>> {
  let date_part = branch.strip_prefix(archive_prefix)?.split('/').next()?;
  let date = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()?;
  date.and_hms_opt(0, 0, 0)?.and_local_timezone(chrono::Utc).single()
}

/// The stack bases of the tips that have a stack note and whose base still exists: one `log` for the notes and one
/// `cat-file --batch-check` for the bases. Best effort: a failure only means detecting without the bases.
fn read_stack_bases(git: &GitCommandExecutor, repo: &str, tips: &[&str]) -> HashMap<String, String> {
  let read = || -> Result<HashMap<String, String>> {
    let mut input = String::with_capacity(tips.len() * 41);
    for tip in tips {
      input.push_str(tip);
      input.push('\n');
    }
    let notes_arg = format!("--notes={STACK_NOTES_REF}");
    let output = git.execute_command_with_input(&["log", "--no-walk", "--stdin", "--format=%H%x1f%N%x1e", &notes_arg], repo, &input)?;
    let mut bases: HashMap<String, String> = output
      .split('\u{1e}')
      .filter_map(|record| record.trim().split_once('\u{1f}'))
      .filter(|(_, base)| !base.trim().is_empty())
      .map(|(tip, base)| (tip.to_string(), base.trim().to_string()))
      .collect();
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
fn compute_summary(branch: &str, subject: Option<&String>) -> String {
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

/// Archive inactive branches using pre-fetched branch data
/// Returns map of newly archived branch names to their commits
fn archive_inactive_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  branch_data: &common::BranchData,
  active_branches: &HashSet<String>,
) -> Result<HashMap<String, String>> {
  // Find inactive virtual branches
  let virtual_prefix = format!("{branch_prefix}/virtual/");
  let inactive_branches: Vec<String> = branch_data
    .virtual_commits
    .keys()
    .filter(|full_name| {
      let simple_name = full_name.strip_prefix(&virtual_prefix).unwrap_or(full_name);
      !active_branches.contains(simple_name)
    })
    .cloned()
    .collect();

  if inactive_branches.is_empty() {
    return Ok(HashMap::new());
  }

  debug!(count = inactive_branches.len(), "Found inactive branches to archive");

  // Archive with pre-fetched data
  let newly_archived = super::archive::batch_archive_inactive_branches(
    git_executor,
    repo_path,
    branch_prefix,
    &inactive_branches,
    &branch_data.virtual_commits,
    &branch_data.archived_today_names,
  )?;

  if !newly_archived.is_empty() {
    debug!(archived_count = newly_archived.len(), "Successfully archived inactive branches");
  }

  Ok(newly_archived)
}

/// Detect integrated and not-integrated branches with a specific detection strategy.
/// Blocking: runs git processes and must be called from a blocking context.
#[instrument(skip(git_executor, config), fields(branch_prefix = %branch_prefix, baseline_branch = %baseline_branch, active_count = config.active_branches.len(), strategy = ?config.strategy))]
pub fn detect_integrated_branches(git_executor: &GitCommandExecutor, repo_path: &str, branch_prefix: &str, baseline_branch: &str, config: DetectConfig<'_>) -> Result<()> {
  // Step 0: Get ALL branch data including parsed cached notes
  let mut branch_data = common::get_all_branch_data(git_executor, repo_path, branch_prefix)?;

  // Step 1: Clean up old archived branches, but only those fully integrated
  // Compute cutoff date based on retention
  let retention_days = i64::try_from(config.retention_days).unwrap_or(i64::MAX);
  let cutoff_date = chrono::Duration::try_days(retention_days)
    .and_then(|retention| chrono::Utc::now().checked_sub_signed(retention))
    .unwrap_or(chrono::DateTime::<chrono::Utc>::MIN_UTC);
  let archive_prefix = format!("{branch_prefix}/archived/");

  // Build list of archived branches to delete: older than cutoff AND cache status Integrated
  let mut branches_to_delete: Vec<String> = Vec::new();
  for branch in &branch_data.archived_all {
    if archive_date(branch, &archive_prefix).is_some_and(|date| date < cutoff_date)
      // Check detection cache status on the branch tip commit
      && let Some(tip_commit) = branch_data.all_branch_commits.get(branch)
      && let Some(cache) = branch_data.branch_notes.get(tip_commit)
      && matches!(cache.info.status, BranchIntegrationStatus::Integrated { .. })
    {
      branches_to_delete.push(branch.clone());
    }
  }

  if !branches_to_delete.is_empty() {
    // Cleanup is best effort: a failure must not fail detection, and nothing is deleted then (one transaction)
    match super::archive::batch_delete_archived_branches(git_executor, repo_path, &branches_to_delete) {
      Ok(deleted) => {
        info!(
          deleted_count = deleted,
          retention_days = config.retention_days,
          "Cleaned up old fully integrated archived branches"
        );

        // Deleted branches must not be reported or re-detected
        let deleted_set: HashSet<&str> = branches_to_delete.iter().map(String::as_str).collect();
        branch_data.archived_all.retain(|branch| !deleted_set.contains(branch.as_str()));
        branch_data.all_branch_commits.retain(|branch, _| !deleted_set.contains(branch.as_str()));
      }
      Err(e) => warn!(error = %e, "Failed to delete old archived branches"),
    }
  }

  // Step 2: Archive inactive branches using the pre-fetched data
  let newly_archived = archive_inactive_branches(git_executor, repo_path, branch_prefix, &branch_data, config.active_branches)?;

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
  let mut all_archived_branches: Vec<String> = Vec::with_capacity(archived_all.len() + newly_archived.len());
  all_archived_branches.extend(newly_archived_names.iter().cloned());
  all_archived_branches.extend(archived_all);
  all_branch_commits.extend(newly_archived);

  // Send event with archived branch names (even if empty) to update UI
  config.progress.send(SyncEvent::ArchivedBranchesFound {
    branch_names: all_archived_branches.clone(),
    newly_archived: newly_archived_names,
  })?;

  // If no archived branches exist, nothing to detect
  if all_archived_branches.is_empty() {
    return Ok(());
  }
  let baseline = resolve_baseline(git_executor, repo_path, baseline_branch)?;

  // Branches merged into baseline per git's native detection; only useful for merge-based workflows
  let merged_branches: HashSet<String> = if matches!(config.strategy, DetectionStrategy::Merge | DetectionStrategy::All) {
    // only Branch Deck's own branches, not every branch of the repository
    let merged = format!("--merged={baseline}");
    let own_branches = format!("refs/heads/{branch_prefix}/");
    let merged_branches: HashSet<String> = git_executor
      .execute_command_lines(&["for-each-ref", &merged, "--format=%(refname:lstrip=2)", &own_branches], repo_path)?
      .into_iter()
      .collect();
    debug!(baseline = %baseline_branch, count = merged_branches.len(), "Found branches merged into baseline");
    merged_branches
  } else {
    HashSet::new()
  };

  // Step 4: Report the results that still hold; queue the rest, one job per tip
  let mut jobs: IndexMap<String, DetectionJob> = IndexMap::new();
  // the baseline of a note -> whether the current baseline descends from it, one process per distinct baseline
  let mut moved_on_from: HashMap<String, bool> = HashMap::new();
  let mut moved_on = |since: &str| {
    *moved_on_from
      .entry(since.to_string())
      .or_insert_with(|| is_ancestor(git_executor, repo_path, since, &baseline))
  };
  for branch in all_archived_branches {
    let Some(tip) = all_branch_commits.remove(&branch) else {
      warn!(branch = %branch, "Could not resolve branch tip");
      continue;
    };
    let is_merged = merged_branches.contains(&branch);
    if let Some(job) = jobs.get_mut(&tip) {
      job.branches.push(branch);
      job.is_merged |= is_merged;
      continue;
    }
    let within_retention = archive_date(&branch, &archive_prefix).is_some_and(|date| date >= cutoff_date);
    let next = match branch_notes.get(&tip) {
      None => Next::Detect(None),
      Some(cached) => match (&cached.info.status, &cached.check) {
        (BranchIntegrationStatus::NotIntegrated { .. }, Some(check)) if check.baseline == baseline => Next::Report(cached),
        (BranchIntegrationStatus::NotIntegrated { .. }, Some(check)) if moved_on(&check.baseline) => Next::Detect(Some((cached.info.status.clone(), check.clone()))),
        // a note without a baseline (version 1), or a rewound or renamed baseline, or one gone in a gc: detected in full
        // once, but only within the retention period, because a full scan of an old archive is slow
        (BranchIntegrationStatus::NotIntegrated { .. }, _) if within_retention => Next::Detect(None),
        _ => Next::Report(cached),
      },
    };
    match next {
      Next::Report(cached) => {
        let mut info = cached.info.clone();
        info.name = branch;
        config.progress.send(SyncEvent::BranchIntegrationDetected { info })?;
      }
      Next::Detect(previous) => {
        jobs.insert(tip.clone(), DetectionJob::new(branch, tip, is_merged, previous));
      }
    }
  }
  let mut jobs: Vec<DetectionJob> = jobs.into_values().collect();
  if !jobs.is_empty() {
    let tips: Vec<&str> = jobs.iter().map(|job| job.tip.as_str()).collect();
    let mut stack_bases = read_stack_bases(git_executor, repo_path, &tips);
    for job in &mut jobs {
      job.stack_base = stack_bases.remove(&job.tip);
    }
  }

  let detected = detect_in_parallel(git_executor, repo_path, &baseline, config.strategy, Some(config.progress), jobs, &commit_subjects)?;

  // Write all caches sequentially to avoid notes ref races
  write_caches(git_executor, repo_path, detected)
}

/// A branch to detect before a sync archives it: the virtual branch of a stacked parent whose commits left HEAD
pub struct PendingBranch<'a> {
  /// The branch name without `refs/heads/`
  pub branch: &'a str,
  pub tip: &'a str,
  /// The subject of the tip, for the summary of an issue-named branch
  pub subject: &'a str,
}

/// Detect `branches` now, in full, as the archiving of the sync would right after: a stacked branch is labelled by
/// whether its parent is integrated, before the parent is archived (spec/branch/stacked-branches.spec.md). Sends no
/// events. `baseline` is a commit id (`resolve_baseline`), recorded in a `NotIntegrated` note. With `cache`, the results
/// are written as detection notes, so the archived branch is reported from its note; without, nothing is written.
/// Returns the status per tip; a branch whose detection fails is left out.
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
      stack_base: stack_bases.remove(branch.tip),
      ..DetectionJob::new(branch.branch.to_string(), branch.tip.to_string(), merged.contains(branch.branch), None)
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

  let worker = || -> Result<()> {
    loop {
      let Some(job) = queue.lock().unwrap_or_else(PoisonError::into_inner).next() else {
        return Ok(());
      };
      let (status, check) = match perform_detection(git, repo, &job, baseline, strategy) {
        Ok(result) => result,
        Err(e) => {
          warn!(branch = %job.branches[0], error = %e, "Integration detection failed for branch");
          continue;
        }
      };
      let infos: Vec<BranchIntegrationInfo> = job
        .branches
        .into_iter()
        .map(|branch| BranchIntegrationInfo {
          summary: compute_summary(&branch, commit_subjects.get(&job.tip)),
          name: branch,
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
      });
    }
  };

  std::thread::scope(|scope| {
    let handles: Vec<_> = (0..worker_count).map(|_| scope.spawn(worker)).collect();
    handles
      .into_iter()
      .try_for_each(|handle| handle.join().unwrap_or_else(|_| Err(anyhow!("integration detection worker panicked"))))
  })?;

  Ok(results.into_inner().unwrap_or_else(PoisonError::into_inner))
}

/// Detect one job with the specified strategy: in full, or for a cached `NotIntegrated` result against the baseline
/// commits since its baseline. Returns the status and, for a `NotIntegrated` one, what its note records for the next
/// check.
fn perform_detection(git: &GitCommandExecutor, repo: &str, job: &DetectionJob, baseline: &str, strategy: DetectionStrategy) -> Result<(BranchIntegrationStatus, Option<Recheck>)> {
  let branch_name = job.branches[0].as_str();
  let stack_base = job.stack_base.as_deref();
  // 1) Merge detection first if enabled and branch is known merged
  if (strategy == DetectionStrategy::Merge || strategy == DetectionStrategy::All)
    && job.is_merged
    && let Some(merge_integrated) = merge::detect_merge_status(git, repo, branch_name, baseline, stack_base)?
  {
    return Ok((merge_integrated, None));
  }

  // 2) Rebase/cherry-pick detection via marker scan, incremental for a recheck
  let (mut status, orphaned) = match &job.previous {
    None => {
      let marks = rebase::scan_right_side_marks(git, repo, baseline, branch_name, stack_base, None)?;
      let status = rebase::detect_rebase_status_with_marks(git, repo, branch_name, baseline, stack_base, &marks)?;
      (status, marks.orphaned)
    }
    Some((previous, check)) => rebase::recheck_rebase_status(git, repo, branch_name, baseline, stack_base, previous, check)?,
  };

  // 3) Squash detection fallback for branches with no integrated commits
  if matches!(status, BranchIntegrationStatus::NotIntegrated { integrated_count: 0, .. }) && (strategy == DetectionStrategy::Squash || strategy == DetectionStrategy::All) {
    let right_count = orphaned.len();
    if let Some(squash_integrated) = squash::detect_squash_status(git, repo, branch_name, baseline, stack_base, right_count)? {
      status = squash_integrated;
    }
  }

  let check = match &status {
    BranchIntegrationStatus::NotIntegrated {
      total_commit_count,
      orphaned_count,
      ..
    } => Some(Recheck {
      baseline: baseline.to_string(),
      // nothing integrated yet: implicitly the whole branch side
      orphaned: (orphaned_count != total_commit_count).then_some(orphaned),
    }),
    _ => None,
  };
  Ok((status, check))
}
