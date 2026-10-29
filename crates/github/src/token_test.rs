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
  chain.reject(&environment);
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(from_gh.clone()));
  assert_eq!(chain.token(path, "acme/other").await, Some(from_gh.clone()));

  // a token that can't see one repository is still tried for another
  chain.deny("acme/monorepo", &from_gh);
  assert_eq!(chain.token(path, "acme/monorepo").await, None);
  assert_eq!(chain.token(path, "acme/other").await, Some(from_gh.clone()));

  // the accepted token is kept; forgetting tries again what was denied
  chain.accept("acme/other", &from_gh);
  assert_eq!(chain.token(path, "acme/other").await, Some(from_gh.clone()));
  chain.forget();
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
  chain.reject(&token("from-gh", TokenSource::Gh));
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(token("from-app", TokenSource::BranchDeck)));
  assert_eq!(chain.first_source().await, Some(TokenSource::BranchDeck));
  chain.reject(&token("from-app", TokenSource::BranchDeck));
  assert_eq!(chain.token(path, "acme/monorepo").await, Some(token("from-git", TokenSource::GitCredential)));
  // git has credentials per repository: Settings names no source
  assert_eq!(chain.first_source().await, None);
}

/// A chain whose only source is a fake `gh` in `dir` that runs `body` after it counted its run in `dir/runs`
fn counting_gh_chain(repo: &TestRepo, dir: &std::path::Path, body: &str) -> TokenChain {
  let gh = fake_executable(dir, "gh", &format!("echo run >> '{}'\n{body}", shell_path(&dir.join("runs"))));
  TokenChain::new(
    TokenSources {
      gh: GhSource::At(gh),
      ..TokenSources::default()
    },
    repo.git_executor().clone(),
  )
}

fn runs(dir: &std::path::Path) -> usize {
  std::fs::read_to_string(dir.join("runs")).map_or(0, |runs| runs.lines().count())
}

/// A shell line that waits until `go` exists, 30 s at most: longer than `TIMEOUT`, so a chain that waited for it fails
/// on the timeout
fn wait_for_go(go: &std::path::Path) -> String {
  format!("i=0; while [ ! -e '{}' ] && [ $i -lt 3000 ]; do sleep 0.01; i=$((i+1)); done", shell_path(go))
}

/// A chain whose `gh` counts its run, then runs until the test lets it end (`go` in `dir`) and answers `from-gh`
fn held_gh_chain(repo: &TestRepo, dir: &std::path::Path) -> (TokenChain, std::path::PathBuf) {
  let go = dir.join("go");
  (counting_gh_chain(repo, dir, &format!("{}\necho from-gh", wait_for_go(&go))), go)
}

/// Lets the held `gh` of `dir` end once it runs, after `meanwhile`
async fn release_held_gh(dir: &std::path::Path, go: &std::path::Path, meanwhile: impl FnOnce()) {
  while runs(dir) == 0 {
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  meanwhile();
  std::fs::write(go, "").unwrap();
}

#[tokio::test]
async fn five_first_requests_at_once_run_gh_once() {
  let repo = TestRepo::new();
  let path = repo.path().to_str().unwrap();
  let dir = tempfile::tempdir().unwrap();
  // gh runs until the test lets it end: the asks meet while it runs
  let (chain, go) = held_gh_chain(&repo, dir.path());
  let ask = || chain.token(path, "acme/monorepo");
  let release = release_held_gh(dir.path(), &go, || {});
  let (tokens, ()) = tokio::time::timeout(TIMEOUT, async { tokio::join!(async { tokio::join!(ask(), ask(), ask(), ask(), ask()) }, release) })
    .await
    .expect("gh never ran");
  let from_gh = Some(token("from-gh", TokenSource::Gh));
  assert_eq!([tokens.0, tokens.1, tokens.2, tokens.3, tokens.4], std::array::from_fn(|_| from_gh.clone()));
  assert_eq!(runs(dir.path()), 1);
  // the answer is kept: another repository asks no process either
  assert_eq!(chain.token(path, "acme/other").await, from_gh);
  assert_eq!(runs(dir.path()), 1);
}

/// A sign-in while `gh` runs: the read in flight answers its callers, keeps nothing, and the next request reads again
#[tokio::test]
async fn a_read_in_flight_during_a_sign_in_keeps_nothing_of_its_answer() {
  let repo = TestRepo::new();
  let path = repo.path().to_str().unwrap();
  let dir = tempfile::tempdir().unwrap();
  let (chain, go) = held_gh_chain(&repo, dir.path());
  let sign_in = release_held_gh(dir.path(), &go, || chain.forget());
  let (token_of_the_read, ()) = tokio::time::timeout(TIMEOUT, async { tokio::join!(chain.token(path, "acme/monorepo"), sign_in) })
    .await
    .expect("gh never ran");
  let from_gh = Some(token("from-gh", TokenSource::Gh));
  assert_eq!(token_of_the_read, from_gh);
  assert_eq!(chain.token(path, "acme/monorepo").await, from_gh);
  assert_eq!(runs(dir.path()), 2, "the answer of the read before the sign-in was not kept");
}

#[tokio::test]
async fn a_signed_out_chain_asks_its_sources_once_a_while_and_again_after_a_sign_in() {
  let repo = TestRepo::new();
  let path = repo.path().to_str().unwrap();
  let dir = tempfile::tempdir().unwrap();
  let chain = counting_gh_chain(&repo, dir.path(), "exit 1");
  assert_eq!(chain.token(path, "acme/monorepo").await, None);
  assert_eq!(chain.token(path, "acme/monorepo").await, None);
  assert_eq!(chain.first_source().await, None);
  assert_eq!(runs(dir.path()), 1);
  // the app's sign-in or sign-out forgets what the sources answered
  chain.forget();
  assert_eq!(chain.token(path, "acme/monorepo").await, None);
  assert_eq!(runs(dir.path()), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_accepted_repository_answers_while_the_credential_of_another_is_read() {
  let dir = tempfile::tempdir().unwrap();
  let (started, go) = (dir.path().join("started"), dir.path().join("go"));
  // the credential of the other repository is read until the test lets the helper end
  let (repo, _helper_dir) = repo_with_helper(&format!("touch '{}'\n{}\necho username=me\necho password=from-git", shell_path(&started), wait_for_go(&go)));
  let path = repo.path().to_str().unwrap().to_string();
  let chain = std::sync::Arc::new(TokenChain::new(
    TokenSources {
      git_credential: true,
      ..TokenSources::default()
    },
    repo.git_executor().clone(),
  ));
  let from_git = token("from-git", TokenSource::GitCredential);
  chain.accept("acme/monorepo", &from_git);
  let slow = tokio::spawn({
    let (chain, path) = (std::sync::Arc::clone(&chain), path.clone());
    async move { chain.token(&path, "acme/other").await }
  });
  while !started.exists() {
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
  // while the helper still runs: a chain that waited for it would answer only after the timeout
  let accepted = tokio::time::timeout(TIMEOUT, chain.token(&path, "acme/monorepo")).await;
  assert_eq!(accepted, Ok(Some(from_git.clone())));
  std::fs::write(&go, "").unwrap();
  assert_eq!(slow.await.unwrap(), Some(from_git));
}
