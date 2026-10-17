use super::copy_commit::CopyCommitError;
use super::ref_transaction::RefTransaction;
use super::rewrite_plan::{History, PlanChecks, PlanStep, PreparedRewrite, Replacement, RewritePlan, drop_commits_from_head, prepare_plan};
use super::rewrite_test_utils::{assert_no_temp_refs, commit_count, log_format, repo_path, show, stage_file, status, write_file};
use anyhow::Result;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const SQUASH_CHECKS: PlanChecks = PlanChecks {
  keep_head_tree: true,
  check_commit_count: true,
};

/// A plan that replaces `target` with itself and the changes of `folded`
fn squash_plan(repo: &TestRepo, target: &str, folded: &[&str], message: &str) -> Result<(History, RewritePlan)> {
  let history = History::read(repo.git_executor(), repo_path(repo))?;
  let mut plan = RewritePlan::default();
  let replacement = Replacement {
    folded: folded.iter().map(|id| history.position(id)).collect::<Result<_, _>>()?,
    message: message.as_bytes().to_vec(),
    encoding: None,
  };
  plan.steps.insert(history.position(target)?, PlanStep::Replace(replacement));
  Ok((history, plan))
}

fn prepare(repo: &TestRepo, history: &History, plan: &RewritePlan) -> Result<PreparedRewrite, CopyCommitError> {
  prepare_plan(repo.git_executor(), repo_path(repo), history, plan, SQUASH_CHECKS)
}

fn squash(repo: &TestRepo, target: &str, folded: &[&str], message: &str) -> Result<()> {
  let (history, plan) = squash_plan(repo, target, folded, message)?;
  prepare(repo, &history, &plan)?.apply(repo.git_executor(), repo_path(repo), RefTransaction::new(), "squash")?;
  Ok(())
}

/// The squash is refused with an error containing `expected`, and nothing changed
fn assert_squash_refused(repo: &TestRepo, target: &str, folded: &[&str], expected: &str) -> Result<()> {
  let head_before = repo.head();
  let (history, plan) = squash_plan(repo, target, folded, "Squashed\n")?;
  let error = prepare(repo, &history, &plan).expect_err("squash should be refused");
  assert!(error.to_string().contains(expected), "{error}");
  assert_eq!(repo.head(), head_before);
  assert_no_temp_refs(repo)
}

fn tree(repo: &TestRepo, rev: &str) -> Result<String> {
  repo.git_executor().resolve_tree_id(repo_path(repo), rev)
}

#[test]
fn squash_adjacent_keeps_head_tree_and_target_author() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit_with_timestamp("Add a", "a.txt", "a", Some(1_700_000_000));
  let fixup = repo.create_commit("Add b", "b.txt", "b");
  repo.create_commit("Add c", "c.txt", "c");
  let head_tree = tree(&repo, "HEAD")?;
  let author = log_format(&repo, &target, "%an <%ae> %ad")?;
  write_file(&repo, "base.txt", "dirty")?;

  squash(&repo, &target, &[&fixup], "Add a and b\n")?;

  assert_eq!(commit_count(&repo)?, 3);
  assert_eq!(tree(&repo, "HEAD")?, head_tree);
  assert_eq!(log_format(&repo, "HEAD~1", "%B")?, "Add a and b");
  assert_eq!(log_format(&repo, "HEAD~1", "%an <%ae> %ad")?, author);
  assert_ne!(log_format(&repo, "HEAD~1", "%cd")?, log_format(&repo, "HEAD~1", "%ad")?);
  assert_eq!(show(&repo, "HEAD~1", "b.txt")?, "b");
  assert_eq!(log_format(&repo, "HEAD", "%s")?, "Add c");
  assert_eq!(status(&repo)?, " M base.txt");
  assert_no_temp_refs(&repo)
}

