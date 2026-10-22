use crate::token::{GhSource, Token, TokenChain, TokenSources, read_gh_token, read_git_credential};
use domain_types::pull_request::TokenSource;
use pretty_assertions::assert_eq;
use std::time::Duration;
use test_utils::fake_executable::{fake_executable, shell_path};
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
  let askpass = fake_executable(dir.path(), "askpass", &format!("touch '{}'", shell_path(&dir.path().join("asked"))));
  repo.set_config("core.askPass", &shell_path(&askpass)).unwrap();
  let path = repo.path().to_str().unwrap();
  repo.git_executor().execute_command(&["config", "--add", "credential.helper", ""], path).unwrap();
  repo
    .git_executor()
    .execute_command(&["config", "--add", "credential.helper", &shell_path(&helper)], path)
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

/// A chain of the token `from-env` and, from a fake `gh` in `dir`, `from-gh`
pub(crate) fn env_and_gh_chain(repo: &TestRepo, dir: &std::path::Path) -> TokenChain {
  let gh = fake_executable(dir, "gh", "echo from-gh");
  TokenChain::new(
    TokenSources {
      environment: Some("from-env".to_string()),
      gh: GhSource::At(gh),
      git_credential: false,
      ..TokenSources::default()
    },
    repo.git_executor().clone(),
  )
}

#[tokio::test]
async fn the_chain_takes_the_first_source_and_skips_rejected_tokens() {
  let repo = TestRepo::new();
  let path = repo.path().to_str().unwrap();
  let dir = tempfile::tempdir().unwrap();
  let chain = env_and_gh_chain(&repo, dir.path());
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

#[tokio::test]
async fn the_sign_in_of_the_app_comes_after_gh_and_before_git() {
  let (repo, dir) = repo_with_helper("echo username=me\necho password=from-git");
  let path = repo.path().to_str().unwrap();
  let token_file = crate::token_store::token_file(dir.path());
  let stored = crate::token_store::StoredToken {
    token: "from-app".to_string(),
    login: "octocat".to_string(),
    scopes: String::new(),
    created_at: 0,
  };
  crate::token_store::write_token(&token_file, &stored).unwrap();
  let gh = fake_executable(dir.path(), "gh", "echo from-gh");
  let chain = TokenChain::new(
    TokenSources {
      environment: None,
      gh: GhSource::At(gh),
      token_file: Some(token_file),
      git_credential: true,
    },
    repo.git_executor().clone(),
  );
  assert_eq!(chain.first_source().await, Some(TokenSource::Gh));
  chain.reject(&token("from-gh", TokenSource::Gh)).await;
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(token("from-app", TokenSource::BranchDeck)));
  assert_eq!(chain.first_source().await, Some(TokenSource::BranchDeck));
  chain.reject(&token("from-app", TokenSource::BranchDeck)).await;
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(token("from-git", TokenSource::GitCredential)));
  // git has credentials per repository: Settings names no source
  assert_eq!(chain.first_source().await, None);
}
