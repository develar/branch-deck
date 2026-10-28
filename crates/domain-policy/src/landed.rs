//! What landed a subject (docs/design/rust-domain-authority.md, D7, D13; spec/branch/next-step.spec.md, row 1): a push
//! of this session put its copy into the target, the fetched baseline has all of it, merged Safe Pushes landed it, or
//! GitHub merged its pull request with the commits it has (`landed_state`, one answer for the view of the app and for
//! `bd safe-push`), what a start says of it
//! (`Landed::reason` of `landed_reason`), and the open pull requests that hold nothing more than what landed
//! (`pull_requests_to_close`, D7): of the branches a merged Safe Push landed whole besides its own, of the branches a push
//! of this session put into the target, of the branches the fetched baseline or a merged pull request has (`Landing`);
//! the app closes them, `bd robot` and `bd land` list them.

use crate::facts::{RemoteBranch, Robots};
use crate::reasons::{landed, stack_landed};
use domain_types::landing::{BaselinePush, Landed, LandedBy, LandedPullRequest};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::{PatronusView, RobotLandedBranch, RobotOf};
use domain_types::session::{LandedSection, stack_readiness_key};
use std::collections::BTreeMap;

/// What the rules read to decide what landed a subject: the landing report of the last sync, what merged Safe Pushes
/// landed of each branch and the user's robots
#[derive(Clone, Copy, Debug)]
pub struct LandedFacts<'a> {
  /// The commits the fetched baseline has, with the check of the update that names its upstream
  pub baseline: &'a LandedSection,
  /// The Patronus view (`landed_by` of each branch); `None` before it stands
  pub view: Option<&'a PatronusView>,
  pub robots: &'a Robots<'a>,
  /// The copies the pushes of this session into the target landed, by subject key, until a fetch shows them in the
  /// baseline; `None` where nothing keeps them (`bd`)
  pub pushed: Option<&'a BTreeMap<String, BaselinePush>>,
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

/// What landed `subject`, the first that holds: a push of this session put it into the target at the tip it is at (the
/// baseline has it once fetched), the fetched baseline has every commit of it (of a whole stack, of every branch),
/// merged Safe Pushes landed it (of a branch also a part of it, `after` the work left; of a whole stack every branch
/// whole), GitHub merged its pull request with the commits it has (`merged_with`, D13). `None` while work is left that
/// none of them has
#[must_use]
pub fn landed_state(facts: &LandedFacts<'_>, subject: &LandedSubject<'_>) -> Option<Landed> {
  by_push(facts.pushed, subject)
    .or_else(|| by_baseline(facts.baseline, subject.of))
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

/// What landed the branches whose open pull requests close (`pull_requests_to_close`)
#[derive(Clone, Copy, Debug)]
pub enum Landing<'a> {
  /// The merged `robot` of Patronus, with the branches it landed as the Patronus view names them (`landed`, by the
  /// hashes or by content)
  Robot { robot: &'a PatronusRobot, landed: &'a [RobotLandedBranch] },
  /// A push of this session put the copy of a piece or of a whole stack into the target: `branches`, what it landed (the
  /// `lands` of its plan, root first). `chain`: the commit it pushed is a virtual branch, whose history has the virtual
  /// branches it is built on (a piece, the last branch of a straight chain), not a stack branch, which copies them anew
  Push { branches: &'a [String], chain: bool },
  /// The fetched baseline has every commit of `branches`, or GitHub merged a pull request that has them (a branch's own,
  /// which has the branches it is built on, a whole stack's)
  Content { branches: &'a [String] },
}

/// The open pull requests that `landing` landed all of, of the branches `subject` finds in the table: only a pull
/// request that holds nothing the branch lacks (`holds_nothing_more`), then the landing has every commit of it.
/// - A merged robot: the branches it landed whole besides its own, of the ancestors of a stacked branch (its branch
///   carries their commits), of every branch of the stack for the robot of the whole stack; the pull request the robot
///   started from is Patronus' to close, and so is the robot's own branch's.
/// - A push: every branch it landed, its own too, whose pull request shows an older version of it (a head the push
///   left out); a pull request whose head is its branch's tip, pushed in the history of the copy (`chain`), is GitHub's:
///   its head is in the target now, which merges it.
/// - The fetched baseline or a merged pull request: every branch it has whose pull request is still open.
#[must_use]
pub fn pull_requests_to_close<'a>(landing: &Landing<'_>, subject: impl Fn(&str) -> Option<LandedSubject<'a>>) -> Vec<LandedPullRequest> {
  let open = |name: &str, kept: &dyn Fn(&PullRequest, Option<&str>) -> bool| -> Option<LandedPullRequest> {
    let found = subject(name)?;
    let pull_request = found.pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open)?;
    (!kept(pull_request, found.tip) && holds_nothing_more(pull_request, found.tip, found.remote)).then(|| LandedPullRequest {
      branch: name.to_owned(),
      number: pull_request.number,
    })
  };
  match *landing {
    Landing::Robot { robot, landed } => {
      let started = robot.pull_request.as_ref().map(|pull_request| pull_request.number);
      landed
        .iter()
        .filter(|branch| !branch.own && branch.after.is_empty())
        .filter_map(|branch| open(&branch.branch, &|pull_request, _| Some(pull_request.number) == started))
        .collect()
    }
    Landing::Push { branches, chain } => branches
      .iter()
      .filter_map(|name| open(name, &|pull_request, tip| chain && head_of(pull_request).is_some() && head_of(pull_request) == tip))
      .collect(),
    Landing::Content { branches } => branches.iter().filter_map(|name| open(name, &|_, _| false)).collect(),
  }
}

