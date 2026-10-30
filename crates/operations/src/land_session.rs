//! The facts of a start of the app (and of the E2E server): the session of its repository (`SessionStart`). The plan
//! is prepared again on the actor of the session over the facts its scheduler holds, the landing needs of the view
//! among them (the landing gate of a branch the session ran in the background after its sync, the one trial of a start,
//! C2), with the replay of what lands onto the newest target, fetched on its own thread, and for a whole stack its
//! landing gate, a trial check of the session's grouping through the object cache of the repository; nothing of it holds
//! the lock. A check that could not run in full holds nothing back: the outcome says so (`StartNote`). The robots and
//! the pull requests after the push are the session's as they are; the robot a comment started is looked for in a list
//! of the robots started since shortly before the comment, and every robot the start starts or cancels goes into the
//! session, so every window shows it at once. A start of a repository that lands by a push into the target runs no check
//! against the newest target (a target that moved refuses the push as no fast-forward, which fetches it), and what its
//! push landed or the refusal of the server goes into the session (`Sessions::tell_baseline_push`), a fetch after a push
//! that was no fast-forward asks the sync that builds the copies anew.

use crate::land::planned_of;
use crate::land_execute::{Current, Locker, Planned, PullRequestText, PushSettings, PushedRef, StartHost, StartServices, lock_failure, shown_remote};
use crate::push::Sent;
use domain_types::RemoteStatusUpdate;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{BaselinePush, Land, LandingRefusal, Subject};
use domain_types::landing_route::LandingRoute;
use domain_types::operation::StartNote;
use domain_types::patronus::{PatronusAvailability, PatronusRobot};
use domain_types::session::{SyncReason, Version};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{git_common_dir, resolve_commit};
use github::service::GitHubService;
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, LockHolder, LockWaitCanceled, lock_repository_while_held};
use object_cache::cache::ObjectCache;
use patronus::service::PatronusService;
use remote_facts::fact_sources::Read;
use remote_facts::pull_request_create::{GetNewPullRequestParams, new_pull_request};
use remote_facts::service_sources::ServiceHost;
use repository_model::derive::{StartChecks, landing_route_of, with_checked_land_sources};
use repository_model::facts::BaselineRefusal;
use repository_model::facts::listed_robots;
use repository_model::patronus_view::landed_commits;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::RemoteSubject;
use std::collections::HashSet;
use std::sync::Arc;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::github_repository::GitHubRepository;
use sync_core::push::LeasedPush;
use sync_core::target_conflict::{TargetOutcome, check_target, fetch_target};
use sync_utils::span::in_current_span;
use tracing::warn;
use trial_check::check::CheckSession;
use trial_check::landing_gate::check_landing_gate;

/// A start in the open session of `key`
pub struct SessionStart {
  pub sessions: Arc<Sessions>,
  pub key: SessionKey,
  pub git: GitCommandExecutor,
  /// The service and the Space repository the robot of a comment is listed from; `new_api` as the settings say. `None`
  /// for a run that starts no robot (a pull request, a review request) off Space or without a token
  pub patronus: Option<Arc<PatronusService>>,
  pub space: String,
  pub new_api: bool,
  /// The object cache of the repository, which the landing gate of a whole stack checks through
  pub cache: ObjectCache,
}

/// What the plan of a start reads of the session before its checks: the tip of what lands and the baseline it is
/// replayed onto, and for a whole stack the commits merged Safe Pushes landed, which its landing gate counts as landed
struct Inputs {
  tip: Option<String>,
  baseline: String,
  landed: Option<HashSet<String>>,
  /// The repository lands by a push into the target: no check runs before the plan
  push_route: bool,
}

impl SessionStart {
  async fn inputs(&self, subject: &Subject) -> Option<Inputs> {
    let subject = subject.clone();
    self
      .sessions
      .read(&self.key, move |state, facts| {
        let table = facts.table_facts(state)?;
        let (tip, landed) = match &subject {
          Subject::Branch { name } => (table.table.tip_of(name).map(str::to_string), None),
          Subject::WholeStack { root } => (
            table.table.stack_branches.iter().find(|stack| stack.root == *root).and_then(|stack| stack.tip.clone()),
            Some(landed_commits(&facts.patronus.part().view)),
          ),
        };
        Some(Inputs {
          tip,
          baseline: table.table.baseline.clone(),
          landed,
          push_route: matches!(landing_route_of(state.header(), &state.sections().landed, facts), LandingRoute::Push { .. }),
        })
      })
      .await
      .flatten()
  }

