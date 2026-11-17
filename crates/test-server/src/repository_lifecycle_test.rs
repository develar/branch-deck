use crate::session_routes::TestSessionEnvironment;
use crate::test_support::{
  Named, capture_warnings, command, create_repository, delete_repository, post, start_test_server, start_test_server_waiting, start_test_server_with_git,
  start_test_server_with_state, sync, wait_until_removed,
};
use pretty_assertions::assert_eq;
use repository_model::runtime::SessionEnvironment;
use repository_model::session::SessionKey;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

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
    Named::Id(&id),
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

  // the body of the app fails as in the app, and its error names its operation
  let mut answer: Value = command(
    &client,
    addr,
    Named::Id(&id),
    "get_commit_files",
    &json!({ "repositoryPath": "/no/such/repository", "commitId": "0".repeat(40) }),
  )
  .send()
  .await
  .unwrap()
  .json()
  .await
  .unwrap();
  let operation_id = answer["error"]["operation_id"].take();
  assert_eq!(operation_id.as_str().map(str::len), Some(26), "{answer}");
  assert_eq!((answer["status"].as_str(), answer["error"]["code"].as_str()), (Some("error"), Some("internal")), "{answer}");
  assert!(!answer["error"]["message"].as_str().unwrap_or_default().is_empty(), "{answer}");
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
    command(&client, addr, Named::Id(&id), "suggest_branch_names", &json!({ "repositoryPath": path, "commits": [] })).send(),
  )
  .await;
  assert!(aborted.is_err(), "the command answered before its delay");

  delete_repository(&client, addr, &id, &path).await;
  assert!(started.elapsed() >= delay, "deleted after {:?}, before the command ended", started.elapsed());

  let late = command(
    &client,
    addr,
    Named::Id(&id),
    "get_commit_files",
    &json!({ "repositoryPath": path, "commitId": "0".repeat(40) }),
  )
  .send()
  .await
  .unwrap();
  assert_eq!(late.status(), 410);
}

/// The deletion at the end of a test removes the files once the computation of the landing needs the session of the
/// repository runs ended: the computation holds the repository as a command does (`TestSessionEnvironment::landing_needs`),
/// so no git of it runs in the deleted directory. Here the computation waits at the gate of its answer, which the
/// deletion releases; it still finds its repository, and the files go only after it
#[tokio::test]
async fn deleting_a_repository_waits_for_the_landing_needs_its_session_computes() {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  sync(&client, addr, &path).await;
  post(&client, addr, &format!("/repositories/{id}/remote-gates"), json!({ "hold": ["landingNeeds"] })).await;

  let environment = TestSessionEnvironment {
    state: std::sync::Arc::downgrade(&state),
  };
  let key = SessionKey {
    worktree: path.clone(),
    prefix: "user-name".to_string(),
  };
  let computed = tokio::task::spawn_blocking(move || environment.landing_needs(&key, &std::collections::HashSet::new()));
  let arrived = Instant::now() + Duration::from_mins(1);
  loop {
    let gates: Value = client
      .get(format!("http://{addr}/repositories/{id}/remote-gates"))
      .send()
      .await
      .unwrap()
      .json()
      .await
      .unwrap();
    if gates["arrived"].as_array().is_some_and(|arrived| arrived.contains(&json!("landingNeeds"))) {
      break;
    }
    assert!(Instant::now() < arrived, "the computation never reached its gate");
    tokio::time::sleep(Duration::from_millis(20)).await;
  }

  delete_repository(&client, addr, &id, &path).await;
  let needs = computed.await.unwrap();
  assert!(needs.is_ok(), "the computation ran while its repository was there: {needs:?}");
}

/// The deletion at the end of a test removes the files only once the sessions of the repository ended: a sync that a
/// Fetch asks and lets go (`fetch_run`), as an update that applied nothing does, runs git in the worktree held by no
/// command, and a git whose directory went pays a removed working directory (on macOS every `getcwd` reads the entries
/// of the parent: seconds beside a large `$TMPDIR`). Here every git of the worktree is slow and says as it ends whether
/// its directory is still there
#[tokio::test]
async fn deleting_a_repository_leaves_no_git_running_in_its_directory() {
  let wrapped = slow_git("0.2");
  let (addr, state) = start_test_server_with_git(wrapped.executor().clone()).await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  start_a_slow_sync(&state, &wrapped, &path).await;

  delete_repository(&client, addr, &id, &path).await;
  let log = std::fs::read_to_string(wrapped.directory().join("log")).unwrap_or_default();
  let count = |what: &str| log.lines().filter(|line| line.starts_with(what)).count();
  assert!(
    count("gone ") == 0 && count("start ") == count("end "),
    "a git ran on past the removal of its directory:\n{log}"
  );
}

