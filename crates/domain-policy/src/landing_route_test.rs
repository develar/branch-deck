use crate::landing_route::{GitHubFacts, RouteFacts, TargetRules, route, rules_target};
use domain_types::error::ApiError;
use domain_types::landing_route::{LandingRemote, LandingRoute, LandingSetting, RouteFact, RouteGate};
use domain_types::pull_request::ViewerPermission::{Admin, Maintain, Read, Triage, Write};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, ViewerPermission};
use domain_types::reason::Reason;
use domain_types::session::{Fact, FactState, PullRequestsView};
use domain_types::update_project::{UpdateCheck, UpdateKind, UpdateState};
use pretty_assertions::assert_eq;

const REPOSITORY: &str = "develar/branch-deck";

/// A repository on github.com whose remote of `origin/master` pushes to `REPOSITORY`
fn on_github(setting: Option<LandingSetting>) -> LandingRemote {
  LandingRemote {
    setting,
    on_space: false,
    push_repository: Some(REPOSITORY.to_owned()),
    target: Some("master".to_owned()),
    remote: Some("origin".to_owned()),
  }
}

/// The monorepo: `origin` on Space, mirrored to GitHub
fn on_space(setting: Option<LandingSetting>) -> LandingRemote {
  LandingRemote {
    setting,
    on_space: true,
    push_repository: None,
    target: Some("master".to_owned()),
    remote: Some("origin".to_owned()),
  }
}

fn listed(permission: ViewerPermission) -> GitHubFacts<'static> {
  GitHubFacts::Listed {
    repository: REPOSITORY,
    permission: Some(permission),
  }
}

fn decide(remote: &LandingRemote, github: GitHubFacts<'_>, rules: TargetRules<'_>) -> LandingRoute {
  route(&RouteFacts {
    remote,
    baseline: "origin/master",
    github,
    rules,
    refused: None,
    update: None,
  })
}

fn push(gate: RouteGate) -> LandingRoute {
  LandingRoute::Push {
    gate,
    remote: Some("origin".to_owned()),
  }
}

fn merge(gate: RouteGate) -> LandingRoute {
  LandingRoute::PullRequest { merge: gate }
}

fn refused(reason: Reason) -> RouteGate {
  RouteGate::Refused { reason }
}

fn target() -> String {
  "master".to_owned()
}

/// Unset, the route is Patronus on Space and the pull request elsewhere, never a push; Patronus named off Space is
/// refused, and a route named on Space is the one named
#[test]
fn the_setting_names_the_route_and_unset_it_follows_the_host() {
  let write = listed(Write);
  let rules = TargetRules::Known(&BranchRules::default());
  let cases = [
    ("unset on Space", on_space(None), LandingRoute::Patronus { gate: RouteGate::Open }),
    ("unset on GitHub", on_github(None), merge(RouteGate::Open)),
    (
      "Patronus on Space",
      on_space(Some(LandingSetting::Patronus)),
      LandingRoute::Patronus { gate: RouteGate::Open },
    ),
    (
      "Patronus off Space",
      on_github(Some(LandingSetting::Patronus)),
      LandingRoute::Patronus {
        gate: refused(Reason::NotOnSpace),
      },
    ),
    ("a pull request on GitHub", on_github(Some(LandingSetting::PullRequest)), merge(RouteGate::Open)),
    ("a push on GitHub", on_github(Some(LandingSetting::Push)), push(RouteGate::Open)),
    // the push URL is Space: no fact of GitHub describes it, the server answers
    ("a push on Space", on_space(Some(LandingSetting::Push)), push(RouteGate::Open)),
  ];
  for (case, remote, expected) in cases {
    assert_eq!(decide(&remote, write, rules), expected, "{case}");
  }
}