  /// The checks of the start on the blocking pool, side by side: against the newest target (the target fetched first, a
  /// failed fetch replays onto the tip fetched last), and of a whole stack its landing gate; what did not run in full is
  /// a note, never a refusal
  async fn checks(&self, subject: &Subject, inputs: Inputs) -> Result<(StartChecks, Vec<StartNote>), ApiError> {
    let (git, path, baseline, tip) = (self.git.clone(), self.key.worktree.clone(), inputs.baseline, inputs.tip);
    let target = tokio::task::spawn_blocking(in_current_span(move || target_check(&git, &path, &baseline, tip)));
    let gate = match (subject, inputs.landed) {
      (Subject::WholeStack { .. }, Some(landed)) => {
        let (git, path, prefix, cache, subject) = (self.git.clone(), self.key.worktree.clone(), self.key.prefix.clone(), self.cache.clone(), subject.clone());
        Some(tokio::task::spawn_blocking(in_current_span(move || {
          let session = CheckSession::open(&git, &path, &prefix, &cache)?;
          let gate = check_landing_gate(&session, &subject, &landed)?;
          anyhow::Ok(gate.refusals(&session))
        })))
      }
      // the landing gate of a branch is the landing needs of the view, which the session keeps for the refs of its sync
      _ => None,
    };
    let failed = |error: tokio::task::JoinError| ApiError::internal(format!("A check of the start failed: {error}"));
    let (target_conflict, mut notes) = target.await.map_err(failed)?;
    let gate = match gate {
      None => None,
      Some(gate) => Some(gate_refusals(gate.await.map_err(failed)?, &mut notes)),
    };
    Ok((StartChecks { gate, target_conflict }, notes))
  }
}

/// The refusals of the landing gate of a whole stack; one that could not run refuses nothing, and the note says so
fn gate_refusals(checked: anyhow::Result<Vec<LandingRefusal>>, notes: &mut Vec<StartNote>) -> Vec<LandingRefusal> {
  checked.unwrap_or_else(|error| {
    warn!(error = format!("{error:#}"), "the landing gate of the whole stack did not run: it holds nothing back");
    notes.push(StartNote::LandingNotChecked { error: format!("{error:#}") });
    Vec::new()
  })
}

/// The check of `tip` against the newest `baseline`: its fetch, then the replay of what lands onto its tip, as Patronus
/// rebases it; the conflict (a copy of the virtual branch and its files), and what did not run in full
fn target_check(git: &GitCommandExecutor, path: &str, baseline: &str, tip: Option<String>) -> (Option<(String, Vec<String>)>, Vec<StartNote>) {
  let Some(tip) = tip else {
    return (None, Vec::new());
  };
  let fetch_error = fetch_target(git, path, None).into_error();
  let mut notes: Vec<StartNote> = fetch_error
    .iter()
    .map(|error| StartNote::TargetFetchFailed {
      target: baseline.to_string(),
      error: error.clone(),
    })
    .collect();
  let target_tip = match resolve_commit(git, path, baseline) {
    Ok(Some(target_tip)) => target_tip,
    Ok(None) => return (None, notes),
    Err(error) => {
      warn!(error = format!("{error:#}"), "the target of the start can't be read: no check against it");
      notes.push(StartNote::TargetNotChecked {
        target: baseline.to_string(),
        error: format!("{error:#}"),
      });
      return (None, notes);
    }
  };
  match check_target(git, path, baseline, &target_tip, &tip, fetch_error).outcome {
    TargetOutcome::Conflict(conflict) => (Some((conflict.commit.id, conflict.files)), notes),
    TargetOutcome::Clean => (None, notes),
    TargetOutcome::NotRun { error } => {
      notes.push(StartNote::TargetNotChecked {
        target: baseline.to_string(),
        error,
      });
      (None, notes)
    }
  }
}

