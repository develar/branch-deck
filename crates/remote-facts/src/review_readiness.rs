//! The review readiness of virtual branches, shared by the sessions of the app (the scheduler of the facts, through the
//! `FactSources` of each host) and `bd safe-push`: the commits of the branches (`patronus_review`), the pre-check of
//! Patronus with the pull request each would start from, cached on disk by what Patronus decides on, then
//! `patronus::review_readiness`. The whole stack is asked as its stack branch (`ReviewReadinessBranch::source`,
//! `{prefix}/stack/<root>`) with its pull request. `kept_review_readiness` answers from the cache alone, so a session
//! answers what it keeps before it asks Patronus anything.
//!
//! The answer of a branch reviewed on GitHub is kept in the `review_answer` store of the object cache, keyed by a hash
//! of the Patronus server and token, the Space repository, the target, the changed paths, and for a start from a pull
//! request its number, its author and its reviews (login, state, request). A sync onto a new baseline keeps the key; a
//! new review changes it, and Patronus is asked once. An answer with the evaluation of the pull request holds for a
//! day, one without it (an older Patronus, a start through the API) as long as before: 2 minutes while approvals are
//! missing, 10 without. Errors are never kept. A repository only on Space keeps its own path (`review_requirement`).

use crate::patronus_review::{ReviewInput, attribute_ownership_paths, read_review_inputs, skipped_review};
use anyhow::anyhow;
use domain_types::branch_names::{BranchKind, strip_branch_namespace, to_final_branch_name};
use domain_types::patronus::{
  BranchReviewReadiness, PatronusAvailability, PatronusReviewRequirement, ReviewReadiness, ReviewReadinessBranch, ReviewStart, UnavailableReadinessReason,
};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::is_object_id;
use object_cache::cache::ObjectCache;
use object_cache::review_answer::ReviewAnswer;
use patronus::client::GitHubPullRequestDto;
use patronus::review::ReviewSubject;
use patronus::review_readiness::{evaluates_pull_request, review_readiness};
use patronus::service::PatronusService;
use sha2::{Digest, Sha256};
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::{SpaceRepository, required_target_branch};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tracing::{debug, instrument};

/// An answer with the evaluation of the pull request changes only with what the key holds, or with the ownership rules
const EVALUATED_TTL_SECONDS: u32 = 24 * 3600;
/// An answer without it may change once approvals arrive in a Space merge request, or once Patronus evaluates pull
/// requests
const REQUIRED_TTL_SECONDS: u32 = 120;
const NOT_REQUIRED_TTL_SECONDS: u32 = 600;

/// Where the branches of a repository go for Patronus
pub struct ReadinessRepository<'a> {
  pub space: &'a SpaceRepository,
  /// The GitHub repository the branches are reviewed in; None for a repository only on Space
  pub github: Option<&'a GitHubRepository>,
  /// The branch a robot pushes into, e.g. `master`
  pub target: &'a str,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline: &'a str,
  pub branch_prefix: &'a str,
}

/// What `branch_review_readiness` is asked with, owned: as the readiness reads of the app and of the E2E test server
/// compose it (`prepare_review_readiness`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessInputs {
  pub space: SpaceRepository,
  pub github: Option<GitHubRepository>,
  /// The branch of origin the baseline tracks, e.g. `master`
  pub target: String,
  pub baseline: String,
  pub branch_prefix: String,
}

impl ReadinessInputs {
  pub fn repository(&self) -> ReadinessRepository<'_> {
    ReadinessRepository {
      space: &self.space,
      github: self.github.as_ref(),
      target: &self.target,
      baseline: &self.baseline,
      branch_prefix: &self.branch_prefix,
    }
  }
}

/// The inputs of `branch_review_readiness` for `branches` of the worktree `repository_path` against `baseline` (as the
/// sync detected it, `origin/master`), with the Space and GitHub repositories the caller resolved: every tip is a
/// commit id (the session asks what its view holds), and the target is the branch of origin the baseline tracks (one
/// git process; a local baseline is refused, Patronus pushes into a branch of origin); the branches are named under
/// `branch_prefix`, which the host resolved
pub fn prepare_review_readiness(
  git: &GitCommandExecutor,
  space: SpaceRepository,
  github: Option<GitHubRepository>,
  branch_prefix: &str,
  repository_path: &str,
  baseline: &str,
  branches: &[ReviewReadinessBranch],
) -> anyhow::Result<ReadinessInputs> {
  if let Some(branch) = branches.iter().find(|branch| !is_object_id(&branch.tip)) {
    anyhow::bail!("Not a commit id: {}", branch.tip);
  }
  let target = required_target_branch(git, repository_path, baseline)?;
  Ok(ReadinessInputs {
    space,
    github,
    target,
    baseline: baseline.to_owned(),
    branch_prefix: branch_prefix.to_owned(),
  })
}

