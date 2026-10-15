//! GitHub pull request status of the virtual branches, through the `gh` CLI (one `gh pr list` per call). The only write
//! is the Patronus command comment (`patronus_command.rs`). `gh` is not git: it never touches the repository, and every
//! failure is an availability, not an error.

use crate::remote_url::{parse_remote_url, split_owner_and_name};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::{BranchKind, strip_branch_namespace};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sync_types::pull_request::{ChecksState, ChecksSummary, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState};
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::{instrument, warn};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetPullRequestsParams {
  pub repository_path: String,
  pub branch_prefix: String,
}

/// `gh` answers within a few seconds; a hung network must not keep a process around
pub const GH_TIMEOUT: Duration = Duration::from_secs(30);

/// The user's recent pull requests: the virtual branches under review are among them
const PR_LIST_LIMIT: &str = "50";

const PR_FIELDS: &str = "number,title,url,state,isDraft,reviewDecision,baseRefName,headRefName,headRefOid,headRepositoryOwner,isCrossRepository,updatedAt,statusCheckRollup,latestReviews,reviewRequests,labels";

/// The label Patronus puts on a pull request it merged: it rebases, so GitHub closes the pull request instead of
/// marking it merged
pub const MERGED_LABEL: &str = "merged";

/// `owner/name` of the GitHub repository of the branches, for a remote that mirrors pushed branches to GitHub (e.g. a
/// Space repository); it wins over a github.com `origin`
pub const GITHUB_REPOSITORY_CONFIG: &str = "branchdeck.githubRepository";

/// Failed check names kept for the popover
const MAX_FAILED_NAMES: usize = 3;

/// The pull requests of the user's virtual branches in the GitHub repository of `origin` (`resolve_github_repository`).
#[instrument(skip(git_executor), fields(repository_path = %params.repository_path))]
pub fn get_pull_requests(git_executor: &GitCommandExecutor, params: &GetPullRequestsParams) -> Result<PullRequestsResult> {
  get_pull_requests_with(git_executor, params, find_gh().as_deref(), GH_TIMEOUT)
}