impl StartHost for SessionStart {
  fn plan<'a>(&'a self, land: &'a Land) -> Read<'a, Result<Option<Planned>, ApiError>> {
    Box::pin(async move {
      let Some(inputs) = self.inputs(&land.subject).await else {
        return Ok(None);
      };
      // a push into the target is its own check against it: one that moved refuses it as no fast-forward
      let (checks, notes) = if inputs.push_route {
        (StartChecks::default(), Vec::new())
      } else {
        self.checks(&land.subject, inputs).await?
      };
      let (prefix, land) = (self.key.prefix.clone(), land.clone());
      let planned = self
        .sessions
        .read(&self.key, move |state, facts| {
          with_checked_land_sources(state, facts, &prefix, &checks, |sources| planned_of(sources, &land))
        })
        .await;
      Ok(planned.flatten().flatten().map(|planned| Planned { notes, ..planned }))
    })
  }

  fn current(&self) -> Read<'_, Current> {
    Box::pin(async move {
      self
        .sessions
        .read(&self.key, |_, facts| {
          let pull_requests = facts.pull_requests.view();
          Current {
            robots: listed_robots(facts.robots.view()),
            pull_requests: pull_requests.pull_requests.values().chain(pull_requests.stack_pull_requests.values()).cloned().collect(),
          }
        })
        .await
        .unwrap_or_default()
    })
  }

  fn robots_since(&self, since: u32) -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> {
    Box::pin(async move {
      let Some(patronus) = &self.patronus else {
        return Err(ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space"));
      };
      robots_since(patronus, &self.space, &self.key.prefix, self.new_api, since).await
    })
  }

  fn shown_remote<'a>(&'a self, pushes: &'a [PushedRef]) -> Read<'a, Vec<Option<RemoteStatusUpdate>>> {
    Box::pin(async move {
      let pushes = pushes.to_vec();
      let count = pushes.len();
      let shown = self
        .sessions
        .read(&self.key, move |state, _facts| pushes.iter().map(|pushed| shown_remote(state, pushed)).collect())
        .await;
      shown.unwrap_or_else(|| vec![None; count])
    })
  }

  fn begin_push(&self, subjects: Vec<RemoteSubject>) {
    drop(self.sessions.begin_push(&self.key, subjects));
  }

  fn end_push<'a>(&'a self, subjects: Vec<RemoteSubject>, sent: Option<&'a Sent>) -> Read<'a, Option<Version>> {
    Box::pin(async move {
      let pushed = sent.is_some_and(|sent| matches!(sent.answer, LeasedPush::Pushed(_)));
      let statuses = match sent.map(|sent| (&sent.answer, sent.prepared.clone())) {
        Some((LeasedPush::Pushed(_), prepared)) => {
          let git = self.git.clone();
          tokio::task::spawn_blocking(in_current_span(move || prepared.statuses(&git)))
            .await
            .ok()
            .and_then(|statuses| statuses.inspect_err(|error| warn!(error = format!("{error:#}"), "no remote status after the push")).ok())
        }
        Some((LeasedPush::RemoteMoved { remote_statuses, .. }, _)) => Some(remote_statuses.clone()),
        _ => None,
      };
      self.sessions.tell_push(&self.key, subjects, statuses, pushed).version().await
    })
  }

  fn write_robot(&self, robot: PatronusRobot) -> Read<'_, Option<Version>> {
    Box::pin(async move { self.sessions.write_robot(&self.key, robot).await })
  }

  fn write_pull_request(&self, write: PullRequestWrite) -> Read<'_, Option<Version>> {
    Box::pin(async move { self.sessions.write_pull_request(&self.key, write).await })
  }

  fn baseline_pushed(&self, landed: Vec<(String, BaselinePush)>, refused: Option<BaselineRefusal>) -> Read<'_, Option<Version>> {
    Box::pin(async move { self.sessions.tell_baseline_push(&self.key, landed, refused).await })
  }

  fn baseline_fetched(&self) {
    drop(self.sessions.sync(&self.key, SyncReason::AfterRewrite));
  }

  /// The text of the pull request of a layer below the subject of Create Pull Request, of its own commits against `base`
  /// (`remote_facts::pull_request_create::new_pull_request`), the summaries of its issues left out: no YouTrack is asked
  fn layer_text<'a>(&'a self, repository_path: &'a str, head: &'a str, base: &'a str) -> Read<'a, Result<(String, String), ApiError>> {
    Box::pin(async move {
      let params = GetNewPullRequestParams {
        repository_path: repository_path.to_owned(),
        head: head.to_owned(),
        base: base.to_owned(),
        stack_ancestors: Vec::new(),
      };
      new_pull_request(&self.git, None, "origin", &params)
        .await
        .map(|text| (text.title, text.body))
        .map_err(|error| ApiError::new(ErrorCode::GitError, format!("Cannot read the commits of {head}: {error:#}")))
    })
  }

  /// A start of the app opens no pull request: Create Pull Request does, with its own text
  fn pull_request_text<'a>(&'a self, _sent: Option<&'a Sent>) -> Read<'a, Result<PullRequestText, ApiError>> {
    Box::pin(std::future::ready(Err(ApiError::new(
      ErrorCode::InvalidArguments,
      "A start of the app opens no pull request: Create Pull Request opens it",
    ))))
  }
}