/// A push needs write access; the rules a writer can't bypass refuse it, and a maintainer or an admin confirms them;
/// unsigned copies pass no rule of signatures; the required status checks a push lands without are confirmed
#[test]
fn a_push_is_gated_by_the_write_access_and_the_rules_of_the_target() {
  let rules = |set: fn(&mut BranchRules)| {
    let mut rules = BranchRules::default();
    set(&mut rules);
    rules
  };
  let pull_request = rules(|rules| rules.requires_pull_request = true);
  let merge_queue = rules(|rules| rules.merge_queue = true);
  let restricted = rules(|rules| rules.restricts_updates = true);
  let signed = rules(|rules| {
    rules.requires_signatures = true;
    rules.requires_pull_request = true;
  });
  let checks = rules(|rules| rules.required_status_checks = true);
  let protected = rules(|rules| {
    rules.requires_pull_request = true;
    rules.restricts_updates = true;
    rules.required_status_checks = true;
    rules.dismiss_stale_reviews_on_push = true;
  });
  let none = BranchRules::default();
  let no_write = refused(Reason::NoWriteAccess {
    repository: REPOSITORY.to_owned(),
    target: target(),
  });
  let confirm = |confirmations: Vec<Reason>| RouteGate::Confirm { confirmations };
  let needs_pull_request = Reason::BaselineRequiresPullRequest { target: target() };
  let restricts = Reason::BaselineRestrictsUpdates { target: target() };
  let status_checks = Reason::BaselineRequiresStatusChecks { target: target() };
  let cases = [
    ("a reader", Read, &none, no_write.clone()),
    ("a triager", Triage, &pull_request, no_write),
    ("a writer, no rules", Write, &none, RouteGate::Open),
    ("a writer, a pull request", Write, &pull_request, refused(needs_pull_request.clone())),
    (
      "a writer, the merge queue",
      Write,
      &merge_queue,
      refused(Reason::BaselineRequiresMergeQueue { target: target() }),
    ),
    ("a writer, restricted updates", Write, &restricted, refused(restricts.clone())),
    ("a maintainer, a pull request", Maintain, &pull_request, confirm(vec![needs_pull_request.clone()])),
    (
      "an admin, every rule",
      Admin,
      &protected,
      confirm(vec![needs_pull_request.clone(), restricts, status_checks.clone()]),
    ),
    ("a writer, status checks", Write, &checks, confirm(vec![status_checks])),
    ("an admin, signatures", Admin, &signed, refused(Reason::BaselineRequiresSignatures { target: target() })),
  ];
  let remote = on_github(Some(LandingSetting::Push));
  for (case, permission, rules, expected) in cases {
    assert_eq!(decide(&remote, listed(permission), TargetRules::Known(rules)), push(expected), "{case}");
  }
  // GitHub named no permission: a rule a maintainer may bypass is asked, never refused
  let unnamed = GitHubFacts::Listed {
    repository: REPOSITORY,
    permission: None,
  };
  assert_eq!(decide(&remote, unnamed, TargetRules::Known(&pull_request)), push(confirm(vec![needs_pull_request])));
}

/// A fact without an answer waits, and so does a read that failed, with its reason: never "no"; the permission comes
/// first, so a reader waits for no rules
#[test]
fn a_push_waits_for_the_facts_of_github_and_never_reads_a_missing_one_as_no() {
  let remote = on_github(Some(LandingSetting::Push));
  let failure = ApiError::internal("GitHub rejected the token");
  let wait = |fact: RouteFact, failed: Option<&ApiError>| push(RouteGate::Wait { fact, failed: failed.cloned() });
  let none = BranchRules::default();
  assert_eq!(decide(&remote, GitHubFacts::Pending, TargetRules::Known(&none)), wait(RouteFact::Permission, None));
  assert_eq!(
    decide(&remote, GitHubFacts::Failed { reason: &failure }, TargetRules::Known(&none)),
    wait(RouteFact::Permission, Some(&failure))
  );
  assert_eq!(decide(&remote, listed(Write), TargetRules::Pending), wait(RouteFact::TargetRules, None));
  assert_eq!(
    decide(&remote, listed(Write), TargetRules::Failed { reason: &failure }),
    wait(RouteFact::TargetRules, Some(&failure))
  );
  assert!(matches!(
    decide(&remote, listed(Read), TargetRules::Pending),
    LandingRoute::Push {
      gate: RouteGate::Refused {
        reason: Reason::NoWriteAccess { .. }
      },
      ..
    }
  ));
}

