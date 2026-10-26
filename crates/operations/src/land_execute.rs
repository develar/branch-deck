//! The run of a start of a Safe Push or a Dry Run (`Intent::Land`; docs/design/rust-domain-authority.md, "Инварианты
//! планов"; docs/design/git-operations.md, "Push and land"), one for the app and `bd`: the plan is prepared again from
//! the facts now (`StartHost::plan`) and runs only on the fingerprint its caller accepted, with every confirmation of it
//! accepted; then, in this order:
//!
//! 1. the cheap witness, before the lock: the tips of the refs the plan moves and leases, one `for-each-ref`;
//! 2. the repository lock, the full witness and the freeze of the commits and the leases of the push under it (no HTTP
//!    and no other git), then the lock is released;
//! 3. `Retarget`: a pull request into a virtual branch goes into the target; GitHub refusing the one the robot starts
//!    from stops the start before anything is canceled or pushed, one of a branch the push takes along is only told;
//! 4. `CancelRobot`: the running Safe Push the start replaces (one that finished meanwhile is `AlreadyFinished`, done);
//! 5. the push of the frozen commits with their leases (`push::send`, once more over a remote that moved without commits
//!    a branch lacks), its end into the session;
//! 6. the late cheap witness: a pushed branch that moved since the push (a rewrite after the lock) skips the start as
//!    `plan_changed`, and the outcome tells what was done before;
//! 7. the robots and the pull requests of the facts as they are now (no request): a robot that lands a part of the
//!    subject started meanwhile, or the pull request of a comment that closed, skips the start as `plan_changed`;
//! 8. the start: through the API, or as a `/patronus` comment with the reply of the bot read on the clock of the session
//!    (`robot_start`); the robot it started goes into the facts.
//!
//! A refusal the facts know (a blocker, an unaccepted confirmation, a changed fingerprint, a held lock) does nothing:
//! every effect comes after the full witness. The first effect that fails stops the start; the effects before it stay
//! done, and the outcome lists each with its status.

use crate::push::{OverMoved, Sent, send};
use crate::robot_start::{ApiStart, CommentStart, Commented, patronus_failure, start_by_comment, start_through_api};
use crate::witness::{Witness, check, read_refs};
use domain_policy::holds::is_active;

use domain_types::error::{ApiError, ErrorCode, ErrorDetails};
use domain_types::landing::{Confirmation, Land, LandingRefusal, PlannedEffect, StartRoute, Subject};
use domain_types::operation::{
  EffectRecord, EffectStatus, Fingerprint, LandResult, NewsKey, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Plan, StartComment, StartNote,
};
use domain_types::patronus::{PatronusMergeRequest, PatronusRobot};
use domain_types::pull_request::CreatedPullRequest;
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::session::Version;
use domain_types::stack::StackBranchState;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use history_rewrite::repository_lock::{LockHolder, RepositoryLock, RepositoryLocked, lock_repository_in};
use patronus::service::PatronusService;
use remote_facts::fact_sources::Read;
use remote_facts::pull_request_create::{CreatePullRequestParams, create_pull_request};
use remote_facts::pull_requests::GitHubWriteError;
use repository_model::ref_claims::claim_refs;
use repository_model::start_guard::start_refusal_text;
use repository_model::table::RemoteSubject;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use sync_core::github_repository::GitHubRepository;
use sync_core::push::{LeasedBranch, LeasedPush, PreparedPush, PushTarget};
use sync_core::push_lease::{LeaseSource, PushOptions, PushRef};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::clock::Clock;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};

/// A plan of a start as the facts have it now, with what the run needs of it besides: the witness it was decided on,
/// the branches its push sends, the ref the robot starts from and the merge request whose approvals count
#[derive(Debug, Clone)]
pub struct Planned {
  pub plan: Plan,
  /// The tips of its refs as the table has them, its pull requests and robots (`land::witness_of`)
  pub witness: Witness,
  /// The branches of its `Push`, root first, as they go
  pub pushes: Vec<PushedRef>,
  /// `refs/heads/{prefix}/virtual/<name>`, or `refs/heads/{prefix}/stack/<root>` of a whole stack that forks
  pub source_ref: String,
  /// The merge request the pre-check of the review named (a start through the API counts its approvals)
  pub merge_request: Option<PatronusMergeRequest>,
  /// What the checks of the host before the plan could not do in full (the outcome tells them)
  pub notes: Vec<StartNote>,
}

/// A branch a start pushes, before its commit and lease are frozen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedRef {
  /// The virtual branch ("auth"), or the root of a whole stack
  pub name: String,
  /// The branch locally and on the remote, without `refs/heads/`
  pub full_name: String,
  /// `{prefix}/virtual/<parent>` of a stacked branch: the commits up to it are the parent's
  pub parent_ref: Option<String>,
  /// The stack branch of a whole stack
  pub whole_stack: bool,
}

