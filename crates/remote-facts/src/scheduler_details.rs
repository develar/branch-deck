//! The details of the robots the windows of a session demand, as its task asks them (the parent module's header)

use super::{RobotRead, SessionFacts};
use domain_policy::refresh::{DetailsRead, RUNNING_DETAILS_REFRESH, robot_details_due};
use domain_types::patronus::PatronusRobotStatus;
use repository_model::facts::FactResult;
use std::collections::HashMap;
use std::time::Duration;

/// By robot a window demands the details of, the last read started while it does
#[derive(Default)]
pub(super) struct DetailsSchedule {
  last: HashMap<String, DetailsRead>,
}

impl SessionFacts {
  /// When the details of a running robot a window shows are read again: `RUNNING_DETAILS_REFRESH` after their last
  /// read, unless one is in flight
  pub(super) fn details_timer(&self) -> Option<Duration> {
    self
      .robots
      .digest
      .details
      .iter()
      .filter(|robot| robot.status == PatronusRobotStatus::Running && !self.robots.reading.contains(&RobotRead::Details(robot.id.clone())))
      .filter_map(|robot| self.details.last.get(&robot.id))
      .filter(|read| read.status == PatronusRobotStatus::Running)
      .map(|read| read.started_at + RUNNING_DETAILS_REFRESH)
      .min()
  }

  /// Reads the details of each demanded robot by `robot_details_due`: unknown, of another status than the robot has now,
  /// or of a running robot a minute old; a robot no window demands any more is forgotten, so a demand of it again reads
  /// them anew
  pub(super) fn read_details(&mut self) {
    if !self.watched {
      return;
    }
    let now = self.now();
    let demanded = self.robots.digest.details.clone();
    self.details.last.retain(|id, _| demanded.iter().any(|robot| robot.id == *id));
    for robot in demanded {
      let what = RobotRead::Details(robot.id.clone());
      if self.robots.reading.contains(&what) || !robot_details_due(now, robot.status, self.details.last.get(&robot.id).copied()) {
        continue;
      }
      self.details.last.insert(
        robot.id.clone(),
        DetailsRead {
          started_at: now,
          status: robot.status,
        },
      );
      let id = robot.id;
      self.read(what, move |scheduler, key| {
        Box::pin(async move {
          let result = scheduler.sources.robot_details(key, id.clone()).await.map(Box::new);
          FactResult::RobotDetails { id, result }
        })
      });
    }
  }
}
