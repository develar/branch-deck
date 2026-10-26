//! The run of `bd safe-push` once its plan holds: the start through the engine (`operations::land_execute::execute`,
//! the app's too) over the one-shot session of the command (`BdStart`), and what its outcome tells in the words of `bd`.
//! The sync ran under the repository lock and released it; the engine takes it again only for the full witness and the
//! freeze of the push, so the reads of the remote, the trial and the check against the target kept no other `bd` writer
//! waiting. A rewrite of another session between the sync and the witness changes the plan: `plan_changed` (exit 3),
//! nothing done; one after the push and before the start skips the start, and the error says what was pushed.

use crate::push_command::push_size_limit;
use crate::safe_push_command::STACK_REMOTE;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::Land;
use domain_types::patronus::{PatronusAvailability, PatronusRobot};
use github::service::GitHubService;
use operations::land::{LandHost, one_shot_planned};
use operations::land_execute::{Current, Locker, Planned, PullRequestText, PushSettings, StartHost, StartServices, bounded_lock};
use operations::push::Sent;
use patronus::service::PatronusService;
use remote_facts::fact_sources::{FactSources, Read};
use repository_model::one_shot::OneShot;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::session::SessionKey;
use repository_model::table::RemoteSubject;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use sync_utils::clock::SystemClock;

/// The facts of a start of `bd`: its one-shot session as the plan read it, prepared again on it (the same facts, so the
/// same plan while the refs are as the sync left them), the robots of a comment asked through `sources`
pub(crate) struct BdStart<'a> {
  pub(crate) session: &'a OneShot,
  pub(crate) host: LandHost<'a>,
  pub(crate) sources: Arc<dyn FactSources>,
  pub(crate) key: SessionKey,
  /// The pull request a `--create-pull-request` opens, worded before the run (its issues read from YouTrack)
  pub(crate) text: Option<PullRequestText>,
}

impl StartHost for BdStart<'_> {
  fn plan<'a>(&'a self, land: &'a Land) -> Read<'a, Result<Option<Planned>, ApiError>> {
    Box::pin(std::future::ready(Ok(one_shot_planned(self.session, land, &self.host))))
  }

  fn current(&self) -> Read<'_, Current> {
    let pull_requests = self.session.facts().pull_requests.view();
    let current = Current {
      robots: self.session.robots().to_vec(),
      pull_requests: pull_requests.pull_requests.values().chain(pull_requests.stack_pull_requests.values()).cloned().collect(),
    };
    Box::pin(std::future::ready(current))
  }

  fn robots_since(&self, since: u32) -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> {
    Box::pin(async move {
      let result = self.sources.robots(&self.key, Some(since), false).await;
      match result.availability {
        PatronusAvailability::Available => Ok(result.robots),
        PatronusAvailability::NotLoggedIn => Err(ApiError::new(
          ErrorCode::NotLoggedIn,
          "Patronus rejected the Space token: run ./safePush.cmd once to log in again",
        )),
        PatronusAvailability::Error { message } => Err(ApiError::new(ErrorCode::PatronusError, message)),
        PatronusAvailability::NotPatronus => Err(ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it")),
      }
    })
  }

  /// A command keeps no session: its output tells the push
  fn begin_push(&self, _subjects: Vec<RemoteSubject>) {}

  fn end_push<'a>(&'a self, _subjects: Vec<RemoteSubject>, _sent: Option<&'a Sent>) -> Read<'a, Option<domain_types::session::Version>> {
    Box::pin(std::future::ready(None))
  }

  fn write_robot(&self, _robot: PatronusRobot) -> Read<'_, Option<domain_types::session::Version>> {
    Box::pin(std::future::ready(None))
  }

  /// A command keeps no session: its output tells the pull request and the reviewers
  fn write_pull_request(&self, _write: PullRequestWrite) -> Read<'_, Option<domain_types::session::Version>> {
    Box::pin(std::future::ready(None))
  }

  fn pull_request_text<'a>(&'a self, _sent: Option<&'a Sent>) -> Read<'a, Result<PullRequestText, ApiError>> {
    let text = self
      .text
      .clone()
      .ok_or_else(|| ApiError::new(ErrorCode::InvalidArguments, "The start opens no pull request without --create-pull-request"));
    Box::pin(std::future::ready(text))
  }
}

/// What a run of the engine of `bd` runs on: the repository of `context`, the Patronus service and the Space repository
/// of its robots (`patronus`: none for a review request off Space), the GitHub service and repository of the branches,
/// the branch of origin they go into, the push to `origin` compared with `baseline`; the lock waited for a bounded time,
/// the reply of a comment read once
pub(crate) fn start_services(
  context: &RepoContext,
  patronus: Option<Arc<PatronusService>>,
  space: String,
  github: Option<(Arc<GitHubService>, GitHubRepository)>,
  target: String,
  baseline: &str,
) -> Result<StartServices, CliError> {
  Ok(StartServices {
    git: context.git.clone(),
    repository_path: context.repository.clone(),
    git_common_dir: context.git_common_dir.clone(),
    prefix: context.prefix.clone(),
    patronus,
    space,
    github,
    target,
    push: PushSettings {
      remote: STACK_REMOTE.to_string(),
      baseline: baseline.to_string(),
      my_email: None,
      size_limit: push_size_limit(context)?,
    },
    lock: locker(context),
    clock: Arc::new(SystemClock::new()),
    wait_for_robot: false,
  })
}

/// The repository lock of `bd` for the full witness and the freeze: the bounded wait of the command (`locked`)
pub(crate) fn locker(context: &RepoContext) -> Locker {
  bounded_lock(context.git_common_dir.clone(), context.lock_timeout)
}

/// An error of the engine as `bd` fails with it
pub(crate) fn cli_error(error: ApiError) -> CliError {
  CliError::new(error.code, error.message)
}
