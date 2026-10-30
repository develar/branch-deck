use crate::bd_command::bd_command;
use crate::error::CliError;
use crate::output::ShortIds;
use app_settings::file::{SETTINGS_FILE_NAME, SettingsFile};
use app_settings::resolve::{cli_branch_prefix, resolve_branch_prefix};
use domain_types::error::ErrorCode;
use domain_types::settings::{AppSettings, PrefixSource};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use git_ops::revs::{current_branch, repository_dirs, with_repository_dirs};
use github::client::GITHUB_API_URL;
use github::token::TokenSources;
use history_rewrite::repository_lock::DEFAULT_LOCK_TIMEOUT;
use object_cache::cache::{CacheConfig, ObjectCache, cache_dir};
use operations::registry_file::ProcessRegistry;
use patronus::client::PATRONUS_URL;
use patronus::token::TOKEN_ENV;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::{SpaceRepository, parse_space_repository};
use sync_core::repo_facts::{Baseline, CommandFacts, Remotes, read_baseline_of, read_remotes};
use sync_utils::operation_id::OperationId;
use tokio::runtime::Runtime;
use youtrack::service::EnvironmentToken;
use youtrack::token_store::token_file;

/// The bundle identifier of the app (`tauri.conf.json`): its data directory holds its settings file
/// (`app_settings::file::SETTINGS_FILE_NAME`) and the tokens of its sign-ins
const APP_IDENTIFIER: &str = "org.develar.branch-deck";

/// What the CLI takes from its process, injectable for tests
pub struct Environment {
  /// The directory `-C` defaults to
  pub cwd: PathBuf,
  /// The settings of the app (`app_settings`), read once as `bd` starts (`app_settings_of`): General → Branch prefix
  /// (`globalUserBranchPrefix`) and Settings → Landing (`patronusNewApi`), with the defaults the app has without them
  pub settings: AppSettings,
  /// The git executable from `BD_GIT`
  pub git: Option<String>,
  /// The global git config of every git bd starts (`GIT_CONFIG_GLOBAL`): `None` leaves git its own (`GIT_CONFIG_GLOBAL`
  /// of the process, else `~/.gitconfig`); a test names an empty one or a file of its own, so the developer's
  /// `branchdeck.*` (the branch prefix, the agent rules) never reaches it
  pub git_config_global: Option<PathBuf>,
  /// How long a rewrite waits for the repository lock
  pub lock_timeout: Duration,
  /// How long `bd lease return` waits for the Bazel servers of a slot to exit
  pub server_stop_timeout: Duration,
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
  /// The wait of a read of GitHub or Patronus before it asks again after a transient failure: `None` keeps the app's
  /// (one second); a test that fakes the failure waits none
  pub remote_retry_delay: Option<Duration>,
  /// The `teamcity` CLI of `bd robot --full`; `None` looks it up as the app does (`known_tools::TEAMCITY`), and only then
  pub teamcity: Option<PathBuf>,
  /// Whether a commit of the object cache and a write of the operations registry reach the drive before they return
  /// (`CacheConfig::sync_commits`, `ProcessRegistry::unflushed`): always for the user; a test, whose thousands of
  /// writes in 20 processes at once queue at the drive, writes without
  pub flush_to_drive: bool,
}

impl Environment {
  pub fn from_process() -> Self {
    Self {
      cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
      settings: app_settings_of(app_data_dir().map(|dir| dir.join(SETTINGS_FILE_NAME)).as_deref()),
      git: std::env::var("BD_GIT").ok().filter(|git| !git.is_empty()),
      git_config_global: None,
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      server_stop_timeout: Duration::from_secs(30),
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
      remote_retry_delay: None,
      teamcity: None,
      flush_to_drive: true,
    }
  }
}

/// The data directory of the app (its bundle identifier): its settings and the token of its GitHub sign-in
fn app_data_dir() -> Option<PathBuf> {
  dirs::data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

/// The settings of the app in `settings_file`, read as the app reads them (`SettingsFile::load`): the defaults without
/// the file, or when it does not parse. The app writes the file only by a rename (`SettingsService`), so a read beside
/// its write gets the old or the new settings, never a cut file
#[must_use]
pub fn app_settings_of(settings_file: Option<&Path>) -> AppSettings {
  settings_file.map_or_default(SettingsFile::load).app
}

/// The git of bd (`BD_GIT`, `PATH`, Homebrew's) with the global config of `environment`, counting its processes: `bd -v`
/// logs how many git processes the command started, by subcommand, when the executor is dropped; `git_not_found`
/// without one
pub fn counted_git(environment: &Environment) -> Result<GitCommandExecutor, CliError> {
  let info = GitInfo::discover_for_cli(environment.git.as_deref()).map_err(|e| CliError::new(ErrorCode::GitNotFound, e))?;
  let git = GitCommandExecutor::with_info(info).with_process_stats("bd");
  Ok(match &environment.git_config_global {
    Some(global) => git.with_env("GIT_CONFIG_GLOBAL", &global.to_string_lossy()),
    None => git,
  })
}

/// The global options that select the repository and how it is read
pub struct RepoOptions<'a> {
  pub repo: Option<&'a Path>,
  pub prefix: Option<&'a str>,
  pub full_ids: bool,
  pub no_cache: bool,
}

