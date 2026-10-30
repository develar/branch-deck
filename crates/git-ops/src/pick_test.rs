use crate::pick::{FollowState, PickPlan, PickRefusal, PickRequest, PickSource, PickState, PresentBy, SubjectPrefix, apply_pick, check_pick, check_pick_undo, pick_dry_run};
use crate::revs::ref_exists;
use crate::rewrite_guard::HeadMoved;
use crate::update_branch::{GuardRefusal, PENDING_UPDATE_PREFIX, ReflogEntry, UndoCheck, apply_update_undo, finish_pending_update, undone_move_base};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use test_utils::history::{HistoryCommit, HistoryStream, one_file_each};

const TRACKING_REF: &str = "refs/remotes/origin/master";

fn from(worktree: &TestWorktree, follow: bool) -> PickRequest<'_> {
  PickRequest {
    source: PickSource::Worktree(&worktree.path),
    upstream: None,
    follow,
    expect_branch: None,
    expect_source_head: None,
    prefix: None,
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

/// Commits `file` in `worktree` with the author and committer date `seconds` (`+0200`); returns the commit
fn commit_at(worktree: &TestWorktree, message: &str, file: &str, seconds: i64) -> String {
  worktree.write(file, "lane\n");
  worktree.git(&["add", "--", file]);
  let date = format!("{seconds} +0200");
  worktree.commit_with(&["-m", message], &[("GIT_AUTHOR_DATE", date.as_str()), ("GIT_COMMITTER_DATE", date.as_str())])
}

/// The conflict of `plan`, which must name `commit` on `a.txt` after the clean `(lane) clean`, nothing moved
fn assert_conflict_on_a(plan: &PickPlan, commit: &str, head: &str) {
  let PickState::Conflict(conflict) = &plan.check.state else {
    panic!("not a conflict: {:?}", plan.check.state);
  };
  assert_eq!(conflict.commit.id, commit);
  assert_eq!(conflict.clean.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["(lane) clean"]);
  assert_eq!(conflict.files, ["a.txt"]);
  assert_eq!(plan.check.new_head, head);
}

/// A detached worktree with one commit `(lane) change` on `base`, a target that moved on (`(own) other`), and the
/// checked pick of the worktree with `--follow`: the repository, `base`, the worktree and the plan
fn a_checked_follow() -> (TestRepo, String, TestWorktree, PickPlan) {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) change", "b.txt", "b\n");
  repo.create_commit("(own) other", "own.txt", "own\n");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  (repo, base, worktree, plan)
}

/// Delete the reflog of the target branch: its fork points are gone
fn expire_reflog(repo: &TestRepo) {
  repo.git(&["reflog", "expire", "--expire=now", "--expire-unreachable=now", "refs/heads/master"]);
}

#[test]
fn a_pick_lands_the_commits_of_a_worktree_and_the_worktree_follows() {
  let (repo, base, worktree, first) = a_lane_cut_at_the_tip();
  worktree.commit("(lane) second", "a.txt", "a2\n");
  // the target moved on: the copies are new commits (a worktree cut at the tip is a fast-forward, and would have nowhere
  // to move)
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
  // no refresh follows `read-tree -m -u`, in the target and in the source that followed: it records the stat data of
  // the files it writes (before `status`, which refreshes them itself)
  assert_eq!(repo.git(&["diff-files", "--name-only"]), "");
  assert_eq!(worktree.git(&["diff-files", "--name-only"]), "");
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
fn a_worktree_cut_at_the_tip_lands_its_commits_as_they_are() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  // dated in the past: a replay now would write other commits
  let first = commit_at(&worktree, "(lane) first", "b.txt", 1_700_000_000);
  let second = commit_at(&worktree, "(lane) second", "a.txt", 1_700_000_100);

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(
    plan.check.picked.iter().map(|commit| (commit.commit.as_str(), commit.new_id.as_str())).collect::<Vec<_>>(),
    [(first.as_str(), first.as_str()), (second.as_str(), second.as_str())]
  );
  assert_eq!(plan.check.new_head, second);

  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(applied.followed, Some(FollowState::Moved));
  assert_eq!(repo.head(), second);
  assert_eq!(read(&repo, "a.txt"), "lane\n");
  assert_eq!(read(&repo, "b.txt"), "lane\n");
  assert_eq!(repo.git(&["status", "--porcelain", "--untracked-files=no"]), "");
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: pick");
  // the worktree's commits are the branch's: it has nowhere to move
  assert_eq!(worktree.head(), second);
  assert_eq!(worktree.git(&["log", "-g", "-n1", "--format=%gs", "HEAD"]), "commit: (lane) second");
}

