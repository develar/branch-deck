//! The push of the virtual branches, one core for the app and `bd` (docs/design/git-operations.md, "Push and land";
//! docs/design/rust-domain-authority.md, "Конкурентность"): the repository lock is held only while the push is decided
//! and its commits and leases are fixed (`PreparedPush`: `<commit>:refs/heads/<name>`, each leased on the remote head
//! the user saw), never across the network. After it, `send` pushes exactly those commits in one push, and a remote
//! branch that moved meanwhile without commits a branch lacks takes the push once more over its new head, the same
//! commits (D4, `PreparedPush::over_moved`); a robot the push restarts is canceled before it and restarted on the pushed
//! commit after it (`cancel_robots`, `restart_robots`).
//!
//! The push of a branch alone and of an unapplied branch is the intent `Push` (`prepare_push`, `execute_push`): its plan
//! (`push_planned`) retargets the pull request of the branch out of a virtual branch first (D3a), cancels the Safe Push
//! of the branch it restarts (`restart_running`), pushes while the branch has work for the remote, asks first for a
//! push that drops remote commits the branch lacks (`Confirmation::ForcePushDrops`, unless the intent forces it) and is
//! refused by what keeps the branch out of a push (parked, held or landed by a Safe Push, not built); its run is the
//! run of a start (`land_execute::run_planned`): admitted per ref, after the sync of the session that runs, the robots
//! asked now (none for an unapplied branch, C3), the witness, the lock only for the freeze, then the effects.
//!
//! The app's push of a stack (`push_branches` of both hosts, until s9-13) is an operation of its session (`PushHost`):
//! admitted per ref it writes (`Resource::Ref`), so a push of a ref another push of the process writes waits for it and
//! the pushes of other refs run side by side, and for a fetch of its refs that runs (`ref_claims`); it waits for a sync
//! of the session that runs (`WaitingForSync`), reads the user's robots, decides under the lock
//! (`push_guard::check_push`) and freezes (`prepare_push_branches`), pushes after it with its progress told to its
//! caller, and its end goes to the session (`Sessions::begin_push`, `tell_push`: the remote statuses it left). It runs
//! in a task of its own, so a caller that goes away leaves it running to its end, its refs held in the registry until
//! its `git push` ended. `bd push` decides its refs on the sync it runs under the lock (the named exception of the lock
//! span, until s11) and freezes them on the tips that sync left (`PreparedPush::leased`), then sends them the same way.

use crate::engine::{run, run_planned_in};
use crate::events::EventSink;
use crate::land_execute::{Accepted, Planned, PushedRef, refusal_of};
use crate::land_session::push_start;
use crate::registry::{Admitted, Answer, OperationRequest, Registry, RunningGuard, Stepper};
use crate::witness::Witness;
use anyhow::{Result, anyhow};
use domain_policy::pull_request::base_of;
use domain_policy::push::{PushState, Verdict, verdict};
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Confirmation, PlannedEffect, Subject};
use domain_types::operation::{
  EffectRecord, EffectStatus, Fingerprint, Intent, NewsKey, OperationResult, OperationStats, OperationStep, Outcome, Plan, PullRequestField, Push, PushScope, Resource,
};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestBase, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::PatronusView;
use domain_types::session::{SyncReason, SyncState, ViewAnswer};
use domain_types::{PushAction, RemoteStatusUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{LockHolder, LockWaitCanceled, RepositoryLocked, lock_repository_while_held};
use patronus::service::PatronusService;
use remote_facts::service_sources::ServiceHost;
use repository_model::derive::{Holds, LandSources, alone_push_of, with_pull_request_sources};
use repository_model::facts::{FactStore, TableFacts};
use repository_model::push_guard::{check_push, guard_facts};
use repository_model::ref_claims::claim_refs;
use repository_model::session::{Applied, SessionKey, Sessions};
use repository_model::table::{RemoteSubject, TableState};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use sync_core::push::{LeasedPush, NOT_SYNCED_YET, PUSH_REMOTE, PreparedPush, PushBranchesOutcome, PushBranchesParams, StackPushBranch, prepare_push_branches, push_ref_names};
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
  /// adapter of `push_branches` answers a push outcome, not the joined one's, so it waits for that push to end and is
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
    // the sync that brings the tips into the table runs: the push checks what it builds
    wait_for_running_sync(sessions, &self.key, &guard.stepper()).await;
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
    result: OperationResult::None,
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

/// The plan of the push `push` in the open session of `key` over the facts it holds now (`push_planned`), the holds of
/// its Patronus view; `None` for a session that is not open or has no table yet, a branch its table lacks, an unapplied
/// branch it doesn't list, and for the pushes of a stack, which run through `push_branches` until they are intents of
/// their own (s9-13)
pub async fn prepare_push(sessions: &Sessions, key: &SessionKey, push: &Push) -> Option<Planned> {
  let (prefix, push) = (key.prefix.clone(), push.clone());
  sessions
    .read(key, move |state, facts| push_planned(state, facts, &prefix, &push, None, ExpectRemote::OfIntent))
    .await
    .flatten()
}

/// The remote head a push leases on and is witnessed with
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectRemote {
  /// The one the user saw (`Push::expect_remote`)
  OfIntent,
  /// The one the session has now: the push waited for a sync or another push, which brought what the remote has, and
  /// goes over it while it drops nothing the user didn't accept
  OfSession,
}

/// The robots a push is checked on, asked now, and the Patronus view of the table of the last sync derived of them
pub struct CheckedHolds {
  pub view: PatronusView,
  pub robots: Vec<PatronusRobot>,
}

impl CheckedHolds {
  const fn holds(&self) -> Holds<'_> {
    Holds {
      view: &self.view,
      robots: self.robots.as_slice(),
    }
  }
}

