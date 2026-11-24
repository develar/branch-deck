//! Whom a review after landing asks when the user names nobody (`brd safe-push --request-review`, `brd request-review
//! --after-landing` without `--reviewer`), read as the app's picker reads them (`remote_facts::show_reviewers`): the
//! owner groups of the paths the branch changes, one available member of each from the `CodeOwners` service, asked with
//! the Space token of the Safe Push CLI, never the author (the user of the GitHub token). Two git processes (the changed
//! paths, the `OWNERSHIP` files), one lookup per group and one `GET /user`.

use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::services::cli_user_agent;
use codeowners::client::CodeOwnersClient;
use domain_policy::shown::ShowCandidates;
use domain_types::error::ErrorCode;
use github::service::GitHubService;
use patronus::token::{TokenLookup, find_token};
use remote_facts::pull_request_create::read_viewer_login;
use remote_facts::pull_requests::failure_message;
use remote_facts::show_reviewers::{lookup_candidates, owner_groups};
use sync_core::github_repository::GitHubRepository;
use sync_utils::timestamp::unix_now;

/// Whom a review after landing of the commits up to `tip` asks: one of each owner group of what they change against
/// `target_ref` (`refs/remotes/origin/master`), and the groups of which nobody can be asked
///
/// # Errors
/// No Space token for the `CodeOwners` service, a failure of git, of the service or of GitHub
pub fn show_reviewers(
  context: &RepoContext,
  environment: &Environment,
  github: &GitHubService,
  repository: &GitHubRepository,
  tip: &str,
  target_ref: &str,
) -> Result<(Vec<String>, ShowCandidates), CliError> {
  let groups = owner_groups(&context.git, &context.repository, target_ref, tip)?;
  if groups.is_empty() {
    return Ok((groups, ShowCandidates::default()));
  }
  let TokenLookup::Found(token) = find_token(environment.home.as_deref(), environment.space_token.clone(), unix_now()) else {
    return Err(CliError::new(
      ErrorCode::NotLoggedIn,
      "No Space token for the CodeOwners service, or it expired: run ./safePush.cmd once to log in, or set SPACE_TOKEN; --reviewer <login> names the reviewers instead",
    ));
  };
  let mut client = CodeOwnersClient::new(environment.codeowners_url.clone(), &cli_user_agent());
  if let Some(delay) = environment.remote_retry_delay {
    client = client.with_retry_delay(delay);
  }
  let runtime = context.runtime()?;
  let author = runtime
    .block_on(read_viewer_login(github, &context.repository, repository))
    .map_err(|failure| CliError::new(ErrorCode::NetworkError, failure_message(&failure)))?;
  let candidates = runtime.block_on(lookup_candidates(&client, &token.value, &groups, &author)).map_err(|error| {
    let code = if matches!(error, codeowners::client::CodeOwnersError::Unauthorized) {
      ErrorCode::NotLoggedIn
    } else {
      ErrorCode::NetworkError
    };
    CliError::new(code, format!("{}; --reviewer <login> names the reviewers instead", error.message()))
  })?;
  Ok((groups, candidates))
}
