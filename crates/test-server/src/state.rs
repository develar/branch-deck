use dashmap::DashMap;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::AtomicU32;
use sync_types::branch_name_suggestion::BranchNameSuggestionOutcome;
use sync_types::patronus::PatronusReviewRequirement;
use sync_types::pull_request::PullRequestAvailability;
use sync_types::review_request::ReviewRequestsResult;

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
  pub templates_dir: PathBuf,
  /// The table each repository's last sync finished, which `get_repository_view` answers from, as the app keeps it
  pub repository_views: repository_view::views::RepositoryViews,
}

impl AppState {
  /// The global git config of a test repository, beside its directory: `GIT_CONFIG_GLOBAL` of its config reads and
  /// writes (`set_repository_config` with `scope: global`), so no test reads or writes the config of the developer,
  /// and two tests that run at once never share one. Deleted with the repository
  pub fn global_config_of(&self, repository_id: &str) -> PathBuf {
    self.test_root_dir.path().join(format!("{repository_id}.gitconfig"))
  }
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  /// Held for reading by every request of the page mock for this repository (`hold_repository` in lib.rs) and for
  /// writing by `delete_repository`, so a repository is deleted only after the commands still running for it
  pub in_flight: std::sync::Arc<tokio::sync::RwLock<()>>,
  pub store: DashMap<String, serde_json::Value>,
  /// How often the app asked for pull requests, so a test can check the refresh policy
  pub pull_request_calls: AtomicU32,
  /// What `get_review_requests` answers and how `fetch_review_head` fails (E2E never runs gh); not on GitHub until a
  /// test sets it
  pub review_requests: RwLock<ReviewRequestsFixture>,
  /// How often the app asked for review requests and fetched a review
  pub review_request_calls: AtomicU32,
  pub review_fetch_calls: AtomicU32,
  /// The `keep` of every `prune_review_refs`, in call order
  pub review_prune_keeps: std::sync::Mutex<Vec<Vec<u32>>>,
  /// The fake services behind the real ones (`remote_host.rs`), started on the first need; stopped with the repository
  pub remote: std::sync::OnceLock<std::sync::Arc<crate::remote_host::RemoteHost>>,
  /// How often the page listed and polled the robots, so a test can check its request policy
  pub patronus_list_calls: AtomicU32,
  pub patronus_poll_calls: AtomicU32,
  /// How often the app asked for the details of a robot: the build progress of its checks comes only with them
  pub patronus_detail_calls: AtomicU32,
  /// What Patronus answers the review pre-check of `get_branch_review_readiness` with (E2E never calls Patronus)
  pub review_answers: RwLock<ReviewAnswersFixture>,
  /// The `get_branch_review_readiness` calls, in order: the branches asked and whether it was a recheck
  pub review_readiness_calls: std::sync::Mutex<Vec<serde_json::Value>>,
  /// What `suggest_branch_names` answers (E2E never runs claude)
  pub branch_name_suggestions: RwLock<BranchNameSuggestionFixture>,
  /// How often the app asked for branch names: nothing may leave before the user enabled AI
  pub branch_name_suggestion_calls: AtomicU32,
  /// The Agent CLI was installed (`install_agent_cli`): `agent_cli_status` names the skills then
  pub agent_cli_installed: std::sync::atomic::AtomicBool,
}

impl TestRepository {
  /// The fake services of the repository, started now if not yet
  pub fn remote(&self, git: &GitCommandExecutor) -> std::sync::Arc<crate::remote_host::RemoteHost> {
    std::sync::Arc::clone(self.remote.get_or_init(|| std::sync::Arc::new(crate::remote_host::RemoteHost::start(git, &self.path))))
  }

  /// The fake services once started
  pub fn existing_remote(&self) -> Option<std::sync::Arc<crate::remote_host::RemoteHost>> {
    self.remote.get().cloned()
  }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequestsFixture {
  /// An empty `headRefOid` or `baseRefOid` of a request is resolved by `fetch_review_head`: the tip of `HEAD`, the
  /// baseline
  #[serde(flatten)]
  pub result: ReviewRequestsResult,
  /// The error `fetch_review_head` fails with; None: it answers the range of the test repository
  #[serde(default)]
  pub fetch_error: Option<String>,
}

impl Default for ReviewRequestsFixture {
  fn default() -> Self {
    Self {
      result: ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
      fetch_error: None,
    }
  }
}

/// The answers of the Patronus pre-check (`POST /repositories/{id}/review-answers`); the readiness is computed from them
/// as the app does (`patronus::review_readiness`), with the reviewers and the start the page sends
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAnswersFixture {
  /// By virtual branch name. A branch without one: the owners of the files it changes by the `OWNERSHIP` files of the
  /// baseline, one group per owner, none approved, no reviewers, as a Patronus that doesn't evaluate pull requests
  #[serde(default)]
  pub answers: std::collections::HashMap<String, PatronusReviewRequirement>,
  /// Patronus fails every check with this message
  #[serde(default)]
  pub unavailable: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchNameSuggestionFixture {
  /// None: two names from the words of the commit messages
  #[serde(default)]
  pub outcome: Option<BranchNameSuggestionOutcome>,
  /// How long the answer takes, so a test sees the request in flight
  #[serde(default = "default_suggestion_delay_ms")]
  pub delay_ms: u64,
}

const fn default_suggestion_delay_ms() -> u64 {
  100
}
