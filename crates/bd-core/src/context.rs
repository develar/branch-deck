use crate::bd_command::bd_command;
use crate::error::{CliError, ErrorCode};
use crate::output::ShortIds;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use git_ops::revs::{RepositoryDirs, repository_dirs};
use github::client::GITHUB_API_URL;
use github::token::TokenSources;
use history_rewrite::repository_lock::DEFAULT_LOCK_TIMEOUT;
use object_cache::cache::ObjectCache;
use patronus::client::PATRONUS_URL;
use patronus::token::TOKEN_ENV;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;
use sync_core::branch_prefix::resolve_branch_prefix;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::patronus::{SpaceRepository, parse_space_repository};
use sync_core::pull_requests::GitHubRepository;
use sync_core::remote_url::origin_push_url;
use tokio::runtime::Runtime;
use tracing::debug;
use youtrack::service::EnvironmentToken;
use youtrack::token_store::token_file;

/// The tauri-plugin-store file of the app (`LazyStore("settings.json")` in `src/stores/settings/app-store.ts`), in the
/// app data directory of the bundle identifier (`tauri.conf.json`)
const APP_IDENTIFIER: &str = "org.develar.branch-deck";
const SETTINGS_FILE_NAME: &str = "settings.json";

/// What the CLI takes from its process, injectable for tests
pub struct Environment {
  /// The directory `-C` defaults to
  pub cwd: PathBuf,
  /// The app settings file; `None` skips the app setting
  pub settings_file: Option<PathBuf>,
  /// The git executable from `BD_GIT`
  pub git: Option<String>,
  /// How long a rewrite waits for the repository lock
  pub lock_timeout: Duration,
  /// How long `bd lease return` waits for the Bazel servers of a slot to exit
  pub server_stop_timeout: Duration,
  /// How long `bd lease return` waits for git's background maintenance to leave a slot
  pub maintenance_timeout: Duration,
  /// The home directory `bd install` writes into
  pub home: Option<PathBuf>,
  /// The running binary, which `bd install` links to
  pub executable: Option<PathBuf>,
  /// The `.AppImage` file the binary runs from (`APPIMAGE`, Linux), which `bd install` links to instead
  pub appimage: Option<PathBuf>,
  /// `PATH`, to tell whether the installed `bd` is found
  pub path: Option<OsString>,
  /// The Patronus instance `bd safe-push` starts robots on: `SAFEPUSH_PATRONUS_URL` (as for the Safe Push CLI), else
  /// the production one
  pub patronus_url: String,
  /// `SPACE_TOKEN`, the Space token when the Safe Push CLI has not logged in (`home` has its token file)
  pub space_token: Option<String>,
  /// The GitHub API the pull requests are read and written through: github.com, a fake one in the tests
  pub github_api_url: String,
  /// Where the GitHub token comes from: `GH_TOKEN`/`GITHUB_TOKEN`, `gh auth token`, the app's sign-in, the git
  /// credentials of github.com
  pub github_tokens: TokenSources,
  /// The YouTrack a new pull request's text takes the summaries of its issues from: `YOUTRACK_BASE_URL` and
  /// `YOUTRACK_TOKEN`, else the token of the app's sign-in (`youtrack_token_file`); only read, never renewed
  pub youtrack_environment: Option<EnvironmentToken>,
  pub youtrack_token_file: Option<PathBuf>,
  /// The `teamcity` CLI of `bd robot --full`; `None` looks it up as the app does (`find_teamcity`), and only then
  pub teamcity: Option<PathBuf>,
}

impl Environment {
  pub fn from_process() -> Self {
    Self {
      cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
      settings_file: app_data_dir().map(|dir| dir.join(SETTINGS_FILE_NAME)),
      git: std::env::var("BD_GIT").ok().filter(|git| !git.is_empty()),
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      server_stop_timeout: Duration::from_secs(30),
      maintenance_timeout: Duration::from_mins(1),
      home: std::env::home_dir(),
      executable: std::env::current_exe().ok(),
      appimage: std::env::var_os("APPIMAGE").filter(|path| !path.is_empty()).map(PathBuf::from),
      path: std::env::var_os("PATH"),
      patronus_url: std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string()),
      space_token: std::env::var(TOKEN_ENV).ok(),
      github_api_url: GITHUB_API_URL.to_string(),
      github_tokens: TokenSources::from_process(app_data_dir().as_deref()),
      youtrack_environment: EnvironmentToken::from_process(),
      youtrack_token_file: app_data_dir().as_deref().map(token_file),
      teamcity: None,
    }
  }
}

