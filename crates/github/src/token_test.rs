use crate::token::{GhSource, Token, TokenChain, TokenSources, read_gh_token, read_git_credential};
use pretty_assertions::assert_eq;
use std::time::Duration;
use sync_types::pull_request::TokenSource;
use test_utils::fake_executable::fake_executable;
use test_utils::git_test_utils::TestRepo;

const TIMEOUT: Duration = Duration::from_secs(10);

fn token(value: &str, source: TokenSource) -> Token {
  Token { value: value.to_string(), source }
}

/// A repository whose only credential helper runs `script` (the user's global and system helpers are reset);
/// `core.askPass` would leave a marker if git ran it
fn repo_with_helper(script: &str) -> (TestRepo, tempfile::TempDir) {
  let repo = TestRepo::new();
  let dir = tempfile::tempdir().unwrap();
  let helper = fake_executable(dir.path(), "helper", script);
  let askpass = fake_executable(dir.path(), "askpass", &format!("touch '{}'", dir.path().join("asked").display()));
  repo.set_config("core.askPass", askpass.to_str().unwrap()).unwrap();
  let path = repo.path().to_str().unwrap();
  repo.git_executor().execute_command(&["config", "--add", "credential.helper", ""], path).unwrap();
  repo
    .git_executor()
    .execute_command(&["config", "--add", "credential.helper", helper.to_str().unwrap()], path)
    .unwrap();
  (repo, dir)
}

#[test_log::test]
fn the_git_credential_of_github_is_its_password() {
  let (repo, dir) = repo_with_helper("cat > \"$(dirname \"$0\")/input\"\necho username=me\necho password=gho_secret\necho password_expiry_utc=1900000000");
  let found = read_git_credential(repo.git_executor(), repo.path().to_str().unwrap(), "acme/monorepo", TIMEOUT);
  assert_eq!(found, Some(token("gho_secret", TokenSource::GitCredential)));
  // the helper gets the host; the path only matters with `credential.useHttpPath`
  let input = std::fs::read_to_string(dir.path().join("input")).unwrap();
  assert!(input.contains("protocol=https\nhost=github.com\n"), "{input}");
}

#[test_log::test]
fn git_asks_its_helper_for_the_repository_and_without_one_asks_nobody() {
  let (repo, dir) = repo_with_helper("echo username=me");
  let found = read_git_credential(repo.git_executor(), repo.path().to_str().unwrap(), "acme/monorepo", TIMEOUT);
  assert_eq!(found, None);
  assert!(!dir.path().join("asked").exists(), "git ran the askpass program");
}

#[test_log::test]
fn gh_gives_its_token_and_nothing_when_logged_out() {
  let dir = tempfile::tempdir().unwrap();
  let args = dir.path().join("args");
  let gh = fake_executable(dir.path(), "gh", &format!("printf '%s\\n' \"$@\" > '{}'\necho gho_from_gh", args.display()));
  assert_eq!(read_gh_token(&gh, TIMEOUT), Some(token("gho_from_gh", TokenSource::Gh)));
  assert_eq!(
    std::fs::read_to_string(&args).unwrap().lines().collect::<Vec<_>>(),
    ["auth", "token", "--hostname", "github.com"]
  );

  let gh = fake_executable(dir.path(), "gh", "echo 'no oauth token found for github.com' >&2\nexit 1");
  assert_eq!(read_gh_token(&gh, TIMEOUT), None);
}

#[tokio::test]
async fn the_chain_takes_the_first_source_and_skips_rejected_tokens() {
  let repo = TestRepo::new();
  let path = repo.path().to_str().unwrap();
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_executable(dir.path(), "gh", "echo from-gh");
  let chain = TokenChain::new(
    TokenSources {
      environment: Some("from-env".to_string()),
      gh: GhSource::At(gh),
      git_credential: false,
    },
    repo.git_executor().clone(),
  );
  let environment = token("from-env", TokenSource::Environment);
  let from_gh = token("from-gh", TokenSource::Gh);
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(environment.clone()));

  // a rejected token is skipped everywhere
  chain.reject(&environment).await;
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(from_gh.clone()));
  assert_eq!(chain.token(path, "acme/other").await, Some(from_gh.clone()));

  // a token that can't see one repository is still tried for another
  chain.deny("acme/monorepo", &from_gh).await;
  assert_eq!(chain.token(path, "acme/monorepo").await, None);
  assert_eq!(chain.token(path, "acme/other").await, Some(from_gh.clone()));

  // the accepted token is kept; forgetting tries again what was denied
  chain.accept("acme/other", &from_gh).await;
  assert_eq!(chain.token(path, "acme/other").await, Some(from_gh.clone()));
  chain.forget().await;
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(from_gh));
}
