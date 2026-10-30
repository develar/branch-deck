//! The open pull requests a landing without a robot closes (docs/design/landing.md, "Closing by content";
//! spec/branch/landing-routes.spec.md, "Closing what landed"): a push of this session into the target, told as it
//! ended, while the table still has the branches its update drops (`push_closes`); the fetched baseline that has every
//! commit of branches of the table, and a pull request GitHub merged with the commits of its branch (`content_closes`,
//! each judged once per tip and head). Which pull requests close is the rule `domain_policy::landed::
//! pull_requests_to_close` (`Landing::Push`, `Landing::Content`); the session tells each landing as news
//! (`NewsEvent::Landed`) and keeps what landed it (`LandedWith`), so the plan of the close (`operations::close`) finds
//! it and words its comments after the rows are gone. A pull request is told once, by any landing, a robot's too.

use crate::branch_table::{BranchTable, in_baseline};
use crate::derive::landed_subject;
use crate::facts::TableFacts;
use crate::table::TableState;
use domain_policy::facts::Repository;
use domain_policy::landed::{Landing, Of, baseline_upstream, merged_pull_request, pull_requests_to_close};
use domain_policy::stack::build_ancestors;
use domain_types::branch_names::baseline_on_remote;
use domain_types::landing::{BaselinePush, LandedPullRequest, LandedWith};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestState};
use domain_types::session::{NewsEvent, PullRequestsView, stack_readiness_key, stack_root_of_readiness_key};
use std::collections::{BTreeSet, HashSet};

/// The landings told, at most: a close is asked as the news comes
const KEPT: usize = 16;

/// A landing without a robot the session told: what landed it and the open pull requests it closes
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToldLanding {
  pub with: LandedWith,
  pub closes: Vec<LandedPullRequest>,
}

/// The landings without a robot a session told, the pull requests every landing told and what the landings by content
/// were judged of
#[derive(Debug, Default)]
pub struct LandedCloses {
  /// The last landings told, oldest first, by the id their news named
  told: Vec<(String, ToldLanding)>,
  /// The pull requests a landing told, a robot's merge too
  numbers: BTreeSet<u32>,
  /// What a landing by content was judged of once: `baseline:<branch>:<tip>`, `pull:<number>:<head>`
  judged: HashSet<String>,
  /// The number of the last landing told
  last: u32,
}

impl LandedCloses {
  /// The landing told as `id`
  #[must_use]
  pub fn get(&self, id: &str) -> Option<&ToldLanding> {
    self.told.iter().find(|(told, _)| told == id).map(|(_, landing)| landing)
  }

  /// The pull requests the news of a robot's merge named: no landing tells them again
  pub fn told_by_robot(&mut self, closes: &[LandedPullRequest]) {
    self.numbers.extend(closes.iter().map(|close| close.number));
  }

  /// Keeps the landing `with` with those of `closes` no landing told; its news, `None` when none is left
  pub fn tell(&mut self, with: LandedWith, closes: Vec<LandedPullRequest>) -> Option<NewsEvent> {
    let closes: Vec<LandedPullRequest> = closes.into_iter().filter(|close| !self.numbers.contains(&close.number)).collect();
    if closes.is_empty() {
      return None;
    }
    self.numbers.extend(closes.iter().map(|close| close.number));
    self.last = self.last.saturating_add(1);
    let id = format!("landing-{}", self.last);
    self.told.push((id.clone(), ToldLanding { with, closes: closes.clone() }));
    if self.told.len() > KEPT {
      self.told.remove(0);
    }
    Some(NewsEvent::Landed { landing: id, closes })
  }

  /// What a push of this session landed (`landed`, as `push_closes` reads it) is judged by the push: its branches at the
  /// tips it pushed and the pull requests of them `pull_requests` lists, which the fetch after it shows in the baseline
  /// and GitHub may show merged (a head the push put into the target)
  pub fn judged_by_push(&mut self, landed: &[(String, BaselinePush)], pull_requests: &PullRequestsView) {
    for (name, pushed) in landed.iter().filter(|(key, _)| stack_root_of_readiness_key(key).is_none()) {
      self.judged.insert(baseline_key(name, &pushed.tip));
      if let Some(pull_request) = pull_requests.pull_requests.get(name) {
        self.judged.insert(pull_key(pull_request));
      }
    }
    for (key, _) in landed {
      if let Some(pull_request) = stack_root_of_readiness_key(key).and_then(|root| pull_requests.stack_pull_requests.get(root)) {
        self.judged.insert(pull_key(pull_request));
      }
    }
  }

  /// Whether `state` shows a landing by content not judged yet: a branch the fetched baseline has whole at a tip, a
  /// merged pull request at a head. Cheap: no table facts are built while nothing is new
  #[must_use]
  pub fn has_unjudged(&self, state: &TableState, table: &BranchTable, pull_requests: &PullRequestsView) -> bool {
    in_baseline(&state.parts.sections.landed.landing).any(|name| table.tip_of(name).is_some_and(|tip| !self.judged.contains(&baseline_key(name, tip))))
      || merged(pull_requests).any(|pull_request| !self.judged.contains(&pull_key(pull_request)))
  }

