use super::{Mirror, Server, View, mirror_window, path_of_uri, progress_of, tools};
use crate::backend::BackendLink;
use crate::hosts::Instances;
use crate::link::Link;
use crate::mirror::MirrorView;
use crate::notices::notices_test::{REPO, failed, robot, view_with, written};
use crate::process::process_test::Brd;
use domain_types::session::{Demand, RepositoryView, TableChanges, Version, ViewPatch};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

/// A root is a `file://` URI: its path, decoded; a Windows root without the slash before its drive
#[test]
fn the_path_of_a_root_is_its_file_uri_decoded() {
  assert_eq!(path_of_uri("file:///Users/me/my%20repo").as_deref(), Some("/Users/me/my repo"));
  assert_eq!(path_of_uri("file:///C:/work/repo").as_deref(), Some("C:/work/repo"));
  assert_eq!(path_of_uri("https://example.com"), None);
}

/// The URI of an existing directory ends with a separator (`Path.toUri` of the IDE): the path of the root has none, as
/// the desktop app spells the repository whose session it keys, but the root of a file system keeps its own
#[test]
fn the_path_of_a_root_has_no_trailing_separator() {
  assert_eq!(path_of_uri("file:///x/repo/").as_deref(), Some("/x/repo"));
  assert_eq!(path_of_uri("file:///C:/work/repo/").as_deref(), Some("C:/work/repo"));
  assert_eq!(path_of_uri("file:///").as_deref(), Some("/"));
  assert_eq!(path_of_uri("file:///C:/").as_deref(), Some("C:/"));
}

/// The code of each reason of x-air-app that the contract gives one (`ErrorObject` of `x-air-app.schema.json`, the copy
/// of the contract): "-32601 unknownMethod and unknownTool, …"; the other codes "are those of JSON-RPC and of air-ide"
fn codes_of_the_contract() -> Vec<(String, i64)> {
  let schema: Value = serde_json::from_str(include_str!("../../../docs/design/ide-embedding/protocol/x-air-app.schema.json")).unwrap();
  let description = schema["$defs"]["ErrorObject"]["description"].as_str().unwrap();
  let listed = description.split_once(". ").unwrap().1.split(';').next().unwrap();
  let mut codes = Vec::new();
  let mut code = None;
  for word in listed.split_whitespace().map(|word| word.trim_end_matches(',')) {
    match word.parse::<i64>() {
      Ok(number) => code = Some(number),
      Err(_) if word == "and" => {}
      Err(_) => codes.push((word.to_owned(), code.unwrap())),
    }
  }
  codes
}

/// Every refusal of the bridge has the code the contract gives its reason: an unknown tool and an unknown method `-32601`, a
/// call of an agent `-32009`; a reason the contract gives no code of its own has the code of JSON-RPC, as the corpus of
/// the copy answers it (`unknownAction` and `badParams` `-32602`, `parse` `-32700`)
#[tokio::test]
async fn every_refusal_of_the_bridge_has_the_code_the_contract_gives_its_reason() {
  let app_data = tempfile::tempdir().unwrap();
  let mut brd = Brd::start(app_data.path());
  let click = json!({ "x-air-app": { "origin": "click", "instance": "main:1" } });
  let refused = [
    (
      "unknownTool",
      json!({ "method": "tools/call", "params": { "name": "nothing", "arguments": {}, "_meta": click } }),
    ),
    ("unknownMethod", json!({ "method": "x-air-app/nothing", "params": {} })),
    (
      "agentOrigin",
      json!({ "method": "tools/call", "params": { "name": "bd_invoke", "arguments": { "command": "get_settings_changes", "args": { "since": 0 } }, "_meta": { "x-air-app": { "origin": "agent" } } } }),
    ),
    (
      "unknownAction",
      json!({ "method": "x-air-app/dispatchAction", "params": { "instance": "main:1", "action": { "type": "branch-deck/nothing" } } }),
    ),
    ("badParams", json!({ "method": "resources/read", "params": { "uri": "ui://branch-deck/nothing.js" } })),
    ("badParams", json!({ "method": "x-air-app/subscribe", "params": { "instance": "main:1", "params": {} } })),
  ];
  let mut answered = Vec::new();
  for (id, (reason, request)) in refused.iter().enumerate() {
    let mut request = request.clone();
    request["jsonrpc"] = json!("2.0");
    request["id"] = json!(id);
    let answer = brd.ask(&request.to_string()).await;
    assert_eq!((&answer["id"], &answer["error"]["data"]["reason"]), (&json!(id), &json!(reason)), "{answer}");
    answered.push(((*reason).to_owned(), answer["error"]["code"].as_i64().unwrap()));
  }
  let answer = brd.ask("not JSON").await;
  answered.push((answer["error"]["data"]["reason"].as_str().unwrap().to_owned(), answer["error"]["code"].as_i64().unwrap()));

  let contract = codes_of_the_contract();
  assert!(contract.contains(&("unknownTool".to_owned(), -32601)), "{contract:?}");
  let json_rpc = [("unknownAction", -32602), ("badParams", -32602), ("parse", -32700)];
  for (reason, code) in &answered {
    let expected = contract
      .iter()
      .find(|(known, _)| known == reason)
      .map(|(_, code)| *code)
      .or_else(|| json_rpc.iter().find(|(known, _)| known == reason).map(|(_, code)| *code));
    assert_eq!(expected, Some(*code), "the code of {reason}");
  }
  brd.end().await;
}

