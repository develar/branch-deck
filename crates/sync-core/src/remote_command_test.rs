use crate::remote_command::execute_remote_command;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The value of `key` as a remote command sees it
fn remote_config(test_repo: &TestRepo, key: &str) -> String {
  execute_remote_command(test_repo.git_executor(), &["config", "--get", key], test_repo.path().to_str().unwrap()).unwrap()
}

#[test]
fn remote_command_keeps_the_users_credential_helper() {
  let test_repo = TestRepo::new();
  let path = test_repo.path().to_str().unwrap();
  test_repo.git_executor().execute_command(&["config", "credential.helper", "osxkeychain"], path).unwrap();

  assert_eq!(remote_config(&test_repo, "credential.helper"), "osxkeychain");
}

#[test]
fn remote_command_never_prompts_and_aborts_stalled_transfers() {
  let test_repo = TestRepo::new();

  assert_eq!(remote_config(&test_repo, "credential.interactive"), "false");
  assert_eq!(remote_config(&test_repo, "http.lowSpeedLimit"), "1000");
  assert_eq!(remote_config(&test_repo, "http.lowSpeedTime"), "30");

  // a shell alias reports the environment git passes to the helpers it runs
  let gcm_interactive = execute_remote_command(
    test_repo.git_executor(),
    &["-c", "alias.gcm-interactive=!printf %s \"$GCM_INTERACTIVE\"", "gcm-interactive"],
    test_repo.path().to_str().unwrap(),
  )
  .unwrap();
  assert_eq!(gcm_interactive, "never");
}
