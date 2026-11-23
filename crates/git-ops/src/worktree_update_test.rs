//! The files of a move by the index of the paths it changes against the same move on the whole index: every fixture
//! runs both, in two repositories with the same history and the same local state, and the index, the files and the
//! status must be the same, as must the refusal.

use super::{INDEX_COPY_PREFIX, IndexLeftBehind, SMALL_INDEX_FROM, WorktreeUpdate, dry_run_from, dry_run_worktree_update};
use crate::index_lock::IndexLock;
use crate::revs::ref_exists;
use crate::update_branch::{FilesBlocked, HeadRef, PENDING_UPDATE_PREFIX, finish_pending_update, move_branch_and_files};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, append_config};

enum Change {
  /// The mode, the path and the content of a file; the target of a symlink
  Put(&'static str, &'static str, &'static str),
  Delete(&'static str),
}

const FILE: &str = "100644";
const EXECUTABLE: &str = "100755";
const SYMLINK: &str = "120000";

/// A commit of a `fast-import` stream on `branch`, after `from` when it has a parent
fn commit(stream: &mut String, branch: &str, from: Option<&str>, changes: &[Change]) {
  let _ = write!(stream, "commit refs/heads/{branch}\ncommitter Test <test@example.com> 1700000000 +0000\ndata 1\nc\n");
  if let Some(from) = from {
    let _ = writeln!(stream, "from refs/heads/{from}");
  }
  for change in changes {
    match change {
      Change::Put(mode, path, content) => {
        let _ = write!(stream, "M {mode} inline {path}\ndata {}\n{content}\n", content.len());
      }
      Change::Delete(path) => {
        let _ = writeln!(stream, "D {path}");
      }
    }
  }
}

/// A repository at `old` of a move to `new` that changes, adds and deletes files, turns a file into a directory and a
/// directory into a file, changes an executable bit, a symlink and a file into a symlink, under attributes that
/// convert line ends and run a filter (as Git LFS does). The two commits by one `fast-import`, the files by one
/// `reset --hard`
fn repo_at_old() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  // a filter whose two ways differ, so a file that git wrote without it shows
  append_config(repo.path(), &[("filter.shout.clean", "tr A-Z a-z"), ("filter.shout.smudge", "tr a-z A-Z")]);
  let mut stream = String::new();
  commit(
    &mut stream,
    "master",
    None,
    &[
      Change::Put(FILE, ".gitattributes", "*.crlf text eol=crlf\n*.shout filter=shout\n"),
      Change::Put(FILE, "a/b/mod.txt", "one\n"),
      Change::Put(FILE, "a/b/gone-too.txt", "gone\n"),
      Change::Put(FILE, "d1/d2/gone.txt", "gone\n"),
      Change::Put(FILE, "keep/same.txt", "same\n"),
      Change::Put(FILE, "unrelated.txt", "unrelated\n"),
      Change::Put(FILE, "staged.txt", "staged\n"),
      Change::Put(FILE, "run.sh", "#!/bin/sh\n"),
      Change::Put(SYMLINK, "link", "keep/same.txt"),
      Change::Put(FILE, "becomes-link", "a file\n"),
      Change::Put(FILE, "becomes-dir", "a file\n"),
      Change::Put(FILE, "becomes-file/inner.txt", "in a directory\n"),
      Change::Put(FILE, "t.crlf", "l1\nl2\n"),
      Change::Put(FILE, "big.shout", "pointer one\n"),
    ],
  );
  commit(
    &mut stream,
    "next",
    Some("master"),
    &[
      Change::Put(FILE, "a/b/mod.txt", "two\n"),
      Change::Delete("a/b/gone-too.txt"),
      Change::Delete("d1/d2/gone.txt"),
      Change::Put(FILE, "a/added.txt", "added\n"),
      Change::Put(EXECUTABLE, "run.sh", "#!/bin/sh\n"),
      Change::Put(SYMLINK, "link", "a/added.txt"),
      Change::Delete("becomes-link"),
      Change::Put(SYMLINK, "becomes-link", "keep/same.txt"),
      Change::Delete("becomes-dir"),
      Change::Put(FILE, "becomes-dir/f.txt", "now in a directory\n"),
      Change::Delete("becomes-file/inner.txt"),
      Change::Put(FILE, "becomes-file", "now a file\n"),
      Change::Put(FILE, "t.crlf", "l1\nl2\nl3\n"),
      Change::Put(FILE, "n.crlf", "n1\nn2\n"),
      Change::Put(FILE, "big.shout", "pointer two\n"),
      Change::Put(FILE, "new.shout", "pointer new\n"),
    ],
  );
  repo
    .git_executor()
    .cmd(&["fast-import", "--quiet"], repo.path_str())
    .stdin(stream.as_bytes())
    .text()
    .unwrap();
  repo.git(&["reset", "--quiet", "--hard"]);
  let ids = repo.git(&["rev-parse", "refs/heads/master", "refs/heads/next"]);
  let (old, new) = ids.split_once('\n').unwrap();
  (repo, old.to_string(), new.to_string())
}

fn index_lock(repo: &TestRepo) -> IndexLock<'_> {
  IndexLock::of(repo.git_executor(), repo.path_str())
}

