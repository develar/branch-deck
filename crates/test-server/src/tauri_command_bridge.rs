use crate::invoke::{CommandResult, Rejection, failed};
use app_api::api_error::IntoApiError;
use axum::{
  extract::State,
  http::{HeaderMap, StatusCode},
  response::Json,
};
use domain_types::error::ApiError;
use domain_types::patronus::PatronusRobotsResult;
use domain_types::review_request::ReviewRequestsResult;
use domain_types::settings::{GitBranchPrefix, ResolvedBranchPrefix};
use repository_model::session::SessionKey;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::github_repository::{GitHubRepository, resolve_github_repository};
use sync_core::patronus::SpaceRepository;
use sync_core::repository_config::{ConfigScope, RepositoryConfig, read_config_entries, read_origin_facts, repository_config_from_entries, with_stacks_off_clashes};

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

/// The production read without the system config, its global config the repository's own file (`config_git`), so the
/// tests are deterministic
fn read_test_repository_config(git_executor: &git_executor::git_command_executor::GitCommandExecutor, repository_path: &str) -> anyhow::Result<RepositoryConfig> {
  let entries: Vec<_> = read_config_entries(git_executor, repository_path)?
    .into_iter()
    .filter(|entry| matches!(entry.scope, ConfigScope::Local | ConfigScope::Global))
    .collect();
  let config = repository_config_from_entries(&entries, read_origin_facts(git_executor, repository_path)?);
  Ok(with_stacks_off_clashes(git_executor, repository_path, config))
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

pub async fn browse_repository(
  State(state): State<Arc<AppState>>,
  axum::extract::Path(repo_id): axum::extract::Path<String>,
) -> Result<Json<sync_core::repository_validation::BrowseResult>, StatusCode> {
  // Get the repository from state
  let repo = state.repositories.get(&repo_id).ok_or_else(|| {
    tracing::error!("Repository not found: {}", repo_id);
    StatusCode::NOT_FOUND
  })?;

  let path = repo.path.clone();

  // Handle NO_REPO case specially - simulate user selecting a non-existent path
  // Return an error result as if the path exists but is not accessible
  if path.starts_with(crate::NO_REPO_PREFIX) {
    return Ok(Json(sync_core::repository_validation::BrowseResult {
      path: Some(path),
      valid: false,
      error: Some("Path does not exist".to_string()),
    }));
  }

  // Use shared validation logic from branch-sync
  Ok(Json(sync_core::repository_validation::validate_and_create_result(path)))
}

#[derive(Deserialize)]
pub struct SetAutoSyncMenuCheckedRequest {
  checked: bool,
}

/// The bd the copied prompts name: a fixed path, the app's `agent_cli_command` reads the running binary
pub const AGENT_CLI: &str = "/Users/test/.local/bin/bd";

#[derive(Deserialize)]
pub struct AgentCliCommandRequest {
  #[serde(rename = "repositoryPath")]
  repository_path: String,
}

/// `AgentCliCommand` of the app for a POSIX shell, with the repository path as it is (test paths are plain)
pub async fn agent_cli_command(Json(request): Json<AgentCliCommandRequest>) -> Json<serde_json::Value> {
  Json(serde_json::json!({ "command": format!("{AGENT_CLI} -C {}", request.repository_path), "powershell": false }))
}

/// The skill files the fake install names
const AGENT_SKILLS: [&str; 2] = ["/Users/test/.claude/skills/branch-deck/SKILL.md", "/Users/test/.agents/skills/branch-deck/SKILL.md"];

/// The repository of the page that sends a command (`X-Repository-Id`)
pub(crate) fn requesting_repository<'a>(state: &'a AppState, headers: &HeaderMap) -> Result<dashmap::mapref::one::Ref<'a, String, TestRepository>, Rejection> {
  headers
    .get(crate::REPOSITORY_ID_HEADER)
    .and_then(|value| value.to_str().ok())
    .and_then(|id| state.repositories.get(id))
    .ok_or_else(|| failed("find the test repository", "no test repository named by the request"))
}

/// `bd install` without its files: the installed state of the test repository, and the agent rules of its own global
/// config, as the app reads the developer's
pub async fn install_agent_cli(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<serde_json::Value> {
  let path = {
    let repository = requesting_repository(&state, &headers)?;
    repository.agent_cli_installed.store(true, Ordering::SeqCst);
    repository.path.clone()
  };
  let git_executor = config_git(&state, &path)?;
  run_command("install the Agent CLI", move || {
    // the global config alone, as `bd install` reads it
    read_test_repository_config(&git_executor, &path).map(|config| {
      serde_json::json!({
        "bd": AGENT_CLI,
        "on_path": true,
        "skills": AGENT_SKILLS,
        "agents": { "lease": config.agent_lease.fallback_value, "subject_branch": config.agent_subject_branch.fallback_value },
      })
    })
  })
  .await
}

/// The skills once `install_agent_cli` ran for the test repository, else none
pub async fn agent_cli_status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> CommandResult<serde_json::Value> {
  let installed = requesting_repository(&state, &headers)?.agent_cli_installed.load(Ordering::SeqCst);
  Ok(Json(if installed {
    serde_json::json!({ "bd": AGENT_CLI, "skills": AGENT_SKILLS })
  } else {
    serde_json::json!({ "bd": null, "skills": [] })
  }))
}

/// The page has no native menu: the check is the request, which a test reads (`checked` of its body)
pub async fn set_auto_sync_menu_checked(Json(request): Json<SetAutoSyncMenuCheckedRequest>) -> Json<serde_json::Value> {
  tracing::debug!(checked = request.checked, "Auto-sync on Focus checked");
  Json(serde_json::Value::Null)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubWindowLoadedRequest {
  pub since: u32,
  /// The label of the page's window
  pub origin: String,
}

/// `sub_window_loaded` of the app: the changes of the settings after the snapshot the window was built with, which the
/// page mock names (`since`, the version of the snapshot the test opened the page with), as the app answers them from
/// the version its build remembered; none for the main window, which asks for them itself (`get_settings_changes`); no
/// open of the window came while it loaded, a test opens the page itself
pub async fn sub_window_loaded(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(request): Json<SubWindowLoadedRequest>) -> CommandResult<serde_json::Value> {
  let repository = requesting_repository(&state, &headers)?;
  let settings = if request.origin == app_settings::windows::MAIN_WINDOW {
    Vec::new()
  } else {
    app_settings::windows::window_changes_since(&repository.store, &request.origin, request.since)
  };
  Ok(Json(serde_json::json!({ "update": null, "settings": settings })))
}

/// `sub_window_drawn` of the app, which counts the content budget of the window from the shell: this server shows no
/// window; the page mock records what the page had when it said it
pub async fn sub_window_drawn() -> Json<serde_json::Value> {
  Json(serde_json::Value::Null)
}
