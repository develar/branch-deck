//! The facts of a start of the app (and of the E2E server): the session of its repository (`SessionStart`). The plan
//! is prepared again on the actor of the session over the facts its scheduler holds, the landing needs of the view
//! among them (the landing gate the session ran in the background after its sync, the one trial of a start, C2), with
//! the replay of what lands onto the newest target, fetched on its own thread; nothing of it holds the lock. The robots
//! and the pull requests after the push are the session's as they are; the robot a comment started is looked for in a
//! list of the robots started since shortly before the comment, and every robot the start starts or cancels goes into
//! the session, so every window shows it at once.

use crate::land::planned_of;
use crate::land_execute::{Current, Locker, Planned, PullRequestText, PushSettings, StartHost, StartServices};
use crate::push::Sent;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Land, Subject};
use domain_types::patronus::{PatronusAvailability, PatronusRobot};
use domain_types::session::Version;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{git_common_dir, resolve_commit};
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, LockHolder, LockWaitCanceled, RepositoryLocked, lock_repository_while_held};
use patronus::service::PatronusService;
use remote_facts::fact_sources::Read;
use remote_facts::service_sources::ServiceHost;
use repository_model::derive::{StartChecks, with_checked_land_sources};
use repository_model::facts::listed_robots;
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::RemoteSubject;
use std::sync::Arc;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::push::LeasedPush;
use sync_core::target_conflict::{TargetOutcome, check_target, fetch_target};
use sync_utils::span::in_current_span;
use tracing::warn;

/// A start in the open session of `key`
pub struct SessionStart {
  pub sessions: Arc<Sessions>,
  pub key: SessionKey,
  pub git: GitCommandExecutor,
  /// The service and the Space repository the robot of a comment is listed from; `new_api` as the settings say
  pub patronus: Arc<PatronusService>,
  pub space: String,
  pub new_api: bool,
}

/// What the plan of a start reads of the session before its check against the target: the tip of what lands and the
/// baseline it is replayed onto
struct Inputs {
  tip: Option<String>,
  baseline: String,
}

impl SessionStart {
  async fn inputs(&self, subject: &Subject) -> Option<Inputs> {
    let subject = subject.clone();
    self
      .sessions
      .read(&self.key, move |state, facts| {
        let table = facts.table_facts(state)?;
        let tip = match &subject {
          Subject::Branch { name } => table.table.tip_of(name).map(str::to_string),
          Subject::WholeStack { root } => table.table.stack_branches.iter().find(|stack| stack.root == *root).and_then(|stack| stack.tip.clone()),
        };
        Some(Inputs {
          tip,
          baseline: table.table.baseline.clone(),
        })
      })
      .await
      .flatten()
  }

  /// The check against the newest target on the blocking pool: the target fetched first (a failed fetch replays onto
  /// the tip fetched last), then the replay of the tip onto it
  async fn checks(&self, inputs: Inputs) -> Result<StartChecks, ApiError> {
    let (git, path, baseline, tip) = (self.git.clone(), self.key.worktree.clone(), inputs.baseline, inputs.tip);
    let target_conflict = tokio::task::spawn_blocking(in_current_span(move || {
      let tip = tip?;
      let fetch_error = fetch_target(&git, &path).into_error();
      let target_tip = match resolve_commit(&git, &path, &baseline) {
        Ok(Some(target_tip)) => target_tip,
        Ok(None) => return None,
        Err(error) => {
          warn!(error = format!("{error:#}"), "the target of the start can't be read: no check against it");
          return None;
        }
      };
      match check_target(&git, &path, &baseline, &target_tip, &tip, fetch_error).outcome {
        TargetOutcome::Conflict(conflict) => Some((conflict.commit.id, conflict.files)),
        _ => None,
      }
    }))
    .await
    .map_err(|error| ApiError::internal(format!("The target check failed: {error}")))?;
    // the landing gate is the landing needs of the view, which the session keeps for the refs of its last sync
    Ok(StartChecks { gate: None, target_conflict })
  }
}