#[test]
fn the_copies_get_the_newest_committer_date_so_the_check_names_the_commits_the_pick_writes() {
  const BASE: i64 = 1_700_000_000;
  let repo = TestRepo::new();
  repo.create_commit_with_timestamp("base", "a.txt", "a\n", Some(BASE));
  let worktree = TestWorktree::detached(&repo);
  // the newest committer date of the range is not its tip's
  commit_at(&worktree, "(lane) first", "b.txt", BASE + 100);
  commit_at(&worktree, "(lane) second", "c.txt", BASE + 50);
  repo.create_commit_with_timestamp("(own) other", "own.txt", "own\n", Some(BASE + 10));

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  let again = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(again.check.new_head, plan.check.new_head);

  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(repo.head(), plan.check.new_head);
  let newest = format!("{} +0200", BASE + 100);
  assert_eq!(
    repo.git(&["log", "--reverse", "--date=raw", "--format=%s|%ad|%cd", "HEAD~2..HEAD"]),
    format!("(lane) first|{newest}|{newest}\n(lane) second|{} +0200|{newest}", BASE + 50)
  );
}

#[test]
fn a_commit_the_target_has_already_is_present_with_the_target_commit_of_its_patch() {
  let (repo, worktree, [landed, same_content, _], copy) = a_lane_with_commits_the_target_has();

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(plan.check.picked.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["(lane) new"]);
  let present: Vec<(&str, Option<&str>, PresentBy)> = plan.check.present.iter().map(|commit| (commit.commit.as_str(), commit.on.as_deref(), commit.by)).collect();
  assert_eq!(
    present,
    [(landed.as_str(), Some(copy.as_str()), PresentBy::PatchId), (same_content.as_str(), None, PresentBy::Empty)]
  );
  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(read(&repo, "d.txt"), "d\n");
}

/// A lane of three commits, `landed`, `same content` and `new`: the target has the first by cherry-pick (`-x`: in the
/// same second, a plain pick would be the same commit; its copy is the last of the answer), and the change of the second
/// inside a larger commit
fn a_lane_with_commits_the_target_has() -> (TestRepo, TestWorktree, [String; 3], String) {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let landed = worktree.commit("(lane) landed", "b.txt", "b\n");
  let same_content = worktree.commit("(lane) same content", "c.txt", "c\n");
  let new = worktree.commit("(lane) new", "d.txt", "d\n");
  repo.git(&["cherry-pick", "-x", &landed]);
  let copy = repo.head();
  repo.create_commit_with_files("(other) bigger", &[("c.txt", "c\n"), ("e.txt", "e\n")]);
  (repo, worktree, [landed, same_content, new], copy)
}