/// The robots of the user in the Space repository `space` started since `since` (Unix seconds), asked now: where the
/// robot of a `/patronus` comment is looked for
///
/// # Errors
/// What Patronus answered instead: an error, a token it rejected, a repository not on Space
pub(crate) async fn robots_since(patronus: &PatronusService, space: &str, prefix: &str, new_api: bool, since: u32) -> Result<Vec<PatronusRobot>, ApiError> {
  let result = patronus.robots(space, prefix, Some(since), false, new_api).await;
  match result.availability {
    PatronusAvailability::Available => Ok(result.robots),
    PatronusAvailability::Error { message } => Err(ApiError::new(ErrorCode::PatronusError, message)),
    PatronusAvailability::NotLoggedIn => Err(ApiError::new(ErrorCode::NotLoggedIn, "Patronus rejected the Space token")),
    PatronusAvailability::NotPatronus => Err(ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space")),
  }
}

/// The start of the app in the open session of `key` and what it runs on, over the services `services` resolves (the
/// app's and the E2E server's `ServiceHost`): the Patronus service with the Space repository, the GitHub repository of
/// the branches, the target of the baseline, the size limit of a push; the repository lock waited for while its holder
/// lives (`lock_repository_while_held`), until the user stops the start; the reply of a comment on the clock of the
/// session, its robot looked for until it is listed
///
/// # Errors
/// A session that is not open or has no table yet, a repository not on Space, a baseline that is no branch of
/// `origin`, git failing
pub async fn session_start(services: &dyn ServiceHost, sessions: &Arc<Sessions>, key: &SessionKey) -> Result<(SessionStart, StartServices), ApiError> {
  let space = services
    .space_repository(key)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it"))?;
  let patronus = services.patronus(key)?;
  let github = match services.github_repository(key).await? {
    Some(repository) => Some((services.github(key)?, repository)),
    None => None,
  };
  start_of(services, sessions, key, Some((patronus, space.name)), github, Target::Required).await
}

/// What the creation of a pull request and a review request of the app run on (`session_start` without Patronus: they
/// start no robot, off Space too); the Patronus service while there is one. Without the GitHub repository of the
/// branches an effect that writes to GitHub fails, the push before it goes ("Continue on GitHub" of a pushed branch
/// writes nothing)
///
/// # Errors
/// As `session_start`, but for Space
pub async fn pull_request_start(services: &dyn ServiceHost, sessions: &Arc<Sessions>, key: &SessionKey) -> Result<(SessionStart, StartServices), ApiError> {
  let (patronus, github) = optional_services(services, key).await?;
  start_of(services, sessions, key, patronus, github, Target::Required).await
}

/// What a push of the app runs on (`operations::push::execute_push`): `pull_request_start` without the target, which a
/// push needs not (no robot starts, its retarget names its base); GitHub for the retarget of its pull request and
/// Patronus for the restart of its Safe Push while there are
///
/// # Errors
/// A session without a sync yet, the git of the inputs failing
pub async fn push_start(services: &dyn ServiceHost, sessions: &Arc<Sessions>, key: &SessionKey) -> Result<(SessionStart, StartServices), ApiError> {
  let (patronus, github) = optional_services(services, key).await?;
  start_of(services, sessions, key, patronus, github, Target::None).await
}

/// The Patronus service with the Space repository and the GitHub service with the repository of the branches, while
/// the host has them
async fn optional_services(
  services: &dyn ServiceHost,
  key: &SessionKey,
) -> Result<(Option<(Arc<PatronusService>, String)>, Option<(Arc<GitHubService>, GitHubRepository)>), ApiError> {
  let github = match services.github_repository(key).await? {
    Some(repository) => Some((services.github(key)?, repository)),
    None => None,
  };
  let patronus = match services.space_repository(key).await {
    Ok(Some(space)) => services.patronus(key).ok().map(|patronus| (patronus, space.name)),
    _ => None,
  };
  Ok((patronus, github))
}

/// Whether a run needs the branch of origin its robots push into
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
  /// It starts a robot, or opens a pull request into it
  Required,
  /// A push: no git process asks for it
  None,
}

