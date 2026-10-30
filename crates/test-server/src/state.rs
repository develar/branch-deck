use dashmap::DashMap;
use domain_types::branch_name_suggestion::BranchNameSuggestionOutcome;
use domain_types::error::ApiError;
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::ReviewRequestsResult;
use domain_types::settings::GitBranchPrefix;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::AtomicU32;

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
  /// The templates the repositories are copied from, and the builds of the missing ones
  pub templates: std::sync::Arc<crate::templates::Templates>,
  /// The git prefix of each repository, which the commands of the sync family resolve the prefix with, as the app keeps
  /// it (`resolve_branch_prefix`)
  pub branch_prefixes: app_settings::branch_prefixes::BranchPrefixes,
  /// The sessions of the repositories (`session_routes`), as the app hosts them; their facts are read by the scheduler
  /// of the facts over the fakes (`TestFactSources`)
  pub sessions: std::sync::Arc<repository_model::session::Sessions>,
  /// The operations of the remote that run or wait in the sessions, as the app keeps them (`operations::registry`)
  pub registry: operations::registry::Registry,
  /// The clocks the sessions age on, one per test repository: each stands still, so no session of a test goes away by
  /// itself, and moves only by its test
  pub session_clocks: std::sync::Arc<WorktreeClocks>,
  /// The git directories and the GitHub repository of each test repository, as the app keeps them, without the object
  /// cache (docs/design/e2e-testing.md, decision 8); forgotten with the repository
  pub repository_state: app_api::repository_state::RepositoryStateCache,
}

impl AppState {
  /// The global git config of a test repository, beside its directory: `GIT_CONFIG_GLOBAL` of its config reads and
  /// writes (`set_repository_config` with `scope: global`), so no test reads or writes the config of the developer,
  /// and two tests that run at once never share one. Deleted with the repository
  pub fn global_config_of(&self, repository_id: &str) -> PathBuf {
    self.test_root_dir.path().join(format!("{repository_id}.gitconfig"))
  }

  /// The git of a test machine for the test repository `repository_id`: its global config (`global_config_of`) and no
  /// system config. Its commands (`context_of`) and the syncs of its sessions (`TestSessionEnvironment::git`) run it
  pub fn git_of(&self, repository_id: &str) -> GitCommandExecutor {
    let global_config = self.global_config_of(repository_id);
    self
      .git_executor
      .with_env("GIT_CONFIG_GLOBAL", &global_config.to_string_lossy())
      .with_env("GIT_CONFIG_NOSYSTEM", "1")
  }

