use crate::invoke::{CommandResult, Rejection, failed};
use app_api::api_error::IntoApiError;
use axum::response::Json;
use domain_types::error::ApiError;
use domain_types::patronus::PatronusRobotsResult;
use domain_types::review_request::ReviewRequestsResult;
use domain_types::settings::{GitBranchPrefix, ResolvedBranchPrefix};
use repository_model::session::SessionKey;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::github_repository::{GitHubRepository, resolve_github_repository};
use sync_core::patronus::SpaceRepository;

use crate::remote_host::HeldAnswer;
use crate::remote_host::RemoteHost;
use crate::state::AppState;

use crate::state::TestRepository;

/// Runs a command off the async runtime, as the app runs its git work (`spawn_blocking`)
pub(crate) async fn run_command<T, E, F>(operation: &'static str, command: F) -> CommandResult<T>
where
  F: FnOnce() -> Result<T, E> + Send + 'static,
  T: Send + 'static,
  E: IntoApiError + Send + 'static,
{
  match tokio::task::spawn_blocking(command).await {
    Ok(Ok(value)) => Ok(Json(value)),
    Ok(Err(error)) => Err(failed(operation, error)),
    Err(join_error) => {
      tracing::error!("Cannot {operation}: the command panicked: {join_error}");
      Err(Rejection::Panicked)
    }
  }
}

pub(crate) fn repository_by_path<'a>(state: &'a AppState, path: &str) -> Result<dashmap::mapref::one::Ref<'a, String, TestRepository>, Rejection> {
  state
    .path_to_id
    .get(path)
    .and_then(|id| state.repositories.get(id.value()))
    .ok_or_else(|| failed("find the test repository", format!("Repository not accessible: {path}")))
}

/// What a command of the sync family resolves the prefix of a test repository with, as the app's `PrefixInputs`: the
/// settings of the test, the git prefixes of the server, the local git prefix as the git read
struct TestPrefixInputs {
  git: git_executor::git_command_executor::GitCommandExecutor,
  settings: domain_types::settings::AppSettings,
  prefixes: app_settings::branch_prefixes::BranchPrefixes,
}

impl TestPrefixInputs {
  /// The prefix of `repository_path` by its git config read now, kept for the commands after it; blocking
  fn fresh(&self, repository_path: &str) -> anyhow::Result<ResolvedBranchPrefix> {
    Ok(
      self
        .prefixes
        .refresh(&self.settings, repository_path, read_local_branch_prefix(&self.git, repository_path)?),
    )
  }
}

/// The inputs of the prefix of the test repository at `repository_path`; an error for an unknown one
fn prefix_inputs(state: &AppState, repository_path: &str) -> Result<TestPrefixInputs, Rejection> {
  let repository = repository_by_path(state, repository_path)?;
  Ok(TestPrefixInputs {
    git: state.git_executor.clone(),
    settings: repository.store.app(),
    prefixes: state.branch_prefixes.clone(),
  })
}

/// The prefix of the test repository at `repository_path` by its local config read now, kept for the commands after it:
/// what the run of its session checks its key against, as the app's run resolves it from the config it reads; blocking
pub(crate) fn branch_prefix_now(state: &AppState, repository_path: &str) -> anyhow::Result<String> {
  let inputs = prefix_inputs(state, repository_path).map_err(|_unknown| anyhow::anyhow!("Repository not accessible: {repository_path}"))?;
  Ok(inputs.fresh(repository_path)?.prefix)
}

/// Settings → Landing, the new API, of the settings of the test repository at `repository_path`, as the app's commands
/// read it from the writer (a test sets it with `initialStoreValues` or through the page's `set_setting`)
pub(crate) fn patronus_new_api_of(state: &AppState, repository_path: &str) -> Result<bool, Rejection> {
  Ok(repository_by_path(state, repository_path)?.store.app().patronus_new_api)
}

/// `branchdeck.branchPrefix` of the repository's local config (the git read of `resolve_branch_prefix`)
pub fn read_local_branch_prefix(git_executor: &git_executor::git_command_executor::GitCommandExecutor, repo_path: &str) -> anyhow::Result<GitBranchPrefix> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["config", "--local", "branchdeck.branchPrefix"], repo_path)?;
  match exit_code {
    0 => Ok(GitBranchPrefix {
      prefix: output.trim().to_string(),
      repository_scoped: true,
    }),
    1 => Ok(GitBranchPrefix::default()),
    128 => Err(anyhow::anyhow!("Repository not accessible: {repo_path}")),
    code => {
      tracing::warn!(code, repo_path, "Unexpected exit code reading local branch prefix");
      Ok(GitBranchPrefix::default())
    }
  }
}

/// The git of the config commands of a test repository, the git of its app (`AppState::context_of`): its global config
/// is the repository's own file (`AppState::global_config_of`), never the one of the developer, for a read and for a
/// write
pub(crate) fn config_git(state: &AppState, repository_path: &str) -> Result<git_executor::git_command_executor::GitCommandExecutor, Rejection> {
  Ok(repository_by_path(state, repository_path)?.context.git.clone())
}

/// The GitHub repository of the repository at `path` as the app resolves it (`RepositoryStateCache::github_repository`):
/// `branchdeck.githubRepository` (a fixture writes it), else none, read by the git of its config (`config_git`)
pub(crate) async fn github_of(state: &AppState, path: &str) -> Result<Option<GitHubRepository>, Rejection> {
  let (git, path) = (config_git(state, path)?, path.to_string());
  Ok(run_command("resolve the GitHub repository", move || resolve_github_repository(&git, &path)).await?.0)
}

