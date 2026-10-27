use crate::actions::{BranchFacts, Children, Problems, RobotFacts, Row, StackFacts, StackShape, branch_actions, robot_actions, stack_actions};
use crate::next_step::{Listing, PatronusAnswer};
use crate::next_step_test::landed_by_oauth;
use domain_types::actions::{ActionBlocker, ActionId, ActionState, Availability, NothingBeyond};
use domain_types::fixtures::{pull_request, remote_status, robot, robot_of};
use domain_types::next_step::{NextStep, StepBlocker, StepFact, StepKind, StepRetarget, StepRow, WithoutReview};
use domain_types::patronus::{PatronusPushMode, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestBase, PullRequestState, PullRequestView};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, RestartAction, RobotRestart};
use domain_types::session::{BranchStatus, ChildrenPush, Parked, StackBranchAlong};
use domain_types::stack::{StackBranchState, StackFix, StackProblem, StackProblemView};
use domain_types::{PushAction, RemoteStatusUpdate};
use pretty_assertions::assert_eq;

use ActionId::{
  AddIssueReference, AmendChanges, CancelRobot, CheckApprovals, CopyRobotFullPrompt, CopyRobotPrompt, CreatePullRequest, CreateStackPullRequest, DryRun, DryRunWholeStack, MoveTo,
  OpenPullRequest, OpenRobot, OpenRobotPullRequest, OpenStackPullRequest, Park, Push, PushAndRestartRobot, PushWithChildren, Rename, RequestReview, RestartRobot,
  RetargetPullRequest, ReviewChanges, ReviewWholeStack, SafePush, SafePushMainLine, SafePushWholeStack, SafePushWithProfile, SafePushWithoutReview, SquashFixups, StackOn, Unapply,
  Ungroup, Unpark, Unstack,
};

/// The actions as the table reads them: the id and why it can't run, `None` when it can
fn table(actions: &[ActionState]) -> Vec<(ActionId, Option<ActionBlocker>)> {
  actions
    .iter()
    .map(|action| {
      let reason = match &action.availability {
        Availability::Enabled => None,
        Availability::Disabled { reason } => Some(reason.clone()),
      };
      (action.id, reason)
    })
    .collect()
}

/// The reason of `id` in `actions`; panics when the action is not offered
fn reason_of(actions: &[ActionState], id: ActionId) -> Option<ActionBlocker> {
  table(actions).into_iter().find(|(it, _)| *it == id).unwrap_or_else(|| panic!("{id:?} is not offered")).1
}

fn ids(actions: &[ActionState]) -> Vec<ActionId> {
  actions.iter().map(|action| action.id).collect()
}

/// Row 11 of the next step: "Safe Push", nothing against either start
fn ready_step() -> NextStep {
  NextStep {
    row: StepRow::Eleven,
    kind: StepKind::SafePush,
    safe_push: None,
    dry_run: None,
    needs_readiness: false,
    stale_readiness: false,
    landed: None,
    retarget: None,
    without_review: None,
    lands: 1,
    unlisted: None,
  }
}

fn row(name: &str) -> Row<'_> {
  Row {
    name,
    syncing: false,
    status: BranchStatus::Unchanged,
    blocked_by: None,
    sync_failed: false,
    problem: None,
    commit_count: 2,
    parked: None,
    declared_parent: None,
    all_referenced: false,
  }
}

fn remote(action: PushAction) -> RemoteStatusUpdate {
  RemoteStatusUpdate { action, ..remote_status("auth") }
}

/// A pushed branch with work for the remote, on Space and on GitHub, without a pull request
fn facts<'a>(step: &'a NextStep, remote: &'a RemoteStatusUpdate) -> BranchFacts<'a> {
  BranchFacts {
    row: row("auth"),
    patronus: PatronusAnswer::Answers { failure: None },
    step,
    hold: None,
    hold_landed: false,
    remote: Some(remote),
    children: None,
    pull_request: None,
    open_pull_request: None,
    listing: Listing::Listed,
    repository: true,
    comment_route: None,
    stacks_enabled: true,
  }
}

const REWRITES: [ActionId; 8] = [Rename, MoveTo, StackOn, Ungroup, AmendChanges, AddIssueReference, SquashFixups, Unapply];

