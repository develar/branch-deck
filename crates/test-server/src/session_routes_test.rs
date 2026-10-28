//! The session commands of app-api on the server as the page mock calls them: a subscription is an event stream that
//! answers first and then carries the patches, each on the version before it, up to the version a sync answers; it ends
//! with `unsubscribe_view`; a repository without a prefix is refused by the frame with the `ApiError` of the app and the
//! id of its operation, on the stream too (the route's side of the contract of `api_error_contract_test.rs`); the span
//! of the frame records the fields of the entry

use crate::invoke::WINDOW_LABEL_HEADER;
use crate::test_support::{Messages, Named, command, create_repository, create_repository_from, post, post_as, post_error, push_alone, rewrite, start_test_server, view_when};
use app_api::api_error::no_branch_prefix;
use domain_types::error::ApiError;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;

/// `subscribe_view` of the repository at `path` on the channel `channel`, for the repository `id`, as the page mock asks
pub(crate) async fn subscribe(client: &reqwest::Client, addr: SocketAddr, id: &str, path: &str, channel: u32) -> Messages {
  let request = command(client, addr, Named::Id(id), "subscribe_view", &json!({ "repositoryPath": path }));
  Messages::of_channel(request, channel).await
}

/// The answer of a subscription, which comes before its first patch: its `data`
pub(crate) async fn subscription(messages: &mut Messages) -> Value {
  let first = messages.next().await;
  assert_eq!(first["result"]["status"], "ok", "{first}");
  first["result"]["data"].clone()
}

#[tokio::test]
async fn a_subscription_answers_its_view_then_the_patches_up_to_the_version_a_sync_answers() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  // no session is open before a window subscribes
  assert_eq!(post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await, Value::Null);

  let mut messages = subscribe(&client, addr, &id, &path, 7).await;
  let subscribed = subscription(&mut messages).await;
  assert_eq!(subscribed["view"]["version"]["seq"], 0, "the subscription opened the session: {subscribed}");

  let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(synced["outcome"], "synced", "{synced}");
  patches_up_to(&mut messages, &subscribed["view"]["version"], &synced["version"]).await;
  // the committed table asks for the facts: their patches follow, the landing needs last
  let version = listed(&client, addr, &path).await;
  patches_up_to(&mut messages, &synced["version"], &version).await;

  let snapshot = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!(snapshot["version"], version);
  assert_eq!(snapshot["sync"], json!({ "kind": "idle" }));
  assert!(!snapshot["order"].as_array().unwrap().is_empty(), "the branches of the sync: {snapshot}");
  // nothing a sync reads changed: a focus runs nothing
  let focused = post(&client, addr, "/invoke/set_window_focus", json!({ "repositoryPath": path, "focused": true })).await;
  assert_eq!(focused["outcome"], "unchanged", "{focused}");

  post_as(&client, addr, Named::Id(&id), "/invoke/unsubscribe_view", json!({ "id": subscribed["id"] })).await;
  assert_eq!(messages.next_or_end().await, None, "the stream ends with the subscription");
}

/// `error` with the id of the operation `answered` names, which must be one (a ULID): the JSON of the `ApiError` the
/// app rejects the same failure with
fn with_its_operation(mut error: ApiError, answered: &Value) -> Value {
  let id = answered["operation_id"].as_str().unwrap_or_else(|| panic!("the error names no operation: {answered}"));
  assert_eq!(id.len(), 26, "{answered}");
  error.operation_id = Some(id.to_owned());
  serde_json::to_value(error).unwrap()
}

/// The route of a session command answers the error of its frame as the app does, with the operation it failed in: the
/// frame resolves the key inside the operation of the command, on the stream of a channel too
#[tokio::test]
async fn a_repository_without_a_prefix_is_refused_with_the_api_error_of_the_app() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, "simple_no_prefix").await;

  let mut messages = subscribe(&client, addr, &id, &path, 8).await;
  let refused = messages.next().await;
  let error = with_its_operation(no_branch_prefix(), &refused["result"]["error"]);
  assert_eq!(refused, json!({ "result": { "status": "error", "error": error } }));
  assert_eq!(messages.next_or_end().await, None, "the stream ends with the answer");

  for name in ["get_snapshot", "sync", "set_window_focus"] {
    let params = json!({ "repositoryPath": path, "reason": "open", "focused": true });
    let response = command(&client, addr, Named::ByPath, name, &params).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK, "{name}: an error is an answer, not a failure of the server");
    let answer: Value = response.json().await.unwrap();
    let error = with_its_operation(no_branch_prefix(), &answer["error"]);
    assert_eq!(answer, json!({ "status": "error", "error": error }), "{name}");
  }
}

