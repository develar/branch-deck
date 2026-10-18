use crate::pick::{FollowState, PickRefusal, PickRequest, PickSource, PickState, apply_pick, check_pick, check_pick_undo};
use crate::revs::ref_exists;
use crate::rewrite_guard::HeadMoved;
use crate::update_branch::{GuardRefusal, PENDING_UPDATE_PREFIX, UndoCheck, apply_update_undo, finish_pending_update};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, TestWorktree};

const TRACKING_REF: &str = "refs/remotes/origin/master";

fn from(worktree: &TestWorktree, follow: bool) -> PickRequest<'_> {
  PickRequest {
    source: PickSource::Worktree(&worktree.path),
    upstream: None,
    follow,
  }
}

fn read(repo: &TestRepo, file: &str) -> String {
  std::fs::read_to_string(repo.path().join(file)).unwrap()
}

fn subjects(repo: &TestRepo, range: &str) -> Vec<String> {
  repo.git(&["log", "--reverse", "--format=%s", range]).lines().map(str::to_string).collect()
}

/// The refusal of a pick from `worktree` onto `repo`, whose baseline is `TRACKING_REF`
fn refusal_with_upstream(repo: &TestRepo, worktree: &TestWorktree) -> PickRefusal {
  let request = PickRequest {
    upstream: Some(TRACKING_REF),
    ..from(worktree, false)
  };
  let error = check_pick(repo.git_executor(), repo.path_str(), &request).unwrap_err();
  error.downcast_ref::<PickRefusal>().cloned().unwrap_or_else(|| panic!("not a refusal: {error:#}"))
}

/// Delete the reflog of the target branch: its fork points are gone
fn expire_reflog(repo: &TestRepo) {
  repo.git(&["reflog", "expire", "--expire=now", "--expire-unreachable=now", "refs/heads/master"]);
}

#[test]
fn a_pick_lands_the_commits_of_a_worktree_and_the_worktree_follows() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let first = worktree.commit("(lane) first", "b.txt", "b\n");
  worktree.commit("(lane) second", "a.txt", "a2\n");
  // the target moved on: the copies are new commits (in the same second, a replay onto the source's own base gives the
  // source's commits back, and the worktree would have nowhere to move)
  let own = repo.create_commit("(own) other", "own.txt", "own\n");
  // a dirty file the pick doesn't write stays as it is
  std::fs::write(repo.path().join("notes.txt"), "mine\n").unwrap();

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(plan.check.base, base);
  assert_eq!(
    plan.check.picked.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(),
    ["(lane) first", "(lane) second"]
  );
  assert_eq!(plan.check.picked[0].commit, first);
  assert_eq!(plan.check.present, []);
  let follow = plan.check.follow.clone().unwrap();
  assert!(follow.detached);
  assert_eq!(follow.worktree_id, "lane");
  // a check writes nothing but objects
  assert_eq!(repo.head(), own);

  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(applied.followed, Some(FollowState::Moved));
  assert_eq!(repo.head(), applied.new_head);
  assert_eq!(subjects(&repo, &format!("{base}..HEAD")), ["(own) other", "(lane) first", "(lane) second"]);
  assert_eq!(read(&repo, "a.txt"), "a2\n");
  assert_eq!(read(&repo, "b.txt"), "b\n");
  assert_eq!(read(&repo, "notes.txt"), "mine\n");
  assert_eq!(repo.git(&["status", "--porcelain", "--untracked-files=no"]), "");
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: pick");
  // the worktree is at the new tip, still detached, its files with it
  assert_eq!(worktree.head(), applied.new_head);
  assert_eq!(worktree.git(&["status", "--porcelain"]), "");
  assert_eq!(worktree.git(&["branch", "--show-current"]), "");
  assert_eq!(worktree.git(&["log", "-g", "-n1", "--format=%gs", "HEAD"]), "branch-deck: pick");

  // the same pick again has nothing left to do
  let again = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert!(again.check.commits.is_empty(), "{:?}", again.check.commits);
  assert_eq!(again.check.new_head, applied.new_head);
}