/// A git whose every run in the worktree whose directory `{dir}/slow` names (once the test wrote it) takes `seconds`
/// more and logs its `start`, then its `end`, or `gone` when its directory went while it ran, into `{dir}/log`; a git
/// elsewhere runs as it is
fn slow_git(seconds: &str) -> test_utils::wrapped_git::WrappedGit {
  let script = concat!(
    "*) [ -f '{dir}/slow' ] || exec git \"$@\"; here=$(pwd -P); case \"$here\" in *\"/$(cat '{dir}/slow')\"*) ;; *) exec git \"$@\";; esac; ",
    "echo \"start $$\" >> '{dir}/log'; sleep SECONDS; git \"$@\"; code=$?; ",
    "if [ -d \"$here\" ]; then echo \"end $$\" >> '{dir}/log'; else echo \"gone $$ $here\" >> '{dir}/log'; fi; exit $code;;"
  );
  test_utils::wrapped_git::WrappedGit::new(&script.replace("SECONDS", seconds))
}

/// The session of the repository at `path` syncs with the slow git of `wrapped` (`slow_git`), which runs its first git
/// in the worktree when this answers; nothing waits for the sync
async fn start_a_slow_sync(state: &crate::state::AppState, wrapped: &test_utils::wrapped_git::WrappedGit, path: &str) {
  let name = std::path::Path::new(path).file_name().unwrap().to_string_lossy().into_owned();
  std::fs::write(wrapped.directory().join("slow"), name).unwrap();
  let key = SessionKey {
    worktree: path.to_owned(),
    prefix: "user-name".to_string(),
  };
  drop(state.sessions.sync(&key, domain_types::session::SyncReason::AfterRewrite));
  let deadline = Instant::now() + Duration::from_mins(1);
  while !std::fs::read_to_string(wrapped.directory().join("log")).unwrap_or_default().contains("start ") {
    assert!(Instant::now() < deadline, "the sync ran no git in the worktree");
    tokio::time::sleep(Duration::from_millis(20)).await;
  }
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
  post(&client, addr, &format!("/repositories/{id}/locks/repository"), json!({ "held": true })).await;
  assert!(std::path::Path::new(&path).join(".git/branch-deck.lock.holder").exists(), "the holder record of the lock");

  delete_repository(&client, addr, &id, &path).await;
  assert_eq!(String::from_utf8_lossy(&logged.lock().unwrap()), "");
}

/// A route test sends a command only through `test_support::command`, which names its repository as the page does
/// (`Named`): a request it builds itself would name none, and the deletion of the repository would not wait for it,
/// removing the files under the git of the command or of a fake it asked (on Windows that fails the removal)
#[test]
fn every_command_of_a_route_test_names_its_repository() {
  // the URL of a command and the header of the repository, in parts, so this scan does not find itself
  let by_hand = [concat!("}", "/invoke/"), concat!("REPOSITORY_", "ID_HEADER")];
  let sources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
  let mut built_by_hand = Vec::new();
  for entry in std::fs::read_dir(&sources).unwrap() {
    let path = entry.unwrap().path();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    if !name.ends_with("_test.rs") {
      continue;
    }
    let text = std::fs::read_to_string(&path).unwrap();
    for (number, line) in text.lines().enumerate() {
      if by_hand.iter().any(|pattern| line.contains(pattern)) {
        built_by_hand.push(format!("{name}:{}: {}", number + 1, line.trim()));
      }
    }
  }
  assert_eq!(built_by_hand, [] as [String; 0], "a command request built outside test_support::command");
}

/// The test the E2E fixture names as it makes the repository of `subscribed` (`TestRepositoryBuilder.init`)
const TEST: &str = "lifecycle/counts.spec.ts:7 › Counts › names what it leaves";

/// A repository from the `simple` template as the E2E fixture makes it, naming its test (`TEST`): its id and path
async fn create_named_repository(client: &reqwest::Client, addr: std::net::SocketAddr) -> (String, String) {
  let created: Value = client
    .post(format!("http://{addr}/repositories"))
    .json(&json!({ "template": "simple", "test": TEST }))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  (created["id"].as_str().unwrap().to_owned(), created["path"].as_str().unwrap().to_owned())
}