/// The next sync reports what a test set for it (`sync-overrides`): the remote status of a branch with the fields given
/// and the newest of its unpushed commits kept, the issue links of a YouTrack user; the one after it leaves out a status set to null, so the branch keeps the one it
/// had; the sync after that reports the server's own status again. Overrides set for every sync stay for each run, until
/// a test sets others
#[tokio::test]
async fn the_next_sync_takes_the_overrides_of_its_repository_and_the_syncs_after_it_do_not_unless_set_for_every_sync() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = subscribe(&client, addr, &id, &path, 9).await;
  subscription(&mut messages).await;
  let links = json!({ "links": [{ "issueRegexp": "\\b[A-Z]+-\\d+\\b", "linkRegexp": "https://youtrack.test/issue/$0" }] });
  let sync_with = |overrides: Value| {
    let (client, path, id) = (client.clone(), path.clone(), id.clone());
    async move {
      if !overrides.is_null() {
        post(&client, addr, &format!("/repositories/{id}/sync-overrides"), overrides).await;
      }
      let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
      assert_eq!(synced["outcome"], "synced", "{synced}");
      post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await
    }
  };
  let remote_of = |view: &Value| {
    let status = &view["branches"]["test-branch"]["remote"]["status"];
    (status["remoteExists"].clone(), status["remoteTip"].clone())
  };

  let pushed = json!({ "remote": { "test-branch": { "remoteExists": true, "remoteTip": "test-branch-pushed", "keepUnpushed": 1 } }, "issueLinks": links });
  let first = sync_with(pushed).await;
  assert_eq!(remote_of(&first), (json!(true), json!("test-branch-pushed")), "{first}");
  let unpushed = first["branches"]["test-branch"]["remote"]["status"]["unpushedCommits"]
    .as_array()
    .cloned()
    .unwrap_or_default();
  let copies = first["branches"]["test-branch"]["copies"].as_array().cloned().unwrap_or_default();
  assert_eq!(unpushed.len(), 1, "the newest of the two commits the template has: {first}");
  assert_eq!(unpushed[0], copies[0]["hash"], "the copy of the newest commit");
  assert_eq!(first["header"]["issueNavigationConfig"], links);

  let second = sync_with(json!({ "remote": { "test-branch": null } })).await;
  assert_eq!(remote_of(&second), (json!(true), json!("test-branch-pushed")), "no status reported: the one before stays");

  let third = sync_with(Value::Null).await;
  assert_eq!(remote_of(&third), (json!(false), Value::Null), "the template has no remote: {third}");

  let every = json!({ "remote": { "test-branch": { "remoteExists": true, "remoteTip": "test-branch-pushed" } }, "every": true });
  let fourth = sync_with(every).await;
  let fifth = sync_with(Value::Null).await;
  assert_eq!(remote_of(&fourth), (json!(true), json!("test-branch-pushed")), "{fourth}");
  assert_eq!(remote_of(&fifth), remote_of(&fourth), "the run after it takes them too: {fifth}");
  let replaced = sync_with(json!({})).await;
  assert_eq!(remote_of(&replaced), (json!(false), Value::Null), "others set: the server's own status again");
}

/// A ref of the repository answers the commit it is at; an unknown one is not found
#[tokio::test]
async fn a_revision_of_the_repository_answers_its_commit() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, _) = create_repository(&client, addr).await;
  let revision = |name: &str| client.get(format!("http://{addr}/repositories/{id}/revision?name={name}")).send();
  let head: Value = revision("HEAD").await.unwrap().json().await.unwrap();
  assert_eq!(head["id"].as_str().map(str::len), Some(40), "{head}");
  assert_eq!(revision("no-such-branch").await.unwrap().status(), 404);
}

