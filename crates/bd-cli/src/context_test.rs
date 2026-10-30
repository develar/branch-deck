use crate::fixture_test::{PREFIX, context_with_the_config_prefix, repo_with_baseline};
use pretty_assertions::assert_eq;

/// The config the context read for the branch prefix is the one of the whole command (the sync takes it); after a wait
/// for the repository lock it is read again: another process may have changed it meanwhile
#[test_log::test]
fn the_config_is_read_once_and_again_after_a_wait_for_the_lock() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let context = context_with_the_config_prefix(&repo);
  let reference = format!("refs/heads/{PREFIX}/virtual/auth");
  repo.set_config(&format!("branch.{PREFIX}/virtual/auth.description"), "Add login\n").unwrap();
  assert_eq!(context.config().unwrap().descriptions().get(&reference), None, "read with the prefix");

  context.forget_config();
  assert_eq!(context.config().unwrap().descriptions()[&reference].title, "Add login");
}

/// HEAD's branch, the remotes with their push URLs and the baseline are read once for the whole command (its sync and
/// the push URL of `origin` take them); after a wait for the repository lock they are read again, as the config
#[test_log::test]
fn the_facts_are_read_once_and_again_after_a_wait_for_the_lock() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let context = context_with_the_config_prefix(&repo);
  let branch = context.head_branch().unwrap();
  let url = context.origin_url().unwrap();
  let baseline = context.baseline().unwrap();
  let processes = context.git.process_stats().unwrap().total();

  repo.git(&["remote", "set-url", "--push", "origin", "ssh://git@git.jetbrains.team/ij/app.git"]);
  repo.git(&["switch", "-q", "--detach"]);
  assert_eq!(context.head_branch().unwrap(), branch);
  assert_eq!(context.origin_url().unwrap(), url);
  assert_eq!(context.baseline().unwrap(), baseline);
  assert_eq!(context.git.process_stats().unwrap().total(), processes, "no process for a fact read already");

  context.forget_facts();
  assert_eq!(context.head_branch().unwrap(), None);
  assert_eq!(context.origin_url().unwrap().as_deref(), Some("ssh://git@git.jetbrains.team/ij/app.git"));
  assert_eq!(context.baseline().unwrap().branch, None);
}