/// Of `branches`, in their order, the answers the object cache keeps for what Patronus decides on that still hold
/// (`is_fresh`), without asking Patronus: one `git log` for the commits of all of them, one read of the cache, the
/// `OWNERSHIP` files for the groups named without their files; a branch too long to check is skipped (as the
/// pre-check skips it). `None` for each the cache doesn't answer, and for all of a repository only on Space (its
/// service keeps its own answers) or without the identity of the service
#[instrument(skip_all, fields(branches = branches.len()))]
pub async fn kept_review_readiness(
  git: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  service: &PatronusService,
  repository: &ReadinessRepository<'_>,
  branches: &[ReviewReadinessBranch],
) -> Vec<Option<BranchReviewReadiness>> {
  let (Some(identity), Some(_)) = (service.identity(), repository.github) else {
    return vec![None; branches.len()];
  };
  let tips: Vec<String> = branches.iter().map(|branch| branch.tip.clone()).collect();
  let (git, path, cache, baseline) = (git.clone(), repository_path.to_string(), cache.clone(), repository.baseline.to_string());
  let read = {
    let (git, path, cache, baseline) = (git.clone(), path.clone(), cache.clone(), baseline.clone());
    tokio::task::spawn_blocking(in_current_span(move || read_inputs(&git, &path, &cache, &baseline, &tips))).await
  };
  let Ok(inputs) = read else {
    return vec![None; branches.len()];
  };
  let keys: Vec<Option<[u8; 32]>> = branches
    .iter()
    .zip(&inputs)
    .map(|(branch, input)| match input {
      Ok(Some(input)) => Some(review_answer_key(&identity, repository, &input.paths, branch)),
      _ => None,
    })
    .collect();
  let branches = branches.to_vec();
  let count = branches.len();
  let kept = tokio::task::spawn_blocking(in_current_span(move || {
    let asked: Vec<[u8; 32]> = keys.iter().flatten().copied().collect();
    let mut found = cache.get_review_answers::<PatronusReviewRequirement>(&asked).into_iter();
    let now = unix_now();
    let mut answers: Vec<Option<PatronusReviewRequirement>> = inputs
      .iter()
      .zip(&keys)
      .map(|(input, key)| match (input, key) {
        (Ok(None), _) => Some(skipped_review()),
        (_, Some(_)) => found.next().flatten().filter(|kept| is_fresh(kept, now)).map(|kept| kept.answer),
        _ => None,
      })
      .collect();
    attribute_answered(&git, &path, &baseline, answers.iter_mut().map(Option::as_mut), &inputs);
    branches
      .into_iter()
      .zip(answers)
      .map(|(branch, answer)| answer.map(|answer| to_branch_readiness(branch, Ok(answer))))
      .collect::<Vec<_>>()
  }))
  .await;
  kept.unwrap_or_else(|_| vec![None; count])
}

/// The readiness of `branches`, in their order. `recheck` asks Patronus again, never the cache, and in the foreground
/// (a background check waits out the shared backoff of Patronus failures). The checks run one after another: the
/// pre-check is the heaviest call of Patronus.
#[instrument(skip_all, fields(branches = branches.len(), recheck))]
pub async fn branch_review_readiness(
  git: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  service: &PatronusService,
  repository: &ReadinessRepository<'_>,
  branches: &[ReviewReadinessBranch],
  recheck: bool,
) -> anyhow::Result<Vec<BranchReviewReadiness>> {
  let inputs = {
    let (git, path, cache, baseline) = (git.clone(), repository_path.to_string(), cache.clone(), repository.baseline.to_string());
    let tips: Vec<String> = branches.iter().map(|branch| branch.tip.clone()).collect();
    tokio::task::spawn_blocking(in_current_span(move || read_inputs(&git, &path, &cache, &baseline, &tips)))
      .await
      .map_err(|e| anyhow!("The review input task failed: {e}"))?
  };

  let identity = service.identity();
  let mut answers: Vec<Result<PatronusReviewRequirement, PatronusAvailability>> = Vec::with_capacity(branches.len());
  for (branch, input) in branches.iter().zip(&inputs) {
    let answer = match input {
      Err(message) => Err(PatronusAvailability::Error { message: message.clone() }),
      Ok(None) => Ok(skipped_review()),
      Ok(Some(input)) => ask(service, cache, repository, identity.as_deref(), branch, input, recheck).await,
    };
    answers.push(answer);
  }

  let (git, path, baseline) = (git.clone(), repository_path.to_string(), repository.baseline.to_string());
  let branches = branches.to_vec();
  tokio::task::spawn_blocking(in_current_span(move || {
    let mut answers = answers;
    attribute_answered(&git, &path, &baseline, answers.iter_mut().map(|answer| answer.as_mut().ok()), &inputs);
    branches.into_iter().zip(answers).map(|(branch, answer)| to_branch_readiness(branch, answer)).collect()
  }))
  .await
  .map_err(|e| anyhow!("The review readiness task failed: {e}"))
}