/// `get_pull_requests` with the `gh` binary (None = not installed) and the timeout given
pub fn get_pull_requests_with(git_executor: &GitCommandExecutor, params: &GetPullRequestsParams, gh: Option<&Path>, timeout: Duration) -> Result<PullRequestsResult> {
  let Some(repository) = resolve_github_repository(git_executor, &params.repository_path)? else {
    return Ok(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  };
  let Some(gh) = gh else {
    return Ok(PullRequestsResult::unavailable(PullRequestAvailability::GhMissing));
  };

  // `--repo`: with several remotes and no `gh repo set-default`, gh would not know which repository to ask
  let repo_arg = format!("github.com/{}/{}", repository.owner, repository.name);
  let args = [
    "pr",
    "list",
    "--repo",
    &repo_arg,
    "--author",
    "@me",
    "--state",
    "all",
    "--limit",
    PR_LIST_LIMIT,
    "--json",
    PR_FIELDS,
  ];
  let output = match run_gh(gh, &params.repository_path, &args, timeout) {
    Ok(output) => output,
    Err(availability) => return Ok(PullRequestsResult::unavailable(availability)),
  };
  match parse_pull_requests(&output, &params.branch_prefix, &repository.owner) {
    Ok(pull_requests) => Ok(PullRequestsResult {
      availability: PullRequestAvailability::Available {
        owner: repository.owner,
        name: repository.name,
      },
      pull_requests,
    }),
    Err(e) => {
      warn!(error = %e, "unexpected gh pr list output");
      Ok(PullRequestsResult::unavailable(PullRequestAvailability::Error {
        message: format!("Unexpected gh output: {e}"),
      }))
    }
  }
}

/// A repository on github.com
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepository {
  pub owner: String,
  pub name: String,
}

/// The GitHub repository the virtual branches end up in: `branchdeck.githubRepository` (`owner/name`), else the
/// repository `origin` pushes to (`push_branch` pushes to origin) when it is on github.com. None: the pull requests of
/// the branches are not on GitHub.
pub fn resolve_github_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<GitHubRepository>> {
  let (configured, exit_code) = git_executor.execute_command_with_status(&["config", GITHUB_REPOSITORY_CONFIG], repository_path)?;
  if exit_code == 0
    && let Some((owner, name)) = split_owner_and_name(configured.trim())
  {
    return Ok(Some(GitHubRepository {
      owner: owner.to_string(),
      name: name.to_string(),
    }));
  }
  let (url, exit_code) = git_executor.execute_command_with_status(&["remote", "get-url", "--push", "origin"], repository_path)?;
  if exit_code != 0 {
    return Ok(None);
  }
  Ok(parse_github_repository(&url))
}

/// The github.com repository of a remote URL: `git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`,
/// `https://github.com/o/r(.git)`; None for another host or a local path.
pub fn parse_github_repository(url: &str) -> Option<GitHubRepository> {
  let remote = parse_remote_url(url)?;
  // `ssh.github.com` is SSH over port 443
  if !remote.host.eq_ignore_ascii_case("github.com") && !remote.host.eq_ignore_ascii_case("ssh.github.com") {
    return None;
  }
  let (owner, name) = split_owner_and_name(remote.path)?;
  Some(GitHubRepository {
    owner: owner.to_string(),
    name: name.to_string(),
  })
}

/// The `gh` binary: Homebrew first, as for git (an app started from the Finder has no Homebrew in `PATH`), then `PATH`
pub fn find_gh() -> Option<PathBuf> {
  #[cfg(target_os = "macos")]
  for path in ["/opt/homebrew/bin/gh", "/usr/local/bin/gh"] {
    if Path::new(path).is_file() {
      return Some(PathBuf::from(path));
    }
  }

  let name = if cfg!(windows) { "gh.exe" } else { "gh" };
  let paths = std::env::var_os("PATH")?;
  std::env::split_paths(&paths).map(|dir| dir.join(name)).find(|path| path.is_file())
}

/// Runs `gh` without a terminal, prompts or update notices, killed after `timeout`; stdout on success.
/// Exit code 4 means not logged in (`gh help exit-codes`).
pub fn run_gh(gh: &Path, repository_path: &str, args: &[&str], timeout: Duration) -> Result<Vec<u8>, PullRequestAvailability> {
  let mut command = Command::new(gh);
  command
    .args(args)
    .current_dir(repository_path)
    .env("GH_PROMPT_DISABLED", "1")
    .env("GH_NO_UPDATE_NOTIFIER", "1")
    .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
    .env("NO_COLOR", "1")
    // terminal-style output would not be JSON
    .env_remove("GH_FORCE_TTY")
    .env_remove("CLICOLOR_FORCE")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(PullRequestAvailability::GhMissing),
    Err(e) => return Err(gh_error(format!("Cannot run gh: {e}"))),
  };
  // read both pipes while waiting: a full pipe would block gh
  let stdout = read_pipe(child.stdout.take());
  let stderr = read_pipe(child.stderr.take());

  let deadline = Instant::now() + timeout;
  let status = loop {
    match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if Instant::now() >= deadline => {
        // the readers are left behind: a grandchild may still hold the pipes open
        let _ = child.kill();
        let _ = child.wait();
        return Err(gh_error(format!("gh did not answer within {timeout:?}")));
      }
      Ok(None) => std::thread::sleep(Duration::from_millis(20)),
      Err(e) => return Err(gh_error(format!("Cannot wait for gh: {e}"))),
    }
  };

  let stdout = stdout.join().unwrap_or_default();
  let stderr = String::from_utf8_lossy(&stderr.join().unwrap_or_default()).trim().to_string();
  match status.code() {
    Some(0) => Ok(stdout),
    // an expired or revoked token fails the request instead of the local auth check
    Some(4) => Err(PullRequestAvailability::NotAuthenticated),
    _ if stderr.contains("HTTP 401") => Err(PullRequestAvailability::NotAuthenticated),
    code => {
      warn!(?code, %stderr, command = args.first().copied().unwrap_or_default(), "gh failed");
      let message = stderr.lines().find(|line| !line.trim().is_empty()).map_or_else(
        || format!("gh exited with {}", code.map_or_else(|| "a signal".to_string(), |code| format!("code {code}"))),
        |line| line.trim().to_string(),
      );
      Err(gh_error(message))
    }
  }
}

