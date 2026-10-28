use crate::actions::{BranchFacts, Children, Problems, RobotFacts, Row, StackFacts, StackShape, branch_actions, on_cached_frame, robot_actions, stack_actions, start_actions};
use crate::next_step::{Listing, PatronusAnswer};
use crate::next_step_test::landed_by_oauth;
use domain_types::actions::{ActionBlocker, ActionId, ActionState, Availability, NothingBeyond};
use domain_types::fixtures::{pull_request, remote_status, robot, robot_of};
use domain_types::next_step::{NextStep, StepBlocker, StepFact, StepKind, StepRetarget, StepRow, WithoutReview};
use domain_types::patronus::{PatronusPushMode, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestBase, PullRequestState, PullRequestView};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, RestartAction, RobotRestart};
use domain_types::session::{BranchStatus, ChildrenPush, FactState, Parked, StackBranchAlong};
use domain_types::stack::{StackBranchState, StackFix, StackProblem, StackProblemView};
use domain_types::{PushAction, RemoteStatusUpdate};
use pretty_assertions::assert_eq;

use ActionId::{
  AddIssueReference, AmendChanges, CancelAutoMerge, CancelRobot, CheckApprovals, CopyRobotFullPrompt, CopyRobotPrompt, CreatePullRequest, CreateStackPullRequest, DryRun,
  DryRunWholeStack, ForgetStart, MergePullRequest, MoveTo, OpenPullRequest, OpenRobot, OpenRobotPullRequest, OpenStackPullRequest, OpenUnknownStart, Park, Push,
  PushAndRestartRobot, PushToBaseline, PushWithChildren, Rename, RequestReview, RestartRobot, RetargetPullRequest, ReviewChanges, ReviewWholeStack, SafePush, SafePushMainLine,
  SafePushWholeStack, SafePushWithProfile, SafePushWithoutReview, SquashFixups, StackOn, Unapply, Ungroup, Unpark, Unstack,
};

