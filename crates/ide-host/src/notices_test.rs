use crate::backend::BackendLink;
use crate::hosts::Instances;
use crate::link::Link;
use crate::notices::{Notices, notice_of};
use crate::settings::SettingsMirror;
use domain_types::patronus::PatronusRobot;
use domain_types::session::{News, NewsEvent, RepositoryView};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) const REPO: &str = "/work/repo";

pub(crate) fn robot(id: &str, branch: Option<&str>, stack_root: Option<&str>) -> PatronusRobot {
  serde_json::from_value(json!({
    "id": id, "name": "Add the login", "status": "failure", "pushMode": "rebaseAutosquash", "branchName": branch,
    "stackRoot": stack_root, "sourceRevision": null, "targetBranch": "master", "startTime": 1, "finishTime": 2,
    "cancellationReason": null, "url": "https://patronus/robot",
  }))
  .unwrap()
}

/// A view whose facts know the robots
pub(crate) fn view_with(robots: &[PatronusRobot]) -> RepositoryView {
  let mut view = RepositoryView::default();
  for robot in robots {
    view.facts.robots.robots.insert(robot.id.clone(), robot.clone());
  }
  view
}

pub(crate) fn failed(id: u32, robot_id: &str, needs_review: Option<bool>) -> News {
  News {
    id,
    event: NewsEvent::RobotFailed {
      robot_id: robot_id.to_owned(),
      needs_review,
    },
  }
}

/// A failed robot is a notification whose action reveals the branch it tested: the root of a whole stack; a robot of a
/// temporary ref has no branch to reveal, and another news is no notification
#[test]
fn a_failed_robot_is_a_notification_whose_action_reveals_its_branch() {
  let view = view_with(&[robot("r1", Some("auth"), None), robot("r2", None, Some("air")), robot("r3", None, None)]);
  assert_eq!(
    notice_of(&view, REPO, &failed(1, "r1", Some(false))),
    Some(json!({
      "title": "Safe Push of auth failed",
      "body": "auth → master",
      "kind": "error",
      "actions": [{ "text": "Show in Branch Deck", "run": { "tool": "reveal_branch", "arguments": { "repositoryPath": REPO, "branchName": "auth" } } }],
    }))
  );
  let stack = notice_of(&view, REPO, &failed(2, "r2", Some(true))).unwrap();
  assert_eq!(stack["title"], "Safe Push of the whole air stack needs review");
  assert_eq!(stack["kind"], "warning");
  assert_eq!(stack["actions"][0]["run"]["arguments"]["branchName"], "air");
  let no_branch = notice_of(&view, REPO, &failed(3, "r3", None)).unwrap();
  assert_eq!(no_branch["title"], "Safe Push of Add the login failed");
  assert_eq!(no_branch.get("actions"), None);
  assert_eq!(notice_of(&view, REPO, &failed(4, "gone", None)), None, "a robot the view lacks");
  let merged = News {
    id: 5,
    event: NewsEvent::RobotMerged {
      robot_id: "r1".to_owned(),
      closes: Vec::new(),
      landed: Vec::new(),
    },
  };
  assert_eq!(notice_of(&view, REPO, &merged), None);
}

/// The requests brd wrote so far, by their method
/// The messages `out` holds since the last look
pub(crate) fn written(out: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Vec<Value> {
  let mut messages = Vec::new();
  while let Ok(message) = out.try_recv() {
    messages.push(serde_json::from_str(&message).unwrap());
  }
  messages
}

/// The backend answers each claim of a news the bridge sent since the last look: the first of each news is the first
fn claims_answered(backend: &BackendLink, sent: &mut tokio::sync::mpsc::UnboundedReceiver<String>, claimed: &mut Vec<Value>) {
  for claim in written(sent) {
    let first = !claimed.contains(&claim["params"]);
    claimed.push(claim["params"].clone());
    backend.answered(&claim["id"], Ok(json!({ "first": first })));
  }
}

/// The bridge tells a failure while the main view is hidden, and leaves out the page's notification of it; while the
/// main view shows, the page's notification of the failure (its frame without the focus) takes the action of the news;
/// with the switch of Settings off the bridge tells none. The bridge tells a news once its claim is the first of the
/// backend
#[test]
fn the_bridge_tells_a_failure_while_the_main_view_is_hidden_and_the_notification_of_the_page_takes_its_action_while_shown() {
  let settings = Arc::new(SettingsMirror::default());
  let (link, mut out) = Link::new();
  let (backend, mut sent) = BackendLink::new();
  let mut claimed = Vec::new();
  let instances = Arc::new(Instances::default());
  let notices = Notices::new(Arc::new(link), Arc::clone(&backend), Arc::clone(&instances), Arc::clone(&settings));
  let view = view_with(&[robot("r1", Some("auth"), None)]);
  let page = |news: u32| json!({ "title": "Safe Push of auth failed", "body": "Fast Tests · auth → master", "extra": { "repositoryPath": REPO, "news": news } });

  instances.shown("main:1", json!({}));
  notices.tell(&view, REPO, &[failed(1, "r1", Some(false))]);
  claims_answered(&backend, &mut sent, &mut claimed);
  assert_eq!(written(&mut out), Vec::<Value>::new(), "the toast of the shown main view tells it");
  let notification = notices.page_notification(&page(1)).unwrap();
  assert_eq!(notification["body"], "Fast Tests · auth → master", "the words of the page");
  assert_eq!(notification["kind"], "error");
  assert_eq!(notification["actions"][0]["run"]["arguments"]["branchName"], "auth");
  // the page tells it, its own claim the first
  notices.tell_page_notification(&page(1), || {});
  assert_eq!(written(&mut sent), Vec::<Value>::new(), "the page claimed it before it notified");
  assert_eq!(written(&mut out).len(), 1);

  instances.hidden("main:1");
  notices.tell(&view, REPO, &[failed(2, "r1", Some(false))]);
  claims_answered(&backend, &mut sent, &mut claimed);
  let told = written(&mut out);
  assert_eq!(told.len(), 1, "{told:#?}");
  assert_eq!(told[0]["method"], "x-air-app/notify");
  assert_eq!(told[0]["params"]["actions"][0]["run"]["tool"], "reveal_branch");
  assert_eq!(notices.page_notification(&page(2)), None, "the bridge told it");
  assert_eq!(
    notices.page_notification(&json!({ "title": "A review request", "body": "#12" })),
    Some(json!({ "title": "A review request", "body": "#12" })),
    "a notification of no news of the bridge stays the page's"
  );

  settings.changed(&json!({ "key": "notifyPatronusFailures", "value": false, "version": 1, "origin": "main" }));
  notices.tell(&view, REPO, &[failed(3, "r1", Some(false))]);
  assert_eq!(written(&mut sent), Vec::<Value>::new(), "Settings turned the failures off: no claim");
  assert_eq!(written(&mut out), Vec::<Value>::new());
}
