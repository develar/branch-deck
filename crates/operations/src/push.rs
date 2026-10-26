//! The push of the virtual branches, one core for the app and `bd` (docs/design/git-operations.md, "Push and land";
//! docs/design/rust-domain-authority.md, "Конкурентность"): the repository lock is held only while the push is decided
//! and its commits and leases are fixed (`PreparedPush`: `<commit>:refs/heads/<name>`, each leased on the remote head
//! the user saw), never across the network. After it, `send` pushes exactly those commits in one push, and a remote
//! branch that moved meanwhile without commits a branch lacks takes the push once more over its new head, the same
//! commits (D4, `PreparedPush::over_moved`); a robot the push restarts is canceled before it and restarted on the pushed
//! commit after it (`cancel_robots`, `restart_robots`).
//!
//! The app's push (`push_branch`, `push_branches` of both hosts) is an operation of its session (`PushHost`): admitted
//! per ref it writes (`Resource::Ref`), so a push of a ref another push of the process writes waits for it and the
//! pushes of other refs run side by side, and for a fetch of its refs that runs (`ref_claims`); it waits for a sync of the session that runs (`WaitingForSync`), reads the
//! user's robots (none for an unapplied branch, C3), decides under the lock (`push_guard::check_push`) and freezes
//! (`prepare_push_branches`), pushes after it with its progress told to its caller, and its end goes to the session
//! (`Sessions::begin_push`, `tell_push`: the remote statuses it left). It runs in a task of its own, so a caller that goes
//! away leaves it running to its end, its refs held in the registry until its `git push` ended. `bd push` decides its refs on the sync it runs under the lock (the named exception of the lock span,
//! until s11) and freezes them on the tips that sync left (`PreparedPush::leased`), then sends them the same way.

use crate::events::EventSink;
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard, Stepper};
use anyhow::{Result, anyhow};
use domain_types::RemoteStatusUpdate;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::operation::{EffectRecord, EffectStatus, Intent, NewsKey, OperationStats, OperationStep, Outcome, Push, PushScope, Resource};
use domain_types::patronus::PatronusRobot;
use domain_types::session::{SyncReason, SyncState, ViewAnswer};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{LockHolder, LockWaitCanceled, RepositoryLocked, lock_repository_while_held};
use patronus::service::PatronusService;
use repository_model::facts::TableFacts;
use repository_model::ref_claims::claim_refs;
use repository_model::push_guard::{check_push, guard_facts};
use repository_model::session::{Applied, SessionKey, Sessions};
use repository_model::table::RemoteSubject;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use sync_core::push::{LeasedPush, NOT_SYNCED_YET, PreparedPush, PushBranchesOutcome, PushBranchesParams, StackPushBranch, prepare_push_branches, push_ref_names};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::span::in_current_span;
use tracing::{Instrument, debug, instrument};

/// Whether a push the remote refused because a remote branch moved goes once more over its new head
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverMoved {
  /// Once, when the remote of each one that moved has no commits its branch lacks (D4)
  Once,
  /// Never: the lease is the one the user named (`bd push --expect-remote`)
  Never,
}

/// A push sent (`send`): the refs as they went, what the remote answered, and what a push again went over
#[derive(Debug)]
pub struct Sent {
  /// The refs of the push that answered `answer`: re-leased on the new heads when it went over a moved remote
  pub prepared: PreparedPush,
  pub answer: LeasedPush,
  /// The push went again over a moved remote: what the first push found
  pub over: Option<Over>,
}

/// What the refused first push of `Sent` found, which the push again went over
#[derive(Debug, Clone)]
pub struct Over {
  /// The refs whose remote had moved, indexes into the refs, parents first
  pub moved: Vec<usize>,
  /// The remote status of every ref after the fetch of the refusal, in their order
  pub statuses: Vec<RemoteStatusUpdate>,
}

impl Sent {
  /// The remote status the ref at `index` went over, when its remote had moved
  #[must_use]
  pub fn moved_to(&self, index: usize) -> Option<&RemoteStatusUpdate> {
    self.over.as_ref().filter(|over| over.moved.contains(&index)).and_then(|over| over.statuses.get(index))
  }
}

