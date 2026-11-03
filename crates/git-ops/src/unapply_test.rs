use crate::notes::{MappingNote, notes_ref, read_notes, write_commit_notes};
use crate::revs::ref_exists;
use crate::revs::resolve_tree_id;
use crate::rewrite_guard::HeadMoved;
use crate::unapply::{
  DropKind, DropPlan, DropRefused, MoveBlocked, UnappliedMove, apply_drop, apply_drop_undo, apply_unapply, apply_unapply_undo, check_drop, check_drop_undo, check_unapply_undo,
};
use crate::update_branch::{PENDING_UPDATE_PREFIX, UndoCheck, finish_pending_update};
use domain_types::commit::NoteSource;
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
  let error = check_drop(repo.git_executor(), repo.path_str(), commits, DropKind::Unapply).unwrap_err();
  error.downcast_ref::<MoveBlocked>().cloned().unwrap_or_else(|| panic!("not blocked: {error:#}"))
}

fn checked(repo: &TestRepo, commits: &[String]) -> DropPlan {
  check_drop(repo.git_executor(), repo.path_str(), commits, DropKind::Unapply).unwrap()
}

/// The refusal of a drop of `commits`, with nothing changed and no temporary ref left
fn drop_refused(repo: &TestRepo, commits: &[String]) -> DropRefused {
  let head = repo.head();
  let error = check_drop(repo.git_executor(), repo.path_str(), commits, DropKind::Drop).unwrap_err();
  assert_eq!(repo.head(), head);
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");
  error.downcast_ref::<DropRefused>().cloned().unwrap_or_else(|| panic!("not a refused drop: {error:#}"))
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
  let error = check_drop(repo.git_executor(), repo.path_str(), &[add_b], DropKind::Unapply).unwrap_err();
  assert!(error.downcast_ref::<DropRefused>().is_some(), "{error:#}");
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
  let (repo, head, new_head, _tip) = unapplied();
  repo.create_commit("(other) more", "d.txt", "d\n");
  let error = check_unapply_undo(repo.git_executor(), repo.path_str(), &head, &new_head).unwrap_err();
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
}

#[test]
fn an_interrupted_unapply_finishes_its_files() {
  let (repo, commits, _) = repo_with_feature();
  let head = repo.head();
  let plan = checked(&repo, &commits);
  assert_an_interrupted_move_finishes(&repo, &head, &plan, "branch-deck: unapply");
}

/// A crash after the transaction of `plan` (reflog `message`): HEAD moved, the mark is set, the files are where they
/// were; the finish lets them follow and removes the mark
fn assert_an_interrupted_move_finishes(repo: &TestRepo, head: &str, plan: &DropPlan, message: &str) {
  let worktree_mark = format!("{PENDING_UPDATE_PREFIX}main");
  repo.git(&["update-ref", "--create-reflog", "-m", message, "refs/heads/master", plan.new_head(), head]);
  repo.git(&["update-ref", &worktree_mark, head]);

  let finished = finish_pending_update(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(finished, Some((head.to_string(), plan.new_head().to_string())));
  assert_eq!(read(repo, "b.txt"), None);
  assert_eq!(repo.git(&["status", "--porcelain"]), "");
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &worktree_mark));
  plan.discard(repo.git_executor(), repo.path_str());
}

/// The mapping note a sync writes on `commit`: copied as `mapped` from the trees of the commit and of its parent
fn mapping_note(repo: &TestRepo, commit: &str, mapped: &str) -> MappingNote {
  MappingNote {
    original: commit.to_string(),
    mapped: mapped.to_string(),
    source: NoteSource {
      parent_tree: resolve_tree_id(repo.git_executor(), repo.path_str(), &format!("{commit}^")).unwrap(),
      tree: resolve_tree_id(repo.git_executor(), repo.path_str(), commit).unwrap(),
    },
    resolved: None,
  }
}

#[test]
fn a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back() {
  let (repo, commits, tip) = repo_with_feature();
  let head = repo.head();
  // a change on a path the drop does not write stays
  std::fs::write(repo.path().join("a.txt"), "mine\n").unwrap();

  let plan = check_drop(repo.git_executor(), repo.path_str(), &commits[..1], DropKind::Drop).unwrap();
  assert!(plan.files_change());
  apply_drop(repo.git_executor(), repo.path_str(), &plan).unwrap();
  let new_head = repo.head();
  assert_eq!(new_head, plan.new_head());
  assert_eq!(repo.git(&["log", "--format=%s"]), "(feature) change c\n(other) change a\nbase");
  assert_eq!(read(&repo, "b.txt"), None);
  assert_eq!(repo.git(&["status", "--porcelain"]), "M a.txt");
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: drop");
  // a drop moves no virtual branch: the next sync builds it anew
  assert_eq!(repo.rev_parse(VIRTUAL).unwrap(), tip);
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");

  // an unapply's undo does not take a drop back, the drop's does
  let error = check_unapply_undo(repo.git_executor(), repo.path_str(), &head, &new_head).unwrap_err();
  assert!(format!("{error:#}").contains("only the last unapply can be undone"), "{error:#}");
  let UndoCheck::Ready(undo) = check_drop_undo(repo.git_executor(), repo.path_str(), DropKind::Drop, &head, &new_head).unwrap() else {
    panic!("not ready");
  };
  apply_drop_undo(repo.git_executor(), repo.path_str(), &undo).unwrap();
  assert_eq!(repo.head(), head);
  assert_eq!(read(&repo, "b.txt").as_deref(), Some("b\n"));
  assert_eq!(repo.git(&["status", "--porcelain"]), "M a.txt");
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: undo drop");
}