/// The dry run names its range beside what it would pick: a commit of the range it would not pick is on the branch (by
/// patch id or by content), and one outside the range is no commit it checked. A range that conflicts is kept whole but
/// for its commits of the same patch id
#[test]
fn a_dry_run_names_its_range_and_the_commits_it_would_pick() {
  let (repo, worktree, [landed, same_content, new], _) = a_lane_with_commits_the_target_has();
  let dry_run = |dir: &str| pick_dry_run(repo.git_executor(), repo.path_str(), "master", dir, None).unwrap();

  let clean = dry_run(&worktree.path);
  assert_eq!(clean.range, [landed.clone(), same_content.clone(), new.clone()]);
  assert_eq!(clean.kept.iter().map(|commit| commit.id.as_str()).collect::<Vec<_>>(), [new.as_str()]);

  repo.create_commit("(other) d", "d.txt", "other d\n");
  let conflicting = dry_run(&worktree.path);
  assert_eq!(conflicting.range, [landed, same_content.clone(), new.clone()]);
  assert_eq!(
    conflicting.kept.iter().map(|commit| commit.id.as_str()).collect::<Vec<_>>(),
    [same_content.as_str(), new.as_str()]
  );
}

#[test]
fn a_commit_with_the_patch_id_of_a_target_commit_is_left_out_and_its_child_lands_on_the_copy_before_it() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) first", "b.txt", "b\n");
  let landed = worktree.commit("(lane) landed", "a.txt", "lane\n");
  let child = worktree.commit("(lane) child", "c.txt", "c\n");
  // the target has the second by its patch, and changed the same line since: replayed, it would conflict
  repo.git(&["cherry-pick", "-x", &landed]);
  let copy = repo.head();
  repo.create_commit("(own) same line", "a.txt", "own\n");

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(
    plan
      .check
      .present
      .iter()
      .map(|commit| (commit.commit.as_str(), commit.on.as_deref(), commit.by))
      .collect::<Vec<_>>(),
    [(landed.as_str(), Some(copy.as_str()), PresentBy::PatchId)]
  );
  let picked: Vec<&str> = plan.check.picked.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(picked, ["(lane) first", "(lane) child"]);
  // one chain: the child's copy stands on the copy of the first, not on the target's HEAD
  let child_copy = &plan.check.picked[1];
  assert_eq!((child_copy.commit.as_str(), child_copy.new_id.as_str()), (child.as_str(), plan.check.new_head.as_str()));
  assert_eq!(repo.git(&["rev-parse", &format!("{}^", plan.check.new_head)]), plan.check.picked[0].new_id);

  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(subjects(&repo, "HEAD~2..HEAD"), ["(lane) first", "(lane) child"]);
  assert_eq!(
    (read(&repo, "a.txt"), read(&repo, "b.txt"), read(&repo, "c.txt")),
    ("own\n".into(), "b\n".into(), "c\n".into())
  );
  let state_file = repo.git(&["rev-parse", "--path-format=absolute", "--git-path", "CHERRY_PICK_HEAD"]);
  assert!(!std::path::Path::new(&state_file).exists());
  assert_eq!(repo.git(&["status", "--porcelain", "--untracked-files=no"]), "");
}

#[test]
fn a_conflict_after_a_commit_left_out_by_its_patch_id_names_the_commit_and_lists_the_left_out_one() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let landed = worktree.commit("(lane) landed", "b.txt", "b\n");
  worktree.commit("(lane) clean", "c.txt", "c\n");
  let conflicting = worktree.commit("(lane) conflicting", "a.txt", "lane\n");
  repo.git(&["cherry-pick", "-x", &landed]);
  let head = repo.create_commit("(own) same line", "a.txt", "own\n");

  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  assert_conflict_on_a(&plan, &conflicting, &head);
  assert_eq!(
    plan.check.present.iter().map(|commit| (commit.commit.as_str(), commit.by)).collect::<Vec<_>>(),
    [(landed.as_str(), PresentBy::PatchId)]
  );
}

