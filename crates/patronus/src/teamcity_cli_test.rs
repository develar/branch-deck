use crate::teamcity_cli::{LogTail, TeamCityCli, find_teamcity_in};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use test_utils::fake_executable::fake_executable;

const TIMEOUT: Duration = Duration::from_secs(10);

/// As `teamcity auth status --json` 1.5 answers: logged in to `ci.example`, not to `other.example`
const AUTH_STATUS: &str = r#"[
  {"server": "https://ci.example", "auth_method": "token", "token_source": "keyring", "user": {"id": 0, "username": "me", "name": "Me"}, "status": "authenticated"},
  {"server": "https://other.example", "auth_method": "token", "status": "expired"}
]"#;

/// As `teamcity run tests <id> --failed --json` answers, trimmed: `firstFailed` names an earlier build, this build, or
/// is absent
const FAILED_TESTS: &str = r#"{"count": 3, "failed": 3, "testOccurrence": [
  {"id": "build:(id:301),id:1", "name": "com.intellij.FooTest.testBar", "status": "FAILURE", "details": "boom",
   "firstFailed": {"id": "", "name": "", "status": "", "build": {"id": 290, "number": "1000444"}}},
  {"id": "build:(id:301),id:2", "name": "com.intellij.FooTest.testBaz", "status": "FAILURE",
   "firstFailed": {"build": {"id": 301, "number": "1000481"}}},
  {"id": "build:(id:301),id:3", "name": "com.intellij.NewTest.testIt", "status": "FAILURE"}
]}"#;

/// A `teamcity` that answers `auth status`, `run log` (its arguments and `TEAMCITY_URL` first, then a coloured line) and
/// `run tests`; `run log` of build 999 hangs
fn fake_cli(dir: &Path, auth_status: &str) -> PathBuf {
  std::fs::write(dir.join("auth.json"), auth_status).unwrap();
  std::fs::write(dir.join("tests.json"), FAILED_TESTS).unwrap();
  let shown = dir.display();
  fake_executable(
    dir,
    "teamcity",
    &format!(
      r#"case "$1 $2" in
  "auth status") cat '{shown}/auth.json' ;;
  "run log")
    if [ "$3" = 999 ]; then sleep 30; fi
    echo "$TEAMCITY_URL $*"
    printf '\033[31mCompilation failed\033[0m\n' ;;
  "run tests") cat '{shown}/tests.json' ;;
  *) echo "unknown command" >&2; exit 1 ;;
esac"#
    ),
  )
}

#[tokio::test]
async fn no_cli_without_the_binary_or_a_server_it_is_logged_in_to() {
  let dir = tempfile::tempdir().unwrap();
  assert!(TeamCityCli::open(dir.path().join("teamcity"), TIMEOUT).await.is_none());
  let logged_out = fake_cli(dir.path(), r#"[{"server": "https://ci.example", "status": "expired"}]"#);
  assert!(TeamCityCli::open(logged_out, TIMEOUT).await.is_none());
}

#[tokio::test]
async fn serves_only_the_builds_of_its_servers() {
  let dir = tempfile::tempdir().unwrap();
  let cli = TeamCityCli::open(fake_cli(dir.path(), AUTH_STATUS), TIMEOUT).await.unwrap();
  assert_eq!(cli.server_for("https://ci.example/build/300"), Some("https://ci.example"));
  assert_eq!(cli.server_for("https://other.example/build/300"), None);
  assert_eq!(cli.server_for("https://evil.example/https://ci.example/build/300"), None);
  assert_eq!(cli.server_for("not a url"), None);
}

#[tokio::test]
async fn the_log_tail_is_asked_of_the_server_of_the_build_without_colours() {
  let dir = tempfile::tempdir().unwrap();
  let cli = TeamCityCli::open(fake_cli(dir.path(), AUTH_STATUS), TIMEOUT).await.unwrap();
  assert_eq!(
    cli.log_tail("https://ci.example", "300").await,
    Some(LogTail {
      text: "https://ci.example run log 300 --tail 150 --raw --no-input --no-color\nCompilation failed".to_string(),
      truncated: false,
    })
  );
}

#[tokio::test]
async fn a_test_failed_before_when_its_first_failure_is_in_an_earlier_build() {
  let dir = tempfile::tempdir().unwrap();
  let cli = TeamCityCli::open(fake_cli(dir.path(), AUTH_STATUS), TIMEOUT).await.unwrap();
  assert_eq!(
    cli.first_failed_before("https://ci.example", "301").await,
    Some(HashMap::from([
      ("com.intellij.FooTest.testBar".to_string(), true),
      ("com.intellij.FooTest.testBaz".to_string(), false),
    ]))
  );
  // not a TeamCity build id
  assert_eq!(cli.first_failed_before("https://ci.example", "abc").await, None);
}

#[tokio::test]
async fn a_command_that_hangs_is_killed_after_the_timeout() {
  let dir = tempfile::tempdir().unwrap();
  let cli = TeamCityCli::open(fake_cli(dir.path(), AUTH_STATUS), TIMEOUT)
    .await
    .unwrap()
    .with_timeout(Duration::from_millis(300));
  let started = Instant::now();
  assert_eq!(cli.log_tail("https://ci.example", "999").await, None);
  assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_cli_on_path_comes_first() {
  let bin = tempfile::tempdir().unwrap();
  let expected = fake_executable(bin.path(), "teamcity", "true");
  assert_eq!(find_teamcity_in(None, Some(bin.path().as_os_str())), Some(expected));
  let home = tempfile::tempdir().unwrap();
  let local = home.path().join(".local/bin");
  std::fs::create_dir_all(&local).unwrap();
  let in_home = fake_executable(&local, "teamcity", "true");
  let empty = tempfile::tempdir().unwrap();
  let found = find_teamcity_in(Some(home.path()), Some(empty.path().as_os_str()));
  if cfg!(windows) {
    // Windows has no Homebrew and no ~/.local/bin convention: only PATH counts there
    assert_eq!(found, None);
  } else {
    // Homebrew's, where it is installed, else the one in ~/.local/bin
    let found = found.unwrap();
    assert!(found == in_home || found.starts_with("/opt/homebrew") || found.starts_with("/usr/local"), "{found:?}");
  }
}
