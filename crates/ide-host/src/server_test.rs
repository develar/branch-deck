use super::{Mirror, MirrorPatches, Resync, Slot, mirror_window, path_of_uri, progress_of, tools};
use crate::hosts::Instances;
use crate::link::Link;
use crate::mirror::MirrorView;
use crate::notices::Notices;
use crate::notices::notices_test::{REPO, failed, robot, view_with};
use crate::process::process_test::Brd;
use app_api::transport::EventSink;
use app_settings::service::SettingsService;
use domain_types::session::{Demand, RepositoryView, TableChanges, Version, ViewPatch};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

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

/// Every refusal of brd has the code the contract gives its reason: an unknown tool and an unknown method `-32601`, a
/// call of an agent `-32009`; a reason the contract gives no code of its own has the code of JSON-RPC, as the corpus of
/// the copy answers it (`unknownAction` and `badParams` `-32602`, `parse` `-32700`)
#[tokio::test]
async fn every_refusal_of_brd_has_the_code_the_contract_gives_its_reason() {
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
/// action of a notification of brd
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

/// The failure of a robot a patch of the session brings is a notification of the IDE, after the change of the mirror
/// that tells the page the news: while the main view is hidden, with the action that reveals the branch
#[test]
fn a_failed_robot_of_a_patch_is_a_notification_of_the_ide_after_the_change_of_the_mirror() {
  let mirror = Mirrored::new();
  let base = Version { epoch: 0, seq: 0 };
  mirror
    .patches
    .send(ViewPatch {
      base,
      next: base.next(),
      changes: failed_robot(),
    })
    .unwrap();
  let Mirrored { patches, mut out, .. } = mirror;
  let mut written = Vec::new();
  while let Ok(message) = out.try_recv() {
    written.push(serde_json::from_str::<Value>(&message).unwrap());
  }
  let methods: Vec<_> = written.iter().map(|message| message["method"].as_str().unwrap_or_default()).collect();
  assert_eq!(methods, ["x-air-app/viewChanged", "x-air-app/notify"]);
  assert_eq!(written[1]["params"]["title"], "Safe Push of auth failed");
  assert_eq!(
    written[1]["params"]["actions"][0]["run"],
    serde_json::json!({ "tool": "reveal_branch", "arguments": { "repositoryPath": REPO, "branchName": "auth" } })
  );
  drop(patches);
}

/// A patch into another epoch of the session (a slot of a lease pool leased again, which opened it anew) changes nothing
/// of the mirror and tells the IDE nothing, its news included: the server is asked to read the view of the session anew,
/// with the slot that got the patch, so a mirror that moved meanwhile moves no more
#[test]
fn a_patch_into_another_epoch_asks_the_server_to_read_the_view_anew() {
  let mut mirror = Mirrored::new();
  let held = mirror.view.clone();
  let base = Version { epoch: 0, seq: 0 };
  mirror
    .patches
    .send(ViewPatch {
      base,
      next: Version { epoch: 1, seq: 0 },
      changes: failed_robot(),
    })
    .unwrap();
  assert!(mirror.out.try_recv().is_err(), "no change of the mirror and no notification");
  let resync = mirror.resynced.try_recv().expect("the server reads the view anew");
  assert_eq!(resync.mirror, REPO);
  assert!(Arc::ptr_eq(&resync.slot, &mirror.patches.slot));
  {
    let Slot::Live(live) = &*mirror.patches.slot.lock().unwrap() else {
      panic!("the mirror shows its view until it moves");
    };
    assert_eq!(live.view(), &held);
  }
  drop(mirror);
}

/// The news of a patch: the robot `r1` of `auth` failed
fn failed_robot() -> TableChanges {
  TableChanges {
    news: vec![failed(1, "r1", Some(false))],
    ..TableChanges::default()
  }
}

/// The patches of the session to a mirror of `REPO` that shows a view with the robot `r1` of `auth`, while the main view
/// is hidden: what brd writes, and what the slot asks the server to read anew
struct Mirrored {
  patches: MirrorPatches,
  view: RepositoryView,
  out: tokio::sync::mpsc::UnboundedReceiver<String>,
  resynced: tokio::sync::mpsc::UnboundedReceiver<Resync>,
  _directory: tempfile::TempDir,
}

impl Mirrored {
  fn new() -> Self {
    let directory = tempfile::tempdir().unwrap();
    let settings = Arc::new(SettingsService::open(directory.path().to_path_buf()).unwrap());
    let (link, out) = Link::new();
    let link = Arc::new(link);
    let instances = Arc::new(Instances::default());
    instances.shown("main:1", serde_json::json!({}));
    instances.hidden("main:1");
    let view = view_with(&[robot("r1", Some("auth"), None)]);
    let (resyncs, resynced) = tokio::sync::mpsc::unbounded_channel();
    let patches = MirrorPatches {
      mirror: REPO.to_owned(),
      slot: Arc::new(Mutex::new(Slot::Live(Box::new(MirrorView::new(view.clone()))))),
      link: Arc::clone(&link),
      notices: Arc::new(Notices::new(Arc::clone(&link), instances, settings)),
      resyncs,
    };
    Self {
      patches,
      view,
      out,
      resynced,
      _directory: directory,
    }
  }
}

/// The views of a mirror share its subscription: the main view and a review each set a demand, and the session gets
/// the union of them, so the demand of the main view never takes back what the review asked; the view that leaves
/// takes only its own demand with it
#[test]
fn the_session_of_a_mirror_gets_the_union_of_the_demands_of_its_views() {
  let mut mirror = Mirror {
    window: mirror_window(REPO),
    subscription: 1,
    instances: HashSet::from(["main:1".to_owned(), "review:1".to_owned()]),
    slot: Arc::new(Mutex::new(Slot::Ended)),
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

  assert_eq!(mirror.leave("review:1"), Some(main.clone()), "the session gets the demand of the main view alone");
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
