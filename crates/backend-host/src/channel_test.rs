//! The repository channels and the windows of the connections on a host of the fixtures (`channel.rs`, `connection.rs`;
//! openspec/specs/repository/backend-protocol/spec.md, "Channel", "Windows")

use crate::corpus_test::{Fixture, Link};
use app_api::app::windows::OpenSubWindowParams;
use app_api::session::windows::PlanSlot;
use app_api::transport::WindowId;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn channel_of(id: u32, repo: &str) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "method": "x-branch-deck/repositoryChannel", "params": { "channel": "ahp-root://", "repositoryPath": repo } })
}

fn subscribe(id: u32, channel: &str, window: &str, have: Option<&Value>) -> Value {
  let mut meta = json!({ "window": window });
  if let Some(have) = have {
    meta["have"] = have.clone();
  }
  json!({ "jsonrpc": "2.0", "id": id, "method": "subscribe", "params": { "channel": channel, "_meta": { "x-branch-deck": meta } } })
}

fn sync(id: u32, repo: &str, window: &str) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "method": "x-branch-deck/sync", "params": { "channel": "ahp-root://", "window": window, "args": { "params": { "repositoryPath": repo, "reason": "manual" } } } })
}

fn set_demand(id: u32, channel: &str, window: &str) -> Value {
  let demand = json!({ "focused": true, "online": true, "visibleBranches": [], "visibleStacks": [], "needed": [], "robotDetails": [], "watchedRobots": [] });
  json!({ "jsonrpc": "2.0", "id": id, "method": "x-branch-deck/setDemand", "params": { "channel": channel, "window": window, "demand": demand } })
}

fn window_life(method: &str, window: &str) -> Value {
  json!({ "jsonrpc": "2.0", "method": method, "params": { "channel": "ahp-root://", "window": window } })
}

/// The URI of the channel of `repo` for `link`
async fn channel(link: &Link, repo: &str) -> String {
  let answer = link.ask(&channel_of(1, repo)).await;
  answer["result"]["channel"].as_str().unwrap().to_owned()
}

/// The `viewPatched` actions of `frames` to `window`, with their `serverSeq`
fn patches_of<'a>(frames: &'a [Value], window: &str) -> Vec<(&'a Value, &'a Value)> {
  frames
    .iter()
    .filter(|frame| frame["method"] == "action" && frame["params"]["window"] == window)
    .map(|frame| (&frame["params"]["serverSeq"], &frame["params"]["action"]["patch"]))
    .collect()
}

/// The frames `link` gets until it got a patch whose next version is `version`
async fn patched_to(link: &Link, window: &str, version: &Value) -> Vec<Value> {
  link
    .until(|frame| frame["method"] == "action" && frame["params"]["window"] == window && frame["params"]["action"]["patch"]["next"] == *version)
    .await
}

/// Two connections on one repository subscribe to one session and get the same actions with the same `serverSeq`; the
/// end of one leaves the other its subscription, which still hears the session
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_connections_on_one_repository_get_the_same_actions_and_the_end_of_one_leaves_the_other() {
  let fixture = Fixture::new().await;
  let _kept = fixture.settled().await;
  let (app, ide) = (fixture.shell().await, fixture.bridge().await);
  let uri = channel(&app, &fixture.repo).await;
  assert_eq!(channel(&ide, &fixture.repo).await, uri, "one channel for the path");
  let view = app.ask(&subscribe(2, &uri, "main", None)).await["result"]["snapshot"]["state"].clone();
  let mirror = format!("mirror:{}", fixture.repo);
  ide.ask(&subscribe(2, &uri, &mirror, Some(&view["version"]))).await;

  let frames = app.frames_of(&sync(3, &fixture.repo, "main")).await;
  let synced = frames.last().unwrap()["result"]["value"]["version"].clone();
  let of_app = patches_of(&frames, "main");
  let ide_frames = patched_to(&ide, &mirror, &synced).await;
  let of_ide = patches_of(&ide_frames, &mirror);
  // a patch of the session of its own (a fact read again) between the two subscribes reaches the app as an action and
  // the bridge as its catch-up: both get every patch from the first they share on
  let first = of_app
    .iter()
    .position(|patch| of_ide.contains(patch))
    .expect("a patch of the sync reaches both connections");
  let of_app = &of_app[first..];
  let of_ide = &of_ide[of_ide.iter().position(|patch| *patch == of_app[0]).unwrap()..];
  assert_eq!(of_app, of_ide, "the same patches with the same serverSeq to each connection");

  drop(ide);
  let frames = app.frames_of(&sync(4, &fixture.repo, "main")).await;
  assert!(frames.last().unwrap()["result"].get("value").is_some(), "{frames:#?}");
  let answer = app.ask(&set_demand(5, &uri, "main")).await;
  assert_eq!(answer["result"], json!({}), "the main window of the app keeps its subscription: {answer:#}");
}

