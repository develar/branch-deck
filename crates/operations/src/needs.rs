//! What the plan of an intent reads of the remote before it decides (docs/design/rust-domain-authority.md, "Факты",
//! "Конкурентность"): the facts of each family (`needs`), and of a start the ones its first decision waits for
//! (`waits`: the readiness of its subject, the rules of its target). `facts::fill` reads them, each once, in its order.

use domain_types::landing::{FactWait, LandFact, Subject};
use domain_types::operation::{Intent, PushScope};

/// A fact of the remote a plan reads
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactNeed {
  /// The user's robots, the full list of the service's window, and the Patronus view of the table over them
  Robots,
  /// The open pull requests of the branches and of the whole stacks, one search
  PullRequests,
  /// The merge profiles of the target of the baseline
  Profiles,
  /// The rules of the base `branch`: whether a push dismisses the approvals of a pull request into it
  Rules { branch: String },
  /// The merge profile of the last robot of a subject when the list names none (its details): the default of a start
  LastProfile(Subject),
  /// The problems of the failed robot of a subject: whether it failed for missing approvals, which its next step says
  FailedRobot(Subject),
  /// The code-owner review of a subject at its tip, with the pull request its start goes from
  Readiness(Subject),
}

impl FactNeed {
  /// A fact the others are read over: the robots, the pull requests, the profiles and the rules are read first, the
  /// ones of a robot or of a review after them and the Patronus view
  #[must_use]
  pub const fn is_first(&self) -> bool {
    matches!(self, Self::Robots | Self::PullRequests | Self::Profiles | Self::Rules { .. })
  }
}

/// The facts the plan of `intent` reads before its first decision: a start reads the robots (and what they land and
/// hold), the pull requests, the profiles and the profile of the last robot of its subject; a push the robots that hold
/// it and the pull requests it retargets into the target first (D3a), none of an unapplied branch (no Safe Push runs on
/// one, its pull request stays as it was); a request of a review the pull requests, the review of
/// its subject (only without `reviewers`: the code owners it asks) and the robots, which hold the push it makes first
/// (`push_first`) and which Patronus lists before it is asked the review; an action on a robot the robots; an action on
/// a pull request the pull requests
#[must_use]
pub fn needs(intent: &Intent) -> Vec<FactNeed> {
  match intent {
    Intent::Land(land) => vec![FactNeed::Robots, FactNeed::PullRequests, FactNeed::Profiles, FactNeed::LastProfile(land.subject.clone())],
    Intent::Push(push) if push.scope == PushScope::Unapplied => Vec::new(),
    Intent::RestartRobot { .. } | Intent::CancelRobot { .. } => vec![FactNeed::Robots],
    Intent::RequestReview { subject, reviewers, push_first } => {
      // the review is asked of a session whose robots Patronus listed
      let robots = (*push_first || reviewers.is_empty()).then_some(FactNeed::Robots);
      let readiness = reviewers.is_empty().then(|| FactNeed::Readiness(subject.clone()));
      robots.into_iter().chain([FactNeed::PullRequests]).chain(readiness).collect()
    }
    Intent::Push(_) | Intent::CloseLandedPullRequests { .. } => vec![FactNeed::Robots, FactNeed::PullRequests],
    Intent::Retarget { .. } | Intent::CreatePullRequest(_) | Intent::ClosePullRequest { .. } => vec![FactNeed::PullRequests],
    Intent::SubmitReview { .. } | Intent::SetIssueState { .. } => Vec::new(),
  }
}

/// What the first plan of a start of `subject` into `target` waits for (`waits`) that it reads now: the review of the
/// subject, the rules of the target for a push into an approved pull request; the other facts it waits for are read
/// first or are the host's (the landing gate, the remote of a sync)
#[must_use]
pub fn waits(waits: &[FactWait], subject: &Subject, target: &str) -> Vec<FactNeed> {
  let mut needs = Vec::new();
  for wait in waits {
    let need = match wait.fact {
      LandFact::Readiness => FactNeed::Readiness(subject.clone()),
      LandFact::Rules { .. } => FactNeed::Rules { branch: target.to_owned() },
      LandFact::Remote | LandFact::Profiles | LandFact::PullRequests | LandFact::LandingNeeds => continue,
    };
    if !needs.contains(&need) {
      needs.push(need);
    }
  }
  needs
}
