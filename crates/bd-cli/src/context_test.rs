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
