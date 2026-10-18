use crate::robot_commits::robot_commits_test::{commit, forking_stack, stacks_repo, sync};
use crate::robot_landing::robot_landings;
use pretty_assertions::assert_eq;
use sync_types::robot_landing::{BranchLanding, RobotLanding, RobotLandingRequest};
use test_utils::git_test_utils::TestRepo;

fn tip(repo: &TestRepo, full_name: &str) -> String {
  repo.git(&["rev-parse", &format!("refs/heads/{full_name}")])
}

fn strings(names: &[&str]) -> Vec<String> {
  names.iter().map(|name| (*name).to_string()).collect()
}

/// The robot of `login`, which ran on `source` and may have landed `auth` and `login`
fn login_robot(source: &str) -> RobotLandingRequest {
  RobotLandingRequest {
    robot_id: "login-robot".to_string(),
    branch_name: Some("login".to_string()),
    stack_root: None,
    source_revision: source.to_string(),
    branches: strings(&["auth", "login"]),
    tip: None,
  }
}

fn landings(repo: &TestRepo, requests: &[RobotLandingRequest]) -> Vec<RobotLanding> {
  robot_landings(repo.git_executor(), repo.path_str(), "test", "master", requests).unwrap()
}

fn landed(branch: &str, landed: &[&str], after: &[&str]) -> BranchLanding {
  BranchLanding {
    branch: branch.to_string(),
    landed: strings(landed),
    after: strings(after),
  }
}

/// What the robot `id` landed: `branches`
fn landing(id: &str, branches: Vec<BranchLanding>) -> Vec<RobotLanding> {
  vec![RobotLanding {
    robot_id: id.to_string(),
    branches,
  }]
}

/// What the robot of `login` landed: `auth` and `login`, each `(id, the ids after it)`
fn piece_landing(auth: (&str, &[&str]), login: (&str, &[&str])) -> Vec<RobotLanding> {
  landing("login-robot", vec![landed("auth", &[auth.0], auth.1), landed("login", &[login.0], login.1)])
}

/// `auth` and `login` built on it, synced: the robot of `login` ran on its tip
struct Piece {
  repo: TestRepo,
  auth: String,
  login: String,
  source: String,
}

async fn piece() -> Piece {
  let repo = stacks_repo();
  let auth = commit(&repo, "(auth) Add auth", "auth.rs", "// auth", 1000);
  let login = commit(&repo, "(auth/login) Add form", "login.rs", "// login", 1001);
  sync(&repo).await;
  let source = tip(&repo, "test/virtual/login");
  Piece { repo, auth, login, source }
}

/// `feature` again on `master` with `(auth) <auth subject>` (`content`) and the commit of `login`: a rewrite below the
/// robot's commit; the new ids of both
async fn rewrite_auth(repo: &TestRepo, subject: &str, content: &str) -> (String, String) {
  repo.git(&["reset", "--hard", "master"]);
  let auth = commit(repo, subject, "auth.rs", content, 1000);
  let login = commit(repo, "(auth/login) Add form", "login.rs", "// login", 1001);
  sync(repo).await;
  (auth, login)
}

#[tokio::test]
async fn a_robot_at_the_tip_landed_the_branch_and_what_it_is_built_on() {
  let Piece { repo, auth, login, source } = piece().await;

  assert_eq!(landings(&repo, &[login_robot(&source)]), piece_landing((&auth, &[]), (&login, &[])));
}

/// A new commit of the parent rebuilds the child: no hash tells any more, the content does. The parent's new commit is
/// the work left; the child, its copy with the same changes, landed
#[tokio::test]
async fn a_commit_added_below_the_robots_commit_is_the_only_work_left() {
  let Piece { repo, auth, login, source } = piece().await;
  let check = commit(&repo, "(auth) Check the token", "token.rs", "// token", 1002);
  sync(&repo).await;
  assert_ne!(tip(&repo, "test/virtual/login"), source, "the child was rebuilt on the parent's new commit");

  assert_eq!(landings(&repo, &[login_robot(&source)]), piece_landing((&auth, &[&check]), (&login, &[])));
}

/// A reword of the parent's commit gives every commit a new hash; their changes are the same, so all of it landed
#[tokio::test]
async fn a_reworded_commit_below_keeps_the_branches_landed() {
  let Piece { repo, source, .. } = piece().await;
  let (auth, login) = rewrite_auth(&repo, "(auth) Add the auth module", "// auth").await;

  assert_eq!(landings(&repo, &[login_robot(&source)]), piece_landing((&auth, &[]), (&login, &[])));
}

/// An amend changes what the branch is: it didn't land as it is now, the child built on it still did
#[tokio::test]
async fn an_amended_commit_leaves_its_branch_not_landed() {
  let Piece { repo, source, .. } = piece().await;
  let (_, login) = rewrite_auth(&repo, "(auth) Add auth", "// auth v2").await;

  assert_eq!(landings(&repo, &[login_robot(&source)]), landing("login-robot", vec![landed("login", &[&login], &[])]));
}

/// A commit after the last sync is not in the virtual branch: it is after the landed ones, also while the ref is at the
/// robot's commit
#[tokio::test]
async fn a_commit_not_synced_yet_is_after_the_landed_ones() {
  let Piece { repo, auth, login, source } = piece().await;
  let button = commit(&repo, "(auth/login) Add the button", "button.rs", "// button", 1002);

  assert_eq!(landings(&repo, &[login_robot(&source)]), piece_landing((&auth, &[]), (&login, &[&button])));
}

/// The robot of the whole stack ran on the stack branch: every branch of the stack landed, and after a new commit of a
/// side branch, which rebuilds the stack branch, that commit is the work left
#[tokio::test]
async fn the_robot_of_the_whole_stack_landed_every_branch_of_it() {
  let (repo, [auth, login, session], source) = forking_stack().await;
  let request = RobotLandingRequest {
    robot_id: "stack-robot".to_string(),
    branch_name: None,
    stack_root: Some("auth".to_string()),
    source_revision: source.clone(),
    branches: strings(&["auth", "login", "session"]),
    tip: None,
  };
  let all = |session_after: &[&str]| {
    landing(
      "stack-robot",
      vec![
        landed("auth", &[&auth], &[]),
        landed("login", &[&login], &[]),
        landed("session", &[&session], session_after),
      ],
    )
  };
  assert_eq!(landings(&repo, std::slice::from_ref(&request)), all(&[]));

  let expire = commit(&repo, "(auth/session) Expire the session", "expire.rs", "// expire", 1003);
  sync(&repo).await;
  assert_ne!(tip(&repo, "test/stack/auth"), source, "the stack branch was rebuilt");
  assert_eq!(landings(&repo, &[request]), all(&[&expire]));
}

/// A source revision the repository lacks (started on another machine, pruned) tells nothing; one the caller knows the
/// ref is at needs no lookup
#[tokio::test]
async fn a_missing_source_lands_nothing() {
  let Piece { repo, auth, login, source } = piece().await;

  assert_eq!(landings(&repo, &[login_robot(&"0".repeat(40))]), []);
  let known = RobotLandingRequest {
    tip: Some(source.clone()),
    ..login_robot(&source)
  };
  assert_eq!(landings(&repo, &[known]), piece_landing((&auth, &[]), (&login, &[])));
}