impl PushedRef {
  fn subject(&self) -> RemoteSubject {
    if self.whole_stack {
      RemoteSubject::WholeStack(self.name.clone())
    } else {
      RemoteSubject::Branch(self.name.clone())
    }
  }
}

/// The robots and the pull requests the facts of a start hold now, as the host read them last: no request
#[derive(Debug, Clone, Default)]
pub struct Current {
  pub robots: Vec<PatronusRobot>,
  pub pull_requests: Vec<PullRequest>,
}

/// Where the facts of a start live: the session of the app (and of the E2E server), or the one-shot session of `bd`
pub trait StartHost: Send + Sync {
  /// The plan of `land` decided on the facts now; `None` for a subject the table lacks
  fn plan<'a>(&'a self, land: &'a Land) -> Read<'a, Result<Option<Planned>, ApiError>>;

  /// The robots and the pull requests the facts hold now, without a request
  fn current(&self) -> Read<'_, Current>;

  /// The user's robots started since `since` (Unix seconds), asked now: the robot a comment started
  fn robots_since(&self, since: u32) -> Read<'_, Result<Vec<PatronusRobot>, ApiError>>;

  /// The push of `subjects` starts: from now on a sync shows no remote status of them
  fn begin_push(&self, subjects: Vec<RemoteSubject>);

  /// The push that `begin_push` began ended as `sent` says (`None`: it failed before the remote answered): the remote
  /// statuses it left go into the table. The version of the view after its patch
  fn end_push<'a>(&'a self, subjects: Vec<RemoteSubject>, sent: Option<&'a Sent>) -> Read<'a, Option<Version>>;

  /// The pull request a start opens (`PlannedEffect::CreatePullRequest`): what it says and who reviews it, after the
  /// push `sent` (its text names what the push sent)
  fn pull_request_text<'a>(&'a self, sent: Option<&'a Sent>) -> Read<'a, Result<PullRequestText, ApiError>>;

  /// The robot the start started or canceled goes into the facts: the version of the view after its patch
  fn write_robot(&self, robot: PatronusRobot) -> Read<'_, Option<Version>>;
}

/// The pull request a start opens, as its host words it
#[derive(Debug, Clone)]
pub struct PullRequestText {
  /// The branch it goes from, without `refs/heads/`
  pub head: String,
  pub base: String,
  pub title: String,
  pub body: String,
  pub reviewers: Vec<String>,
  pub assign_self: bool,
}

/// The repository lock of a start, blocking: `bd` waits a bounded time (`locked`), the app as long as the holder lives
/// and until the user stops the start (`canceled`, the second argument); `on_wait` (the first) is called while another
/// holds it, with the holder each time it is read again (`None`: a wait that names nobody)
pub type Locker = Arc<dyn Fn(&dyn Fn(Option<&LockHolder>), &dyn Fn() -> bool) -> Result<RepositoryLock, ApiError> + Send + Sync>;

/// The lock of `git_common_dir` waited for at most `timeout` (`bd`): another holder refuses the start as `locked`
#[must_use]
pub fn bounded_lock(git_common_dir: std::path::PathBuf, timeout: std::time::Duration) -> Locker {
  Arc::new(move |on_wait, _canceled| {
    lock_repository_in(&git_common_dir, timeout, &|| on_wait(None)).map_err(|error| match error.downcast_ref::<RepositoryLocked>() {
      Some(locked) => ApiError::new(ErrorCode::Locked, locked.to_string()),
      None => ApiError::internal(format!("{error:#}")),
    })
  })
}

/// What a start runs on besides its facts
#[derive(Clone)]
pub struct StartServices {
  pub git: GitCommandExecutor,
  pub repository_path: String,
  /// `$GIT_COMMON_DIR` of the repository: the refs a fetch of this process writes are claimed there (`ref_claims`)
  pub git_common_dir: std::path::PathBuf,
  pub prefix: String,
  pub patronus: Arc<PatronusService>,
  /// The Space repository the robots run in
  pub space: String,
  /// The GitHub service and the repository the branches are mirrored to; `None` without one
  pub github: Option<(Arc<GitHubService>, GitHubRepository)>,
  /// The branch of origin the robot pushes into ("master")
  pub target: String,
  pub push: PushSettings,
  pub lock: Locker,
  /// The clock of the session, which the reply of a comment is read on
  pub clock: Arc<dyn Clock>,
  /// The robot of a comment is looked for until the reply wait ends (the app), or once after the bot accepted (`bd`)
  pub wait_for_robot: bool,
}

/// Where the push of a start goes and what its remote statuses compare with
#[derive(Debug, Clone)]
pub struct PushSettings {
  pub remote: String,
  /// "origin/master"
  pub baseline: String,
  pub my_email: Option<String>,
  /// `branchdeck.pushSizeLimit`
  pub size_limit: Option<u64>,
}