/// The app file the installer writes (the example of the contract)
const APP_FILE: &str = include_str!("../../../docs/design/ide-embedding/protocol/examples/branch-deck.yaml");

/// Every tool is only for the app: the two of the page, the tool each action of the app file runs, and the tool of the
/// action of a notification of the bridge
#[test]
fn every_tool_is_only_for_the_app_and_each_action_of_the_app_file_has_its_tool() {
  let tools = tools();
  let names: Vec<_> = tools.as_array().unwrap().iter().map(|tool| tool["name"].as_str().unwrap()).collect();
  assert_eq!(names, ["bd_invoke", "bd_bootstrap", "sync", "open_command_palette", "reveal_branch"]);
  assert!(tools.as_array().unwrap().iter().all(|tool| tool["_meta"]["ui"]["visibility"] == serde_json::json!(["app"])));
  let run: Vec<_> = APP_FILE.lines().filter_map(|line| line.trim().strip_prefix("run: { tool: ")?.strip_suffix(" }")).collect();
  assert_eq!(run, ["sync", "open_command_palette"], "the actions of the app file that run a tool");
  assert!(run.iter().all(|tool| names.contains(tool)));
}

/// A server of the bridge whose backend is the test: the lines it sends the backend and the IDE, and the frames of the
/// backend it is handed (`Server::from_backend`)
struct Bridged {
  server: Arc<Server>,
  backend: Arc<BackendLink>,
  to_backend: UnboundedReceiver<String>,
  to_ide: UnboundedReceiver<String>,
}

impl Bridged {
  /// The bridge with the main view of the IDE hidden
  fn new() -> Self {
    let (link, to_ide) = Link::new();
    let (backend, to_backend) = BackendLink::new();
    let instances = Arc::new(Instances::default());
    instances.shown("main:1", json!({}));
    instances.hidden("main:1");
    let server = Server::new(Arc::clone(&backend), Arc::new(link), instances, Arc::default());
    Self {
      server,
      backend,
      to_backend,
      to_ide,
    }
  }

  /// A mirror of `REPO` that shows `view`, its window subscribed to its channel
  fn with_mirror(self, view: RepositoryView) -> Self {
    super::locked(&self.server.mirrors).insert(
      REPO.to_owned(),
      Mirror {
        window: mirror_window(REPO),
        channel: "x-branch-deck:/repository/1".to_owned(),
        instances: HashSet::from(["main:1".to_owned()]),
        view: View::Live(Box::new(MirrorView::new(view))),
        demands: HashMap::new(),
      },
    );
    self
  }

  /// The backend sends `frame`
  fn frame(&self, frame: &Value) {
    self.server.from_backend(frame);
  }

  /// What the bridge wrote to the IDE since the last look
  fn ide(&mut self) -> Vec<Value> {
    written(&mut self.to_ide)
  }

  /// What the bridge sent the backend since the last look
  fn backend(&mut self) -> Vec<Value> {
    written(&mut self.to_backend)
  }