/// The plan of the push `push` over the table of `state` and the facts of `facts`, with what its run needs: of a branch
/// alone (its row, its preview, its review window, the toast of a remote that moved) or of an unapplied branch; `checked`
/// the holds a run of it checks (the robots asked for it now), else those of the Patronus view of the session. `None` as
/// `prepare_push`
#[must_use]
pub fn push_planned(state: &TableState, facts: &FactStore, prefix: &str, push: &Push, checked: Option<&CheckedHolds>, expect: ExpectRemote) -> Option<Planned> {
  let Subject::Branch { name } = &push.subject else {
    return None;
  };
  match push.scope {
    PushScope::Alone => with_pull_request_sources(state, facts, prefix, |sources| {
      let holds = checked.map_or(
        Holds {
          view: sources.view,
          robots: sources.robots,
        },
        CheckedHolds::holds,
      );
      alone_planned(state, facts, sources, holds, name, push, expect)
    })
    .flatten(),
    PushScope::Unapplied => unapplied_planned(state, name, push, expect),
    PushScope::WithAncestors | PushScope::WithChildren | PushScope::UpTo { .. } => None,
  }
}

/// The push of the branch `name` alone over `sources` (`derive::alone_push_of`): the retarget of its open pull request
/// that goes into a virtual branch (D3a), the cancel of its Safe Push while it runs and the push restarts it, the push
/// while the branch has work for the remote, then the restart (`restart_running`). A push that drops remote commits the
/// branch lacks asks first (`Confirmation::ForcePushDrops`) unless the intent forces it; a branch that stays local, one a
/// running Safe Push holds (or a merged one landed whole) and one the last sync did not build refuse it
fn alone_planned(state: &TableState, facts: &FactStore, sources: &LandSources<'_>, holds: Holds<'_>, name: &str, push: &Push, expect: ExpectRemote) -> Option<Planned> {
  let full = |branch: &str| to_final_branch_name(sources.prefix, branch).ok();
  let full_name = full(name)?;
  let info = sources.table.branch(name)?;
  // the Safe Push of the branch the push restarts on its commit, canceled first while it runs
  let robot = if push.restart_running {
    let id = holds.view.branches.get(name).and_then(|facts| facts.robot_id.as_deref());
    id.and_then(|id| holds.robots.iter().find(|robot| robot.id == id))
  } else {
    None
  };
  // one canceling already (the cancel of a restart whose push the remote refused) is canceled once
  let running = robot.filter(|robot| robot.status == PatronusRobotStatus::Running);
  let alone = alone_push_of(state, facts, holds, name, running.map(|robot| robot.id.as_str()))?;
  let goes = matches!(alone.verdict, Verdict::Goes | Verdict::AsksFirst);
  let mut effects = Vec::new();
  // its pull request goes into the target first, where every pull request of a virtual branch goes (D3a)
  let retarget = sources
    .pull_requests
    .pull_requests
    .get(name)
    .filter(|pull_request| goes && !sources.target.is_empty() && pull_request.state == PullRequestState::Open)
    .filter(|pull_request| matches!(base_of(pull_request, sources.target, sources.prefix), PullRequestBase::Virtual { .. }));
  if let Some(pull_request) = retarget {
    effects.push(PlannedEffect::Retarget {
      branch: name.to_string(),
      number: pull_request.number,
      from: pull_request.base_ref_name.clone(),
      to: sources.target.to_string(),
    });
  }
  if let Some(robot) = running {
    effects.push(PlannedEffect::CancelRobot { robot_id: robot.id.clone() });
  }
  let mut pushed = Vec::new();
  if goes {
    effects.push(PlannedEffect::Push {
      branches: vec![name.to_string()],
      force: alone.verdict == Verdict::AsksFirst,
    });
    pushed.push(PushedRef {
      name: name.to_string(),
      full_name: full_name.clone(),
      parent_ref: info.stack.as_ref().and_then(|stack| stack.effective_base.as_deref()).and_then(full),
      whole_stack: false,
      unapplied: false,
    });
  }
  if let Some(robot) = robot {
    effects.push(PlannedEffect::RestartRobot { robot_id: robot.id.clone() });
  }
  let tip = sources.table.tips.get(&full_name).cloned();
  let expect_remote = match expect {
    ExpectRemote::OfIntent => push.expect_remote.clone(),
    ExpectRemote::OfSession => sources.remote.branches.get(name).and_then(|remote| remote.remote_tip.clone()),
  };
  let plan = PushPlan {
    push,
    expect_remote,
    effects,
    blockers: alone.blocker.into_iter().collect(),
    drops: alone.drops,
    pushed,
    tip,
    retargeted: retarget,
    robot,
  };
  Some(plan.planned(&full_name, name))
}