/// The network part of every push, after the repository lock: the frozen commits of `prepared` in one push, its
/// progress to `on_progress`; refused because remote branches moved without commits their branches lack, once more over
/// what the remote has now (`over`), the same commits. Blocking
///
/// # Errors
/// What git failed with
#[instrument(skip_all, fields(branches = prepared.branches().len()))]
pub fn send(git: &GitCommandExecutor, prepared: PreparedPush, over: OverMoved, on_progress: &mut dyn FnMut(TransferProgress)) -> Result<Sent> {
  let answer = prepared.push(git, on_progress)?;
  let again = match (over, &answer) {
    (OverMoved::Once, LeasedPush::RemoteMoved { moved, remote_statuses }) => prepared.over_moved(&answer).map(|again| {
      let found = Over {
        moved: moved.clone(),
        statuses: remote_statuses.clone(),
      };
      (again, found)
    }),
    _ => None,
  };
  let Some((again, found)) = again else {
    return Ok(Sent { prepared, answer, over: None });
  };
  debug!(moved = ?found.moved, "a remote branch moved without commits its branch lacks: the push goes again over it");
  let answer = again.push(git, on_progress)?;
  Ok(Sent {
    prepared: again,
    answer,
    over: Some(found),
  })
}

/// Cancels `robots` before the push that restarts them (`--restart-running`), after the lock: a push won't land while
/// one runs on its branch
///
/// # Errors
/// What Patronus answered for the first that failed; the ones before it stay canceled
pub async fn cancel_robots(service: &PatronusService, branch_prefix: &str, robots: &[PatronusRobot]) -> Result<(), String> {
  for robot in robots {
    service.cancel(branch_prefix, &robot.id).await?;
  }
  Ok(())
}

/// Starts the canceled `robots` again on the commits just pushed: the new robots, in their order
///
/// # Errors
/// What Patronus answered for the first that failed
pub async fn restart_robots(service: &PatronusService, branch_prefix: &str, robots: &[PatronusRobot]) -> Result<Vec<PatronusRobot>, String> {
  let mut restarted = Vec::with_capacity(robots.len());
  for robot in robots {
    restarted.push(service.restart(branch_prefix, &robot.id).await?);
  }
  Ok(restarted)
}

/// What the app's push of a repository runs on, the same for the app's commands and the E2E server's routes: git, the
/// `$GIT_COMMON_DIR` of the repository (where its lock lives), the registry of the operations of its sessions, the key of
/// its session (the worktree and the branch prefix the host resolved for the command), the robots of the user and where
/// the caller hears the operation. Owned: the push runs in a task of its own (`push_branches`)
#[derive(Clone)]
pub struct PushHost {
  pub git: GitCommandExecutor,
  pub git_common_dir: PathBuf,
  pub registry: Registry,
  pub key: SessionKey,
  /// The service and the name of the Space repository of the robots that hold a push; `None` off Space: nothing holds it
  pub patronus: Option<(Arc<PatronusService>, String)>,
  pub events: Arc<dyn EventSink>,
  /// How long a lock whose holder no record names is waited for (`DEFAULT_LOCK_TIMEOUT`); a live holder keeps the push
  /// waiting as long as it lives
  pub unknown_holder_timeout: Duration,
}

