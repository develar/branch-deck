//! The notifications of the IDE (openspec/specs/repository/ide-embedding/spec.md, "A notification of the OS is a
//! notification of the IDE"): the failure of a robot, a news of the session that the mirror of its repository hears, is
//! `x-air-app/notify` with the action Show in Branch Deck, the tool `reveal_branch` of the branch the robot tested (the
//! root of a whole stack), which shows the main view and reveals the branch in it.
//!
//! The bridge tells the failure itself while the main view is not shown (the tool window is hidden), as the desktop app
//! tells it by the OS while no window has the focus, if the switch "Safe Push failures" of Settings is on; while the
//! main view is shown its toast tells it. The page's own notification of a news (`plugin:notification|notify` with
//! `extra: {repositoryPath, news}`, sent while its frame has no focus) is left out once the bridge told that news, and
//! takes the action of the news when it did not, so the IDE shows one notification of a failure, and each opens its
//! branch. A news is told once by the backend: the bridge claims each news it tells (`x-branch-deck/claimNews`, the news
//! by what it tells, `NewsClaim`: a robot that failed), and only the first claim of the backend tells it, so a second
//! IDE on the repository, or the desktop app, tells it not again; the page claims its own (`claim_news`) before it
//! notifies.

use crate::backend::BackendLink;
use crate::hosts::Instances;
use crate::link::Link;
use crate::settings::SettingsMirror;
use domain_types::landing::Subject;
use domain_types::session::{News, NewsEvent, RepositoryView};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::patronus_start::push_mode_name;

/// The tool of the action of a notification that shows a branch in the main view
pub const REVEAL_BRANCH_TOOL: &str = "reveal_branch";
/// The event the main view reveals a branch on, `{repositoryPath, branchName}`
pub const REVEAL_BRANCH_EVENT: &str = "reveal-branch";

/// The switch of Settings → Notifications of the failures of robots, on by default
const FAILURES_SWITCH: &str = "notifyPatronusFailures";

/// How many news of the failures the notices keep for the page's notifications of them, the last ones
const KEPT: usize = 64;

/// The notification of the IDE of a news of the session of `repository_path`, as `view` (the view after the patch that
/// brought it) names its object: the failure of a robot, with the action that reveals the branch it tested; none for
/// another news, and none of a robot the view lacks
pub fn notice_of(view: &RepositoryView, repository_path: &str, news: &News) -> Option<Value> {
  let NewsEvent::RobotFailed { robot_id, needs_review } = &news.event else {
    return None;
  };
  let robot = view.facts.robots.robots.get(robot_id)?;
  let subject = Subject::of_robot(robot);
  let words = subject.as_ref().map_or_else(|| robot.name.clone(), Subject::words);
  let mode = push_mode_name(robot.push_mode);
  let mut notice = if *needs_review == Some(true) {
    json!({ "title": format!("{mode} of {words} needs review"), "body": "The code owners did not approve the changes", "kind": "warning" })
  } else {
    json!({ "title": format!("{mode} of {words} failed"), "body": format!("{words} → {}", robot.target_branch), "kind": "error" })
  };
  if let Some(subject) = subject {
    notice["actions"] = json!([{
      "text": "Show in Branch Deck",
      "run": { "tool": REVEAL_BRANCH_TOOL, "arguments": { "repositoryPath": repository_path, "branchName": subject.name() } },
    }]);
  }
  Some(notice)
}

/// A news brd built the notification of
struct Handled {
  repository_path: String,
  news: u32,
  notice: Value,
  /// brd sent it: the main view was not shown
  told: bool,
}

/// The notifications of the news of the mirrors of one connection
pub struct Notices {
  link: Arc<Link>,
  backend: Arc<BackendLink>,
  instances: Arc<Instances>,
  settings: Arc<SettingsMirror>,
  handled: Mutex<VecDeque<Handled>>,
}

