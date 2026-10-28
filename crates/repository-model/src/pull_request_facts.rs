//! The GitHub pull requests of a session (docs/design/rust-domain-authority.md, "Факты"; spec/branch/pull-requests.spec.md,
//! "Refreshing" and "Reading pull requests"): the part `facts.pullRequests` of the view, keyed by branch and by the root
//! of a whole stack, as the listings of the scheduler (`remote_facts::scheduler`) and the writes of the app leave them.
//!
//! - A listing replaces the pull requests when GitHub listed them, and when it can't list them for good (no token, a
//!   rejected one, SSO, no access, not on GitHub: no listing stands, never "no pull request"); a failure GitHub recovers
//!   from (an error, the rate limit) keeps the last listing, `Stale` with the reason. What the token may do in the
//!   repository (`viewer_permission`) goes with the listing the same way.
//! - A write of the app (a retarget, a close, the reviewers it asked, a pull request it created) changes the pull
//!   request at once and takes its number when it applies: a listing that started before (its number is older) keeps
//!   the pull request as the write left it, so the search that answered before the write never rolls it back and the
//!   next push doesn't retarget again.
//! - A pull request the app created stays until the search lists it, at most `CREATED_KEEP` on the session clock:
//!   GitHub's search lists a new pull request only after a while.

use crate::facts::Observation;
use crate::facts::Seq;
use domain_policy::pull_request::base_of;
use domain_types::branch_names::{BranchKind, baseline_on_remote, strip_branch_namespace};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState, PullRequestsResult, Reviewer, ReviewerState};
use domain_types::session::{FactState, PullRequestsView};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

/// GitHub's search lists a new pull request only after a while: one the app created is kept this long meanwhile
pub const CREATED_KEEP: Duration = Duration::from_mins(5);

/// Whose pull request: a virtual branch's, or the whole stack's of a root (its stack branch)
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PullRequestSubject {
  Branch(String),
  Stack(String),
}

/// A write of the app to a pull request, as its command answered it
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PullRequestWrite {
  Retargeted {
    number: u32,
    base: String,
  },
  Closed {
    number: u32,
  },
  /// GitHub asked `requested` to review
  ReviewersRequested {
    number: u32,
    requested: Vec<String>,
  },
  Created {
    pull_request: Box<PullRequest>,
  },
}

impl PullRequestWrite {
  /// Whether the pull requests are listed again after the write, as after a push: what the next step of the branch reads
  /// changed; a close asks nothing (the landed pull requests a merge closes would each list again)
  #[must_use]
  pub const fn lists_again(&self) -> bool {
    !matches!(self, Self::Closed { .. })
  }
}

/// A branch or a whole stack a push of the app pushed, with the tip it left on the remote: the listing after the push
/// shows its pull request on it, else one more listing looks a little later (`refresh::pull_requests_push_recheck_at`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushedHead {
  pub subject: PullRequestSubject,
  pub tip: Option<String>,
}

impl PushedHead {
  /// Whether `result` shows the pull request of the pushed subject on another commit than the pushed tip; a subject
  /// without a pull request, or without a tip, shows none
  #[must_use]
  pub fn shows_old_head(&self, result: &PullRequestsResult) -> bool {
    let pull_request = match &self.subject {
      PullRequestSubject::Branch(name) => result.pull_requests.get(name),
      PullRequestSubject::Stack(root) => result.stack_pull_requests.get(root),
    };
    matches!((pull_request, &self.tip), (Some(pull_request), Some(tip)) if &pull_request.head_ref_oid != tip)
  }
}

/// The pull requests as a session keeps them (the module's header): the part of the view, and by subject the number of
/// the app's last write of its pull request and the ones it created
#[derive(Clone, Debug, Default)]
pub struct PullRequestsFact {
  view: PullRequestsView,
  written: HashMap<PullRequestSubject, u32>,
  /// The pull requests the app created, until the search lists them or the time on the session clock passed
  created: HashMap<PullRequestSubject, (PullRequest, Duration)>,
  /// A listing stands: a failure GitHub recovers from keeps it
  listed: bool,
}

impl PullRequestsFact {
  /// The part of the view
  #[must_use]
  pub const fn view(&self) -> &PullRequestsView {
    &self.view
  }