/// A server with a repository whose window subscribed to its session, which the subscription opened
struct Subscribed {
  addr: std::net::SocketAddr,
  state: std::sync::Arc<crate::state::AppState>,
  client: reqwest::Client,
  id: String,
  path: String,
  messages: crate::test_support::Messages,
}

async fn subscribed() -> Subscribed {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_named_repository(&client, addr).await;
  let mut messages = crate::session_routes_test::subscribe(&client, addr, &id, &path, 9).await;
  crate::session_routes_test::subscription(&mut messages).await;
  Subscribed {
    addr,
    state,
    client,
    id,
    path,
    messages,
  }
}

/// The deletion of a repository closes its sessions: the stream of a window still subscribed ends, after a patch the
/// session sent before (the sign-ins it reads at its open), so the deletion does not wait for it, and the session is gone
#[tokio::test]
async fn deleting_a_repository_closes_its_sessions() {
  let Subscribed {
    addr,
    state,
    client,
    id,
    path,
    mut messages,
  } = subscribed().await;
  let key = SessionKey {
    worktree: path.clone(),
    prefix: "user-name".to_string(),
  };
  assert!(state.sessions.is_open(&key));

  // the stream of a subscribed window never ends by itself: a deletion within a minute, which no run reaches, did not
  // wait for it
  let started = Instant::now();
  delete_repository(&client, addr, &id, &path).await;
  assert!(started.elapsed() < Duration::from_mins(1), "the deletion waited for the stream: {:?}", started.elapsed());
  // the patch of what the session read before it closed (the sign-ins, read at its open) may come first
  let mut after = Vec::new();
  while let Some(message) = messages.next_or_end().await {
    assert!(message.get("progress").is_some() && after.len() < 2, "the stream of the closed session ended: {message}");
    after.push(message);
  }
  assert!(!state.sessions.is_open(&key));
}

/// `/debug/counts` of the server, read until `done` holds, within a minute: what the server holds settles a moment after
/// the command or the deletion that changed it (the first look of a watch, the end of a subscription)
async fn counts_when(client: &reqwest::Client, addr: std::net::SocketAddr, done: impl Fn(&Value) -> bool) -> Value {
  let deadline = Instant::now() + Duration::from_mins(1);
  loop {
    let counts: Value = client.get(format!("http://{addr}/debug/counts")).send().await.unwrap().json().await.unwrap();
    if done(&counts) {
      return counts;
    }
    assert!(Instant::now() < deadline, "the counts did not settle: {counts}");
    tokio::time::sleep(Duration::from_millis(20)).await;
  }
}

/// `/debug/counts` tells what the server holds: a repository whose window subscribed is one repository, named by its
/// test, one session and one watch that looked, with no orphan; a session whose worktree no live test repository has is
/// an orphan, named by its worktree, the repository and the test of its path and what keeps it (its window); once the
/// repository is deleted the server holds nothing of it, not the app of its commands with the memory of its diffs
/// (`keptContexts`), and no session is left an orphan, while the totals of the watches since the start keep the looks
/// and the reads of the one that ended
#[tokio::test]
async fn the_debug_counts_tell_the_repositories_the_sessions_the_orphans_and_the_watches() {
  let Subscribed {
    addr,
    state,
    client,
    id,
    path,
    mut messages,
  } = subscribed().await;

  let counts = counts_when(&client, addr, |counts| counts["watches"]["looks"].as_u64() >= Some(1)).await;
  let total = &counts["watches"]["total"];
  assert!(
    total["reads"].as_u64() >= Some(1) && total["asked"].as_u64() >= Some(1),
    "the first look read the registry: {counts}"
  );
  assert_eq!(
    (
      &counts["repositories"],
      &counts["tests"],
      &counts["keptContexts"],
      &counts["sessions"],
      &counts["orphanSessions"],
      &counts["watches"]["repositories"],
      &counts["watches"]["sessions"]
    ),
    (
      &json!(1),
      &json!([{ "repository": id, "test": TEST }]),
      &json!([]),
      &json!(1),
      &json!([]),
      &json!(1),
      &json!(1)
    ),
    "{counts}"
  );

  // the server forgets the worktree of the session, as a deletion that left the session open would
  let (worktree, of) = state.path_to_id.remove(&path).unwrap();
  let counts = counts_when(&client, addr, |_| true).await;
  assert_eq!(
    counts["orphanSessions"],
    json!([{ "worktree": path, "prefix": "user-name", "repository": id, "test": TEST, "subscribers": 1, "operations": 0 }]),
    "{counts}"
  );
  state.path_to_id.insert(worktree, of);

  delete_repository(&client, addr, &id, &path).await;
  while messages.next_or_end().await.is_some() {}
  // the app of its commands goes with it, and the memory of its diffs and highlighting with the app
  let counts = counts_when(&client, addr, |counts| {
    counts["watches"]["repositories"] == json!(0) && counts["deleting"] == json!([]) && counts["keptContexts"] == json!([])
  })
  .await;
  assert_eq!(
    (
      &counts["repositories"],
      &counts["tests"],
      &counts["deleting"],
      &counts["keptContexts"],
      &counts["sessions"],
      &counts["orphanSessions"],
      &counts["watches"]["sessions"],
      &counts["watches"]["looks"]
    ),
    (&json!(0), &json!([]), &json!([]), &json!([]), &json!(0), &json!([]), &json!(0), &json!(0)),
    "{counts}"
  );
  let total = &counts["watches"]["total"];
  assert!(
    total["looks"].as_u64() >= Some(1) && total["reads"].as_u64() >= Some(1),
    "the totals keep the watch that ended: {counts}"
  );
}