/// The number of the pull request of `subject` GitHub merged with the commits it has (D13); `None` while none did
#[must_use]
pub fn merged_pull_request(subject: &LandedSubject<'_>) -> Option<u32> {
  match by_pull_request(subject)?.by {
    LandedBy::PullRequest { number } => Some(number),
    _ => None,
  }
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

/// The push of this session that put the copy of `subject`, as it is now, into the target: a rebuild or a new commit
/// since is new work
fn by_push(pushed: Option<&BTreeMap<String, BaselinePush>>, subject: &LandedSubject<'_>) -> Option<Landed> {
  let key = match subject.of {
    Of::Branch(name) => name.to_owned(),
    Of::WholeStack { root, .. } => stack_readiness_key(root),
  };
  let push = pushed?.get(&key).filter(|push| subject.tip == Some(push.tip.as_str()))?;
  Some(Landed {
    by: LandedBy::Baseline { upstream: push.upstream.clone() },
    after: Vec::new(),
  })
}

/// The landing of the fetched baseline: every commit of the branch, of every branch of a whole stack
fn by_baseline(section: &LandedSection, of: Of<'_>) -> Option<Landed> {
  let whole = |name: &str| section.landing.iter().any(|branch| branch.name.as_deref() == Some(name) && branch.remaining == 0);
  let landed = match of {
    Of::Branch(name) => whole(name),
    Of::WholeStack { members, .. } => !members.is_empty() && members.iter().all(|member| whole(member)),
  };
  landed.then(|| Landed {
    by: LandedBy::Baseline {
      upstream: baseline_upstream(section).to_string(),
    },
    after: Vec::new(),
  })
}

/// The fetched baseline as the landing report names it: the upstream of the check of the update (`origin/master`), else
/// the baseline branch
#[must_use]
pub fn baseline_upstream(section: &LandedSection) -> &str {
  let baseline = section.baseline.as_ref();
  baseline.and_then(|status| status.update.as_ref()).map_or_else(
    || baseline.map_or("the baseline", |status| status.baseline_branch.as_str()),
    |update| update.upstream.as_str(),
  )
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