/// The facts of GitHub count only for the repository the remote of the baseline pushes to: another host, a mirror, or
/// none, and the server answers; a local baseline has no remote branch to push into
#[test]
fn a_push_elsewhere_than_the_repository_of_the_facts_has_no_gate() {
  let reader = listed(Read);
  let unread = TargetRules::Pending;
  let gitlab = LandingRemote {
    push_repository: None,
    ..on_github(Some(LandingSetting::Push))
  };
  assert_eq!(decide(&gitlab, reader, unread), push(RouteGate::Open));
  let mirror = LandingRemote {
    push_repository: Some("develar/branch-deck-mirror".to_owned()),
    ..on_github(Some(LandingSetting::Push))
  };
  assert_eq!(decide(&mirror, reader, unread), push(RouteGate::Open));
  assert_eq!(decide(&on_github(Some(LandingSetting::Push)), GitHubFacts::NotGitHub, unread), push(RouteGate::Open));
  // GitHub names a repository in its own case
  let upper = GitHubFacts::Listed {
    repository: "Develar/Branch-Deck",
    permission: Some(Read),
  };
  assert!(matches!(
    decide(&on_github(Some(LandingSetting::Push)), upper, unread),
    LandingRoute::Push {
      gate: RouteGate::Refused { .. },
      ..
    }
  ));
  let local = LandingRemote {
    push_repository: None,
    target: None,
    ..on_github(Some(LandingSetting::Push))
  };
  assert_eq!(
    decide(&local, reader, unread),
    push(refused(Reason::LocalBaseline {
      baseline: "origin/master".to_owned()
    }))
  );
}

/// A merge needs write access to the repository `origin` pushes to: without it the route stays and others merge; a
/// pull request of a mirror merges nothing into the target, and off GitHub there is none
#[test]
fn a_merge_needs_write_access_to_the_repository_origin_pushes_to() {
  let remote = on_github(Some(LandingSetting::PullRequest));
  let rules = TargetRules::Pending;
  assert_eq!(decide(&remote, listed(Write), rules), merge(RouteGate::Open));
  assert_eq!(decide(&remote, listed(Admin), rules), merge(RouteGate::Open));
  assert_eq!(
    decide(&remote, listed(Triage), rules),
    merge(refused(Reason::NoWriteAccess {
      repository: REPOSITORY.to_owned(),
      target: target(),
    }))
  );
  assert_eq!(
    decide(&remote, GitHubFacts::Pending, rules),
    merge(RouteGate::Wait {
      fact: RouteFact::Permission,
      failed: None
    })
  );
  assert_eq!(decide(&remote, GitHubFacts::NotGitHub, rules), merge(refused(Reason::NotOnGitHub)));
  // the monorepo names the pull request route: its pull requests are of the GitHub mirror
  assert_eq!(
    decide(&on_space(Some(LandingSetting::PullRequest)), listed(Write), rules),
    merge(refused(Reason::GitHubMirror {
      repository: REPOSITORY.to_owned()
    }))
  );
}

/// The session reads the rules of the target only for a push into it on GitHub
#[test]
fn the_rules_of_the_target_are_read_only_for_a_push_on_github() {
  assert_eq!(rules_target(&on_github(Some(LandingSetting::Push))), Some("master"));
  assert_eq!(rules_target(&on_github(None)), None);
  assert_eq!(rules_target(&on_github(Some(LandingSetting::PullRequest))), None);
  assert_eq!(rules_target(&on_space(Some(LandingSetting::Push))), None);
}

/// The facts as the session holds them: a listing that stands names its repository and the permission, also through a
/// failed refresh; a failure before any listing, with its reason; none before GitHub's first answer. The rules: known,
/// failed, or not read yet
#[test]
fn the_facts_of_the_session_read_as_the_rule_takes_them() {
  let mut view = PullRequestsView::default();
  assert!(matches!(GitHubFacts::of(&view), GitHubFacts::Pending));
  let failure = ApiError::internal("No GitHub token");
  view.head = Fact {
    value: Some(PullRequestAvailability::SignedOut),
    state: FactState::Unavailable { reason: failure.clone() },
    seq: 1,
    observed_at: 0,
  };
  assert!(matches!(GitHubFacts::of(&view), GitHubFacts::Failed { reason } if *reason == failure));
  view.head.value = Some(PullRequestAvailability::NotGitHub);
  view.head.state = FactState::Fresh;
  assert!(matches!(GitHubFacts::of(&view), GitHubFacts::NotGitHub));
  view.repository = Some(REPOSITORY.to_owned());
  view.viewer_permission = Some(Maintain);
  view.head.state = FactState::Stale {
    reason: Some(ApiError::internal("offline")),
  };
  assert!(matches!(
    GitHubFacts::of(&view),
    GitHubFacts::Listed {
      repository: REPOSITORY,
      permission: Some(Maintain)
    }
  ));

  assert!(matches!(TargetRules::of(None), TargetRules::Pending));
  assert!(matches!(TargetRules::of(Some(&Fact::default())), TargetRules::Pending));
  let rules = Fact {
    value: Some(BranchRules::default()),
    state: FactState::Fresh,
    seq: 1,
    observed_at: 0,
  };
  assert!(matches!(TargetRules::of(Some(&rules)), TargetRules::Known(_)));
  let failed = Fact::<BranchRules> {
    value: None,
    state: FactState::Unavailable { reason: failure },
    seq: 1,
    observed_at: 0,
  };
  assert!(matches!(TargetRules::of(Some(&failed)), TargetRules::Failed { .. }));
}