/// The channel of a path is the same after its last window left, and a subscribe to it opens the session again
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_after_the_last_unsubscribe_is_the_same_channel_and_opens_the_session_again() {
  let fixture = Fixture::new().await;
  let link = fixture.bridge().await;
  let uri = channel(&link, &fixture.repo).await;
  link.ask(&subscribe(2, &uri, "main", None)).await;
  let left = link
    .ask(&json!({ "jsonrpc": "2.0", "id": 3, "method": "unsubscribe", "params": { "channel": uri, "_meta": { "x-branch-deck": { "window": "main" } } } }))
    .await;
  assert_eq!(left["result"], json!({}));
  fixture.ctx.sessions.close_worktree(&fixture.repo);
  assert_eq!(channel(&link, &fixture.repo).await, uri);
  let again = link.ask(&subscribe(4, &uri, "main", None)).await;
  assert!(again["result"]["snapshot"]["state"]["version"].is_object(), "{again:#}");
}

/// A window that holds a view of another epoch gets the whole view
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_seed_of_another_epoch_gets_the_whole_view() {
  let fixture = Fixture::new().await;
  let link = fixture.bridge().await;
  let uri = channel(&link, &fixture.repo).await;
  let first = link.ask(&subscribe(2, &uri, "main", None)).await;
  let version = &first["result"]["snapshot"]["state"]["version"];
  let other = json!({ "epoch": version["epoch"].as_u64().unwrap().wrapping_add(1) % u64::from(u32::MAX), "seq": 0 });
  let answer = link.ask(&subscribe(3, &uri, "review-1", Some(&other))).await;
  let snapshot = &answer["result"]["snapshot"];
  assert!(snapshot["state"]["version"].is_object(), "{answer:#}");
  assert!(snapshot.get("_meta").is_none(), "no held: {answer:#}");
}

/// The demand of a window is the demand of its subscription, and a closed window's subscription and demand end with it;
/// the session asks for the facts of the union of its subscriptions
/// (`the_demand_of_a_session_is_the_union_of_its_windows_and_goes_with_each` of repository-model)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_demand_of_a_window_ends_with_the_window() {
  let fixture = Fixture::new().await;
  let link = fixture.shell().await;
  let uri = channel(&link, &fixture.repo).await;
  link.send(&window_life("x-branch-deck/windowShown", "review-1"));
  link.ask(&subscribe(2, &uri, "main", None)).await;
  link.ask(&subscribe(3, &uri, "review-1", None)).await;
  assert_eq!(link.ask(&set_demand(4, &uri, "main")).await["result"], json!({}));
  assert_eq!(link.ask(&set_demand(5, &uri, "review-1")).await["result"], json!({}));

  link.send(&window_life("x-branch-deck/windowClosed", "review-1"));
  let closed = link.ask(&set_demand(6, &uri, "review-1")).await;
  assert_eq!(closed["error"]["data"]["reason"], "notSubscribed", "{closed:#}");
  assert_eq!(link.ask(&set_demand(7, &uri, "main")).await["result"], json!({}));
}

/// The main windows of two connections are two windows: the end of the IDE's connection forgets its windows alone, so
/// the main window of the app keeps its reads
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_end_of_a_connection_forgets_only_its_windows() {
  let fixture = Fixture::new().await;
  let (app, ide) = (fixture.shell().await, fixture.bridge().await);
  let (app_main, ide_main) = (WindowId::main_of(app.id()), WindowId::main_of(ide.id()));
  assert_ne!(app_main, ide_main);
  let uri = channel(&app, &fixture.repo).await;
  app.ask(&subscribe(2, &uri, "main", None)).await;
  ide.ask(&subscribe(2, &uri, "main", None)).await;
  let of_app = fixture.ctx.windows.begin_plan(&app_main, PlanSlot::Scope);
  let of_ide = fixture.ctx.windows.begin_plan(&ide_main, PlanSlot::Scope);

  drop(ide);
  assert!(of_ide.is_superseded(), "the read of the window of the IDE ends with its connection");
  assert!(!of_app.is_superseded(), "the read of the main window of the app goes on");
  assert_eq!(app.ask(&set_demand(3, &uri, "main")).await["result"], json!({}));
}

