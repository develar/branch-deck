use super::copy_commit::CopyCommitError;
use super::history::History;
use super::ref_transaction::RefTransaction;
use super::revs::resolve_tree_id;
use super::rewrite_guard::HeadMoved;
use super::rewrite_plan::PreparedRewrite;
use super::rewrite_test_utils::{assert_no_temp_refs, commit_count, log_format, repo_path, show, stage_file, worktree_snapshot, write_file};
use super::squash_commits::{SquashGroup, SquashMessage, prepare_squash, restore_branch_head, restore_branch_head_with};
use anyhow::Result;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn group(target: &str, folded: &[&str], message: SquashMessage) -> SquashGroup {
  SquashGroup {
    target: target.to_string(),
    folded: folded.iter().map(|id| (*id).to_string()).collect(),
    message,
  }
}

fn text(message: &str) -> SquashMessage {
  SquashMessage::Text(message.to_string())
}

fn prepare(repo: &TestRepo, groups: &[SquashGroup]) -> Result<PreparedRewrite, CopyCommitError> {
  let ids: Vec<&str> = groups
    .iter()
    .flat_map(|group| std::iter::once(group.target.as_str()).chain(group.folded.iter().map(String::as_str)))
    .collect();
  let history = History::read_to(repo.git_executor(), repo_path(repo), &ids)?;
  prepare_squash(repo.git_executor(), repo_path(repo), &history, groups)
}

/// Squashes `folded` into `target`; returns the prepared rewrite (moved already)
fn squash(repo: &TestRepo, target: &str, folded: &[&str], message: &str) -> Result<PreparedRewrite> {
  let prepared = prepare(repo, &[group(target, folded, text(message))])?;
  prepared.apply(repo.git_executor(), repo_path(repo), RefTransaction::new(), "squash 2 commits")?;
  Ok(prepared)
}

/// The squash is refused with an error containing `expected`, and nothing changed
fn assert_refused(repo: &TestRepo, groups: &[SquashGroup], expected: &str) -> Result<()> {
  let head_before = repo.head();
  let error = prepare(repo, groups).expect_err("squash should be refused");
  assert!(error.to_string().contains(expected), "{error}");
  assert_eq!(repo.head(), head_before);
  assert_no_temp_refs(repo)
}

/// "Base", then "Add a" (the target) and "Add b" (the folded commit, HEAD)
fn base_a_b() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit("Add a", "a.txt", "a");
  let folded = repo.create_commit("Add b", "b.txt", "b");
  (repo, target, folded)
}

fn git(repo: &TestRepo, args: &[&str]) -> Result<String> {
  Ok(String::from_utf8_lossy(&repo.git_executor().cmd(args, repo_path(repo)).bytes()?).into_owned())
}

fn subjects(repo: &TestRepo) -> Result<String> {
  repo.git_executor().execute_command(&["log", "--format=%s", "HEAD"], repo_path(repo))
}

#[test]
fn squashes_adjacent_commits_into_the_oldest() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit_with_timestamp("(auth) Add login", "login.txt", "login", Some(1_700_000_000));
  let folded = repo.create_commit("(auth) Add logout", "logout.txt", "logout");
  repo.create_commit("Later", "later.txt", "later");
  let author = log_format(&repo, &target, "%an <%ae> %ad")?;

  let prepared = squash(&repo, &target, &[&folded], "(auth) Add login and logout\n\nBoth flows")?;

  assert_eq!(subjects(&repo)?, "Later\n(auth) Add login and logout\nBase");
  let squashed = prepared.replacement_of(&target).expect("the target is replaced").to_string();
  assert_eq!(git(&repo, &["rev-parse", "HEAD~1"])?.trim(), squashed);
  assert_eq!(log_format(&repo, &squashed, "%B")?, "(auth) Add login and logout\n\nBoth flows");
  assert_eq!(log_format(&repo, &squashed, "%an <%ae> %ad")?, author);
  assert_eq!(show(&repo, &squashed, "logout.txt")?, "logout");
  assert_eq!(prepared.new_head(), repo.head());
  assert_no_temp_refs(&repo)
}

#[test]
fn keeps_the_commit_in_between_of_another_branch() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit("(auth) Add login", "login.txt", "1\n");
  repo.create_commit("(docs) Document", "docs.md", "docs");
  let folded = repo.create_commit("(auth) Fix login", "login.txt", "2\n");
  let head_tree = resolve_tree_id(repo.git_executor(), repo_path(&repo), "HEAD")?;

  squash(&repo, &target, &[&folded], "(auth) Add login\n")?;

  assert_eq!(subjects(&repo)?, "(docs) Document\n(auth) Add login\nBase");
  assert_eq!(show(&repo, "HEAD~1", "login.txt")?, "2");
  assert_eq!(resolve_tree_id(repo.git_executor(), repo_path(&repo), "HEAD")?, head_tree);
  assert_no_temp_refs(&repo)
}