/// A branch ready to land offers every start, its push, the review, Create Pull Request…, the rewrites and the park,
/// all enabled; off Space no start of Patronus, before its answer each start waits for what its step waits for
#[test]
fn a_ready_branch_offers_every_action_enabled_and_off_space_no_start_of_patronus() {
  let step = ready_step();
  let status = remote(PushAction::Push);
  let enabled = |ids: &[ActionId]| ids.iter().map(|id| (*id, None)).collect::<Vec<_>>();
  assert_eq!(
    table(&branch_actions(&facts(&step, &status))),
    enabled(&[
      SafePush,
      SafePushWithProfile,
      DryRun,
      CheckApprovals,
      Push,
      ReviewChanges,
      CreatePullRequest,
      Rename,
      MoveTo,
      StackOn,
      Ungroup,
      AmendChanges,
      AddIssueReference,
      SquashFixups,
      Unapply,
      Park
    ])
  );
  let off_space = BranchFacts {
    patronus: PatronusAnswer::NotOnSpace,
    stacks_enabled: false,
    ..facts(&step, &status)
  };
  assert_eq!(
    ids(&branch_actions(&off_space)),
    [
      Push,
      ReviewChanges,
      CreatePullRequest,
      Rename,
      MoveTo,
      Ungroup,
      AmendChanges,
      AddIssueReference,
      SquashFixups,
      Unapply,
      Park
    ]
  );
  let pending = NextStep {
    row: StepRow::Zero,
    kind: StepKind::Pending { fact: StepFact::Patronus },
    safe_push: Some(StepBlocker::Step),
    dry_run: Some(StepBlocker::Step),
    ..ready_step()
  };
  let waiting = branch_actions(&BranchFacts {
    patronus: PatronusAnswer::Pending,
    ..facts(&pending, &status)
  });
  let step_blocker = Some(ActionBlocker::Step { blocker: StepBlocker::Step });
  assert_eq!(
    table(&waiting)[..4],
    [
      (SafePush, step_blocker.clone()),
      (SafePushWithProfile, step_blocker.clone()),
      (DryRun, step_blocker),
      (CheckApprovals, None)
    ]
  );
}

fn held(kind: HoldKind) -> Reason {
  Reason::Held {
    branch: "auth".to_string(),
    hold: HoldView {
      kind,
      by: robot_of("auth", "r1"),
      carried: None,
    },
    mode: PatronusPushMode::RebaseAutosquash,
    target: "master".to_string(),
  }
}

/// The push waits for what its button reads, in this order: parked, blocked, failed, no commits, held, a sync that
/// rebuilds the branch, nothing to push
#[test]
fn the_push_waits_for_parked_blocked_failed_held_a_rebuild_and_nothing_to_push_in_this_order() {
  let step = ready_step();
  let up_to_date = remote(PushAction::UpToDate);
  let base = || facts(&step, &up_to_date);
  let parked = Parked {
    note: String::new(),
    with: Some("core".to_string()),
  };
  let parked_reason = Reason::Parked {
    branch: "auth".to_string(),
    with: Some("core".to_string()),
    note: String::new(),
  };
  let blocked = Row {
    status: BranchStatus::Blocked,
    blocked_by: Some("core"),
    ..row("auth")
  };
  let own = held(HoldKind::Own);
  let cases: Vec<(&str, BranchFacts<'_>, Option<ActionBlocker>)> = vec![
    ("up to date", base(), Some(ActionBlocker::UpToDate)),
    (
      "the sync rebuilds it",
      BranchFacts {
        row: Row { syncing: true, ..row("auth") },
        ..base()
      },
      Some(ActionBlocker::Syncing),
    ),
    (
      "held",
      BranchFacts {
        row: Row { syncing: true, ..row("auth") },
        hold: Some(&own),
        ..base()
      },
      Some(ActionBlocker::Rule { reason: own.clone() }),
    ),
    (
      "no commits",
      BranchFacts {
        row: Row { commit_count: 0, ..row("auth") },
        hold: Some(&own),
        ..base()
      },
      Some(ActionBlocker::NoCommits),
    ),
    (
      "failed to sync",
      BranchFacts {
        row: Row { sync_failed: true, ..row("auth") },
        hold: Some(&own),
        ..base()
      },
      Some(ActionBlocker::SyncFailed { problem: None }),
    ),
    (
      "blocked",
      BranchFacts {
        row: Row { sync_failed: true, ..blocked },
        ..base()
      },
      Some(ActionBlocker::Rule {
        reason: Reason::Blocked {
          branch: "auth".to_string(),
          by: Some("core".to_string()),
        },
      }),
    ),
    (
      "parked with its parent",
      BranchFacts {
        row: Row { parked: Some(&parked), ..blocked },
        ..base()
      },
      Some(ActionBlocker::Rule { reason: parked_reason }),
    ),
  ];
  for (case, facts, expected) in cases {
    assert_eq!(reason_of(&branch_actions(&facts), Push), expected, "{case}");
  }
  let created = BranchFacts { remote: None, ..base() };
  assert_eq!(reason_of(&branch_actions(&created), Push), None, "no remote status yet: the push creates the branch");
}

