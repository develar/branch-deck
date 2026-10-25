//! What landed a subject (docs/design/rust-domain-authority.md, D7, D13; spec/branch/next-step.spec.md, row 1): the
//! fetched baseline has all of it, merged Safe Pushes landed it, or GitHub merged its pull request with the commits it
//! has (`landed_state`, one answer for the view of the app and for `bd safe-push`), what a start says of it
//! (`Landed::reason` of `landed_reason`), and the open pull requests of the branches a merged Safe Push landed whole
//! besides its own (`pull_requests_to_close`: the app closes them, `bd robot` lists them, D7).

use crate::facts::{RemoteBranch, Robots};
use crate::reasons::{landed, stack_landed};
use domain_types::landing::{Landed, LandedBy, LandedPullRequest};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::{PatronusView, RobotLandedBranch, RobotOf};
use domain_types::session::LandedSection;

/// What the rules read to decide what landed a subject: the landing report of the last sync, what merged Safe Pushes
/// landed of each branch and the user's robots
#[derive(Clone, Copy, Debug)]
pub struct LandedFacts<'a> {
  /// The commits the fetched baseline has, with the check of the update that names its upstream
  pub baseline: &'a LandedSection,
  /// The Patronus view (`landed_by` of each branch); `None` before it stands
  pub view: Option<&'a PatronusView>,
  pub robots: &'a Robots<'a>,
}

/// What a subject is
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Of<'a> {
  Branch(&'a str),
  /// The whole stack of `root`, the branches of its stack branch
  WholeStack {
    root: &'a str,
    members: &'a [String],
  },
}

/// A subject as `landed_state` reads it: what it is, where its branch is now and its pull request
#[derive(Clone, Copy, Debug)]
pub struct LandedSubject<'a> {
  pub of: Of<'a>,
  /// The tip of its virtual branch, of the stack branch of a whole stack; `None` while the sync did not build it
  pub tip: Option<&'a str>,
  /// Its remote branch, once a remote status said it
  pub remote: Option<RemoteBranch<'a>>,
  /// Its pull request in the last listing of GitHub, in any state; `None` without one or before a listing
  pub pull_request: Option<&'a PullRequest>,
}

/// What landed `subject`, the first that holds: the fetched baseline has every commit of it (of a whole stack, of every
/// branch), merged Safe Pushes landed it (of a branch also a part of it, `after` the work left; of a whole stack every
/// branch whole), GitHub merged its pull request with the commits it has (`merged_with`, D13). `None` while work is left
/// that none of them has
#[must_use]
pub fn landed_state(facts: &LandedFacts<'_>, subject: &LandedSubject<'_>) -> Option<Landed> {
  by_baseline(facts.baseline, subject.of)
    .or_else(|| by_robots(facts, subject.of))
    .or_else(|| by_pull_request(subject))
}

/// What a start of `subject` says of it while all of it landed (`landed_state`): nothing of it is left to land. `None`
/// while work is left
#[must_use]
pub fn landed_reason(facts: &LandedFacts<'_>, subject: &LandedSubject<'_>) -> Option<Reason> {
  let state = landed_state(facts, subject).filter(Landed::is_whole)?;
  Some(match state.by {
    LandedBy::Baseline { upstream } => Reason::LandedIn { upstream },
    LandedBy::PullRequest { number } => Reason::PullRequestMerged { number },
    LandedBy::Robots { .. } => robots_reason(facts, subject.of)?,
  })
}

/// The open pull requests of the branches the merged `robot` landed whole besides its own (`landed`, as the Patronus view
/// names them, by the hashes or by content), of the branches `subject` finds in the table: of the ancestors of a stacked
/// branch (its branch carries their commits), of every branch of the stack for the robot of the whole stack. Only a pull
/// request that holds nothing the branch lacks (`holds_nothing_more`): then the merge has every commit of it. The pull
/// request the robot started from is Patronus' to close, and so is the robot's own branch's
#[must_use]
pub fn pull_requests_to_close<'a>(robot: &PatronusRobot, landed: &[RobotLandedBranch], subject: impl Fn(&str) -> Option<LandedSubject<'a>>) -> Vec<LandedPullRequest> {
  let started = robot.pull_request.as_ref().map(|pull_request| pull_request.number);
  landed
    .iter()
    .filter(|branch| !branch.own && branch.after.is_empty())
    .filter_map(|branch| {
      let found = subject(&branch.branch)?;
      let pull_request = found.pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open)?;
      (Some(pull_request.number) != started && holds_nothing_more(pull_request, found.tip, found.remote)).then(|| LandedPullRequest {
        branch: branch.branch.clone(),
        number: pull_request.number,
      })
    })
    .collect()
}