/// A server with a repository that a command holds (`InFlight::hold`, as `hold_repository` holds it for an
/// `execute`), and the key of its session
struct Held {
  addr: std::net::SocketAddr,
  state: std::sync::Arc<crate::state::AppState>,
  client: reqwest::Client,
  id: String,
  path: String,
  key: SessionKey,
  command: crate::state::CommandHold,
}

async fn held_by_a_command() -> Held {
  let (addr, state) = start_test_server_with_state().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let key = SessionKey {
    worktree: path.clone(),
    prefix: "user-name".to_string(),
  };
  let command = state.repositories.get(&id).unwrap().in_flight.hold("/invoke/execute").unwrap();
  Held {
    addr,
    state,
    client,
    id,
    path,
    key,
    command,
  }
}

/// A command that runs past the deletion and asks the session of its repository again (the sync after a rewrite or an
/// update) opens a new one, which no window holds and no sweep removes, as no test moves its clock: the deletion closes
/// it once the command ended, so no session outlives its repository (`orphanSessions`)
#[tokio::test]
async fn a_session_a_command_opened_again_after_the_deletion_closes_once_the_command_ended() {
  let Held {
    addr,
    state,
    client,
    id,
    path,
    key,
    command,
  } = held_by_a_command().await;
  let deleted = client.delete(format!("http://{addr}/repositories/{id}")).send().await.unwrap();
  assert_eq!(deleted.status(), 202);

  // the end of the command asks the session for its sync, as `rewrite::finish` does, then the command ends
  drop(state.sessions.sync(&key, domain_types::session::SyncReason::AfterRewrite));
  assert!(state.sessions.is_open(&key), "the request opened the session again");
  drop(command);
  wait_until_removed(&path).await;
  let counts = counts_when(&client, addr, |counts| counts["sessions"] == json!(0)).await;
  assert_eq!(counts["orphanSessions"], json!([]), "{counts}");
}

/// A command whose operation waits on the clock of its session (the reply of the bot to a start comment, the merge of a
/// stack), which stands still until its test moves it, ends with the deletion of its repository: the time of the
/// repository runs out (`TestClock::auto_advance`), its wait runs to its deadline, and the deletion waits for nothing
/// more; before, it waited `DELETE_WAIT` and the command waited for ever, holding the app of the repository
#[tokio::test]
async fn a_command_that_waits_on_the_clock_of_its_session_ends_with_the_deletion() {
  waits_on_the_clock_through_the_deletion(false).await;
}

/// A command that takes the clock of its session only after the deletion answered (the start of a review, the start of
/// a Safe Push asked as the test ended) gets the clock that ran out, not a new one that stands still for ever
#[tokio::test]
async fn a_command_that_takes_the_clock_of_its_session_after_the_deletion_ends_with_it_too() {
  waits_on_the_clock_through_the_deletion(true).await;
}