/// What a move left: the entries of the index with their flags, the status (which names a file whose executable bit
/// is not the one of its entry), and every file with its content or the target of a symlink
#[derive(Debug, PartialEq, Eq)]
struct State {
  index: String,
  status: String,
  files: BTreeMap<String, String>,
}

fn state(repo: &TestRepo) -> State {
  let mut files = BTreeMap::new();
  read_files(repo.path(), repo.path(), &mut files);
  State {
    index: repo.git(&["ls-files", "-s", "-v"]),
    status: repo.git(&["status", "--porcelain=v1", "--untracked-files=all"]),
    files,
  }
}

fn read_files(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) {
  for entry in std::fs::read_dir(dir).unwrap() {
    let path = entry.unwrap().path();
    if path == root.join(".git") {
      continue;
    }
    let name = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    let what = if metadata.is_symlink() {
      format!("link to {}", std::fs::read_link(&path).unwrap().display())
    } else if metadata.is_dir() {
      read_files(root, &path, files);
      "directory".to_string()
    } else {
      format!("file {:?}", String::from_utf8_lossy(&std::fs::read(&path).unwrap()))
    };
    files.insert(name, what);
  }
}

/// The files follow a move of `master` from `old` to `new` by the small index (or on the `whole` index): what git
/// answered, and whether the small index ran. A refused move moves the branch back, as the callers do; a move whose
/// files can't be prepared never moves it
fn follow(repo: &TestRepo, old: &str, new: &str, whole: bool) -> (Result<(), String>, bool) {
  let lock = index_lock(repo);
  let files = if whole {
    WorktreeUpdate::prepare_whole(repo.git_executor(), repo.path_str(), old, new, &lock)
  } else {
    WorktreeUpdate::prepare_from(repo.git_executor(), repo.path_str(), old, new, &lock, false, 0)
  };
  let files = match files {
    Ok(files) => files,
    Err(e) => return (Err(format!("{e:#}")), false),
  };
  let small = files.is_small();
  repo.git(&["update-ref", "refs/heads/master", new, old]);
  let followed = files.apply().map_err(|e| format!("{e:#}"));
  if followed.is_err() {
    repo.git(&["update-ref", "refs/heads/master", old, new]);
  }
  let git_dir = repo.path().join(".git");
  assert!(!git_dir.join("index.lock").exists(), "the index lock is released");
  if small {
    let copies: Vec<_> = std::fs::read_dir(&git_dir)
      .unwrap()
      .flatten()
      .filter(|entry| entry.file_name().to_string_lossy().starts_with(INDEX_COPY_PREFIX))
      .collect();
    assert!(copies.is_empty(), "no copy of the index stays: {copies:?}");
  }
  (followed, small)
}

/// The exact dry run of the move from `old` to `new`, by the small index from `small_index_from` bytes of the index file
/// on: git's message if it refuses. It writes no index
fn dry_run(repo: &TestRepo, old: &str, new: &str, small_index_from: u64) -> Result<Option<String>, String> {
  let index_file = repo.path().join(".git/index");
  let index = std::fs::read(&index_file).ok();
  let answer = dry_run_from(repo.git_executor(), repo.path_str(), old, new, true, small_index_from).map_err(|e| format!("{e:#}"));
  assert!(index.is_some() && std::fs::read(&index_file).ok() == index, "a dry run writes no index");
  assert!(!repo.path().join(".git/index.lock").exists(), "a dry run takes no lock");
  answer
}

