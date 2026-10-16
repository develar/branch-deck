//! The GitHub token Branch Deck asks with, the first of: `GH_TOKEN` (else `GITHUB_TOKEN`), `gh auth token`, the token
//! of the app's own sign-in (`token_store.rs`), and the credential git stores for github.com (`git credential fill`).
//! `gh` comes before the app's sign-in: organizations that restrict OAuth apps have usually approved it, and not yet
//! the Branch Deck app. The token GitHub accepted is kept per repository; a token it
//! rejected (401) is never sent again, and one that can't see a repository is skipped for that repository for a while,
//! so a `gh auth login` takes effect without a restart.

use crate::token_store::{read_token, token_file};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use sync_types::pull_request::TokenSource;
use sync_utils::child_process::{find_executable, run_child};
use tracing::{debug, info, instrument, warn};

/// `gh auth token` and a credential helper answer at once, unless the helper waits for the user (an unlock dialog)
pub const SOURCE_TIMEOUT: Duration = Duration::from_secs(10);
/// A token that could not see a repository is tried for it again after this long: access may have been granted
const NO_ACCESS_RETRY: Duration = Duration::from_secs(600);

/// A token and where it came from
#[derive(Clone, PartialEq, Eq)]
pub struct Token {
  pub value: String,
  pub source: TokenSource,
}

impl std::fmt::Debug for Token {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Token").field("source", &self.source).finish_non_exhaustive()
  }
}

impl Token {
  fn key(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    self.value.hash(&mut hasher);
    hasher.finish()
  }
}

/// Which sources the chain asks: all of them for the app and `bd`, only a given token in the tests
#[derive(Clone, Debug, Default)]
pub struct TokenSources {
  /// `GH_TOKEN`, else `GITHUB_TOKEN`, as the process got them
  pub environment: Option<String>,
  /// The `gh` to ask `gh auth token`
  pub gh: GhSource,
  /// The token file of the app's sign-in (`token_store::token_file`)
  pub token_file: Option<PathBuf>,
  /// Ask `git credential fill`
  pub git_credential: bool,
}

/// The order the sources are asked in
const SOURCE_ORDER: [TokenSource; 4] = [TokenSource::Environment, TokenSource::Gh, TokenSource::BranchDeck, TokenSource::GitCredential];

impl TokenSources {
  /// The sources of the app and `bd`: the process environment, the installed `gh`, the token file in
  /// `app_data_dir`, git
  pub fn from_process(app_data_dir: Option<&Path>) -> Self {
    let variable = |name: &str| std::env::var(name).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty());
    Self {
      environment: variable("GH_TOKEN").or_else(|| variable("GITHUB_TOKEN")),
      gh: GhSource::Find,
      token_file: app_data_dir.map(token_file),
      git_credential: true,
    }
  }

  /// Only `token`, as if it came from the environment
  pub fn fixed(token: &str) -> Self {
    Self {
      environment: Some(token.to_string()),
      ..Self::default()
    }
  }
}

/// Which `gh` the chain asks
#[derive(Clone, Debug, Default)]
pub enum GhSource {
  #[default]
  None,
  /// The installed one (`find_gh`), looked up each time: it may be installed while the app runs
  Find,
  /// This one, in the tests
  At(PathBuf),
}

#[derive(Default)]
struct ChainState {
  /// By repository (`owner/name`): the token GitHub last accepted for it
  accepted: HashMap<String, Token>,
  /// Tokens GitHub rejected: never sent again
  rejected: HashSet<u64>,
  /// By repository and token: since when the token can't see the repository
  no_access: HashMap<(String, u64), Instant>,
}

pub struct TokenChain {
  sources: TokenSources,
  git: GitCommandExecutor,
  state: tokio::sync::Mutex<ChainState>,
}

impl TokenChain {
  pub fn new(sources: TokenSources, git: GitCommandExecutor) -> Self {
    Self {
      sources,
      git,
      state: tokio::sync::Mutex::new(ChainState::default()),
    }
  }

  /// The token to ask about `repository` (`owner/name`) with: the one GitHub last accepted, else the first source's
  /// that is not rejected for it. `repository_path` is where git reads the credential config.
  #[instrument(skip(self))]
  pub async fn token(&self, repository_path: &str, repository: &str) -> Option<Token> {
    // held while the sources run: two refreshes at once don't start two `gh` processes
    let state = self.state.lock().await;
    if let Some(token) = state.accepted.get(repository) {
      return Some(token.clone());
    }
    let usable = |token: &Token| {
      let key = token.key();
      !state.rejected.contains(&key) && state.no_access.get(&(repository.to_string(), key)).is_none_or(|since| since.elapsed() >= NO_ACCESS_RETRY)
    };
    for source in SOURCE_ORDER {
      if let Some(token) = self.read_source(source, repository_path, repository).await
        && usable(&token)
      {
        return Some(token);
      }
    }
    None
  }

  /// Where the token comes from that the chain would take first, without git (its credentials are per repository):
  /// for Settings
  pub async fn first_source(&self) -> Option<TokenSource> {
    let state = self.state.lock().await;
    for source in [TokenSource::Environment, TokenSource::Gh, TokenSource::BranchDeck] {
      if let Some(token) = self.read_source(source, "", "").await
        && !state.rejected.contains(&token.key())
      {
        return Some(source);
      }
    }
    None
  }