/// Pick Remote Commits is offered only while the remote has commits the branch lacks, enabled unless the branch is
/// parked, blocked, held or rebuilt by a sync
#[test]
fn pick_remote_commits_is_offered_while_the_remote_has_commits_the_branch_lacks() {
  let step = ready_step();
  let pushed = remote(PushAction::Push);
  assert!(!ids(&branch_actions(&facts(&step, &pushed))).contains(&ActionId::PickRemoteCommits), "nothing to take in");
  let theirs = RemoteStatusUpdate {
    commits_behind: 1,
    others_behind: 1,
    ..remote(PushAction::Force { drops: true, replaces: false })
  };
  let base = || facts(&step, &theirs);
  let own = held(HoldKind::Own);
  let cases: Vec<(&str, BranchFacts<'_>, Option<ActionBlocker>)> = vec![
    ("ready", base(), None),
    (
      "the sync rebuilds it",
      BranchFacts {
        row: Row { syncing: true, ..row("auth") },
        ..base()
      },
      Some(ActionBlocker::Syncing),
    ),
    ("held", BranchFacts { hold: Some(&own), ..base() }, Some(ActionBlocker::Rule { reason: own.clone() })),
  ];
  for (case, facts, expected) in cases {
    assert_eq!(reason_of(&branch_actions(&facts), ActionId::PickRemoteCommits), expected, "{case}");
  }
}

fn problem(fixes: Vec<StackFix>) -> StackProblemView {
  StackProblemView {
    problem: StackProblem::Cycle(vec!["auth".to_string(), "core".to_string()]),
    label: "stack cycle".to_string(),
    reason: "auth is in a stack cycle".to_string(),
    message: String::new(),
    commits: Vec::new(),
    fixes,
  }
}