impl PushHost {
  /// The app's push of `params` in its session: admitted per ref it writes (a push of the same refs waits for the one
  /// before, then goes over what that one sent; the pushes of other refs run beside it), after the sync of the session
  /// that runs, if one does; the user's robots (one full list asked now, none for unapplied branches), then, in a
  /// blocking step, the repository lock, the checks (`check_push`: a branch the last sync did not build, a parked one,
  /// one a merged Safe Push landed whole, one a running Safe Push holds) and the commits and leases fixed, then after the
  /// lock the push of those commits (`send`, once more over a remote that moved without commits a branch lacks). While
  /// another holder has the lock, the push waits as long as it lives, its progress naming it, until `canceled` says the
  /// user stopped waiting (`LockWaitCanceled`, nothing pushed); a lock no record names refuses it after
  /// `unknown_holder_timeout`, and so does a repository not synced yet. `on_progress` gets how far the push is. Answers
  /// the outcome with the version of the view after its patch (`None`: the session is not open). The push runs in a task
  /// of its own: a caller that goes away (its future dropped) leaves it running to its end, which goes to the session and
  /// the registry when it ended, so no push of its refs is admitted while its `git push` still sends
  ///
  /// # Errors
  /// A refusal of the checks or the lock, a push the remote refused for another reason, what git failed with
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branches = params.branches.len()))]
  pub async fn push_branches(
    &self,
    params: PushBranchesParams,
    on_progress: impl FnMut(TransferProgress) + Send + 'static,
    canceled: impl Fn() -> bool + Send + 'static,
  ) -> Result<ViewAnswer<PushBranchesOutcome>> {
    let host = self.clone();
    tokio::spawn(async move { host.push_to_its_end(params, on_progress, canceled).await }.in_current_span()).await?
  }

  /// `push_branches` in its own task
  async fn push_to_its_end(
    self,
    params: PushBranchesParams,
    on_progress: impl FnMut(TransferProgress) + Send + 'static,
    canceled: impl Fn() -> bool + Send + 'static,
  ) -> Result<ViewAnswer<PushBranchesOutcome>> {
    let names: Vec<String> = push_ref_names(&self.key.prefix, &params)?.into_iter().map(|(name, _)| name).collect();
    let request = OperationRequest {
      intent: Intent::Push(intent_of(&params.branches)),
      resources: names.iter().map(|name| Resource::Ref { name: name.clone() }).collect(),
      step: OperationStep::Checking,
      moves: Vec::new(),
    };
    let guard = self.admitted(request).await?;
    // a fetch of a remote branch whose pull request moved writes the remote-tracking ref too: the push is newer
    let _claim = claim_refs(&self.git_common_dir, &names, || debug!("the push waits for the fetch of its refs")).await;
    let subjects: Vec<RemoteSubject> = params.branches.iter().map(RemoteSubject::of).collect();
    let mut in_session = InSession::begin(self.registry.sessions(), &self.key, subjects);
    let pushed = self.run(&guard, params, on_progress, canceled).await;
    let version = in_session.end(pushed.as_ref().ok()).version().await;
    let answer = answer_of(&guard, &names, &pushed, version);
    guard.finish(answer);
    pushed.map(|outcome| ViewAnswer { outcome, version })
  }

  /// The guard of `request` once the registry runs it. A request of the same intent joined the push before it: the
  /// adapters of `push_branch(es)` answer a push outcome, not the joined one's, so it waits for that push to end and is
  /// admitted again, to push what the remote still lacks
  async fn admitted(&self, request: OperationRequest) -> Result<RunningGuard> {
    loop {
      match self.registry.admit(&self.key, request.clone(), Arc::clone(&self.events)).await? {
        Admitted::Run(guard) => return Ok(guard),
        Admitted::Joined(joined) => {
          debug!(op = %joined.op().0, "the push waits for the same push before it");
          drop(joined.answer().await);
        }
      }
    }
  }

  /// The admitted push: the sync it waits for, the robots, the checks and the freeze under the lock, the push after it
  async fn run(
    &self,
    guard: &RunningGuard,
    params: PushBranchesParams,
    mut on_progress: impl FnMut(TransferProgress) + Send + 'static,
    canceled: impl Fn() -> bool + Send + 'static,
  ) -> Result<PushBranchesOutcome> {
    let sessions = self.registry.sessions();
    // the sync that brings the tips into the table runs (a rewrite's, a focus'): the push checks what it builds; a sync
    // asked after it is not waited for
    if sessions.apply_with(&self.key, |state| matches!(state.sync(), SyncState::Running { .. })).await == Some(true) {
      guard.step(OperationStep::WaitingForSync);
      if let Err(error) = sessions.sync(&self.key, SyncReason::Open).wait().await {
        debug!(error = %error.message, "the push checks the table of the last successful sync");
      }
      guard.step(OperationStep::Checking);
    }
    let robots = if params.branches.iter().all(|branch| branch.unapplied) {
      Vec::new()
    } else {
      self.robots().await
    };
    // the table of the last sync: a sync takes no lock, so the one read under it would be no newer
    let table = sessions.table(&self.key).await;
    let (git, common_dir, timeout, prefix, stepper) = (
      self.git.clone(),
      self.git_common_dir.clone(),
      self.unknown_holder_timeout,
      self.key.prefix.clone(),
      guard.stepper(),
    );
    let (prepared, on_progress) = tokio::task::spawn_blocking(in_current_span(move || {
      let lock = Lock {
        common_dir,
        unknown_holder_timeout: timeout,
        canceled: &canceled,
      };
      let prepared = freeze(&git, &lock, table.as_ref(), robots, &prefix, &params, &stepper, &mut on_progress);
      (prepared, on_progress)
    }))
    .await?;
    let prepared = prepared?;
    guard.step(OperationStep::Pushing);
    let (git, stepper) = (self.git.clone(), guard.stepper());
    tokio::task::spawn_blocking(in_current_span(move || {
      let mut on_progress = on_progress;
      let mut told = |tick: TransferProgress| {
        stepper.progress(u32::from(tick.percent), tick.line.clone());
        on_progress(tick);
      };
      let sent = send(&git, prepared, OverMoved::Once, &mut told)?;
      let pushed_over = sent.over.map_or_default(|over| over.statuses);
      sent.prepared.outcome(&git, sent.answer, pushed_over)
    }))
    .await?
  }

  /// The user's robots for the checks of the push, which hold nothing up: one full list asked now
  /// (`PatronusService::robots_for_checks`), before the lock, as no network request runs under it; none off Space
  async fn robots(&self) -> Vec<PatronusRobot> {
    match &self.patronus {
      Some((service, space)) => service.robots_for_checks(space, &self.key.prefix).await,
      None => Vec::new(),
    }
  }
}

