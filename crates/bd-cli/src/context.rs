use crate::error::{CliError, ErrorCode};
use crate::install::bd_command;
use crate::output::ShortIds;
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use github::client::GITHUB_API_URL;
use github::token::TokenSources;
use object_cache::cache::ObjectCache;
use patronus::client::PATRONUS_URL;
use patronus::token::TOKEN_ENV;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_core::branch_prefix::get_branch_prefix_from_git_config_sync;
use sync_core::repository_lock::DEFAULT_LOCK_TIMEOUT;
use tracing::debug;

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
}

impl Environment {
  pub fn from_process() -> Self {
    Self {
      cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
      settings_file: app_data_dir().map(|dir| dir.join(SETTINGS_FILE_NAME)),
      git: std::env::var("BD_GIT").ok().filter(|git| !git.is_empty()),
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      home: std::env::home_dir(),
      executable: std::env::current_exe().ok(),
      appimage: std::env::var_os("APPIMAGE").filter(|path| !path.is_empty()).map(PathBuf::from),
      path: std::env::var_os("PATH"),
      patronus_url: std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string()),
      space_token: std::env::var(TOKEN_ENV).ok(),
      github_api_url: GITHUB_API_URL.to_string(),
      github_tokens: TokenSources::from_process(app_data_dir().as_deref()),
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
}

impl RepoContext {
  /// Resolve git, the repository top level and the branch prefix: `--prefix`, else the app setting
  /// `globalUserBranchPrefix`, else `branchdeck.branchPrefix`. An empty prefix is an error
  pub fn resolve(environment: &Environment, options: &RepoOptions<'_>) -> Result<Self, CliError> {
    let info = GitInfo::discover_for_cli(environment.git.as_deref()).map_err(|e| CliError::new(ErrorCode::GitNotFound, e))?;
    let git = GitCommandExecutor::with_info(info);

    let directory = match options.repo {
      Some(repo) if repo.is_relative() => environment.cwd.join(repo),
      Some(repo) => repo.to_path_buf(),
      None => environment.cwd.clone(),
    };
    if !directory.is_dir() {
      return Err(CliError::new(ErrorCode::NotARepository, format!("No such directory: {}", directory.display())));
    }
    let directory = directory.to_string_lossy();
    let (output, exit_code) = git.execute_command_with_status(&["rev-parse", "--show-toplevel", "--path-format=absolute", "--git-common-dir"], &directory)?;
    let mut lines = output.lines().map(str::trim);
    let (0, Some(repository), Some(git_common_dir)) = (exit_code, lines.next().filter(|line| !line.is_empty()), lines.next()) else {
      return Err(CliError::new(ErrorCode::NotARepository, format!("Not inside a git working tree: {directory}")));
    };
    let (repository, git_common_dir) = (repository.to_string(), PathBuf::from(git_common_dir));

    let prefix = match options.prefix {
      Some(prefix) => prefix.trim().to_string(),
      None => match environment.settings_file.as_deref().and_then(read_app_branch_prefix) {
        Some(prefix) => prefix,
        None => get_branch_prefix_from_git_config_sync(&git, &repository)?,
      },
    };
    if prefix.is_empty() {
      return Err(CliError::new(
        ErrorCode::NoBranchPrefix,
        "No branch prefix: pass --prefix, set it in the app, or run `git config --global branchdeck.branchPrefix <name>`",
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
    })
  }

  /// The object cache of the repository, or a disabled one with --no-cache
  pub fn cache(&self) -> ObjectCache {
    if self.use_cache {
      ObjectCache::open(&self.git_common_dir)
    } else {
      ObjectCache::disabled()
    }
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