/// The data directory of the app (its bundle identifier): its settings and the token of its GitHub sign-in
fn app_data_dir() -> Option<PathBuf> {
  dirs::data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

/// The global options that select the repository and how it is read
pub struct RepoOptions<'a> {
  pub repo: Option<&'a Path>,
  pub prefix: Option<&'a str>,
  pub full_ids: bool,
  pub no_cache: bool,
}

/// The repository a command runs on
pub struct RepoContext {
  pub git: GitCommandExecutor,
  /// The top level of the working tree
  pub repository: String,
  pub prefix: String,
  pub short_ids: bool,
  /// `git rev-parse --git-common-dir`, shared by all worktrees: the cache lives there
  pub git_common_dir: PathBuf,
  pub use_cache: bool,
  pub lock_timeout: Duration,
  /// This bd in this repository, quoted for the shell (`bd -C <repository>`), for the next commands an output names
  pub bd: String,
  /// The object cache, opened by the first that asks: opening reads the database names of every environment
  cache: OnceLock<ObjectCache>,
  /// The blocks and the blobs of the scans of this command (`file_diffs`)
  file_diffs: OnceLock<FileDiffs>,
  /// The git config, read once (`config`)
  config: Mutex<Option<ConfigSnapshot>>,
  /// The push URL of `origin` (`None` without it), read by the first that asks: the Space repository, the GitHub
  /// repository and the `--repo` of a `gh pr create` are all found by it
  origin_url: OnceLock<Option<String>>,
  /// The GitHub repository of the branches (`None` when unknown), resolved by the first that asks: a git process for the
  /// Space mirror, and `bd push` asks before its push and again for the retargets after it
  github_repository: OnceLock<Option<GitHubRepository>>,
  /// The runtime of the sync and of the requests to Patronus and GitHub, started by the first that asks
  runtime: OnceLock<Runtime>,
}

impl RepoContext {
  /// Resolve git, the repository top level and the branch prefix: `--prefix`, else the specific one first
  /// (`resolve_branch_prefix`): `branchdeck.branchPrefix` of the repository's local or worktree config, the app setting
  /// `globalUserBranchPrefix`, the global or system `branchdeck.branchPrefix`. An empty prefix is an error
  pub fn resolve(environment: &Environment, options: &RepoOptions<'_>) -> Result<Self, CliError> {
    let info = GitInfo::discover_for_cli(environment.git.as_deref()).map_err(|e| CliError::new(ErrorCode::GitNotFound, e))?;
    // `bd -v` logs how many git processes the command started, by subcommand, when the context is dropped
    let git = GitCommandExecutor::with_info(info).with_process_stats("bd");

    let directory = match options.repo {
      Some(repo) if repo.is_relative() => environment.cwd.join(repo),
      Some(repo) => repo.to_path_buf(),
      None => environment.cwd.clone(),
    };
    if !directory.is_dir() {
      return Err(CliError::new(ErrorCode::NotARepository, format!("No such directory: {}", directory.display())));
    }
    let directory = directory.to_string_lossy();
    let Some(RepositoryDirs {
      top_level: repository,
      git_common_dir,
      ..
    }) = repository_dirs(&git, &directory)?
    else {
      return Err(CliError::new(ErrorCode::NotARepository, format!("Not inside a git working tree: {directory}")));
    };
    let git_common_dir = PathBuf::from(git_common_dir);

    // the config the sync reads too: one `git config` for the prefix, the settings and the descriptions
    let mut config = None;
    let prefix = if let Some(prefix) = options.prefix {
      prefix.trim().to_string()
    } else {
      let snapshot = ConfigSnapshot::read(&git, &repository)?;
      let git_prefix = snapshot.branch_prefix();
      config = Some(snapshot);
      resolve_branch_prefix(&git_prefix, environment.settings_file.as_deref().and_then(read_app_branch_prefix).as_deref())
    };
    if prefix.is_empty() {
      return Err(CliError::new(
        ErrorCode::NoBranchPrefix,
        "No branch prefix: pass --prefix, set it in the app (Settings), or run `git config --global branchdeck.branchPrefix <name>` \
         (`git config branchdeck.branchPrefix <name>` for this repository only)",
      ));
    }

    Ok(Self {
      git,
      prefix,
      short_ids: !options.full_ids,
      git_common_dir,
      use_cache: !options.no_cache,
      lock_timeout: environment.lock_timeout,
      bd: bd_command(environment).map_or_else(|_| "bd".to_string(), |command| command.for_shell_in(&repository)),
      repository,
      cache: OnceLock::new(),
      file_diffs: OnceLock::new(),
      config: Mutex::new(config),
      origin_url: OnceLock::new(),
      github_repository: OnceLock::new(),
      runtime: OnceLock::new(),
    })
  }

