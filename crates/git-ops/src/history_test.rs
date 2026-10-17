use super::history::History;
use super::rewrite_test_utils::repo_path;
use anyhow::Result;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// `count` commits on master, oldest first
fn linear(repo: &TestRepo, count: usize) -> Vec<String> {
  (0..count)
    .map(|index| repo.create_commit(&format!("Commit {index}"), &format!("f{index}.txt"), "x"))
    .collect()
}

/// A merge of a side branch with one commit into master after `master_commits`, then one more commit; returns the side
/// commit and the merge
fn with_merge(repo: &TestRepo, master_commits: usize) -> Result<(String, String)> {
  for index in 0..master_commits {
    repo.create_commit(&format!("Before {index}"), &format!("before-{index}.txt"), "x");
  }
  repo.checkout_new_branch("side").map_err(anyhow::Error::msg)?;
  let side = repo.create_commit("Side", "side.txt", "side");
  repo.checkout("master").map_err(anyhow::Error::msg)?;
  repo.create_commit("Main", "main.txt", "main");
  let merge = repo.merge_no_ff("side", "Merge side").map_err(anyhow::Error::msg)?;
  repo.create_commit("After", "after.txt", "after");
  Ok((side, merge))
}

/// The walk stops at the parent of the oldest target: a long history below it is not read
#[test]
fn reads_down_to_the_parent_of_the_oldest_target() -> Result<()> {
  let repo = TestRepo::new();
  let commits = linear(&repo, 8);
  let history = History::read_to(repo.git_executor(), repo_path(&repo), &[&commits[6], &commits[4]])?;

  assert_eq!(history.head(), Some(commits[7].as_str()));
  let read: Vec<&str> = history.commits().iter().map(|commit| commit.id.as_str()).collect();
  assert_eq!(read, [&commits[7], &commits[6], &commits[5], &commits[4]]);
  assert_eq!(history.position(&commits[4])?, 3);
  assert_eq!(history.parent(3), Some(commits[3].as_str()));
  assert_eq!(
    history.parent_tree(3),
    Some(repo.rev_parse(&format!("{}^{{tree}}", commits[3])).map_err(anyhow::Error::msg)?.as_str())
  );
  assert_eq!(history.tree_of(&commits[3]), history.parent_tree(3));
  Ok(())
}

/// The root commit as a target: there is nothing below it
#[test]
fn reads_down_to_the_root() -> Result<()> {
  let repo = TestRepo::new();
  let commits = linear(&repo, 3);
  let history = History::read_to(repo.git_executor(), repo_path(&repo), &[&commits[0]])?;

  assert_eq!(history.commits().len(), 3);
  assert_eq!(history.parent(2), None);
  assert_eq!(history.parent_tree(2), None);
  Ok(())
}

/// A commit that is not on the first-parent line of HEAD (the side of a merge, or no commit at all) is refused by
/// `position`, with the message of a stale request
#[test]
fn refuses_a_commit_off_the_first_parent_line() -> Result<()> {
  let repo = TestRepo::new();
  let (side, merge) = with_merge(&repo, 2)?;
  let history = History::read_to(repo.git_executor(), repo_path(&repo), &[&side])?;

  let error = history.position(&side).expect_err("the side commit is not on the first-parent line");
  assert_eq!(error.to_string(), format!("Commit {} is not on the current branch, sync and try again", &side[..8]));
  assert!(history.find(&merge).is_some_and(|position| history.commit(position).merge));
  assert!(
    History::read_to(repo.git_executor(), repo_path(&repo), &["0123456789abcdef0123456789abcdef01234567"])?
      .find("0123456789abcdef0123456789abcdef01234567")
      .is_none()
  );
  Ok(())
}

/// The rewrite guard finds a merge among the commits from the oldest rewritten one up to HEAD without a process, and
/// not one below them
#[test]
fn the_guard_refuses_a_merge_in_the_range_only() -> Result<()> {
  let repo = TestRepo::new();
  let (_, merge) = with_merge(&repo, 2)?;
  let history = History::read_to(repo.git_executor(), repo_path(&repo), &[&merge])?;
  let merge_position = history.position(&merge)?;

  let error = history
    .check_rewrite(repo.git_executor(), repo_path(&repo), merge_position)
    .expect_err("a merge is in the range");
  assert!(
    error.to_string().starts_with(&format!("Cannot rewrite history that contains merge commit {}", &merge[..8])),
    "{error}"
  );
  let target = history.check_rewrite(repo.git_executor(), repo_path(&repo), merge_position - 1)?;
  assert_eq!((target.branch.as_str(), Some(target.head.as_str())), ("master", history.head()));
  Ok(())
}

/// Ahead of the baseline: the commits above it, newest first, and the tree of the commit below them, also when a merge
/// among them adds its second parent to the boundary
#[test]
fn reads_the_commits_ahead_of_the_baseline() -> Result<()> {
  let repo = TestRepo::new();
  let base = linear(&repo, 2);
  repo.git(&["branch", "baseline", &base[1]]);
  let (_, merge) = with_merge(&repo, 1)?;
  let history = History::read_ahead_of(repo.git_executor(), repo_path(&repo), "baseline", true)?;

  // After, the merge, Main and the first commit after the baseline; Side is a boundary commit only
  assert_eq!(history.commits().len(), 4);
  assert_eq!(history.baseline(), Some("baseline"));
  assert_eq!(history.find(&merge), Some(1));
  assert_eq!(history.parent(3), Some(base[1].as_str()));
  assert_eq!(history.parent_tree(3), Some(repo.rev_parse("baseline^{tree}").map_err(anyhow::Error::msg)?.as_str()));
  Ok(())
}
