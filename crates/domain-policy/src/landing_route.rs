//! The route of a repository into master, for the app and `bd` alike (spec/branch/landing-routes.spec.md): a robot of
//! Patronus, a push of the copy into the target, or a merge of the pull request on GitHub, each with its gate.
//!
//! - `route`: the setting names the route (`branchdeck.landing`); unset, it is Patronus on Space and the pull request
//!   elsewhere, never a push. Patronus named off Space is refused. A push is refused below write access, by the rules of
//!   the target that no push of an unsigned copy passes (`required_signatures`), and for a writer by the rules only a
//!   maintainer or an admin may bypass (a pull request, the merge queue, restricted updates), which those confirm; the
//!   required status checks a push lands without are a confirmation. A merge needs write access too; without it the
//!   route stays and others merge. The facts of GitHub count only when the remote of the baseline pushes to the
//!   repository they describe: otherwise there is no gate, and the server answers. A fact without an answer waits,
//!   never "no". A refusal of the server of a push into the target in this session (`[remote rejected]`: a hook, a
//!   protected branch) is asked first before the next push (`BaselineRefusedPush`): a hook may refuse only some commits.
//!   A fetched baseline with commits the fork point of the checked out branch lacks refuses a push that the gate would
//!   let go (`BaselineAhead`): the copies are built on the fork point, so the server would refuse it as no fast-forward;
//!   the update comes first
//! - `rules_target`: the branch whose rules the session reads for the route: the target, while the repository lands by
//!   a push into it on GitHub

use domain_types::error::ApiError;
use domain_types::landing_route::{LandingRemote, LandingRoute, LandingSetting, RouteFact, RouteGate};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, ViewerPermission};
use domain_types::reason::Reason;
use domain_types::session::{Fact, FactState, PullRequestsView};
use domain_types::update_project::UpdateCheck;

/// What GitHub answered of the repository of the pull requests, as the listing of the session stands
#[derive(Clone, Copy, Debug)]
pub enum GitHubFacts<'a> {
  /// No answer yet
  Pending,
  /// No listing stands: the read failed with `reason`
  Failed { reason: &'a ApiError },
  /// The branches are not on GitHub
  NotGitHub,
  /// A listing of `repository` (`owner/name`) stands, with what the token may do there (`None`: GitHub named nothing)
  Listed { repository: &'a str, permission: Option<ViewerPermission> },
}

impl<'a> GitHubFacts<'a> {
  /// Of the pull requests of the session (`FactsView::pull_requests`): a listing that stands names its repository, also
  /// through a failed refresh after it; none before GitHub's first answer
  #[must_use]
  pub fn of(view: &'a PullRequestsView) -> Self {
    if let Some(repository) = &view.repository {
      return Self::Listed {
        repository,
        permission: view.viewer_permission,
      };
    }
    match (&view.head.value, &view.head.state) {
      (Some(PullRequestAvailability::NotGitHub), _) => Self::NotGitHub,
      (Some(_), FactState::Unavailable { reason } | FactState::Stale { reason: Some(reason) }) => Self::Failed { reason },
      _ => Self::Pending,
    }
  }
}

/// The rules of the target, as the session read them
#[derive(Clone, Copy, Debug)]
pub enum TargetRules<'a> {
  /// Not read yet
  Pending,
  /// The read failed with `reason`
  Failed {
    reason: &'a ApiError,
  },
  Known(&'a BranchRules),
}

impl<'a> TargetRules<'a> {
  /// Of the fact of the rules of the target (`FactsView::rules`); `None`: the session has not asked
  #[must_use]
  pub const fn of(fact: Option<&'a Fact<BranchRules>>) -> Self {
    match fact {
      Some(Fact { value: Some(rules), .. }) => Self::Known(rules),
      Some(Fact {
        state: FactState::Unavailable { reason } | FactState::Stale { reason: Some(reason) },
        ..
      }) => Self::Failed { reason },
      _ => Self::Pending,
    }
  }
}

/// Everything the route is decided on
#[derive(Clone, Copy, Debug)]
pub struct RouteFacts<'a> {
  /// What the sync read of git: the setting, `origin` on Space, where the remote of the baseline pushes
  pub remote: &'a LandingRemote,
  /// The baseline as the sync named it (`origin/master`, a local `master`)
  pub baseline: &'a str,
  pub github: GitHubFacts<'a>,
  /// The rules of the target (`rules_target`)
  pub rules: TargetRules<'a>,
  /// What the server said when it refused a push into the target in this session; `None` without one (`bd`)
  pub refused: Option<&'a str>,
  /// The check of the update of the checked out branch onto the fetched baseline, as the last sync reported it: its
  /// fork point (`base`), which the copies are built on, and the head of the baseline; `None` before one, or without a
  /// sync (`bd status`)
  pub update: Option<&'a UpdateCheck>,
}

/// The route of the repository with its gate
#[must_use]
pub fn route(facts: &RouteFacts<'_>) -> LandingRoute {
  let remote = facts.remote;
  match remote.setting.unwrap_or_else(|| LandingSetting::default_for(remote.on_space)) {
    LandingSetting::Patronus => LandingRoute::Patronus {
      gate: if remote.on_space { RouteGate::Open } else { refused(Reason::NotOnSpace) },
    },
    LandingSetting::Push => LandingRoute::Push {
      gate: with_refusal(behind(push_gate(facts), facts), facts),
      remote: remote.remote.clone(),
    },
    LandingSetting::PullRequest => LandingRoute::PullRequest { merge: merge_gate(facts) },
  }
}