/// Where the caller hears a start: its steps, the progress of its push; and whether the user stopped it
#[derive(Clone)]
pub struct Steps {
  step: Arc<dyn Fn(OperationStep) + Send + Sync>,
  progress: Arc<dyn Fn(&TransferProgress) + Send + Sync>,
  canceled: Arc<AtomicBool>,
}

impl Steps {
  /// Steps nobody hears, never canceled: `bd`
  #[must_use]
  pub fn none() -> Self {
    Self::new(Arc::new(|_| {}), Arc::new(|_| {}), Arc::new(AtomicBool::new(false)))
  }

  #[must_use]
  pub fn new(step: Arc<dyn Fn(OperationStep) + Send + Sync>, progress: Arc<dyn Fn(&TransferProgress) + Send + Sync>, canceled: Arc<AtomicBool>) -> Self {
    Self { step, progress, canceled }
  }

  fn step(&self, step: OperationStep) {
    (self.step)(step);
  }

  fn canceled(&self) -> bool {
    self.canceled.load(Ordering::SeqCst)
  }
}

/// What the caller of a start accepted: the fingerprint of the plan it showed and the confirmations of it the user
/// accepted
#[derive(Debug, Clone)]
pub struct Accepted {
  pub fingerprint: Fingerprint,
  pub confirmations: Vec<Confirmation>,
}

/// How a start ran: its plan as it ran, its outcome (each effect with its status), and what the effects answered
#[derive(Debug)]
pub struct Started {
  pub plan: Plan,
  pub outcome: Outcome,
  /// The answer of each retarget of the plan, in its order (a branch the push takes along whose pull request GitHub
  /// refused: a warning)
  pub retargets: Vec<Result<(), GitHubWriteError>>,
  /// The robot it canceled, as Patronus answered (or as listed when it had finished meanwhile)
  pub canceled: Option<PatronusRobot>,
  /// Its push, as it went
  pub pushed: Option<Sent>,
  /// The robot it started through the API, or the one a comment started once listed
  pub robot: Option<PatronusRobot>,
  pub commented: Option<Commented>,
  /// The pull request it opened (or found open)
  pub created: Option<CreatedPullRequest>,
}

/// Runs the start `land` as `op` on the plan its caller accepted (`fingerprint`, with `accepted` among the
/// confirmations): prepared again from the facts of `host`, then the witness, the lock and the freeze, then its effects
/// in the order of the plan (see the module)
///
/// # Errors
/// Before any effect: a subject the table lacks, a fingerprint or a confirmation the plan no longer has
/// (`plan_changed`), what refuses it (the code of its first blocker), a fact it still waits for, a lock another holds
/// (`locked`), git failing. After the first effect a failure is the status of its effect in the outcome
#[instrument(skip_all, fields(op = %op, subject = %land.subject.name()))]
pub async fn execute(host: &dyn StartHost, services: &StartServices, steps: &Steps, op: OperationId, land: &Land, accepted: &Accepted) -> Result<Started, ApiError> {
  let began = std::time::Instant::now();
  steps.step(OperationStep::Checking);
  let planned = host.plan(land).await?.ok_or_else(|| unknown_subject(&land.subject))?;
  // what refuses it now is told as itself, a refusal the check against the newest target found too
  if let Some(blocker) = planned.plan.blockers.first() {
    return Err(refusal(blocker, &planned.plan.blockers, &land.subject, &host.current().await.robots));
  }
  check(&accepted.fingerprint, &planned.plan.fingerprint)?;
  if planned.plan.confirmations.iter().any(|confirmation| !accepted.confirmations.contains(confirmation)) {
    return Err(plan_changed("the start asks the user to accept more than was accepted"));
  }
  if !planned.plan.waits.is_empty() {
    return Err(ApiError::new(ErrorCode::PlanChanged, "The start still waits for facts nobody answered; prepare it again"));
  }
  // the cheap witness: a stale plan goes without a wait for the lock
  let names: Vec<String> = planned.witness.refs.keys().cloned().collect();
  let (git, path) = (services.git.clone(), services.repository_path.clone());
  let cheap = blocking(move || read_refs(&git, &path, &names).map_err(git_failure)).await?;
  witnessed(&planned.witness, &cheap)?;
  let frozen = frozen_under_lock(services, steps, &planned.witness).await?;
  witnessed(&planned.witness, &frozen)?;
  let prepared = freeze(services, &planned.pushes, &frozen)?;
  debug!(effects = planned.plan.effects.len(), "the witness holds: the effects run");
  // the robots the facts held when the plan ran: one listed later that lands a part of it started meanwhile
  let known: HashSet<String> = host.current().await.robots.into_iter().map(|robot| robot.id).collect();
  let mut run = Run::new(host, services, steps, planned, known);
  run.effects(prepared).await;
  Ok(run.started(op, began))
}

