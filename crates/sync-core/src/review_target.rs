//! What a review shows of a target (`ReviewTarget`, spec/review/review-window.spec.md), read-only:
//!
//! - a branch: a stack from its root to the branch, each part with its base, its head and its commits, from the virtual
//!   branches. One `for-each-ref` for the tips, one `git log` of the baseline to the last tip, split at the tips. A
//!   child is built on the head of its parent (`release_children` of the sync), so the log of the last tip passes
//!   through the tip of every parent; the base of the root is the parent of the oldest commit, the range Patronus checks
//!   (`read_review_commits`);
//! - the whole stack: one part, the stack branch `{prefix}/stack/<root>` on the baseline, every commit of the stack in
//!   the order of the history;
//! - a kept branch (unapplied, archived): one part, its own commits from where they start (`own_commits_start`: its
//!   stack base, else where it left the baseline), as its row lists them;
//! - a list of commits: one `log --no-walk` of their ids, in the order of the list.
//!
//! The app memoises a review (`ReviewMemo`) by the tips and the baseline the `for-each-ref` reads, so that a review
//! opened again runs only that `for-each-ref`; a list of commits, whose objects never change, runs nothing.

use anyhow::{Result, anyhow, bail};
use branch_integration::archive::own_commits_start;
use domain_types::branch_names::{kept_branch_name, to_final_branch_name, to_stack_branch_name};
use domain_types::review::{GetReviewParams, ReviewSegment, ReviewTarget, TargetReview};
use domain_types::review_request::ReviewCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, instrument};

/// At most this many commits from the base to the head; a longer log is no reviewable change
pub const MAX_REVIEW_COMMITS: usize = 2000;

/// `-z`: a NUL ends each commit, as NULs separate its fields; the message may hold any other byte
const LOG_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%at%x00%ct%x00%s%x00%B";

/// The review of the target of `params`, its branches named under `branch_prefix` (the host resolved it)
#[instrument(skip(git), fields(target = ?params.target))]
pub fn get_review(git: &GitCommandExecutor, branch_prefix: &str, params: &GetReviewParams) -> Result<TargetReview> {
  match read_refs(git, branch_prefix, params)? {
    Some(refs) => review_from_tips(git, branch_prefix, params, &refs.tips),
    None => read_commits(git, params),
  }
}

/// What a review of refs is built from, by one `for-each-ref`
struct TargetRefs {
  /// In the order of the chain; the tip alone of a whole stack and of a kept branch
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

/// The name a review shows of the kept branch `name` (a full name, `kept_branch_name`); refused outside the kept
/// namespaces of the prefix
fn kept_name<'a>(branch_prefix: &str, name: &'a str) -> Result<&'a str> {
  kept_branch_name(name, branch_prefix).ok_or_else(|| anyhow!("{name} is no unapplied or archived branch of {branch_prefix}"))
}

/// The branches a target names, as `refs/heads/…`, with the names their errors use; none for a list of commits
fn target_refs(branch_prefix: &str, target: &ReviewTarget) -> Result<Option<Vec<(String, String)>>> {
  let refs = match target {
    ReviewTarget::Branch { chain } => {
      if chain.is_empty() {
        bail!("No branch to review");
      }
      chain
        .iter()
        .map(|name| to_final_branch_name(branch_prefix, name).map(|branch| (format!("refs/heads/{branch}"), name.clone())))
        .collect::<Result<_>>()?
    }
    ReviewTarget::WholeStack { root } => vec![(format!("refs/heads/{}", to_stack_branch_name(branch_prefix, root)?), root.clone())],
    ReviewTarget::Kept { name } => vec![(format!("refs/heads/{name}"), kept_name(branch_prefix, name)?.to_string())],
    ReviewTarget::Commits { .. } => return Ok(None),
  };
  Ok(Some(refs))
}

fn read_refs(git: &GitCommandExecutor, branch_prefix: &str, params: &GetReviewParams) -> Result<Option<TargetRefs>> {
  let Some(refs) = target_refs(branch_prefix, &params.target)? else {
    return Ok(None);
  };
  let baseline_refs = ref_candidates(&params.baseline_branch);
  let mut args = vec!["for-each-ref", "--format=%(refname)%00%(objectname)"];
  args.extend(refs.iter().map(|(reference, _)| reference.as_str()).chain(baseline_refs.iter().map(String::as_str)));
  let output = git.execute_command(&args, &params.repository_path)?;
  let found: HashMap<&str, &str> = output.lines().filter_map(|line| line.split_once('\0')).collect();
  let tips = refs
    .iter()
    .map(|(reference, name)| match found.get(reference.as_str()) {
      Some(tip) => Ok((*tip).to_string()),
      None => Err(match params.target {
        ReviewTarget::WholeStack { .. } => anyhow!("The whole {name} stack has no stack branch: it builds only for a stack that forks"),
        ReviewTarget::Kept { .. } => anyhow!("{name} no longer exists"),
        _ => anyhow!("{name} has no virtual branch yet: sync first"),
      }),
    })
    .collect::<Result<_>>()?;
  let baseline = baseline_refs
    .into_iter()
    .filter_map(|reference| found.get(reference.as_str()).map(|id| (reference.clone(), (*id).to_string())))
    .collect();
  Ok(Some(TargetRefs { tips, baseline }))
}

