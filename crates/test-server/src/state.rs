use dashmap::DashMap;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::AtomicU32;
use sync_types::branch_name_suggestion::BranchNameSuggestionOutcome;
use sync_types::patronus::PatronusRobotsResult;
use sync_types::pull_request::{PullRequestAvailability, PullRequestsResult};
use sync_types::review_request::ReviewRequestsResult;

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
  pub templates_dir: PathBuf,
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  /// Held for reading by every request of the page mock for this repository (`hold_repository` in lib.rs) and for
  /// writing by `delete_repository`, so a repository is deleted only after the commands still running for it
  pub in_flight: std::sync::Arc<tokio::sync::RwLock<()>>,
  pub store: DashMap<String, serde_json::Value>,
  /// What `get_pull_requests` answers (E2E never runs gh); not on GitHub until a test sets it
  pub pull_requests: RwLock<PullRequestsResult>,
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
  /// What `get_patronus_robots` answers (E2E never calls Patronus); not on Space until a test sets it
  pub patronus: RwLock<PatronusRobotsResult>,
  /// How often the app listed and polled the robots, so a test can check the request policy
  pub patronus_list_calls: AtomicU32,
  pub patronus_poll_calls: AtomicU32,
  /// How often the app asked for the details of a robot: the build progress of its checks comes only with them
  pub patronus_detail_calls: AtomicU32,
  /// What the details answer as the progress of a running robot, by robot id (`POST /repositories/{id}/patronus-progress`)
  pub patronus_progress: RwLock<std::collections::HashMap<String, sync_types::patronus::PatronusRobotProgress>>,
  /// What `suggest_branch_names` answers (E2E never runs claude)
  pub branch_name_suggestions: RwLock<BranchNameSuggestionFixture>,
  /// How often the app asked for branch names: nothing may leave before the user enabled AI
  pub branch_name_suggestion_calls: AtomicU32,
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
