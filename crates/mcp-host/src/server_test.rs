use super::{MirrorPatches, Resync, Slot, path_of_uri, tools};
use crate::hosts::Instances;
use crate::link::Link;
use crate::mirror::MirrorView;
use crate::notices::Notices;
use crate::notices::notices_test::{REPO, failed, robot, view_with};
use app_api::transport::EventSink;
use app_settings::service::SettingsService;
use domain_types::session::{RepositoryView, TableChanges, Version, ViewPatch};
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// A root is a `file://` URI: its path, decoded; a Windows root without the slash before its drive
#[test]
fn the_path_of_a_root_is_its_file_uri_decoded() {
  assert_eq!(path_of_uri("file:///Users/me/my%20repo").as_deref(), Some("/Users/me/my repo"));
  assert_eq!(path_of_uri("file:///C:/work/repo").as_deref(), Some("C:/work/repo"));
  assert_eq!(path_of_uri("https://example.com"), None);
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
