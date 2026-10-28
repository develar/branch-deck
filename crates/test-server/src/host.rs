//! What the app of a test repository gives the commands of app-api beside the services of its sessions
//! (`app_api::transport::AppHost`): the accounts of the repository's fakes (`RemoteHost`, started on the first need),
//! no `teamcity` CLI, the branch names of its fixture (E2E never runs claude), and the page's clock in UTC, so the weeks
//! and the hours of the Safe Push statistics of a test are fixed. A read of a status the test holds waits at the gate
//! of its answer (`held_or_failed`)

use crate::invoke::{Rejection, failed};
use crate::remote_host::{HeldAnswer, PAGE_CLOCK_START, RemoteHost};
use crate::state::AppState;
use crate::tauri_command_bridge::rejected;
use app_api::transport::AppHost;
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::teamcity_cli::TeamCityCli;
use remote_facts::fact_sources::Read;
use remote_facts::sign_in::SignIns;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use sync_core::branch_name_suggestion::clean_names;
use youtrack::service::YouTrackService;

/// The host of the app of the test repository `repository` (`AppState::context_of`)
pub struct TestHost {
  pub state: Weak<AppState>,
  /// The id of the test repository
  pub repository: String,
}

impl TestHost {
  /// The fakes of the test repository, started now if not yet
  fn remote(&self) -> Result<Arc<RemoteHost>, ApiError> {
    let state = self.state.upgrade().ok_or_else(|| ApiError::internal("The test server stopped"))?;
    let repository = state.repositories.get(&self.repository);
    let repository = repository.ok_or_else(|| ApiError::internal(format!("The test repository {} is gone", self.repository)))?;
    Ok(repository.remote(&state.git_executor))
  }
}

impl AppHost for TestHost {
  fn sign_ins(&self) -> Result<SignIns, ApiError> {
    Ok(self.remote()?.sign_ins().clone())
  }

  fn youtrack(&self) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(self.remote()?.shared_youtrack_service())
  }

  /// Held and failed as the test says (`HeldAnswer::AuthStatus`)
  fn before_auth_status(&self) -> Read<'_, Result<(), ApiError>> {
    Box::pin(async move {
      let remote = self.remote()?;
      held_or_failed(&remote, HeldAnswer::AuthStatus, "read the sign-ins").await.map_err(rejected)
    })
  }

  /// Nothing of the developer's machine: no `teamcity` CLI
  fn teamcity(&self) -> Read<'_, Option<TeamCityCli>> {
    Box::pin(async { None })
  }

  /// The fixture of the repository after its delay (`POST /repositories/{id}/branch-name-suggestions`), counted, by
  /// default two names from the words of the commit messages; blocking, as the app's `claude` is
  fn suggest_branch_names(&self, _git: &GitCommandExecutor, params: &SuggestBranchNamesParams, _is_superseded: &dyn Fn() -> bool) -> BranchNameSuggestionOutcome {
    let fixture = self.state.upgrade().and_then(|state| {
      let repository = state.repositories.get(&self.repository)?;
      repository.branch_name_suggestion_calls.fetch_add(1, Ordering::SeqCst);
      let fixture = repository.branch_name_suggestions.read().expect("branch name suggestion fixture lock").clone();
      Some(fixture)
    });
    let Some(fixture) = fixture else {
      return BranchNameSuggestionOutcome::Error {
        message: format!("The test repository {} is gone", self.repository),
      };
    };
    std::thread::sleep(std::time::Duration::from_millis(fixture.delay_ms));
    if let Some(outcome) = fixture.outcome {
      return outcome;
    }
    let words: Vec<&str> = params
      .commits
      .iter()
      .flat_map(|commit| commit.message.split_whitespace())
      .filter(|word| word.len() > 3)
      .collect();
    let names = clean_names(&[
      format!("fix-{}", words.first().unwrap_or(&"issue")),
      format!("update-{}", words.get(1).unwrap_or(&"feature")),
    ]);
    BranchNameSuggestionOutcome::Suggested { names }
  }

  /// The start of the page's clock (`PAGE_CLOCK_START`)
  fn now(&self) -> u32 {
    PAGE_CLOCK_START
  }

  fn calendar(&self) -> Calendar {
    Calendar::utc()
  }
}

/// Waits while a test holds `answer` (`/remote-gates`), then fails it while the test fails it: a read of a status the
/// server gives itself
pub(crate) async fn held_or_failed(host: &RemoteHost, answer: HeldAnswer, operation: &str) -> Result<(), Rejection> {
  host.pass_held(answer).await;
  if host.fails(answer) {
    return Err(failed(operation, "the fixture fails this read"));
  }
  Ok(())
}
