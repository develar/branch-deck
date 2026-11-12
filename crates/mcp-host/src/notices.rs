//! The notifications of the IDE (openspec/specs/repository/ide-embedding/spec.md, "A notification of the OS is a
//! notification of the IDE"): the failure of a robot, a news of the session that the mirror of its repository hears, is
//! `x-air-app/notify` with the action Show in Branch Deck, the tool `reveal_branch` of the branch the robot tested (the
//! root of a whole stack), which shows the main view and reveals the branch in it.
//!
//! brd tells the failure itself while the main view is not shown (the IDE shows another tab), as the desktop app tells
//! it by the OS while no window has the focus, if the switch "Safe Push failures" of Settings is on; while the main
//! view is shown its toast tells it. The page's own notification of a news (`plugin:notification|notify` with
//! `extra: {repositoryPath, news}`, sent while its frame has no focus) is left out once brd told that news, and takes
//! the action of the news when brd did not, so the IDE shows one notification of a failure, and each opens its branch.

use crate::hosts::Instances;
use crate::link::Link;
use app_settings::service::SettingsService;
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
  instances: Arc<Instances>,
  settings: Arc<SettingsService>,
  handled: Mutex<VecDeque<Handled>>,
}

impl Notices {
  pub fn new(link: Arc<Link>, instances: Arc<Instances>, settings: Arc<SettingsService>) -> Self {
    Self {
      link,
      instances,
      settings,
      handled: Mutex::default(),
    }
  }

  /// The news a patch of the mirror of `repository_path` brought, `view` the view after it: the failure of a robot is
  /// a notification of the IDE while the main view is not shown and Settings lets it
  pub fn tell(&self, view: &RepositoryView, repository_path: &str, news: &[News]) {
    for news in news {
      let Some(notice) = notice_of(view, repository_path, news) else {
        continue;
      };
      let told = !self.instances.main_shown() && self.settings.value(FAILURES_SWITCH).as_bool().unwrap_or(true);
      if told {
        self.link.ask("x-air-app/notify", notice.clone());
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

  /// The notification of the IDE of the page's `plugin:notification|notify {options}`: its title and its body; of a
  /// news brd told, none, and of one it did not, with the kind and the action of that news
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