/// The move with the local state of `local` on the whole index and by the small one: the same answer and the same
/// state. Returns the answer and the state, and whether the small index ran
fn both(local: impl Fn(&TestRepo)) -> (Result<(), String>, State, bool) {
  let (whole_repo, old, new) = repo_at_old();
  local(&whole_repo);
  let whole_dry_run = dry_run(&whole_repo, &old, &new, u64::MAX);
  let (whole, _) = follow(&whole_repo, &old, &new, true);
  let (small_repo, old, new) = repo_at_old();
  local(&small_repo);
  let small_dry_run = dry_run(&small_repo, &old, &new, 0);
  let (small, was_small) = follow(&small_repo, &old, &new, false);
  assert_eq!(small_dry_run, whole_dry_run, "the dry run of the small index, then of the whole one");
  if let Ok(refusal) = &small_dry_run {
    assert_eq!(refusal.is_some(), small.is_err(), "the dry run says what the move does: {refusal:?}, {small:?}");
  }
  assert_eq!(small, whole, "the answer of the small index, then of the whole one");
  let state_of_small = state(&small_repo);
  assert_eq!(state_of_small, state(&whole_repo), "the state after the small index, then after the whole one");
  (small, state_of_small, was_small)
}

/// `both` of a move that the small index runs and git takes
fn both_follow(local: impl Fn(&TestRepo)) -> State {
  let (answer, state, small) = both(local);
  assert_eq!(answer, Ok(()));
  assert!(small, "the small index runs");
  state
}

/// `both` of a move that the small index runs and git refuses: git's message
fn both_refuse(local: impl Fn(&TestRepo)) -> String {
  let (answer, _, small) = both(local);
  assert!(small, "the small index runs");
  answer.unwrap_err()
}

#[test]
fn a_clean_move_by_the_small_index_is_the_move_of_the_whole_index() {
  let state = both_follow(|_| {});
  assert_eq!(state.status, "", "{state:#?}");
  assert_eq!(state.files["a/b/mod.txt"], "file \"two\\n\"");
  assert_eq!(state.files["a/added.txt"], "file \"added\\n\"");
  assert!(!state.files.contains_key("d1"), "the directories of the deleted file are gone: {state:#?}");
  assert_eq!(state.files["becomes-dir/f.txt"], "file \"now in a directory\\n\"");
  assert_eq!(state.files["becomes-file"], "file \"now a file\\n\"");
  // the attributes of the index: the line ends and the filter
  assert_eq!(state.files["t.crlf"], "file \"l1\\r\\nl2\\r\\nl3\\r\\n\"");
  assert_eq!(state.files["n.crlf"], "file \"n1\\r\\nn2\\r\\n\"");
  assert_eq!(state.files["big.shout"], "file \"POINTER TWO\\n\"");
  assert_eq!(state.files["new.shout"], "file \"POINTER NEW\\n\"");
  assert!(
    state.index.contains("100755") && state.index.contains("120000"),
    "the modes are those of the tree: {}",
    state.index
  );
  // a symlink, or its target in a file where the file system has none
  assert!(state.files["link"].contains("a/added.txt"), "{state:#?}");
  assert!(state.files["becomes-link"].contains("keep/same.txt"), "{state:#?}");
}

#[test]
fn the_entries_the_small_index_puts_back_have_the_stat_data_of_their_files() {
  let (repo, old, new) = repo_at_old();
  // a copy of the index that a killed move left goes with the next one (`follow` looks)
  std::fs::create_dir_all(repo.path().join(".git").join(format!("{INDEX_COPY_PREFIX}stale"))).unwrap();
  let (followed, small) = follow(&repo, &old, &new, false);
  assert_eq!((followed, small), (Ok(()), true));
  assert_eq!(repo.git(&["write-tree"]), repo.git(&["rev-parse", &format!("{new}^{{tree}}")]), "the index is the new tree");
  // an entry without stat data has a size of 0: `git status` would hash its file each time
  let entries = repo.git(&["ls-files", "--debug", "a/b/mod.txt", "a/added.txt", "becomes-file"]);
  assert_eq!(entries.matches("size: ").count(), 3, "{entries}");
  assert!(!entries.contains("size: 0\t"), "{entries}");
}

