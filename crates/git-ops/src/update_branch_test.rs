use crate::update_branch::{CheckPurpose, PENDING_UPDATE_PREFIX, UpstreamRef, check_update, finish_pending_update, no_landed_candidates, previous_upstream_tips};
use git_executor::git_command_executor::GitCommandExecutor;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const TRACKING_REF: &str = "refs/remotes/origin/master";

/// `master` with one local commit on `base`, and `origin/master` with two upstream commits on it; returns the first
/// upstream commit
fn diverged(repo: &TestRepo) -> String {
  repo.create_commit("base", "base.txt", "base\n");
  repo.checkout_new_branch("upstream").unwrap();
  let first = repo.create_commit("upstream 1", "upstream.txt", "one\n");
  let tip = repo.create_commit("upstream 2", "upstream.txt", "two\n");
  repo.git(&["update-ref", TRACKING_REF, &tip]);
  repo.checkout("master").unwrap();
  repo.create_commit("local", "local.txt", "local\n");
  first
}

fn upstream(tracking_ref: &str) -> UpstreamRef<'_> {
  UpstreamRef {
    tracking_ref,
    name: "origin/master",
    previous: None,
  }
}

/// Delete the loose object of `id`: the history through it can't be read
fn delete_object(repo: &TestRepo, id: &str) {
  let path = repo.git(&["rev-parse", "--path-format=absolute", "--git-path", &format!("objects/{}/{}", &id[..2], &id[2..])]);
  std::fs::remove_file(path).unwrap();
}

#[test]
fn a_history_git_cannot_read_is_an_error_with_its_message_not_unrelated_histories() {
  let repo = TestRepo::new();
  let unreadable = diverged(&repo);
  delete_object(&repo, &unreadable);
  // `merge-base` exits with 1 only when there is no common history; here it fails reading the upstream
  let error = check_update(repo.git_executor(), repo.path_str(), upstream(TRACKING_REF), CheckPurpose::Preview, &no_landed_candidates).unwrap_err();
  let message = format!("{error:#}");
  assert!(!message.contains("no common history"), "{message}");
  assert!(message.contains(&unreadable), "{message}");
}

#[test]
fn unrelated_histories_have_no_common_history() {
  let repo = TestRepo::new();
  repo.create_commit("local", "local.txt", "local\n");
  repo.git(&["checkout", "--orphan", "other"]);
  let other = repo.create_commit("other", "other.txt", "other\n");
  repo.git(&["update-ref", TRACKING_REF, &other]);
  repo.checkout("master").unwrap();
  let error = check_update(repo.git_executor(), repo.path_str(), upstream(TRACKING_REF), CheckPurpose::Preview, &no_landed_candidates).unwrap_err();
  assert_eq!(format!("{error:#}"), "master and origin/master have no common history");
}

#[test]
fn the_fork_point_of_a_tracking_ref_that_is_gone_is_an_error() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "base.txt", "base\n");
  let head = repo.create_commit("local", "local.txt", "local\n");
  // `merge-base --fork-point` exits with 1 without a reflog or a fork point, with 128 for a ref that is not there
  let error = previous_upstream_tips(repo.git_executor(), repo.path_str(), upstream("refs/remotes/origin/gone"), &head, &base).unwrap_err();
  assert!(format!("{error:#}").contains("refs/remotes/origin/gone"), "{error:#}");
  // the fork point is the merge base: no previous tip
  repo.git(&["update-ref", TRACKING_REF, &base]);
  assert_eq!(
    previous_upstream_tips(repo.git_executor(), repo.path_str(), upstream(TRACKING_REF), &head, &base).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn finishing_an_interrupted_update_whose_files_followed_reads_the_worktree_once() {
  let repo = TestRepo::new();
  let from = repo.create_commit("one", "a.txt", "one\n");
  let to = repo.create_commit("two", "a.txt", "two\n");
  // the update moved the branch from `from` to `to`, and the files followed, but the mark stayed
  repo.git(&["update-ref", "refs/heads/master", &from]);
  repo.git(&["update-ref", "-m", "branch-deck: update project", "HEAD", &to, &from]);
  let mark = format!("{PENDING_UPDATE_PREFIX}main");
  repo.git(&["update-ref", &mark, &from]);

  let git = GitCommandExecutor::new().with_process_stats("test");
  assert_eq!(finish_pending_update(&git, repo.path_str()).unwrap(), Some((from, to)));
  assert!(!repo.git_executor().ref_exists(repo.path_str(), &mark));
  let by_command = git.process_stats().unwrap().by_command();
  // the worktree, the mark, the guard's operation markers, HEAD and the branch's previous value
  assert_eq!(by_command.get("rev-parse").map(|stats| stats.count), Some(5), "{by_command:#?}");
}