  /// The answer of a listing that answered at `observation.at`; whether it changed the part. An answer not newer than
  /// the last one changes nothing
  pub fn answer(&mut self, observation: Observation, result: PullRequestsResult) -> bool {
    let seq = observation.seq.0;
    if seq <= self.view.head.seq {
      return false;
    }
    let PullRequestsResult {
      availability,
      viewer_permission,
      pull_requests,
      stack_pull_requests,
    } = result;
    let recovers = matches!(availability, PullRequestAvailability::Error { .. } | PullRequestAvailability::RateLimited { .. });
    let mut changed = false;
    let state = if recovers && self.listed {
      FactState::Stale {
        reason: Some(availability_error(&availability)),
      }
    } else {
      // GitHub listed them, listed none, or can't list them: no pull request stands but the app's own
      let listed = self.listing(false, pull_requests, seq, observation.at);
      let stacks = self.listing(true, stack_pull_requests, seq, observation.at);
      let repository = match &availability {
        PullRequestAvailability::Available { owner, name } => Some(format!("{owner}/{name}")),
        _ => None,
      };
      changed |=
        self.view.pull_requests != listed || self.view.stack_pull_requests != stacks || self.view.repository != repository || self.view.viewer_permission != viewer_permission;
      self.view.pull_requests = listed;
      self.view.stack_pull_requests = stacks;
      self.view.repository = repository;
      self.view.viewer_permission = viewer_permission;
      self.listed = matches!(availability, PullRequestAvailability::Available { .. });
      // a write the listing saw needs keeping no more
      self.written.retain(|_, written| *written > seq);
      match availability {
        PullRequestAvailability::Available { .. } | PullRequestAvailability::NotGitHub => FactState::Fresh,
        ref failed => FactState::Unavailable {
          reason: availability_error(failed),
        },
      }
    };
    let head = &mut self.view.head;
    changed |= head.value.as_ref() != Some(&availability) || head.state != state;
    head.value = Some(availability);
    head.state = state;
    head.seq = seq;
    head.observed_at = observation.observed_at;
    changed
  }

  /// `incoming` of the branches (or of the stacks) with the pull requests the app created that the search doesn't list
  /// yet, and with each pull request the app wrote after the listing `seq` started as the session has it
  fn listing(&mut self, stack: bool, mut incoming: BTreeMap<String, PullRequest>, seq: u32, at: Duration) -> BTreeMap<String, PullRequest> {
    self.created.retain(|subject, (created, until)| {
      let Some(name) = subject.name_of(stack) else {
        return true;
      };
      match incoming.get(name) {
        Some(listed) if listed.number == created.number => false,
        _ if at > *until => false,
        listed => {
          if listed.is_none_or(|listed| listed.state == PullRequestState::Closed) {
            incoming.insert(name.to_owned(), created.clone());
          }
          true
        }
      }
    });
    let current = if stack { &self.view.stack_pull_requests } else { &self.view.pull_requests };
    for (subject, written) in &self.written {
      if let Some(name) = subject.name_of(stack)
        && *written > seq
        && let Some(kept) = current.get(name)
      {
        incoming.insert(name.to_owned(), kept.clone());
      }
    }
    incoming
  }

  /// The write of the app numbered `seq`, applied at `at` on the session clock, in the session of `prefix`; whether it
  /// changed a pull request. A write of a pull request the session doesn't have changes nothing
  pub fn write(&mut self, seq: Seq, at: Duration, prefix: &str, write: PullRequestWrite) -> bool {
    let subject = match &write {
      PullRequestWrite::Created { pull_request } => subject_of_head(&pull_request.head_ref_name, prefix),
      PullRequestWrite::Retargeted { number, .. } | PullRequestWrite::Closed { number } | PullRequestWrite::ReviewersRequested { number, .. } => self.subject_of(*number),
    };
    let Some(subject) = subject else {
      return false;
    };
    let map = match &subject {
      PullRequestSubject::Branch(_) => &mut self.view.pull_requests,
      PullRequestSubject::Stack(_) => &mut self.view.stack_pull_requests,
    };
    let name = subject.name().to_owned();
    match write {
      PullRequestWrite::Created { pull_request } => {
        self.created.insert(subject.clone(), ((*pull_request).clone(), at + CREATED_KEEP));
        map.insert(name, *pull_request);
      }
      PullRequestWrite::Retargeted { base, .. } => {
        if let Some(pull_request) = map.get_mut(&name) {
          pull_request.base_ref_name = base;
        }
      }
      PullRequestWrite::Closed { .. } => {
        if let Some(pull_request) = map.get_mut(&name) {
          pull_request.state = PullRequestState::Closed;
        }
      }
      PullRequestWrite::ReviewersRequested { requested, .. } => {
        if let Some(pull_request) = map.get_mut(&name) {
          mark_requested(&mut pull_request.reviewers, &requested);
        }
      }
    }
    self.written.insert(subject, seq.0);
    true
  }