#[test]
fn a_target_rewritten_after_the_worktree_was_cut_from_it_gives_back_none_of_its_old_commits() {
  let (repo, _) = TestRepo::with_commits(&one_file_each([("base", "a.txt", "a\n"), ("(own) local", "own.txt", "one\n")]));
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
  // `master` with two commits of its own on the base, which `TRACKING_REF` names: one stream
  let mut history = HistoryStream::new("refs/heads/master");
  for (message, file, content) in [("base", "a.txt", "a\n"), ("(own) one", "own.txt", "one\n"), ("(own) two", "two.txt", "two\n")] {
    history.commit(&HistoryCommit::new(message, &[(file, content)]));
  }
  history.reset_to_root(TRACKING_REF);
  let _base = history.write(repo.path()).unwrap().remove(0).unwrap();
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
      expect_branch: None,
      expect_source_head: None,
      prefix: None,
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
  let (repo, [base, _]) = TestRepo::with_commits(&one_file_each([("base", "a.txt", "a\n"), ("(own) local", "own.txt", "one\n")]));
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
  assert_conflict_on_a(&plan, &conflicting, &head);
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

/// The detached source switched after the check to a new branch at another commit: the follow's transaction fails on
/// the old value of HEAD, and the check after the failure names the branch (`HeadAttached`), not a moved HEAD; the
/// source and its branch stay
#[test]
fn a_source_that_switched_to_a_branch_at_another_commit_after_the_check_is_attached_and_stays() {
  let (repo, base, worktree, plan) = a_checked_follow();
  worktree.git(&["switch", "-q", "-c", "parked", &base]);

  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  let Some(FollowState::HeadMoved(message)) = &applied.followed else {
    panic!("the follow did not stop: {:?}", applied.followed);
  };
  assert!(message.starts_with("HEAD is on parked now, no longer detached"), "{message}");
  assert_eq!(repo.head(), applied.new_head);
  assert_eq!(worktree.git(&["rev-parse", "HEAD", "refs/heads/parked"]), format!("{base}\n{base}"));
  assert_eq!(worktree.git(&["status", "--porcelain"]), "");
}

#[test]
fn a_pick_and_a_follow_killed_before_their_files_followed_are_finished() {
  let (repo, [from_commit, to]) = TestRepo::with_commits(&one_file_each([("one", "a.txt", "one\n"), ("two", "a.txt", "two\n")]));
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

/// A repository with `base` and a worktree cut at it with one commit, `first` on `b.txt`: a pick of it is a fast-forward
fn a_lane_cut_at_the_tip() -> (TestRepo, String, TestWorktree, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let first = worktree.commit("(lane) first", "b.txt", "b\n");
  (repo, base, worktree, first)
}

/// Apply `plan`, which must roll back: a file the move writes appears in the target after the check
fn roll_back_pick(repo: &TestRepo, plan: &PickPlan, file: &str) {
  std::fs::write(repo.path().join(file), "in the way\n").unwrap();
  let error = apply_pick(repo.git_executor(), repo.path_str(), plan).unwrap_err();
  assert!(format!("{error:#}").starts_with("Files changed during the pick; nothing was changed"), "{error:#}");
  assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: pick rolled back");
  std::fs::remove_file(repo.path().join(file)).unwrap();
}

#[test]
fn a_fast_forward_pick_that_rolled_back_is_no_fork_point_and_the_next_pick_lands_the_commit() {
  let (repo, base, worktree, first) = a_lane_cut_at_the_tip();
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert_eq!(plan.check.new_head, first, "a fast-forward: the source's own tip lands in the reflog of master");
  roll_back_pick(&repo, &plan, "b.txt");
  assert_eq!(repo.head(), base);
  assert_eq!(repo.git(&["merge-base", "--fork-point", "refs/heads/master", &first]), first);

  let again = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert_eq!(again.check.base, base);
  assert_eq!(again.check.picked.iter().map(|commit| commit.commit.as_str()).collect::<Vec<_>>(), [first.as_str()]);
  let applied = apply_pick(repo.git_executor(), repo.path_str(), &again).unwrap();
  assert_eq!(repo.head(), first);
  assert_eq!(applied.followed, Some(FollowState::Moved));
  assert_eq!(worktree.head(), first);
  assert_eq!(read(&repo, "b.txt"), "b\n");
}

#[test]
fn a_worktree_that_went_on_from_a_rolled_back_tip_lands_the_commits_below_it_too() {
  let (repo, base, worktree, first) = a_lane_cut_at_the_tip();
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  roll_back_pick(&repo, &plan, "b.txt");
  let second = worktree.commit("(lane) second", "c.txt", "c\n");

  let again = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert_eq!(again.check.base, base);
  assert_eq!(
    again.check.picked.iter().map(|commit| commit.commit.as_str()).collect::<Vec<_>>(),
    [first.as_str(), second.as_str()]
  );
}

#[test]
fn an_undone_fast_forward_pick_is_no_fork_point() {
  let (repo, base, worktree, first) = a_lane_cut_at_the_tip();
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  let UndoCheck::Ready(undo) = check_pick_undo(repo.git_executor(), repo.path_str(), &base, &first).unwrap() else {
    panic!("not ready");
  };
  apply_update_undo(repo.git_executor(), repo.path_str(), &undo).unwrap();
  assert_eq!(repo.head(), base);

  let again = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, true)).unwrap();
  assert_eq!(again.check.base, base);
  assert_eq!(again.check.picked.iter().map(|commit| commit.commit.as_str()).collect::<Vec<_>>(), [first.as_str()]);
}