#[test]
fn local_changes_on_the_paths_of_a_move_refuse_both_indexes_with_nothing_written() {
  let refusal = both_refuse(|repo| repo.write_file("a/b/mod.txt", "one\nlocal\n"));
  assert!(refusal.contains("a/b/mod.txt"), "{refusal}");
  let refusal = both_refuse(|repo| repo.write_file("a/added.txt", "mine\n"));
  assert!(refusal.contains("a/added.txt"), "{refusal}");
  // staged, and the file as the old tree has it again
  let refusal = both_refuse(|repo| {
    repo.write_file("a/b/mod.txt", "staged\n");
    repo.git(&["add", "a/b/mod.txt"]);
    repo.write_file("a/b/mod.txt", "one\n");
  });
  assert!(refusal.contains("a/b/mod.txt"), "{refusal}");
  let refusal = both_refuse(|repo| {
    repo.git(&["rm", "--quiet", "--cached", "a/b/mod.txt"]);
  });
  assert!(refusal.contains("a/b/mod.txt"), "{refusal}");
  // an untracked file in the directory that becomes a file
  let refusal = both_refuse(|repo| repo.write_file("becomes-file/mine.txt", "mine\n"));
  assert!(refusal.contains("becomes-file"), "{refusal}");
}

#[test]
fn local_changes_beside_the_paths_of_a_move_stay_in_both_indexes() {
  let state = both_follow(|repo| {
    repo.write_file("staged.txt", "staged here\n");
    repo.git(&["add", "staged.txt"]);
    repo.write_file("unrelated.txt", "unrelated\nedited\n");
    repo.write_file("untracked.txt", "untracked\n");
  });
  assert_eq!(state.status, "M  staged.txt\n M unrelated.txt\n?? untracked.txt", "{state:#?}");
  // an assume-unchanged entry beside the move keeps its flag
  let state = both_follow(|repo| {
    repo.git(&["update-index", "--assume-unchanged", "keep/same.txt"]);
  });
  assert!(state.index.lines().any(|line| line.starts_with("h ") && line.ends_with("keep/same.txt")), "{}", state.index);
}

#[test]
fn a_file_that_was_only_touched_or_has_the_new_content_follows_in_both_indexes() {
  both_follow(|repo| {
    let touched = std::fs::File::options().write(true).open(repo.path().join("a/b/mod.txt")).unwrap();
    touched.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600)).unwrap();
  });
  // staged as the new tree has it
  both_follow(|repo| {
    repo.write_file("a/b/mod.txt", "two\n");
    repo.git(&["add", "a/b/mod.txt"]);
  });
  // deleted in the working tree, not in the index
  both_follow(|repo| {
    sync_utils::fs_retry::remove_file(repo.path().join("a/b/gone-too.txt")).unwrap();
  });
}

#[test]
fn the_attributes_of_a_move_are_those_of_the_index_in_both_indexes() {
  // a local edit of `.gitattributes` that takes the conversion from a file the move adds: git reads the attributes
  // of a file it checks out from the index first
  let state = both_follow(|repo| repo.write_file(".gitattributes", "*.crlf text eol=crlf\nn.crlf -text\n*.shout filter=shout\n"));
  assert_eq!(state.files["n.crlf"], "file \"n1\\r\\nn2\\r\\n\"");
  // `core.autocrlf` converts the files without an attribute
  let state = both_follow(|repo| append_config(repo.path(), &[("core.autocrlf", "true")]));
  assert_eq!(state.files["a/added.txt"], "file \"added\\r\\n\"");
}

#[test]
fn a_staged_file_in_a_directory_that_becomes_a_file_goes_in_both_indexes() {
  // git takes the entry out and removes its file: the same loss on the whole index
  let state = both_follow(|repo| {
    repo.write_file("becomes-file/mine.txt", "mine\n");
    repo.git(&["add", "becomes-file/mine.txt"]);
  });
  assert_eq!(state.files["becomes-file"], "file \"now a file\\n\"");
}