/// The full witness and the freeze under the repository lock: the tips of the refs of the plan, one `for-each-ref`, then
/// the lock is released
async fn frozen_under_lock(services: &StartServices, steps: &Steps, witness: &Witness) -> Result<BTreeMap<String, Option<String>>, ApiError> {
  let names: Vec<String> = witness.refs.keys().cloned().collect();
  let (git, path, lock, steps) = (services.git.clone(), services.repository_path.clone(), Arc::clone(&services.lock), steps.clone());
  blocking(move || {
    let waited = AtomicBool::new(false);
    // the step once, and whom it waits for each time the holder is read (the slot of the caller names it)
    let on_wait = |holder: Option<&LockHolder>| {
      if !waited.swap(true, Ordering::SeqCst) {
        steps.step(OperationStep::WaitingForLock);
      }
      if let Some(holder) = holder {
        (steps.progress)(&TransferProgress::waiting_for(holder));
      }
    };
    let canceled = || steps.canceled();
    let held = lock(&on_wait, &canceled)?;
    if waited.load(Ordering::SeqCst) {
      steps.step(OperationStep::Checking);
    }
    let refs = read_refs(&git, &path, &names).map_err(git_failure);
    drop(held);
    refs
  })
  .await
}

/// The witness holds when the refs read now are the ones the plan was decided on
fn witnessed(witness: &Witness, read: &BTreeMap<String, Option<String>>) -> Result<(), ApiError> {
  if witness.refs == *read {
    return Ok(());
  }
  let moved: Vec<&str> = read
    .iter()
    .filter(|(name, tip)| witness.refs.get(*name) != Some(tip))
    .map(|(name, _)| name.as_str())
    .collect();
  debug!(?moved, "a ref of the plan moved before its first effect");
  Err(plan_changed(&format!("{} moved since the plan", moved.join(", "))))
}

/// The push of the plan on the frozen tips and leases: one atomic push of every branch it sends; `None` without one
fn freeze(services: &StartServices, pushes: &[PushedRef], frozen: &BTreeMap<String, Option<String>>) -> Result<Option<PreparedPush>, ApiError> {
  if pushes.is_empty() {
    return Ok(None);
  }
  let mut branches = Vec::with_capacity(pushes.len());
  for pushed in pushes {
    let tip = frozen.get(&format!("refs/heads/{}", pushed.full_name)).cloned().flatten().ok_or_else(|| {
      ApiError::new(
        ErrorCode::BranchNotBuilt,
        format!("Nothing was pushed or started: {} has no tip; sync, then start again", pushed.full_name),
      )
    })?;
    let lease = frozen.get(&format!("refs/remotes/{}/{}", services.push.remote, pushed.full_name)).cloned().flatten();
    let source = if lease.is_some() { LeaseSource::Expected } else { LeaseSource::Absent };
    branches.push(LeasedBranch {
      name: pushed.name.clone(),
      push: PushRef {
        full_name: pushed.full_name.clone(),
        tip,
        lease,
        source,
      },
      parent_ref: pushed.parent_ref.clone(),
    });
  }
  let target = PushTarget {
    repository_path: &services.repository_path,
    remote: &services.push.remote,
    baseline: &services.push.baseline,
    my_email: services.push.my_email.as_deref(),
    size_limit: services.push.size_limit,
  };
  let whole_stack = pushes.iter().map(|pushed| pushed.whole_stack).collect();
  Ok(Some(PreparedPush::leased(&target, branches, whole_stack, PushOptions { atomic: true, dry_run: false })))
}

/// A blocking step of a start on the blocking pool
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, ApiError> + Send + 'static) -> Result<T, ApiError> {
  tokio::task::spawn_blocking(in_current_span(work))
    .await
    .map_err(|error| ApiError::internal(format!("The step of the start failed: {error}")))?
}

#[expect(clippy::needless_pass_by_value, reason = "the error of a blocking step comes into `map_err` by value")]
fn git_failure(error: anyhow::Error) -> ApiError {
  ApiError::new(ErrorCode::GitError, format!("{error:#}"))
}

fn plan_changed(what: &str) -> ApiError {
  ApiError::new(
    ErrorCode::PlanChanged,
    format!("What the start was planned on changed before its first effect ({what}); nothing was done: prepare it again"),
  )
}

fn unknown_subject(subject: &Subject) -> ApiError {
  ApiError::new(ErrorCode::UnknownBranch, format!("The last sync has no {}: nothing was pushed or started", subject.words()))
}