/// An interrupted rebase defers the syncs of the session, its table as it was; once the rebase is gone it syncs
#[tokio::test]
async fn a_rebase_in_progress_defers_the_sync_until_it_is_gone() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = subscribe(&client, addr, &id, &path, 10).await;
  subscription(&mut messages).await;
  let rebase = format!("/repositories/{id}/rebase-in-progress");

  post(&client, addr, &rebase, json!({ "inProgress": true })).await;
  let deferred = post_error(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(deferred["code"], "operation_in_progress", "{deferred}");
  post(&client, addr, &rebase, json!({ "inProgress": false })).await;
  let synced = post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(synced["outcome"], "synced", "{synced}");
}

/// A prefix set in a terminal (`branch-prefix`): the run of the session under the old prefix builds nothing and answers
/// `prefix_changed`, as the app's; the commands after it resolve the prefix the run read, whose session syncs
#[tokio::test]
async fn a_prefix_set_in_a_terminal_refuses_the_run_of_the_session_under_the_old_one() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let sync = || post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" }));
  let first = sync().await;
  assert_eq!(first["outcome"], "synced", "{first}");
  let revision = |name: &str| client.get(format!("http://{addr}/repositories/{id}/revision?name={name}")).send();

  post(&client, addr, &format!("/repositories/{id}/branch-prefix"), json!({ "prefix": "team" })).await;
  let refused = post_error(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  assert_eq!(refused["code"], "prefix_changed", "{refused}");
  assert_eq!(revision("team/virtual/test-branch").await.unwrap().status(), 404, "the refused run built nothing");

  let synced = sync().await;
  assert_eq!(synced["outcome"], "synced", "{synced}");
  assert_ne!(synced["version"]["epoch"], first["version"]["epoch"], "another session");
  assert_eq!(revision("team/virtual/test-branch").await.unwrap().status(), 200);
}

/// The patches on `messages` up to `version`, each on the one before, from `from`
async fn patches_up_to(messages: &mut Messages, from: &Value, version: &Value) -> Vec<Value> {
  let mut at = from.clone();
  let mut patches = Vec::new();
  while at != *version {
    let patch = messages.next().await["progress"].clone();
    assert_eq!(patch["base"], at, "each patch follows the one before");
    at = patch["next"].clone();
    patches.push(patch);
  }
  patches
}

/// A window on the session of a repository of `template`, synced once, with what the sync asked answered (`listed`): the
/// client, the path, the window's stream and the version of its view
async fn synced_window(template: &str, channel: u32) -> (reqwest::Client, SocketAddr, String, Messages, Value) {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, template).await;
  let mut messages = subscribe(&client, addr, &id, &path, channel).await;
  let subscribed = subscription(&mut messages).await;
  post(&client, addr, "/invoke/sync", json!({ "repositoryPath": path, "reason": "manual" })).await;
  // the first committed table asks for the facts: their patches follow the sync's, the landing needs last
  let listed = listed(&client, addr, &path).await;
  patches_up_to(&mut messages, &subscribed["view"]["version"], &listed).await;
  (client, addr, path, messages, listed)
}

/// The version of the view once what the first sync of the session at `path` asks is answered: the robots, the pull
/// requests and the review requests (the lists after it), the view of what the robots hold derived of them, and the
/// landing needs of the table, which are computed after that view and come in a patch of their own. Until a test asks
/// for something again, no patch follows this version
async fn listed(client: &reqwest::Client, addr: SocketAddr, path: &str) -> Value {
  let settled = |view: &Value| {
    let answered = |state: &Value| state["kind"] != "loading";
    let facts = &view["facts"];
    let lists = [&facts["robots"], &facts["pullRequests"], &facts["reviewRequests"]];
    lists.into_iter().all(|part| answered(&part["head"]["state"])) && view["patronus"]["state"]["kind"] == "fresh" && answered(&view["patronus"]["landingNeeds"])
  };
  view_when(client, addr, path, settled).await["version"].clone()
}