/// Gives the groups of each answer Patronus sent without paths the changed paths of its branch that their `OWNERSHIP`
/// entries own (`attribute_ownership_paths`, one read for all)
fn attribute_answered<'a>(
  git: &GitCommandExecutor,
  repository_path: &str,
  baseline: &str,
  answers: impl Iterator<Item = Option<&'a mut PatronusReviewRequirement>>,
  inputs: &'a [Result<Option<ReviewInput>, String>],
) {
  let mut owed: Vec<(&mut PatronusReviewRequirement, &[String])> = answers
    .zip(inputs)
    .filter_map(|(answer, input)| Some((answer?, input.as_ref().ok()?.as_ref()?.paths.as_slice())))
    .collect();
  attribute_ownership_paths(git, repository_path, baseline, &mut owed);
}

/// The commits and paths of each tip, one `git log` for all; when it fails (a tip gone since the page sent it), each tip
/// alone, so only the branch of a bad tip is unavailable, not the whole batch
fn read_inputs(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, baseline: &str, tips: &[String]) -> Vec<Result<Option<ReviewInput>, String>> {
  match read_review_inputs(git, repository_path, cache, baseline, tips) {
    Ok(inputs) => inputs.into_iter().map(Ok).collect(),
    Err(error) if tips.len() == 1 => vec![Err(format!("Cannot read the commits of the branch: {error}"))],
    Err(_) => tips
      .iter()
      .map(|tip| {
        read_review_inputs(git, repository_path, cache, baseline, std::slice::from_ref(tip))
          .map(|mut inputs| inputs.pop().flatten())
          .map_err(|error| format!("Cannot read the commits of the branch: {error}"))
      })
      .collect(),
  }
}

/// The answer of Patronus for one branch: the cached one unless `recheck`, else the pre-check, kept
async fn ask(
  service: &PatronusService,
  cache: &ObjectCache,
  repository: &ReadinessRepository<'_>,
  identity: Option<&str>,
  branch: &ReviewReadinessBranch,
  input: &ReviewInput,
  recheck: bool,
) -> Result<PatronusReviewRequirement, PatronusAvailability> {
  let source_branch = source_branch(repository.branch_prefix, branch).map_err(|message| PatronusAvailability::Error { message })?;
  let subject = ReviewSubject {
    project_key: &repository.space.project_key,
    repository: &repository.space.name,
    target_branch: repository.target,
    source_branch: &source_branch,
    commits: &input.commits,
    base: input.base(),
  };
  let Some(github) = repository.github else {
    // a repository only on Space: its merge request, cached by the service
    return service
      .review_requirement(&subject, recheck)
      .await
      .map_err(|message| PatronusAvailability::Error { message });
  };

  let key = identity.map(|identity| review_answer_key(identity, repository, &input.paths, branch));
  if !recheck
    && let Some(key) = key
    && let Some(Some(cached)) = cache.get_review_answers::<PatronusReviewRequirement>(&[key]).pop()
    && is_fresh(&cached, unix_now())
  {
    debug!(branch = branch.branch_name, "cached review answer");
    return Ok(cached.answer);
  }
  let full_name = github.full_name();
  let pull_request = match branch.start {
    ReviewStart::PullRequest { number } => Some(GitHubPullRequestDto { repository: &full_name, number }),
    ReviewStart::Rest => None,
  };
  let asked_at = unix_now();
  let requirement = service.review_check(&subject, pull_request, !recheck).await?;
  if let Some(key) = key {
    // a background answer replaces no answer kept since it was asked (a recheck beside it): that one is newer. A kept
    // answer older than the ask was not fresh, else the ask had returned it
    let newer_kept = !recheck
      && cache
        .get_review_answers::<PatronusReviewRequirement>(&[key])
        .pop()
        .flatten()
        .is_some_and(|kept| kept.fetched_at >= asked_at);
    if !newer_kept {
      cache.put_review_answers(&[(
        key,
        ReviewAnswer {
          fetched_at: unix_now(),
          answer: requirement.clone(),
        },
      )]);
    }
  }
  Ok(requirement)
}