/// A rewrite needs commits, a build and no sync that rebuilds the branch; the fixes of a stack problem run while the problem blocks
/// the rest, a sync stops them too; Add Issue Reference says when every commit has one; Unstack only of a stacked branch
/// or where a fix offers it
#[test]
fn a_rewrite_needs_commits_a_build_and_no_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest() {
  let step = ready_step();
  let status = remote(PushAction::Push);
  let cycle = problem(vec![StackFix::StackOn("main".to_string()), StackFix::Unstack]);
  let failed = BranchFacts {
    row: Row {
      sync_failed: true,
      problem: Some(&cycle),
      ..row("auth")
    },
    ..facts(&step, &status)
  };
  let actions = branch_actions(&failed);
  let failure = Some(ActionBlocker::SyncFailed {
    problem: Some("auth is in a stack cycle".to_string()),
  });
  for id in [Rename, MoveTo, Ungroup, AmendChanges, AddIssueReference, SquashFixups, Unapply] {
    assert_eq!(reason_of(&actions, id), failure, "{id:?}");
  }
  assert_eq!((reason_of(&actions, StackOn), reason_of(&actions, Unstack)), (None, None), "the fixes of the problem run");
  let rebuilt = branch_actions(&BranchFacts {
    row: Row {
      syncing: true,
      sync_failed: true,
      problem: Some(&cycle),
      ..row("auth")
    },
    ..facts(&step, &status)
  });
  assert_eq!((reason_of(&rebuilt, StackOn), reason_of(&rebuilt, Rename)), (Some(ActionBlocker::Syncing), failure));

  let syncing = branch_actions(&BranchFacts {
    row: Row { syncing: true, ..row("auth") },
    ..facts(&step, &status)
  });
  for id in REWRITES {
    assert_eq!(reason_of(&syncing, id), Some(ActionBlocker::Syncing), "{id:?} waits for the sync that rebuilds the branch");
  }
  let empty = branch_actions(&BranchFacts {
    row: Row { commit_count: 0, ..row("auth") },
    ..facts(&step, &status)
  });
  assert_eq!(reason_of(&empty, AmendChanges), Some(ActionBlocker::NoCommits));
  let referenced = branch_actions(&BranchFacts {
    row: Row {
      all_referenced: true,
      declared_parent: Some("core"),
      ..row("auth")
    },
    ..facts(&step, &status)
  });
  assert_eq!(reason_of(&referenced, AddIssueReference), Some(ActionBlocker::AllReferenced));
  assert_eq!(reason_of(&referenced, Unstack), None, "a stacked branch unstacks");
  assert!(!ids(&branch_actions(&facts(&step, &status))).contains(&Unstack), "a root has nothing to unstack from");
}

/// A park waits for a robot that lands the branch (a merged one landed it: the park goes), its commits and the sync; a
/// parked branch edits its note and unparks, one parked with its parent only unparks the parent
#[test]
fn a_park_waits_for_a_robot_that_lands_the_branch_and_a_parked_branch_unparks() {
  let step = ready_step();
  let status = remote(PushAction::Push);
  let own = held(HoldKind::Own);
  let landed = held(HoldKind::Landed);
  let park = |facts: BranchFacts<'_>| reason_of(&branch_actions(&facts), Park);
  assert_eq!(
    park(BranchFacts {
      hold: Some(&own),
      ..facts(&step, &status)
    }),
    Some(ActionBlocker::ParkHeld { reason: own.clone() })
  );
  assert_eq!(
    park(BranchFacts {
      row: Row { syncing: true, ..row("auth") },
      hold: Some(&landed),
      hold_landed: true,
      ..facts(&step, &status)
    }),
    Some(ActionBlocker::Syncing)
  );
  let note = Parked {
    note: "after the release".to_string(),
    with: None,
  };
  let parked = branch_actions(&BranchFacts {
    row: Row {
      parked: Some(&note),
      ..row("auth")
    },
    ..facts(&step, &status)
  });
  assert_eq!(table(&parked)[table(&parked).len() - 2..], [(Park, None), (Unpark, None)]);
  let with = Parked {
    note: String::new(),
    with: Some("core".to_string()),
  };
  let carried = branch_actions(&BranchFacts {
    row: Row {
      parked: Some(&with),
      syncing: true,
      ..row("auth")
    },
    ..facts(&step, &status)
  });
  assert_eq!(table(&carried).last(), Some(&(Unpark, Some(ActionBlocker::Syncing))));
  assert!(!ids(&carried).contains(&Park));
}

fn view_of(pull_request: &PullRequest, draft: bool) -> PullRequestView {
  PullRequestView {
    number: pull_request.number,
    base: PullRequestBase::Target,
    draft,
    candidates: Vec::new(),
  }
}