#[test]
fn squashes_head_into_an_older_commit() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit("Add a", "a.txt", "a");
  repo.create_commit("Add b", "b.txt", "b");
  let head = repo.create_commit("Add c", "c.txt", "c");

  squash(&repo, &target, &[&head], "Add a and c\n")?;

  assert_eq!(subjects(&repo)?, "Add b\nAdd a and c\nBase");
  assert_eq!(show(&repo, "HEAD~1", "c.txt")?, "c");
  assert_eq!(commit_count(&repo)?, 3);
  assert_no_temp_refs(&repo)
}

#[test]
fn squashes_several_groups_in_one_rewrite() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let a = repo.create_commit("Add a", "a.txt", "a");
  let b = repo.create_commit("Add b", "b.txt", "b");
  let a2 = repo.create_commit("Fix a", "a.txt", "a2");
  let b2 = repo.create_commit("Fix b", "b.txt", "b2");

  let groups = [group(&a, &[&a2], text("Add a\n")), group(&b, &[&b2], text("Add b\n"))];
  prepare(&repo, &groups)?.apply(repo.git_executor(), repo_path(&repo), RefTransaction::new(), "squash")?;

  assert_eq!(subjects(&repo)?, "Add b\nAdd a\nBase");
  assert_eq!(show(&repo, "HEAD~1", "a.txt")?, "a2");
  assert_eq!(show(&repo, "HEAD", "b.txt")?, "b2");
  assert_no_temp_refs(&repo)
}

#[test]
fn target_message_keeps_the_bytes_and_the_encoding() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  repo.set_config("i18n.commitEncoding", "ISO-8859-1").map_err(anyhow::Error::msg)?;
  // "Caf\xe9" in Latin-1, written as is
  let message = b"Caf\xe9\n";
  let tree = resolve_tree_id(repo.git_executor(), repo_path(&repo), "HEAD")?;
  let head = repo.head();
  let target = repo
    .git_executor()
    .cmd(&["commit-tree", &tree, "-p", &head, "-F", "-"], repo_path(&repo))
    .stdin(message)
    .bytes()?;
  let target = String::from_utf8(target)?.trim().to_string();
  git(&repo, &["update-ref", "HEAD", &target])?;
  let folded = repo.create_commit("fixup! Café", "a.txt", "a");

  let prepared = prepare(&repo, &[group(&target, &[&folded], SquashMessage::Target)])?;
  prepared.apply(repo.git_executor(), repo_path(&repo), RefTransaction::new(), "squash")?;

  let raw = repo.git_executor().cmd(&["cat-file", "commit", "HEAD"], repo_path(&repo)).bytes()?;
  assert!(raw.windows(20).any(|w| w == b"encoding ISO-8859-1\n"), "{}", String::from_utf8_lossy(&raw));
  assert!(raw.ends_with(message));
  assert_eq!(show(&repo, "HEAD", "a.txt")?, "a");
  assert_no_temp_refs(&repo)
}

#[test]
fn refuses_a_range_with_merges() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let branch = git(&repo, &["symbolic-ref", "--short", "HEAD"])?.trim().to_string();
  let target = repo.create_commit("Add a", "a.txt", "a");
  git(&repo, &["checkout", "-q", "-b", "side"])?;
  repo.create_commit("Side", "side.txt", "side");
  git(&repo, &["checkout", "-q", &branch])?;
  let folded = repo.create_commit("Add b", "b.txt", "b");
  git(&repo, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"])?;

  assert_refused(&repo, &[group(&target, &[&folded], text("Add a and b\n"))], "merge commit")
}

#[test]
fn keeps_dirty_and_staged_changes_byte_for_byte() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base\n");
  let target = repo.create_commit("Add a", "a.txt", "a\n");
  let folded = repo.create_commit("Add b", "b.txt", "b\n");
  repo.create_commit("Add c", "c.txt", "c\n");
  // staged and unstaged changes, also on a squashed file, and an untracked file
  stage_file(&repo, "a.txt", "a staged\n")?;
  write_file(&repo, "a.txt", "a staged and dirty\n")?;
  stage_file(&repo, "new.txt", "new\n")?;
  write_file(&repo, "b.txt", "b dirty\n")?;
  write_file(&repo, "untracked.txt", "untracked\n")?;
  let before = worktree_snapshot(&repo)?;

  squash(&repo, &target, &[&folded], "Add a and b\n")?;

  assert_eq!(subjects(&repo)?, "Add c\nAdd a and b\nBase");
  assert_eq!(worktree_snapshot(&repo)?, before);
  assert_eq!(std::fs::read_to_string(repo.path().join("a.txt"))?, "a staged and dirty\n");
  Ok(())
}