#[test]
fn squash_non_adjacent_moves_later_change_into_oldest() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit("Add a", "a.txt", "1\n");
  repo.create_commit("Add k", "k.txt", "k");
  let fixup = repo.create_commit("Change a", "a.txt", "2\n");
  let head_tree = tree(&repo, "HEAD")?;

  squash(&repo, &target, &[&fixup], "Add a\n")?;

  assert_eq!(commit_count(&repo)?, 3);
  assert_eq!(tree(&repo, "HEAD")?, head_tree);
  assert_eq!(show(&repo, "HEAD~1", "a.txt")?, "2");
  assert_eq!(log_format(&repo, "HEAD", "%s")?, "Add k");
  assert_eq!(status(&repo)?, "");
  assert_no_temp_refs(&repo)
}

#[test]
fn squash_conflicting_with_commit_in_between_changes_nothing() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit("Add x", "x.txt", "1\n");
  repo.create_commit("Change x", "x.txt", "2\n");
  let fixup = repo.create_commit("Change x again", "x.txt", "3\n");

  assert_squash_refused(&repo, &target, &[&fixup], "conflicts")
}

#[test]
fn refusal_after_writing_a_commit_leaves_no_temp_refs() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let target = repo.create_commit("Add a", "a.txt", "a");
  let fixup = repo.create_commit("Add b", "b.txt", "b");
  let dropped = repo.create_commit("Add c", "c.txt", "1\n");
  repo.create_commit("Change c", "c.txt", "2\n");
  let head_before = repo.head();
  let (history, mut plan) = squash_plan(&repo, &target, &[&fixup], "Add a and b\n")?;
  plan.steps.insert(history.position(&dropped)?, PlanStep::Drop);

  // the squashed commit is written first, then replaying the commit after the dropped one conflicts
  let error = prepare(&repo, &history, &plan).expect_err("the rewrite should conflict");

  assert!(error.to_string().contains("Removing commit"), "{error}");
  assert_eq!(repo.head(), head_before);
  assert_no_temp_refs(&repo)
}

#[test]
fn squash_refuses_empty_result() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "x.txt", "0\n");
  let target = repo.create_commit("Change x", "x.txt", "1\n");
  let revert = repo.create_commit("Revert x", "x.txt", "0\n");

  assert_squash_refused(&repo, &target, &[&revert], "leaves no changes")
}

#[test]
fn squash_refuses_kept_commit_that_becomes_empty() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "x.txt", "0\n");
  let target = repo.create_commit("Add a", "a.txt", "a");
  // after squashing, x is already 1 when this commit is replayed, so `git replay` drops it; HEAD's tree stays the same
  repo.create_commit("Set x", "x.txt", "1\n");
  let revert = repo.create_commit("Reset x", "x.txt", "0\n");
  let again = repo.create_commit("Set x again", "x.txt", "1\n");

  assert_squash_refused(&repo, &target, &[&revert, &again], "empty")
}

#[test]
fn squash_into_root_commit() -> Result<()> {
  let repo = TestRepo::new();
  let root = repo.create_commit("Add a", "a.txt", "a");
  repo.create_commit("Add k", "k.txt", "k");
  let fixup = repo.create_commit("Add b", "b.txt", "b");
  let head_tree = tree(&repo, "HEAD")?;

  squash(&repo, &root, &[&fixup], "Add a and b\n")?;

  assert_eq!(commit_count(&repo)?, 2);
  assert_eq!(tree(&repo, "HEAD")?, head_tree);
  assert_eq!(log_format(&repo, "HEAD~1", "%P")?, "");
  assert_eq!(show(&repo, "HEAD~1", "b.txt")?, "b");
  assert_no_temp_refs(&repo)
}

/// A staged change on a path the drop changes refuses it, and the error names that path as git stores it (a quote in
/// the name included), not the paths whose staged changes the drop keeps
#[test]
fn a_drop_under_a_staged_change_names_the_path() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let dropped = repo.create_commit("Add it's", "it's.txt", "one");
  repo.create_commit("Add other", "other.txt", "other");
  stage_file(&repo, "it's.txt", "staged")?;
  stage_file(&repo, "base.txt", "staged too")?;
  let head_before = repo.head();

  let error = drop_commits_from_head(repo.git_executor(), repo_path(&repo), &[dropped]).expect_err("the drop should be refused");

  assert!(error.to_string().ends_with("unstage or commit them first: it's.txt"), "{error}");
  assert_eq!(repo.head(), head_before);
  assert_no_temp_refs(&repo)
}