/// The push of the unapplied branch `name` (the branch it was) under its own name, so the remote keeps a copy: nothing
/// refuses it, no robot is read for it (C3), and its pull request stays as it was; it asks first only when it drops
/// remote commits and the intent doesn't force it
fn unapplied_planned(state: &TableState, name: &str, push: &Push, expect: ExpectRemote) -> Option<Planned> {
  let branch = state.sections().unapplied.iter().find(|branch| branch.branch == name)?;
  // no remote status: the remote has no such branch, the push creates it
  let action = branch.remote.as_ref().map_or(PushAction::Create, |status| status.action);
  let verdict = verdict(&PushState { action: Some(action), kept: None });
  let goes = matches!(verdict, Verdict::Goes | Verdict::AsksFirst);
  let pushed = goes
    .then(|| PushedRef {
      name: name.to_string(),
      full_name: branch.name.clone(),
      parent_ref: None,
      whole_stack: false,
      unapplied: true,
    })
    .into_iter()
    .collect();
  let expect_remote = match expect {
    ExpectRemote::OfIntent => push.expect_remote.clone(),
    ExpectRemote::OfSession => branch.remote.as_ref().and_then(|status| status.remote_tip.clone()),
  };
  let plan = PushPlan {
    push,
    expect_remote,
    effects: goes
      .then(|| PlannedEffect::Push {
        branches: vec![name.to_string()],
        force: verdict == Verdict::AsksFirst,
      })
      .into_iter()
      .collect(),
    blockers: Vec::new(),
    drops: if verdict == Verdict::AsksFirst {
      branch.remote.as_ref().map_or(0, |status| status.commits_behind)
    } else {
      0
    },
    pushed,
    tip: Some(branch.tip.clone()),
    retargeted: None,
    robot: None,
  };
  Some(plan.planned(&branch.name, name))
}

/// What the plan of a push is made of, before its witness
struct PushPlan<'a> {
  push: &'a Push,
  /// The remote head it leases on (`ExpectRemote`)
  expect_remote: Option<String>,
  effects: Vec<PlannedEffect>,
  blockers: Vec<Reason>,
  /// The remote commits the push drops while it asks first
  drops: u32,
  pushed: Vec<PushedRef>,
  /// The tip of the branch as the table has it
  tip: Option<String>,
  retargeted: Option<&'a PullRequest>,
  /// The robot it restarts
  robot: Option<&'a PatronusRobot>,
}

