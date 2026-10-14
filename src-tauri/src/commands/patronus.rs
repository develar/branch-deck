use git_executor::git_command_executor::GitCommandExecutor;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use moka::sync::Cache;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::token::find_user_token;
use std::time::Duration;
use sync_core::patronus::{SpaceRepository, find_space_repository};
use sync_types::patronus::{GetPatronusRobotsParams, PatronusAvailability, PatronusRobotsResult, PollPatronusRobotsParams};
use tauri::State;

/// The Patronus client of the app: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  service: PatronusService,
  /// By repository path; None = not on Space. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
}

impl PatronusState {
  pub fn new(app_version: &str) -> Self {
    let user_agent = format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH);
    // the variable of the Safe Push CLI, for the staging and dev instances
    let base_url = std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string());
    Self {
      service: PatronusService::new(PatronusClient::new(base_url, &user_agent), find_user_token),
      repositories: Cache::builder().max_capacity(100).time_to_live(Duration::from_secs(600)).build(),
    }
  }

  async fn space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<SpaceRepository>, CommandError> {
    if let Some(repository) = self.repositories.get(repository_path) {
      return Ok(repository);
    }
    let git = git_executor.clone();
    let path = repository_path.to_string();
    let repository = run_blocking(move || find_space_repository(&git, &path)).await?;
    self.repositories.insert(repository_path.to_string(), repository.clone());
    Ok(repository)
  }
}

/// The user's Patronus robots in the Space repository `origin` pushes to: the last 7 days, or the ones started since
/// `since`. When Patronus can't be asked, the availability says why.
#[tauri::command]
#[specta::specta]
pub async fn get_patronus_robots(
  git_executor: State<'_, GitCommandExecutor>,
  patronus: State<'_, PatronusState>,
  params: GetPatronusRobotsParams,
) -> Result<PatronusRobotsResult, CommandError> {
  let Some(repository) = patronus.space_repository(&git_executor, &params.repository_path).await? else {
    return Ok(PatronusRobotsResult::from(PatronusAvailability::NotPatronus));
  };
  Ok(patronus.service.robots(&repository.name, &params.branch_prefix, params.since, params.background).await)
}

/// The robots of `params.robots` whose status changed since the app saw it; the others are not in the result
#[tauri::command]
#[specta::specta]
pub async fn poll_patronus_robots(patronus: State<'_, PatronusState>, params: PollPatronusRobotsParams) -> Result<PatronusRobotsResult, CommandError> {
  Ok(patronus.service.poll(&params.branch_prefix, &params.robots).await)
}
