use crate::revs::ref_exists;
use crate::rewrite_guard::HeadMoved;
use crate::unapply::{MoveBlocked, UnappliedMove, UnapplyPlan, apply_unapply, apply_unapply_undo, check_unapply, check_unapply_undo};
use crate::update_branch::{PENDING_UPDATE_PREFIX, UndoCheck, finish_pending_update};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const VIRTUAL: &str = "refs/heads/user/virtual/feature";
const UNAPPLIED: &str = "refs/heads/user/unapplied/feature";

/// `base`, then `(feature) add b` (adds `b.txt`), `(other) change a`, `(feature) change c`; the virtual branch of
/// `feature` points at a commit of its own. Returns the two feature commits and the virtual tip
fn repo_with_feature() -> (TestRepo, Vec<String>, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("base", &[("a.txt", "a\n"), ("c.txt", "c\n")]);
  let add_b = repo.create_commit("(feature) add b", "b.txt", "b\n");
  repo.create_commit("(other) change a", "a.txt", "a2\n");
  let change_c = repo.create_commit("(feature) change c", "c.txt", "c2\n");
  // the virtual branch: a copy chain on the base, as a sync writes it
  let tip = repo.git(&[
    "commit-tree",
    "-p",
    &base,
    "-m",
    "add b and change c",
    &repo.git(&["rev-parse", &format!("{change_c}^{{tree}}")]),
  ]);
  repo.git(&["update-ref", VIRTUAL, &tip]);
  (repo, vec![add_b, change_c], tip)
}

fn refs(tip: &str) -> UnappliedMove<'_> {
  UnappliedMove {
    virtual_ref: VIRTUAL,
    unapplied_ref: UNAPPLIED,
    tip,
  }
}

fn read(repo: &TestRepo, file: &str) -> Option<String> {
  std::fs::read_to_string(repo.path().join(file)).ok()
}

fn blocked(repo: &TestRepo, commits: &[String]) -> MoveBlocked {
  let error = check_unapply(repo.git_executor(), repo.path_str(), commits).unwrap_err();
  error.downcast_ref::<MoveBlocked>().cloned().unwrap_or_else(|| panic!("not blocked: {error:#}"))
}

fn checked(repo: &TestRepo, commits: &[String]) -> UnapplyPlan {
  check_unapply(repo.git_executor(), repo.path_str(), commits).unwrap()
}

#[test]
fn the_files_follow_the_unapply_and_the_virtual_branch_moves_in_the_same_transaction() {
  let (repo, commits, tip) = repo_with_feature();
  let head = repo.head();
  // a change on a path the unapply does not write stays, staged and unstaged
  std::fs::write(repo.path().join("a.txt"), "mine\n").unwrap();
  std::fs::write(repo.path().join("staged.txt"), "staged\n").unwrap();
  repo.git(&["add", "staged.txt"]);

  let plan = checked(&repo, &commits);
  assert_eq!(plan.head(), head);
  // a check writes nothing but objects
  assert_eq!(repo.head(), head);
  assert!(ref_exists(repo.git_executor(), repo.path_str(), VIRTUAL));

  apply_unapply(repo.git_executor(), repo.path_str(), &plan, &refs(&tip)).unwrap();
  assert_eq!(repo.head(), plan.new_head());
  assert_eq!(repo.git(&["log", "--format=%s", "-n2"]), "(other) change a\nbase");
  assert_eq!(read(&repo, "b.txt"), None);
  assert_eq!(read(&repo, "c.txt").as_deref(), Some("c\n"));
  assert_eq!(read(&repo, "a.txt").as_deref(), Some("mine\n"));
  // `git` trims the leading space of ` M`
  assert_eq!(repo.git(&["status", "--porcelain"]), "M a.txt\nA  staged.txt");
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), VIRTUAL));
  assert_eq!(repo.rev_parse(UNAPPLIED).unwrap(), tip);
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: unapply");
  // no temporary ref and no mark of pending files is left
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");
}

#[test]
fn a_local_change_on_a_path_the_unapply_writes_refuses_it_staged_or_not() {
  let (repo, commits, _) = repo_with_feature();
  let head = repo.head();
  std::fs::write(repo.path().join("c.txt"), "edited\n").unwrap();
  assert_eq!(blocked(&repo, &commits).paths, ["c.txt"]);
  repo.git(&["add", "c.txt"]);
  assert_eq!(blocked(&repo, &commits).paths, ["c.txt"]);
  // nothing changed, and no temporary ref is left
  assert_eq!(repo.head(), head);
  assert_eq!(read(&repo, "c.txt").as_deref(), Some("edited\n"));
  assert!(ref_exists(repo.git_executor(), repo.path_str(), VIRTUAL));
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");
}