#[test]
fn a_pick_waits_for_the_index_lock_of_another_process_and_then_lands_with_its_files() {
  let (repo, base, _worktree, plan) = a_checked_follow();
  // an IDE's `git status` holds the lock of the target for a while: open, so the wait never takes it for a lock that no
  // process owns and removes it itself (`IndexLock`), however late the holder lets it go. The wait polls the lock on
  // the system clock and tells no one that it waits (`IndexLock::wait_while_locked`, which the plan builds itself): the
  // holder keeps the lock for two of its polls after the wait starts, and the outcome is the same whenever it lets go
  let lock = repo.path().join(".git").join("index.lock");
  let held = std::fs::File::create(&lock).unwrap();
  let (waits, wait_started) = std::sync::mpsc::channel::<()>();
  let release = std::thread::spawn(move || {
    wait_started.recv().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(held);
    std::fs::remove_file(lock).unwrap();
  });
  waits.send(()).unwrap();
  plan.wait_for_index_locks(repo.git_executor(), repo.path_str()).unwrap();
  release.join().unwrap();
  let applied = apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(applied.followed, Some(FollowState::Moved));
  assert_eq!(read(&repo, "b.txt"), "b\n");
  assert_eq!(repo.git(&["status", "--porcelain", "--untracked-files=no"]), "");
  let moves = repo.git(&["log", "-g", "--format=%gs", "refs/heads/master"]);
  assert!(!moves.contains("rolled back"), "{moves}");
  assert_eq!(subjects(&repo, &format!("{base}..HEAD")), ["(own) other", "(lane) change"]);
}

#[test]
fn a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  worktree.commit("(lane) change", "b.txt", "b\n");
  repo.create_commit("(own) other", "own.txt", "own\n");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &from(&worktree, false)).unwrap();
  // what a git killed by SIGKILL leaves: nobody has it open, nobody will remove it
  let lock = repo.path().join(".git").join("index.lock");
  std::fs::write(&lock, "").unwrap();
  let result = apply_pick(repo.git_executor(), repo.path_str(), &plan);
  if cfg!(windows) {
    // no owner check: the lock stays for the whole wait, and the branch did not move
    let error = result.unwrap_err();
    assert!(error.downcast_ref::<crate::index_lock::IndexLocked>().is_some(), "{error:#}");
    assert_eq!(repo.head(), plan.check.head);
  } else {
    result.unwrap();
    assert!(!lock.exists());
    assert_eq!(read(&repo, "b.txt"), "b\n");
    assert_eq!(repo.git(&["log", "-g", "-n1", "--format=%gs", "refs/heads/master"]), "branch-deck: pick");
  }
}