#[test]
fn an_index_the_small_one_does_not_model_merges_whole() {
  let whole = |local: &dyn Fn(&TestRepo)| {
    let (answer, state, small) = both(local);
    assert!(!small, "the whole index merges");
    (answer, state)
  };
  // a skip-worktree entry anywhere, an assume-unchanged entry on a path of the move
  let (answer, state) = whole(&|repo| {
    repo.git(&["update-index", "--skip-worktree", "keep/same.txt"]);
  });
  assert_eq!(answer, Ok(()));
  assert!(state.index.lines().any(|line| line.starts_with("S ") && line.ends_with("keep/same.txt")), "{}", state.index);
  let (answer, _) = whole(&|repo| {
    repo.git(&["update-index", "--assume-unchanged", "a/b/mod.txt"]);
  });
  assert_eq!(answer, Ok(()));
  // a sparse checkout, by its patterns file
  let (answer, _) = whole(&|repo| {
    std::fs::create_dir_all(repo.path().join(".git/info")).unwrap();
    std::fs::write(repo.path().join(".git/info/sparse-checkout"), "/*\n").unwrap();
  });
  assert_eq!(answer, Ok(()));
  // an unmerged entry, on whatever path: git refuses the whole index
  let (answer, _) = whole(&|repo| {
    let blob = repo.git(&["rev-parse", ":unrelated.txt"]);
    repo.git(&["update-index", "--force-remove", "unrelated.txt"]);
    let entries = format!("100644 {blob} 1\tunrelated.txt\n100644 {blob} 2\tunrelated.txt\n");
    repo
      .git_executor()
      .cmd(&["update-index", "--index-info"], repo.path_str())
      .stdin(entries.as_bytes())
      .text()
      .unwrap();
  });
  assert!(answer.unwrap_err().contains("unrelated.txt"));
}

#[test]
fn a_move_of_a_submodule_merges_whole() {
  let repo = TestRepo::new();
  let old = repo.create_commit("old", "a.txt", "one\n");
  let link = format!("160000,{old},sub");
  repo.git(&["update-index", "--add", "--cacheinfo", &link]);
  let new = repo.commit_with(&["-m", "a submodule"], &[]);
  repo.git(&["reset", "--quiet", "--hard", &old]);
  let (followed, small) = follow(&repo, &old, &new, false);
  assert_eq!((followed, small), (Ok(()), false));
  assert!(repo.git(&["ls-files", "-s", "sub"]).starts_with("160000"));
}

#[test]
fn a_staged_deletion_of_every_path_of_a_move_refuses_the_small_index() {
  // the cut is empty: the small index must not be unborn, or the merge would take every path of the new tree
  let repo = TestRepo::new();
  let old = repo.create_commit("old", "a.txt", "one\n");
  let new = repo.create_commit("new", "a.txt", "two\n");
  repo.git(&["reset", "--quiet", "--hard", &old]);
  repo.git(&["rm", "--quiet", "--cached", "a.txt"]);
  let (followed, small) = follow(&repo, &old, &new, false);
  assert!(small);
  assert!(followed.unwrap_err().contains("a.txt"));
  assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "one\n");
}

#[test]
fn an_index_that_cannot_follow_takes_the_files_of_the_small_index_back() {
  let (repo, old, new) = repo_at_old();
  let before = state(&repo);
  let lock = index_lock(&repo);
  let files = WorktreeUpdate::prepare_from(repo.git_executor(), repo.path_str(), &old, &new, &lock, false, 0).unwrap();
  assert!(files.is_small());
  // the index can't be copied: a directory is in its place
  let index = repo.path().join(".git/index");
  let saved = repo.path().join(".git/index.saved");
  sync_utils::fs_retry::rename(&index, &saved).unwrap();
  std::fs::create_dir_all(&index).unwrap();
  let error = files.apply().unwrap_err();
  assert!(error.downcast_ref::<IndexLeftBehind>().is_none(), "the files went back: {error:#}");
  assert!(format!("{error:#}").contains("Failed to copy the index"), "{error:#}");
  sync_utils::fs_retry::remove_dir(&index).unwrap();
  sync_utils::fs_retry::rename(&saved, &index).unwrap();
  assert!(!repo.path().join(".git/index.lock").exists());
  assert_eq!(state(&repo), before, "nothing changed");
}