  /// The token of `source`; a process source runs off the async workers
  async fn read_source(&self, source: TokenSource, repository_path: &str, repository: &str) -> Option<Token> {
    match source {
      TokenSource::Environment => self.sources.environment.clone().map(|value| Token { value, source }),
      TokenSource::Gh => {
        let gh = match &self.sources.gh {
          GhSource::None => return None,
          GhSource::Find => None,
          GhSource::At(path) => Some(path.clone()),
        };
        run_source(move || read_gh_token(gh.or_else(find_gh)?.as_path(), SOURCE_TIMEOUT)).await
      }
      TokenSource::BranchDeck => {
        let path = self.sources.token_file.clone()?;
        run_source(move || read_token(&path).map(|stored| Token { value: stored.token, source })).await
      }
      TokenSource::GitCredential => {
        if !self.sources.git_credential || repository.is_empty() {
          return None;
        }
        let git = self.git.clone();
        let path = repository_path.to_string();
        let repository = repository.to_string();
        run_source(move || read_git_credential(&git, &path, &repository, SOURCE_TIMEOUT)).await
      }
    }
  }

  /// GitHub answered `repository` with `token`
  pub async fn accept(&self, repository: &str, token: &Token) {
    let mut state = self.state.lock().await;
    if state.accepted.get(repository) != Some(token) {
      info!(repository, source = ?token.source, "GitHub accepted the token");
      state.accepted.insert(repository.to_string(), token.clone());
    }
  }

  /// GitHub rejected `token` (401): it is not sent again for any repository
  pub async fn reject(&self, token: &Token) {
    let mut state = self.state.lock().await;
    warn!(source = ?token.source, "GitHub rejected the token");
    let key = token.key();
    state.rejected.insert(key);
    state.accepted.retain(|_, accepted| accepted.key() != key);
  }

  /// `token` can't see `repository`: the next source is tried for it
  pub async fn deny(&self, repository: &str, token: &Token) {
    let mut state = self.state.lock().await;
    warn!(repository, source = ?token.source, "the token can't see the repository");
    state.no_access.insert((repository.to_string(), token.key()), Instant::now());
    if state.accepted.get(repository) == Some(token) {
      state.accepted.remove(repository);
    }
  }

  /// Forgets what GitHub accepted and denied: a new token was stored or removed
  pub async fn forget(&self) {
    let mut state = self.state.lock().await;
    state.accepted.clear();
    state.no_access.clear();
  }
}

/// A source that starts a process, off the async workers
async fn run_source(read: impl FnOnce() -> Option<Token> + Send + 'static) -> Option<Token> {
  match tokio::task::spawn_blocking(read).await {
    Ok(token) => token,
    Err(e) => {
      warn!(error = %e, "the token source panicked");
      None
    }
  }
}

/// The `gh` binary: Homebrew first, as for git (an app started from the Finder has no Homebrew in `PATH`), then `PATH`
pub fn find_gh() -> Option<PathBuf> {
  #[cfg(target_os = "macos")]
  for path in ["/opt/homebrew/bin/gh", "/usr/local/bin/gh"] {
    if Path::new(path).is_file() {
      return Some(PathBuf::from(path));
    }
  }

  find_executable("gh", &std::env::var_os("PATH")?)
}

/// `gh auth token` of github.com; None when gh is not logged in (it exits with 1) or fails
pub fn read_gh_token(gh: &Path, timeout: Duration) -> Option<Token> {
  let mut command = Command::new(gh);
  command
    .args(["auth", "token", "--hostname", "github.com"])
    .env("GH_PROMPT_DISABLED", "1")
    .env("GH_NO_UPDATE_NOTIFIER", "1")
    .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
    .env("NO_COLOR", "1");
  let output = match run_child(&mut command, None, timeout, &|| false) {
    Ok(output) => output,
    Err(e) => {
      debug!(error = ?e, "cannot run gh auth token");
      return None;
    }
  };
  if !output.status.success() {
    debug!(stderr = %output.stderr, "gh has no token");
    return None;
  }
  let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
  (!value.is_empty()).then_some(Token { value, source: TokenSource::Gh })
}

/// Config of `git credential fill`, as for a fetch (`sync_core::remote_command`): stored credentials only, nothing
/// asks for new ones
const CREDENTIAL_ARGS: [&str; 4] = ["-c", "credential.interactive=false", "credential", "fill"];

/// - `GIT_ASKPASS=` (empty): git would run an askpass program before it looks at `GIT_TERMINAL_PROMPT`
/// - `GCM_INTERACTIVE=never`, `GCM_GUI_PROMPT=false`: the Git Credential Manager shows no sign-in window
const CREDENTIAL_ENV: [(&str, &str); 3] = [("GIT_ASKPASS", ""), ("GCM_INTERACTIVE", "never"), ("GCM_GUI_PROMPT", "false")];

/// The password git stores for `https://github.com/<repository>.git` (a token: GitHub has no passwords over HTTPS);
/// None without one. Exit code 128 is the normal answer without a stored credential.
pub fn read_git_credential(git: &GitCommandExecutor, repository_path: &str, repository: &str, timeout: Duration) -> Option<Token> {
  let input = format!("protocol=https\nhost=github.com\npath={repository}.git\n\n");
  let (stdout, exit_code) = match git.execute_command_with_input_until(&CREDENTIAL_ARGS, repository_path, &CREDENTIAL_ENV, input.as_bytes(), timeout) {
    Ok(answer) => answer,
    Err(e) => {
      warn!(error = %e, "cannot ask git for the GitHub credentials");
      return None;
    }
  };
  if exit_code != 0 {
    debug!(exit_code, "git has no credentials for github.com");
    return None;
  }
  let value = String::from_utf8_lossy(&stdout)
    .lines()
    .find_map(|line| line.strip_prefix("password="))
    .map(str::trim)
    .filter(|password| !password.is_empty())?
    .to_string();
  Some(Token {
    value,
    source: TokenSource::GitCredential,
  })
}