/// The first blocker of a plan as the start answers it: the code `bd` refuses it with, in the words of the guards; a
/// refusal of the landing gate with every refusal of it among `blockers` (each with its fix), a conflict with the target
/// with its commit and files
fn refusal(reason: &Reason, blockers: &[Reason], subject: &Subject, robots: &[PatronusRobot]) -> ApiError {
  let code = match reason {
    Reason::Landed { .. } | Reason::StackLanded { .. } | Reason::LandedIn { .. } | Reason::PullRequestMerged { .. } => ErrorCode::Landed,
    Reason::StackNotBuilt {
      state: StackBranchState::Conflict,
      ..
    } => ErrorCode::StackConflict,
    Reason::StackNotBuilt { .. } | Reason::NoStackBranch { .. } => ErrorCode::StackNotBuilt,
    Reason::DropsRemote { .. } | Reason::CantPush { .. } => ErrorCode::PushRefused,
    Reason::TargetConflict { .. } => ErrorCode::TargetConflict,
    Reason::Draft { .. } | Reason::Unprotected { .. } => ErrorCode::PatronusRefused,
    Reason::OtherBase { .. } => ErrorCode::PullRequestOtherBase,
    Reason::ApprovalsMissing { .. } => ErrorCode::ApprovalsMissing,
    Reason::DismissesApprovals { .. } => ErrorCode::PushDismissesApprovals,
    Reason::Parked { .. } | Reason::StackParked { .. } => ErrorCode::Parked,
    Reason::LandingRefused { refusal } => match **refusal {
      LandingRefusal::DanglingReference { .. } => ErrorCode::DanglingReference,
      LandingRefusal::MissingDeclaration { .. } => ErrorCode::MissingDeclaration,
    },
    Reason::Blocked { .. } | Reason::LastCommit { .. } | Reason::Descendant { .. } | Reason::Ancestor { .. } | Reason::Held { .. } | Reason::StackHeld { .. } => {
      ErrorCode::RobotRunning
    }
  };
  let words = match reason {
    Reason::LandingRefused { refusal } => refusal.reason(),
    _ => start_refusal_text(reason, &subject.words(), robots),
  };
  let error = ApiError::new(code, format!("Nothing was pushed or started: {words}"));
  match reason {
    Reason::LandingRefused { .. } => error.with_details(ErrorDetails::Landing {
      refusals: blockers
        .iter()
        .filter_map(|blocker| match blocker {
          Reason::LandingRefused { refusal } => Some((**refusal).clone()),
          _ => None,
        })
        .collect(),
    }),
    Reason::TargetConflict { commit, files } => error.with_details(ErrorDetails::Conflict {
      commits: vec![commit.clone()],
      paths: files.clone(),
    }),
    _ => error,
  }
}

/// Why the effects after one are skipped
enum Stopped {
  /// The effect failed
  Failed,
  /// The facts changed before the effect, or the user stopped the start
  Changed(ApiError),
}

/// The effects of a start as they run, and what they answered
struct Run<'a> {
  host: &'a dyn StartHost,
  services: &'a StartServices,
  steps: &'a Steps,
  planned: Planned,
  records: Vec<EffectRecord>,
  retargets: Vec<Result<(), GitHubWriteError>>,
  canceled: Option<PatronusRobot>,
  pushed: Option<Sent>,
  robot: Option<PatronusRobot>,
  commented: Option<Commented>,
  created: Option<CreatedPullRequest>,
  version: Option<Version>,
  /// Once an effect failed or the facts changed: why the effects after it are skipped
  stopped: Option<Stopped>,
  /// The checks before the start ran (`before_start`)
  checked: bool,
  /// The robots the facts held when the run began
  known: HashSet<String>,
}

impl<'a> Run<'a> {
  const fn new(host: &'a dyn StartHost, services: &'a StartServices, steps: &'a Steps, planned: Planned, known: HashSet<String>) -> Self {
    Self {
      host,
      services,
      steps,
      planned,
      known,
      records: Vec::new(),
      retargets: Vec::new(),
      canceled: None,
      pushed: None,
      robot: None,
      commented: None,
      created: None,
      version: None,
      stopped: None,
      checked: false,
    }
  }

  /// Each effect of the plan in its order, until one fails; the ones after it are skipped
  async fn effects(&mut self, mut prepared: Option<PreparedPush>) {
    for effect in self.planned.plan.effects.clone() {
      let status = if let Some(stopped) = &self.stopped {
        EffectStatus::Skipped {
          reason: match stopped {
            Stopped::Failed => None,
            Stopped::Changed(reason) => Some(reason.clone()),
          },
        }
      } else {
        match &effect {
          PlannedEffect::Retarget { branch, number, to, .. } => self.retarget(branch, *number, to).await,
          PlannedEffect::CancelRobot { robot_id } => self.cancel(robot_id).await,
          PlannedEffect::Push { .. } => self.push(prepared.take()).await,
          PlannedEffect::StartRobot { route } => self.start(route).await,
          PlannedEffect::CreatePullRequest => self.create_pull_request().await,
        }
      };
      self.records.push(EffectRecord { effect, status });
    }
  }

  /// The first failure: the effects after it are skipped
  fn fail(&mut self, error: ApiError) -> EffectStatus {
    debug!(code = ?error.code, "an effect of the start failed: the ones after it are skipped");
    self.stopped = Some(Stopped::Failed);
    EffectStatus::Failed { error }
  }

