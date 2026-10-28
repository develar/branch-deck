use git_executor::git_command_executor::GitCommandExecutor;
use github::client::GitHubClient;
use github::service::GitHubService;
use github::token::{TokenChain, TokenSources};
use std::path::Path;
use std::sync::Arc;

/// The GitHub client of the app: one HTTP client (kept-alive connection) and the token chain with what GitHub accepted
/// and rejected, shared by the windows; its own sign-in is one of the sign-ins of the app (`commands::sign_in`)
pub struct GitHubState {
  pub service: Arc<GitHubService>,
}

impl GitHubState {
  pub fn new(app_version: &str, git: GitCommandExecutor, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: Arc::new(GitHubService::new(
        GitHubClient::new(&app_user_agent(app_version)),
        TokenChain::new(TokenSources::from_process(app_data_dir), git),
      )),
    }
  }
}

/// `Branch Deck/<version> (<os>; <arch>)`, the user agent of the app's HTTP requests
pub fn app_user_agent(app_version: &str) -> String {
  format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH)
}