#[test]
fn a_commit_the_target_has_already_is_present_with_the_target_commit_of_its_patch() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let landed = worktree.commit("(lane) landed", "b.txt", "b\n");
  let same_content = worktree.commit("(lane) same content", "c.txt", "c\n");
  worktree.commit("(lane) new", "d.txt", "d\n");
  // the target has the first by cherry-pick (`-x`: in the same second, a plain pick would be the same commit), and the
  // change of the second inside a larger commit
  repo.git(&["cherry-pick", "-x", &landed]);
  let copy = repo.head();
  repo.create_commit_with_files("(other) bigger", &[("c.txt", "c\n"), ("e.txt", "e\n")]);

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(plan.check.picked.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["(lane) new"]);
  let present: Vec<(&str, Option<&str>)> = plan.check.present.iter().map(|commit| (commit.commit.as_str(), commit.on.as_deref())).collect();
  assert_eq!(present, [(landed.as_str(), Some(copy.as_str())), (same_content.as_str(), None)]);
  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(read(&repo, "d.txt"), "d\n");
}

#[test]
fn a_target_rewritten_after_the_worktree_was_cut_from_it_gives_back_none_of_its_old_commits() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  repo.create_commit("(own) local", "own.txt", "one\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) new", "b.txt", "b\n");
  // the owner rewrites the local commit: a new date and a folded fixup change its id and its patch
  repo.git(&["commit", "--amend", "--no-edit", "--date=2001-01-01T00:00:00Z"]);
  repo.create_commit("fixup! (own) local", "own.txt", "two\n");
  repo.git(&["reset", "--soft", "HEAD~2"]);
  repo.git(&["commit", "-q", "-m", "(own) local"]);

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(plan.check.commits.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["(lane) new"]);
  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(read(&repo, "own.txt"), "two\n");
  assert_eq!(read(&repo, "b.txt"), "b\n");
}

#[test]
fn without_a_fork_point_a_local_shared_history_is_refused_and_a_range_names_the_commits() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  repo.git(&["update-ref", TRACKING_REF, &base]);
  repo.create_commit("(own) one", "own.txt", "one\n");
  repo.create_commit("(own) two", "two.txt", "two\n");
  let worktree = TestWorktree::detached(&repo);
  let new = worktree.commit("(lane) new", "b.txt", "b\n");
  repo.git(&["commit", "--amend", "-q", "-m", "(own) two, reworded"]);
  expire_reflog(&repo);

  assert!(matches!(refusal_with_upstream(&repo, &worktree), PickRefusal::RangeUnknown(_)));

  for revisions in [new.clone(), format!("{new}^..{new}")] {
    let request = PickRequest {
      source: PickSource::Revisions(&revisions),
      upstream: Some(TRACKING_REF),
      follow: false,
    };
    let plan = check_pick(repo.git_executor(), repo.path_str(), &request).unwrap();
    assert_eq!(
      plan.check.picked.iter().map(|commit| commit.commit.as_str()).collect::<Vec<_>>(),
      [new.as_str()],
      "{revisions}"
    );
  }
}

#[test]
fn a_worktree_on_a_newer_baseline_is_refused() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  repo.create_commit("(own) local", "own.txt", "one\n");
  let worktree = TestWorktree::detached(&repo);
  // the baseline moved on, and the worktree stands on it
  worktree.git(&["checkout", "-q", "--detach", &base]);
  let upstream = worktree.commit("upstream", "up.txt", "up\n");
  repo.git(&["update-ref", TRACKING_REF, &upstream]);
  worktree.commit("(lane) new", "b.txt", "b\n");

  assert!(matches!(refusal_with_upstream(&repo, &worktree), PickRefusal::SourceOnNewerBaseline(_)));
}