impl PushPlan<'_> {
  /// The plan of the branch `name` (`full_name` on the remote) with its witness: the tip of the branch and the remote
  /// head it leases on (`expect_remote`) while it pushes (none of them else: a plan that moves and leases no ref takes
  /// no lock), the pull request it retargets and the robot it restarts
  fn planned(self, full_name: &str, name: &str) -> Planned {
    let force = self.effects.iter().any(|effect| matches!(effect, PlannedEffect::Push { force: true, .. }));
    let confirmations = if force && !self.push.force {
      vec![Confirmation::ForcePushDrops {
        branch: name.to_string(),
        commits: self.drops,
      }]
    } else {
      Vec::new()
    };
    let refs = if self.pushed.is_empty() {
      BTreeMap::new()
    } else {
      BTreeMap::from([
        (format!("refs/heads/{full_name}"), self.tip),
        (format!("refs/remotes/{PUSH_REMOTE}/{full_name}"), self.expect_remote),
      ])
    };
    let witness = Witness::of(refs, self.retargeted, self.robot);
    let mut plan = Plan {
      intent: Intent::Push(self.push.clone()),
      fingerprint: Fingerprint(String::new()),
      lands: Vec::new(),
      effects: self.effects,
      confirmations,
      blockers: self.blockers,
      waits: Vec::new(),
      alternatives: None,
      pull_request: None,
      approvals_skipped_by: None,
    };
    plan.fingerprint = witness.fingerprint(&plan);
    Planned {
      plan,
      witness,
      pushes: self.pushed,
      source_ref: format!("refs/heads/{full_name}"),
      merge_request: None,
      notes: Vec::new(),
      subject: self.push.subject.clone(),
      mode: self.robot.map_or(PatronusPushMode::RebaseAutosquash, |robot| robot.push_mode),
    }
  }
}

/// What the push `push` asks the registry for: the ref of its branch, whatever its plan pushes now (a sync it waits for
/// may give it work), the robot it cancels and restarts and the base of the pull request it retargets (of `planned`, its
/// plan as the session has it now)
fn push_request(prefix: &str, push: &Push, planned: Option<&Planned>) -> OperationRequest {
  let name = push.subject.name();
  let full_name = match push.scope {
    PushScope::Unapplied => qualified_branch_name(prefix, BranchKind::Unapplied, name),
    _ => to_final_branch_name(prefix, name),
  };
  let mut resources: Vec<Resource> = full_name.ok().map(|name| Resource::Ref { name }).into_iter().collect();
  for effect in planned.map_or(&[][..], |planned| planned.plan.effects.as_slice()) {
    match effect {
      PlannedEffect::Retarget { number, .. } => resources.push(Resource::PullRequest {
        number: *number,
        field: PullRequestField::Base,
      }),
      PlannedEffect::CancelRobot { robot_id } | PlannedEffect::RestartRobot { robot_id } => {
        let robot = Resource::Robot { id: robot_id.clone() };
        if !resources.contains(&robot) {
          resources.push(robot);
        }
      }
      _ => {}
    }
  }
  OperationRequest {
    intent: Intent::Push(push.clone()),
    resources,
    step: OperationStep::Checking,
    moves: Vec::new(),
  }
}

/// The push has no plan in the session: "Nothing was pushed: the last sync has no auth"
fn no_push_plan(push: &Push) -> ApiError {
  match push.scope {
    PushScope::Unapplied => ApiError::new(
      ErrorCode::UnknownBranch,
      format!("Nothing was pushed: no unapplied branch {} is listed", push.subject.name()),
    ),
    PushScope::Alone => ApiError::new(ErrorCode::UnknownBranch, format!("Nothing was pushed: the last sync has no {}", push.subject.words())),
    PushScope::WithAncestors | PushScope::WithChildren | PushScope::UpTo { .. } => not_an_intent_yet(),
  }
}

/// The pushes of a stack run through `push_branches` until they are intents of their own (s9-13)
#[must_use]
pub fn not_an_intent_yet() -> ApiError {
  ApiError::new(
    ErrorCode::InvalidArguments,
    "A push of a stack runs through push_branches, not through prepare and execute yet",
  )
}

