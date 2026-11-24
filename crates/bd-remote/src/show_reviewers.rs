//! Whom a review after landing asks when the user names nobody (`brd safe-push --request-review`, `brd request-review
//! --after-landing` without `--reviewer`): the owner groups of the paths the branch changes, by the `OWNERSHIP` files of
//! the target (`git_ops::ownership`, the groups Patronus resolves too), their members and reviewers from the `CodeOwners`
//! service (`codeowners::client`, asked with the Space token of Patronus), one of each group by
//! `domain_policy::shown::show_candidates`, never the author (the user of the GitHub token). Two git processes (the
//! changed paths, the `OWNERSHIP` files), one lookup per group and one `GET /user`.

use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::services::cli_user_agent;
use codeowners::client::CodeOwnersClient;
use domain_policy::shown::{ShowCandidates, show_candidates};
use domain_types::error::ErrorCode;
use git_ops::ownership::read_file_owners;
use git_ops::paths::changed_paths_since_fork;
use github::service::GitHubService;
use patronus::token::{TokenLookup, find_token};
use remote_facts::pull_request_create::read_viewer_login;
use remote_facts::pull_requests::failure_message;
use std::collections::BTreeSet;
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
  let changed = changed_paths_since_fork(&context.git, &context.repository, target_ref, tip, |_| true)?;
  let paths: Vec<String> = changed.iter().map(|changed| String::from_utf8_lossy(changed.path()).into_owned()).collect();
  let owners: BTreeSet<String> = read_file_owners(&context.git, &context.repository, target_ref, &paths)?
    .into_iter()
    .map(|owner| owner.owner)
    .collect();
  let groups: Vec<String> = owners.into_iter().collect();
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
  let mut owner_groups = Vec::with_capacity(groups.len());
  for name in &groups {
    let group = runtime.block_on(client.lookup_group(&token.value, name)).map_err(|error| {
      let code = if matches!(error, codeowners::client::CodeOwnersError::Unauthorized) {
        ErrorCode::NotLoggedIn
      } else {
        ErrorCode::NetworkError
      };
      CliError::new(code, format!("{}; --reviewer <login> names the reviewers instead", error.message()))
    })?;
    owner_groups.push(group);
  }
  Ok((groups, show_candidates(&owner_groups, &author)))
}