#[test]
fn branch_moved_after_prepare_changes_nothing() -> Result<()> {
  let (repo, target, folded) = base_a_b();
  let prepared = prepare(&repo, &[group(&target, &[&folded], text("Add a and b\n"))])?;
  // a commit made meanwhile, e.g. in a terminal
  let concurrent = repo.create_commit("Concurrent", "c.txt", "c");

  let error = prepared
    .apply(repo.git_executor(), repo_path(&repo), RefTransaction::new(), "squash")
    .expect_err("the branch moved");

  assert!(error.to_string().contains("changed while rewriting"), "{error}");
  assert_eq!(repo.head(), concurrent);
  assert_no_temp_refs(&repo)
}

#[test]
fn records_the_reflog_message() -> Result<()> {
  let (repo, target, folded) = base_a_b();

  squash(&repo, &target, &[&folded], "Add a and b\n")?;

  assert_eq!(git(&repo, &["reflog", "-1", "--format=%gs"])?.trim(), "branch-deck: squash 2 commits");
  Ok(())
}

#[test]
fn validates_the_groups() -> Result<()> {
  let repo = TestRepo::new();
  let root = repo.create_commit("Base", "base.txt", "base");
  let a = repo.create_commit("Add a", "a.txt", "a");
  let b = repo.create_commit("Add b", "b.txt", "b");
  let c = repo.create_commit("Add c", "c.txt", "c");

  assert_refused(&repo, &[], "No commits")?;
  assert_refused(&repo, &[group(&a, &[], text("Add a\n"))], "at least two")?;
  assert_refused(&repo, &[group(&a, &[&b, &b], text("Add a\n"))], "squashed twice")?;
  assert_refused(&repo, &[group(&a, &[&b], text("Add a\n")), group(&c, &[&b], text("Add c\n"))], "squashed twice")?;
  assert_refused(&repo, &[group(&b, &[&a], text("Add b\n"))], "older than")?;
  assert_refused(&repo, &[group(&a, &[&b], text("\n\nbody only"))], "subject")?;
  // not on the first-parent line of HEAD
  git(&repo, &["branch", "side", &root])?;
  git(&repo, &["update-ref", "HEAD", &b])?;
  assert_refused(&repo, &[group(&a, &[&c], text("Add a\n"))], "not on the current branch")
}

#[test]
fn undo_restores_the_previous_head() -> Result<()> {
  let (repo, target, folded) = base_a_b();
  let previous_head = repo.head();
  write_file(&repo, "a.txt", "dirty")?;
  let prepared = squash(&repo, &target, &[&folded], "Add a and b\n")?;

  restore_branch_head(repo.git_executor(), repo_path(&repo), prepared.new_head(), &previous_head, "undo squash")?;

  assert_eq!(repo.head(), previous_head);
  assert_eq!(git(&repo, &["reflog", "-1", "--format=%gs"])?.trim(), "branch-deck: undo squash");
  assert_eq!(git(&repo, &["status", "--porcelain", "--untracked-files=no"])?.trim_end(), " M a.txt");
  Ok(())
}

/// `base_a_b` with b squashed into a; returns the head before and after the squash
fn squashed_a_b() -> Result<(TestRepo, String, String)> {
  let (repo, target, folded) = base_a_b();
  let previous_head = repo.head();
  let new_head = squash(&repo, &target, &[&folded], "Add a and b\n")?.new_head().to_string();
  Ok((repo, previous_head, new_head))
}

#[test]
fn undo_is_refused_when_the_branch_moved() -> Result<()> {
  let (repo, previous_head, new_head) = squashed_a_b()?;
  let later = repo.create_commit("Later", "c.txt", "c");

  // the moved branch is the typed `HeadMoved`, which an undo answers as `undo_stale`
  let error = restore_branch_head(repo.git_executor(), repo_path(&repo), &new_head, &previous_head, "undo squash")
    .expect_err("the branch moved")
    .into_anyhow();
  let moved = error.downcast_ref::<HeadMoved>().unwrap_or_else(|| panic!("not HeadMoved: {error:#}"));
  assert_eq!((moved.expected.as_str(), moved.actual.as_str()), (new_head.as_str(), later.as_str()));
  assert_eq!(repo.head(), later);

  // another tree: the index and the working tree would no longer match
  let error = restore_branch_head(repo.git_executor(), repo_path(&repo), &later, &previous_head, "undo squash").expect_err("the trees differ");
  assert!(error.to_string().contains("other files"), "{error}");
  assert_eq!(repo.head(), later);
  Ok(())
}