  /// The app of the test repository `repository_id`, what its commands of app-api run with: the git of a test machine
  /// (the global config of the repository, `global_config_of`, and no system config), the settings of its page, its own
  /// memory of the diffs and of the reviews of branches, its own windows and its own search of the conflict fix, the
  /// fixture of the repository as its symbol index (`FixtureIndexes`), the fakes of the repository as its services
  /// (`TestServices`) and its host (`TestHost`), the event streams of its pages (`TestEvents`), its desktop
  /// (`TestDesktop`, `TestUpdater`, `TestAgentCli`), and the git prefixes, the repository state, the sessions and the
  /// registry of the server
  pub fn context_of(self: &std::sync::Arc<Self>, repository_id: &str, hosts: &TestHosts) -> app_api::context::AppContext {
    let services = crate::session_routes::test_services(self);
    app_api::context::AppContext {
      git: self.git_of(repository_id),
      settings: std::sync::Arc::clone(&hosts.settings),
      prefixes: self.branch_prefixes.clone(),
      repository_state: self.repository_state.clone(),
      file_diffs: file_diffs::service::FileDiffs::default(),
      symbol_indexes: std::sync::Arc::new(FixtureIndexes),
      conflict_fix: app_api::reads::conflict_fix::ConflictFixSearch::default(),
      review_plans: app_api::reads::review_plan::ReviewPlans::default(),
      sessions: std::sync::Arc::clone(&self.sessions),
      registry: self.registry.clone(),
      windows: std::sync::Arc::default(),
      services: std::sync::Arc::clone(&services) as std::sync::Arc<dyn remote_facts::service_sources::ServiceHost>,
      rewrites: services,
      events: std::sync::Arc::clone(&hosts.events) as std::sync::Arc<dyn app_api::transport::AppEvents>,
      host: std::sync::Arc::new(crate::host::TestHost {
        state: std::sync::Arc::downgrade(self),
        repository: repository_id.to_owned(),
        cli_tools: std::sync::Arc::clone(&hosts.cli_tools),
      }),
      remote: app_api::remote::RemoteMemory::default(),
      desktop: app_api::transport::Desktop {
        windows: std::sync::Arc::clone(&hosts.desktop) as std::sync::Arc<dyn app_api::transport::WindowHost>,
        dialog: std::sync::Arc::clone(&hosts.desktop) as std::sync::Arc<dyn app_api::transport::DialogHost>,
        menu: std::sync::Arc::clone(&hosts.desktop) as std::sync::Arc<dyn app_api::transport::MenuHost>,
        restart: std::sync::Arc::clone(&hosts.desktop) as std::sync::Arc<dyn app_api::transport::RestartHost>,
        updater: std::sync::Arc::clone(&hosts.updater) as std::sync::Arc<dyn app_api::transport::Updater>,
        agent_cli: std::sync::Arc::new(crate::host::TestAgentCli::default()),
      },
      sub_windows: std::sync::Arc::default(),
      cli_tools: app_api::app::cli_tools::CliToolsState::default(),
      air_watches: std::sync::Arc::default(),
      runtime: tokio::runtime::Handle::current(),
    }
  }

  /// The app data directory of a test repository's fake services (`remote_host.rs`): the token files of the sign-ins
  /// and the fake `gh`, beside the repository as its global git config is, so a failed sign-in test can be read there.
  /// Deleted with the repository
  pub fn app_data_of(&self, repository_id: &str) -> PathBuf {
    self.test_root_dir.path().join(format!("{repository_id}.app-data"))
  }

  /// The test repository at `path`, the one a command of its session names; the app's error for a path that no test
  /// repository has
  pub fn repository(&self, path: &str) -> Result<dashmap::mapref::one::Ref<'_, String, TestRepository>, ApiError> {
    self
      .path_to_id
      .get(path)
      .and_then(|id| self.repositories.get(id.value()))
      .ok_or_else(|| ApiError::internal(format!("Repository not accessible: {path}")))
  }

  /// The prefix of the test repository at `path` by its local config read now, with the settings of its page, kept for
  /// the commands after it (`BranchPrefixes::refresh`): what the run of its session checks its key against, as the app's
  /// run resolves it from the config it reads; blocking
  pub fn branch_prefix_now(&self, path: &str) -> anyhow::Result<String> {
    let settings = self.repository(path).map_err(|unknown| anyhow::anyhow!(unknown.message))?.store.app();
    let local = read_local_branch_prefix(&self.git_executor, path)?;
    Ok(self.branch_prefixes.refresh(&settings, path, local).prefix)
  }
}

/// `branchdeck.branchPrefix` of the repository's local config (the git read of `resolve_branch_prefix`)
pub fn read_local_branch_prefix(git_executor: &GitCommandExecutor, repo_path: &str) -> anyhow::Result<GitBranchPrefix> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["config", "--local", "branchdeck.branchPrefix"], repo_path)?;
  match exit_code {
    0 => Ok(GitBranchPrefix {
      prefix: output.trim().to_string(),
      repository_scoped: true,
    }),
    1 => Ok(GitBranchPrefix::default()),
    128 => Err(anyhow::anyhow!("Repository not accessible: {repo_path}")),
    code => {
      tracing::warn!(code, repo_path, "Unexpected exit code reading local branch prefix");
      Ok(GitBranchPrefix::default())
    }
  }
}

