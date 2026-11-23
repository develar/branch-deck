//! The history of the finished robots of a repository in its object cache (`object_cache::robot_history`): what the
//! estimates of the running robots learn from beside the robots a session lists (`domain_policy::robot_estimate`),
//! beyond the 7 days a list of Patronus reaches and after a restart. A session reads it as it opens
//! (`SessionHost::robot_history`) and has the host keep what it learnt (`SessionHost::record_robot_history`); every
//! process of the user writes it (the app, `brd mcp`), so a write merges with what the cache has, by robot id
//! (`learn`), and an older write never takes back a newer one's robots.

use domain_policy::robot_estimate::{learn, prune};
use domain_types::patronus::FinishedRobot;
use object_cache::cache::ObjectCache;

/// The history the cache has, within its limits, the newest finish first; none without one
#[must_use]
pub fn read_history(cache: &ObjectCache) -> Vec<FinishedRobot> {
  let mut history: Vec<FinishedRobot> = cache.get_robot_history().unwrap_or_default();
  prune(&mut history);
  history
}

/// Keeps `robots` in the history of the cache, merged with the one it has; nothing when they add nothing to it
pub fn record_history(cache: &ObjectCache, robots: &[FinishedRobot]) {
  if !cache.is_enabled() {
    return;
  }
  let mut history = read_history(cache);
  if learn(&mut history, robots) {
    cache.put_robot_history(&history);
  }
}

#[cfg(test)]
#[path = "robot_history_test.rs"]
mod robot_history_test;