/// A `reference-transaction` hook that moves the branch between the check of an undo and its transaction, as a commit
/// in a terminal could: git refuses the first transaction after it is installed (`prepared`, the locks held), and once
/// git released the locks (`aborted`) the hook moves `branch_ref` to `moved_to` itself; every later transaction runs
fn move_the_branch_in_the_next_transaction(repo: &TestRepo, branch_ref: &str, moved_to: &str) -> Result<()> {
  let hooks = repo.path().join(".git/test-hooks");
  std::fs::create_dir_all(&hooks)?;
  let done = repo.path().join(".git/test-hook-done");
  let hook = hooks.join("reference-transaction");
  let done = done.to_str().unwrap();
  std::fs::write(
    &hook,
    format!(
      "#!/bin/sh\ncat >/dev/null\n[ -e '{done}' ] && exit 0\ncase \"$1\" in\n  prepared) exit 1 ;;\n  aborted) touch '{done}'; git update-ref {branch_ref} {moved_to} ;;\nesac\nexit 0\n"
    ),
  )?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
  }
  repo.set_config("core.hooksPath", hooks.to_str().unwrap()).map_err(anyhow::Error::msg)
}

/// A branch that moves between the check and the transaction is `HeadMoved` too, with the commit it moved to: the
/// failed transaction is explained by a new read of the branch
#[test]
fn undo_is_refused_when_the_branch_moved_during_the_transaction() -> Result<()> {
  let (repo, previous_head, new_head) = squashed_a_b()?;
  // a commit on top with the same files, so the check of the trees passes and only the moved branch refuses
  let later = repo
    .git(&["commit-tree", &format!("{new_head}^{{tree}}"), "-p", &new_head, "-m", "Later"])
    .trim()
    .to_string();
  move_the_branch_in_the_next_transaction(&repo, "refs/heads/master", &later)?;

  let error = restore_branch_head(repo.git_executor(), repo_path(&repo), &new_head, &previous_head, "undo squash")
    .expect_err("the branch moved")
    .into_anyhow();
  let moved = error.downcast_ref::<HeadMoved>().unwrap_or_else(|| panic!("not HeadMoved: {error:#}"));
  assert_eq!(
    (moved.branch.as_str(), moved.expected.as_str(), moved.actual.as_str()),
    ("master", new_head.as_str(), later.as_str())
  );
  assert_eq!(repo.head(), later);
  Ok(())
}

/// A transaction that fails while the branch stays where the undo expects it, because a ref moved with it (`also`: the
/// issue groups of an undo of a regroup) is elsewhere, is no `HeadMoved`: the branch did not move, and the error names
/// the failed move; nothing moves
#[test]
fn undo_whose_other_ref_moved_is_not_head_moved() -> Result<()> {
  let (repo, previous_head, new_head) = squashed_a_b()?;
  repo.git(&["update-ref", "refs/branch-deck/issue-groups", &previous_head]);
  let mut also = RefTransaction::new();
  also.update("refs/branch-deck/issue-groups", &previous_head, Some(&new_head));

  let error = restore_branch_head_with(repo.git_executor(), repo_path(&repo), &new_head, &previous_head, "undo regroup", &also)
    .expect_err("the side ref is elsewhere")
    .into_anyhow();
  assert!(error.downcast_ref::<HeadMoved>().is_none(), "HeadMoved: {error:#}");
  assert!(format!("{error:#}").starts_with("Cannot move branch master back"), "{error:#}");
  assert_eq!(repo.head(), new_head);
  assert_eq!(repo.rev_parse("refs/branch-deck/issue-groups").map_err(anyhow::Error::msg)?, previous_head);
  Ok(())
}

/// The heads of an undo come from the app or an undo token: a revision expression or an option is refused before any
/// git reads it
#[test]
fn undo_takes_only_full_commit_ids() -> Result<()> {
  let (repo, previous_head, new_head) = squashed_a_b()?;
  for (expected, previous) in [(new_head.as_str(), "HEAD@{1}"), (new_head.as_str(), "--all"), ("HEAD", previous_head.as_str())] {
    let error = restore_branch_head(repo.git_executor(), repo_path(&repo), expected, previous, "undo squash").expect_err("not a commit id");
    assert!(error.to_string().contains("is not a full commit id"), "{expected} {previous}: {error}");
    assert_eq!(repo.head(), new_head);
  }
  Ok(())
}