  /// The effect and the ones after it are not made: `reason` changed the plan before it, or the user stopped the start
  fn skip(&mut self, reason: ApiError) -> EffectStatus {
    self.stopped = Some(Stopped::Changed(reason.clone()));
    EffectStatus::Skipped { reason: Some(reason) }
  }

  /// The pull request `number` of `branch` goes into `to`: GitHub refusing the one the robot starts from (or of the
  /// subject) stops the start; one of a branch its push takes along is told, and the start goes on
  async fn retarget(&mut self, branch: &str, number: u32, to: &str) -> EffectStatus {
    self.steps.step(OperationStep::Retargeting);
    let Some((github, repository)) = &self.services.github else {
      return self.fail(ApiError::new(ErrorCode::InvalidArguments, "The GitHub repository of the branches is unknown"));
    };
    let answer = crate::pull_request::retarget(github, &self.services.repository_path, repository, number, to).await;
    self.retargets.push(answer.clone());
    match answer {
      Ok(()) => EffectStatus::Done,
      Err(error) => {
        let own = self.planned.plan.pull_request == Some(number) || branch == self.planned.plan_subject_name();
        let error = ApiError::new(
          ErrorCode::NetworkError,
          format!(
            "Cannot retarget pull request #{number} of {branch} to {to}: {}",
            crate::robot_start::github_failure(error).message
          ),
        );
        if own { self.fail(error) } else { EffectStatus::Failed { error } }
      }
    }
  }

  /// The running Safe Push the start replaces is canceled; one that finished meanwhile (as the facts or its status,
  /// one request, say) is as it is: nothing is left to cancel, and the start goes on
  async fn cancel(&mut self, robot_id: &str) -> EffectStatus {
    self.steps.step(OperationStep::CancelingRobot);
    let current = self.host.current().await;
    if let Some(finished) = current.robots.iter().find(|robot| robot.id == robot_id && !is_active(robot)) {
      debug!(robot_id, "the robot the start replaces finished meanwhile");
      self.canceled = Some(finished.clone());
      return EffectStatus::Done;
    }
    // it may have finished since the session listed it; a status that can't be read asks for the cancel all the same
    if let Ok(status) = self.services.patronus.robot_status(robot_id).await
      && status.is_finished()
    {
      debug!(robot_id, ?status, "the robot the start replaces finished since it was listed");
      if let Some(listed) = current.robots.into_iter().find(|robot| robot.id == robot_id) {
        let finished = PatronusRobot { status, ..listed };
        self.version = self.host.write_robot(finished.clone()).await.or(self.version);
        self.canceled = Some(finished);
      }
      return EffectStatus::Done;
    }
    match self.services.patronus.cancel(&self.services.prefix, robot_id).await {
      Ok(robot) => {
        self.version = self.host.write_robot(robot.clone()).await.or(self.version);
        self.canceled = Some(robot);
        EffectStatus::Done
      }
      Err(message) => self.fail(patronus_failure(message)),
    }
  }
}

impl Planned {
  /// The branch of a branch start, the root of a whole stack
  fn plan_subject_name(&self) -> &str {
    match &self.plan.intent {
      domain_types::operation::Intent::Land(land) => land.subject.name(),
      _ => "",
    }
  }
}

