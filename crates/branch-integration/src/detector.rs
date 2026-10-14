use super::cache::{CacheOps, STACK_NOTES_REF};
use super::{common, merge, rebase, squash, strategy::DetectionStrategy};
use anyhow::{Result, anyhow};
use git_executor::concurrency::max_parallel_git_processes;
use git_executor::git_command_executor::GitCommandExecutor;
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

/// An archived branch that needs fresh detection (no cached result for its tip)
struct DetectionJob {
  branch: String,
  tip: String,
  is_merged: bool,
  /// The commit a stacked branch was built on (its stack note), so only its own commits count
  stack_base: Option<String>,
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

/// Write all collected caches with one notes commit
fn write_caches(git_executor: &GitCommandExecutor, repo_path: &str, caches_to_write: &[(String, BranchIntegrationInfo)]) -> Result<()> {
  if !caches_to_write.is_empty() {
    CacheOps::new(git_executor, repo_path)
      .write_all(caches_to_write)
      .inspect_err(|e| trace!(error = %e, "Failed to write caches"))?;
    for (branch_tip, cache) in caches_to_write {
      debug!(branch_tip = %branch_tip, status = ?cache.status, "Cached detection result");
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
    // Extract date from branch path: <prefix>/archived/YYYY-MM-DD/...
    if let Some(date_part) = branch.strip_prefix(&archive_prefix).and_then(|p| p.split('/').next())
      && let Ok(branch_date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
    {
      let branch_datetime = branch_date.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(chrono::Utc).single().unwrap();
      if branch_datetime < cutoff_date {
        // Check detection cache status on the branch tip commit
        if let Some(tip_commit) = branch_data.all_branch_commits.get(branch)
          && let Some(cache) = branch_data.branch_notes.get(tip_commit)
          && matches!(cache.status, BranchIntegrationStatus::Integrated { .. })
        {
          branches_to_delete.push(branch.clone());
        }
      }
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

  // Branches merged into baseline per git's native detection; only useful for merge-based workflows
  let merged_branches: HashSet<String> = if matches!(config.strategy, DetectionStrategy::Merge | DetectionStrategy::All) {
    // only Branch Deck's own branches, not every branch of the repository
    let merged = format!("--merged={baseline_branch}");
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

  // Step 4: Report cached results directly; queue the rest for fresh detection
  let mut jobs = Vec::new();
  for branch in all_archived_branches {
    let Some(tip) = all_branch_commits.remove(&branch) else {
      warn!(branch = %branch, "Could not resolve branch tip");
      continue;
    };
    if let Some(cached) = branch_notes.get(&tip) {
      let mut info = cached.clone();
      info.name = branch;
      config.progress.send(SyncEvent::BranchIntegrationDetected { info })?;
    } else {
      let is_merged = merged_branches.contains(&branch);
      jobs.push(DetectionJob {
        branch,
        tip,
        is_merged,
        stack_base: None,
      });
    }
  }
  if !jobs.is_empty() {
    let tips: Vec<&str> = jobs.iter().map(|job| job.tip.as_str()).collect();
    let mut stack_bases = read_stack_bases(git_executor, repo_path, &tips);
    for job in &mut jobs {
      job.stack_base = stack_bases.remove(&job.tip);
    }
  }

  let detected = detect_in_parallel(git_executor, repo_path, baseline_branch, config.strategy, Some(config.progress), jobs, &commit_subjects)?;

  // Write all caches sequentially to avoid notes ref races
  write_caches(git_executor, repo_path, &cacheable(detected))
}

/// A branch to detect before a sync archives it: the virtual branch of a stacked parent whose commits left HEAD
pub struct PendingBranch<'a> {
  /// The branch name without `refs/heads/`
  pub branch: &'a str,
  pub tip: &'a str,
  /// The subject of the tip, for the summary of an issue-named branch
  pub subject: &'a str,
}

/// Detect `branches` now, as the archiving of the sync would right after: a stacked branch is placed by whether its
/// parent is integrated, before the parent is archived (spec/branch/stacked-branches.spec.md). Sends no events. With
/// `cache`, the results are written as detection notes by the rule of a sync (a stacked branch only when `Integrated`),
/// so the archived branch is reported from its note; without, nothing is written. Returns the status per tip; a branch
/// whose detection fails is left out.
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
      branch: branch.branch.to_string(),
      tip: branch.tip.to_string(),
      is_merged: merged.contains(branch.branch),
      stack_base: stack_bases.remove(branch.tip),
    })
    .collect();
  let subjects: HashMap<String, String> = branches.iter().map(|branch| (branch.tip.to_string(), branch.subject.to_string())).collect();

  let detected = detect_in_parallel(git, repo, baseline, strategy, None, jobs, &subjects)?;
  let statuses = detected.iter().map(|result| (result.tip.clone(), result.info.status.clone())).collect();
  if cache {
    write_caches(git, repo, &cacheable(detected))?;
  }
  Ok(statuses)
}

/// The result of a fresh detection
struct Detected {
  tip: String,
  info: BranchIntegrationInfo,
  /// A stacked branch's result depends on more than its tip (its parent may still be integrated), so only a final
  /// `Integrated` is cached
  cacheable: bool,
}

/// The `(tip, info)` pairs to cache
fn cacheable(detected: Vec<Detected>) -> Vec<(String, BranchIntegrationInfo)> {
  detected.into_iter().filter(|result| result.cacheable).map(|result| (result.tip, result.info)).collect()
}

/// Run fresh detection for `jobs` on a bounded pool of scoped threads, sending each result to `progress` as soon as it is
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
      let status = match perform_fresh_detection(git, repo, &job.branch, baseline, job.is_merged, job.stack_base.as_deref(), strategy) {
        Ok(status) => status,
        Err(e) => {
          warn!(branch = %job.branch, error = %e, "Integration detection failed for branch");
          continue;
        }
      };
      let info = BranchIntegrationInfo {
        summary: compute_summary(&job.branch, commit_subjects.get(&job.tip)),
        name: job.branch,
        status,
      };
      // Send unified event immediately as each branch completes detection
      if let Some(progress) = progress {
        progress.send(SyncEvent::BranchIntegrationDetected { info: info.clone() })?;
      }
      let cacheable = job.stack_base.is_none() || matches!(info.status, BranchIntegrationStatus::Integrated { .. });
      results.lock().unwrap_or_else(PoisonError::into_inner).push(Detected { tip: job.tip, info, cacheable });
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

/// Perform fresh detection using the specified strategy
fn perform_fresh_detection(
  git: &GitCommandExecutor,
  repo: &str,
  branch_name: &str,
  baseline: &str,
  is_merged: bool,
  stack_base: Option<&str>,
  strategy: DetectionStrategy,
) -> Result<BranchIntegrationStatus> {
  // 1) Merge detection first if enabled and branch is known merged
  if (strategy == DetectionStrategy::Merge || strategy == DetectionStrategy::All)
    && is_merged
    && let Some(merge_integrated) = merge::detect_merge_status(git, repo, branch_name, baseline, stack_base)?
  {
    return Ok(merge_integrated);
  }

  // 2) Rebase/cherry-pick detection via marker scan
  let marks = rebase::scan_right_side_marks(git, repo, baseline, branch_name, stack_base)?;
  let mut status = rebase::detect_rebase_status_with_marks(git, repo, branch_name, baseline, stack_base, marks)?;

  // 3) Squash detection fallback for branches with no integrated commits
  if matches!(status, BranchIntegrationStatus::NotIntegrated { integrated_count: 0, .. }) && (strategy == DetectionStrategy::Squash || strategy == DetectionStrategy::All) {
    let right_count = marks.orphaned as usize;
    if let Some(squash_integrated) = squash::detect_squash_status(git, repo, branch_name, baseline, stack_base, right_count)? {
      status = squash_integrated;
    }
  }

  Ok(status)
}