impl StartHost for SessionStart {
  fn plan<'a>(&'a self, land: &'a Land) -> Read<'a, Result<Option<Planned>, ApiError>> {
    Box::pin(async move {
      let Some(inputs) = self.inputs(&land.subject).await else {
        return Ok(None);
      };
      let checks = self.checks(inputs).await?;
      let (prefix, land) = (self.key.prefix.clone(), land.clone());
      let planned = self
        .sessions
        .read(&self.key, move |state, facts| {
          with_checked_land_sources(state, facts, &prefix, &checks, |sources| planned_of(sources, &land))
        })
        .await;
      Ok(planned.flatten().flatten())
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
      let result = self.patronus.robots(&self.space, &self.key.prefix, Some(since), false, self.new_api).await;
      match result.availability {
        PatronusAvailability::Available => Ok(result.robots),
        PatronusAvailability::Error { message } => Err(ApiError::new(ErrorCode::PatronusError, message)),
        PatronusAvailability::NotLoggedIn => Err(ApiError::new(ErrorCode::NotLoggedIn, "Patronus rejected the Space token")),
        PatronusAvailability::NotPatronus => Err(ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space")),
      }
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

  /// A start of the app opens no pull request: Create Pull Request does, with its own text
  fn pull_request_text<'a>(&'a self, _sent: Option<&'a Sent>) -> Read<'a, Result<PullRequestText, ApiError>> {
    Box::pin(std::future::ready(Err(ApiError::new(
      ErrorCode::InvalidArguments,
      "A start of the app opens no pull request: Create Pull Request opens it",
    ))))
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
  let baseline = sessions
    .table(key)
    .await
    .map(|table| table.table.baseline.clone())
    .ok_or_else(|| ApiError::new(ErrorCode::NoSession, "The repository has no open session with a sync yet"))?;
  let space = services
    .space_repository(key)
    .await?
    .ok_or_else(|| ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it"))?;
  let patronus = services.patronus(key)?;
  let github = match services.github_repository(key).await? {
    Some(repository) => Some((services.github(key)?, repository)),
    None => None,
  };
  let target = services.target(key, &baseline).await?;
  let (git, path) = (services.git(), key.worktree.clone());
  let (common_dir, size_limit) = tokio::task::spawn_blocking(in_current_span(move || {
    let common_dir = git_common_dir(&git, &path)?;
    let size_limit = ConfigSnapshot::read(&git, &path)?.push_size_limit()?;
    anyhow::Ok((std::path::PathBuf::from(common_dir), size_limit))
  }))
  .await
  .map_err(|error| ApiError::internal(format!("The inputs of the start failed: {error}")))?
  .map_err(|error| ApiError::new(ErrorCode::GitError, format!("{error:#}")))?;
  let start = SessionStart {
    sessions: Arc::clone(sessions),
    key: key.clone(),
    git: services.git(),
    patronus: Arc::clone(&patronus),
    space: space.name.clone(),
    new_api: services.new_api(key),
  };
  let lock_dir = common_dir.clone();
  let lock: Locker = Arc::new(move |on_wait, canceled| {
    let mut waited = false;
    let mut on_holder = |_: &LockHolder| {
      if !std::mem::replace(&mut waited, true) {
        on_wait();
      }
    };
    lock_repository_while_held(&lock_dir, DEFAULT_LOCK_TIMEOUT, &mut on_holder, canceled).map_err(|error| {
      if error.downcast_ref::<LockWaitCanceled>().is_some() {
        return ApiError::new(ErrorCode::Canceled, "The start was stopped while it waited for the repository lock; nothing was done");
      }
      match error.downcast_ref::<RepositoryLocked>() {
        Some(locked) => ApiError::new(ErrorCode::Locked, locked.to_string()),
        None => ApiError::new(ErrorCode::GitError, format!("{error:#}")),
      }
    })
  });
  let run = StartServices {
    git: services.git(),
    repository_path: key.worktree.clone(),
    git_common_dir: common_dir,
    prefix: key.prefix.clone(),
    patronus,
    space: space.name,
    github,
    target,
    push: PushSettings {
      remote: "origin".to_string(),
      baseline,
      my_email: None,
      size_limit,
    },
    lock,
    clock: sessions.clock(key),
    wait_for_robot: true,
  };
  Ok((start, run))
}
