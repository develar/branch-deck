use crate::admission::{admit, holds_across};
use domain_types::error::HolderTool;
use domain_types::fixtures::{land_intent, push_intent};
use domain_types::landing::Subject;
use domain_types::operation::{Admission, Intent, OperationId, OperationStep, Origin, PullRequestField, Resource, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::rewrite::{RewriteIntent, TableWitness, UndoKind, UndoToken};
use pretty_assertions::assert_eq;

fn push(branch: &str) -> Intent {
  push_intent(branch)
}

fn land(branch: &str, mode: PatronusPushMode) -> Intent {
  land_intent(branch, mode)
}

fn reference(branch: &str) -> Resource {
  Resource::Ref {
    name: format!("user/virtual/{branch}"),
  }
}

fn lands(branch: &str) -> Resource {
  Resource::Lands { branch: branch.to_string() }
}

fn field(number: u32, field: PullRequestField) -> Resource {
  Resource::PullRequest { number, field }
}

fn id(text: &str) -> OperationId {
  OperationId(text.to_string())
}

/// An operation of this process that runs
fn running(op: &str, intent: Intent, resources: &[Resource]) -> RunningOperation {
  RunningOperation {
    id: id(op),
    intent,
    resources: resources.to_vec(),
    step: OperationStep::Checking,
    origin: Origin::ThisProcess,
    moves: Vec::new(),
    joined: 0,
  }
}

/// The same operation, of another process (`bd` with pid 4242)
fn of_bd(operation: RunningOperation) -> RunningOperation {
  RunningOperation {
    origin: Origin::OtherProcess { tool: HolderTool::Bd, pid: 4242 },
    ..operation
  }
}

/// A remote action over a remote action, row by row of the table "Конкурентность": the same intent joins, a resource
/// held waits, nothing held runs
#[test]
fn a_remote_action_joins_the_same_intent_waits_for_a_held_resource_and_runs_beside_the_rest() {
  let auth = running("a", push("auth"), &[reference("auth")]);
  // the same intent and subject: a second click joins the first
  assert_eq!(admit(std::slice::from_ref(&auth), &push("auth"), &[reference("auth")]), Admission::Join { op: id("a") });
  // another intent that needs what it holds waits behind it: a Safe Push that pushes auth
  let safe_push = land("auth", PatronusPushMode::RebaseAutosquash);
  assert_eq!(
    admit(std::slice::from_ref(&auth), &safe_push, &[reference("auth"), lands("auth")]),
    Admission::Queue { op: id("a") }
  );
  // nothing it holds: side by side
  assert_eq!(admit(std::slice::from_ref(&auth), &push("login"), &[reference("login")]), Admission::Run);
  assert_eq!(admit(&[], &push("auth"), &[reference("auth")]), Admission::Run);
}

/// An operation of another process that the session shows decides nothing in it: the operations registry of the
/// repository admitted the new one over the file as it is now (`holds_across`), and the record the session shows may have
/// left it since. The new one neither joins it nor waits behind it, and waits behind an operation of this process only
#[test]
fn an_operation_of_another_process_that_the_session_shows_decides_nothing() {
  let auth = running("a", push("auth"), &[reference("auth")]);
  let safe_push = land("auth", PatronusPushMode::RebaseAutosquash);
  assert_eq!(admit(&[of_bd(auth.clone())], &push("auth"), &[reference("auth")]), Admission::Run);
  assert_eq!(admit(&[of_bd(auth.clone())], &safe_push, &[reference("auth")]), Admission::Run);
  let own = running("b", land("login", PatronusPushMode::DryRun), &[reference("auth")]);
  assert_eq!(admit(&[own, of_bd(auth)], &safe_push, &[reference("auth")]), Admission::Queue { op: id("b") });
}

/// The ones that wait for a resource run in the order they came: a new one waits behind the newest that holds what it
/// needs, a waiting one included, never behind the first only
#[test]
fn a_new_operation_waits_behind_the_newest_that_holds_what_it_needs() {
  let first = running("a", push("auth"), &[reference("auth")]);
  let queued = RunningOperation {
    step: OperationStep::Queued { behind: id("a") },
    ..running("b", land("auth", PatronusPushMode::DryRun), &[reference("auth"), lands("auth")])
  };
  let both = [first, queued.clone()];
  assert_eq!(
    admit(&both, &land("auth", PatronusPushMode::RebaseAutosquash), &[lands("auth")]),
    Admission::Queue { op: id("b") }
  );
  // the same intent as a waiting one joins it
  assert_eq!(admit(&both, &queued.intent, &queued.resources), Admission::Join { op: id("b") });
}

/// A resource is one only when it is equal: two writes of a pull request wait for each other on the same field only, a
/// ref and what a robot lands of a branch of that name are two, two Safe Pushes that land a branch are one
#[test]
fn only_an_equal_resource_is_held() {
  let reviewers = running(
    "r",
    Intent::RequestReview {
      subject: Subject::branch("auth"),
      reviewers: vec!["alice".to_string()],
      push_first: false,
    },
    &[field(12, PullRequestField::Reviewers)],
  );
  let retarget = Intent::Retarget { number: 12 };
  assert_eq!(admit(std::slice::from_ref(&reviewers), &retarget, &[field(12, PullRequestField::Base)]), Admission::Run);
  assert_eq!(
    admit(std::slice::from_ref(&reviewers), &retarget, &[field(12, PullRequestField::Reviewers)]),
    Admission::Queue { op: id("r") }
  );
  assert_eq!(admit(&[reviewers], &retarget, &[field(13, PullRequestField::Reviewers)]), Admission::Run);

  let start = running("s", land("login", PatronusPushMode::RebaseAutosquash), &[reference("login"), lands("auth"), lands("login")]);
  assert_eq!(admit(std::slice::from_ref(&start), &push("auth"), &[reference("auth")]), Admission::Run);
  // a Safe Push of auth would land what the running one lands: it waits and is decided again after it
  let of_auth = land("auth", PatronusPushMode::RebaseAutosquash);
  assert_eq!(admit(&[start], &of_auth, &[reference("auth"), lands("auth")]), Admission::Queue { op: id("s") });
}

/// An operation of another process holds what the same intent or a resource of it needs under its own branch prefix;
/// under another prefix only what all the worktrees of the repository share (a ref by its full name, a robot, a pull
/// request), never a branch named without its prefix or the intent, which names its subject so
#[test]
fn another_process_holds_the_same_intent_and_its_resources_and_under_another_prefix_only_the_shared_ones() {
  let start = of_bd(running("s", land("auth", PatronusPushMode::RebaseAutosquash), &[reference("auth"), lands("auth")]));
  let safe_push = land("auth", PatronusPushMode::RebaseAutosquash);
  assert!(holds_across(&start, "user", "user", &safe_push, &[]));
  assert!(holds_across(&start, "user", "user", &land("auth", PatronusPushMode::DryRun), &[lands("auth")]));
  assert!(holds_across(&start, "user", "user", &push("auth"), &[reference("auth")]));
  assert!(!holds_across(&start, "user", "user", &push("docs"), &[reference("docs")]));
  // another prefix: its auth is another branch, its ref another name
  assert!(!holds_across(&start, "user", "other", &safe_push, &[lands("auth")]));
  assert!(holds_across(&start, "user", "other", &push("auth"), &[reference("auth")]));
  let restart = of_bd(running(
    "r",
    Intent::RestartRobot { robot_id: "r1".to_string() },
    &[Resource::Robot { id: "r1".to_string() }],
  ));
  assert!(holds_across(
    &restart,
    "user",
    "other",
    &Intent::CancelRobot { robot_id: "r1".to_string() },
    &[Resource::Robot { id: "r1".to_string() }]
  ));
  let review = Intent::RequestReview {
    subject: Subject::branch("auth"),
    reviewers: Vec::new(),
    push_first: false,
  };
  let reviewers = of_bd(running("v", review.clone(), &[field(12, PullRequestField::Reviewers)]));
  assert!(holds_across(&reviewers, "user", "other", &review, &[field(12, PullRequestField::Reviewers)]));
  assert!(!holds_across(
    &reviewers,
    "user",
    "other",
    &Intent::Retarget { number: 12 },
    &[field(12, PullRequestField::Base)]
  ));
}

/// The rewrite of `branch` (an unstack) on HEAD `head`
fn unstack(branch: &str, head: &str) -> Intent {
  Intent::Rewrite(Box::new(RewriteIntent::Unstack {
    branch_name: branch.to_string(),
    witness: TableWitness {
      head: head.to_string(),
      ..TableWitness::default()
    },
  }))
}

/// Every rewrite holds HEAD: within a process the same rewrite joins, another waits behind the newest rewrite (an undo
/// too, never refused), and a push runs beside them; a rewrite of another process makes none busy, the repository lock
/// orders them, but the same rewrite under the same prefix does
#[test]
fn a_rewrite_waits_for_the_rewrites_of_its_process_and_for_none_of_another() {
  let first = running("a", unstack("beta", "h1"), &[Resource::Head]);
  let undo = Intent::Rewrite(Box::new(RewriteIntent::Undo {
    token: UndoToken {
      kind: UndoKind::Regroup,
      token: "h0..h1".to_string(),
    },
  }));
  let operations = [first.clone()];
  assert_eq!(admit(&operations, &first.intent, &[Resource::Head]), Admission::Join { op: id("a") });
  assert_eq!(admit(&operations, &unstack("gamma", "h1"), &[Resource::Head]), Admission::Queue { op: id("a") });
  assert_eq!(admit(&operations, &undo, &[Resource::Head]), Admission::Queue { op: id("a") });
  assert_eq!(admit(&operations, &push("beta"), &[reference("beta")]), Admission::Run);

  let of_another = of_bd(first);
  assert!(!holds_across(&of_another, "user", "user", &unstack("gamma", "h1"), &[Resource::Head]));
  assert!(!holds_across(&of_another, "user", "user", &undo, &[Resource::Head]));
  assert!(holds_across(&of_another, "user", "user", &unstack("beta", "h1"), &[Resource::Head]));
}