fn entry(index: usize, id: &str, message: &str) -> ReflogEntry {
  ReflogEntry {
    index,
    id: id.to_string(),
    message: message.to_string(),
  }
}

#[test]
fn only_a_value_whose_every_move_was_rolled_back_or_undone_gives_way_to_the_value_before() {
  let rolled_back = [
    entry(0, "h", "branch-deck: pick rolled back"),
    entry(1, "t", "branch-deck: pick"),
    entry(2, "h", "commit: h"),
  ];
  assert_eq!(undone_move_base(&rolled_back, "t").as_deref(), Some("h"));
  let undone = [entry(0, "h", "branch-deck: undo pick"), entry(1, "t", "branch-deck: pick"), entry(2, "h", "commit: h")];
  assert_eq!(undone_move_base(&undone, "t").as_deref(), Some("h"));
  // twice rolled back, then a newer value
  let twice = [
    entry(0, "n", "branch-deck: pick"),
    entry(1, "h", "branch-deck: update project rolled back"),
    entry(2, "u", "branch-deck: update project"),
    entry(3, "h", "branch-deck: pick rolled back"),
    entry(4, "t", "branch-deck: pick"),
    entry(5, "h", "branch-deck: pick rolled back"),
    entry(6, "t", "branch-deck: pick"),
    entry(7, "h", "commit: h"),
  ];
  assert_eq!(undone_move_base(&twice, "t").as_deref(), Some("h"));

  // the value stood: the ref is there, an undo of its undo rolled back, another move put it there too, the user did
  let cases: [(&str, Vec<ReflogEntry>); 5] = [
    ("the tip", vec![entry(0, "t", "branch-deck: pick"), entry(1, "h", "commit: h")]),
    (
      "a rolled back undo",
      vec![
        entry(0, "t", "branch-deck: undo pick rolled back"),
        entry(1, "h", "branch-deck: undo pick"),
        entry(2, "t", "branch-deck: pick"),
        entry(3, "h", "commit: h"),
      ],
    ),
    (
      "another move to it stood",
      vec![
        entry(0, "n", "commit: n"),
        entry(1, "t", "branch-deck: pick"),
        entry(2, "h", "branch-deck: pick rolled back"),
        entry(3, "t", "branch-deck: pick"),
        entry(4, "h", "commit: h"),
      ],
    ),
    (
      "a commit of the user",
      vec![entry(0, "h", "reset: moving to h"), entry(1, "t", "commit: t"), entry(2, "h", "commit: h")],
    ),
    // `log -g` skipped an entry whose commit is gone: the one after the move is unknown
    (
      "a gap",
      vec![
        entry(0, "h", "branch-deck: pick rolled back"),
        entry(2, "t", "branch-deck: pick"),
        entry(3, "h", "commit: h"),
      ],
    ),
  ];
  for (name, entries) in cases {
    assert_eq!(undone_move_base(&entries, "t"), None, "{name}");
  }
  // a rollback to another value than the one before is no undo of the move
  let elsewhere = [
    entry(0, "x", "branch-deck: pick rolled back"),
    entry(1, "t", "branch-deck: pick"),
    entry(2, "h", "commit: h"),
  ];
  assert_eq!(undone_move_base(&elsewhere, "t"), None);
}