/// The branch whose rules the route reads: the target of a push route whose remote pushes to GitHub; none for another
/// route, which reads no rules of the target
#[must_use]
pub fn rules_target(remote: &LandingRemote) -> Option<&str> {
  match (remote.setting, &remote.push_repository) {
    (Some(LandingSetting::Push), Some(_)) => remote.target.as_deref(),
    _ => None,
  }
}

const fn refused(reason: Reason) -> RouteGate {
  RouteGate::Refused { reason }
}

fn wait(fact: RouteFact, failed: Option<&ApiError>) -> RouteGate {
  RouteGate::Wait { fact, failed: failed.cloned() }
}

/// `gate` asking first about the refusal of the server of a push into the target in this session, unless it refuses
/// already
fn with_refusal(gate: RouteGate, facts: &RouteFacts<'_>) -> RouteGate {
  let (Some(message), Some(target)) = (facts.refused, facts.remote.target.as_deref()) else {
    return gate;
  };
  let refused = Reason::BaselineRefusedPush {
    target: target.to_owned(),
    message: message.to_owned(),
  };
  match gate {
    RouteGate::Open => RouteGate::Confirm { confirmations: vec![refused] },
    RouteGate::Confirm { mut confirmations } => {
      confirmations.insert(0, refused);
      RouteGate::Confirm { confirmations }
    }
    gate @ (RouteGate::Wait { .. } | RouteGate::Refused { .. }) => gate,
  }
}

/// `gate` refusing a push it would let go while the fetched baseline has commits the fork point of the checked out
/// branch lacks (`BaselineAhead`): the copy is built on the fork point, so the push is no fast-forward
fn behind(gate: RouteGate, facts: &RouteFacts<'_>) -> RouteGate {
  match (gate, facts.update) {
    (RouteGate::Open | RouteGate::Confirm { .. }, Some(update)) if update.base != update.upstream_head => refused(Reason::BaselineAhead {
      upstream: update.upstream.clone(),
    }),
    (gate, _) => gate,
  }
}

/// The gate of a push into the target
fn push_gate(facts: &RouteFacts<'_>) -> RouteGate {
  let remote = facts.remote;
  let Some(target) = remote.target.as_deref() else {
    return refused(Reason::LocalBaseline {
      baseline: facts.baseline.to_owned(),
    });
  };
  let Some(pushed_to) = remote.push_repository.as_deref() else {
    // not GitHub: the server answers
    return RouteGate::Open;
  };
  let permission = match facts.github {
    GitHubFacts::Pending => return wait(RouteFact::Permission, None),
    GitHubFacts::Failed { reason } => return wait(RouteFact::Permission, Some(reason)),
    // the facts are of another repository (a mirror), or of none: the server answers
    GitHubFacts::NotGitHub => return RouteGate::Open,
    GitHubFacts::Listed { repository, .. } if !repository.eq_ignore_ascii_case(pushed_to) => return RouteGate::Open,
    GitHubFacts::Listed { permission, .. } => permission,
  };
  if permission.is_some_and(|permission| !permission.can_write()) {
    return refused(Reason::NoWriteAccess {
      repository: pushed_to.to_owned(),
      target: target.to_owned(),
    });
  }
  let rules = match facts.rules {
    TargetRules::Pending => return wait(RouteFact::TargetRules, None),
    TargetRules::Failed { reason } => return wait(RouteFact::TargetRules, Some(reason)),
    TargetRules::Known(rules) => rules,
  };
  let target = || target.to_owned();
  if rules.requires_signatures {
    return refused(Reason::BaselineRequiresSignatures { target: target() });
  }
  let mut confirmations: Vec<Reason> = [
    (rules.requires_pull_request, Reason::BaselineRequiresPullRequest { target: target() }),
    (rules.merge_queue, Reason::BaselineRequiresMergeQueue { target: target() }),
    (rules.restricts_updates, Reason::BaselineRestrictsUpdates { target: target() }),
  ]
  .into_iter()
  .filter_map(|(applies, reason)| applies.then_some(reason))
  .collect();
  // GitHub named no permission: whether the user may bypass is not known, so it is asked, never refused
  if !confirmations.is_empty() && !permission.is_none_or(ViewerPermission::may_bypass) {
    return refused(confirmations.swap_remove(0));
  }
  if rules.required_status_checks {
    confirmations.push(Reason::BaselineRequiresStatusChecks { target: target() });
  }
  if confirmations.is_empty() {
    RouteGate::Open
  } else {
    RouteGate::Confirm { confirmations }
  }
}

/// The gate of a merge of the pull request on GitHub
fn merge_gate(facts: &RouteFacts<'_>) -> RouteGate {
  let (repository, permission) = match facts.github {
    GitHubFacts::Pending => return wait(RouteFact::Permission, None),
    GitHubFacts::Failed { reason } => return wait(RouteFact::Permission, Some(reason)),
    GitHubFacts::NotGitHub => return refused(Reason::NotOnGitHub),
    GitHubFacts::Listed { repository, permission } => (repository, permission),
  };
  if !facts.remote.push_repository.as_deref().is_some_and(|pushed_to| pushed_to.eq_ignore_ascii_case(repository)) {
    return refused(Reason::GitHubMirror {
      repository: repository.to_owned(),
    });
  }
  if permission.is_some_and(|permission| !permission.can_write()) {
    return refused(Reason::NoWriteAccess {
      repository: repository.to_owned(),
      target: facts.remote.target.clone().unwrap_or_else(|| facts.baseline.to_owned()),
    });
  }
  RouteGate::Open
}