/// The actions as the table reads them: the id and why it can't run, `None` when it can
fn table(actions: &[ActionState]) -> Vec<(ActionId, Option<ActionBlocker>)> {
  actions
    .iter()
    .map(|action| {
      let reason = match &action.availability {
        Availability::Enabled => None,
        Availability::Disabled { reason } => Some(reason.clone()),
        operation => panic!("no operation runs, yet {:?} is {operation:?}", action.id),
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
    push_route: false,
    merge_route: false,
    cached: false,
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
/// parked, blocked or held; a sync that rebuilds it holds the pick back no more than another rewrite
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
      None,
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

/// A rewrite needs commits and a build, and no sync holds it back (the session admits it at once, C14); the fixes of a
/// stack problem run while the problem blocks the rest; Add Issue Reference says when every commit has one; Unstack only
/// of a stacked branch or where a fix offers it
#[test]
fn a_rewrite_needs_commits_and_a_build_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest() {
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
  assert_eq!((reason_of(&rebuilt, StackOn), reason_of(&rebuilt, Rename)), (None, failure));

  let syncing = branch_actions(&BranchFacts {
    row: Row { syncing: true, ..row("auth") },
    ..facts(&step, &status)
  });
  for id in REWRITES {
    assert_eq!(reason_of(&syncing, id), None, "{id:?} goes to the session during the sync that rebuilds the branch");
  }
  assert_eq!(reason_of(&syncing, Push), Some(ActionBlocker::Syncing), "the push waits for the build");
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

/// A park waits for a robot that lands the branch (a merged one landed it: the park goes) and its commits, never for a
/// sync; a parked branch edits its note and unparks, one parked with its parent only unparks the parent
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
    None
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
  assert_eq!(table(&carried).last(), Some(&(Unpark, None)));
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
    push_route: false,
    merge_route: false,
    cached: false,
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
    cached: false,
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
    cached: false,
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

/// A start whose answer was lost offers Open in Patronus where its robot would be, and Forget This Start, both enabled;
/// without a place to look, only the forget; an operation that runs offers neither
#[test]
fn a_start_whose_answer_was_lost_offers_open_in_patronus_and_forget_this_start() {
  use domain_types::operation::{OperationId, OperationStep, Origin, RunningOperation};
  let operation = |step: OperationStep| RunningOperation {
    id: OperationId("lost".to_string()),
    intent: domain_types::fixtures::land_intent("auth", PatronusPushMode::RebaseAutosquash),
    resources: Vec::new(),
    step,
    origin: Origin::OtherProcess {
      tool: domain_types::error::HolderTool::Bd,
      pid: 4242,
    },
    moves: Vec::new(),
    joined: 0,
  };
  let unknown = |url: Option<&str>| {
    operation(OperationStep::StartUnknown {
      sent_at: 1_800_000_000,
      url: url.map(ToString::to_string),
      pull_request: None,
    })
  };
  assert_eq!(
    table(&start_actions(&unknown(Some("https://patronus.example")))),
    [(OpenUnknownStart, None), (ForgetStart, None)]
  );
  assert_eq!(table(&start_actions(&unknown(None))), [(ForgetStart, None)]);
  assert_eq!(table(&start_actions(&operation(OperationStep::Starting))), []);
}

/// An operation of the session as the view has it
fn operation(
  id: &str,
  intent: domain_types::operation::Intent,
  resources: Vec<domain_types::operation::Resource>,
  step: domain_types::operation::OperationStep,
) -> domain_types::operation::RunningOperation {
  domain_types::operation::RunningOperation {
    id: domain_types::operation::OperationId(id.to_string()),
    intent,
    resources,
    step,
    origin: domain_types::operation::Origin::ThisProcess,
    moves: Vec::new(),
    joined: 0,
  }
}

/// The rewrite `rewrite` of the engine, holding HEAD
fn rewrite_operation(id: &str, rewrite: domain_types::rewrite::RewriteIntent, step: domain_types::operation::OperationStep) -> domain_types::operation::RunningOperation {
  operation(
    id,
    domain_types::operation::Intent::Rewrite(Box::new(rewrite)),
    vec![domain_types::operation::Resource::Head],
    step,
  )
}

fn op(id: &str) -> domain_types::operation::OperationId {
  domain_types::operation::OperationId(id.to_string())
}

/// The operations of the session lie over the actions of a branch: the item of its own intent runs (or waits in the queue
/// behind the one its step names), every other rewrite that can run waits behind the newest operation that holds HEAD
/// (its start queues, never refused), a push whose ref an operation holds is busy, and an item disabled for its own
/// reason keeps it; a menu item of a move names no commits, so it only waits
#[test]
fn the_operations_of_the_session_lie_over_the_actions_its_own_runs_a_rewrite_waits_a_push_behind_its_ref_is_busy() {
  use crate::actions::{Of, with_operations};
  use domain_types::operation::{OperationStep, Origin, Resource};
  use domain_types::rewrite::{RewriteIntent, TableWitness};
  let step = ready_step();
  let status = remote(PushAction::Push);
  let offered = || {
    branch_actions(&BranchFacts {
      row: Row {
        declared_parent: Some("core"),
        ..row("auth")
      },
      ..facts(&step, &status)
    })
  };
  let auth = Of::Branch {
    name: "auth",
    pushes: Some("user/virtual/auth"),
    unparks: "auth",
    pull_request: Some(7),
  };
  let unstack = |branch: &str| RewriteIntent::Unstack {
    branch_name: branch.to_string(),
    witness: TableWitness::default(),
  };
  let availability = |actions: &[ActionState], id: ActionId| {
    actions
      .iter()
      .find(|action| action.id == id)
      .unwrap_or_else(|| panic!("{id:?} is not offered"))
      .availability
      .clone()
  };

  assert_eq!(with_operations(offered(), auth, &[]), offered(), "nothing runs: as the facts offer them");

  let running = [rewrite_operation("u1", unstack("auth"), OperationStep::Rewriting)];
  let actions = with_operations(offered(), auth, &running);
  assert_eq!(
    availability(&actions, Unstack),
    Availability::Running { op: op("u1") },
    "its own intent runs: a start joins it"
  );
  for id in [Rename, MoveTo, StackOn, Ungroup, AmendChanges, AddIssueReference, SquashFixups, Unapply, Park] {
    assert_eq!(availability(&actions, id), Availability::Waits { op: op("u1") }, "{id:?} queues behind the unstack");
  }
  for id in [SafePush, DryRun, Push, ReviewChanges, CreatePullRequest] {
    assert_eq!(availability(&actions, id), Availability::Enabled, "{id:?} runs beside a rewrite");
  }

  // a second rewrite queued behind the first: its own item waits in the queue, the others behind the newest
  let queued = [
    rewrite_operation("u1", unstack("login"), OperationStep::Rewriting),
    rewrite_operation("u2", unstack("auth"), OperationStep::Queued { behind: op("u1") }),
  ];
  let actions = with_operations(offered(), auth, &queued);
  assert_eq!(availability(&actions, Unstack), Availability::Queued { op: op("u2"), behind: op("u1") });
  assert_eq!(availability(&actions, Rename), Availability::Waits { op: op("u2") });

  // a push of a stack that sends auth holds its ref: the push of auth waits for it, busy; its own push runs
  let stack_push = [operation(
    "p1",
    domain_types::fixtures::push_intent("login"),
    vec![
      Resource::Ref {
        name: "user/virtual/auth".to_string(),
      },
      Resource::Ref {
        name: "user/virtual/login".to_string(),
      },
    ],
    OperationStep::Pushing,
  )];
  let actions = with_operations(offered(), auth, &stack_push);
  assert_eq!(
    availability(&actions, Push),
    Availability::Disabled {
      reason: ActionBlocker::Busy { op: op("p1") }
    }
  );
  assert_eq!(availability(&actions, Rename), Availability::Enabled, "a rewrite runs beside a push");
  let own_push = [operation(
    "p2",
    domain_types::fixtures::push_intent("auth"),
    vec![Resource::Ref {
      name: "user/virtual/auth".to_string(),
    }],
    OperationStep::Pushing,
  )];
  assert_eq!(availability(&with_operations(offered(), auth, &own_push), Push), Availability::Running { op: op("p2") });

  // a review request that pushes first holds the base of the pull request too: the retarget waits for it
  let review = [operation(
    "r1",
    domain_types::operation::Intent::RequestReview {
      subject: domain_types::landing::Subject::branch("auth"),
      reviewers: Vec::new(),
      push_first: true,
    },
    vec![Resource::PullRequest {
      number: 7,
      field: domain_types::operation::PullRequestField::Reviewers,
    }],
    OperationStep::Writing,
  )];
  let retarget = NextStep {
    retarget: Some(StepRetarget {
      number: 7,
      base: "user/virtual/core".to_string(),
      target: "master".to_string(),
    }),
    ..ready_step()
  };
  let with_review = BranchFacts {
    step: &retarget,
    ..facts(&step, &status)
  };
  let actions = with_operations(branch_actions(&with_review), auth, &review);
  assert_eq!(availability(&actions, RetargetPullRequest), Availability::Enabled, "another field of the pull request");

  // a push of another process holds the ref: busy, never joined; an item disabled for its own reason keeps it
  let foreign = [domain_types::operation::RunningOperation {
    origin: Origin::OtherProcess {
      tool: domain_types::error::HolderTool::Bd,
      pid: 4242,
    },
    ..own_push[0].clone()
  }];
  assert_eq!(
    availability(&with_operations(offered(), auth, &foreign), Push),
    Availability::Disabled {
      reason: ActionBlocker::Busy { op: op("p2") }
    }
  );
  let empty = branch_actions(&BranchFacts {
    row: Row { commit_count: 0, ..row("auth") },
    ..facts(&step, &status)
  });
  assert_eq!(
    availability(&with_operations(empty, auth, &running), Rename),
    Availability::Disabled { reason: ActionBlocker::NoCommits }
  );
}

/// The restart and the cancel of a robot run in the session: the item of the one that runs is running, the other waits
/// for it, busy; Forget This Start runs while its forget does
#[test]
fn the_restart_and_the_cancel_of_a_robot_run_once_and_hold_each_other_back() {
  use crate::actions::{Of, with_operations};
  use domain_types::operation::{Intent, OperationStep, Resource};
  let failed = robot("r1", Some("auth"), PatronusRobotStatus::Running, 1);
  let restart = restart(RestartAction::Restart, None);
  let offered = robot_actions(&robot_facts(&failed, Some(&restart)));
  let restarting = [operation(
    "o1",
    Intent::RestartRobot { robot_id: "r1".to_string() },
    vec![Resource::Robot { id: "r1".to_string() }],
    OperationStep::Starting,
  )];
  let actions = with_operations(offered, Of::Robot { id: "r1" }, &restarting);
  let of = |id: ActionId| actions.iter().find(|action| action.id == id).map(|action| action.availability.clone());
  assert_eq!(of(RestartRobot), Some(Availability::Running { op: op("o1") }));
  assert_eq!(
    of(CancelRobot),
    Some(Availability::Disabled {
      reason: ActionBlocker::Busy { op: op("o1") }
    })
  );
  assert_eq!(of(OpenRobot), Some(Availability::Enabled));
}

/// A repository that lands by a push into the target offers its push, of a branch and of a whole stack that forks, with
/// the reason of its step, and none of the starts of the robot: on Space neither
#[test]
fn a_push_into_the_target_offers_its_push_with_the_reason_of_its_step_and_no_start_of_the_robot() {
  let step = NextStep {
    row: StepRow::Push,
    kind: StepKind::PushToBaseline { target: "master".to_string() },
    without_review: Some(WithoutReview { blocker: None }),
    ..ready_step()
  };
  let status = remote(PushAction::Push);
  let push_route = |patronus| BranchFacts {
    patronus,
    push_route: true,
    ..facts(&step, &status)
  };
  for patronus in [PatronusAnswer::NotOnSpace, PatronusAnswer::Answers { failure: None }] {
    let actions = branch_actions(&push_route(patronus));
    assert_eq!(ids(&actions)[..3], [PushToBaseline, Push, ReviewChanges]);
    assert_eq!(reason_of(&actions, PushToBaseline), None);
  }
  let no_access = Reason::NoWriteAccess {
    repository: "acme/monorepo".to_string(),
    target: "master".to_string(),
  };
  let refused = NextStep {
    safe_push: Some(StepBlocker::Rule { reason: no_access.clone() }),
    ..step.clone()
  };
  let refused_facts = BranchFacts {
    push_route: true,
    ..facts(&refused, &status)
  };
  assert_eq!(
    reason_of(&branch_actions(&refused_facts), PushToBaseline),
    Some(ActionBlocker::Step {
      blocker: StepBlocker::Rule { reason: no_access }
    })
  );

  let built = StackShape::Forking {
    not_built: None,
    tip: true,
    main_line: None,
  };
  let stack = StackFacts {
    push_route: true,
    ..stack_facts(built, Some(&step))
  };
  assert_eq!(
    table(&stack_actions(&stack)),
    [(PushToBaseline, None), (CreateStackPullRequest, None), (ReviewWholeStack, None)]
  );
  let chain = StackFacts {
    push_route: true,
    ..stack_facts(StackShape::Chain { length: 2, review: None }, Some(&step))
  };
  assert_eq!(ids(&stack_actions(&chain)), [ReviewWholeStack]);
}

/// A merge of the pull request on GitHub offers its merge with the reason of its step, none of the robot's starts; while
/// its auto-merge is enabled, Cancel Auto-Merge instead; of a whole stack that forks the merge of its stack branch's
#[test]
fn a_merge_of_the_pull_request_offers_its_merge_and_cancel_auto_merge_while_github_merges_it() {
  use domain_types::pull_request::MergeMethod;
  let step = NextStep {
    row: StepRow::Merge,
    kind: StepKind::MergePullRequest {
      number: 7,
      auto: false,
      method: MergeMethod::Rebase,
    },
    ..ready_step()
  };
  let status = remote(PushAction::Push);
  let merge_route = |step: &NextStep| {
    let facts = BranchFacts {
      patronus: PatronusAnswer::NotOnSpace,
      merge_route: true,
      ..facts(step, &status)
    };
    branch_actions(&facts)
  };
  let actions = merge_route(&step);
  assert_eq!(ids(&actions)[..3], [MergePullRequest, Push, ReviewChanges]);
  assert_eq!(reason_of(&actions, MergePullRequest), None);
  let approvals = NextStep {
    row: StepRow::MergeDoublePrime,
    kind: StepKind::MergeHeld { number: 7 },
    safe_push: Some(StepBlocker::Rule {
      reason: Reason::MergeNeedsApprovals { number: 7 },
    }),
    ..step.clone()
  };
  assert_eq!(
    reason_of(&merge_route(&approvals), MergePullRequest),
    Some(ActionBlocker::Step {
      blocker: StepBlocker::Rule {
        reason: Reason::MergeNeedsApprovals { number: 7 }
      }
    })
  );
  let auto_merging = NextStep {
    row: StepRow::MergePrime,
    kind: StepKind::AutoMerging {
      number: 7,
      method: MergeMethod::Rebase,
    },
    safe_push: Some(StepBlocker::Step),
    ..step.clone()
  };
  let actions = merge_route(&auto_merging);
  assert_eq!(ids(&actions)[..2], [CancelAutoMerge, Push]);
  assert_eq!(reason_of(&actions, CancelAutoMerge), None);

  let built = StackShape::Forking {
    not_built: None,
    tip: true,
    main_line: None,
  };
  let stack = StackFacts {
    patronus: PatronusAnswer::NotOnSpace,
    merge_route: true,
    ..stack_facts(built, Some(&step))
  };
  assert_eq!(ids(&stack_actions(&stack))[..1], [MergePullRequest]);
}

/// The start of a whole stack on a route without a robot, in the header of its island and in Whole Stack › of the menu
/// of its root (both read `stack_actions`): of a stack that forks the push of its stack branch into the target, or the
/// merge of the pull request of its stack branch, with the reason of its step; while GitHub merges it Cancel Auto-Merge;
/// never a start of the robot, also on Space, nor the Safe Push of the main line of a stack branch the sync did not
/// build; a chain lands by its last branch, so its header offers no start of its own
#[test]
fn a_whole_stack_off_the_robot_offers_the_push_or_the_merge_of_its_stack_branch_with_the_reason_of_its_step() {
  use domain_types::pull_request::MergeMethod;
  let built = StackShape::Forking {
    not_built: None,
    tip: true,
    main_line: None,
  };
  let leaf = ready_step();
  let not_built = Reason::StackNotBuilt {
    root: "air".to_string(),
    state: StackBranchState::Blocked,
    blocked_by: Some("deploy".to_string()),
    conflict: None,
    error: None,
  };
  let blocked = StackShape::Forking {
    not_built: Some(&not_built),
    tip: false,
    main_line: Some(&leaf),
  };
  let chain = StackShape::Chain { length: 2, review: None };
  let no_access = Reason::NoWriteAccess {
    repository: "acme/monorepo".to_string(),
    target: "master".to_string(),
  };
  let refused = |step: &NextStep| NextStep {
    safe_push: Some(StepBlocker::Rule { reason: no_access.clone() }),
    ..step.clone()
  };
  let starts = |actions: &[ActionState]| {
    actions
      .iter()
      .filter(|action| {
        matches!(
          action.id,
          PushToBaseline | MergePullRequest | CancelAutoMerge | SafePushWholeStack | DryRunWholeStack | SafePushMainLine
        )
      })
      .map(|action| (action.id, reason_of(actions, action.id)))
      .collect::<Vec<_>>()
  };

  let push = NextStep {
    row: StepRow::Push,
    kind: StepKind::PushToBaseline { target: "master".to_string() },
    ..ready_step()
  };
  let push_route = |shape, step, patronus| StackFacts {
    patronus,
    push_route: true,
    ..stack_facts(shape, Some(step))
  };
  for patronus in [PatronusAnswer::NotOnSpace, PatronusAnswer::Answers { failure: None }] {
    assert_eq!(starts(&stack_actions(&push_route(built, &push, patronus))), [(PushToBaseline, None)]);
  }
  let push_refused = refused(&push);
  assert_eq!(
    starts(&stack_actions(&push_route(built, &push_refused, PatronusAnswer::NotOnSpace))),
    [(
      PushToBaseline,
      Some(ActionBlocker::Step {
        blocker: StepBlocker::Rule { reason: no_access.clone() }
      })
    )]
  );
  assert_eq!(starts(&stack_actions(&push_route(blocked, &push, PatronusAnswer::NotOnSpace))), [(PushToBaseline, None)]);
  assert_eq!(starts(&stack_actions(&push_route(chain, &push, PatronusAnswer::NotOnSpace))), []);

  let merge = NextStep {
    row: StepRow::Merge,
    kind: StepKind::MergePullRequest {
      number: 12,
      auto: false,
      method: MergeMethod::Rebase,
    },
    ..ready_step()
  };
  let merge_route = |shape, step, patronus| StackFacts {
    patronus,
    merge_route: true,
    ..stack_facts(shape, Some(step))
  };
  for patronus in [PatronusAnswer::NotOnSpace, PatronusAnswer::Answers { failure: None }] {
    assert_eq!(starts(&stack_actions(&merge_route(built, &merge, patronus))), [(MergePullRequest, None)]);
  }
  let approvals = NextStep {
    row: StepRow::MergeDoublePrime,
    kind: StepKind::MergeHeld { number: 12 },
    safe_push: Some(StepBlocker::Rule {
      reason: Reason::MergeNeedsApprovals { number: 12 },
    }),
    ..merge.clone()
  };
  assert_eq!(
    starts(&stack_actions(&merge_route(built, &approvals, PatronusAnswer::NotOnSpace))),
    [(
      MergePullRequest,
      Some(ActionBlocker::Step {
        blocker: StepBlocker::Rule {
          reason: Reason::MergeNeedsApprovals { number: 12 }
        }
      })
    )]
  );
  let auto_merging = NextStep {
    row: StepRow::MergePrime,
    kind: StepKind::AutoMerging {
      number: 12,
      method: MergeMethod::Rebase,
    },
    safe_push: Some(StepBlocker::Step),
    ..merge.clone()
  };
  assert_eq!(
    starts(&stack_actions(&merge_route(built, &auto_merging, PatronusAnswer::NotOnSpace))),
    [(CancelAutoMerge, None)]
  );
  assert_eq!(
    starts(&stack_actions(&merge_route(blocked, &merge, PatronusAnswer::NotOnSpace))),
    [(MergePullRequest, None)]
  );
  assert_eq!(starts(&stack_actions(&merge_route(chain, &merge, PatronusAnswer::NotOnSpace))), []);
}

/// On a frame nobody checked yet (the cached frame of an earlier run, or a fact of it the actions read) every remote
/// action of a branch, a whole stack, a robot and a pull request to review waits for the first sync and the fresh
/// answers, whatever else the frame says of it (a push it holds back for its own reason too); the rewrites, the park,
/// the reviews and the links stay as they are
#[test]
fn on_a_frame_nobody_checked_every_remote_action_waits_for_the_first_sync_and_the_rest_stays() {
  use crate::actions::review_actions;
  use ActionId::{Approve, RequestChanges, Review, SafePushPullRequest};
  let cached = Some(ActionBlocker::Cached);
  let writes = [
    SafePush,
    SafePushWithProfile,
    DryRun,
    Push,
    PushWithChildren,
    PushToBaseline,
    RequestReview,
    RetargetPullRequest,
    CreatePullRequest,
    SafePushWholeStack,
    DryRunWholeStack,
    SafePushMainLine,
    CreateStackPullRequest,
    RestartRobot,
    PushAndRestartRobot,
    CancelRobot,
    Approve,
    SafePushPullRequest,
    RequestChanges,
  ];
  let assert_waits = |of: &str, fresh: Vec<ActionState>, frame: Vec<ActionState>| {
    assert_eq!(ids(&frame), ids(&fresh), "{of}: the frame offers what the facts offer");
    for ((id, reason), (_, fresh)) in table(&frame).into_iter().zip(table(&fresh)) {
      let expected = if writes.contains(&id) { cached.clone() } else { fresh };
      assert_eq!(reason, expected, "{of}: {id:?}");
    }
  };

  let step = ready_step();
  let status = remote(PushAction::UpToDate);
  let branch = facts(&step, &status);
  let frame = branch_actions(&BranchFacts { cached: true, ..branch.clone() });
  assert_eq!(reason_of(&frame, Push), cached, "up to date as the frame says: the push waits for the sync all the same");
  assert_eq!((reason_of(&frame, Rename), reason_of(&frame, Park), reason_of(&frame, CheckApprovals)), (None, None, None));
  assert_waits("a branch", branch_actions(&branch), frame);

  let stack = stack_facts(
    StackShape::Forking {
      not_built: None,
      tip: true,
      main_line: None,
    },
    Some(&step),
  );
  assert_waits("a whole stack", stack_actions(&stack), stack_actions(&StackFacts { cached: true, ..stack.clone() }));

  let failed = robot("r1", Some("auth"), PatronusRobotStatus::Failure, 1);
  let restart = restart(RestartAction::Restart, None);
  let of_robot = robot_facts(&failed, Some(&restart));
  assert_waits("a failed robot", robot_actions(&of_robot), robot_actions(&RobotFacts { cached: true, ..of_robot.clone() }));
  let running = robot("r2", Some("auth"), PatronusRobotStatus::Running, 1);
  let of_running = robot_facts(&running, None);
  assert_waits(
    "a running robot",
    robot_actions(&of_running),
    robot_actions(&RobotFacts {
      cached: true,
      ..of_running.clone()
    }),
  );

  let to_review = pull_request_to_review();
  let frame = review_actions(&crate::actions::PullRequestFacts { cached: true, ..to_review });
  assert_eq!(reason_of(&frame, Review), None, "the review opens: it reads the pull request as GitHub has it");
  assert_waits("a pull request to review", review_actions(&to_review), frame);
}

/// The remote actions read a frame nobody checked while the table is the cached frame of an earlier run, or one of the
/// facts they read is a value of it; facts of this process, however they stand, check it
#[test]
fn a_frame_is_unchecked_while_its_table_or_a_fact_the_actions_read_is_the_cached_one() {
  let fresh = FactState::Fresh;
  let stale = FactState::Stale { reason: None };
  assert!(on_cached_frame(true, [&fresh]));
  assert!(on_cached_frame(false, [&fresh, &FactState::Cached]));
  assert!(!on_cached_frame(false, [&fresh, &stale, &FactState::Loading, &FactState::Unknown]));
  assert!(!on_cached_frame(false, []));
}