#[test]
fn a_file_changed_after_the_check_moves_head_and_the_branch_back() {
  let (repo, commits, tip) = repo_with_feature();
  let head = repo.head();
  let plan = checked(&repo, &commits);
  std::fs::write(repo.path().join("b.txt"), "edited since the check\n").unwrap();

  let error = apply_unapply(repo.git_executor(), repo.path_str(), &plan, &refs(&tip)).unwrap_err();
  assert!(format!("{error:#}").contains("nothing was changed"), "{error:#}");
  assert_eq!(repo.head(), head);
  assert_eq!(read(&repo, "b.txt").as_deref(), Some("edited since the check\n"));
  assert_eq!(repo.rev_parse(VIRTUAL).unwrap(), tip);
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), UNAPPLIED));
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", PENDING_UPDATE_PREFIX]), "");
}

#[test]
fn a_conflict_of_the_drop_changes_nothing() {
  let (repo, _, _) = repo_with_feature();
  // a later commit changes the file the dropped one added
  let add_b = repo.git(&["log", "--format=%H", "--grep=add b", "-n1"]);
  repo.create_commit("(other) change b", "b.txt", "b2\n");
  let head = repo.head();
  let error = check_unapply(repo.git_executor(), repo.path_str(), &[add_b]).unwrap_err();
  assert!(error.downcast_ref::<MoveBlocked>().is_none(), "{error:#}");
  assert_eq!(repo.head(), head);
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");
}

/// The repository of `repo_with_feature` after the unapply of its feature commits: HEAD before and after, the tip
fn unapplied() -> (TestRepo, String, String, String) {
  let (repo, commits, tip) = repo_with_feature();
  let head = repo.head();
  let plan = checked(&repo, &commits);
  apply_unapply(repo.git_executor(), repo.path_str(), &plan, &refs(&tip)).unwrap();
  let new_head = repo.head();
  (repo, head, new_head, tip)
}

#[test]
fn the_undo_brings_the_commits_their_files_and_the_virtual_branch_back() {
  let (repo, head, new_head, tip) = unapplied();

  let UndoCheck::Ready(undo) = check_unapply_undo(repo.git_executor(), repo.path_str(), &head, &new_head).unwrap() else {
    panic!("not ready");
  };
  assert_eq!(undo.blocked(), [] as [String; 0]);
  apply_unapply_undo(repo.git_executor(), repo.path_str(), &undo, &refs(&tip)).unwrap();
  assert_eq!(repo.head(), head);
  assert_eq!(read(&repo, "b.txt").as_deref(), Some("b\n"));
  assert_eq!(read(&repo, "c.txt").as_deref(), Some("c2\n"));
  assert_eq!(repo.git(&["status", "--porcelain"]), "");
  assert_eq!(repo.rev_parse(VIRTUAL).unwrap(), tip);
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), UNAPPLIED));
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: undo unapply");

  // undone already: no error, nothing to do
  assert!(matches!(
    check_unapply_undo(repo.git_executor(), repo.path_str(), &head, &new_head).unwrap(),
    UndoCheck::AlreadyUndone { .. }
  ));
}

#[test]
fn the_undo_of_an_unapply_that_is_not_the_last_move_is_refused() {
  let (repo, head, new_head, tip) = unapplied();
  repo.create_commit("(other) more", "d.txt", "d\n");
  let error = check_unapply_undo(repo.git_executor(), repo.path_str(), &head, &new_head).unwrap_err();
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
}

#[test]
fn an_interrupted_unapply_finishes_its_files() {
  let (repo, commits, _) = repo_with_feature();
  let head = repo.head();
  let plan = checked(&repo, &commits);
  // a crash after the transaction: HEAD and the refs moved, the mark is set, the files are where they were
  let worktree_mark = format!("{PENDING_UPDATE_PREFIX}main");
  repo.git(&["update-ref", "--create-reflog", "-m", "branch-deck: unapply", "refs/heads/master", plan.new_head(), &head]);
  repo.git(&["update-ref", &worktree_mark, &head]);

  let finished = finish_pending_update(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(finished, Some((head, plan.new_head().to_string())));
  assert_eq!(read(&repo, "b.txt"), None);
  assert_eq!(repo.git(&["status", "--porcelain"]), "");
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &worktree_mark));
  plan.discard(repo.git_executor(), repo.path_str());
}