/// A command waits on the clock of its session, taken before the deletion or once it answered (`after_delete`), with a
/// poll every 5 s until a minute passed, as `follow_start_comment` waits for the reply: it looks as often as its
/// deadline lets it and ends, and the deletion with it
async fn waits_on_the_clock_through_the_deletion(after_delete: bool) {
  let Held {
    addr,
    state,
    client,
    id,
    path,
    key,
    command,
  } = held_by_a_command().await;
  let (deleted, answered) = tokio::sync::oneshot::channel::<()>();
  let sessions = std::sync::Arc::clone(&state.sessions);
  let waits = tokio::spawn(async move {
    if after_delete {
      answered.await.unwrap();
    }
    let clock = sessions.clock(&key);
    let deadline = clock.now() + Duration::from_mins(1);
    let mut looks = 0;
    while clock.now() + Duration::from_secs(5) <= deadline {
      looks += 1;
      repository_model::runtime::sleep_until(&*clock, clock.now() + Duration::from_secs(5)).await;
    }
    drop(command);
    looks
  });
  // the task runs up to its first wait (the runtime of the test has one thread)
  tokio::task::yield_now().await;
  assert!(!waits.is_finished(), "the clock of the session stands still");

  let response = client.delete(format!("http://{addr}/repositories/{id}")).send().await.unwrap();
  assert_eq!(response.status(), 202);
  // a command that took the clock before may have ended already
  let _ = deleted.send(());
  wait_until_removed(&path).await;
  assert_eq!(waits.await.unwrap(), 12, "the wait looked as often as its deadline lets it");
  // the counts name the clock that ran out with the waits it served
  let counts = counts_when(&client, addr, |_| true).await;
  let busiest = &counts["runOutWaits"]["busiest"][0];
  assert!(
    busiest["repository"] == json!(id) && busiest["waits"].as_u64() >= Some(12) && counts["runOutWaits"]["total"].as_u64() >= Some(12),
    "{counts}"
  );
}

/// A deletion that waited longer than it may (`AppState::delete_wait`, here 300 ms) warns of what holds the repository:
/// the test that made it, the commands still running, the worktrees whose closed sessions still ran at the end of
/// either wait, and a session of its worktree open again since the deletion closed it, with what keeps it; then it
/// deletes the files and closes that session. Here a command of the page holds the repository, the sync of its closed
/// session runs a slow git, and a request of a sync opens the session again after the close
#[tokio::test]
async fn a_deletion_that_waited_too_long_warns_of_the_test_the_commands_and_the_sessions_that_hold_it() {
  let wrapped = slow_git("3");
  let (addr, state) = start_test_server_waiting(wrapped.executor().clone(), Duration::from_millis(300)).await;
  let logged = capture_warnings();
  let client = reqwest::Client::new();
  let (id, path) = create_named_repository(&client, addr).await;
  let in_flight = std::sync::Arc::clone(&state.repositories.get(&id).unwrap().in_flight);
  let command = in_flight.hold("/invoke/execute").unwrap();
  start_a_slow_sync(&state, &wrapped, &path).await;

  let response = client.delete(format!("http://{addr}/repositories/{id}")).send().await.unwrap();
  assert_eq!(response.status(), 202);
  let key = SessionKey {
    worktree: path.clone(),
    prefix: "user-name".to_string(),
  };
  drop(state.sessions.sync(&key, domain_types::session::SyncReason::AfterRewrite));
  let warning = || String::from_utf8_lossy(&logged.lock().unwrap()).into_owned();
  let deadline = Instant::now() + Duration::from_mins(1);
  while !warning().contains("still run after") {
    assert!(Instant::now() < deadline, "no warning of the deletion: {}", warning());
    tokio::time::sleep(Duration::from_millis(20)).await;
  }
  let warning = warning();
  for named in [
    "still run after 300ms; deleting it anyway".to_owned(),
    format!("test=\"{TEST}\""),
    "commands=[\"/invoke/execute\"]".to_owned(),
    format!("closing=[\"{path}\"]"),
    format!("reopened=[\"{path} under user-name (0 windows, "),
  ] {
    assert!(warning.contains(&named), "{named} in {warning}");
  }

  // the deletion goes on: the files go, the session opened again closes, the closed one ends with its git
  drop(command);
  wait_until_removed(&path).await;
  let counts = counts_when(&client, addr, |counts| counts["sessions"] == json!(0) && counts["deleting"] == json!([])).await;
  assert_eq!(counts["orphanSessions"], json!([]), "{counts}");
}