/// A push reaches the session as a patch of the remote status it left, whose version its answer names; no sync runs
/// for it
#[tokio::test]
async fn a_push_answers_the_version_of_its_patch_and_runs_no_sync() {
  let (client, addr, path, mut messages, synced) = synced_window("simple", 11).await;
  let pushed = push_alone(&client, addr, &path, "test-branch", None).await;
  assert_eq!(pushed["effects"][0]["status"]["kind"], "done", "{pushed}");
  patches_up_to(&mut messages, &synced, &pushed["versionAfter"]).await;
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  let status = &view["branches"]["test-branch"]["remote"]["status"];
  assert_eq!((status["remoteExists"].clone(), status["unpushedCommits"].clone()), (json!(true), json!([])), "{view}");
  // what follows is the view of the robots on the remote branch the push moved, no sync
  let derived = listed(&client, addr, &path).await;
  let after = patches_up_to(&mut messages, &pushed["versionAfter"], &derived).await;
  assert!(after.iter().all(|patch| patch["changes"].get("sync").is_none()), "no sync ran: {after:?}");
  assert_eq!(view["sync"], json!({ "kind": "idle" }), "no sync ran: {view}");
}

/// The deletion of an archived branch (the rewrite `deleteArchived` of the engine) reaches the session as a patch of the
/// archived section, whose version its outcome names (`versionAfter`); no sync runs for it
#[tokio::test]
async fn a_deletion_of_an_archived_branch_answers_the_version_of_its_patch_and_runs_no_sync() {
  let (client, addr, path, mut messages, synced) = synced_window("archived_branches", 12).await;
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  let archived = view["sections"]["archived"]["names"][0].clone();
  assert!(archived.is_string(), "{view}");

  let outcome = rewrite(&client, addr, &path, json!({ "rewrite": "deleteArchived", "branchName": archived })).await;
  assert_eq!(outcome["result"], json!({ "kind": "rewrite", "rewrite": "deleted" }), "{outcome}");
  let deleted = outcome["versionAfter"].clone();
  let patches = patches_up_to(&mut messages, &synced, &deleted).await;
  // the operation shows in the part `operations` while it runs; one patch removes the row
  let removals: Vec<&Value> = patches.iter().filter(|patch| patch["changes"].get("archived").is_some()).collect();
  assert_eq!(removals.len(), 1, "{patches:?}");
  let names = removals[0]["changes"]["archived"]["names"].as_array().unwrap();
  assert!(!names.contains(&archived), "{patches:?}");
  assert!(patches.iter().all(|patch| patch["changes"].get("sync").is_none()), "no sync ran: {patches:?}");
  // the end of the operation follows as a patch of the part `operations`
  let view = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert_eq!(view["sync"], json!({ "kind": "idle" }), "no sync ran: {view}");
  assert!(!view["sections"]["archived"]["names"].as_array().unwrap().contains(&archived), "{view}");
}

/// The clock of the sessions of a repository moves only by its test (`POST /repositories/{id}/clock/advance`): a session
/// no window shows any more goes once it was idle for two minutes on it, not before; another repository's clock moves
/// none of its sessions. A window tells its session what it shows by its subscription
/// (`set_demand`)
#[tokio::test]
async fn the_clock_of_the_sessions_moves_only_by_a_test_and_sweeps_the_idle_ones() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository(&client, addr).await;
  let mut messages = subscribe(&client, addr, &id, &path, 13).await;
  let subscribed = subscription(&mut messages).await;
  let demand = json!({
    "focused": true,
    "online": true,
    "visibleBranches": ["test-branch"],
    "visibleStacks": [],
    "needed": [],
    "robotDetails": [],
    "watchedRobots": [],
  });
  let set = post_as(
    &client,
    addr,
    Named::Id(&id),
    "/invoke/set_demand",
    json!({ "subscription": subscribed["id"], "demand": demand }),
  )
  .await;
  assert_eq!(set, Value::Null);
  post_as(&client, addr, Named::Id(&id), "/invoke/unsubscribe_view", json!({ "id": subscribed["id"] })).await;

  let (other, _) = create_repository(&client, addr).await;
  post(&client, addr, &format!("/repositories/{other}/clock/advance"), json!({ "ms": 600_000 })).await;
  post(&client, addr, &format!("/repositories/{id}/clock/advance"), json!({ "ms": 119_000 })).await;
  let snapshot = post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await;
  assert!(snapshot.is_object(), "idle for 119 s of 120: {snapshot}");
  post(&client, addr, &format!("/repositories/{id}/clock/advance"), json!({ "ms": 1_000 })).await;
  assert_eq!(post(&client, addr, "/invoke/get_snapshot", json!({ "repositoryPath": path })).await, Value::Null);
}