#[test]
fn a_conflict_names_the_first_conflicting_commit_and_moves_nothing() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) clean", "b.txt", "b\n");
  let conflicting = worktree.commit("(lane) conflicting", "a.txt", "lane\n");
  worktree.commit("(lane) after", "c.txt", "c\n");
  let head = repo.create_commit("(own) same line", "a.txt", "own\n");

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  let PickState::Conflict(conflict) = &plan.check.state else {
    panic!("not a conflict: {:?}", plan.check.state);
  };
  assert_eq!(conflict.commit.id, conflicting);
  assert_eq!(conflict.clean.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["(lane) clean"]);
  assert_eq!(conflict.files, ["a.txt"]);
  assert_eq!(plan.check.new_head, head);
  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap_err();
  assert_eq!(repo.head(), head);
}

#[test]
fn local_changes_on_a_path_the_pick_writes_block_it_in_either_worktree() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) change", "a.txt", "lane\n");
  std::fs::write(repo.path().join("a.txt"), "mine\n").unwrap();

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(
    plan.check.state,
    PickState::Blocked {
      worktree: repo.path_str().to_string(),
      paths: vec!["a.txt".to_string()],
      path_count: 1,
    }
  );
  repo.git(&["checkout", "--", "a.txt"]);

  // the source follows onto a commit of the target, and a draft of that file in the source blocks the follow
  repo.create_commit("(own) other", "own.txt", "own\n");
  worktree.write("own.txt", "lane draft\n");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  let PickState::Blocked { worktree: blocked, paths, .. } = &plan.check.state else {
    panic!("not blocked: {:?}", plan.check.state);
  };
  assert_eq!((blocked.as_str(), paths.as_slice()), (worktree.path.as_str(), ["own.txt".to_string()].as_slice()));
  assert_ne!(repo.head(), base);
  assert_eq!(read(&repo, "a.txt"), "a\n");
}

#[test]
fn a_target_that_moved_since_the_check_is_head_moved_with_nothing_changed() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) change", "b.txt", "b\n");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  let moved = repo.create_commit("(own) meanwhile", "c.txt", "c\n");

  let error = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap_err();
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
  assert_eq!(repo.head(), moved);
  assert!(!repo.path().join("b.txt").exists());
}

#[test]
fn a_source_on_a_branch_moves_its_branch_and_one_with_a_cherry_pick_in_progress_is_refused() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.git(&["switch", "-q", "-c", "lane"]);
  worktree.commit("(lane) change", "b.txt", "b\n");

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert!(!plan.check.follow.as_ref().unwrap().detached);
  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(applied.followed, Some(FollowState::Moved));
  assert_eq!(worktree.git(&["rev-parse", "refs/heads/lane"]), applied.new_head);
  assert_eq!(worktree.git(&["branch", "--show-current"]), "lane");

  worktree.commit("(lane) next", "c.txt", "c\n");
  let state_file = worktree.git(&["rev-parse", "--path-format=absolute", "--git-path", "CHERRY_PICK_HEAD"]);
  std::fs::write(&state_file, format!("{}\n", worktree.head())).unwrap();
  let error = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap_err();
  assert!(matches!(error.downcast_ref::<GuardRefusal>(), Some(GuardRefusal::OperationInProgress(_))), "{error:#}");
  // without --follow the source is only read
  check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
}

#[test]
fn a_source_that_switched_after_the_check_stays_and_the_same_pick_again_moves_it() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let source_head = worktree.commit("(lane) change", "b.txt", "b\n");
  // the target moved on: the replayed commit is a new one, not the source's HEAD
  repo.create_commit("(own) other", "own.txt", "own\n");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  worktree.git(&["switch", "-q", "-c", "parked"]);

  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert!(matches!(applied.followed, Some(FollowState::HeadMoved(_))), "{:?}", applied.followed);
  assert_eq!(repo.head(), applied.new_head);
  assert_eq!(worktree.head(), source_head);

  let again = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert!(again.check.picked.is_empty(), "{:?}", again.check.picked);
  assert_eq!(again.check.present.len(), 1);
  assert_eq!(again.check.new_head, applied.new_head);
  let followed = apply_pick(repo.git_executor(), repo.path_str(), &again).unwrap();
  assert_eq!(followed.followed, Some(FollowState::Moved));
  assert_eq!(worktree.git(&["rev-parse", "refs/heads/parked"]), applied.new_head);
}