/// The review of a target whose tips are `tips`: one `git log` from its base to the last tip, split at the tips
fn review_from_tips(git: &GitCommandExecutor, branch_prefix: &str, params: &GetReviewParams, tips: &[String]) -> Result<TargetReview> {
  let Some(last_tip) = tips.last() else {
    bail!("No branch to review");
  };
  match &params.target {
    ReviewTarget::Branch { chain } => {
      let log = read_log(git, params, &params.baseline_branch, last_tip, chain.last().map_or("", String::as_str))?;
      let tips: Vec<&str> = tips.iter().map(String::as_str).collect();
      split_segments(chain, &tips, log)
    }
    ReviewTarget::WholeStack { root } => {
      let log = read_log(git, params, &params.baseline_branch, last_tip, root)?;
      split_segments(std::slice::from_ref(root), &[last_tip.as_str()], log)
    }
    ReviewTarget::Kept { name } => {
      let Some(base) = own_commits_start(git, &params.repository_path, &params.baseline_branch, last_tip)? else {
        bail!("{name} has no history in common with {}", params.baseline_branch);
      };
      let short = kept_name(branch_prefix, name)?;
      let log = read_log(git, params, &base, last_tip, short)?;
      Ok(TargetReview::Range {
        segments: vec![ReviewSegment {
          name: short.to_string(),
          base,
          head: last_tip.clone(),
          commits: log.into_iter().map(|commit| commit.commit).collect(),
        }],
      })
    }
    ReviewTarget::Commits { .. } => bail!("A list of commits has no tips"),
  }
}

/// The commits of `^exclusion tip`, newest first along the first parents; the target named `name` in the errors
fn read_log(git: &GitCommandExecutor, params: &GetReviewParams, exclusion: &str, tip: &str, name: &str) -> Result<Vec<LogCommit>> {
  let max_count = format!("--max-count={}", MAX_REVIEW_COMMITS + 1);
  let exclusion = format!("^{exclusion}");
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
        tip,
        "--",
      ],
      &params.repository_path,
    )
    .bytes()?;
  let log = parse_log(&String::from_utf8_lossy(&output));
  if log.is_empty() {
    bail!("{name} has no commits the baseline lacks");
  }
  if log.len() > MAX_REVIEW_COMMITS {
    bail!("{name} has more than {MAX_REVIEW_COMMITS} commits on top of the baseline");
  }
  Ok(log)
}

/// The commits of a list by their ids, in its order: one `log --no-walk=unsorted`
fn read_commits(git: &GitCommandExecutor, params: &GetReviewParams) -> Result<TargetReview> {
  let ReviewTarget::Commits { ids } = &params.target else {
    bail!("Only a list of commits is read by its ids");
  };
  if ids.is_empty() {
    bail!("No commit to review");
  }
  if ids.len() > MAX_REVIEW_COMMITS {
    bail!("More than {MAX_REVIEW_COMMITS} commits to review");
  }
  let mut args = vec!["log", "-z", "--no-walk=unsorted", "--no-show-signature", LOG_FORMAT, "--end-of-options"];
  args.extend(ids.iter().map(String::as_str));
  args.push("--");
  let output = git.cmd(&args, &params.repository_path).bytes()?;
  let commits = parse_log(&String::from_utf8_lossy(&output)).into_iter().map(|commit| commit.commit).collect();
  Ok(TargetReview::Commits { commits })
}

/// The reviews the app built, by what they are built from: the params, the tips and the baseline's ref ids. A moved
/// tip or baseline is another key; the commits are objects and never change. Cloning is cheap and shares the cache
#[derive(Clone)]
pub struct ReviewMemo {
  cache: Cache<ReviewKey, Arc<TargetReview>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ReviewKey {
  repository_path: String,
  branch_prefix: String,
  baseline_branch: String,
  target: ReviewTarget,
  tips: Vec<String>,
  baseline: Vec<(String, String)>,
}

impl ReviewMemo {
  /// Remembers the latest reviews: a window shows one, a user goes back and forth between a few
  pub fn new() -> Self {
    Self { cache: Cache::new(32) }
  }

  /// `get_review`, whose `log` runs only for tips and a baseline not seen before; a review of refs whose baseline names
  /// no ref is never memoised, a list of commits always is
  #[instrument(skip(self, git), fields(target = ?params.target))]
  pub fn get(&self, git: &GitCommandExecutor, branch_prefix: &str, params: &GetReviewParams) -> Result<Arc<TargetReview>> {
    let refs = read_refs(git, branch_prefix, params)?;
    let (tips, baseline) = match refs {
      Some(TargetRefs { tips, baseline }) if baseline.is_empty() => return review_from_tips(git, branch_prefix, params, &tips).map(Arc::new),
      Some(TargetRefs { tips, baseline }) => (Some(tips), baseline),
      None => (None, Vec::new()),
    };
    let key = ReviewKey {
      repository_path: params.repository_path.clone(),
      branch_prefix: branch_prefix.to_owned(),
      baseline_branch: params.baseline_branch.clone(),
      target: params.target.clone(),
      tips: tips.clone().unwrap_or_default(),
      baseline,
    };
    let mut memoised = true;
    let review = self
      .cache
      .try_get_with_by_ref(&key, || {
        memoised = false;
        match &tips {
          Some(tips) => review_from_tips(git, branch_prefix, params, tips),
          None => read_commits(git, params),
        }
        .map(Arc::new)
      })
      .map_err(|e| anyhow!("{e:#}"))?;
    debug!(memoised, "review");
    Ok(review)
  }
}

impl Default for ReviewMemo {
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
fn split_segments(chain: &[String], tips: &[&str], log: Vec<LogCommit>) -> Result<TargetReview> {
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
      ReviewSegment {
        name: chain[index].clone(),
        base: if index == 0 { root_base.clone() } else { tips[index - 1].to_string() },
        head: tips[index].to_string(),
        commits: commits[positions[index]..end].to_vec(),
      }
    })
    .collect();
  Ok(TargetReview::Range { segments })
}
