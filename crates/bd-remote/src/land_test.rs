use bd_cli::fixture_test::{assert_error, git, next_steps, run_in};
use bd_cli::remote_fixture_test::{push_from_elsewhere_to, remote_tip, repo_with_remote};
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_utils::git_test_utils::{TestRepo, pre_receive_hook};

const AUTH: &str = "test-user/virtual/auth";

/// `repo_with_remote` of `(auth) Add login` whose remote has the baseline as its master, landing by a push into it
fn landing_by_push() -> (TestRepo, TempDir) {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  git(&repo, &["push", "-q", remote.path().to_str().unwrap(), "refs/remotes/origin/master:refs/heads/master"]);
  git(&repo, &["config", "branchdeck.landing", "push"]);
  (repo, remote)
}

/// A repository that lands by a push: `--check` names what lands and what CI did not check, pushing nothing; the push
/// itself refuses what the user didn't accept (`checks_not_passed`); with `--without-checks` the copy of the branch
/// lands into master as a fast-forward, the remote-tracking ref of master stays, and `bd update` is the next step,
/// which drops the original whose copy landed
#[test_log::test]
fn lands_the_copy_of_a_branch_into_master_once_the_user_accepted_what_ci_did_not_check() {
  let (repo, remote) = landing_by_push();
  let base = remote_tip(&remote, "master");
  let (exit_code, check) = run_in(&repo, &["land", "auth", "--check"]);
  assert_eq!(exit_code, 0, "{check}");
  assert_eq!((&check["route"], &check["lands"], &check["landed"]), (&json!("push"), &json!(["auth"]), &json!(false)));
  assert_eq!(check["refusals"][0]["code"], json!("checks_not_passed"), "{check}");
  assert_eq!(check["next_step"]["step"], json!("push_to_baseline"), "{check}");
  assert_eq!(remote_tip(&remote, "master"), base, "a check pushes nothing");

  let (exit_code, refused) = run_in(&repo, &["land", "auth"]);
  assert_error(&refused, exit_code, (4, "checks_not_passed"));
  assert_eq!(remote_tip(&remote, "master"), base);

  let (exit_code, landed) = run_in(&repo, &["land", "auth", "--without-checks"]);
  assert_eq!(exit_code, 0, "{landed}");
  assert_eq!((&landed["landed"], &landed["next"]), (&json!(true), &json!("bd update")));
  let copy = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, "master"), copy);
  assert_eq!(git(&repo, &["rev-parse", "refs/remotes/origin/master"]), base, "only a fetch moves it");

  let (exit_code, updated) = run_in(&repo, &["update"]);
  assert_eq!(exit_code, 0, "{updated}");
  assert_eq!(git(&repo, &["rev-parse", "refs/remotes/origin/master"]), copy);
  assert_eq!(git(&repo, &["log", "--format=%s", "origin/master..HEAD"]), "", "the original whose copy landed is dropped");
}

/// A target that moved since the copy was built on it refuses the push as no fast-forward: `baseline_moved`, master
/// fetched, `bd update` the next step; the server refusing it is `baseline_refused` in its own words
#[test_log::test]
fn a_moved_target_is_baseline_moved_and_a_refusal_of_the_server_baseline_refused() {
  let (repo, remote) = landing_by_push();
  let theirs = push_from_elsewhere_to(&remote, "master", "Their fix");
  let (exit_code, moved) = run_in(&repo, &["land", "auth", "--without-checks"]);
  assert_error(&moved, exit_code, (3, "baseline_moved"));
  assert_eq!(next_steps(&moved["error"]), [("bd update".to_string(), true)]);
  assert_eq!(git(&repo, &["rev-parse", "refs/remotes/origin/master"]), theirs, "the push fetched it");

  let (repo, remote) = landing_by_push();
  pre_receive_hook(remote.path(), "echo 'master is protected' >&2\nexit 1");
  let (exit_code, refused) = run_in(&repo, &["land", "auth", "--without-checks"]);
  assert_error(&refused, exit_code, (4, "baseline_refused"));
  assert!(refused["error"]["message"].as_str().unwrap().contains("master is protected"), "{refused}");
}

/// A fetched master with a commit the branch is not built on refuses the push before it is sent, as `baseline_moved`
/// with `bd update` next: the copy is built on the fork point, so the push would be no fast-forward; `--check` names
/// the update as the step
#[test_log::test]
fn a_fetched_target_ahead_of_the_branch_refuses_the_push_before_it_is_sent() {
  let (repo, remote) = landing_by_push();
  let theirs = push_from_elsewhere_to(&remote, "master", "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let (exit_code, check) = run_in(&repo, &["land", "auth", "--check"]);
  assert_eq!(exit_code, 0, "{check}");
  assert_eq!(
    (&check["next_step"]["row"], &check["next_step"]["step"]),
    (&json!("push′"), &json!("update_project")),
    "{check}"
  );
  assert_eq!(check["refusals"][0]["code"], json!("baseline_moved"), "{check}");
  let (exit_code, refused) = run_in(&repo, &["land", "auth", "--without-checks"]);
  assert_error(&refused, exit_code, (3, "baseline_moved"));
  assert_eq!(next_steps(&refused["error"]), [("bd update".to_string(), true)]);
  assert_eq!(remote_tip(&remote, "master"), theirs, "nothing was pushed");
}

/// The merge of a pull request on GitHub is no route of `bd land` yet; Patronus off Space neither
#[test_log::test]
fn a_repository_that_lands_through_its_pull_request_is_not_landed_by_bd_yet() {
  let (repo, _remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["land", "auth"]);
  assert_error(&output, exit_code, (4, "landing_route_unsupported"));
  git(&repo, &["config", "branchdeck.landing", "patronus"]);
  let (exit_code, output) = run_in(&repo, &["land", "auth", "--check"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("not_on_space")), "{output}");
}

/// `bd status` decides the steps by the route of the repository, as `bd land --check` does: a branch of a repository
/// that lands by a push has no Dry Run of Patronus, and its push into master waits for the pull requests GitHub lists
/// (whether CI passed on its tip), which `bd status` doesn't read
#[test_log::test]
fn bd_status_decides_the_steps_by_the_route_of_the_repository() {
  let (repo, _remote) = landing_by_push();
  let (exit_code, synced) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{synced}");
  let (exit_code, status) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{status}");
  let step = &status["branches"][0]["next_step"];
  assert_eq!((&step["step"], &step["waits_for"]), (&json!("pending"), &json!("pull_requests")), "{status}");
  assert_eq!(
    step["dry_run_blocked"],
    json!("a Dry Run is a robot of Patronus, and the repository lands by a push into its target")
  );
}