/// The review opens a built branch with commits; Open Pull Request of a branch with one, any state; Request Review… of
/// an open one, waiting for a draft; Create Pull Request… of a branch without a live one on GitHub, waiting for the
/// listing and refused once all of it landed; the retarget the step offers; the profile a comment can't choose; the
/// start without review of rows 7 and 11
#[test]
fn the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts() {
  let status = remote(PushAction::Push);
  let step = NextStep {
    retarget: Some(StepRetarget {
      number: 7,
      base: "user/virtual/core".to_string(),
      target: "master".to_string(),
    }),
    without_review: Some(WithoutReview {
      blocker: Some(StepBlocker::Syncing),
    }),
    ..ready_step()
  };
  let open = pull_request("auth", 7);
  let draft = view_of(&open, true);
  let actions = branch_actions(&BranchFacts {
    pull_request: Some(&open),
    open_pull_request: Some(&draft),
    comment_route: Some(7),
    ..facts(&step, &status)
  });
  assert_eq!(
    table(&actions)[..10],
    [
      (SafePush, None),
      (SafePushWithProfile, Some(ActionBlocker::CommentRoute { number: 7 })),
      (DryRun, None),
      (CheckApprovals, None),
      (SafePushWithoutReview, Some(ActionBlocker::Step { blocker: StepBlocker::Syncing })),
      (Push, None),
      (ReviewChanges, None),
      (OpenPullRequest, None),
      (RequestReview, Some(ActionBlocker::Draft { number: 7 })),
      (RetargetPullRequest, None),
    ]
  );
  assert!(!ids(&actions).contains(&CreatePullRequest), "a branch with an open pull request gets no second one");

  let ready = ready_step();
  let create = |facts: BranchFacts<'_>| {
    table(&branch_actions(&facts))
      .into_iter()
      .find(|(id, _)| *id == CreatePullRequest)
      .map(|(_, reason)| reason)
  };
  let closed = PullRequest {
    state: PullRequestState::Closed,
    ..pull_request("auth", 3)
  };
  assert_eq!(
    create(BranchFacts {
      pull_request: Some(&closed),
      ..facts(&ready, &status)
    }),
    Some(None),
    "beside a closed one"
  );
  assert_eq!(
    create(BranchFacts {
      listing: Listing::Loading,
      ..facts(&ready, &status)
    }),
    Some(Some(ActionBlocker::Pending { fact: StepFact::PullRequests }))
  );
  let landed = NextStep {
    landed: Some(landed_by_oauth()),
    ..ready_step()
  };
  assert_eq!(create(facts(&landed, &status)), Some(Some(ActionBlocker::Landed { landed: landed_by_oauth() })));
  assert_eq!(
    create(BranchFacts {
      repository: false,
      ..facts(&ready, &status)
    }),
    None,
    "without the GitHub repository of the branches"
  );

  let review = |row: Row<'static>| reason_of(&branch_actions(&BranchFacts { row, ..facts(&ready, &status) }), ReviewChanges);
  assert_eq!(review(Row { commit_count: 0, ..row("auth") }), Some(ActionBlocker::NoCommits));
  assert_eq!(review(Row { sync_failed: true, ..row("auth") }), Some(ActionBlocker::SyncFailed { problem: None }));
}

fn children(push: &ChildrenPush) -> Children<'_> {
  Children {
    push,
    statuses_pending: false,
    landed: None,
  }
}