/// A review tab of the bridge closes: its read of a review plan is superseded, and the window is forgotten
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_review_tab_that_closes_is_forgotten() {
  let fixture = Fixture::new().await;
  let ide = fixture.bridge().await;
  ide.send(&window_life("x-branch-deck/windowShown", "review:3"));
  // a command of the window: its page is known from now on
  ide
    .ask(&json!({ "jsonrpc": "2.0", "id": 2, "method": "ping", "params": { "channel": "ahp-root://" } }))
    .await;
  let review = WindowId::of(ide.id(), "review:3");
  let plan = fixture.ctx.windows.begin_plan(&review, PlanSlot::Scope);
  ide.send(&window_life("x-branch-deck/windowClosed", "review:3"));
  ide
    .ask(&json!({ "jsonrpc": "2.0", "id": 3, "method": "ping", "params": { "channel": "ahp-root://" } }))
    .await;
  assert!(plan.is_superseded());
}

/// `open_sub_window` of the bridge is its `openWindow`: no window of the app opens, and the bridge's label of the window
/// is open for the host from then on
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_review_opened_in_the_ide_opens_no_window_of_the_app() {
  let fixture = Fixture::new().await;
  let ide = fixture.bridge().await;
  ide.send(&window_life("x-branch-deck/windowShown", "main"));
  let params = json!({ "windowId": "review-branch:feature", "url": "/review", "title": "feature", "width": null, "height": null, "data": "{}", "openedAt": 0 });
  ide
    .send(&json!({ "jsonrpc": "2.0", "id": 2, "method": "x-branch-deck/open_sub_window", "params": { "channel": "ahp-root://", "window": "main", "args": { "params": params } } }));
  let request = ide.next().await;
  assert_eq!(request["method"], "x-branch-deck/openWindow", "{request:#}");
  let echoed: OpenSubWindowParams = serde_json::from_value(request["params"]["params"].clone()).unwrap();
  assert_eq!(echoed.window_id, "review-branch:feature");
  ide.send(&json!({ "jsonrpc": "2.0", "id": request["id"], "result": { "window": "review:1" } }));
  let answer = ide.until(|frame| frame["id"] == 2 && frame.get("method").is_none()).await.pop().unwrap();
  assert_eq!(answer["result"], json!({ "value": null }));
  assert!(!fixture.ctx.app_desktop.windows.is_open("review-branch:feature"), "no window of the app");
  assert!(fixture.ctx.desktop(&WindowId::main_of(ide.id())).windows.is_open("review-branch:feature"));
}

fn follow_prefix(id: u32, channel: &str, window: &str) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "method": "x-branch-deck/followPrefix", "params": { "channel": channel, "window": window } })
}

/// `followPrefix` of a window takes the session of the prefix the git config names now: under the same prefix it answers
/// at once and moves nothing; a prefix set meanwhile (in a terminal) moves every window of the channel to the session of
/// the new prefix, each with one resync of another epoch, before the answer; a window that subscribes not is refused
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_follow_of_the_prefix_moves_every_window_of_the_channel_before_its_answer() {
  let fixture = Fixture::new().await;
  let (app, ide) = (fixture.shell().await, fixture.bridge().await);
  let uri = channel(&app, &fixture.repo).await;
  let view = app.ask(&subscribe(2, &uri, "main", None)).await["result"]["snapshot"]["state"].clone();
  let mirror = format!("mirror:{}", fixture.repo);
  channel(&ide, &fixture.repo).await;
  ide.ask(&subscribe(2, &uri, &mirror, Some(&view["version"]))).await;

  let frames = ide.frames_of(&follow_prefix(3, &uri, &mirror)).await;
  assert_eq!(frames.last().unwrap()["result"], json!({}), "{frames:#?}");
  assert!(
    frames.iter().all(|frame| frame["method"] != "x-branch-deck/resync"),
    "the same prefix moves nothing: {frames:#?}"
  );

  crate::corpus_test::git(&fixture.repo, &["config", "branchdeck.branchPrefix", "team"]);
  let frames = ide.frames_of(&follow_prefix(4, &uri, &mirror)).await;
  let resync = frames
    .iter()
    .find(|frame| frame["method"] == "x-branch-deck/resync" && frame["params"]["window"] == mirror)
    .unwrap_or_else(|| panic!("the window of the bridge moves before the answer: {frames:#?}"));
  assert_eq!(resync["params"]["snapshot"]["state"]["header"]["prefix"], "team");
  assert_ne!(resync["params"]["snapshot"]["state"]["version"]["epoch"], view["version"]["epoch"]);
  assert_eq!(frames.last().unwrap()["result"], json!({}));
  // the window of the app moves too
  let moved = app.until(|frame| frame["method"] == "x-branch-deck/resync").await.pop().unwrap();
  assert_eq!(
    (&moved["params"]["window"], &moved["params"]["snapshot"]["state"]["header"]["prefix"]),
    (&json!("main"), &json!("team"))
  );

  let refused = ide.ask(&follow_prefix(5, &uri, "review:9")).await;
  assert_eq!(refused["error"]["data"]["reason"], "notSubscribed", "{refused:#}");
}