/// A commit and its revert leave HEAD's tree as it is: the files stay, and a later commit whose trees stay keeps its
/// mapping note, so a sync keeps its virtual copy
#[test]
fn a_drop_of_a_commit_and_its_revert_moves_no_file_and_a_later_commit_keeps_its_note() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let added = repo.create_commit("(feature) add b", "b.txt", "b\n");
  let other = repo.create_commit("(other) add c", "c.txt", "c\n");
  repo.git(&["revert", "--no-edit", &added]);
  let revert = repo.head();
  let after = repo.create_commit("(other) add d", "d.txt", "d\n");
  let notes = [mapping_note(&repo, &other, &"1".repeat(40)), mapping_note(&repo, &after, &"2".repeat(40))];
  write_commit_notes(repo.git_executor(), repo.path_str(), &notes).unwrap();
  let head_tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
  std::fs::write(repo.path().join("b.txt"), "untracked where the dropped commit was\n").unwrap();

  let plan = check_drop(repo.git_executor(), repo.path_str(), &[added, revert], DropKind::Drop).unwrap();
  assert!(!plan.files_change());
  plan.write_carried_notes(repo.git_executor(), repo.path_str());
  apply_drop(repo.git_executor(), repo.path_str(), &plan).unwrap();

  assert_eq!(repo.git(&["log", "--format=%s"]), "(other) add d\n(other) add c\nbase");
  assert_eq!(repo.git(&["rev-parse", "HEAD^{tree}"]), head_tree);
  assert_eq!(read(&repo, "b.txt").as_deref(), Some("untracked where the dropped commit was\n"));
  let new_ids = repo.git(&["rev-list", "-2", "HEAD"]);
  let new_ids: Vec<&str> = new_ids.lines().collect();
  let notes_ref = notes_ref(repo.git_executor(), repo.path_str()).unwrap();
  let carried = read_notes(repo.git_executor(), repo.path_str(), &notes_ref, &new_ids).unwrap();
  // the commit between the pair lost the dropped file from its trees, so its copy is made again; the one after the
  // revert has its trees, so it keeps its copy
  assert_eq!(carried.keys().collect::<Vec<_>>(), [new_ids[0]]);
  assert_eq!(carried[new_ids[0]], notes[1].content());
}

#[test]
fn a_drop_is_refused_by_a_later_commit_that_depends_on_it_would_become_empty_or_by_the_root() {
  let repo = TestRepo::new();
  let root = repo.create_commit("base", "a.txt", "a\n");
  let added = repo.create_commit("(feature) add b", "b.txt", "b\n");
  let changed = repo.create_commit("(feature) change b", "b.txt", "b2\n");
  assert!(drop_refused(&repo, std::slice::from_ref(&added)).0.contains("depend on its changes"));

  // the revert of a dropped commit would hold no change: git replay would leave it out with its message
  repo.git(&["revert", "--no-edit", &changed]);
  let emptied = drop_refused(&repo, std::slice::from_ref(&changed)).0;
  assert!(emptied.contains("would become empty: drop it too"), "{emptied}");

  let root_refused = drop_refused(&repo, &[root]).0;
  assert!(root_refused.contains("root commit"), "{root_refused}");
}

/// A staged change on a path the drop changes refuses it, and the refusal names that path as git stores it (a quote in
/// the name included), not the paths whose staged changes the drop keeps
#[test]
fn a_drop_under_a_staged_change_names_the_path() {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let dropped = repo.create_commit("Add it's", "it's.txt", "one");
  repo.create_commit("Add other", "other.txt", "other");
  std::fs::write(repo.path().join("it's.txt"), "staged").unwrap();
  std::fs::write(repo.path().join("base.txt"), "staged too").unwrap();
  repo.git(&["add", "it's.txt", "base.txt"]);
  let head = repo.head();

  let error = check_drop(repo.git_executor(), repo.path_str(), &[dropped], DropKind::Drop).unwrap_err();
  let blocked = error.downcast_ref::<MoveBlocked>().unwrap_or_else(|| panic!("not blocked: {error:#}"));
  assert_eq!(blocked.paths, ["it's.txt"]);
  assert_eq!(repo.head(), head);
  assert_eq!(repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"]), "");
}

/// The guards of a rewrite refuse the drop while a merge is in progress, before anything is written
#[test]
fn a_drop_is_refused_during_a_merge() {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.rs", "// base");
  let commit = repo.create_commit("(feature) Add login", "login.rs", "// login");
  std::fs::write(repo.path().join(".git/MERGE_HEAD"), format!("{commit}\n")).unwrap();

  let error = check_drop(repo.git_executor(), repo.path_str(), std::slice::from_ref(&commit), DropKind::Drop).unwrap_err();
  assert!(format!("{error:#}").contains("a merge is in progress"), "{error:#}");
  assert_eq!(repo.head(), commit);
}

#[test]
fn an_interrupted_drop_finishes_its_files() {
  let (repo, commits, _) = repo_with_feature();
  let head = repo.head();
  let plan = check_drop(repo.git_executor(), repo.path_str(), &commits[..1], DropKind::Drop).unwrap();
  assert_an_interrupted_move_finishes(&repo, &head, &plan, "branch-deck: drop");
}