/// Push with Children pushes what Rust orders while it pushes more than the branch; else it says what it lacks: the
/// children that ask first, only the branch's own commits, landed whole, the stack branch that stays, all on the remote;
/// parked and a running Safe Push refuse it, a merged one leaves the children to push; a running sync's statuses are to
/// come
#[test]
fn push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks() {
  let step = ready_step();
  let status = remote(PushAction::UpToDate);
  let own = held(HoldKind::Own);
  let landed = held(HoldKind::Landed);
  let with = |push| BranchFacts {
    children: Some(children(push)),
    ..facts(&step, &status)
  };
  let reason = |facts: BranchFacts<'_>| reason_of(&branch_actions(&facts), PushWithChildren);
  let offers = ChildrenPush {
    offers: true,
    ..ChildrenPush::default()
  };
  assert_eq!(reason(with(&offers)), None);
  assert_eq!(
    reason(BranchFacts {
      hold: Some(&own),
      ..with(&offers)
    }),
    Some(ActionBlocker::Rule { reason: own.clone() })
  );
  assert_eq!(
    reason(BranchFacts {
      hold: Some(&landed),
      hold_landed: true,
      ..with(&offers)
    }),
    None
  );
  let nothing = |why: NothingBeyond| Some(ActionBlocker::NothingBeyond { why });
  let ask_first = ChildrenPush {
    ask_first: vec!["login".to_string()],
    order: Some(vec!["auth".to_string()]),
    ..ChildrenPush::default()
  };
  assert_eq!(reason(with(&ask_first)), nothing(NothingBeyond::AskFirst { names: vec!["login".to_string()] }));
  let only = ChildrenPush {
    order: Some(vec!["auth".to_string()]),
    ..ChildrenPush::default()
  };
  assert_eq!(reason(with(&only)), nothing(NothingBeyond::OnlyBranch));
  let on_remote = ChildrenPush {
    order: Some(Vec::new()),
    ..ChildrenPush::default()
  };
  assert_eq!(reason(with(&on_remote)), nothing(NothingBeyond::OnRemote));
  let unpublished = ChildrenPush {
    stack_branch: Some(StackBranchAlong::Unpublished),
    ..on_remote.clone()
  };
  assert_eq!(
    reason(with(&unpublished)),
    nothing(NothingBeyond::StackBranch {
      along: StackBranchAlong::Unpublished
    })
  );
  let landed_whole = NothingBeyond::Landed {
    by: vec![robot_of("oauth", "r9")],
    own: false,
    target: "master".to_string(),
  };
  let landed_children = || BranchFacts {
    children: Some(Children {
      landed: Some(landed_whole.clone()),
      ..children(&on_remote)
    }),
    ..facts(&step, &status)
  };
  assert_eq!(reason_of(&branch_actions(&landed_children()), PushWithChildren), nothing(landed_whole.clone()));
  let pending = BranchFacts {
    children: Some(Children {
      statuses_pending: true,
      ..children(&on_remote)
    }),
    ..facts(&step, &status)
  };
  assert_eq!(reason_of(&branch_actions(&pending), PushWithChildren), Some(ActionBlocker::Syncing));
  let note = Parked { note: String::new(), with: None };
  let parked = BranchFacts {
    row: Row {
      parked: Some(&note),
      ..row("auth")
    },
    ..landed_children()
  };
  assert!(matches!(
    reason_of(&branch_actions(&parked), PushWithChildren),
    Some(ActionBlocker::Rule { reason: Reason::Parked { .. } })
  ));
}

fn stack_facts<'a>(shape: StackShape<'a>, step: Option<&'a NextStep>) -> StackFacts<'a> {
  StackFacts {
    root: "air",
    patronus: PatronusAnswer::Answers { failure: None },
    shape,
    step,
    pull_request: None,
    open_pull_request: None,
    repository: true,
    landed: None,
  }
}

/// The whole stack starts as its step says; while the sync didn't build its stack branch it lands in pieces (the Safe
/// Push of its main line) and neither its pull request nor its review can be made; a chain reviews its last branch
/// from the root, a stack of one branch has no whole review; the pull request of the stack opens while it is not closed
#[test]
fn the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built() {
  let step = ready_step();
  let leaf = NextStep {
    safe_push: Some(StepBlocker::Syncing),
    ..ready_step()
  };
  let not_built = Reason::StackNotBuilt {
    root: "air".to_string(),
    state: StackBranchState::Blocked,
    blocked_by: Some("deploy".to_string()),
    conflict: None,
    error: None,
  };
  let forking = StackShape::Forking {
    not_built: Some(&not_built),
    tip: false,
    main_line: Some(&leaf),
  };
  let rule = Some(ActionBlocker::Rule { reason: not_built.clone() });
  assert_eq!(
    table(&stack_actions(&stack_facts(forking, Some(&step)))),
    [
      (SafePushWholeStack, None),
      (DryRunWholeStack, None),
      (SafePushMainLine, Some(ActionBlocker::Step { blocker: StepBlocker::Syncing })),
      (CreateStackPullRequest, rule.clone()),
      (ReviewWholeStack, rule),
    ]
  );
  let built = StackShape::Forking {
    not_built: None,
    tip: true,
    main_line: Some(&leaf),
  };
  let merged = PullRequest {
    state: PullRequestState::Merged,
    ..pull_request("stack/air", 12)
  };
  let open = view_of(&merged, false);
  let with_pull_request = StackFacts {
    pull_request: Some(&merged),
    open_pull_request: Some(&open),
    patronus: PatronusAnswer::NotOnSpace,
    ..stack_facts(built, Some(&step))
  };
  assert_eq!(
    table(&stack_actions(&with_pull_request)),
    [(OpenStackPullRequest, None), (RequestReview, None), (ReviewWholeStack, None)]
  );
  let landed = landed_by_oauth();
  let landed_stack = StackFacts {
    landed: Some(&landed),
    ..stack_facts(built, None)
  };
  assert_eq!(
    table(&stack_actions(&landed_stack))[0],
    (CreateStackPullRequest, Some(ActionBlocker::Landed { landed: landed.clone() }))
  );
  let unsynced = StackShape::Forking {
    not_built: None,
    tip: false,
    main_line: None,
  };
  assert!(matches!(
    reason_of(&stack_actions(&stack_facts(unsynced, None)), ReviewWholeStack),
    Some(ActionBlocker::Rule {
      reason: Reason::StackNotBuilt {
        state: StackBranchState::Built,
        ..
      }
    })
  ));
  let blocked = ActionBlocker::NoCommits;
  let chain = StackShape::Chain {
    length: 2,
    review: Some(&blocked),
  };
  assert_eq!(table(&stack_actions(&stack_facts(chain, Some(&leaf))))[2], (ReviewWholeStack, Some(blocked.clone())));
  let lone = StackShape::Chain { length: 1, review: None };
  assert!(!ids(&stack_actions(&stack_facts(lone, None))).contains(&ReviewWholeStack));
}