#[test]
fn the_small_index_holds_the_index_lock_from_its_cut_to_its_put_back() {
  let (repo, old, new) = repo_at_old();
  let lock = index_lock(&repo);
  let files = WorktreeUpdate::prepare_from(repo.git_executor(), repo.path_str(), &old, &new, &lock, false, 0).unwrap();
  assert!(files.is_small());
  // a git of the user can't write the index now: a `git add` between the files and the index would stage, and a
  // `git commit` commit, the old index over the new files
  repo.write_file("unrelated.txt", "edited\n");
  let (refusal, exit_code) = repo.git_executor().execute_command_with_status(&["add", "unrelated.txt"], repo.path_str()).unwrap();
  assert_ne!(exit_code, 0, "{refusal}");
  assert!(refusal.contains("index.lock"), "{refusal}");
  repo.git(&["update-ref", "refs/heads/master", &new, &old]);
  files.apply().unwrap();
  assert!(!repo.path().join(".git/index.lock").exists());
  repo.git(&["add", "unrelated.txt"]);
  assert_eq!(repo.git(&["status", "--porcelain=v1"]), "M  unrelated.txt");

  // a move that never ran releases its lock
  let (repo, old, new) = repo_at_old();
  let lock = index_lock(&repo);
  let files = WorktreeUpdate::prepare_from(repo.git_executor(), repo.path_str(), &old, &new, &lock, false, 0).unwrap();
  assert!(repo.path().join(".git/index.lock").exists());
  drop(files);
  assert!(!repo.path().join(".git/index.lock").exists());
}

/// A pick that moved `master` of `repo` from `old` to `new` and was killed after its files followed and before its
/// index did: the files of the new commit, the index of the old one, the mark of the pending files
fn killed_before_the_index(repo: &TestRepo, old: &str, new: &str) -> String {
  let index = repo.path().join(".git/index");
  let saved = repo.path().join(".git/index.saved");
  std::fs::copy(&index, &saved).unwrap();
  assert_eq!(follow(repo, old, new, false), (Ok(()), true));
  sync_utils::fs_retry::rename(&saved, &index).unwrap();
  repo.git(&["update-ref", "refs/heads/master", old]);
  repo.git(&["update-ref", "-m", "branch-deck: pick", "HEAD", new, old]);
  let mark = format!("{PENDING_UPDATE_PREFIX}main");
  repo.git(&["update-ref", &mark, old]);
  mark
}

/// The state a move of `repo_at_old` leaves on the whole index
fn moved_whole() -> State {
  let (repo, old, new) = repo_at_old();
  assert_eq!(follow(&repo, &old, &new, true), (Ok(()), false));
  state(&repo)
}

#[test]
fn a_move_killed_between_its_files_and_its_index_is_finished_by_the_next_one() {
  let (repo, old, new) = repo_at_old();
  let mark = killed_before_the_index(&repo, &old, &new);
  assert!(repo.git(&["status", "--porcelain=v1"]).contains("a/b/mod.txt"), "the index is behind the files");

  assert_eq!(finish_pending_update(repo.git_executor(), repo.path_str()).unwrap(), Some((old, new.clone())));
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &mark));
  assert_eq!(repo.git(&["rev-parse", "refs/heads/master"]), new, "the branch stays at the picked commit");
  assert_eq!(state(&repo), moved_whole());
}

#[test]
fn a_move_killed_in_the_middle_of_its_files_is_finished_by_the_next_one() {
  let (repo, old, new) = repo_at_old();
  let mark = killed_before_the_index(&repo, &old, &new);
  // these were not written yet: a changed file, an added one, a deleted one, the file that becomes a directory
  repo.write_file("a/b/mod.txt", "one\n");
  sync_utils::fs_retry::remove_file(repo.path().join("a/added.txt")).unwrap();
  repo.write_file("a/b/gone-too.txt", "gone\n");
  sync_utils::fs_retry::remove_dir_all(repo.path().join("becomes-dir")).unwrap();
  repo.write_file("becomes-dir", "a file\n");

  assert_eq!(finish_pending_update(repo.git_executor(), repo.path_str()).unwrap(), Some((old, new.clone())));
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &mark));
  assert_eq!(repo.git(&["rev-parse", "refs/heads/master"]), new);
  assert_eq!(state(&repo), moved_whole());
}