/// How the push takes the repository lock: the file of `common_dir`, waited for while its holder lives
struct Lock<'a> {
  common_dir: PathBuf,
  unknown_holder_timeout: Duration,
  canceled: &'a dyn Fn() -> bool,
}

/// The part of the app's push under the repository lock (no git process takes it): the checks on `table`, the table of
/// the last sync of the session, with `robots` (`check_push`), and the commits and leases of the push fixed
/// (`prepare_push_branches`, which refuses a branch the last sync did not build). While another holder has the lock,
/// `on_progress` hears whom it waits for (`TransferProgress::waiting_for`) and the view shows `WaitingForLock`. Blocking
#[expect(clippy::too_many_arguments, reason = "the lock, the facts of the checks, the push and its progress")]
fn freeze(
  git: &GitCommandExecutor,
  lock: &Lock<'_>,
  table: Option<&TableFacts>,
  robots: Vec<PatronusRobot>,
  branch_prefix: &str,
  params: &PushBranchesParams,
  stepper: &Stepper,
  on_progress: &mut dyn FnMut(TransferProgress),
) -> Result<PreparedPush> {
  let mut waited = false;
  let mut on_holder = |holder: &LockHolder| {
    if !waited {
      waited = true;
      stepper.step(OperationStep::WaitingForLock);
    }
    on_progress(TransferProgress::waiting_for(holder));
  };
  let held =
    lock_repository_while_held(&lock.common_dir, lock.unknown_holder_timeout, &mut on_holder, lock.canceled).map_err(|error| match error.downcast_ref::<RepositoryLocked>() {
      Some(locked) => anyhow!(locked_refusal(locked)),
      None => error,
    })?;
  if waited {
    stepper.step(OperationStep::Checking);
  }
  let table = table.ok_or_else(|| anyhow!(NOT_SYNCED_YET))?;
  let facts = guard_facts(git, &params.repository_path, table, robots);
  check_push(&facts, &params.branches).map_err(|refusal| anyhow!(refusal))?;
  let prepared = prepare_push_branches(git, branch_prefix, params, &facts.table().not_built)?;
  drop(held);
  Ok(prepared)
}

/// "Nothing was pushed: `bd sync` (pid 4242, for 3 s) holds the repository; try again when it is done"
fn locked_refusal(locked: &RepositoryLocked) -> String {
  let holder = locked.holder.as_ref().map_or_else(|| "another Branch Deck operation".to_string(), ToString::to_string);
  format!("Nothing was pushed: {holder} holds the repository; try again when it is done")
}

/// A push the session knows sends (`Sessions::begin_push`): its end goes to the session once (`tell_push`), also when
/// its task panics, as a push that failed
struct InSession<'a> {
  sessions: &'a Sessions,
  key: &'a SessionKey,
  /// `None` once the end was told
  subjects: Option<Vec<RemoteSubject>>,
}