/// The repository facts of a command that `RepoContext` keeps: `None` until the first that asks reads it
#[derive(Default)]
struct Facts {
  remotes: Option<Remotes>,
  head: Option<HeadBranch>,
  baseline: Option<Baseline>,
}

/// The branch HEAD is on as `symbolic-ref` read it: `None` inside for a detached HEAD
struct HeadBranch(Option<String>);

/// The repository a command runs on
pub struct RepoContext {
  pub git: GitCommandExecutor,
  /// The top level of the working tree
  pub repository: String,
  pub prefix: String,
  /// Where `prefix` came from (`app_settings::resolve`): `cli` for `--prefix`; never `none`, which is an error
  pub prefix_source: PrefixSource,
  pub short_ids: bool,
  /// `git rev-parse --git-common-dir`, shared by all worktrees: the cache lives there
  pub git_common_dir: PathBuf,
  pub use_cache: bool,
  pub lock_timeout: Duration,
  /// This bd in this repository, quoted for the shell (`bd -C <repository>`), for the next commands an output names
  pub bd: String,
  /// Settings → Landing of the app (`patronusNewApi`): the progress of a running robot in one request and the merge
  /// profile from the list, as the app reads them; off without the setting
  pub patronus_new_api: bool,
  /// The operation of the command (`with_operation_id`): the id of its root span and of its JSON, which its operation of
  /// the remote is recorded and journaled under; a new one for a context that no command resolved
  pub operation_id: OperationId,
  /// The object cache, opened by the first that asks: opening reads the database names of every environment
  cache: OnceLock<ObjectCache>,
  /// The config `cache` opens with: the app's, its commits not waiting for the drive in a test
  /// (`Environment::flush_to_drive`, which `operations_registry` reads here too)
  cache_config: CacheConfig,
  /// The blocks and the blobs of the scans of this command (`file_diffs`)
  file_diffs: OnceLock<FileDiffs>,
  /// The git config, read once (`config`)
  config: Mutex<Option<ConfigSnapshot>>,
  /// The repository facts of the command, each read by the first that asks and forgotten after a wait for the
  /// repository lock (`forget_facts`)
  facts: Mutex<Facts>,
  /// The GitHub repository of the branches (`None` when unknown), resolved by the first that asks: a git process for the
  /// Space mirror, and `bd push` asks before its push and again for the retargets after it
  github_repository: OnceLock<Option<GitHubRepository>>,
  /// The runtime of the sync and of the requests to Patronus and GitHub, started by the first that asks
  runtime: OnceLock<Runtime>,
}

impl RepoContext {
  /// Resolve git, the repository top level and the branch prefix: `--prefix`, else the specific one first
  /// (`app_settings::resolve::resolve_branch_prefix`): `branchdeck.branchPrefix` of the repository's local or worktree config, the app setting
  /// `globalUserBranchPrefix`, the global or system `branchdeck.branchPrefix`. An empty prefix is an error
  pub fn resolve(environment: &Environment, options: &RepoOptions<'_>) -> Result<Self, CliError> {
    let git = counted_git(environment)?;

    let directory = match options.repo {
      Some(repo) if repo.is_relative() => environment.cwd.join(repo),
      Some(repo) => repo.to_path_buf(),
      None => environment.cwd.clone(),
    };
    if !directory.is_dir() {
      return Err(CliError::new(ErrorCode::NotARepository, format!("No such directory: {}", directory.display())));
    }
    let directory = directory.to_string_lossy();
    let Some(dirs) = repository_dirs(&git, &directory)? else {
      return Err(CliError::new(ErrorCode::NotARepository, format!("Not inside a git working tree: {directory}")));
    };
    let repository = dirs.top_level.clone();
    let git_common_dir = PathBuf::from(&dirs.git_common_dir);
    // every step of the command takes the directories from here: the guards, the lock, the worktree, a dry run's index
    let git = with_repository_dirs(&git, &repository, dirs);

    // the config the sync reads too: one `git config` for the prefix, the settings and the descriptions
    let mut config = None;
    let prefix = if let Some(prefix) = options.prefix {
      cli_branch_prefix(prefix)
    } else {
      let snapshot = ConfigSnapshot::read(&git, &repository)?;
      let git_prefix = snapshot.branch_prefix();
      config = Some(snapshot);
      resolve_branch_prefix(&git_prefix, environment.settings.global_user_branch_prefix.as_deref())
    };
    if prefix.source == PrefixSource::None {
      return Err(CliError::new(
        ErrorCode::NoBranchPrefix,
        "No branch prefix: pass --prefix, set it in the app (Settings), or run `git config --global branchdeck.branchPrefix <name>` \
         (`git config branchdeck.branchPrefix <name>` for this repository only)",
      ));
    }

    let mut cache_config = CacheConfig::default();
    cache_config.sync_commits = environment.flush_to_drive;
    Ok(Self {
      git,
      prefix: prefix.prefix,
      prefix_source: prefix.source,
      short_ids: !options.full_ids,
      git_common_dir,
      use_cache: !options.no_cache,
      cache_config,
      lock_timeout: environment.lock_timeout,
      patronus_new_api: environment.settings.patronus_new_api,
      operation_id: OperationId::new(),
      bd: bd_command(environment).map_or_else(|_| "bd".to_string(), |command| command.for_shell_in(&repository)),
      repository,
      cache: OnceLock::new(),
      file_diffs: OnceLock::new(),
      config: Mutex::new(config),
      facts: Mutex::new(Facts::default()),
      github_repository: OnceLock::new(),
      runtime: OnceLock::new(),
    })
  }