#[test]
fn the_source_is_another_worktree_of_this_repository() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let request = |source| PickRequest {
    source,
    upstream: None,
    follow: false,
    expect_branch: None,
    expect_source_head: None,
    prefix: None,
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

/// The commits of someone else on a remote branch: `theirs` on top of `older`, a commit of the branch as it was pushed,
/// committed in `worktree` as Lev on a side line of its repository; returns both
fn remote_line(worktree: &TestWorktree) -> (String, String) {
  let older = commit_at(worktree, "(feature) older version", "feature.txt", 1_700_000_100);
  worktree.git(&["config", "user.name", "Lev Leontev"]);
  worktree.git(&["config", "user.email", "lev@example.com"]);
  let theirs = commit_at(worktree, "fix ultimate opened as JPS", "fix.txt", 1_700_000_200);
  (older, theirs)
}

fn listed<'a>(ids: &'a [String], prefix: Option<SubjectPrefix<'a>>) -> PickRequest<'a> {
  PickRequest {
    source: PickSource::Commits(ids),
    upstream: None,
    follow: false,
    expect_branch: None,
    expect_source_head: None,
    prefix,
  }
}

/// `bd pick --remote`: only the listed commits are replayed (not the older commit they were made on), and their copies
/// take the prefix of the branch; the author stays, and the dry run names the commit the pick writes
#[test]
fn listed_commits_land_alone_with_the_prefix_of_their_branch() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let (_older, theirs) = remote_line(&worktree);
  repo.create_commit("(feature) the branch as it is here", "feature.txt", "feature here\n");

  let ids = vec![theirs.clone()];
  let prefix = SubjectPrefix { group: "feature", commits: &ids };
  let plan = check_pick(repo.git_executor(), repo.path_str(), &listed(&ids, Some(prefix))).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
  assert_eq!(plan.check.picked.len(), 1);
  assert_eq!(plan.check.picked[0].commit, theirs);
  assert_eq!(plan.check.picked[0].subject, "(feature) fix ultimate opened as JPS");
  let again = check_pick(repo.git_executor(), repo.path_str(), &listed(&ids, Some(prefix))).unwrap();
  assert_eq!(again.check.new_head, plan.check.new_head, "the same pick writes the same commit");

  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(repo.head(), plan.check.new_head);
  assert_eq!(
    repo.git(&["log", "-1", "--format=%s|%an <%ae>"]),
    "(feature) fix ultimate opened as JPS|Lev Leontev <lev@example.com>"
  );
  assert_eq!(read(&repo, "fix.txt"), "lane\n");
  assert_eq!(read(&repo, "feature.txt"), "feature here\n", "the older commit it was made on is not picked");
}

/// Listed commits that are no parent and child replay each on its own, after its own parent
#[test]
fn listed_commits_apart_replay_each_after_its_parent() {
  let repo = TestRepo::new();
  repo.create_commit("base", "a.txt", "a\n");
  let worktree = TestWorktree::detached(&repo);
  let first = commit_at(&worktree, "first", "first.txt", 1_700_000_100);
  commit_at(&worktree, "skipped", "skipped.txt", 1_700_000_200);
  let third = commit_at(&worktree, "third", "third.txt", 1_700_000_300);

  let ids = vec![first, third];
  let plan = check_pick(repo.git_executor(), repo.path_str(), &listed(&ids, None)).unwrap();
  apply_pick(repo.git_executor(), repo.path_str(), &plan).unwrap();
  assert_eq!(subjects(&repo, "HEAD~2..HEAD"), ["first", "third"]);
  assert!(!repo.path().join("skipped.txt").exists());
}

/// A pick that expects the source at a commit (`bd lease pick` after its check of the tree) refuses a source whose HEAD
/// moved since, before anything is written
#[test]
fn a_source_that_moved_from_the_expected_head_is_refused() {
  let (repo, _base, worktree, first) = a_lane_cut_at_the_tip();
  let second = worktree.commit("(lane) second", "a.txt", "a2\n");
  let request = |expected| PickRequest {
    expect_source_head: Some(expected),
    ..from(&worktree, true)
  };
  let error = check_pick(repo.git_executor(), repo.path_str(), &request(&first)).unwrap_err();
  assert!(matches!(error.downcast_ref::<PickRefusal>(), Some(PickRefusal::SourceMoved(_))), "{error:#}");
  let plan = check_pick(repo.git_executor(), repo.path_str(), &request(&second)).unwrap();
  assert_eq!(plan.check.state, PickState::Clean);
}
