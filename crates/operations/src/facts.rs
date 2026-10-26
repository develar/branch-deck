//! The facts an operation is planned on (docs/design/rust-domain-authority.md, "Факты", "Конкурентность"): the reads its
//! needs name (`needs`), through the reads every host has (`FactSources`, `ServiceFactSources`), into the facts of its
//! session (`OneShot`), numbered as a session numbers them. Side by side, one request per fact: first what the others
//! are read over (the robots, the pull requests, the profiles, the rules), then, for a need of the robot of a subject, the
//! Patronus view of the table over the robots (`OneShot::derive_patronus`, which the caller asks itself otherwise), then
//! what the robots, the view and the pull requests name (the profile of the last robot of a subject when
//! the list names none, the problems of its failed robot, its review, all subjects in one read). A read that failed is
//! the answer its fact keeps; a read that panicked answers nothing.

use crate::needs::FactNeed;
use domain_types::patronus::PatronusRobotStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use remote_facts::fact_sources::FactSources;
use repository_model::facts::{FactResult, RobotsRead};
use repository_model::one_shot::OneShot;
use repository_model::readiness_facts::ReadinessDigest;
use repository_model::session::SessionKey;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{Instrument, instrument};

/// A read that answers a fact, owning what it reads with
type Pending = Pin<Box<dyn Future<Output = FactResult> + Send>>;

/// Reads `needs` into the facts of `session` through `sources`; the Patronus view a need of the robot of a subject reads
/// derives with `git` on the caller's thread, between the two rounds of reads
#[instrument(skip_all, fields(needs = needs.len()))]
pub async fn fill(session: &mut OneShot, sources: &Arc<dyn FactSources>, git: &GitCommandExecutor, needs: &[FactNeed]) {
  let key = session.key().clone();
  let baseline = session.facts().table.value().map_or_default(|table| table.baseline.clone());
  let first: Vec<Pending> = unique(needs.iter().filter(|need| need.is_first()))
    .into_iter()
    .filter_map(|need| first_read(sources, &key, &baseline, need))
    .collect();
  apply_all(session, first).await;
  // the robot of a subject is the view's: what matches it costs git, so only a need of one derives it here
  if needs.iter().any(|need| matches!(need, FactNeed::LastProfile(_) | FactNeed::FailedRobot(_))) {
    session.derive_patronus(git);
  }

  let mut then = Vec::new();
  let mut robots: Vec<(String, bool)> = Vec::new();
  let mut readiness = Vec::new();
  for need in needs.iter().filter(|need| !need.is_first()) {
    match need {
      FactNeed::LastProfile(subject) => {
        let unnamed = session
          .subject_robot(subject)
          .filter(|robot| robot.profile.as_ref().is_none_or(|profile| profile.id.is_none()));
        robots.extend(unnamed.map(|robot| (robot.id.clone(), true)));
      }
      FactNeed::FailedRobot(subject) => {
        let failed = session.subject_robot(subject).filter(|robot| robot.status == PatronusRobotStatus::Failure);
        robots.extend(failed.map(|robot| (robot.id.clone(), false)));
      }
      FactNeed::Readiness(subject) => readiness.push(subject.key()),
      FactNeed::Robots | FactNeed::PullRequests | FactNeed::Profiles | FactNeed::Rules { .. } => {}
    }
  }
  for (id, details) in unique(robots.iter()) {
    then.push(robot_read(sources, &key, id.clone(), *details));
  }
  if !readiness.is_empty() {
    let digest = session.readiness_asks(&readiness);
    then.extend(readiness_read(sources, &key, digest));
  }
  apply_all(session, then).await;
}

/// `items` without the repeated ones, in their order
fn unique<'a, T: PartialEq>(items: impl Iterator<Item = &'a T>) -> Vec<&'a T> {
  let mut unique: Vec<&T> = Vec::new();
  for item in items {
    if !unique.contains(&item) {
      unique.push(item);
    }
  }
  unique
}

/// The read of a fact of the first round; none of the others
fn first_read(sources: &Arc<dyn FactSources>, key: &SessionKey, baseline: &str, need: &FactNeed) -> Option<Pending> {
  let (sources, key) = (Arc::clone(sources), key.clone());
  let read: Pending = match need {
    FactNeed::Robots => Box::pin(async move {
      let result = sources.robots(&key, None, false).await;
      FactResult::Robots {
        read: RobotsRead::List { full: true },
        result,
      }
    }),
    FactNeed::PullRequests => Box::pin(async move { FactResult::PullRequests(sources.pull_requests(&key).await) }),
    FactNeed::Profiles => {
      let baseline = baseline.to_owned();
      Box::pin(async move {
        let result = sources.profiles(&key, baseline.clone()).await;
        FactResult::Profiles { baseline, result }
      })
    }
    FactNeed::Rules { branch } => {
      let branch = branch.clone();
      Box::pin(async move {
        let result = sources.branch_rules(&key, branch.clone()).await;
        FactResult::Rules { branch, result }
      })
    }
    FactNeed::LastProfile(_) | FactNeed::FailedRobot(_) | FactNeed::Readiness(_) => return None,
  };
  Some(read)
}

/// The details of the robot `id` (its profile), else its problems
fn robot_read(sources: &Arc<dyn FactSources>, key: &SessionKey, id: String, details: bool) -> Pending {
  let (sources, key) = (Arc::clone(sources), key.clone());
  Box::pin(async move {
    if details {
      let result = sources.robot_details(&key, id.clone()).await.map(Box::new);
      FactResult::RobotDetails { id, result }
    } else {
      let result = sources.problems(&key, id.clone()).await;
      FactResult::Problems { id, result }
    }
  })
}

/// The review of the subjects `digest` asks, in one read that asks Patronus again (the start needs it now); none when
/// it asks nothing (no tip, Patronus did not list the robots)
fn readiness_read(sources: &Arc<dyn FactSources>, key: &SessionKey, digest: ReadinessDigest) -> Option<Pending> {
  if digest.subjects.is_empty() {
    return None;
  }
  let (sources, key) = (Arc::clone(sources), key.clone());
  Some(Box::pin(async move {
    let asks = digest.subjects.iter().map(|subject| subject.ask.clone()).collect();
    let result = sources.readiness(&key, digest.baseline.clone(), asks, true).await;
    let answers = digest.answers(result);
    FactResult::Readiness(answers)
  }))
}

/// Runs `reads` side by side and applies their answers in their order
async fn apply_all(session: &mut OneShot, reads: Vec<Pending>) {
  let mut set = JoinSet::new();
  for (index, read) in reads.into_iter().enumerate() {
    set.spawn(async move { (index, read.await) }.in_current_span());
  }
  let mut answers = Vec::with_capacity(set.len());
  while let Some(joined) = set.join_next().await {
    match joined {
      Ok(answer) => answers.push(answer),
      Err(error) => tracing::warn!(error = %error, "a read of a fact panicked"),
    }
  }
  answers.sort_by_key(|(index, _)| *index);
  for (_, result) in answers {
    session.apply(result);
  }
}