/// A refusal of the server of a push into the target in this session is asked before the next push, first among what
/// the gate asks: a hook may refuse only some commits; a gate that waits or refuses keeps its answer
#[test]
fn a_refusal_of_the_server_in_this_session_is_asked_before_the_next_push() {
  let remote = on_github(Some(LandingSetting::Push));
  let checks = BranchRules {
    required_status_checks: true,
    ..BranchRules::default()
  };
  let with_refusal = |github: GitHubFacts<'_>, rules: TargetRules<'_>| {
    route(&RouteFacts {
      remote: &remote,
      baseline: "origin/master",
      github,
      rules,
      refused: Some("remote: master is protected"),
      update: None,
    })
  };
  let refused = Reason::BaselineRefusedPush {
    target: "master".to_owned(),
    message: "remote: master is protected".to_owned(),
  };
  assert_eq!(
    with_refusal(listed(Write), TargetRules::Known(&BranchRules::default())),
    push(RouteGate::Confirm {
      confirmations: vec![refused.clone()]
    })
  );
  assert_eq!(
    with_refusal(listed(Write), TargetRules::Known(&checks)),
    push(RouteGate::Confirm {
      confirmations: vec![refused, Reason::BaselineRequiresStatusChecks { target: "master".to_owned() }]
    })
  );
  assert!(matches!(
    with_refusal(listed(Read), TargetRules::Pending),
    LandingRoute::Push {
      gate: RouteGate::Refused { .. },
      ..
    }
  ));
  assert!(matches!(
    with_refusal(GitHubFacts::Pending, TargetRules::Pending),
    LandingRoute::Push { gate: RouteGate::Wait { .. }, .. }
  ));
}

/// A fetched baseline with commits the fork point of the checked out branch lacks, which the copies are built on,
/// refuses a push the gate would let go or ask about: it is no fast-forward, the update comes first; a gate that
/// refuses or waits keeps its answer, and a branch on the head of the baseline pushes
#[test]
fn a_fetched_baseline_ahead_of_the_fork_point_refuses_the_push_until_the_update() {
  let remote = on_github(Some(LandingSetting::Push));
  let check = |base: &str| UpdateCheck {
    branch: "master".to_owned(),
    upstream: "origin/master".to_owned(),
    head: "head".to_owned(),
    upstream_head: "fetched".to_owned(),
    base: base.to_owned(),
    state: UpdateState::Clean { kind: UpdateKind::Rebase },
    fetched_at: None,
    resolved_by_words: Vec::new(),
  };
  let (behind, current) = (check("fork-point"), check("fetched"));
  let checks = BranchRules {
    required_status_checks: true,
    ..BranchRules::default()
  };
  let with = |github: GitHubFacts<'_>, rules: TargetRules<'_>, update: &UpdateCheck| {
    route(&RouteFacts {
      remote: &remote,
      baseline: "origin/master",
      github,
      rules,
      refused: None,
      update: Some(update),
    })
  };
  let ahead = push(RouteGate::Refused {
    reason: Reason::BaselineAhead {
      upstream: "origin/master".to_owned(),
    },
  });
  assert_eq!(with(listed(Write), TargetRules::Known(&BranchRules::default()), &behind), ahead);
  assert_eq!(with(listed(Write), TargetRules::Known(&checks), &behind), ahead);
  assert_eq!(with(listed(Write), TargetRules::Known(&BranchRules::default()), &current), push(RouteGate::Open));
  assert_eq!(
    with(listed(Write), TargetRules::Pending, &behind),
    push(RouteGate::Wait {
      fact: RouteFact::TargetRules,
      failed: None,
    })
  );
  assert_eq!(
    with(listed(Read), TargetRules::Pending, &behind),
    push(RouteGate::Refused {
      reason: Reason::NoWriteAccess {
        repository: REPOSITORY.to_owned(),
        target: "master".to_owned(),
      }
    })
  );
}
