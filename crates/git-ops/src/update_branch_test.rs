use crate::update_branch::{
  CheckPurpose, PENDING_UPDATE_PREFIX, UpstreamRef, changed_paths, check_update, finish_pending_update, index_left_behind, no_landed_candidates, previous_upstream_tips,
};
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

/// The tracking ref comes from the configuration: in place of a ref name, git would read an option, a range or a
/// revision expression
#[test]
fn a_tracking_ref_that_is_not_a_ref_name_is_refused() {
  let repo = TestRepo::new();
  diverged(&repo);
  for tracking_ref in ["--all", "refs/remotes/origin/master..HEAD", "refs/remotes/origin/master@{1}", "HEAD~1", "origin/master"] {
    let error = check_update(repo.git_executor(), repo.path_str(), upstream(tracking_ref), CheckPurpose::Preview, &no_landed_candidates).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("is not a full ref name"), "{tracking_ref}: {message}");
  }
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

/// A path as Linux allows it and APFS refuses: Latin-1, not UTF-8
const LATIN_1_PATH: &[u8] = b"dir/caf\xe9.txt";

/// The tree of `content` at `LATIN_1_PATH`, written without the file system
fn latin_1_tree(repo: &TestRepo, content: &str) -> (String, String) {
  let git = repo.git_executor();
  let blob = git.execute_command_with_input(&["hash-object", "-w", "--stdin"], repo.path_str(), content).unwrap();
  let entry = |name: &[u8], mode_type_id: String| [mode_type_id.as_bytes(), b"\t", name, b"\0"].concat();
  let dir = git
    .execute_command_with_input_bytes(&["mktree", "-z"], repo.path_str(), &entry(b"caf\xe9.txt", format!("100644 blob {blob}")))
    .unwrap();
  let dir = String::from_utf8(dir).unwrap().trim().to_string();
  let tree = git
    .execute_command_with_input_bytes(&["mktree", "-z"], repo.path_str(), &entry(b"dir", format!("040000 tree {dir}")))
    .unwrap();
  (String::from_utf8(tree).unwrap().trim().to_string(), blob)
}

#[test]
fn a_path_that_is_not_utf8_names_its_index_entry() {
  let repo = TestRepo::new();
  repo.create_commit("one", "a.txt", "a\n");
  let (old_tree, old_blob) = latin_1_tree(&repo, "old\n");
  let (new_tree, _) = latin_1_tree(&repo, "new\n");
  let changed = changed_paths(repo.git_executor(), repo.path_str(), &old_tree, &new_tree).unwrap();
  assert_eq!(changed.iter().map(|c| c.path.as_slice()).collect::<Vec<_>>(), [LATIN_1_PATH]);

  // the index still holds the old side of the path: the files did not follow
  let entry = [format!("100644 {old_blob}\t").as_bytes(), LATIN_1_PATH, b"\0"].concat();
  repo
    .git_executor()
    .execute_command_with_input_bytes(&["update-index", "-z", "--index-info"], repo.path_str(), &entry)
    .unwrap();
  assert!(index_left_behind(repo.git_executor(), repo.path_str(), &changed).unwrap());
}