/// The branch Patronus is asked about: the `source` of the branch, a stack branch of the prefix (the whole stack), else
/// the virtual branch of its name
fn source_branch(branch_prefix: &str, branch: &ReviewReadinessBranch) -> Result<String, String> {
  match &branch.source {
    Some(source) if strip_branch_namespace(source, branch_prefix, BranchKind::Stack).is_some() => Ok(source.clone()),
    Some(source) => Err(format!("{source} is not a stack branch of {branch_prefix}")),
    None => to_final_branch_name(branch_prefix, &branch.branch_name).map_err(|e| format!("{e:#}")),
  }
}

/// Whether a kept answer still holds at `now`
pub fn is_fresh(cached: &ReviewAnswer<PatronusReviewRequirement>, now: u32) -> bool {
  let ttl = if evaluates_pull_request(&cached.answer) {
    EVALUATED_TTL_SECONDS
  } else if cached.answer.required {
    REQUIRED_TTL_SECONDS
  } else {
    NOT_REQUIRED_TTL_SECONDS
  };
  now.saturating_sub(cached.fetched_at) < ttl
}

/// The key of an answer: what Patronus decides on. The paths, not the commits: a rebase onto a new baseline keeps it.
/// With a pull request its reviews too, sorted, so a new review asks again.
pub fn review_answer_key(identity: &str, repository: &ReadinessRepository<'_>, paths: &[String], branch: &ReviewReadinessBranch) -> [u8; 32] {
  let mut hasher = Sha256::new();
  let mut field = |value: &str| {
    hasher.update(value.as_bytes());
    hasher.update([0]);
  };
  field("review_answer.v1");
  field(identity);
  field(&repository.space.project_key);
  field(&repository.space.name);
  field(repository.target);
  field(&paths.len().to_string());
  for path in paths {
    field(path);
  }
  match branch.start {
    ReviewStart::PullRequest { number } => {
      field(&format!("pull request {number}"));
      field(branch.author.as_deref().unwrap_or(""));
      let mut reviews: Vec<String> = branch
        .reviewers
        .iter()
        .map(|reviewer| format!("{} {:?} {}", reviewer.login.to_lowercase(), reviewer.state, reviewer.requested))
        .collect();
      reviews.sort_unstable();
      for review in reviews {
        field(&review);
      }
    }
    ReviewStart::Rest => field("rest"),
  }
  hasher.finalize().into()
}

fn to_branch_readiness(branch: ReviewReadinessBranch, answer: Result<PatronusReviewRequirement, PatronusAvailability>) -> BranchReviewReadiness {
  let (requirement, readiness) = match answer {
    Ok(requirement) => {
      let reviewers = matches!(branch.start, ReviewStart::PullRequest { .. }).then_some(branch.reviewers.as_slice());
      // where the approvals of a repository only on Space count is the rule's (`domain_policy::review::review_state`)
      let readiness = review_readiness(&requirement, reviewers, branch.start);
      (Some(requirement), readiness)
    }
    Err(availability) => (None, unavailable(availability)),
  };
  BranchReviewReadiness {
    branch_name: branch.branch_name,
    tip: branch.tip,
    requirement,
    readiness,
  }
}

fn unavailable(availability: PatronusAvailability) -> ReviewReadiness {
  match availability {
    PatronusAvailability::NotLoggedIn => ReviewReadiness::Unavailable {
      reason: UnavailableReadinessReason::NotLoggedIn,
      message: "Run ./safePush.cmd once to log in to Patronus".to_string(),
    },
    PatronusAvailability::Error { message } => ReviewReadiness::Unavailable {
      reason: UnavailableReadinessReason::Error,
      message,
    },
    PatronusAvailability::Available | PatronusAvailability::NotPatronus => ReviewReadiness::Unavailable {
      reason: UnavailableReadinessReason::Error,
      message: "Patronus is not available for the repository".to_string(),
    },
  }
}
