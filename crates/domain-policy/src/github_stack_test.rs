use crate::github_stack::{chain_of, layer_parent, rebased_by_github, stack_writes};
use crate::stack_test::{Spec, repository, stacks_of};
use domain_types::fixtures::pull_request;
use domain_types::pull_request::{PullRequest, PullRequestStackEntry, StackWrite};
use domain_types::reason::Reason;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// A branch built on its declared parent: `(name, parent)`
const fn on(name: &'static str, parent: Option<&'static str>) -> Spec {
  (name, parent, parent)
}

/// oauth on login on auth, in the order of the table (newest first)
const CHAIN: [Spec; 3] = [on("oauth", Some("login")), on("login", Some("auth")), on("auth", None)];

/// A straight chain is a stack of GitHub of all its branches, bottom first, for every layer of it; a branch alone is
/// none, its pull request goes into the target
#[test]
fn a_straight_chain_is_a_stack_of_github_and_a_branch_alone_is_none() {
  let mut specs = CHAIN.to_vec();
  specs.push(on("docs", None));
  let (stacks, parked) = (stacks_of(&specs), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  for name in ["auth", "login", "oauth"] {
    assert_eq!(chain_of(&repo, |_| false, |_| false, name), Some(vec!["auth", "login", "oauth"]), "{name}");
  }
  assert_eq!(chain_of(&repo, |_| false, |_| false, "docs"), None);
  let chain = chain_of(&repo, |_| false, |_| false, "oauth");
  assert_eq!(layer_parent(chain.as_deref(), "oauth"), Some("login"));
  assert_eq!(layer_parent(chain.as_deref(), "auth"), None);
  assert_eq!(layer_parent(None, "oauth"), None);
}

/// A stack that forks is no stack of GitHub: GitHub stacks are linear, and the stack branch lands it whole
#[test]
fn a_stack_that_forks_is_none() {
  let mut specs = CHAIN.to_vec();
  specs.push(on("session", Some("auth")));
  let (stacks, parked) = (stacks_of(&specs), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(chain_of(&repo, |root| root == "auth", |_| false, "oauth"), None);
  // and a fork the sync did not build a stack branch of yet
  assert_eq!(chain_of(&repo, |_| false, |_| false, "oauth"), None);
}

/// The chain starts above the branches that landed; a chain of one branch left is none
#[test]
fn the_chain_starts_above_what_landed() {
  let (stacks, parked) = (stacks_of(&CHAIN), BTreeMap::new());
  let repo = repository(&stacks, &parked);
  assert_eq!(chain_of(&repo, |_| false, |branch| branch == "auth", "oauth"), Some(vec!["login", "oauth"]));
  assert_eq!(chain_of(&repo, |_| false, |branch| branch == "auth", "auth"), None);
  assert_eq!(chain_of(&repo, |_| false, |branch| branch != "oauth", "oauth"), None);
}

/// A parked branch, a blocked one (its build took no base) and one built on the baseline end the chain at the branch
/// below them
#[test]
fn a_parked_a_blocked_or_a_rebased_branch_ends_the_chain() {
  let parked_login = BTreeMap::from([("login".to_string(), "wait".to_string())]);
  let stacks = stacks_of(&CHAIN);
  let repo = repository(&stacks, &parked_login);
  assert_eq!(chain_of(&repo, |_| false, |_| false, "auth"), None);

  let blocked = stacks_of(&[on("oauth", Some("login")), ("login", None, Some("auth")), on("auth", None)]);
  let parked = BTreeMap::new();
  let repo = repository(&blocked, &parked);
  assert_eq!(chain_of(&repo, |_| false, |_| false, "oauth"), None);

  let blocked_top = stacks_of(&[("oauth", None, Some("login")), on("login", Some("auth")), on("auth", None)]);
  let repo = repository(&blocked_top, &parked);
  assert_eq!(chain_of(&repo, |_| false, |_| false, "login"), Some(vec!["auth", "login"]));
  assert_eq!(chain_of(&repo, |_| false, |_| false, "oauth"), None);
}

fn open_pull_request(branch: &str, number: u32, base: &str, stack: Option<(u32, u32, u32)>) -> PullRequest {
  let mut pull_request = pull_request(branch, number);
  pull_request.head_ref_name = format!("develar/virtual/{branch}");
  pull_request.base_ref_name = base.to_string();
  pull_request.stack = stack.map(|(number, size, position)| PullRequestStackEntry {
    number,
    base_ref_name: "master".to_string(),
    size,
    position,
  });
  pull_request
}

fn writes(pull_requests: &[PullRequest]) -> Vec<StackWrite> {
  let by_branch: BTreeMap<&str, &PullRequest> = pull_requests
    .iter()
    .map(|pull_request| (pull_request.head_ref_name.trim_start_matches("develar/virtual/"), pull_request))
    .collect();
  stack_writes(&["auth", "login", "oauth"], |branch| by_branch.get(branch).copied(), "master", "develar", false)
}

/// The layers of a chain without a stack make a new one; with fewer than two layers there is none to make
#[test]
fn layers_outside_a_stack_make_one() {
  let auth = open_pull_request("auth", 1, "master", None);
  let login = open_pull_request("login", 2, "develar/virtual/auth", None);
  let oauth = open_pull_request("oauth", 3, "develar/virtual/login", None);
  assert_eq!(writes(&[auth.clone(), login, oauth]), [StackWrite::Create { pull_requests: vec![1, 2, 3] }]);
  // a layer without a pull request ends the layers
  assert_eq!(writes(&[auth.clone(), open_pull_request("oauth", 3, "develar/virtual/login", None)]), []);
  assert_eq!(writes(std::slice::from_ref(&auth)), []);
  // so does a pull request of before the stacks into the target above the bottom, which the user takes in
  assert_eq!(writes(&[auth, open_pull_request("login", 2, "master", None)]), []);
}

/// A stack as the chain is needs nothing; new layers on top of it are added, a layer into another virtual branch is
/// retargeted first
#[test]
fn a_stack_in_line_needs_nothing_and_new_layers_go_on_top() {
  let auth = open_pull_request("auth", 1, "master", Some((4, 2, 1)));
  let login = open_pull_request("login", 2, "develar/virtual/auth", Some((4, 2, 2)));
  assert_eq!(writes(&[auth.clone(), login.clone()]), []);
  let oauth = open_pull_request("oauth", 3, "develar/virtual/login", None);
  assert_eq!(writes(&[auth.clone(), login.clone(), oauth]), [StackWrite::Add { stack: 4, pull_requests: vec![3] }]);
  let stale = open_pull_request("oauth", 3, "develar/virtual/auth", None);
  assert_eq!(
    writes(&[auth, login, stale]),
    [
      StackWrite::Retarget {
        branch: "oauth".to_string(),
        number: 3,
        from: "develar/virtual/auth".to_string(),
        to: "develar/virtual/login".to_string(),
      },
      StackWrite::Add { stack: 4, pull_requests: vec![3] },
    ]
  );
}

/// After a partial merge the layers left are the top of their stack above the merged ones: in line
#[test]
fn the_layers_left_after_a_partial_merge_are_in_line() {
  let login = open_pull_request("login", 2, "master", Some((4, 3, 2)));
  let oauth = open_pull_request("oauth", 3, "develar/virtual/login", Some((4, 3, 3)));
  let by_branch = BTreeMap::from([("login", &login), ("oauth", &oauth)]);
  assert_eq!(stack_writes(&["login", "oauth"], |branch| by_branch.get(branch).copied(), "master", "develar", false), []);
}

/// A stack out of line with the chain (another order, a member to retarget, layers of two stacks) is unstacked, the
/// layers retargeted, and a new stack made of them
#[test]
fn a_stack_out_of_line_is_made_anew() {
  let auth = open_pull_request("auth", 1, "master", Some((4, 2, 1)));
  let login = open_pull_request("login", 2, "develar/virtual/oauth", Some((4, 2, 2)));
  let oauth = open_pull_request("oauth", 3, "develar/virtual/login", None);
  assert_eq!(
    writes(&[auth, login, oauth]),
    [
      StackWrite::Unstack { stack: 4 },
      StackWrite::Retarget {
        branch: "login".to_string(),
        number: 2,
        from: "develar/virtual/oauth".to_string(),
        to: "develar/virtual/auth".to_string(),
      },
      StackWrite::Create { pull_requests: vec![1, 2, 3] },
    ]
  );
  let auth = open_pull_request("auth", 1, "master", Some((4, 2, 1)));
  let login = open_pull_request("login", 2, "develar/virtual/auth", Some((7, 2, 2)));
  assert_eq!(
    writes(&[auth, login]),
    [
      StackWrite::Unstack { stack: 4 },
      StackWrite::Unstack { stack: 7 },
      StackWrite::Create { pull_requests: vec![1, 2] }
    ]
  );
}

/// After the layers below it merged, GitHub moves a layer onto the target with copies of its commits: its push waits
/// for the update while the branch is still built on what merged, and goes once the update built it on the target
#[test]
fn a_layer_github_moved_onto_the_target_waits_for_the_update() {
  let moved = open_pull_request("oauth", 3, "master", Some((4, 3, 3)));
  assert_eq!(
    rebased_by_github("oauth", &moved, Some("login"), "master"),
    Some(Reason::LayerRebased {
      branch: "oauth".to_string(),
      number: 3,
      target: "master".to_string(),
    })
  );
  assert_eq!(rebased_by_github("oauth", &moved, None, "master"), None);
  // the bottom layer goes into the target from the start
  let bottom = open_pull_request("auth", 1, "master", Some((4, 3, 1)));
  assert_eq!(rebased_by_github("auth", &bottom, Some("base"), "master"), None);
  let layer = open_pull_request("oauth", 3, "develar/virtual/login", Some((4, 3, 3)));
  assert_eq!(rebased_by_github("oauth", &layer, Some("login"), "master"), None);
}

/// A pull request of before the stacks of GitHub into the target above the bottom ends the layers of a push, which
/// leaves it as it is; Link GitHub Stack (`join`) retargets it into the branch below it and makes the stack
#[test]
fn an_old_pull_request_into_the_target_joins_the_stack_only_when_the_user_links_it() {
  let auth = open_pull_request("auth", 1, "master", None);
  let login = open_pull_request("login", 2, "master", None);
  let by_branch = BTreeMap::from([("auth", &auth), ("login", &login)]);
  let writes = |join| stack_writes(&["auth", "login"], |branch| by_branch.get(branch).copied(), "master", "develar", join);
  assert_eq!(writes(false), []);
  assert_eq!(
    writes(true),
    [
      StackWrite::Retarget {
        branch: "login".to_string(),
        number: 2,
        from: "master".to_string(),
        to: "develar/virtual/auth".to_string(),
      },
      StackWrite::Create { pull_requests: vec![1, 2] },
    ]
  );
}