#[test]
fn a_pick_and_a_follow_killed_before_their_files_followed_are_finished() {
  let repo = TestRepo::new();
  let from_commit = repo.create_commit("one", "a.txt", "one\n");
  let to = repo.create_commit("two", "a.txt", "two\n");
  let worktree = TestWorktree::detached(&repo);
  // the target: the pick moved the branch, the files stayed at `from`
  repo.git(&["reset", "-q", "--hard", &from_commit]);
  repo.git(&["update-ref", "-m", "branch-deck: pick", "HEAD", &to, &from_commit]);
  let mark = format!("{PENDING_UPDATE_PREFIX}main");
  repo.git(&["update-ref", &mark, &from_commit]);
  assert_eq!(
    finish_pending_update(repo.git_executor(), repo.path_str()).unwrap(),
    Some((from_commit.clone(), to.clone()))
  );
  assert_eq!(read(&repo, "a.txt"), "two\n");
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &mark));

  // the detached source: the follow moved HEAD, the files stayed
  worktree.git(&["checkout", "-q", "--detach", &from_commit]);
  worktree.git(&["update-ref", "--no-deref", "-m", "branch-deck: pick", "HEAD", &to, &from_commit]);
  let mark = format!("{PENDING_UPDATE_PREFIX}lane");
  worktree.git(&["update-ref", &mark, &from_commit]);
  assert_eq!(finish_pending_update(repo.git_executor(), &worktree.path).unwrap(), Some((from_commit, to)));
  assert_eq!(worktree.read("a.txt"), "two\n");
  assert_eq!(worktree.git(&["status", "--porcelain"]), "");
}

#[test]
fn the_undo_moves_the_target_and_the_detached_source_back() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let source_head = worktree.commit("(lane) change", "a.txt", "lane\n");
  let own = repo.create_commit("(own) other", "own.txt", "own\n");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();

  for (path, previous) in [(repo.path_str(), own.as_str()), (worktree.path.as_str(), source_head.as_str())] {
    let UndoCheck::Ready(undo) = check_pick_undo(repo.git_executor(), path, previous, &applied.new_head).unwrap() else {
      panic!("not ready: {path}");
    };
    assert_eq!(undo.blocked(), [] as [String; 0]);
    apply_update_undo(repo.git_executor(), path, &undo).unwrap();
    assert_eq!(GitCommandExecutor::new().execute_command(&["rev-parse", "HEAD"], path).unwrap(), previous);
    assert!(matches!(
      check_pick_undo(repo.git_executor(), path, previous, &applied.new_head).unwrap(),
      UndoCheck::AlreadyUndone { .. }
    ));
  }
  assert_eq!(read(&repo, "a.txt"), "a\n");
  assert_eq!(worktree.read("a.txt"), "lane\n");
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: undo pick");

  // an undo after another move of the branch is stale
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  repo.create_commit("(own) after", "c.txt", "c\n");
  let error = check_pick_undo(repo.git_executor(), repo.path_str(), &own, &applied.new_head).unwrap_err();
  assert!(error.downcast_ref::<HeadMoved>().is_some(), "{error:#}");
}

#[test]
fn the_source_is_another_worktree_of_this_repository() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let request = |source| PickRequest {
    source,
    upstream: None,
    follow: false,
  };
  let error = check_pick(repo.git_executor(), repo.path_str(), &request(PickSource::Worktree(repo.path_str()))).unwrap_err();
  assert!(matches!(error.downcast_ref::<PickRefusal>(), Some(PickRefusal::InvalidSource(_))), "{error:#}");

  let other = TestRepo::new();
  other.create_commit("other", "o.txt", "o\n");
  let error = check_pick(repo.git_executor(), repo.path_str(), &request(PickSource::Worktree(other.path_str()))).unwrap_err();
  assert!(format!("{error:#}").contains("another repository"), "{error:#}");

  let error = check_pick(repo.git_executor(), repo.path_str(), &request(PickSource::Revisions("HEAD...HEAD"))).unwrap_err();
  assert!(matches!(error.downcast_ref::<PickRefusal>(), Some(PickRefusal::InvalidSource(_))), "{error:#}");
}
