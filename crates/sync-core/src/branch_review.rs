//! The parts of a branch a review shows, read-only: a stack from its root to the branch, each part with its base, its
//! head and its commits, from the virtual branches. One `for-each-ref` for the tips, one `git log` of the baseline to
//! the last tip, split at the tips. A child is built on the head of its parent (`release_children` of the sync), so the
//! log of the last tip passes through the tip of every parent; the base of the root is the parent of the oldest
//! commit, the range Patronus checks (`read_review_commits`).
//!
//! The whole stack (`whole_stack`) is one part: the stack branch `{prefix}/stack/<root>` on the baseline, every commit
//! of the stack in the order of the history.
//!
//! The app memoises a review (`BranchReviewMemo`) by the tips and the baseline the `for-each-ref` reads, so that a
//! review opened again runs only that `for-each-ref`.

use anyhow::{Result, anyhow, bail};
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::review_request::ReviewCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, instrument};

/// At most this many commits from the baseline to the branch; a longer log is not a stack of reviewable branches
pub const MAX_BRANCH_REVIEW_COMMITS: usize = 2000;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetBranchReviewParams {
  pub repository_path: String,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  /// The branches from the root of the stack to the reviewed branch, without the `{prefix}/virtual/` namespace; one
  /// name for a branch outside a stack
  pub chain: Vec<String>,
  /// The whole stack of the root `chain` names alone: its stack branch `{prefix}/stack/<root>` (spec/branch/whole-stack.spec.md),
  /// one part named by the root
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub whole_stack: bool,
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
  /// Newest first; the commits of a pull request are of the same type, a review shows both alike
  pub commits: Vec<ReviewCommit>,
}

/// `-z`: a NUL ends each commit, as NULs separate its fields; the message may hold any other byte
const LOG_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%at%x00%ct%x00%s%x00%B";

/// The review of the chain of `params`, its branches named under `branch_prefix` (the host resolved it)
#[instrument(skip(git), fields(chain = ?params.chain))]
pub fn get_branch_review(git: &GitCommandExecutor, branch_prefix: &str, params: &GetBranchReviewParams) -> Result<BranchReview> {
  let tips = read_tips(git, branch_prefix, params)?;
  review_from_tips(git, params, &tips.tips)
}

/// What a review is built from, by one `for-each-ref`
struct BranchTips {
  /// In the order of the chain
  tips: Vec<String>,
  /// The refs the baseline may name, as git resolves a short name (`refs/remotes/origin/master` for `origin/master`),
  /// with their object ids; empty for a baseline that names no ref
  baseline: Vec<(String, String)>,
}

/// The full ref names a revision `name` may stand for, in the order git tries them (`gitrevisions`)
fn ref_candidates(name: &str) -> Vec<String> {
  if name.starts_with("refs/") {
    return vec![name.to_string()];
  }
  ["refs/", "refs/tags/", "refs/heads/", "refs/remotes/"]
    .iter()
    .map(|prefix| format!("{prefix}{name}"))
    .collect()
}

fn read_tips(git: &GitCommandExecutor, branch_prefix: &str, params: &GetBranchReviewParams) -> Result<BranchTips> {
  if params.whole_stack && params.chain.len() != 1 {
    bail!("The whole stack is reviewed by its root alone");
  }
  let full_name = |name: &str| {
    if params.whole_stack {
      to_stack_branch_name(branch_prefix, name)
    } else {
      to_final_branch_name(branch_prefix, name)
    }
  };
  let refs: Vec<String> = params
    .chain
    .iter()
    .map(|name| full_name(name).map(|branch| format!("refs/heads/{branch}")))
    .collect::<Result<_>>()?;
  if refs.is_empty() {
    bail!("No branch to review");
  }
  let baseline_refs = ref_candidates(&params.baseline_branch);
  let mut args = vec!["for-each-ref", "--format=%(refname)%00%(objectname)"];
  args.extend(refs.iter().chain(&baseline_refs).map(String::as_str));
  let output = git.execute_command(&args, &params.repository_path)?;
  let found: HashMap<&str, &str> = output.lines().filter_map(|line| line.split_once('\0')).collect();
  let tips = refs
    .iter()
    .zip(&params.chain)
    .map(|(reference, name)| match found.get(reference.as_str()) {
      Some(tip) => Ok((*tip).to_string()),
      None if params.whole_stack => Err(anyhow!("The whole {name} stack has no stack branch: it builds only for a stack that forks")),
      None => Err(anyhow!("{name} has no virtual branch yet: sync first")),
    })
    .collect::<Result<_>>()?;
  let baseline = baseline_refs
    .into_iter()
    .filter_map(|reference| found.get(reference.as_str()).map(|id| (reference.clone(), (*id).to_string())))
    .collect();
  Ok(BranchTips { tips, baseline })
}

