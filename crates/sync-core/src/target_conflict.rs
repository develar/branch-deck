//! The target conflict check before a robot starts, of `bd safe-push` and the app's Safe Push: Patronus rebases what it
//! lands onto the newest target and fails the robot at once when a commit conflicts. The check fetches the target's
//! remote branch first (as Update Project does), then replays the commits onto its tip as a plain `git rebase` does,
//! with no resolution by words (`git_ops::trial_rebase`). A fetch that fails does not stop the check: it replays onto
//! the last fetched tip, and the result says so (docs/design/git-operations.md, "Target conflict check").

use crate::baseline_fetch::{FetchBaselineParams, fetch_baseline_core, fetch_baseline_of};
use crate::repo_facts::Baseline;
use anyhow::{Result, anyhow};
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::update_project::UpdateCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::trial_rebase::{TrialRebase, trial_rebase};
use serde::{Deserialize, Serialize};
use tracing::{instrument, warn};

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TargetConflictParams {
  pub repository_path: String,
  /// As the sync detected it, e.g. `origin/master`: the robot pushes into the branch it tracks
  pub baseline_branch: String,
  /// The virtual branch the robot starts from, without the `{prefix}/virtual/` namespace; with `stack`, the root of the
  /// stack
  pub branch_name: String,
  /// The whole stack of the root `branch_name`: the robot starts from its stack branch `{prefix}/stack/<root>`
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: bool,
}