/// What the app of a test repository is made with beside the server's own: its settings, the event streams of its
/// pages, its desktop, its updater and the fakes of its CLIs, which its fixture routes reach too (`TestRepository`)
pub struct TestHosts {
  pub settings: std::sync::Arc<app_settings::service::SettingsService>,
  pub events: std::sync::Arc<crate::host::TestEvents>,
  pub desktop: std::sync::Arc<crate::host::TestDesktop>,
  pub updater: std::sync::Arc<crate::updater::TestUpdater>,
  pub cli_tools: std::sync::Arc<crate::cli_tools_fixtures::CliToolsHost>,
}

/// The symbol index of a test repository is its fixture, `.git/branch-deck-accio.json` (`FixtureIndex`), as accio
/// answers: for the symbol searches and for the fan-in of a review plan (`get_review_plan`); read at once, so a cancel
/// has no query to kill
pub struct FixtureIndexes;

impl app_api::reads::symbols::SymbolIndexes for FixtureIndexes {
  fn search<'a>(&'a self, _canceled: &'a (dyn Fn() -> bool + Sync)) -> Option<Box<dyn file_diffs::accio::SymbolIndex + 'a>> {
    Some(Box::new(file_diffs::accio::FixtureIndex))
  }
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  /// `AppState::app_data_of`: where the fake services keep the app's token files
  pub app_data: PathBuf,
  /// Held by every request of the page mock for this repository (`hold_repository` in lib.rs) and waited out by its
  /// deletion (`remove_repository`), so a repository is deleted only after the commands still running for it
  pub in_flight: std::sync::Arc<InFlight>,
  /// The settings of the page: the writer of the app on `settings.json` and `repository-state.json` in `app_data`,
  /// which `set_setting` and the `/store` fixture routes change
  pub store: std::sync::Arc<app_settings::service::SettingsService>,
  /// What the commands of app-api run with for this repository (`AppState::context_of`)
  pub context: std::sync::Arc<app_api::context::AppContext>,
  /// The event streams of the pages of the repository, which the app's events and the pages' own go to; closed with the
  /// repository
  pub events: std::sync::Arc<crate::host::TestEvents>,
  /// How often the app asked for pull requests, so a test can check the refresh policy
  pub pull_request_calls: AtomicU32,
  /// What the session reads of the review requests and how `fetch_review_head` fails (E2E never runs gh); not on GitHub until a
  /// test sets it
  pub review_requests: RwLock<ReviewRequestsFixture>,
  /// How often the app asked for review requests and fetched a review
  pub review_request_calls: AtomicU32,
  /// How many of them read the rest of Reviews too (the Reviews view showed)
  pub reviews_calls: AtomicU32,
  pub review_fetch_calls: AtomicU32,
  /// The `keep` of every `prune_review_refs`, in call order
  pub review_prune_keeps: std::sync::Mutex<Vec<Vec<u32>>>,
  /// The fake services behind the real ones (`remote_host.rs`), started on the first need; stopped with the repository
  pub remote: std::sync::OnceLock<std::sync::Arc<crate::remote_host::RemoteHost>>,
  /// The sessions of the server, which a sign-in of the repository's fakes has read the facts of its worktree again
  pub sessions: std::sync::Weak<repository_model::session::Sessions>,
  /// How often the session of the repository listed and polled the robots, so a test can check its request policy
  pub patronus_list_calls: AtomicU32,
  pub patronus_poll_calls: AtomicU32,
  /// By id, the robots the lists and the polls of the session answered last: a robot of a fixture without its revision
  /// has the one the fake resolved (`robots` of `GET /repositories/{id}/patronus`)
  pub patronus_answered: std::sync::Mutex<std::collections::BTreeMap<String, domain_types::patronus::PatronusRobot>>,
  /// How often the session read the details of a robot a window shows: the build progress of its checks comes only with them
  pub patronus_detail_calls: AtomicU32,
  /// The readiness reads of the sessions (`TestFactSources::readiness`), in order: the subjects asked and whether it was a recheck
  pub review_readiness_calls: std::sync::Mutex<Vec<serde_json::Value>>,
  /// What `suggest_branch_names` answers (E2E never runs claude)
  pub branch_name_suggestions: RwLock<BranchNameSuggestionFixture>,
  /// How often the app asked for branch names: nothing may leave before the user enabled AI
  pub branch_name_suggestion_calls: AtomicU32,
  /// What the desktop of the app recorded (its windows, its window menus) and the folder its dialog picks (`host.rs`)
  pub desktop: std::sync::Arc<crate::host::TestDesktop>,
  /// The version a check for updates finds (`updater.rs`); none: the app is up to date
  pub updater: std::sync::Arc<crate::updater::TestUpdater>,
  /// The fakes of the `teamcity` and `youtrack` CLIs and of `brew` (`cli_tools_fixtures`)
  pub cli_tools: std::sync::Arc<crate::cli_tools_fixtures::CliToolsHost>,
  /// The repository lock the server holds for a test (`hold_lock`), as another process (`bd pick`) would
  pub held_lock: std::sync::Mutex<Option<history_rewrite::repository_lock::RepositoryLock>>,
  /// The fetch lock the server holds for a test (`hold_lock`), as an agent's `bd update` that fetches the baseline
  /// would: a fetch of the page waits for it
  pub fetch_held: std::sync::Mutex<Option<history_rewrite::repository_lock::RepositoryLock>>,
  /// The operation an agent's `bd` runs in the repository for a test (`set_foreign_operation`)
  pub foreign_operation: std::sync::Arc<std::sync::Mutex<Option<crate::remote_fixtures::ForeignOperation>>>,
  /// What the next sync of the repository's session reports differently (`sync_overrides`), until a run takes it (never,
  /// for the overrides of every sync)
  pub sync_overrides: std::sync::Mutex<Option<crate::sync_overrides::SyncOverrides>>,
  /// The frame of the repository's session (`repository_model::warm_start`), as the app keeps it in the object cache:
  /// in memory, and only once a test built one (`POST /repositories/{id}/warm-start/build`), so the sessions of every
  /// other test seed from none; its sessions then save theirs here
  pub frame: std::sync::Mutex<Option<repository_model::warm_start::CachedSession>>,
}