/// The review of the chain whose tips are `tips`: one `git log` of the baseline to the last tip, split at the tips
fn review_from_tips(git: &GitCommandExecutor, params: &GetBranchReviewParams, tips: &[String]) -> Result<BranchReview> {
  let Some(last) = params.chain.last() else {
    bail!("No branch to review");
  };
  let max_count = format!("--max-count={}", MAX_BRANCH_REVIEW_COMMITS + 1);
  let exclusion = format!("^{}", params.baseline_branch);
  let last_tip = &tips[tips.len() - 1];
  let output = git
    .cmd(
      &[
        "log",
        "-z",
        "--first-parent",
        "--no-show-signature",
        &max_count,
        LOG_FORMAT,
        "--end-of-options",
        &exclusion,
        last_tip,
        "--",
      ],
      &params.repository_path,
    )
    .bytes()?;
  let output = String::from_utf8_lossy(&output);
  let log = parse_log(&output);
  if log.is_empty() {
    bail!("{last} has no commits the baseline lacks");
  }
  if log.len() > MAX_BRANCH_REVIEW_COMMITS {
    bail!("{last} has more than {MAX_BRANCH_REVIEW_COMMITS} commits on top of the baseline");
  }
  let tips: Vec<&str> = tips.iter().map(String::as_str).collect();
  split_segments(&params.chain, &tips, log)
}

/// The reviews the app built, by what they are built from: the params, the tips and the baseline's ref ids. A moved
/// tip or baseline is another key; the commits are objects and never change. Cloning is cheap and shares the cache
#[derive(Clone)]
pub struct BranchReviewMemo {
  cache: Cache<BranchReviewKey, Arc<BranchReview>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BranchReviewKey {
  repository_path: String,
  branch_prefix: String,
  baseline_branch: String,
  chain: Vec<String>,
  whole_stack: bool,
  tips: Vec<String>,
  baseline: Vec<(String, String)>,
}

impl BranchReviewMemo {
  /// Remembers the latest reviews: a window shows one, a user goes back and forth between a few
  pub fn new() -> Self {
    Self { cache: Cache::new(32) }
  }

  /// `get_branch_review`, whose `log` runs only for tips and a baseline not seen before; a baseline that names no
  /// ref is never memoised
  #[instrument(skip(self, git), fields(chain = ?params.chain))]
  pub fn get(&self, git: &GitCommandExecutor, branch_prefix: &str, params: &GetBranchReviewParams) -> Result<Arc<BranchReview>> {
    let BranchTips { tips, baseline } = read_tips(git, branch_prefix, params)?;
    if baseline.is_empty() {
      return review_from_tips(git, params, &tips).map(Arc::new);
    }
    let key = BranchReviewKey {
      repository_path: params.repository_path.clone(),
      branch_prefix: branch_prefix.to_owned(),
      baseline_branch: params.baseline_branch.clone(),
      chain: params.chain.clone(),
      whole_stack: params.whole_stack,
      tips,
      baseline,
    };
    let mut memoised = true;
    let review = self
      .cache
      .try_get_with_by_ref(&key, || {
        memoised = false;
        review_from_tips(git, params, &key.tips).map(Arc::new)
      })
      .map_err(|e| anyhow!("{e:#}"))?;
    debug!(memoised, "branch review");
    Ok(review)
  }
}

impl Default for BranchReviewMemo {
  fn default() -> Self {
    Self::new()
  }
}

/// A commit of the log with its first parent
struct LogCommit {
  commit: ReviewCommit,
  first_parent: Option<String>,
}

fn parse_log(output: &str) -> Vec<LogCommit> {
  let mut fields = output.split('\0');
  let mut log = Vec::new();
  while let (Some(id), Some(parents), Some(author), Some(author_time), Some(committer_time), Some(subject), Some(message)) =
    (fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
  {
    // the NUL that ends a commit is followed by the newline git puts between commits
    let id = id.trim_start_matches('\n');
    if id.is_empty() {
      break;
    }
    log.push(LogCommit {
      commit: ReviewCommit::from_log_fields(id, subject, author, author_time, committer_time, message),
      first_parent: parents.split_whitespace().next().map(str::to_string),
    });
  }
  log
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
  let commits: Vec<ReviewCommit> = log.into_iter().map(|commit| commit.commit).collect();
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