/// The run in the open session of `key` over `services`, with the Patronus service and the Space repository `patronus`
/// and the GitHub of the branches `github`, and the target the robots push into as `target` says (empty without it)
async fn start_of(
  services: &dyn ServiceHost,
  sessions: &Arc<Sessions>,
  key: &SessionKey,
  patronus: Option<(Arc<PatronusService>, String)>,
  github: Option<(Arc<GitHubService>, GitHubRepository)>,
  target: Target,
) -> Result<(SessionStart, StartServices), ApiError> {
  let baseline = sessions
    .table(key)
    .await
    .map(|table| table.table.baseline.clone())
    .ok_or_else(|| ApiError::new(ErrorCode::NoSession, "The repository has no open session with a sync yet"))?;
  let (patronus, space) = patronus.map_or((None, String::new()), |(patronus, space)| (Some(patronus), space));
  let target = match target {
    Target::Required => services.target(key, &baseline).await?,
    Target::None => String::new(),
  };
  let (git, config_git, path) = (services.git(), services.config_git(key), key.worktree.clone());
  let (common_dir, size_limit) = tokio::task::spawn_blocking(in_current_span(move || {
    let common_dir = git_common_dir(&git, &path)?;
    let size_limit = ConfigSnapshot::read(&config_git, &path)?.push_size_limit()?;
    anyhow::Ok((std::path::PathBuf::from(common_dir), size_limit))
  }))
  .await
  .map_err(|error| ApiError::internal(format!("The inputs of the start failed: {error}")))?
  .map_err(|error| ApiError::new(ErrorCode::GitError, format!("{error:#}")))?;
  let start = SessionStart {
    sessions: Arc::clone(sessions),
    key: key.clone(),
    git: services.git(),
    patronus: patronus.clone(),
    space: space.clone(),
    new_api: services.new_api(key),
    cache: services.object_cache(key).await,
  };
  let lock_dir = common_dir.clone();
  let lock: Locker = Arc::new(move |on_wait, canceled| {
    let mut on_holder = |holder: &LockHolder| on_wait(Some(holder));
    lock_repository_while_held(&lock_dir, DEFAULT_LOCK_TIMEOUT, &mut on_holder, canceled).map_err(|error| {
      if error.downcast_ref::<LockWaitCanceled>().is_some() {
        return ApiError::new(ErrorCode::Canceled, "The start was stopped while it waited for the repository lock; nothing was done");
      }
      lock_failure(&error)
    })
  });
  let run = StartServices {
    git: services.git(),
    repository_path: key.worktree.clone(),
    git_common_dir: common_dir,
    prefix: key.prefix.clone(),
    patronus,
    space,
    github,
    target,
    push: PushSettings {
      remote: "origin".to_string(),
      baseline,
      size_limit,
    },
    lock,
    clock: sessions.clock(key),
    wait_for_robot: true,
  };
  Ok((start, run))
}