fn restart(action: RestartAction, refusal: Option<Reason>) -> RobotRestart {
  RobotRestart { action, refusal }
}

fn robot_facts<'a>(robot: &'a domain_types::patronus::PatronusRobot, restart: Option<&'a RobotRestart>) -> RobotFacts<'a> {
  RobotFacts {
    robot,
    restart,
    pushes: true,
    needs_review: Some(false),
    failing: false,
    problems: Problems::Known,
    step: None,
  }
}

/// A failed robot restarts and copies its prompt once Patronus tells why it failed (a failed answer says so); its
/// restart waits for the approvals its step still misses, then for what refuses a start; a running one cancels, and
/// offers the prompt while a required check fails; a push and restart needs a subject to push
#[test]
fn a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count() {
  let failed = robot("r1", Some("auth"), PatronusRobotStatus::Failure, 1);
  let again = restart(RestartAction::Restart, None);
  assert_eq!(
    table(&robot_actions(&robot_facts(&failed, Some(&again)))),
    [(RestartRobot, None), (CopyRobotPrompt, None), (CopyRobotFullPrompt, None), (OpenRobot, None)]
  );
  let pending = Some(ActionBlocker::Pending { fact: StepFact::RobotProblems });
  assert_eq!(
    table(&robot_actions(&RobotFacts {
      problems: Problems::Pending,
      needs_review: None,
      ..robot_facts(&failed, Some(&again))
    })),
    [
      (RestartRobot, pending.clone()),
      (CopyRobotPrompt, pending.clone()),
      (CopyRobotFullPrompt, pending),
      (OpenRobot, None)
    ]
  );
  let unknown = Some(ActionBlocker::ProblemsFailed { message: "502".to_string() });
  assert_eq!(
    reason_of(
      &robot_actions(&RobotFacts {
        problems: Problems::Failed("502"),
        ..robot_facts(&failed, Some(&again))
      }),
      RestartRobot
    ),
    unknown
  );
  let awaiting = NextStep {
    row: StepRow::Ten,
    kind: StepKind::Awaiting { number: 7, groups: Vec::new() },
    safe_push: Some(StepBlocker::Step),
    ..ready_step()
  };
  let needs_review = robot_actions(&RobotFacts {
    needs_review: Some(true),
    step: Some(&awaiting),
    ..robot_facts(&failed, Some(&again))
  });
  assert_eq!(
    table(&needs_review),
    [(RestartRobot, Some(ActionBlocker::Step { blocker: StepBlocker::Step })), (OpenRobot, None)]
  );
  let refused = restart(RestartAction::Restart, Some(held(HoldKind::Landed)));
  assert_eq!(
    reason_of(&robot_actions(&robot_facts(&failed, Some(&refused))), RestartRobot),
    Some(ActionBlocker::Rule { reason: held(HoldKind::Landed) })
  );

  let running = domain_types::patronus::PatronusRobot {
    pull_request: Some(domain_types::patronus::PatronusPullRequestLink {
      number: 7,
      url: "https://github.com/acme/monorepo/pull/7".to_string(),
    }),
    ..robot("r2", Some("auth"), PatronusRobotStatus::Running, 1)
  };
  let push_and_restart = restart(RestartAction::PushAndRestart, None);
  assert_eq!(
    ids(&robot_actions(&RobotFacts {
      failing: true,
      ..robot_facts(&running, Some(&push_and_restart))
    })),
    [PushAndRestartRobot, CancelRobot, CopyRobotPrompt, CopyRobotFullPrompt, OpenRobot, OpenRobotPullRequest]
  );
  assert_eq!(
    ids(&robot_actions(&RobotFacts {
      pushes: false,
      ..robot_facts(&running, Some(&push_and_restart))
    })),
    [CancelRobot, OpenRobot, OpenRobotPullRequest]
  );
}