  /// The git config of the repository (`ConfigSnapshot`): the branch prefix `resolve` read, the settings and the
  /// descriptions of the sync, one `git config` for the whole command; read by the first that asks, and again after
  /// a wait for the repository lock (`forget_config`), when another process may have changed it
  pub fn config(&self) -> Result<ConfigSnapshot, CliError> {
    let mut config = self.config.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(config) = config.as_ref() {
      return Ok(config.clone());
    }
    let read = ConfigSnapshot::read(&self.git, &self.repository)?;
    *config = Some(read.clone());
    Ok(read)
  }

  /// The next `config` reads the config again
  pub fn forget_config(&self) {
    *self.config.lock().unwrap_or_else(PoisonError::into_inner) = None;
  }

  /// The one runtime of the command: the sync runs on it, and the requests to Patronus and GitHub
  pub fn runtime(&self) -> Result<&Runtime, CliError> {
    if let Some(runtime) = self.runtime.get() {
      return Ok(runtime);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .map_err(|e| CliError::new(ErrorCode::GitError, format!("Cannot start the runtime: {e}")))?;
    Ok(self.runtime.get_or_init(|| runtime))
  }

  /// The push URL of `origin`, `None` without that remote: one `remote get-url --push` for the whole command. A failed
  /// read is not remembered
  pub fn origin_url(&self) -> Result<Option<&str>, CliError> {
    if let Some(url) = self.origin_url.get() {
      return Ok(url.as_deref());
    }
    let url = origin_push_url(&self.git, &self.repository)?;
    Ok(self.origin_url.get_or_init(|| url).as_deref())
  }

  /// The GitHub repository of the branches: `resolve` finds it the first time, every later ask has its answer. A failed
  /// resolve is not remembered
  pub fn github_repository(&self, resolve: impl FnOnce() -> Result<Option<GitHubRepository>, CliError>) -> Result<Option<GitHubRepository>, CliError> {
    if let Some(repository) = self.github_repository.get() {
      return Ok(repository.clone());
    }
    let repository = resolve()?;
    Ok(self.github_repository.get_or_init(|| repository).clone())
  }

  /// The Space repository `origin` pushes to; `None` when it is not on Space
  pub fn space_repository(&self) -> Result<Option<SpaceRepository>, CliError> {
    Ok(self.origin_url()?.and_then(parse_space_repository))
  }

  /// The diffs of the scans of this command (`FileDiffs::for_scans`): the changes of the commits and their patches read
  /// each blob once
  pub fn file_diffs(&self) -> &FileDiffs {
    self.file_diffs.get_or_init(FileDiffs::for_scans)
  }

  /// The object cache of the repository, or a disabled one with --no-cache; a handle is cheap to clone
  pub fn cache(&self) -> ObjectCache {
    self
      .cache
      .get_or_init(|| {
        if self.use_cache {
          ObjectCache::open(&self.git_common_dir)
        } else {
          ObjectCache::disabled()
        }
      })
      .clone()
  }

  /// `ids` are all the commit ids the output names
  pub fn short_ids<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> ShortIds {
    if self.short_ids { ShortIds::new(ids) } else { ShortIds::full() }
  }
}

/// `appSettings.globalUserBranchPrefix` of the app settings file; `None` if the file, the key or a non-blank value is
/// missing
fn read_app_branch_prefix(settings_file: &Path) -> Option<String> {
  let text = std::fs::read_to_string(settings_file).ok()?;
  let settings: serde_json::Value = match serde_json::from_str(&text) {
    Ok(settings) => settings,
    Err(e) => {
      debug!(error = %e, file = %settings_file.display(), "ignoring unreadable app settings");
      return None;
    }
  };
  let prefix = settings.get("appSettings")?.get("globalUserBranchPrefix")?.as_str()?.trim();
  (!prefix.is_empty()).then(|| prefix.to_string())
}