/// The claim of a news of `repository_path` by what it tells: the failure of the robot `robot_id` (`NewsClaim` of
/// domain-types; the id of a news starts again with each session, the robot does not)
fn claim(repository_path: &str, robot_id: &str) -> Value {
  json!({ "repositoryPath": repository_path, "news": { "kind": "robotFailed", "robotId": robot_id } })
}

impl Notices {
  pub fn new(link: Arc<Link>, backend: Arc<BackendLink>, instances: Arc<Instances>, settings: Arc<SettingsMirror>) -> Self {
    Self {
      link,
      backend,
      instances,
      settings,
      handled: Mutex::default(),
    }
  }

  /// Tells `notification` once the backend answered that the claim `claim` of its news is the first, then runs `then`
  fn tell_claimed(&self, claim: Value, notification: Option<Value>, then: impl FnOnce() + Send + 'static) {
    let link = Arc::clone(&self.link);
    self.backend.request_then("x-branch-deck/claimNews", claim, move |reply| {
      let first = reply.as_ref().is_ok_and(|answer| answer["first"] == true);
      if let (true, Some(notification)) = (first, notification) {
        link.ask("x-air-app/notify", notification);
      }
      then();
    });
  }

  /// The news a patch of the mirror of `repository_path` brought, `view` the view after it: the failure of a robot is
  /// a notification of the IDE while the main view is not shown and Settings lets it, when the claim of the news is the
  /// first of the backend
  pub fn tell(&self, view: &RepositoryView, repository_path: &str, news: &[News]) {
    for news in news {
      let Some(notice) = notice_of(view, repository_path, news) else {
        continue;
      };
      let told = !self.instances.main_shown() && self.settings.value(FAILURES_SWITCH).as_bool().unwrap_or(true);
      if told && let NewsEvent::RobotFailed { robot_id, .. } = &news.event {
        self.tell_claimed(claim(repository_path, robot_id), Some(notice.clone()), || {});
      }
      let mut handled = self.handled.lock().unwrap_or_else(PoisonError::into_inner);
      if handled.len() == KEPT {
        handled.pop_front();
      }
      handled.push_back(Handled {
        repository_path: repository_path.to_owned(),
        news: news.id,
        notice,
        told,
      });
    }
  }

  /// Tells the page's `plugin:notification|notify {options}` as `page_notification` words it, then runs `then`: the page
  /// claimed its news itself before it notifies (`claim_news`)
  pub fn tell_page_notification(&self, options: &Value, then: impl FnOnce() + Send + 'static) {
    if let Some(notification) = self.page_notification(options) {
      self.link.ask("x-air-app/notify", notification);
    }
    then();
  }

  /// The notification of the IDE of the page's `plugin:notification|notify {options}`: its title and its body; of a
  /// news the bridge told, none, and of one it did not, with the kind and the action of that news
  pub fn page_notification(&self, options: &Value) -> Option<Value> {
    let mut notification = json!({ "title": options["title"].as_str().unwrap_or("Branch Deck") });
    if let Some(body) = options["body"].as_str() {
      notification["body"] = Value::String(body.to_owned());
    }
    if let Some(actions) = options.get("actions").filter(|actions| actions.is_array()) {
      notification["actions"] = actions.clone();
    }
    let extra = &options["extra"];
    if let (Some(repository_path), Some(id)) = (extra["repositoryPath"].as_str(), extra["news"].as_u64()) {
      let handled = self.handled.lock().unwrap_or_else(PoisonError::into_inner);
      if let Some(news) = handled.iter().find(|news| news.repository_path == repository_path && u64::from(news.news) == id) {
        if news.told {
          return None;
        }
        for field in ["kind", "actions"] {
          if let Some(value) = news.notice.get(field) {
            notification[field] = value.clone();
          }
        }
      }
    }
    Some(notification)
  }
}

#[cfg(test)]
#[path = "notices_test.rs"]
pub(crate) mod notices_test;