/// A pull request of someone else into master, not approved, Patronus pushing into master
fn pull_request_to_review() -> crate::actions::PullRequestFacts<'static> {
  crate::actions::PullRequestFacts {
    number: 101,
    head: "head",
    base: "master",
    own: false,
    linked: false,
    branch: None,
    approved: None,
    target: crate::actions::Target::Branch("master"),
  }
}

#[test]
fn a_pull_request_to_review_offers_the_review_the_decisions_and_its_safe_push_into_the_target() {
  use crate::actions::{Target, review_actions};
  use ActionId::{Approve, RemoveFromReviews, RequestChanges, Review, SafePushPullRequest, ShowBranch};
  let facts = pull_request_to_review();
  assert_eq!(
    table(&review_actions(&facts)),
    vec![
      (Review, None),
      (Approve, None),
      (SafePushPullRequest, None),
      (RequestChanges, None),
      (OpenPullRequest, None)
    ]
  );
  // a link can be removed; an own pull request of the table shows its branch
  let linked = review_actions(&crate::actions::PullRequestFacts { linked: true, ..facts });
  assert_eq!(ids(&linked).last(), Some(&RemoveFromReviews));
  let own = review_actions(&crate::actions::PullRequestFacts {
    own: true,
    branch: Some("auth"),
    ..facts
  });
  assert_eq!(
    table(&own),
    vec![
      (Review, None),
      (Approve, Some(ActionBlocker::OwnPullRequest)),
      (SafePushPullRequest, Some(ActionBlocker::OwnPullRequest)),
      (RequestChanges, Some(ActionBlocker::OwnPullRequest)),
      (OpenPullRequest, None),
      (ShowBranch, None),
    ]
  );
  // where Patronus doesn't merge, no Safe Push
  let off_patronus = review_actions(&crate::actions::PullRequestFacts {
    target: Target::NoPatronus,
    ..facts
  });
  assert!(!ids(&off_patronus).contains(&SafePushPullRequest));
}

#[test]
fn an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into() {
  use crate::actions::{PullRequestFacts, Target, review_actions};
  use ActionId::{Approve, SafePushPullRequest};
  let facts = pull_request_to_review();
  let approved = review_actions(&PullRequestFacts { approved: Some("head"), ..facts });
  assert_eq!(reason_of(&approved, Approve), Some(ActionBlocker::Approved { commit: "head".to_owned() }));
  assert_eq!(reason_of(&approved, SafePushPullRequest), None);
  // an approval of an older commit approves nothing of the head
  let older = review_actions(&PullRequestFacts { approved: Some("older"), ..facts });
  assert_eq!(reason_of(&older, Approve), None);
  let safe_push = |target| reason_of(&review_actions(&PullRequestFacts { target, ..facts }), SafePushPullRequest);
  assert_eq!(safe_push(Target::Unread), Some(ActionBlocker::TargetUnread));
  assert_eq!(safe_push(Target::None), Some(ActionBlocker::NoTarget));
  assert_eq!(
    safe_push(Target::Branch("release/253")),
    Some(ActionBlocker::OtherBase {
      number: 101,
      base: "master".to_owned(),
      target: "release/253".to_owned(),
    })
  );
}