#[test]
fn a_killed_move_with_a_local_change_on_a_path_it_did_not_write_moves_the_branch_back() {
  let (repo, old, new) = repo_at_old();
  let mark = killed_before_the_index(&repo, &old, &new);
  repo.write_file("a/b/mod.txt", "local\n");

  let error = finish_pending_update(repo.git_executor(), repo.path_str()).unwrap_err();
  let blocked = error.downcast_ref::<FilesBlocked>().unwrap_or_else(|| panic!("{error:#}"));
  assert!(blocked.paths.contains(&"a/b/mod.txt".to_string()), "{blocked:?}");
  assert_eq!(repo.git(&["rev-parse", "refs/heads/master"]), old);
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &mark));
  assert_eq!(std::fs::read_to_string(repo.path().join("a/b/mod.txt")).unwrap(), "local\n", "the local change stays");
}

/// The rig behind the numbers of "The files of a move" (docs/design/git-operations.md): the files of the throwaway
/// repository `MOVE_FILES_REPO`, whose HEAD is on a branch at `MOVE_FILES_FROM`, follow a move of it to
/// `MOVE_FILES_TO`, after the exact dry run of the move, by the small index or, with `MOVE_FILES_WHOLE`, on the whole
/// one; the wall time and the git processes of the dry run, then of both, are printed. It writes the repository: never one of the user's. `cargo nextest run -p git-ops
/// --run-ignored only -E 'test(/the_files_of_a_move_of_a_rig/)' --no-capture`
#[test]
#[ignore = "a measurement on a throwaway repository: MOVE_FILES_REPO, MOVE_FILES_FROM, MOVE_FILES_TO, MOVE_FILES_WHOLE"]
fn the_files_of_a_move_of_a_rig() {
  let (Ok(repo), Ok(from), Ok(to)) = (std::env::var("MOVE_FILES_REPO"), std::env::var("MOVE_FILES_FROM"), std::env::var("MOVE_FILES_TO")) else {
    return;
  };
  let whole = std::env::var("MOVE_FILES_WHOLE").is_ok();
  let git = GitCommandExecutor::new().with_process_stats("move");
  let lock = IndexLock::of(&git, &repo);
  let dry_run_started = std::time::Instant::now();
  let refusal = dry_run_from(&git, &repo, &from, &to, true, if whole { u64::MAX } else { 0 }).unwrap();
  println!("the dry run in {:?}: {refusal:?}", dry_run_started.elapsed());
  for (command, stats) in git.process_stats().unwrap().by_command() {
    println!("  dry run {command}: {} in {:?}", stats.count, stats.elapsed);
  }
  let started = std::time::Instant::now();
  let files = if whole {
    WorktreeUpdate::prepare_whole(&git, &repo, &from, &to, &lock)
  } else {
    WorktreeUpdate::prepare_from(&git, &repo, &from, &to, &lock, false, 0)
  }
  .unwrap();
  let prepared = started.elapsed();
  let small = files.is_small();
  git.cmd(&["update-ref", "HEAD", &to, &from], &repo).text().unwrap();
  let moved = std::time::Instant::now();
  files.apply().unwrap();
  println!(
    "small index: {small}; prepared in {prepared:?}, the files and the index in {:?}, {:?} in all",
    moved.elapsed(),
    started.elapsed()
  );
  for (command, stats) in git.process_stats().unwrap().by_command() {
    println!("  {command}: {} in {:?}", stats.count, stats.elapsed);
  }
}

#[test]
fn a_small_index_file_merges_whole() {
  // the small index costs eight processes more than the whole one: a repository of a few files keeps the two it had
  let (repo, old, new) = repo_at_old();
  let lock = index_lock(&repo);
  let git = GitCommandExecutor::new().with_process_stats("test");
  assert_eq!(dry_run_worktree_update(&git, repo.path_str(), &old, &new, true).unwrap(), None);
  let files = WorktreeUpdate::prepare(&git, repo.path_str(), &old, &new, &lock, false).unwrap();
  assert!(!files.is_small());
  repo.git(&["update-ref", "refs/heads/master", &new, &old]);
  files.apply().unwrap();
  let counts = git.process_stats().unwrap().counts();
  // the git dir once, the dry run on a copy of the index with its refresh, the refresh of the user's index, the merge
  assert_eq!(counts, [("read-tree".to_string(), 2), ("rev-parse".to_string(), 1), ("update-index".to_string(), 2)]);
  assert_eq!(repo.git(&["status", "--porcelain=v1"]), "");
}

