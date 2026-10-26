use crate::admission::admit;
use domain_types::error::HolderTool;
use domain_types::fixtures::{land_intent, push_intent};
use domain_types::landing::Subject;
use domain_types::operation::{Admission, Intent, OperationId, OperationStep, Origin, PullRequestField, Resource, RunningOperation};
use domain_types::patronus::PatronusPushMode;
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
/// held waits, nothing held runs; an operation of another process makes either busy
#[test]
fn a_remote_action_joins_the_same_intent_waits_for_a_held_resource_and_runs_beside_the_rest() {
  let auth = running("a", push("auth"), &[reference("auth")]);
  // the same intent and subject: a second click joins the first
  assert_eq!(admit(std::slice::from_ref(&auth), &push("auth"), &[reference("auth")]), Admission::Join { op: id("a") });
  // of another process: busy, never a wait for a process
  assert_eq!(admit(&[of_bd(auth.clone())], &push("auth"), &[reference("auth")]), Admission::Busy { holder: id("a") });
  // another intent that needs what it holds waits behind it: a Safe Push that pushes auth
  let safe_push = land("auth", PatronusPushMode::RebaseAutosquash);
  assert_eq!(
    admit(std::slice::from_ref(&auth), &safe_push, &[reference("auth"), lands("auth")]),
    Admission::Queue { op: id("a") }
  );
  // of another process: busy
  assert_eq!(admit(&[of_bd(auth.clone())], &safe_push, &[reference("auth")]), Admission::Busy { holder: id("a") });
  // nothing it holds: side by side
  assert_eq!(admit(std::slice::from_ref(&auth), &push("login"), &[reference("login")]), Admission::Run);
  assert_eq!(admit(&[of_bd(auth)], &push("login"), &[reference("login")]), Admission::Run);
  assert_eq!(admit(&[], &push("auth"), &[reference("auth")]), Admission::Run);
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