/// The repository of a session, held as a command holds it while a read of its facts runs: its deletion waits
pub(crate) async fn holding(state: &AppState, key: &SessionKey) -> Result<tokio::sync::OwnedRwLockReadGuard<()>, ApiError> {
  let in_flight = Arc::clone(&repository_by_path(state, &key.worktree).map_err(rejected)?.in_flight);
  Ok(in_flight.read_owned().await)
}

/// The review requests of the fixture set by `POST /repositories/{id}/review-requests`, as the session of a test
/// repository reads them (`TestFactSources`): GitHub is never asked in E2E; counted
pub(crate) async fn review_requests_of(state: &AppState, key: &SessionKey, reviews: bool) -> Result<ReviewRequestsResult, ApiError> {
  let mut result = {
    let repo = repository_by_path(state, &key.worktree).map_err(rejected)?;
    let fixture = repo.review_requests.read().expect("review request fixture lock").clone();
    let mut result = fixture.result.clone();
    // the rest of Reviews only while the view shows, its links as the app keeps them; a link that went is forgotten
    result.others = None;
    if reviews && let domain_types::pull_request::PullRequestAvailability::Available { owner, name } = &result.availability {
      let full_name = format!("{owner}/{name}");
      let others = crate::review_fixtures::fixture_reviews(&fixture, &app_settings::linked_pull_requests::linked(&repo.store, &full_name));
      let gone = domain_policy::reviews::gone_links(&others);
      if !gone.is_empty() {
        let _ = app_settings::linked_pull_requests::forget(&repo.store, &full_name, &gone);
      }
      result.others = Some(others);
    }
    result
  };
  // an empty head is the tip of the branch the request names, as GitHub would show it (the fetch reviews that commit);
  // resolved before the call is counted, so a test that sees the count sees an answer that only has to be applied
  resolve_review_heads(&key.worktree, &mut result);
  let host = {
    let repo = repository_by_path(state, &key.worktree).map_err(rejected)?;
    repo.review_request_calls.fetch_add(1, Ordering::SeqCst);
    if reviews {
      repo.reviews_calls.fetch_add(1, Ordering::SeqCst);
    }
    repo.existing_remote()
  };
  // a test that holds the answer (`/remote-gates`) sees the page before it; no entry of the map is held meanwhile, so the
  // deletion of the repository, which releases it, can remove the repository
  if let Some(host) = host {
    host.pass_held(HeldAnswer::ReviewRequests).await;
  }
  Ok(result)
}

/// The heads of the fixture's pull requests that are empty, as the tips of the branches they name (else `HEAD`), read
/// from the refs of the repository without a git process: the answer of a listing stays as quick as the fixture
fn resolve_review_heads(path: &str, result: &mut ReviewRequestsResult) {
  let git_dir = std::path::Path::new(path).join(".git");
  if !git_dir.is_dir() {
    return;
  }
  let packed = std::fs::read_to_string(git_dir.join("packed-refs")).unwrap_or_default();
  let tip = |reference: &str| -> Option<String> {
    let loose = std::fs::read_to_string(git_dir.join(reference)).ok().map(|id| id.trim().to_owned());
    loose.filter(|id| id.len() == 40).or_else(|| {
      packed
        .lines()
        .find_map(|line| line.strip_suffix(reference).and_then(|id| id.strip_suffix(' ')).map(str::to_owned))
    })
  };
  let head = std::fs::read_to_string(git_dir.join("HEAD"))
    .ok()
    .and_then(|head| head.trim().strip_prefix("ref: ").map(str::to_owned))
    .and_then(|reference| tip(&reference))
    .unwrap_or_default();
  let mut resolve = |request: &mut domain_types::review_request::ReviewRequest| {
    if request.head_ref_oid.is_empty() {
      request.head_ref_oid = tip(&format!("refs/heads/{}", request.head_ref_name)).unwrap_or_else(|| head.clone());
    }
  };
  result.requests.iter_mut().for_each(&mut resolve);
  result.answered.iter_mut().for_each(|answered| resolve(&mut answered.request));
  if let Some(others) = &mut result.others {
    let linked = others.linked.iter_mut().filter_map(|linked| match linked {
      domain_types::review_request::LinkedAnswer::Open { entry } => Some(entry.as_mut()),
      _ => None,
    });
    for entry in others
      .team
      .entries
      .iter_mut()
      .chain(&mut others.reviewed.entries)
      .chain(&mut others.authored.entries)
      .chain(linked)
    {
      resolve(&mut entry.request);
    }
  }
}

/// The fake services of the repository at `path` (`remote_host.rs`), started now if not yet; the repository is not held
pub(crate) fn remote_of(state: &AppState, path: &str) -> Result<Arc<RemoteHost>, Rejection> {
  Ok(repository_by_path(state, path)?.remote(&state.git_executor))
}

/// The Space repository of the repository at `path`, None while no Patronus fixture put it on Space
pub(crate) fn space_of(state: &AppState, path: &str) -> Result<Option<(Arc<RemoteHost>, SpaceRepository)>, Rejection> {
  let host = repository_by_path(state, path)?.existing_remote();
  Ok(host.and_then(|host| host.space_repository().map(|space| (host, space))))
}

/// Keeps the robots of an answer by id, for the tests that pin a revision the fake resolved
pub(crate) fn keep_answered(state: &AppState, path: &str, result: &PatronusRobotsResult) {
  if let Ok(repository) = repository_by_path(state, path) {
    let mut answered = repository.patronus_answered.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    answered.extend(result.robots.iter().map(|robot| (robot.id.clone(), robot.clone())));
  }
}

/// The error of a rejection, a read of the session's facts answers it
pub(crate) fn rejected(rejection: Rejection) -> ApiError {
  match rejection {
    Rejection::Error(error) => error,
    Rejection::Panicked => ApiError::internal("The test server failed"),
  }
}