/// Entries of files that are not there, with paths long enough for an index file of `SMALL_INDEX_FROM` bytes in a
/// few thousand entries: the index of a large repository, without its files
fn pad_index(repo: &TestRepo) {
  let blob = repo.git(&["rev-parse", ":keep/same.txt"]);
  let directory = format!("pad/{}", vec!["d".repeat(200); 19].join("/"));
  let mut entries = String::new();
  let count = SMALL_INDEX_FROM / 3_800 + 1;
  for n in 0..count {
    let _ = write!(entries, "100644 {blob} 0\t{directory}/{n}\0");
  }
  repo
    .git_executor()
    .cmd(&["update-index", "-z", "--index-info"], repo.path_str())
    .stdin(entries.as_bytes())
    .text()
    .unwrap();
  assert!(std::fs::metadata(repo.path().join(".git/index")).unwrap().len() >= SMALL_INDEX_FROM);
}

#[test]
fn a_move_in_a_large_index_follows_by_the_small_index_and_its_dry_run_merges_it() {
  let (repo, old, new) = repo_at_old();
  pad_index(&repo);
  let git = GitCommandExecutor::new().with_process_stats("test");
  assert_eq!(dry_run_worktree_update(&git, repo.path_str(), &old, &new, true).unwrap(), None);
  // the changed paths, their trees, the cut, the small index with its stat data, the merge without a write; the git
  // dir once
  assert_eq!(
    git.process_stats().unwrap().counts(),
    [
      ("diff-tree".to_string(), 1),
      ("ls-files".to_string(), 1),
      ("read-tree".to_string(), 1),
      ("rev-parse".to_string(), 1),
      ("update-index".to_string(), 3),
      ("write-tree".to_string(), 1),
    ]
  );
  repo.write_file("a/b/mod.txt", "one\nlocal\n");
  let refusal = dry_run_worktree_update(repo.git_executor(), repo.path_str(), &old, &new, true).unwrap();
  assert!(refusal.is_some_and(|refusal| refusal.contains("a/b/mod.txt")));
  repo.write_file("a/b/mod.txt", "one\n");

  // the move of the branch with its mark, as a pick moves it
  let git = GitCommandExecutor::new().with_process_stats("test");
  move_branch_and_files(&git, repo.path_str(), &HeadRef::of("master"), &old, &new, true, "pick").unwrap();
  let counts = git.process_stats().unwrap().counts();
  // the files by the small index: the changed paths, their trees (`update-index`, `write-tree`), the cut
  // (`ls-files`), the small index and its stat data (`update-index` twice), the merge, its entries (`ls-files`), the
  // copy of the user's index and its stat data (`update-index` twice)
  let of = |command: &str| counts.iter().find(|(name, _)| name == command).map_or(0, |(_, count)| *count);
  assert_eq!(
    (of("diff-tree"), of("ls-files"), of("read-tree"), of("update-index"), of("write-tree")),
    (1, 2, 1, 5, 1),
    "{counts:?}"
  );
  assert_eq!(repo.git(&["rev-parse", "refs/heads/master"]), new);
  assert_eq!(repo.git(&["log", "-g", "-1", "--format=%gs", "refs/heads/master"]), "branch-deck: pick");
  assert!(!ref_exists(repo.git_executor(), repo.path_str(), &format!("{PENDING_UPDATE_PREFIX}main")));
  assert!(!repo.path().join(".git/index.lock").exists());
  // the files of the move, and no change but the padding, whose files were never there
  assert_eq!(std::fs::read_to_string(repo.path().join("a/b/mod.txt")).unwrap(), "two\n");
  assert_eq!(std::fs::read_to_string(repo.path().join("big.shout")).unwrap(), "POINTER TWO\n");
  let changed = repo.git(&["status", "--porcelain=v1", "--untracked-files=all"]);
  let beside_the_padding: Vec<&str> = changed.lines().filter(|line| !line[3..].starts_with("pad/")).collect();
  assert_eq!(beside_the_padding, Vec::<&str>::new());
}