  /// Whose pull request `number` is
  fn subject_of(&self, number: u32) -> Option<PullRequestSubject> {
    let find = |map: &BTreeMap<String, PullRequest>| map.iter().find(|(_, pull_request)| pull_request.number == number).map(|(name, _)| name.clone());
    find(&self.view.pull_requests)
      .map(PullRequestSubject::Branch)
      .or_else(|| find(&self.view.stack_pull_requests).map(PullRequestSubject::Stack))
  }

  /// The bases of the open pull requests someone approved, sorted: the rules a push into them is checked against; an
  /// unknown (empty) base is the target of the baseline (`baseline_on_remote`, as the page reads a start into it), and so
  /// is a virtual branch of `prefix`, which a start retargets into the target first (D11); neither before a table names
  /// the baseline
  #[must_use]
  pub fn approved_bases(&self, baseline: Option<&str>, prefix: &str) -> Vec<String> {
    let target = baseline.map(baseline_on_remote);
    let bases: BTreeSet<&str> = self
      .view
      .pull_requests
      .values()
      .chain(self.view.stack_pull_requests.values())
      .filter(|pull_request| pull_request.state == PullRequestState::Open && pull_request.reviewers.iter().any(|reviewer| reviewer.state == ReviewerState::Approved))
      .filter_map(|pull_request| match base_of(pull_request, target.unwrap_or_default(), prefix) {
        PullRequestBase::Target | PullRequestBase::Virtual { .. } => target,
        PullRequestBase::Other { .. } => Some(pull_request.base_ref_name.as_str()),
      })
      .collect();
    bases.into_iter().map(str::to_owned).collect()
  }
}

impl PullRequestSubject {
  #[must_use]
  pub fn name(&self) -> &str {
    match self {
      Self::Branch(name) | Self::Stack(name) => name,
    }
  }

  /// The name, when the subject is of the stacks (`stack`) or of the branches
  fn name_of(&self, stack: bool) -> Option<&str> {
    match self {
      Self::Branch(name) if !stack => Some(name),
      Self::Stack(root) if stack => Some(root),
      Self::Branch(_) | Self::Stack(_) => None,
    }
  }
}

/// The subject of a pull request whose head is `head` (`{prefix}/virtual/<branch>` or `{prefix}/stack/<root>`)
fn subject_of_head(head: &str, prefix: &str) -> Option<PullRequestSubject> {
  strip_branch_namespace(head, prefix, BranchKind::Virtual)
    .map(|name| PullRequestSubject::Branch(name.to_owned()))
    .or_else(|| strip_branch_namespace(head, prefix, BranchKind::Stack).map(|root| PullRequestSubject::Stack(root.to_owned())))
}

/// The reviewers with `requested` asked: a reviewer of the pull request is marked requested, a new one is added as
/// requested; logins compare without case, as GitHub's
fn mark_requested(reviewers: &mut Vec<Reviewer>, requested: &[String]) {
  for reviewer in reviewers.iter_mut() {
    if requested.iter().any(|login| login.eq_ignore_ascii_case(&reviewer.login)) {
      reviewer.requested = true;
    }
  }
  for login in requested {
    if !reviewers.iter().any(|reviewer| reviewer.login.eq_ignore_ascii_case(login)) {
      reviewers.push(Reviewer {
        login: login.clone(),
        state: ReviewerState::Requested,
        requested: true,
      });
    }
  }
}

/// Why GitHub gave no listing, as the error of the fact (of the pull requests, of the review requests)
pub(crate) fn availability_error(availability: &PullRequestAvailability) -> ApiError {
  let message = match availability {
    PullRequestAvailability::Error { message } => return ApiError::new(ErrorCode::NetworkError, message.clone()),
    PullRequestAvailability::RateLimited { .. } => "GitHub refused the request for its rate limit",
    PullRequestAvailability::SignedOut => "No GitHub token",
    PullRequestAvailability::NotAuthenticated { .. } => "GitHub rejected the token",
    PullRequestAvailability::SsoRequired { .. } => "The organization requires SAML SSO for the token",
    PullRequestAvailability::NoAccess { message, .. } => return ApiError::internal(message.clone()),
    PullRequestAvailability::Available { .. } | PullRequestAvailability::NotGitHub => "GitHub answered",
  };
  ApiError::internal(message)
}

#[cfg(test)]
#[path = "pull_request_facts_test.rs"]
mod pull_request_facts_test;