fn gh_error(message: String) -> PullRequestAvailability {
  PullRequestAvailability::Error { message }
}

fn read_pipe(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
  std::thread::spawn(move || {
    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
      let _ = pipe.read_to_end(&mut buffer);
    }
    buffer
  })
}

// ---- gh JSON (`gh pr list --json`); GraphQL nulls and empty enums are both possible ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequest {
  number: u32,
  title: String,
  url: String,
  state: String,
  #[serde(default)]
  is_draft: bool,
  review_decision: Option<String>,
  head_ref_name: String,
  head_ref_oid: String,
  #[serde(default)]
  base_ref_name: String,
  head_repository_owner: Option<GhLogin>,
  #[serde(default)]
  is_cross_repository: bool,
  updated_at: Option<String>,
  status_check_rollup: Option<Vec<GhCheck>>,
  latest_reviews: Option<Vec<GhReview>>,
  review_requests: Option<Vec<GhReviewRequest>>,
  labels: Option<Vec<GhLabel>>,
}

#[derive(Debug, Deserialize)]
struct GhLabel {
  name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhLogin {
  login: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
enum GhCheck {
  CheckRun {
    name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
  },
  StatusContext {
    context: Option<String>,
    state: Option<String>,
  },
  #[serde(other)]
  Other,
}

#[derive(Debug, Deserialize)]
struct GhReview {
  author: Option<GhLogin>,
  state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhReviewRequest {
  /// A user or a bot
  login: Option<String>,
  /// A team
  slug: Option<String>,
}

/// Parses `gh pr list --json` output and keeps the pull requests of virtual branches pushed to `owner`'s repository,
/// by virtual branch name. A fork's branch of the same name is not ours. Of several pull requests of one branch, an
/// open one wins over a merged one over a closed one, then the most recently updated.
pub fn parse_pull_requests(json: &[u8], branch_prefix: &str, owner: &str) -> serde_json::Result<BTreeMap<String, PullRequest>> {
  let list: Vec<GhPullRequest> = serde_json::from_slice(json)?;
  let mut result: BTreeMap<String, PullRequest> = BTreeMap::new();
  for gh in list {
    let same_owner = gh
      .head_repository_owner
      .as_ref()
      .and_then(|it| it.login.as_deref())
      .is_some_and(|login| login.eq_ignore_ascii_case(owner));
    if gh.is_cross_repository || !same_owner {
      continue;
    }
    let Some(name) = strip_branch_namespace(&gh.head_ref_name, branch_prefix, BranchKind::Virtual) else {
      continue;
    };
    let name = name.to_string();
    let pull_request = to_pull_request(gh);
    if result.get(&name).is_none_or(|existing| preference(&pull_request) > preference(existing)) {
      result.insert(name, pull_request);
    }
  }
  Ok(result)
}

fn preference(pull_request: &PullRequest) -> (u8, u32) {
  let state = match pull_request.state {
    PullRequestState::Open => 2,
    PullRequestState::Merged => 1,
    PullRequestState::Closed => 0,
  };
  (state, pull_request.updated_at)
}

fn to_pull_request(gh: GhPullRequest) -> PullRequest {
  let merged_by_patronus = || {
    gh.labels
      .iter()
      .flatten()
      .any(|label| label.name.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(MERGED_LABEL)))
  };
  let state = match gh.state.as_str() {
    "MERGED" => PullRequestState::Merged,
    "CLOSED" if merged_by_patronus() => PullRequestState::Merged,
    "CLOSED" => PullRequestState::Closed,
    _ => PullRequestState::Open,
  };
  let review_decision = match gh.review_decision.as_deref() {
    Some("APPROVED") => Some(ReviewDecision::Approved),
    Some("CHANGES_REQUESTED") => Some(ReviewDecision::ChangesRequested),
    Some("REVIEW_REQUIRED") => Some(ReviewDecision::ReviewRequired),
    _ => None,
  };
  PullRequest {
    number: gh.number,
    title: gh.title,
    url: gh.url,
    state,
    is_draft: gh.is_draft,
    review_decision,
    head_ref_name: gh.head_ref_name,
    head_ref_oid: gh.head_ref_oid,
    base_ref_name: gh.base_ref_name,
    checks: summarize_checks(&gh.status_check_rollup.unwrap_or_default()),
    updated_at: gh.updated_at.as_deref().and_then(parse_rfc3339_timestamp).unwrap_or(0),
    reviewers: collect_reviewers(gh.latest_reviews.unwrap_or_default(), gh.review_requests.unwrap_or_default()),
  }
}

enum CheckOutcome {
  Passed,
  Failed,
  Pending,
}

fn check_outcome(check: &GhCheck) -> Option<(CheckOutcome, Option<&str>)> {
  match check {
    GhCheck::CheckRun { name, status, conclusion } => {
      let outcome = if status.as_deref() == Some("COMPLETED") {
        match conclusion.as_deref() {
          Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE") => CheckOutcome::Failed,
          _ => CheckOutcome::Passed,
        }
      } else {
        CheckOutcome::Pending
      };
      Some((outcome, name.as_deref()))
    }
    GhCheck::StatusContext { context, state } => {
      let outcome = match state.as_deref() {
        Some("FAILURE" | "ERROR") => CheckOutcome::Failed,
        Some("PENDING" | "EXPECTED") => CheckOutcome::Pending,
        _ => CheckOutcome::Passed,
      };
      Some((outcome, context.as_deref()))
    }
    GhCheck::Other => None,
  }
}

/// Failing if any check failed, else pending if any is running, else passing; None without checks
fn summarize_checks(checks: &[GhCheck]) -> Option<ChecksSummary> {
  let mut summary = ChecksSummary {
    state: ChecksState::Passing,
    passed: 0,
    failed: 0,
    pending: 0,
    failed_names: Vec::new(),
  };
  for (outcome, name) in checks.iter().filter_map(check_outcome) {
    match outcome {
      CheckOutcome::Passed => summary.passed += 1,
      CheckOutcome::Pending => summary.pending += 1,
      CheckOutcome::Failed => {
        summary.failed += 1;
        if let Some(name) = name
          && summary.failed_names.len() < MAX_FAILED_NAMES
        {
          summary.failed_names.push(name.to_string());
        }
      }
    }
  }
  if summary.passed + summary.failed + summary.pending == 0 {
    return None;
  }
  summary.state = if summary.failed > 0 {
    ChecksState::Failing
  } else if summary.pending > 0 {
    ChecksState::Pending
  } else {
    ChecksState::Passing
  };
  Some(summary)
}

/// The latest review per reviewer (pending and dismissed ones say nothing), then the open review requests; a reviewer
/// asked again after a review is requested.
fn collect_reviewers(reviews: Vec<GhReview>, requests: Vec<GhReviewRequest>) -> Vec<Reviewer> {
  let mut reviewers: Vec<Reviewer> = reviews
    .into_iter()
    .filter_map(|review| {
      let state = match review.state.as_deref()? {
        "APPROVED" => ReviewerState::Approved,
        "CHANGES_REQUESTED" => ReviewerState::ChangesRequested,
        "COMMENTED" => ReviewerState::Commented,
        _ => return None,
      };
      Some(Reviewer {
        login: review.author?.login?,
        state,
      })
    })
    .collect();
  for login in requests.into_iter().filter_map(|request| request.login.or(request.slug)) {
    reviewers.retain(|reviewer| reviewer.login != login);
    reviewers.push(Reviewer {
      login,
      state: ReviewerState::Requested,
    });
  }
  reviewers
}