impl Run<'_> {
  /// The push of the frozen commits with their leases, its end into the session
  async fn push(&mut self, prepared: Option<PreparedPush>) -> EffectStatus {
    let Some(prepared) = prepared else {
      return EffectStatus::Done;
    };
    if self.steps.canceled() {
      return self.skip(canceled());
    }
    self.steps.step(OperationStep::Pushing);
    let subjects: Vec<RemoteSubject> = self.planned.pushes.iter().map(PushedRef::subject).collect();
    let names: Vec<String> = self.planned.pushes.iter().map(|pushed| pushed.full_name.clone()).collect();
    // a fetch of a remote branch whose pull request moved writes the remote-tracking ref too: the push is newer
    let _claim = claim_refs(&self.services.git_common_dir, &names, || debug!("the push of the start waits for the fetch of its refs")).await;
    self.host.begin_push(subjects.clone());
    let (git, progress) = (self.services.git.clone(), Arc::clone(&self.steps.progress));
    let sent = blocking(move || send(&git, prepared, OverMoved::Once, &mut |tick: TransferProgress| progress(&tick)).map_err(git_failure)).await;
    let sent = match sent {
      Ok(sent) => sent,
      Err(error) => {
        self.version = self.host.end_push(subjects, None).await.or(self.version);
        return self.fail(error);
      }
    };
    self.version = self.host.end_push(subjects, Some(&sent)).await.or(self.version);
    let status = match &sent.answer {
      LeasedPush::Pushed(_) => EffectStatus::Done,
      LeasedPush::RemoteMoved { moved, .. } => {
        let moved: Vec<&str> = moved.iter().filter_map(|index| names.get(*index)).map(String::as_str).collect();
        let mut error = ApiError::new(ErrorCode::LeaseRejected, format!("The remote branch of {} moved: nothing was pushed", moved.join(", ")));
        error.details = Some(ErrorDetails::Branches {
          branches: moved.iter().map(ToString::to_string).collect(),
        });
        EffectStatus::Failed { error }
      }
      LeasedPush::Refused { summary } => EffectStatus::Failed {
        error: ApiError::new(ErrorCode::PushRefused, format!("The remote refused the push: {summary}")),
      },
      LeasedPush::WouldReject => EffectStatus::Failed {
        error: ApiError::new(ErrorCode::LeaseRejected, "The remote would reject the push"),
      },
      LeasedPush::TooLarge(too_large) => EffectStatus::Failed {
        error: ApiError::new(ErrorCode::PushTooLarge, too_large.to_string()),
      },
    };
    self.pushed = Some(sent);
    if let EffectStatus::Failed { error } = status {
      return self.fail(error);
    }
    status
  }

  /// Before the start, once: a pushed branch that moved since its push (a rewrite after the lock), a robot of what it
  /// lands that started meanwhile, a pull request of the comment that closed skip it (`plan_changed`); and the user
  /// stopped it
  async fn before_start(&mut self) -> Option<EffectStatus> {
    if std::mem::replace(&mut self.checked, true) {
      return None;
    }
    if self.steps.canceled() {
      return Some(self.skip(canceled()));
    }
    if let Some(sent) = &self.pushed {
      let tips: BTreeMap<String, Option<String>> = sent
        .prepared
        .branches()
        .iter()
        .map(|branch| (format!("refs/heads/{}", branch.push.full_name), Some(branch.push.tip.clone())))
        .collect();
      let names: Vec<String> = tips.keys().cloned().collect();
      let (git, path) = (self.services.git.clone(), self.services.repository_path.clone());
      match blocking(move || read_refs(&git, &path, &names).map_err(git_failure)).await {
        Err(error) => return Some(self.fail(error)),
        Ok(read) if read != tips => {
          let moved: Vec<String> = read.iter().filter(|(name, tip)| tips.get(*name) != Some(tip)).map(|(name, _)| short(name)).collect();
          return Some(self.skip(changed_before_start(&format!("{} changed after the push", moved.join(", ")))));
        }
        Ok(_) => {}
      }
    }
    let current = self.host.current().await;
    let plan = &self.planned.plan;
    let dry_run = plan_mode_is_dry_run(plan);
    let canceled = self.canceled.as_ref().map(|robot| robot.id.as_str());
    let started = current.robots.iter().find(|robot| {
      let lands = robot.branch_name.as_ref().is_some_and(|branch| plan.lands.contains(branch)) || robot.stack_root.as_deref() == Some(self.planned.plan_subject_name());
      lands
        && is_active(robot)
        && (robot.push_mode == domain_types::patronus::PatronusPushMode::DryRun) == dry_run
        && !self.known.contains(&robot.id)
        && Some(robot.id.as_str()) != canceled
    });
    if let Some(robot) = started {
      return Some(self.skip(changed_before_start(&format!("a robot of what it lands started meanwhile ({})", robot.url))));
    }
    if let Some(StartRoute::Comment { number }) = plan.start_route()
      && let Some(closed) = current
        .pull_requests
        .iter()
        .find(|pull_request| pull_request.number == *number && pull_request.state != PullRequestState::Open)
    {
      return Some(self.skip(changed_before_start(&format!("pull request #{} is {:?} now", closed.number, closed.state).to_lowercase())));
    }
    None
  }
}

/// "auth" of `refs/heads/user/virtual/auth`
fn short(full: &str) -> String {
  full.rsplit('/').next().unwrap_or(full).to_string()
}

fn plan_mode_is_dry_run(plan: &Plan) -> bool {
  matches!(&plan.intent, domain_types::operation::Intent::Land(land) if land.mode == domain_types::patronus::PatronusPushMode::DryRun)
}

/// What the start was planned on changed after its first effects, before its robot (`what`): no robot started, and the
/// effects before stay done
fn changed_before_start(what: &str) -> ApiError {
  ApiError::new(ErrorCode::PlanChanged, format!("{what}: no robot started"))
}

/// The user stopped the start before this effect (`cancel_operation`)
fn canceled() -> ApiError {
  ApiError::new(ErrorCode::Canceled, "The start was stopped before this effect")
}