/// The pull request shows nothing the branch lacks, by content: its head is the branch's tip, or the remote branch the
/// remote status compared, which has no commit the branch lacks (older versions of commits amended or restacked here are
/// no such commits: their newer versions landed)
fn holds_nothing_more(pull_request: &PullRequest, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  let head = head_of(pull_request);
  head.is_some() && (head == tip || remote.is_some_and(|remote| remote.tip == head && remote.drops == 0))
}

/// The pull request merged the commits the branch has, by content: its head is the branch's tip, or the remote branch,
/// which has what the branch has (a branch only rebuilt on a newer baseline since). One merged with other commits than
/// the branch has now is new work under the same name
fn merged_with(pull_request: &PullRequest, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  let head = head_of(pull_request);
  pull_request.state == PullRequestState::Merged && head.is_some() && (head == tip || remote.is_some_and(|remote| remote.tip == head && remote.up_to_date))
}

/// The head of the pull request; `None` when GitHub didn't name it
fn head_of(pull_request: &PullRequest) -> Option<&str> {
  Some(pull_request.head_ref_oid.as_str()).filter(|head| !head.is_empty())
}

/// The landing of the fetched baseline: every commit of the branch, of every branch of a whole stack
fn by_baseline(section: &LandedSection, of: Of<'_>) -> Option<Landed> {
  let whole = |name: &str| section.landing.iter().any(|branch| branch.name.as_deref() == Some(name) && branch.remaining == 0);
  let landed = match of {
    Of::Branch(name) => whole(name),
    Of::WholeStack { members, .. } => !members.is_empty() && members.iter().all(|member| whole(member)),
  };
  let baseline = section.baseline.as_ref();
  let upstream = baseline.and_then(|status| status.update.as_ref()).map_or_else(
    || baseline.map_or("the baseline", |status| status.baseline_branch.as_str()),
    |update| update.upstream.as_str(),
  );
  landed.then(|| Landed {
    by: LandedBy::Baseline { upstream: upstream.to_string() },
    after: Vec::new(),
  })
}

/// What merged Safe Pushes landed: of a branch the robot the Patronus view names, with the work left after it; of a
/// whole stack every branch whole, each robot once in the order they merged
fn by_robots(facts: &LandedFacts<'_>, of: Of<'_>) -> Option<Landed> {
  match of {
    Of::Branch(name) => {
      let landed = facts.view?.branches.get(name)?.landed_by.as_ref()?;
      let robot = facts.robots.get(&landed.by.robot_id)?;
      Some(Landed {
        by: LandedBy::Robots {
          by: vec![landed.by.clone()],
          own: landed.own,
          target: robot.target_branch.clone(),
        },
        after: landed.after.clone(),
      })
    }
    Of::WholeStack { .. } => match robots_reason(facts, of)? {
      Reason::StackLanded { by, own, target, .. } => Some(Landed {
        by: LandedBy::Robots { by, own, target },
        after: Vec::new(),
      }),
      _ => None,
    },
  }
}

/// The reason of what merged Safe Pushes landed whole (`reasons::landed`, `reasons::stack_landed`)
fn robots_reason(facts: &LandedFacts<'_>, of: Of<'_>) -> Option<Reason> {
  let whole = |name: &str| -> Option<(&RobotOf, &PatronusRobot, bool)> {
    let landed = facts.view?.branches.get(name)?.landed_by.as_ref().filter(|landed| landed.after.is_empty())?;
    Some((&landed.by, facts.robots.get(&landed.by.robot_id)?, landed.own))
  };
  match of {
    Of::Branch(name) => {
      let landed_by = facts.view?.branches.get(name)?.landed_by.as_ref()?;
      let (_, robot, _) = whole(name)?;
      Some(landed(name, landed_by, robot))
    }
    Of::WholeStack { root, members } => {
      let landed: Vec<(&RobotOf, &PatronusRobot)> = members.iter().map(|member| whole(member).map(|(by, robot, _)| (by, robot))).collect::<Option<_>>()?;
      stack_landed(root, &landed)
    }
  }
}

/// GitHub merged the pull request of the subject with the commits it has (D13)
fn by_pull_request(subject: &LandedSubject<'_>) -> Option<Landed> {
  let pull_request = subject.pull_request.filter(|pull_request| merged_with(pull_request, subject.tip, subject.remote))?;
  Some(Landed {
    by: LandedBy::PullRequest { number: pull_request.number },
    after: Vec::new(),
  })
}
