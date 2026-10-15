//! The parts of a branch a review shows, read-only: a stack from its root to the branch, each part with its base, its
//! head and its commits, from the virtual branches. One `for-each-ref` for the tips, one `git log` of the baseline to
//! the last tip, split at the tips. A child is built on the head of its parent (`release_children` of the sync), so the
//! log of the last tip passes through the tip of every parent; the base of the root is the parent of the oldest
//! commit, the range Patronus checks (`read_review_commits`).

use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::instrument;

/// At most this many commits from the baseline to the branch; a longer log is not a stack of reviewable branches
pub const MAX_BRANCH_REVIEW_COMMITS: usize = 2000;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetBranchReviewParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  /// The branches from the root of the stack to the reviewed branch, without the `{prefix}/virtual/` namespace; one
  /// name for a branch outside a stack
  pub chain: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchReview {
  /// In the order of the chain: the root first
  pub segments: Vec<BranchReviewSegment>,
}

/// A branch of the chain: its commits on top of its parent (the baseline for the root)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchReviewSegment {
  pub name: String,
  /// The head of the parent, or the commit the root is built on
  pub base: String,
  pub head: String,
  /// Newest first
  pub commits: Vec<BranchReviewCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchReviewCommit {
  pub id: String,
  pub subject: String,
  pub author_time: u32,
}

const LOG_FORMAT: &str = "--format=%x1e%H%x00%P%x00%at%x00%s";

#[instrument(skip(git), fields(chain = ?params.chain))]
pub fn get_branch_review(git: &GitCommandExecutor, params: &GetBranchReviewParams) -> Result<BranchReview> {
  let Some(last) = params.chain.last() else {
    bail!("No branch to review");
  };
  let refs: Vec<String> = params
    .chain
    .iter()
    .map(|name| to_final_branch_name(&params.branch_prefix, name).map(|branch| format!("refs/heads/{branch}")))
    .collect::<Result<_>>()?;
  let mut args = vec!["for-each-ref", "--format=%(refname)%00%(objectname)"];
  args.extend(refs.iter().map(String::as_str));
  let output = git.execute_command(&args, &params.repository_path)?;
  let tips: HashMap<&str, &str> = output.lines().filter_map(|line| line.split_once('\0')).collect();
  let tips: Vec<&str> = refs
    .iter()
    .zip(&params.chain)
    .map(|(reference, name)| match tips.get(reference.as_str()) {
      Some(tip) => Ok(*tip),
      None => Err(anyhow::anyhow!("{name} has no virtual branch yet: sync first")),
    })
    .collect::<Result<_>>()?;

  let max_count = format!("--max-count={}", MAX_BRANCH_REVIEW_COMMITS + 1);
  let exclusion = format!("^{}", params.baseline_branch);
  let last_tip = tips[tips.len() - 1];
  let output = git.execute_command(
    &["log", "--first-parent", "--no-show-signature", &max_count, LOG_FORMAT, &exclusion, last_tip, "--"],
    &params.repository_path,
  )?;
  let log = parse_log(&output);
  if log.is_empty() {
    bail!("{last} has no commits the baseline lacks");
  }
  if log.len() > MAX_BRANCH_REVIEW_COMMITS {
    bail!("{last} has more than {MAX_BRANCH_REVIEW_COMMITS} commits on top of the baseline");
  }
  split_segments(&params.chain, &tips, log)
}

/// A commit of the log with its first parent
struct LogCommit {
  commit: BranchReviewCommit,
  first_parent: Option<String>,
}

fn parse_log(output: &str) -> Vec<LogCommit> {
  output
    .split('\x1e')
    .filter_map(|record| {
      let mut fields = record.trim_end_matches('\n').splitn(4, '\0');
      let id = fields.next().filter(|id| !id.is_empty())?.to_string();
      let first_parent = fields.next()?.split_whitespace().next().map(str::to_string);
      let author_time = fields.next()?.parse().unwrap_or(0);
      let subject = fields.next().unwrap_or("").to_string();
      Some(LogCommit {
        commit: BranchReviewCommit { id, subject, author_time },
        first_parent,
      })
    })
    .collect()
}

/// Cut the log (newest first) at the tips of the parents, from the reviewed branch down to the root
fn split_segments(chain: &[String], tips: &[&str], log: Vec<LogCommit>) -> Result<BranchReview> {
  let root_base = log.last().and_then(|commit| commit.first_parent.clone()).unwrap_or_default();
  if root_base.is_empty() {
    bail!("{} starts at a root commit", chain[0]);
  }
  let mut positions = Vec::with_capacity(tips.len());
  for (index, tip) in tips.iter().enumerate() {
    let Some(position) = log.iter().position(|commit| commit.commit.id == *tip) else {
      // only a parent's tip can be missing: the log starts at the last one
      bail!("{} isn't built on the head of {}: sync first", chain[chain.len() - 1], chain[index]);
    };
    positions.push(position);
  }
  // a child's head is newer than its parent's
  if positions.windows(2).any(|pair| pair[1] >= pair[0]) {
    bail!("{} isn't built on the head of its parent: sync first", chain[chain.len() - 1]);
  }
  let commits: Vec<BranchReviewCommit> = log.into_iter().map(|commit| commit.commit).collect();
  let segments = (0..chain.len())
    .map(|index| {
      // from the tip of the branch down to the tip of its parent
      let end = if index == 0 { commits.len() } else { positions[index - 1] };
      BranchReviewSegment {
        name: chain[index].clone(),
        base: if index == 0 { root_base.clone() } else { tips[index - 1].to_string() },
        head: tips[index].to_string(),
        commits: commits[positions[index]..end].to_vec(),
      }
    })
    .collect();
  Ok(BranchReview { segments })
}