  /// The backend answers the request `request` with `result`
  fn answer(&self, request: &Value, result: Value) {
    self.backend.answered(&request["id"], Ok(result));
  }
}

/// The action of the channel of the mirror of `REPO` with `patch`
fn patched(patch: &ViewPatch) -> Value {
  json!({ "jsonrpc": "2.0", "method": "action", "params": {
    "channel": "x-branch-deck:/repository/1", "action": { "type": "x-branch-deck/viewPatched", "patch": patch },
    "window": mirror_window(REPO), "serverSeq": 7,
  } })
}

/// A bridge whose mirror of `REPO` got the patch of the failure of the robot `r1` of `auth`
fn robot_failed() -> Bridged {
  let bridged = Bridged::new().with_mirror(view_with(&[robot("r1", Some("auth"), None)]));
  let base = Version { epoch: 0, seq: 0 };
  bridged.frame(&patched(&ViewPatch {
    base,
    next: base.next(),
    changes: failed_robot(),
  }));
  bridged
}

/// The first notification of the IDE among `told`: a failure of `auth` whose action reveals its branch
fn assert_failure_told(told: &[Value]) {
  assert_eq!(told[0]["method"], "x-air-app/notify", "{told:#?}");
  assert_eq!(told[0]["params"]["title"], "Safe Push of auth failed");
  assert_eq!(
    told[0]["params"]["actions"][0]["run"],
    json!({ "tool": "reveal_branch", "arguments": { "repositoryPath": REPO, "branchName": "auth" } })
  );
}

/// The failure of a robot a patch of the session brings is a notification of the IDE, after the change of the mirror
/// that tells the page the news: while the main view is hidden, with the action that reveals the branch, once the
/// backend answered that the claim of the news is its first
#[test]
fn a_failed_robot_of_a_patch_is_a_notification_of_the_ide_after_the_change_of_the_mirror() {
  let mut bridged = robot_failed();
  let written = bridged.ide();
  let methods: Vec<_> = written.iter().map(|message| message["method"].as_str().unwrap_or_default()).collect();
  assert_eq!(methods, ["x-air-app/viewChanged"], "the notification waits for the claim of its news");
  let claims = bridged.backend();
  assert_eq!(claims.len(), 1, "{claims:#?}");
  assert_eq!(claims[0]["method"], "x-branch-deck/claimNews");
  assert_eq!(
    claims[0]["params"],
    json!({ "channel": "ahp-root://", "repositoryPath": REPO, "news": { "kind": "robotFailed", "robotId": "r1" } })
  );
  bridged.answer(&claims[0], json!({ "first": true }));
  assert_failure_told(&bridged.ide());
}

/// A news another client of the backend claimed first (another IDE on the repository, the desktop app) is told by that
/// one: the bridge tells it not again
#[test]
fn a_news_another_client_claimed_first_is_no_notification_of_the_bridge() {
  let mut bridged = robot_failed();
  bridged.ide();
  let claims = bridged.backend();
  bridged.answer(&claims[0], json!({ "first": false }));
  assert_eq!(bridged.ide(), Vec::<Value>::new());
}