  /// The landings by content of `state` not judged yet, each with the open pull requests it closes, judged now: the
  /// fetched baseline (every branch of the table it has whole, at the tip the table has), each pull request GitHub merged
  /// with the commits of its branch (the branch with the branches it is built on, of a whole stack every branch). Only
  /// over a listing of the pull requests
  pub fn content_closes(&mut self, state: &TableState, table: &TableFacts, pull_requests: &PullRequestsView) -> Vec<(LandedWith, Vec<LandedPullRequest>)> {
    if !matches!(pull_requests.head.value, Some(PullRequestAvailability::Available { .. })) {
      return Vec::new();
    }
    let (branches, remote) = (&table.table, &table.remote);
    let subject = |name: &str| {
      branches
        .branch(name)
        .map(|found| landed_subject(branches, remote, Of::Branch(&found.name), Some(pull_requests)))
    };
    let mut landings = Vec::new();
    let section = &state.parts.sections.landed;
    let fetched: Vec<String> = in_baseline(&section.landing)
      .filter_map(|name| Some((name, branches.tip_of(name)?)))
      .filter(|(name, tip)| self.judged.insert(baseline_key(name, tip)))
      .map(|(name, _)| name.to_owned())
      .collect();
    if !fetched.is_empty() {
      let closes = pull_requests_to_close(&Landing::Content { branches: &fetched }, subject);
      landings.push((
        LandedWith::Baseline {
          upstream: baseline_upstream(section).to_owned(),
        },
        closes,
      ));
    }
    let repo = Repository::of_grouping(&branches.branches, &branches.parked);
    let unjudged: Vec<&PullRequest> = merged(pull_requests).filter(|pull_request| self.judged.insert(pull_key(pull_request))).collect();
    for pull_request in unjudged {
      let of_branch = pull_requests.pull_requests.iter().find(|(_, listed)| listed.number == pull_request.number);
      let (key, landed) = if let Some((name, _)) = of_branch {
        let found = subject(name);
        let lands = found.as_ref().and_then(merged_pull_request) == Some(pull_request.number);
        let mut lands_with: Vec<String> = build_ancestors(&repo, name).into_iter().map(str::to_owned).collect();
        lands_with.push(name.clone());
        (name.clone(), lands.then_some(lands_with))
      } else {
        let Some((root, _)) = pull_requests.stack_pull_requests.iter().find(|(_, listed)| listed.number == pull_request.number) else {
          continue;
        };
        let Some(stack) = branches.stack(root) else {
          continue;
        };
        let found = landed_subject(branches, remote, Of::WholeStack { root, members: &stack.members }, Some(pull_requests));
        let lands = merged_pull_request(&found) == Some(pull_request.number);
        (stack_readiness_key(root), lands.then(|| stack.members.clone()))
      };
      if let Some(landed) = landed {
        let closes = pull_requests_to_close(&Landing::Content { branches: &landed }, subject);
        let with = LandedWith::PullRequest {
          number: pull_request.number,
          subject: key,
          target: pull_request.base_ref_name.clone(),
        };
        landings.push((with, closes));
      }
    }
    landings
  }
}

/// What the push of this session that ended landed and the open pull requests that closes, over `table` as it is before
/// its update: `landed` as the push told it (`Sessions::tell_baseline_push`: each branch its plan lands at the tip it
/// pushed, root first, then the subject at the commit pushed, a whole stack by `stack:<root>`); `None` for a push that
/// landed nothing
#[must_use]
pub fn push_closes(table: &TableFacts, pull_requests: &PullRequestsView, landed: &[(String, BaselinePush)]) -> Option<(LandedWith, Vec<LandedPullRequest>)> {
  let stack = landed.iter().find(|(key, _)| stack_root_of_readiness_key(key).is_some());
  let (subject, pushed) = stack.or_else(|| landed.last())?;
  let branches: Vec<String> = landed
    .iter()
    .filter(|(key, _)| stack_root_of_readiness_key(key).is_none())
    .map(|(key, _)| key.clone())
    .collect();
  // a stack branch copies the branches anew; the virtual branch of a piece, of the last branch of a straight chain, has
  // the virtual branches it is built on in its history
  let chain = stack.is_none_or(|(_, stack)| landed.iter().any(|(key, branch)| stack_root_of_readiness_key(key).is_none() && branch.tip == stack.tip));
  let (rows, remote) = (&table.table, &table.remote);
  let target = baseline_on_remote(&pushed.upstream);
  let closes = pull_requests_to_close(
    &Landing::Push {
      branches: &branches,
      chain,
      target,
    },
    |name| rows.branch(name).map(|found| landed_subject(rows, remote, Of::Branch(&found.name), Some(pull_requests))),
  );
  let with = LandedWith::Push {
    subject: subject.clone(),
    tip: pushed.tip.clone(),
    upstream: pushed.upstream.clone(),
  };
  Some((with, closes))
}

/// The merged pull requests of the listing, of the branches and of the whole stacks
fn merged(pull_requests: &PullRequestsView) -> impl Iterator<Item = &PullRequest> {
  pull_requests
    .pull_requests
    .values()
    .chain(pull_requests.stack_pull_requests.values())
    .filter(|pull_request| pull_request.state == PullRequestState::Merged)
}

fn baseline_key(name: &str, tip: &str) -> String {
  format!("baseline:{name}:{tip}")
}

fn pull_key(pull_request: &PullRequest) -> String {
  format!("pull:{}:{}", pull_request.number, pull_request.head_ref_oid)
}

#[cfg(test)]
#[path = "landed_closes_test.rs"]
mod landed_closes_test;