  /// The context of the command whose operation is `operation_id` (`bd-cli::execute_operation`)
  #[must_use]
  pub const fn with_operation_id(mut self, operation_id: OperationId) -> Self {
    self.operation_id = operation_id;
    self
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

  /// The remotes with their push URLs (`remote -v`): one process for the whole command, the sync's listing, the push
  /// URL of `origin` and the landing route included; read by the first that asks, again after `forget_facts`
  pub fn remotes(&self) -> Result<Remotes, CliError> {
    let mut facts = self.facts.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(remotes) = &facts.remotes {
      return Ok(remotes.clone());
    }
    let remotes = read_remotes(&self.git, &self.repository)?;
    facts.remotes = Some(remotes.clone());
    Ok(remotes)
  }

  /// The branch HEAD is on, `None` for a detached HEAD (`symbolic-ref`): one process for the whole command, the guards
  /// of its sync included; read by the first that asks, again after `forget_facts`
  pub fn head_branch(&self) -> Result<Option<String>, CliError> {
    let mut facts = self.facts.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(HeadBranch(branch)) = &facts.head {
      return Ok(branch.clone());
    }
    let branch = current_branch(&self.git, &self.repository)?;
    facts.head = Some(HeadBranch(branch.clone()));
    Ok(branch)
  }

  /// HEAD's branch and the remotes, as the sync takes them (`SyncOptions::facts`)
  pub fn command_facts(&self) -> Result<CommandFacts, CliError> {
    Ok(CommandFacts {
      branch: self.head_branch()?,
      remotes: self.remotes()?,
    })
  }

  /// The baseline of HEAD's branch and its upstream (`read_baseline_of` the `command_facts`): one `for-each-ref` more
  /// for the whole command; read by the first that asks, again after `forget_facts`
  pub fn baseline(&self) -> Result<Baseline, CliError> {
    if let Some(baseline) = &self.facts.lock().unwrap_or_else(PoisonError::into_inner).baseline {
      return Ok(baseline.clone());
    }
    let baseline = read_baseline_of(&self.git, &self.repository, &self.command_facts()?)?;
    self.facts.lock().unwrap_or_else(PoisonError::into_inner).baseline = Some(baseline.clone());
    Ok(baseline)
  }

  /// The next `remotes`, `head_branch` and `baseline` read them again: another process may have changed them while this
  /// one waited for the repository lock
  pub fn forget_facts(&self) {
    *self.facts.lock().unwrap_or_else(PoisonError::into_inner) = Facts::default();
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

  /// The push URL of `origin`, `None` without that remote, from the `remotes` of the command: the Space repository, the
  /// GitHub repository and the `--repo` of a `gh pr create` are all found by it
  pub fn origin_url(&self) -> Result<Option<String>, CliError> {
    Ok(self.remotes()?.push_url("origin").map(str::to_string))
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
    Ok(self.origin_url()?.as_deref().and_then(parse_space_repository))
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
          ObjectCache::open_dir(&cache_dir(&self.git_common_dir), &self.cache_config)
        } else {
          ObjectCache::disabled()
        }
      })
      .clone()
  }

  /// The operations registry of the repository, its writes not waiting for the drive in a test
  /// (`Environment::flush_to_drive`)
  pub fn operations_registry(&self) -> ProcessRegistry {
    let registry = ProcessRegistry::of(&self.git_common_dir);
    if self.cache_config.sync_commits { registry } else { registry.unflushed() }
  }

  /// `ids` are all the commit ids the output names
  pub fn short_ids<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> ShortIds {
    if self.short_ids { ShortIds::new(ids) } else { ShortIds::full() }
  }
}
