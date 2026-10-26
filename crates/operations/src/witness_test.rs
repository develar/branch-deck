use crate::witness::{Witness, check, read_refs};
use domain_types::error::ErrorCode;
use domain_types::fixtures::{pull_request, push_intent as push, robot};
use domain_types::patronus::PatronusRobotStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::git_test_utils::TestRepo;

/// The tips of the refs a plan moves and leases, an absent one `None`; a ref under one of them doesn't count
#[test]
fn the_witness_reads_the_tips_of_exactly_the_refs_it_names() {
  let repo = TestRepo::new();
  let tip = repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  repo.git(&["update-ref", "refs/heads/user/virtual/auth", &tip]);
  repo.git(&["update-ref", "refs/heads/user/virtual/auth-ui/x", &tip]);
  let refs = ["refs/heads/user/virtual/auth", "refs/heads/user/virtual/auth-ui", "refs/remotes/origin/user/virtual/auth"].map(str::to_string);
  let read = read_refs(&GitCommandExecutor::new(), repo.path_str(), &refs).unwrap();
  assert_eq!(read, BTreeMap::from([(refs[0].clone(), Some(tip)), (refs[1].clone(), None), (refs[2].clone(), None),]));
}

/// The fingerprint of a plan is the same while the intent and the facts it read are, and another when a ref moved, a pull
/// request changed its base or became a draft, or a robot it reads finished: then nothing of the plan may run
#[test]
fn a_fingerprint_changes_with_any_fact_the_plan_read_and_the_check_refuses_it() {
  let refs = BTreeMap::from([("refs/heads/user/virtual/auth".to_string(), Some("a1".to_string()))]);
  let pull_requests = [pull_request("auth", 12)];
  let robots = [robot("r1", Some("auth"), PatronusRobotStatus::Running, 1)];
  let witness = Witness::of(refs.clone(), &pull_requests, &robots);
  let planned = witness.fingerprint(&push("auth"));
  assert_eq!(Witness::of(refs.clone(), &pull_requests, &robots).fingerprint(&push("auth")), planned);
  assert_eq!(check(&planned, &planned), Ok(()));

  let moved = Witness::of(
    BTreeMap::from([("refs/heads/user/virtual/auth".to_string(), Some("a2".to_string()))]),
    &pull_requests,
    &robots,
  );
  let mut retargeted = pull_requests.clone();
  retargeted[0].base_ref_name = "user/virtual/login".to_string();
  let mut draft = pull_requests.clone();
  draft[0].is_draft = true;
  let finished = [robot("r1", Some("auth"), PatronusRobotStatus::Failure, 1)];
  for changed in [
    moved,
    Witness::of(refs.clone(), &retargeted, &robots),
    Witness::of(refs.clone(), &draft, &robots),
    Witness::of(refs, &pull_requests, &finished),
  ] {
    let error = check(&planned, &changed.fingerprint(&push("auth"))).unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanChanged);
  }
  // another intent on the same facts is another plan
  assert_ne!(witness.fingerprint(&push("login")), planned);
}