/// Runs the push `push` in the session of `key` on the plan its caller accepted: admitted over the operations of the
/// session per ref (the same push joins it, another push of the ref waits for it), after the sync of the session that
/// runs, if one does; then planned again and run as a start runs (`land_execute::run_planned`: the witness, the lock only
/// for the full witness and the freeze, the retarget, the cancel, the push of the frozen commit with its lease, once more
/// over a remote that moved without commits the branch lacks, then the restart) over the services of `services`
/// (`push_start`: neither Patronus nor GitHub needed), its steps in the view and its progress and outcome to `events`.
/// After a wait (the sync, the operations it queued behind) the push is decided again on what they left, over the
/// remote head the session has now (`ExpectRemote::OfSession`), and goes while the user accepted what it asks: the click
/// asked for this push, and a remote that brought commits the branch lacks asks first again
///
/// # Errors
/// What refuses it before its first effect, a branch the session has no plan of, a session that closed, a push of a
/// stack
#[instrument(skip_all, fields(subject = %push.subject.name()))]
pub async fn execute_push(registry: &Registry, key: &SessionKey, push: Push, accepted: Accepted, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Answer {
  if !matches!(push.scope, PushScope::Alone | PushScope::Unapplied) {
    return Err(not_an_intent_yet());
  }
  let sessions = registry.sessions();
  let planned = prepare_push(sessions, key, &push).await;
  let request = push_request(&key.prefix, &push, planned.as_ref());
  run(registry, key, request, events, async |guard| {
    let began = std::time::Instant::now();
    let synced = wait_for_running_sync(sessions, key, &guard.stepper()).await;
    let waited = synced || guard.queued();
    let checked = checked_holds(services, sessions, key, &push).await.map(Arc::new);
    let (prefix, intent, holds) = (key.prefix.clone(), push.clone(), checked.clone());
    let expect = if waited { ExpectRemote::OfSession } else { ExpectRemote::OfIntent };
    let planned = sessions
      .read(key, move |state, facts| push_planned(state, facts, &prefix, &intent, holds.as_deref(), expect))
      .await
      .flatten()
      .ok_or_else(|| no_push_plan(&push))?;
    // what refuses it, in the words of the robots it was checked on
    if let Some(refusal) = refusal_of(&planned, checked.as_ref().map_or(&[][..], |checked| checked.robots.as_slice())) {
      return Err(refusal);
    }
    let accepted = if waited {
      Accepted {
        fingerprint: planned.plan.fingerprint.clone(),
        confirmations: accepted.confirmations.clone(),
      }
    } else {
      accepted.clone()
    };
    run_planned_in(guard, push_start(services, sessions, key).await?, planned, &accepted, began).await
  })
  .await
}

/// The holds the run of `push` checks: the user's robots asked now, one full list (`PatronusService::robots_for_checks`,
/// which a robot started meanwhile in the IDE or by `bd` is in), and the Patronus view of the table of the last sync of
/// the session derived of them (`push_guard::guard_facts`); none for an unapplied branch, which nothing holds (C3), and
/// off Space or without a token, where no robot runs, nor before the first sync
async fn checked_holds(services: &dyn ServiceHost, sessions: &Sessions, key: &SessionKey, push: &Push) -> Option<CheckedHolds> {
  if push.scope == PushScope::Unapplied {
    return None;
  }
  let space = services.space_repository(key).await.ok().flatten()?;
  let patronus = services.patronus(key).ok()?;
  let robots = patronus.robots_for_checks(&space.name, &key.prefix).await;
  let table = sessions.table(key).await?;
  let (git, path) = (services.git(), key.worktree.clone());
  let facts = tokio::task::spawn_blocking(in_current_span(move || guard_facts(&git, &path, &table, robots))).await.ok()?;
  Some(CheckedHolds {
    view: facts.view,
    robots: facts.robots,
  })
}

/// Waits for the sync of the session of `key` that runs, if one does (a rewrite's, a focus'), with the step
/// `WaitingForSync`: it brings the tips into the table the push is checked on; a sync asked after it is not waited for.
/// Whether it waited
async fn wait_for_running_sync(sessions: &Sessions, key: &SessionKey, stepper: &Stepper) -> bool {
  if sessions.apply_with(key, |state| matches!(state.sync(), SyncState::Running { .. })).await != Some(true) {
    return false;
  }
  stepper.step(OperationStep::WaitingForSync);
  if let Err(error) = sessions.sync(key, SyncReason::Open).wait().await {
    debug!(error = %error.message, "the push checks the table of the last successful sync");
  }
  stepper.step(OperationStep::Checking);
  true
}

#[cfg(test)]
#[path = "push_test.rs"]
mod push_test;