/// A patch into another epoch of the session (a slot of a lease pool leased again, which opened it anew) changes nothing
/// of the mirror and tells the IDE nothing, its news included: the bridge reads the view of its window anew
/// (`get_snapshot`), and the mirror takes it in one change of every part, the version of the new epoch last
#[test]
fn a_patch_into_another_epoch_reads_the_view_anew() {
  let view = view_with(&[robot("r1", Some("auth"), None)]);
  let mut bridged = Bridged::new().with_mirror(view.clone());
  let base = Version { epoch: 0, seq: 0 };
  bridged.frame(&patched(&ViewPatch {
    base,
    next: Version { epoch: 1, seq: 0 },
    changes: failed_robot(),
  }));
  assert_eq!(bridged.ide(), Vec::<Value>::new(), "no change of the mirror and no notification");
  let asked = bridged.backend();
  assert_eq!(asked.len(), 1, "{asked:#?}");
  assert_eq!(asked[0]["method"], "x-branch-deck/get_snapshot");
  assert_eq!(asked[0]["params"]["window"], mirror_window(REPO));
  assert_eq!(asked[0]["params"]["args"], json!({ "params": { "repositoryPath": REPO } }));

  let mut anew = view;
  anew.version = Version { epoch: 1, seq: 2 };
  bridged.answer(&asked[0], json!({ "value": anew }));
  let changed = bridged.ide();
  assert_eq!(changed.len(), 1, "{changed:#?}");
  assert_eq!(changed[0]["method"], "x-air-app/viewChanged");
  let ops = changed[0]["params"]["ops"].as_array().unwrap();
  assert_eq!(ops.last().unwrap(), &json!({ "op": "part", "part": "version", "value": { "epoch": 1, "seq": 2 } }));
  // the patches of the new epoch apply to it
  bridged.frame(&patched(&ViewPatch {
    base: Version { epoch: 1, seq: 2 },
    next: Version { epoch: 1, seq: 3 },
    changes: TableChanges {
      order: Some(vec!["auth".to_owned()]),
      ..TableChanges::default()
    },
  }));
  let changed = bridged.ide();
  assert_eq!(changed[0]["params"]["ops"][0], json!({ "op": "part", "part": "order", "value": ["auth"] }));
}

/// The news of a patch: the robot `r1` of `auth` failed
fn failed_robot() -> TableChanges {
  TableChanges {
    news: vec![failed(1, "r1", Some(false))],
    ..TableChanges::default()
  }
}

/// The views of a mirror share its subscription: the main view and a review each set a demand, and the window of the
/// mirror gets the union of them, so the demand of the main view never takes back what the review asked; the view that
/// leaves takes only its own demand with it
#[test]
fn the_session_of_a_mirror_gets_the_union_of_the_demands_of_its_views() {
  let mut mirror = Mirror {
    window: mirror_window(REPO),
    channel: "x-branch-deck:/repository/1".to_owned(),
    instances: HashSet::from(["main:1".to_owned(), "review:1".to_owned()]),
    view: View::Ended,
    demands: HashMap::new(),
  };
  let main = Demand {
    focused: true,
    online: true,
    visible_branches: vec!["auth".to_owned()],
    ..Demand::default()
  };
  let review = Demand {
    online: true,
    reviews: true,
    ..Demand::default()
  };
  assert_eq!(mirror.set_demand("main:1", main.clone()), main);
  let both = mirror.set_demand("review:1", review);
  assert_eq!((both.reviews, both.focused, both.visible_branches.as_slice()), (true, true, ["auth".to_owned()].as_slice()));
  // the main view sends its demand again: what the review asked stays asked while the review shows
  assert!(mirror.set_demand("main:1", main.clone()).reviews);

  assert_eq!(mirror.leave("review:1"), Some(main.clone()), "the window gets the demand of the main view alone");
  assert_eq!(mirror.demand(), Some(main));
  // the last view leaves: the subscription ends with the mirror, no demand is sent
  assert_eq!(mirror.leave("main:1"), None);
}

/// The progress of the Dock is `setProgress` of the main view: a normal one its fraction with the text of what runs, an
/// indeterminate one with a text the text alone (a progress without a percent), the rest the clear. An IDE before a
/// progress without a fraction reads that request as the clear
#[test]
fn the_progress_of_the_dock_is_set_progress_of_the_main_view_with_the_text_of_what_runs() {
  assert_eq!(
    progress_of(&json!({ "status": "normal", "progress": 37, "text": "Safe Push" })),
    json!({ "view": "main", "fraction": 0.37, "text": "Safe Push" })
  );
  assert_eq!(progress_of(&json!({ "status": "normal", "progress": 40 })), json!({ "view": "main", "fraction": 0.4 }));
  assert_eq!(
    progress_of(&json!({ "status": "indeterminate", "text": "Dry Run" })),
    json!({ "view": "main", "text": "Dry Run" })
  );
  // an indeterminate one without a text, as the desktop page sends it, and none: the clear
  assert_eq!(
    progress_of(&json!({ "status": "indeterminate", "progress": 0 })),
    json!({ "view": "main", "fraction": null })
  );
  assert_eq!(progress_of(&json!({ "status": "none", "text": "Safe Push" })), json!({ "view": "main", "fraction": null }));
}