/// What the check found
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TargetCheck {
  /// The baseline the commits were replayed onto, e.g. `origin/master`
  pub baseline: String,
  /// Why the fetch of the target failed; the check then used the last fetched tip. `None` after a fetch
  pub fetch_error: Option<String>,
  pub outcome: TargetOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TargetOutcome {
  /// Every commit applies onto the target tip
  Clean,
  /// A commit conflicts: Patronus fails the robot before any check
  Conflict(TargetConflict),
  /// The check did not run to its end: nothing is known
  #[serde(rename_all = "camelCase")]
  NotRun { error: String },
}

/// The first commit whose replay onto the target conflicts
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TargetConflict {
  /// The tip of the target the commits were replayed onto
  pub target_tip: String,
  /// The commit of the branch the robot starts from: a copy of the virtual branch, its subject without the prefix
  pub commit: TargetCommit,
  /// The conflicting files, at most 50 (all of them when only one more)
  pub files: Vec<String>,
  /// How many files conflict
  pub file_count: u32,
  /// The commits of the target after the base of the branch that change `files`, the newest 10, oldest first
  pub target_commits: Vec<TargetCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TargetCommit {
  pub id: String,
  pub author: String,
  /// Unix seconds
  pub author_time: u32,
  pub subject: String,
}

impl From<UpdateCommit> for TargetCommit {
  fn from(commit: UpdateCommit) -> Self {
    Self {
      id: commit.id,
      author: commit.author,
      author_time: commit.time,
      subject: commit.subject,
    }
  }
}

/// How the fetch of the target before the check went
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFetch {
  /// The upstream fetched, e.g. `origin/master`
  Fetched { upstream: String },
  /// Why the fetch failed: the check uses the last fetched tip
  Failed(String),
}

impl TargetFetch {
  /// Why the fetch failed; `None` after a fetch
  pub fn into_error(self) -> Option<String> {
    match self {
      Self::Fetched { .. } => None,
      Self::Failed(error) => Some(error),
    }
  }

  /// Why the fetch did not fetch `upstream`, the upstream of the baseline the check replays onto (`None` for a local
  /// baseline): it failed, or it fetched the upstream of another baseline, because HEAD switched to another branch
  /// while it ran. The check then uses the last fetched tip of `upstream`; `None` after a fetch of `upstream`
  pub fn error_for(self, upstream: Option<&str>) -> Option<String> {
    match self {
      Self::Fetched { upstream: fetched } if Some(fetched.as_str()) == upstream => None,
      Self::Fetched { upstream: fetched } => Some(format!("HEAD switched to another branch while {fetched} was fetched")),
      Self::Failed(error) => Some(error),
    }
  }
}

/// Fetches only the baseline's remote branch, the target of the robot, as Update Project does: of the baseline the
/// caller read (`known`, `bd`), else read now. Never under the repository lock
pub fn fetch_target(git_executor: &GitCommandExecutor, repository_path: &str, known: Option<Baseline>) -> TargetFetch {
  let fetched = if let Some(baseline) = known {
    fetch_baseline_of(git_executor, repository_path, baseline, &mut |_| {}, &|| false)
  } else {
    let params = FetchBaselineParams {
      repository_path: repository_path.to_string(),
    };
    fetch_baseline_core(git_executor, &params, &mut |_| {}, &|| false)
  };
  match fetched {
    Ok(fetched) => TargetFetch::Fetched { upstream: fetched.upstream },
    Err(error) => {
      warn!(error = %format!("{error:#}"), "the target conflict check uses the last fetched target");
      TargetFetch::Failed(format!("{error:#}"))
    }
  }
}

/// Replays the commits of `tip` that the target lacks onto `target_tip`, the tip of `baseline`. It writes objects only
pub fn check_target(git_executor: &GitCommandExecutor, repository_path: &str, baseline: &str, target_tip: &str, tip: &str, fetch_error: Option<String>) -> TargetCheck {
  let outcome = match trial_rebase(git_executor, repository_path, target_tip, tip) {
    Ok(TrialRebase::Clean) => TargetOutcome::Clean,
    Ok(TrialRebase::Conflict(conflict)) => {
      let conflict = *conflict;
      TargetOutcome::Conflict(TargetConflict {
        target_tip: target_tip.to_string(),
        commit: conflict.commit.into(),
        files: conflict.files,
        file_count: conflict.file_count,
        target_commits: conflict.target_commits.into_iter().map(TargetCommit::from).collect(),
      })
    }
    Err(error) => {
      warn!(error = %format!("{error:#}"), "the target conflict check failed");
      TargetOutcome::NotRun { error: format!("{error:#}") }
    }
  };
  TargetCheck {
    baseline: baseline.to_string(),
    fetch_error,
    outcome,
  }
}

/// The check of the app's Safe Push start: the fetch of the target, then `check_branch_target`
#[instrument(skip(git_executor))]
pub fn target_conflict_core(git_executor: &GitCommandExecutor, branch_prefix: &str, params: &TargetConflictParams) -> TargetCheck {
  let fetch_error = fetch_target(git_executor, &params.repository_path, None).into_error();
  check_branch_target(git_executor, branch_prefix, params, fetch_error)
}

/// The check of the branch the robot starts from against the baseline as it is now (one `rev-parse`, then
/// `check_target`), the branch named under `branch_prefix`; a branch or a baseline that does not resolve is `NotRun`
pub fn check_branch_target(git_executor: &GitCommandExecutor, branch_prefix: &str, params: &TargetConflictParams, fetch_error: Option<String>) -> TargetCheck {
  let baseline = params.baseline_branch.as_str();
  match resolve_tips(git_executor, branch_prefix, params) {
    Ok((target_tip, tip)) => check_target(git_executor, &params.repository_path, baseline, &target_tip, &tip, fetch_error),
    Err(error) => TargetCheck {
      baseline: baseline.to_string(),
      fetch_error,
      outcome: TargetOutcome::NotRun { error: format!("{error:#}") },
    },
  }
}

/// The commits of the baseline and of the branch the robot starts from
fn resolve_tips(git_executor: &GitCommandExecutor, branch_prefix: &str, params: &TargetConflictParams) -> Result<(String, String)> {
  let source = if params.stack {
    to_stack_branch_name(branch_prefix, &params.branch_name)?
  } else {
    to_final_branch_name(branch_prefix, &params.branch_name)?
  };
  let baseline = format!("{}^{{commit}}", params.baseline_branch);
  let branch = format!("refs/heads/{source}^{{commit}}");
  let lines = git_executor.execute_command_lines(&["rev-parse", &baseline, &branch], &params.repository_path)?;
  match lines.as_slice() {
    [target_tip, tip] => Ok((target_tip.clone(), tip.clone())),
    _ => Err(anyhow!("Cannot resolve {} and {source}", params.baseline_branch)),
  }
}

#[cfg(test)]
#[path = "target_conflict_test.rs"]
mod target_conflict_test;