impl<'a> InSession<'a> {
  fn begin(sessions: &'a Sessions, key: &'a SessionKey, subjects: Vec<RemoteSubject>) -> Self {
    drop(sessions.begin_push(key, subjects.clone()));
    Self {
      sessions,
      key,
      subjects: Some(subjects),
    }
  }

  /// The push ended with `outcome` (`None`: it failed): the remote statuses it left go into the table
  fn end(&mut self, outcome: Option<&PushBranchesOutcome>) -> Applied {
    let Some(subjects) = self.subjects.take() else {
      return Applied::none();
    };
    let statuses = outcome.map(|outcome| match outcome {
      PushBranchesOutcome::Pushed { remote_statuses, .. } | PushBranchesOutcome::RemoteMoved { remote_statuses, .. } => remote_statuses.clone(),
    });
    let pushed = matches!(outcome, Some(PushBranchesOutcome::Pushed { .. }));
    self.sessions.tell_push(self.key, subjects, statuses, pushed)
  }
}

impl Drop for InSession<'_> {
  fn drop(&mut self) {
    drop(self.end(None));
  }
}

/// The intent of the app's push of `branches`, parents first, as the registry admits it: the push of the last one, of
/// its whole stack for a stack branch; one branch alone, up to a commit or unapplied as the branch says, several with the
/// ones it is built on. The page decides which branches go until its push is an intent of its own (s9-12, s9-13)
fn intent_of(branches: &[StackPushBranch]) -> Push {
  let (subject, scope, expect_remote) = match branches {
    [branch] => {
      let scope = match &branch.tip {
        _ if branch.unapplied => PushScope::Unapplied,
        Some(commit) => PushScope::UpTo { commit: commit.clone() },
        None => PushScope::Alone,
      };
      (subject_of(branch), scope, branch.expected_remote.clone())
    }
    [.., last] => (subject_of(last), PushScope::WithAncestors, None),
    [] => (Subject::branch(""), PushScope::Alone, None),
  };
  Push {
    subject,
    scope,
    expect_remote,
    force: false,
    restart_running: false,
  }
}

fn subject_of(branch: &StackPushBranch) -> Subject {
  if branch.whole_stack {
    Subject::whole_stack(&branch.branch_name)
  } else {
    Subject::branch(&branch.branch_name)
  }
}

/// The answer of the operation of the push of `names` (the full names of its refs), the registry's: the push effect done,
/// failed as `lease_rejected` when a remote branch moved, or the error that stopped it
fn answer_of(guard: &RunningGuard, names: &[String], pushed: &Result<PushBranchesOutcome>, version: Option<domain_types::session::Version>) -> Answer {
  let status = match pushed {
    Ok(PushBranchesOutcome::Pushed { .. }) => EffectStatus::Done,
    Ok(PushBranchesOutcome::RemoteMoved { branch_name, .. }) => EffectStatus::Failed {
      error: ApiError::new(ErrorCode::LeaseRejected, format!("The remote branch of {branch_name} moved: nothing was pushed")),
    },
    Err(error) => return Err(operation_error(error)),
  };
  Ok(Outcome {
    op: guard.id().clone(),
    effects: vec![EffectRecord {
      effect: PlannedEffect::Push {
        branches: names.to_vec(),
        force: false,
      },
      status,
    }],
    news_key: names.last().map(|branch| NewsKey::Push { branch: branch.clone() }),
    version_after: version,
    stats: OperationStats::default(),
  })
}

/// The error that stopped an operation, as the registry keeps it: the code of a canceled wait or a held lock, else
/// internal; the command answers its own (`app_api::api_error`)
fn operation_error(error: &anyhow::Error) -> ApiError {
  if error.downcast_ref::<LockWaitCanceled>().is_some() {
    return ApiError::new(ErrorCode::Canceled, format!("{error:#}"));
  }
  if let Some(error) = error.downcast_ref::<ApiError>() {
    return error.clone();
  }
  ApiError::internal(format!("{error:#}"))
}

#[cfg(test)]
#[path = "push_test.rs"]
mod push_test;
