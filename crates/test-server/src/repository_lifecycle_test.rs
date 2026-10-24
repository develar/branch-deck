use crate::test_support::{capture_warnings, create_repository, delete_repository, post, start_test_server, start_test_server_with_state, sync};
use domain_types::repository_view::{GetRepositoryViewParams, RemoteFacts, RepositoryView, RobotMatch, WaitLandingNeedsParams};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// A request of the page mock for the repository `id`, which holds it while the command runs
fn command(client: &reqwest::Client, addr: SocketAddr, id: &str, command: &str, params: &Value) -> reqwest::RequestBuilder {
  client.post(format!("http://{addr}/invoke/{command}")).header(crate::REPOSITORY_ID_HEADER, id).json(params)
}

/// A failed command answers with 200 and its `ApiError`, as Tauri rejects a command, so neither the server nor the page
/// logs a failed request for the expected answer of an error-path test
#[tokio::test]
async fn a_failed_command_answers_with_its_api_error() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;

  let response = command(
    &client,
    addr,
    &id,
    "get_archived_branch_commits",
    &json!({ "repositoryPath": path, "branchName": "user-name/archived/2025-01-01/no-such-branch" }),
  )
  .send()
  .await
  .unwrap();
  assert_eq!(response.status(), 200);
  let answer: Value = response.json().await.unwrap();
  assert_eq!(answer["status"], "error");
  assert!(!answer["error"]["message"].as_str().unwrap().is_empty(), "{answer}");

  let answer: Value = command(
    &client,
    addr,
    &id,
    "get_commit_files",
    &json!({ "repositoryPath": "/no/such/repository", "commitId": "0".repeat(40) }),
  )
  .send()
  .await
  .unwrap()
  .json()
  .await
  .unwrap();
  assert_eq!(
    answer,
    json!({
      "status": "error",
      "error": { "code": "internal", "message": "Repository not accessible: /no/such/repository", "details": null, "retry": "no", "operation_id": null }
    })
  );
}

/// The deletion at the end of a test removes the files once the command still running for the repository ended, also
/// when the page has closed and the browser aborted the request, and refuses a request that comes after it
#[tokio::test]
async fn deleting_a_repository_waits_for_its_running_command() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let delay_ms: u64 = 600;
  let delay = Duration::from_millis(delay_ms);
  client
    .post(format!("http://{addr}/repositories/{id}/branch-name-suggestions"))
    .json(&json!({ "delayMs": delay_ms }))
    .send()
    .await
    .unwrap();

  let started = Instant::now();
  // the page closes while the command runs: the client gives up on the request
  let aborted = tokio::time::timeout(
    Duration::from_millis(100),
    command(&client, addr, &id, "suggest_branch_names", &json!({ "repositoryPath": path, "commits": [] })).send(),
  )
  .await;
  assert!(aborted.is_err(), "the command answered before its delay");

  delete_repository(&client, addr, &id, &path).await;
  assert!(started.elapsed() >= delay, "deleted after {:?}, before the command ended", started.elapsed());

  let late = command(&client, addr, &id, "get_commit_files", &json!({ "repositoryPath": path, "commitId": "0".repeat(40) }))
    .send()
    .await
    .unwrap();
  assert_eq!(late.status(), 410);
}

/// The view of `path` under the prefix `user-name` as the page asks it: it starts the computation of the landing needs
/// in the background, beyond the command
async fn ask_view(state: &std::sync::Arc<crate::state::AppState>, path: &str) -> RepositoryView {
  let (views, git) = (state.repository_views.clone(), state.git_executor.clone());
  let params = GetRepositoryViewParams {
    repository_path: path.to_string(),
    revision: 1,
    robots: Vec::new(),
    matched: RobotMatch::default(),
    remote: RemoteFacts::default(),
  };
  tokio::task::spawn_blocking(move || views.view(&git, &ObjectCache::disabled(), "user-name", &params))
    .await
    .unwrap()
}

/// The deletion at the end of a test removes the files once the computation of the landing needs a view started ended,
/// which no command holds, and forgets the views of the repository: no git of it runs in the deleted directory
#[tokio::test]
async fn deleting_a_repository_waits_for_its_landing_needs_and_forgets_its_views() {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  sync(&client, addr, &path).await;
  let view = ask_view(&state, &path).await;
  assert_eq!((view.table_generation > 0, view.landing_generation), (true, 0), "the computation started: {view:?}");

  delete_repository(&client, addr, &id, &path).await;
  let wait = WaitLandingNeedsParams {
    repository_path: path.clone(),
    after: 0,
  };
  assert_eq!(state.repository_views.wait_landing_needs("user-name", &wait), 0, "no computation is left to wait for");
  assert_eq!(ask_view(&state, &path).await.table_generation, 0, "the table of the sync is forgotten");
}

/// A test that ended while it held the repository lock (`holdRepositoryLock`) has it released with the deletion,
/// before the directory goes: the lock deletes its holder record, which is still there, and logs nothing
#[tokio::test]
async fn deleting_a_repository_releases_the_lock_its_test_held_before_the_directory_goes() {
  // what is logged once the server started: its start warns of a `dist` without index.html, which CI builds only after
  // the Rust tests
  let addr = start_test_server().await;
  let logged = capture_warnings();
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  post(&client, addr, &format!("/repositories/{id}/repository-lock"), json!({ "held": true })).await;
  assert!(std::path::Path::new(&path).join(".git/branch-deck.lock.holder").exists(), "the holder record of the lock");

  delete_repository(&client, addr, &id, &path).await;
  assert_eq!(String::from_utf8_lossy(&logged.lock().unwrap()), "");
}