impl TestRepository {
  /// The fake services of the repository, started now if not yet
  pub fn remote(&self, git: &GitCommandExecutor) -> std::sync::Arc<crate::remote_host::RemoteHost> {
    std::sync::Arc::clone(self.remote.get_or_init(|| {
      let (cli_tools, sessions) = (std::sync::Arc::clone(&self.cli_tools), std::sync::Weak::clone(&self.sessions));
      let events = std::sync::Arc::clone(&self.events) as std::sync::Arc<dyn app_api::transport::AppEvents>;
      std::sync::Arc::new(crate::remote_host::RemoteHost::start(git, &self.path, self.app_data.clone(), cli_tools, sessions, events))
    }))
  }

  /// The fake services once started
  pub fn existing_remote(&self) -> Option<std::sync::Arc<crate::remote_host::RemoteHost>> {
    self.remote.get().cloned()
  }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequestsFixture {
  /// An empty `headRefOid` or `baseRefOid` of a request is resolved by `fetch_review_head`: the tip of `HEAD`, the
  /// baseline
  #[serde(flatten)]
  pub result: ReviewRequestsResult,
  /// The error `fetch_review_head` fails with; None: it answers the range of the test repository
  #[serde(default)]
  pub fetch_error: Option<String>,
  /// The pull requests the fake GitHub has beside the requests and the rest of Reviews, which a link of the user can
  /// name (`link_pull_request`)
  #[serde(default)]
  pub linkable: Vec<LinkablePull>,
}

/// A pull request a link can name: open, or merged or closed
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkablePull {
  #[serde(flatten)]
  pub request: domain_types::review_request::ReviewRequest,
  #[serde(default)]
  pub closed: bool,
}

impl ReviewRequestsFixture {
  /// The fixture with the title of every pull request split at its issue key, as the reader of GitHub splits it
  /// (`remote_facts::review_requests`): the session takes the fixture in as GitHub's answer
  #[must_use]
  pub fn with_title_leads(mut self) -> Self {
    let lead = |request: &mut domain_types::review_request::ReviewRequest| request.title_lead = sync_utils::issue_pattern::issue_lead(&request.title);
    self.result.requests.iter_mut().for_each(lead);
    self.result.answered.iter_mut().for_each(|answered| lead(&mut answered.request));
    if let Some(others) = &mut self.result.others {
      for search in [&mut others.team, &mut others.reviewed, &mut others.authored] {
        search.entries.iter_mut().for_each(|entry| lead(&mut entry.request));
      }
      for linked in &mut others.linked {
        if let domain_types::review_request::LinkedAnswer::Open { entry } = linked {
          lead(&mut entry.request);
        }
      }
    }
    self.linkable.iter_mut().for_each(|pull| lead(&mut pull.request));
    self
  }
}

impl Default for ReviewRequestsFixture {
  fn default() -> Self {
    Self {
      result: ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
      fetch_error: None,
      linkable: Vec::new(),
    }
  }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchNameSuggestionFixture {
  /// None: two names from the words of the commit messages
  #[serde(default)]
  pub outcome: Option<BranchNameSuggestionOutcome>,
  /// How long the answer takes, so a test sees the request in flight
  #[serde(default = "default_suggestion_delay_ms")]
  pub delay_ms: u64,
}

const fn default_suggestion_delay_ms() -> u64 {
  100
}

/// The clocks of the sessions and of the scheduler of their facts, one per worktree (a test repository's path): each
/// stands still until its test moves it (`POST /repositories/{id}/clock/advance`), so a test that moves the time of its
/// sessions (their polls, their retries) moves no session of a test beside it
#[derive(Default)]
pub struct WorktreeClocks(DashMap<String, sync_utils::clock::TestClock>);

impl WorktreeClocks {
  /// The clock of the sessions of `worktree`
  pub fn of_worktree(&self, worktree: &str) -> sync_utils::clock::TestClock {
    self.0.entry(worktree.to_owned()).or_default().clone()
  }