impl Run<'_> {
  /// The start of the robot through the API, or the `/patronus` comment on the pull request; the robot into the facts
  async fn start(&mut self, route: &StartRoute) -> EffectStatus {
    if let Some(skipped) = self.before_start().await {
      return skipped;
    }
    self.steps.step(OperationStep::Starting);
    let land = match &self.planned.plan.intent {
      domain_types::operation::Intent::Land(land) => land.clone(),
      _ => return self.fail(ApiError::internal("The plan of a start has another intent")),
    };
    match route {
      StartRoute::Api { profile } => {
        let start = ApiStart {
          space: &self.services.space,
          prefix: &self.services.prefix,
          source_ref: &self.planned.source_ref,
          target: &self.services.target,
          mode: land.mode,
          profile: profile.as_ref().map(|profile| profile.id.clone()),
          merge_request: self.planned.merge_request.clone(),
        };
        match start_through_api(&self.services.patronus, start).await {
          Ok(robot) => {
            self.version = self.host.write_robot(robot.clone()).await.or(self.version);
            self.robot = Some(robot);
            EffectStatus::Done
          }
          Err(error) => self.fail(error),
        }
      }
      StartRoute::Comment { number } => self.comment(*number, land.mode).await,
    }
  }

  /// The `/patronus` comment on the pull request `number` and the reply of the bot: accepted is done, no reply within
  /// the wait is unknown (the bot may still answer it)
  async fn comment(&mut self, number: u32, mode: domain_types::patronus::PatronusPushMode) -> EffectStatus {
    let Some((github, repository)) = &self.services.github else {
      return self.fail(ApiError::new(ErrorCode::InvalidArguments, "The GitHub repository of the branches is unknown"));
    };
    // a canceled Dry Run or the replaced Safe Push of the same pull request a minute before is never the new robot
    let current = self.host.current().await;
    let mut known: HashSet<String> = current.robots.iter().filter(|robot| !is_active(robot)).map(|robot| robot.id.clone()).collect();
    known.extend(self.canceled.iter().map(|robot| robot.id.clone()));
    let start = CommentStart {
      github,
      repository_path: &self.services.repository_path,
      repository,
      number,
      mode,
      known,
      until_listed: self.services.wait_for_robot,
    };
    let host = self.host;
    let robots_since = move |since: u32| host.robots_since(since);
    match start_by_comment(start, &*self.services.clock, &robots_since).await {
      Ok(commented) => {
        if let Some(robot) = &commented.robot {
          self.version = self.host.write_robot(robot.clone()).await.or(self.version);
          self.robot = Some(robot.clone());
        }
        let status = if commented.accepted { EffectStatus::Done } else { EffectStatus::Unknown };
        self.commented = Some(commented);
        status
      }
      Err(error) => self.fail(error),
    }
  }

  /// The pull request of the subject is opened, after its push (GitHub may not know the branch for a moment)
  async fn create_pull_request(&mut self) -> EffectStatus {
    if let Some(skipped) = self.before_start().await {
      return skipped;
    }
    self.steps.step(OperationStep::Writing);
    let Some((github, repository)) = &self.services.github else {
      return self.fail(ApiError::new(ErrorCode::InvalidArguments, "The GitHub repository of the branches is unknown"));
    };
    let text = match self.host.pull_request_text(self.pushed.as_ref()).await {
      Ok(text) => text,
      Err(error) => return self.fail(error),
    };
    let params = CreatePullRequestParams {
      repository_path: self.services.repository_path.clone(),
      branch_name: self.planned.plan_subject_name().to_string(),
      head: text.head,
      base: text.base,
      title: text.title,
      body: text.body,
      reviewers: text.reviewers,
      assign_self: text.assign_self,
      wait_for_mirror: self.pushed.is_some(),
    };
    match create_pull_request(github, repository, &params).await {
      Ok(created) => {
        self.created = Some(created);
        EffectStatus::Done
      }
      Err(error) => self.fail(crate::robot_start::github_failure(error)),
    }
  }

  /// The run as its outcome tells it, with what its effects answered
  fn started(self, op: OperationId, began: std::time::Instant) -> Started {
    let subject = self.planned.plan_subject_name().to_string();
    let mode = match &self.planned.plan.intent {
      domain_types::operation::Intent::Land(land) => land.mode,
      _ => domain_types::patronus::PatronusPushMode::RebaseAutosquash,
    };
    let result = OperationResult::Land(Box::new(LandResult {
      robot: self.robot.clone(),
      canceled: self.canceled.clone(),
      comment: self.commented.as_ref().map(|commented| StartComment {
        number: commented.number,
        url: commented.comment_url.clone(),
        accepted: commented.accepted,
      }),
      notes: self.planned.notes.clone(),
    }));
    let outcome = Outcome {
      op,
      effects: self.records,
      news_key: Some(NewsKey::RobotSubject { subject, mode }),
      version_after: self.version,
      result,
      stats: OperationStats {
        millis: u32::try_from(began.elapsed().as_millis()).unwrap_or(u32::MAX),
        ..OperationStats::default()
      },
    };
    Started {
      plan: self.planned.plan,
      outcome,
      retargets: self.retargets,
      canceled: self.canceled,
      pushed: self.pushed,
      robot: self.robot,
      commented: self.commented,
      created: self.created,
    }
  }
}

#[cfg(test)]
#[path = "land_execute_test.rs"]
mod land_execute_test;