/// A client that shows its windows itself (the bridge of the IDE: no `showWindow` among its host requests) gets no
/// `showWindow` when a window of it is due to show (a deadline of a window built hidden, a page that settled); a client
/// that declares it does
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_that_shows_its_windows_itself_gets_no_show_window() {
  let fixture = Fixture::new().await;
  for (host_requests, shown) in [(json!(["openWindow"]), false), (json!(["openWindow", "showWindow"]), true)] {
    let client = fixture.client(&json!({ "hostRequests": host_requests })).await;
    client.send(&window_life("x-branch-deck/windowShown", "review:1"));
    let desktop = fixture.ctx.desktop(&WindowId::of(client.id(), "review:1"));
    let show = desktop.windows.show("review:1", app_api::app::windows::Shown::Settled { open_id: 1 });
    if shown {
      let request = client.next().await;
      assert_eq!((&request["method"], &request["params"]["window"]), (&json!("x-branch-deck/showWindow"), &json!("review:1")));
      client.send(&json!({ "jsonrpc": "2.0", "id": request["id"], "result": {} }));
      show.await;
    } else {
      show.await;
      let ping = client
        .ask(&json!({ "jsonrpc": "2.0", "id": 9, "method": "ping", "params": { "channel": "ahp-root://" } }))
        .await;
      assert_eq!(ping["result"], json!({}));
    }
  }
}

/// The count of the clients is the bridges connected now that initialized, the app's own connection left out: what
/// keeps a backend without a window alive
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_count_of_the_clients_leaves_out_the_connection_of_the_app() {
  let fixture = Fixture::new().await;
  let clients = fixture.host.clients();
  let app = crate::in_process::InProcess::app(&fixture.host);
  app.send(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
    "channel": "ahp-root://", "protocolVersions": ["1.0.0"], "clientId": "app", "clientInfo": { "name": "branch-deck-app", "build": fixture.host.build() },
    "_meta": { "x-branch-deck": { "version": "1.0.0", "capabilities": {} } },
  } }));
  let answer: Value = serde_json::from_str(&app.next().await.unwrap().into_text()).unwrap();
  assert!(answer.get("result").is_some(), "{answer:#}");
  assert_eq!(*clients.borrow(), 0, "the app's own connection");
  let ide = fixture.bridge().await;
  assert_eq!(*clients.borrow(), 1);
  drop(ide);
  assert_eq!(*clients.borrow(), 0);
}

/// The desktop of the bridge of the IDE, which declares the host requests alone: no folder dialog (the roots of the
/// project name the repositories), no install of `brd` or of the app file, a menu and a restart that do nothing, no
/// updater (the app updates with the desktop app)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_desktop_of_the_bridge_has_no_folder_dialog_and_installs_nothing() {
  let fixture = Fixture::new().await;
  let ide = fixture.bridge().await;
  let desktop = fixture.ctx.desktop(&WindowId::main_of(ide.id()));
  assert!(!desktop.dialog.has_folder_dialog(), "the page offers no Open Repository… in the IDE");
  assert_eq!(desktop.dialog.pick_folder("Open"), None);
  assert!(!desktop.agent_cli.status().installable, "the page offers no install of brd or of the app file in the IDE");
  let refused = desktop.agent_cli.install().unwrap_err();
  assert_ne!(refused.message, "");
  desktop.menu.set_auto_sync_checked(true).unwrap();
  let refused = desktop.updater.check().await.unwrap_err();
  assert_ne!(refused.message, "", "no updater of the app runs for the IDE");
}