  /// The repository went: its clock with it
  pub fn forget(&self, worktree: &str) {
    self.0.remove(worktree);
  }
}

impl repository_model::session::SessionClocks for WorktreeClocks {
  fn of(&self, key: &repository_model::session::SessionKey) -> std::sync::Arc<dyn sync_utils::clock::Clock> {
    std::sync::Arc::new(self.of_worktree(&key.worktree))
  }
}

/// The commands of the page that run for a test repository: a count each holds one of while it runs, which the deletion
/// of the repository waits out, never a lock a command waits for
#[derive(Default)]
pub struct InFlight {
  commands: tokio::sync::watch::Sender<Commands>,
}

#[derive(Default)]
struct Commands {
  running: usize,
  /// The deletion began: no command starts any more
  closed: bool,
}

/// A command that runs for a test repository, until it is dropped
pub struct CommandHold(std::sync::Arc<InFlight>);

impl InFlight {
  /// A command of the repository holds it until it ends; none once its deletion began
  pub fn hold(self: &std::sync::Arc<Self>) -> Option<CommandHold> {
    let held = self.commands.send_if_modified(|commands| {
      if commands.closed {
        return false;
      }
      commands.running += 1;
      true
    });
    held.then(|| CommandHold(std::sync::Arc::clone(self)))
  }

  /// No command holds the repository from now on; resolves once those that hold it ended
  pub async fn close(&self) {
    self.commands.send_modify(|commands| commands.closed = true);
    let mut commands = self.commands.subscribe();
    // the sender lives in `self`: the wait ends only when the count does
    let _ = commands.wait_for(|commands| commands.running == 0).await;
  }
}

impl Drop for CommandHold {
  fn drop(&mut self) {
    self.0.commands.send_modify(|commands| commands.running -= 1);
  }
}