/// A span the process opened: its name and its fields, `name=value` in their order
type OpenedSpan = (String, Vec<String>);

/// The spans the process opened
#[derive(Clone, Default)]
struct OpenedSpans(std::sync::Arc<std::sync::Mutex<Vec<OpenedSpan>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for OpenedSpans {
  fn on_new_span(&self, attributes: &tracing::span::Attributes<'_>, _id: &tracing::span::Id, _context: tracing_subscriber::layer::Context<'_, S>) {
    let mut fields = SpanFields::default();
    attributes.record(&mut fields);
    let name = attributes.metadata().name().to_owned();
    self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((name, fields.0));
  }
}

impl OpenedSpans {
  /// The fields of the spans named `name`, in the order they opened
  fn named(&self, name: &str) -> Vec<Vec<String>> {
    let spans = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    spans.iter().filter(|(span, _)| span == name).map(|(_, fields)| fields.clone()).collect()
  }
}

/// The fields a span recorded, `name=value`
#[derive(Default)]
struct SpanFields(Vec<String>);

impl tracing::field::Visit for SpanFields {
  fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
    self.0.push(format!("{}={value:?}", field.name()));
  }
}

/// The frame of a session command opens its span with the fields of its entry beside the repository (its body opens no
/// span of its own), so a line of the log says why a sync ran, which fact a refresh read, whether a window gained the
/// focus, which window subscribed (`session(window)`, the label of the call), which target the statistics read: here
/// before the key, which a repository without a prefix refuses
#[tokio::test]
async fn the_span_of_a_session_command_records_the_fields_its_entry_names() {
  use tracing_subscriber::layer::SubscriberExt;
  use tracing_subscriber::util::SubscriberInitExt;

  // the global subscriber, which sees the tasks of the server too: sound because nextest runs each test in a process
  let spans = OpenedSpans::default();
  tracing_subscriber::registry().with(spans.clone()).init();
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, "simple_no_prefix").await;

  let calls = [
    ("sync", json!({ "repositoryPath": path, "reason": "focus" })),
    (
      "refresh_facts",
      json!({ "repositoryPath": path, "family": "readiness", "reason": "manual", "subject": "auth" }),
    ),
    ("set_window_focus", json!({ "repositoryPath": path, "focused": true })),
  ];
  for (command, params) in calls {
    let error = post_error(&client, addr, &format!("/invoke/{command}"), params).await;
    assert_eq!(error["code"], "no_branch_prefix", "{command}: {error}");
  }
  // the commands with a channel, refused on its stream, from a window that is not the main one
  let streamed = [
    ("subscribe_view", json!({ "repositoryPath": path })),
    (
      "get_robot_stats",
      json!({ "repository_path": path, "baseline_branch": "master", "period": "14d", "refresh": true }),
    ),
  ];
  for (channel, (name, params)) in (1..).zip(streamed) {
    let request = command(&client, addr, Named::Id(&id), name, &params).header(WINDOW_LABEL_HEADER, "review-feature");
    let refused = Messages::of_channel(request, channel).await.next().await;
    assert_eq!(refused["result"]["error"]["code"], "no_branch_prefix", "{name}: {refused}");
  }
  let repository = format!("repository_path={path}");
  assert_eq!(spans.named("subscribe_view"), [vec![repository.clone(), "window=review-feature".to_owned()]]);
  assert_eq!(
    spans.named("get_robot_stats"),
    [vec![
      repository.clone(),
      "baseline_branch=master".to_owned(),
      "period=Days14".to_owned(),
      "refresh=true".to_owned()
    ]]
  );
  assert_eq!(spans.named("sync"), [vec![repository.clone(), "reason=Focus".to_owned()]]);
  assert_eq!(
    spans.named("refresh_facts"),
    [vec![
      repository.clone(),
      "family=Readiness".to_owned(),
      "reason=Manual".to_owned(),
      r#"subject=Some("auth")"#.to_owned()
    ]]
  );
  assert_eq!(spans.named("set_window_focus"), [vec![repository, "focused=true".to_owned()]]);
}
