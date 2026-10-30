//! The run of a start of a Safe Push or a Dry Run (`Intent::Land`; docs/design/rust-domain-authority.md, "Инварианты
//! планов"; docs/design/git-operations.md, "Push and land"), one for the app and `bd`: the plan is prepared again from
//! the facts now (`StartHost::plan`) and runs only on the fingerprint its caller accepted, with every confirmation of it
//! accepted. A restart of a robot (`Intent::RestartRobot`, `robot::execute_restart`) runs the same way (`run_planned`):
//! its plan pushes first while the branch has work the remote lacks, and its last effect restarts the robot
//! (`PlannedEffect::RestartRobot`) where a start starts one; so do the creation of a pull request and a review request
//! (`pull_request::execute_pull_request`), whose last effect opens the pull request (`CreatePullRequest`, the text of the
//! intent), waits until GitHub has the pushed branch (`WaitForMirror`) or asks the reviewers (`RequestReviewers`), each
//! written into the facts. Then, in this order:
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
//!    (`robot_start`), or the restart; the robot it started goes into the facts. Before its request goes out the record
//!    of the operation in the operations registry tells what it sends (`RecordHandle::send_start`), and the comment
//!    once GitHub answered its post; an answer lost after the request went out (a timeout, a connection that broke off,
//!    a comment the bot did not answer within the wait) is `EffectStatus::Unknown`, never sent again: the record stays as
//!    a start whose answer was lost, which holds what it held until a read of Patronus settles it
//!    (`operations::reconcile`) or the user forgets it (`Intent::ForgetStart`).
//!
//! A start of a repository that lands by a push into the target (`PlannedEffect::PushBaseline`,
//! `domain_policy::land_direct`) runs the same frame: its witness the tip of the copy, the remote-tracking ref of the
//! baseline the copy was built on and the checked out branch that holds the originals, under the lock; then, after the
//! lock, the push of the copy into the target by the push URL (`sync_core::baseline_push`). A push that landed goes into
//! the facts as what landed its subject until a fetch shows it (`StartHost::baseline_pushed`); a target that moved since
//! (no fast-forward) is fetched (`fetch_baseline`, which joins a fetch that runs) and synced, `baseline_moved`; a refusal
//! of the server (`[remote rejected]`) goes into the facts with its words, which the gate of the route asks about next,
//! `baseline_refused`. Update Project after a push that landed is an operation of its own (`engine::execute_land`).
//!
//! A refusal the facts know (a blocker, an unaccepted confirmation, a changed fingerprint, a held lock) does nothing:
//! every effect comes after the full witness (a plan that moves and leases no ref reads none and takes no lock). The
//! first effect that fails stops the start; the effects before it stay done, and the outcome lists each with its
//! status.

use crate::push::{Sent, send};
use crate::registry_file::{PendingStart, RecordHandle};
use crate::robot_start::{ApiStart, CommentStart, Commented, NoRobot, RobotWait, follow_start_comment, patronus_failure, post_start_comment, start_through_api};
use crate::witness::{Witness, check, plan_changed, read_refs};
use domain_policy::github_stack::stack_writes;
use domain_policy::holds::is_active;
use domain_types::RemoteStatusUpdate;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::{ApiError, ErrorCode, ErrorDetails};
use domain_types::landing::{BaselinePush as PushedCopy, Confirmation, Land, LandingRefusal, PlannedEffect, StartRoute, Subject};
use domain_types::operation::{
  EffectRecord, EffectStatus, Fingerprint, Intent, LandResult, NewsKey, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Plan, PushResult, RobotResult,
  StartComment, StartNote,
};
use domain_types::patronus::{PatronusCommandSubject, PatronusMergeRequest, PatronusPushMode, PatronusRobot};
use domain_types::pull_request::{AsyncMergeStatus, CreatedPullRequest, MergeMethod, RequestedReviewers};
use domain_types::pull_request::{PullRequest, PullRequestState, StackWrite};
use domain_types::reason::Reason;
use domain_types::repository_view::HoldKind;
use domain_types::session::Version;
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use history_rewrite::repository_lock::{LockHolder, RepositoryLock, RepositoryLocked, lock_repository_in};
use patronus::service::PatronusService;
use remote_facts::fact_sources::Read;
use remote_facts::pull_request_create::{CreatePullRequestParams, MIRROR_WAIT, create_pull_request, wait_for_branch};
use remote_facts::pull_request_writes::{
  MergeWhen, MergeWrite, add_to_stack, create_stack, merge_async, merge_async_status, merge_pull_request, request_pull_request_reviewers, unstack,
};
use remote_facts::pull_requests::GitHubWriteError;
use remote_facts::pull_requests::failure_message;
use repository_model::branch_table::BranchTable;
use repository_model::facts::BaselineRefusal;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::ref_claims::claim_refs;
use repository_model::refusal_words::start_refusal_text;
use repository_model::runtime::sleep_until;
use repository_model::table::{RemoteSubject, TableState};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use sync_core::baseline::fetch_baseline;
use sync_core::baseline_push::{BaselinePush, BaselinePushed, push_baseline};
use sync_core::github_repository::GitHubRepository;
use sync_core::push::{LeasedBranch, LeasedPush, PreparedPush, PushTarget};
use sync_core::push_lease::{LeaseSource, PushOptions, PushRef};
use sync_core::transfer_progress::TransferProgress;
use sync_utils::clock::Clock;
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tracing::{debug, instrument, warn};

/// How often the result of the asynchronous merge of a layer of a stack of GitHub is asked
const STACK_MERGE_POLL: std::time::Duration = std::time::Duration::from_secs(2);
/// How long the asynchronous merge of a layer is waited for: the merge of a stack took seconds where it was measured
const STACK_MERGE_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

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
  /// What it lands: the subject of the start, or of the robot a restart starts again
  pub subject: Subject,
  /// The push mode of the robot it starts or restarts
  pub mode: PatronusPushMode,
  /// The open pull requests of the branches of the stack of GitHub its `LinkStack` brings in line, by branch, as the
  /// plan read them; the run adds the ones it opens
  pub stack_pull_requests: BTreeMap<String, PullRequest>,
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
  /// An unapplied branch, `name` the branch it was (`Intent::Push` with `PushScope::Unapplied`)
  pub unapplied: bool,
  /// Push Up to Here: the copy of a commit of the branch it pushes instead of its tip, so the remote gets the branch up
  /// to it (`PushScope::UpTo`); the tip of the branch is witnessed all the same, so a sync that rebuilt it changes the plan
  pub up_to: Option<String>,
  /// The remote head it leases on
  pub lease: PushLease,
  /// The branch's author (`GroupedBranchInfo::my_email`; of a stack branch the author of the commits of all its members,
  /// `StackBranchUpdate::my_email`): the remote status the push leaves counts that author's commits as the user's, as a
  /// sync does; `None` counts none (an unapplied branch)
  pub my_email: Option<String>,
}

/// The remote head a ref of a push leases on
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushLease {
  /// The one the full witness froze under the lock (a start, a restart, a pull request: their witness names the
  /// remote-tracking ref)
  Frozen,
  /// The one the plan saw (the intent `Push`: the row's, or the session's after a wait); `None`: no remote branch. A
  /// remote that moved since refuses the push, which fetches it and goes again over it once when nothing is lost
  Planned(Option<String>),
}

/// The remote status `state` shows of `pushed`, as a push of it goes over it: of a stack branch the stack's, of an
/// unapplied branch the one its section has, of a Push Up to Here the part up to its commit (the commits after it stay
/// unpushed); `None` for a ref the table has no status of
#[must_use]
pub fn shown_remote(state: &TableState, pushed: &PushedRef) -> Option<RemoteStatusUpdate> {
  let status = if pushed.whole_stack {
    state.stack(&pushed.name)?.remote.clone()
  } else if pushed.unapplied {
    state.sections().unapplied.iter().find(|branch| branch.branch == pushed.name)?.remote.clone()
  } else {
    state.branch(&pushed.name)?.remote.status.clone()
  }?;
  Some(match &pushed.up_to {
    Some(commit) => sent_part(status, commit),
    None => status,
  })
}

/// `status` of a push up to `commit`: the commits up to it are the ones it sends
fn sent_part(mut status: RemoteStatusUpdate, commit: &str) -> RemoteStatusUpdate {
  if let Some(at) = status.unpushed_commits.iter().position(|id| id == commit) {
    status.unpushed_commits.truncate(at + 1);
    status.my_unpushed_count = status.my_unpushed_count.min(u32::try_from(at + 1).unwrap_or(u32::MAX));
  }
  status
}

impl PushedRef {
  /// The virtual branch `name` of `table` as a push sends it: under `prefix`, built on the branch its stack names, by its
  /// author, leased on `lease`; `None` for a branch the table lacks
  #[must_use]
  pub fn branch(table: &BranchTable, prefix: &str, name: &str, lease: PushLease) -> Option<Self> {
    let full = |branch: &str| to_final_branch_name(prefix, branch).ok();
    let info = table.branch(name)?;
    Some(Self {
      name: name.to_string(),
      full_name: full(name)?,
      parent_ref: info.stack.as_ref().and_then(|stack| stack.effective_base.as_deref()).and_then(full),
      whole_stack: false,
      unapplied: false,
      up_to: None,
      lease,
      my_email: info.my_email.clone(),
    })
  }

  /// The stack branch of the whole stack `stack` as a push sends it, by the author the sync found of the commits of its
  /// members, leased on `lease`
  #[must_use]
  pub fn stack_branch(stack: &StackBranchUpdate, lease: PushLease) -> Self {
    Self {
      name: stack.root.clone(),
      full_name: stack.branch_name.clone(),
      parent_ref: None,
      whole_stack: true,
      unapplied: false,
      up_to: None,
      lease,
      my_email: stack.my_email.clone(),
    }
  }

  fn subject(&self) -> RemoteSubject {
    if self.whole_stack {
      RemoteSubject::WholeStack(self.name.clone())
    } else if self.unapplied {
      RemoteSubject::Unapplied(self.name.clone())
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

  /// The remote status the facts show of each of `pushes` (`shown_remote`), in their order: what the push goes over,
  /// which its toast and its Activity entry say what it did from
  fn shown_remote<'a>(&'a self, pushes: &'a [PushedRef]) -> Read<'a, Vec<Option<RemoteStatusUpdate>>>;

  /// The push of `subjects` starts: from now on a sync shows no remote status of them
  fn begin_push(&self, subjects: Vec<RemoteSubject>);

  /// The push that `begin_push` began ended as `sent` says (`None`: it failed before the remote answered): the remote
  /// statuses it left go into the table. The version of the view after its patch
  fn end_push<'a>(&'a self, subjects: Vec<RemoteSubject>, sent: Option<&'a Sent>) -> Read<'a, Option<Version>>;

  /// The pull request a start opens (`PlannedEffect::CreatePullRequest`): what it says and who reviews it, after the
  /// push `sent` (its text names what the push sent)
  fn pull_request_text<'a>(&'a self, sent: Option<&'a Sent>) -> Read<'a, Result<PullRequestText, ApiError>>;

  /// The title and the body of the pull request of `head` (its full name), a layer below the subject in its stack of
  /// GitHub, into `base`: of its own commits against the branch below it (`CreateLayerPullRequest`); a host that opens no
  /// pull request of a layer refuses it
  fn layer_text<'a>(&'a self, _repository_path: &'a str, _head: &'a str, _base: &'a str) -> Read<'a, Result<(String, String), ApiError>> {
    Box::pin(std::future::ready(Err(ApiError::new(
      ErrorCode::InvalidArguments,
      "This start opens no pull request of a layer of a stack of GitHub",
    ))))
  }

  /// The robot the start started or canceled goes into the facts: the version of the view after its patch
  fn write_robot(&self, robot: PatronusRobot) -> Read<'_, Option<Version>>;

  /// The pull request a run opened, or the reviewers it asked, goes into the facts: the version of the view after its
  /// patch; `None` when nothing keeps it (`bd`)
  fn write_pull_request(&self, write: PullRequestWrite) -> Read<'_, Option<Version>>;

  /// A push into the target ended: the copies it landed (`landed`, by subject key), which land their subjects until a
  /// fetch shows them in the baseline, or the refusal of the server (`refused`), which the gate of the route asks about
  /// before the next push, go into the facts: the version of the view after its patch; `None` when nothing keeps them
  /// (`bd`, whose process ends with the command)
  fn baseline_pushed(&self, _landed: Vec<(String, PushedCopy)>, _refused: Option<BaselineRefusal>) -> Read<'_, Option<Version>> {
    Box::pin(std::future::ready(None))
  }

  /// The baseline was fetched after a push into the target that was no fast-forward: a sync builds the copies anew on
  /// it (`bd`: nothing, its next command syncs)
  fn baseline_fetched(&self) {}
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

/// The repository lock of a start, blocking: after the operations of this process before it in its line, `bd` waits a
/// bounded time for another process (`locked`), the app as long as the holder lives and until the user stops the start
/// (`canceled`, the second argument); `on_wait` (the first) is called while another holds it, with the holder each time
/// it is read again (`None`: a wait that names nobody)
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

/// A file lock that could not be taken (the repository lock of the app, the lock of the operations registry): `locked`
/// while another process holds it, its holder named, else `git_error`
pub(crate) fn lock_failure(error: &anyhow::Error) -> ApiError {
  match error.downcast_ref::<RepositoryLocked>() {
    Some(locked) => ApiError::new(ErrorCode::Locked, locked.to_string()),
    None => ApiError::new(ErrorCode::GitError, format!("{error:#}")),
  }
}

/// What a start runs on besides its facts
#[derive(Clone)]
pub struct StartServices {
  pub git: GitCommandExecutor,
  pub repository_path: String,
  /// `$GIT_COMMON_DIR` of the repository: the refs a fetch of this process writes are claimed there (`ref_claims`)
  pub git_common_dir: std::path::PathBuf,
  pub prefix: String,
  /// The service of the robots; `None` off Space or without a token: a plan that starts, restarts or cancels no robot
  /// (a pull request, a review request) runs without it
  pub patronus: Option<Arc<PatronusService>>,
  /// The Space repository the robots run in; empty off Space
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

impl StartServices {
  /// The service of the robots, or why there is none
  fn patronus(&self) -> Result<&PatronusService, ApiError> {
    self
      .patronus
      .as_deref()
      .ok_or_else(|| ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space, or Patronus has no token: no robot can be asked"))
  }
}

/// Where the push of a start goes and what its remote statuses compare with
#[derive(Debug, Clone)]
pub struct PushSettings {
  pub remote: String,
  /// "origin/master"
  pub baseline: String,
  /// `branchdeck.pushSizeLimit`
  pub size_limit: Option<u64>,
}

/// Where the caller hears a start: its steps, the progress of its push; whether the user stopped it; and the record of
/// its operation in the operations registry, which tells the other processes what its start sent
#[derive(Clone)]
pub struct Steps {
  step: Arc<dyn Fn(OperationStep) + Send + Sync>,
  progress: Arc<dyn Fn(&TransferProgress) + Send + Sync>,
  canceled: Arc<AtomicBool>,
  record: Option<RecordHandle>,
}

impl Steps {
  /// Steps nobody hears, never canceled: `bd`
  #[must_use]
  pub fn none() -> Self {
    Self::new(Arc::new(|_| {}), Arc::new(|_| {}), Arc::new(AtomicBool::new(false)))
  }

  #[must_use]
  pub fn new(step: Arc<dyn Fn(OperationStep) + Send + Sync>, progress: Arc<dyn Fn(&TransferProgress) + Send + Sync>, canceled: Arc<AtomicBool>) -> Self {
    Self {
      step,
      progress,
      canceled,
      record: None,
    }
  }

  /// The same steps, the start told in `record` (none: nothing is recorded, as in the tests of a registry without files)
  #[must_use]
  pub fn with_record(mut self, record: Option<RecordHandle>) -> Self {
    self.record = record;
    self
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

impl Accepted {
  /// What the caller accepted, for `plan`, decided again once its operation was admitted. After a wait (`waited`: in
  /// the queue behind the operations of the session it needs what they hold of, or for the sync a push waits for) the
  /// request asked for this intent, and what it waited for changed the facts on purpose: it runs on the plan they left
  /// (its fingerprint) while the user accepted every confirmation of it, and one that asks more refuses it
  /// (`plan_changed`). Without a wait as the caller accepted it: a plan that changed under the click refuses it
  #[must_use]
  pub fn decided_again(&self, plan: &Plan, waited: bool) -> Self {
    if !waited {
      return self.clone();
    }
    Self {
      fingerprint: plan.fingerprint.clone(),
      confirmations: self.confirmations.clone(),
    }
  }
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
  /// The robot it started through the API, or the one a comment started once listed; of a restart the robot it
  /// restarted last
  pub robot: Option<PatronusRobot>,
  /// The robots its restarts started, in the order of its effects (a push of several branches restarts the Safe Push of
  /// each)
  pub restarted: Vec<PatronusRobot>,
  pub commented: Option<Commented>,
  /// The pull request it opened (or found open)
  pub created: Option<CreatedPullRequest>,
  /// Whom its review request asked, and whom GitHub refused
  pub requested: Option<RequestedReviewers>,
}

/// Runs the start `land` as `op` on the plan its caller accepted (`fingerprint`, with `accepted` among the
/// confirmations): prepared again from the facts of `host`, on what the operations it `waited` for left
/// (`Accepted::decided_again`), then the witness, the lock and the freeze, then its effects in the order of the plan
/// (see the module)
///
/// # Errors
/// Before any effect: a subject the table lacks, a fingerprint or a confirmation the plan no longer has
/// (`plan_changed`), what refuses it (the code of its first blocker), a fact it still waits for, a lock another holds
/// (`locked`), git failing. After the first effect a failure is the status of its effect in the outcome
#[instrument(skip_all, fields(subject = %land.subject.name()))]
pub async fn execute(host: &dyn StartHost, services: &StartServices, steps: &Steps, op: OperationId, land: &Land, accepted: &Accepted, waited: bool) -> Result<Started, ApiError> {
  steps.step(OperationStep::Checking);
  let planned = host.plan(land).await?.ok_or_else(|| unknown_subject(&land.subject))?;
  let accepted = accepted.decided_again(&planned.plan, waited);
  run_planned(host, services, steps, op, planned, &accepted).await
}

/// Runs `planned`, a plan prepared again just now (a start, or a restart of a robot, `robot::execute_restart`), as `op`
/// on what its caller accepted: its blockers refuse it, then the fingerprint and the confirmations, the witness, the
/// lock and the freeze (none for a plan that moves and leases no ref), then its effects in their order (see the module)
///
/// # Errors
/// As `execute`, before any effect
pub async fn run_planned(host: &dyn StartHost, services: &StartServices, steps: &Steps, op: OperationId, planned: Planned, accepted: &Accepted) -> Result<Started, ApiError> {
  // what refuses it now is told as itself, a refusal the check against the newest target found too
  if let Some(blocker) = planned.plan.blockers.first() {
    return Err(refusal(blocker, &planned.plan, &planned.subject, &host.current().await.robots));
  }
  let intent = &planned.plan.intent;
  check(&accepted.fingerprint, &planned.plan.fingerprint, intent)?;
  if planned.plan.confirmations.iter().any(|confirmation| !accepted.confirmations.contains(confirmation)) {
    return Err(plan_changed(intent, Some("it asks to accept more than was accepted")));
  }
  if !planned.plan.waits.is_empty() {
    return Err(plan_changed(intent, Some("it waits for facts nobody answered yet")));
  }
  // a plan that moves and leases no ref (a restart on the remote branch as it is) has no witness to read, no lock
  let prepared = if planned.witness.refs.is_empty() {
    None
  } else {
    // the cheap witness: a stale plan goes without a wait for the lock
    let names: Vec<String> = planned.witness.refs.keys().cloned().collect();
    let (git, path) = (services.git.clone(), services.repository_path.clone());
    let cheap = blocking(move || read_refs(&git, &path, &names).map_err(git_failure)).await?;
    witnessed(&planned.plan.intent, &planned.witness, &cheap)?;
    let frozen = frozen_under_lock(services, steps, &planned.witness).await?;
    witnessed(&planned.plan.intent, &planned.witness, &frozen)?;
    freeze(services, &planned.pushes, &frozen, false)?
  };
  debug!(effects = planned.plan.effects.len(), "the witness holds: the effects run");
  // the robots the facts held when the plan ran: one listed later that lands a part of it started meanwhile
  let known: HashSet<String> = host.current().await.robots.into_iter().map(|robot| robot.id).collect();
  let mut run = Run::new(host, services, steps, planned, known);
  run.effects(prepared).await;
  Ok(run.started(op))
}

/// The full witness and the freeze under the repository lock: the tips of the refs of the plan, one `for-each-ref`, then
/// the lock is released
async fn frozen_under_lock(services: &StartServices, steps: &Steps, witness: &Witness) -> Result<BTreeMap<String, Option<String>>, ApiError> {
  let names: Vec<String> = witness.refs.keys().cloned().collect();
  let (git, path, lock, steps) = (services.git.clone(), services.repository_path.clone(), Arc::clone(&services.lock), steps.clone());
  blocking(move || {
    let waited = AtomicBool::new(false);
    // the step, with whom it waits for each time the holder is read: its time grows, and every window names it
    let on_wait = |holder: Option<&LockHolder>| {
      waited.store(true, Ordering::SeqCst);
      steps.step(OperationStep::WaitingForLock {
        holder: holder.map(LockHolder::info),
      });
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

/// The witness holds when the refs read now are the ones the plan of `intent` was decided on
fn witnessed(intent: &Intent, witness: &Witness, read: &BTreeMap<String, Option<String>>) -> Result<(), ApiError> {
  if witness.refs == *read {
    return Ok(());
  }
  let moved: Vec<&str> = read
    .iter()
    .filter(|(name, tip)| witness.refs.get(*name) != Some(tip))
    // as git names a branch: "user/virtual/auth", "origin/user/virtual/auth"
    .map(|(name, _)| name.strip_prefix("refs/heads/").or_else(|| name.strip_prefix("refs/remotes/")).unwrap_or(name))
    .collect();
  debug!(?moved, "a ref of the plan moved before its first effect");
  Err(plan_changed(intent, Some(&format!("{} moved", moved.join(", ")))))
}

/// The push of the plan on the frozen tips and leases: one atomic push of every branch it sends (`git push --dry-run`
/// with `dry_run`); `None` without one
fn freeze(services: &StartServices, pushes: &[PushedRef], frozen: &BTreeMap<String, Option<String>>, dry_run: bool) -> Result<Option<PreparedPush>, ApiError> {
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
    // a commit of the branch, which the witness of its tip keeps a commit of it
    let tip = pushed.up_to.clone().unwrap_or(tip);
    let lease = match &pushed.lease {
      PushLease::Frozen => frozen.get(&format!("refs/remotes/{}/{}", services.push.remote, pushed.full_name)).cloned().flatten(),
      PushLease::Planned(lease) => lease.clone(),
    };
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
      my_email: pushed.my_email.clone(),
    });
  }
  let target = PushTarget {
    repository_path: &services.repository_path,
    remote: &services.push.remote,
    baseline: &services.push.baseline,
    size_limit: services.push.size_limit,
  };
  // one ref needs no `--atomic`, which a remote may not support
  let options = PushOptions {
    atomic: pushes.len() > 1,
    dry_run,
  };
  Ok(Some(PreparedPush::leased(&target, branches, options)))
}

/// What the remote would answer the push of `planned` (`bd push --dry-run`): its blockers refuse it as its run would,
/// then `git push --dry-run` of its commits on the tips and the leases it was decided on, which a dry run need not read
/// again: it moves nothing, so no lock, no witness, no other effect; `None` for a plan that pushes nothing
///
/// # Errors
/// What refuses the plan, git failing
pub async fn dry_run(services: &StartServices, planned: &Planned, robots: &[PatronusRobot]) -> Result<Option<Sent>, ApiError> {
  if let Some(refusal) = refusal_of(planned, robots) {
    return Err(refusal);
  }
  let Some(prepared) = freeze(services, &planned.pushes, &planned.witness.refs, true)? else {
    return Ok(None);
  };
  let git = services.git.clone();
  blocking(move || send(&git, prepared, &mut |_| {}).map_err(git_failure)).await.map(Some)
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

fn unknown_subject(subject: &Subject) -> ApiError {
  ApiError::new(ErrorCode::UnknownBranch, format!("The last sync has no {}: nothing was pushed or started", subject.words()))
}

/// What refuses `planned` as its run would refuse it before anything (`run_planned`): its first blocker, with the code
/// `bd` fails with and the words of the guards, the robots of the hold named among `robots`; `None` when nothing does
#[must_use]
pub fn refusal_of(planned: &Planned, robots: &[PatronusRobot]) -> Option<ApiError> {
  let blocker = planned.plan.blockers.first()?;
  Some(refusal(blocker, &planned.plan, &planned.subject, robots))
}

/// The code a start, a push or a review request is refused with for `reason`, as `bd` fails with it
#[must_use]
pub fn refusal_code(reason: &Reason) -> ErrorCode {
  match reason {
    Reason::Landed { .. } | Reason::StackLanded { .. } | Reason::LandedIn { .. } | Reason::PullRequestMerged { .. } => ErrorCode::Landed,
    // a merged Safe Push landed the branch whole: nothing of it is left to push
    Reason::Held { hold, .. } if hold.kind == HoldKind::Landed => ErrorCode::Landed,
    Reason::StackNotBuilt {
      state: StackBranchState::Conflict,
      ..
    } => ErrorCode::StackConflict,
    Reason::StackNotBuilt { .. } | Reason::NoStackBranch { .. } => ErrorCode::StackNotBuilt,
    Reason::DropsRemote { .. } | Reason::CantPush { .. } | Reason::RewrittenSincePush { .. } | Reason::BaselineRequiresStatusChecks { .. } | Reason::LocalBaseline { .. } => {
      ErrorCode::PushRefused
    }
    // the gates of a push into the target (`domain_policy::landing_route`): the server would refuse it
    Reason::NoWriteAccess { .. } => ErrorCode::NoWriteAccess,
    // the server would refuse it as no fast-forward: the update first, as after a target that moved; and GitHub moved
    // the layer onto the target
    Reason::BaselineAhead { .. } | Reason::LayerRebased { .. } => ErrorCode::BaselineMoved,
    Reason::BaselineRequiresPullRequest { .. } => ErrorCode::BaselineRequiresPullRequest,
    Reason::BaselineRequiresMergeQueue { .. } | Reason::BaselineRestrictsUpdates { .. } | Reason::BaselineRequiresSignatures { .. } | Reason::BaselineRefusedPush { .. } => {
      ErrorCode::BaselineRefused
    }
    Reason::TargetConflict { .. } => ErrorCode::TargetConflict,
    Reason::Draft { .. } | Reason::Unprotected { .. } => ErrorCode::PatronusRefused,
    Reason::NeedsPush { .. } => ErrorCode::NeedsPush,
    Reason::NoReviewer { .. } => ErrorCode::NoReviewer,
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
    // no robot lands off Space, nor where a push into the target lands it; no pull request of GitHub merges it
    Reason::NotOnSpace | Reason::DryRunNeedsPatronus => ErrorCode::NotOnSpace,
    Reason::GitHubMirror { .. } | Reason::NotOnGitHub | Reason::NoPullRequestToMerge { .. } => ErrorCode::NoPullRequest,
    // GitHub can't merge it: a conflict, a head behind its base, a branch to push first
    Reason::MergeConflict { .. } | Reason::MergeBehind { .. } | Reason::MergeHeadBehind { .. } | Reason::LayerNotLinked { .. } => ErrorCode::NotMergeable,
    // the rules of the base hold it back until what they need comes
    Reason::MergeDraft { .. }
    | Reason::MergeNeedsApprovals { .. }
    | Reason::MergeChangesRequested { .. }
    | Reason::MergeChecksFail { .. }
    | Reason::MergeChecksRunning { .. }
    | Reason::MergeBlocked { .. } => ErrorCode::MergeBlocked,
  }
}

/// The first blocker of `plan` as the start answers it: the code `bd` refuses it with, in the words of the guards; a
/// refusal of the landing gate with every refusal of it among its blockers (each with its fix), a conflict with the
/// target with its commit and files
fn refusal(reason: &Reason, plan: &Plan, subject: &Subject, robots: &[PatronusRobot]) -> ApiError {
  let blockers = plan.blockers.as_slice();
  let code = refusal_code(reason);
  let review = matches!(plan.intent, Intent::RequestReview { .. });
  // a draft asks nobody: GitHub marks it ready for review
  let code = if review && matches!(reason, Reason::Draft { .. }) {
    ErrorCode::NoPullRequest
  } else {
    code
  };
  let words = match reason {
    Reason::LandingRefused { refusal } => refusal.reason(),
    Reason::Draft { number } if review => format!("pull request #{number} is a draft: mark it ready for review on GitHub first"),
    _ => start_refusal_text(reason, &subject.words(), robots),
  };
  let nothing = match &plan.intent {
    Intent::Push(_) => "Nothing was pushed",
    Intent::RestartRobot { .. } => "Nothing was restarted",
    Intent::CreatePullRequest(_) => "Nothing was pushed or created",
    Intent::RequestReview { .. } => "Nothing was pushed or requested",
    _ => "Nothing was pushed or started",
  };
  let error = ApiError::new(code, format!("{nothing}: {words}"));
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
  /// The remote status of each ref of the push as it found it (`EffectRecord::remote`), once it ran
  pushed_remote: Vec<Option<RemoteStatusUpdate>>,
  robot: Option<PatronusRobot>,
  restarted: Vec<PatronusRobot>,
  commented: Option<Commented>,
  /// Why the answer of the start is not known (`EffectStatus::Unknown`)
  lost: Option<ApiError>,
  created: Option<CreatedPullRequest>,
  /// The pull requests of the layers below the subject it opened (`CreateLayerPullRequest`), in their order
  created_layers: Vec<CreatedPullRequest>,
  requested: Option<RequestedReviewers>,
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
      pushed_remote: Vec::new(),
      robot: None,
      restarted: Vec::new(),
      commented: None,
      lost: None,
      created: None,
      created_layers: Vec::new(),
      requested: None,
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
          PlannedEffect::CreatePullRequest { base } => self.create_pull_request(base.as_deref()).await,
          PlannedEffect::CreateLayerPullRequest { branch, base } => self.create_layer_pull_request(branch, base).await,
          PlannedEffect::LinkStack { chain } => self.link_stack(chain).await,
          PlannedEffect::RestartRobot { robot_id } => self.restart(robot_id).await,
          PlannedEffect::WaitForMirror { branch } => self.wait_for_mirror(branch).await,
          PlannedEffect::RequestReviewers { number, reviewers } => self.request_reviewers(*number, reviewers).await,
          PlannedEffect::PushBaseline { tip, remote, target, .. } => self.push_baseline(tip, remote, target).await,
          PlannedEffect::MergePullRequest { number, method, head, .. } => self.merge(*number, *method, head, false).await,
          PlannedEffect::EnableAutoMerge { number, method, head, .. } => self.merge(*number, *method, head, true).await,
          PlannedEffect::MergeStack { number, method, head, layers, .. } => self.merge_stack(*number, *method, head, layers).await,
          // a close, a review, the state of an issue and a forget are operations of their own, never a step of a start
          PlannedEffect::ClosePullRequest { .. }
          | PlannedEffect::DisableAutoMerge { .. }
          | PlannedEffect::SubmitReview { .. }
          | PlannedEffect::SetIssueState { .. }
          | PlannedEffect::ForgetStart { .. }
          | PlannedEffect::Rewrite { .. }
          | PlannedEffect::Sync
          | PlannedEffect::Fetch { .. }
          | PlannedEffect::Update
          | PlannedEffect::DismissLanded
          | PlannedEffect::FetchReview { .. } => self.fail(ApiError::internal(
            "A start closes and reviews no pull request, sets the state of no issue, forgets no start and rewrites, fetches or updates nothing",
          )),
        }
      };
      let remote = if matches!(effect, PlannedEffect::Push { .. }) {
        std::mem::take(&mut self.pushed_remote)
      } else {
        Vec::new()
      };
      self.records.push(EffectRecord {
        remote,
        ..EffectRecord::new(effect, status)
      });
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
  /// subject) stops the start; one of a branch its push takes along, and the one of a push (`Intent::Push`), is told, and
  /// the run goes on
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
        // a push goes on without the retarget: the outcome says it (D3a)
        let own = !matches!(self.planned.plan.intent, Intent::Push(_)) && (self.planned.plan.pull_request == Some(number) || branch == self.planned.plan_subject_name());
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

  /// The running Safe Push the start replaces (or the robot a restart pushes into) is canceled; one that finished
  /// meanwhile is as it is (`robot::cancel`): nothing is left to cancel, and the start goes on
  async fn cancel(&mut self, robot_id: &str) -> EffectStatus {
    self.steps.step(OperationStep::CancelingRobot);
    let current = self.host.current().await;
    let host = self.host;
    let write = move |robot: PatronusRobot| host.write_robot(robot);
    let patronus = match self.services.patronus() {
      Ok(patronus) => patronus,
      Err(error) => return self.fail(error),
    };
    match crate::robot::cancel(patronus, &self.services.prefix, &current.robots, robot_id, &write).await {
      Ok(canceled) => {
        self.version = canceled.version.or(self.version);
        self.canceled = Some(canceled.robot);
        EffectStatus::Done
      }
      Err(error) => self.fail(error),
    }
  }

  /// The robot is restarted, after the checks before a start (a pushed branch that moved since, a robot of its subject
  /// started meanwhile); the new robot into the facts
  async fn restart(&mut self, robot_id: &str) -> EffectStatus {
    if let Some(skipped) = self.before_start().await {
      return skipped;
    }
    self.steps.step(OperationStep::Starting);
    let answer = match self.services.patronus() {
      Ok(patronus) => patronus.restart_robot(&self.services.prefix, robot_id).await.map_err(patronus_failure),
      Err(error) => Err(error),
    };
    let status = self.robot_started(answer).await;
    self.restarted.extend(self.robot.clone().filter(|_| status == EffectStatus::Done));
    status
  }

  /// The robot a start or a restart started goes into the facts; what Patronus refused it with fails the effect
  async fn robot_started(&mut self, answer: Result<PatronusRobot, ApiError>) -> EffectStatus {
    match answer {
      Ok(robot) => {
        self.version = self.host.write_robot(robot.clone()).await.or(self.version);
        self.robot = Some(robot);
        EffectStatus::Done
      }
      Err(error) => self.fail(error),
    }
  }
}

impl Planned {
  /// The branch of a branch start, the root of a whole stack
  fn plan_subject_name(&self) -> &str {
    self.subject.name()
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
    let shown = self.host.shown_remote(&self.planned.pushes).await;
    self.host.begin_push(subjects.clone());
    let (git, progress) = (self.services.git.clone(), Arc::clone(&self.steps.progress));
    let sent = blocking(move || send(&git, prepared, &mut |tick: TransferProgress| progress(&tick)).map_err(git_failure)).await;
    let sent = match sent {
      Ok(sent) => sent,
      Err(error) => {
        self.version = self.host.end_push(subjects, None).await.or(self.version);
        return self.fail(error);
      }
    };
    self.version = self.host.end_push(subjects, Some(&sent)).await.or(self.version);
    self.pushed_remote = match (&sent.answer, &sent.over) {
      (LeasedPush::RemoteMoved { remote_statuses, .. }, _) => remote_statuses.iter().cloned().map(Some).collect(),
      (_, Some(over)) => over.statuses.iter().cloned().map(Some).collect(),
      _ => shown,
    };
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

  /// The push of the copy `tip` into the target `target` of `remote` by the push URL (the module's header), after the
  /// lock: what it landed into the facts, a target that moved fetched and synced, a refusal of the server into the facts
  /// with its words
  async fn push_baseline(&mut self, tip: &str, remote: &str, target: &str) -> EffectStatus {
    if self.steps.canceled() {
      return self.skip(canceled());
    }
    self.steps.step(OperationStep::Pushing);
    let (git, path, baseline, size_limit) = (
      self.services.git.clone(),
      self.services.repository_path.clone(),
      self.services.push.baseline.clone(),
      self.services.push.size_limit,
    );
    let (owned_tip, owned_remote, owned_target, progress) = (tip.to_owned(), remote.to_owned(), target.to_owned(), Arc::clone(&self.steps.progress));
    let pushed = blocking(move || {
      let push = BaselinePush {
        repository_path: &path,
        baseline: &baseline,
        remote: &owned_remote,
        target: &owned_target,
        tip: &owned_tip,
        size_limit,
      };
      push_baseline(&git, &push, &mut |tick: TransferProgress| progress(&tick)).map_err(git_failure)
    })
    .await;
    let report = match pushed {
      Ok(report) => report,
      Err(error) => return self.fail(error),
    };
    let baseline = &self.services.push.baseline;
    match report.pushed {
      BaselinePushed::Pushed { .. } => {
        let landed = self.landed_by_push(tip);
        self.version = self.host.baseline_pushed(landed, None).await.or(self.version);
        EffectStatus::Done
      }
      BaselinePushed::Moved { summary } => {
        // the target as it is now: the fetch of the baseline, which joins a fetch of any process that runs, and the sync
        // that shows it; the copies are built on the fork point of the checked out branch, so the update onto the moved
        // target builds them anew (`land_direct::update_after`: the app runs it, `bd land` names it)
        let (git, path, upstream) = (self.services.git.clone(), self.services.repository_path.clone(), report.upstream);
        let fetched = blocking(move || fetch_baseline(&git, &path, &upstream, &mut |_| {}).map_err(git_failure)).await;
        let after = match fetched {
          Ok(_) => {
            self.host.baseline_fetched();
            format!("{baseline} is fetched: Update Project builds the copy anew on it")
          }
          Err(error) => format!("its fetch failed ({}): Update Project fetches {baseline} and builds the copy anew on it", error.message),
        };
        self.fail(ApiError::new(
          ErrorCode::BaselineMoved,
          format!("{target} moved since the copy was built on it ({summary}): nothing was pushed; {after}, then push again"),
        ))
      }
      BaselinePushed::Refused { message } => {
        let refusal = BaselineRefusal {
          target: target.to_owned(),
          message: message.clone(),
        };
        self.version = self.host.baseline_pushed(Vec::new(), Some(refusal)).await.or(self.version);
        self.fail(ApiError::new(ErrorCode::BaselineRefused, format!("The server refused the push into {target}: {message}")))
      }
      BaselinePushed::TooLarge(too_large) => self.fail(ApiError::new(ErrorCode::PushTooLarge, too_large.to_string())),
    }
  }

  /// GitHub merges the open pull request `number` by `method` while it shows `head`, now or (`auto`) once its checks
  /// pass, after the checks before a start (a pushed branch that moved since its push): what it did into the facts; a
  /// head that moved meanwhile skips it as `plan_changed`, what GitHub refused fails it as `not_mergeable` in its words
  async fn merge(&mut self, number: u32, method: MergeMethod, head: &str, auto: bool) -> EffectStatus {
    let (github, repository) = match self.before_write().await {
      Ok(github) => github,
      Err(status) => return status,
    };
    let when = if auto { MergeWhen::ChecksPass } else { MergeWhen::Now };
    let written = merge_pull_request(&github, &self.services.repository_path, &repository, number, method, head, when).await;
    let what = if auto { "enable the auto-merge of" } else { "merge" };
    match written {
      Ok(MergeWrite::Done) => {
        let write = if auto {
          PullRequestWrite::AutoMerge { number, method: Some(method) }
        } else {
          PullRequestWrite::Merged { number }
        };
        self.version = self.host.write_pull_request(write).await.or(self.version);
        EffectStatus::Done
      }
      Ok(MergeWrite::HeadMoved { head: shown }) => {
        let moved = format!("#{number} shows {} now, not {}", short_id(&shown), short_id(head));
        self.skip(ApiError::new(ErrorCode::PlanChanged, format!("{moved}: nothing was merged")))
      }
      Err(GitHubWriteError::Refused(message)) => self.fail(ApiError::new(ErrorCode::NotMergeable, format!("Cannot {what} #{number}: {message}"))),
      Err(error) => {
        let failure = crate::robot_start::github_failure(error);
        self.fail(ApiError::new(failure.code, format!("Cannot {what} #{number}: {}", failure.message)))
      }
    }
  }

  /// GitHub merges the layer `number` of a stack of GitHub with `layers` below it by `method` while its head is `head`
  /// (`merge-async`), its result asked every `STACK_MERGE_POLL` until it is merged, enqueued or failed, at most
  /// `STACK_MERGE_WAIT`: the merged layers go into the facts as merged
  async fn merge_stack(&mut self, number: u32, method: MergeMethod, head: &str, layers: &[u32]) -> EffectStatus {
    let (github, repository) = match self.before_write().await {
      Ok(github) => github,
      Err(status) => return status,
    };
    let path = self.services.repository_path.clone();
    let cannot = |message: String| ApiError::new(ErrorCode::NotMergeable, format!("Cannot merge #{number}: {message}"));
    let mut merge = match merge_async(&github, &path, &repository, number, head, method).await {
      Ok(merge) => merge,
      Err(GitHubWriteError::Refused(message)) => return self.fail(cannot(message)),
      Err(error) => {
        let failure = crate::robot_start::github_failure(error);
        return self.fail(ApiError::new(failure.code, format!("Cannot merge #{number}: {}", failure.message)));
      }
    };
    let deadline = self.services.clock.now() + STACK_MERGE_WAIT;
    // asked at once first: the merge of a stack took seconds where it was measured
    let mut asked = false;
    while merge.status == AsyncMergeStatus::Pending {
      let Some(uuid) = merge.uuid.clone() else {
        return self.fail(cannot("GitHub named no merge to ask about".to_owned()));
      };
      if std::mem::replace(&mut asked, true) {
        if self.services.clock.now() >= deadline {
          return self.fail(cannot(format!("GitHub did not merge it within {} s; it may still merge", STACK_MERGE_WAIT.as_secs())));
        }
        sleep_until(self.services.clock.as_ref(), self.services.clock.now() + STACK_MERGE_POLL).await;
      }
      merge = match merge_async_status(&github, &path, &repository, number, &uuid).await {
        Ok(merge) => merge,
        Err(error) => {
          let failure = crate::robot_start::github_failure(error);
          return self.fail(ApiError::new(failure.code, format!("Cannot tell whether #{number} merged: {}", failure.message)));
        }
      };
    }
    match merge.status {
      AsyncMergeStatus::Merged { .. } => {
        for merged in layers.iter().chain([&number]) {
          let write = PullRequestWrite::Merged { number: *merged };
          self.version = self.host.write_pull_request(write).await.or(self.version);
        }
        EffectStatus::Done
      }
      // the merge queue of the base merges it later: its merge shows on the pull request
      AsyncMergeStatus::Enqueued => EffectStatus::Done,
      AsyncMergeStatus::Failed { message } => self.fail(cannot(message)),
      AsyncMergeStatus::Pending => self.fail(cannot("GitHub still merges it".to_owned())),
    }
  }

  /// What a push of the copy at `tip` landed, by subject key: each branch the plan lands at the tip its witness froze, and
  /// the subject at `tip` (of a whole stack, `stack:<root>`)
  fn landed_by_push(&self, tip: &str) -> Vec<(String, PushedCopy)> {
    let upstream = &self.services.push.baseline;
    let pushed = |tip: &str| PushedCopy {
      tip: tip.to_owned(),
      upstream: upstream.clone(),
    };
    let tip_of = |name: &str| {
      let full = to_final_branch_name(&self.services.prefix, name).ok()?;
      self.planned.witness.refs.get(&format!("refs/heads/{full}"))?.as_deref()
    };
    let mut landed: Vec<(String, PushedCopy)> = self.planned.plan.lands.iter().filter_map(|name| Some((name.clone(), pushed(tip_of(name)?)))).collect();
    let key = self.planned.subject.key();
    if !landed.iter().any(|(landed, _)| *landed == key) {
      landed.push((key, pushed(tip)));
    }
    landed
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
          let changed = changed_before_start(&format!("{} changed after the push", moved.join(", ")), &self.planned.plan);
          return Some(self.skip(changed));
        }
        Ok(_) => {}
      }
    }
    // a plan that lands nothing (a pull request, a review request) starts no robot another one could race
    if self.planned.plan.lands.is_empty() {
      return None;
    }
    let current = self.host.current().await;
    let plan = &self.planned.plan;
    let dry_run = self.planned.mode == PatronusPushMode::DryRun;
    let canceled = self.canceled.as_ref().map(|robot| robot.id.as_str());
    let started = current.robots.iter().find(|robot| {
      let lands = robot.branch_name.as_ref().is_some_and(|branch| plan.lands.contains(branch)) || robot.stack_root.as_deref() == Some(self.planned.plan_subject_name());
      lands && is_active(robot) && (robot.push_mode == PatronusPushMode::DryRun) == dry_run && !self.known.contains(&robot.id) && Some(robot.id.as_str()) != canceled
    });
    if let Some(robot) = started {
      return Some(self.skip(changed_before_start(&format!("a robot of what it lands started meanwhile ({})", robot.url), plan)));
    }
    if let Some(StartRoute::Comment { number }) = plan.start_route()
      && let Some(closed) = current
        .pull_requests
        .iter()
        .find(|pull_request| pull_request.number == *number && pull_request.state != PullRequestState::Open)
    {
      return Some(self.skip(changed_before_start(
        &format!("pull request #{} is {:?} now", closed.number, closed.state).to_lowercase(),
        plan,
      )));
    }
    None
  }
}

/// The first 7 characters of a commit id, as a message names it
fn short_id(id: &str) -> &str {
  id.get(..7).unwrap_or(id)
}

/// "auth" of `refs/heads/user/virtual/auth`
fn short(full: &str) -> String {
  full.rsplit('/').next().unwrap_or(full).to_string()
}

/// What the run of `plan` was planned on changed after its first effects, before its last one (`what`): no robot
/// started, no pull request was opened or merged, nobody was asked; the effects before stay done
fn changed_before_start(what: &str, plan: &Plan) -> ApiError {
  let merges = plan.effects.iter().any(|effect| {
    matches!(
      effect,
      PlannedEffect::MergePullRequest { .. } | PlannedEffect::EnableAutoMerge { .. } | PlannedEffect::MergeStack { .. }
    )
  });
  let undone = match &plan.intent {
    Intent::Land(_) if merges => "nothing was merged",
    Intent::CreatePullRequest(create) if create.on_github => "GitHub's form was not opened",
    Intent::CreatePullRequest(_) => "no pull request was opened",
    Intent::RequestReview { .. } => "nobody was asked to review",
    Intent::Push(_) => "the Safe Push was not restarted",
    _ => "no robot started",
  };
  ApiError::new(ErrorCode::PlanChanged, format!("{what}: {undone}"))
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
    let mode = self.planned.mode;
    match route {
      StartRoute::Api { profile } => {
        let patronus = match self.services.patronus() {
          Ok(patronus) => patronus,
          Err(error) => return self.fail(error),
        };
        let pending = self.pending_start(route.clone(), None, Some(patronus.web_url()));
        if let Err(error) = self.send_start(pending).await {
          return self.fail(error);
        }
        let start = ApiStart {
          space: &self.services.space,
          prefix: &self.services.prefix,
          source_ref: &self.planned.source_ref,
          target: &self.services.target,
          mode,
          profile: profile.as_ref().map(|profile| profile.id.clone()),
          merge_request: self.planned.merge_request.clone(),
        };
        match start_through_api(patronus, start).await {
          Ok(robot) => {
            self.answered().await;
            self.robot_started(Ok(robot)).await
          }
          Err(NoRobot::Refused(error)) => {
            self.answered().await;
            self.fail(error)
          }
          Err(NoRobot::Lost(error)) => self.lost(error).await,
        }
      }
      StartRoute::Comment { number } => self.comment(route, *number, mode).await,
    }
  }

  /// What the start sends now through `route`, on the commit its source ref has (pushed by the run, else as the plan
  /// witnessed it), for the record of its operation
  fn pending_start(&self, route: StartRoute, pr: Option<u32>, url: Option<String>) -> PendingStart {
    let source = &self.planned.source_ref;
    let pushed = self.pushed.as_ref().and_then(|sent| {
      let branches = sent.prepared.branches();
      branches
        .iter()
        .find(|branch| format!("refs/heads/{}", branch.push.full_name) == *source)
        .map(|branch| branch.push.tip.clone())
    });
    let tip = pushed.or_else(|| self.planned.witness.refs.get(source).cloned().flatten()).unwrap_or_default();
    PendingStart {
      source_ref: source.clone(),
      tip,
      route,
      pr,
      comment_id: None,
      sent_at: unix_now(),
      url,
    }
  }

  /// The record of the operation tells `pending` before its request goes out; nothing recorded (no registry) goes on
  async fn send_start(&self, pending: PendingStart) -> Result<(), ApiError> {
    let Some(record) = self.steps.record.clone() else {
      return Ok(());
    };
    blocking(move || record.send_start(pending)).await
  }

  /// The start was answered: its record tells nothing pending any more (a failed write is taken out with the record)
  async fn answered(&self) {
    if let Some(record) = self.steps.record.clone()
      && let Err(error) = blocking(move || record.answered()).await
    {
      warn!(error = %error.message, "the record of the start can't tell its answer");
    }
  }

  /// The answer of the start was lost after its request went out: unknown, never sent again; its record stays as a start
  /// whose answer was lost, and the effects after it are skipped
  async fn lost(&mut self, error: ApiError) -> EffectStatus {
    warn!(error = %error.message, "the answer of the start was lost: whether its robot started is not known");
    if let Some(record) = self.steps.record.clone()
      && let Err(failure) = blocking(move || record.unknown()).await
    {
      warn!(error = %failure.message, "the record of the start can't keep it as unknown");
    }
    self.lost = Some(error);
    self.stopped = Some(Stopped::Failed);
    EffectStatus::Unknown
  }

  /// The `/patronus` comment on the pull request `number` and the reply of the bot: accepted is done; a post whose answer
  /// was lost, and no reply within the wait, are unknown (the bot may still answer it)
  async fn comment(&mut self, route: &StartRoute, number: u32, mode: PatronusPushMode) -> EffectStatus {
    let Some((github, repository)) = self.services.github.clone() else {
      return self.fail(ApiError::new(ErrorCode::InvalidArguments, "The GitHub repository of the branches is unknown"));
    };
    // a canceled Dry Run or the replaced Safe Push of the same pull request a minute before is never the new robot
    let current = self.host.current().await;
    let mut known: HashSet<String> = current.robots.iter().filter(|robot| !is_active(robot)).map(|robot| robot.id.clone()).collect();
    known.extend(self.canceled.iter().map(|robot| robot.id.clone()));
    let url = current
      .pull_requests
      .iter()
      .find(|pull_request| pull_request.number == number)
      .map(|pull_request| pull_request.url.clone());
    if let Err(error) = self.send_start(self.pending_start(route.clone(), Some(number), url)).await {
      return self.fail(error);
    }
    let start = CommentStart {
      github: &github,
      repository_path: &self.services.repository_path,
      repository: &repository,
      number,
      mode,
      known,
      subject: PatronusCommandSubject::OwnBranch,
      robot_wait: if self.services.wait_for_robot { RobotWait::UntilListed } else { RobotWait::Once },
    };
    let comment = match post_start_comment(&start).await {
      Ok(comment) => comment,
      Err(NoRobot::Refused(error)) => {
        self.answered().await;
        return self.fail(error);
      }
      Err(NoRobot::Lost(error)) => return self.lost(error).await,
    };
    let posted_at = unix_now();
    if let (Some(record), Ok(id)) = (self.steps.record.clone(), comment.id.parse::<u64>()) {
      let url = comment.url.clone();
      if let Err(error) = blocking(move || record.comment_posted(id, url)).await {
        warn!(error = %error.message, "the record of the start can't tell its comment");
      }
    }
    let host = self.host;
    let robots_since = move |since: u32| host.robots_since(since);
    match follow_start_comment(start, comment, posted_at, &*self.services.clock, &robots_since).await {
      Ok(commented) => {
        if let Some(robot) = &commented.robot {
          self.version = self.host.write_robot(robot.clone()).await.or(self.version);
          self.robot = Some(robot.clone());
        }
        let accepted = commented.accepted;
        let number = commented.number;
        self.commented = Some(commented);
        if accepted {
          self.answered().await;
          return EffectStatus::Done;
        }
        let silent = ApiError::new(
          ErrorCode::PatronusError,
          format!("Patronus did not answer the comment on pull request #{number} within a minute"),
        );
        self.lost(silent).await
      }
      Err(error) => {
        self.answered().await;
        self.fail(error.into_error())
      }
    }
  }

  /// Before a write to GitHub, the last effect of a plan: the checks before a start, the step, the GitHub service and
  /// repository of the branches; else the status of the effect
  async fn before_write(&mut self) -> Result<(Arc<GitHubService>, GitHubRepository), EffectStatus> {
    if let Some(skipped) = self.before_start().await {
      return Err(skipped);
    }
    self.steps.step(OperationStep::Writing);
    match &self.services.github {
      Some((github, repository)) => Ok((Arc::clone(github), repository.clone())),
      None => Err(self.fail(ApiError::new(ErrorCode::InvalidArguments, "The GitHub repository of the branches is unknown"))),
    }
  }

  /// The pull request of the subject is opened into `base` (`None`: the target), after its push (GitHub may not know the
  /// branch for a moment)
  async fn create_pull_request(&mut self, base: Option<&str>) -> EffectStatus {
    // the text the user wrote, else the host's (`bd safe-push --create-pull-request`)
    let text = match &self.planned.plan.intent {
      Intent::CreatePullRequest(create) => PullRequestText {
        head: self.planned.source_ref.trim_start_matches("refs/heads/").to_string(),
        base: self.services.target.clone(),
        title: create.title.clone(),
        body: create.body.clone(),
        reviewers: create.reviewers.clone(),
        assign_self: create.assign_self,
      },
      _ => match self.host.pull_request_text(self.pushed.as_ref()).await {
        Ok(text) => text,
        Err(error) => return self.fail(error),
      },
    };
    let text = PullRequestText {
      base: base.map_or(text.base, str::to_owned),
      ..text
    };
    let branch = self.planned.plan_subject_name().to_string();
    match self.open_pull_request(branch, text).await {
      Ok(created) => {
        self.created = Some(created);
        EffectStatus::Done
      }
      Err(status) => status,
    }
  }

  /// The pull request of `branch`, a layer below the subject in its stack of GitHub, is opened into `base` with the text
  /// of its own commits the host reads, assigned as the subject's is, no reviewer asked: the subject's are asked for its
  /// own layer
  async fn create_layer_pull_request(&mut self, branch: &str, base: &str) -> EffectStatus {
    let Ok(head) = to_final_branch_name(&self.services.prefix, branch) else {
      return self.fail(ApiError::new(ErrorCode::InvalidArguments, format!("{branch} is no virtual branch name")));
    };
    let assign_self = matches!(&self.planned.plan.intent, Intent::CreatePullRequest(create) if create.assign_self);
    let text = match self.host.layer_text(&self.services.repository_path, &head, base).await {
      Ok((title, body)) => PullRequestText {
        head,
        base: base.to_owned(),
        title,
        body,
        reviewers: Vec::new(),
        assign_self,
      },
      Err(error) => return self.fail(error),
    };
    match self.open_pull_request(branch.to_owned(), text).await {
      Ok(created) => {
        self.created_layers.push(created);
        EffectStatus::Done
      }
      Err(status) => status,
    }
  }

  /// GitHub opens the pull request of `branch` as `text` says; it goes into the facts and the open pull requests of the
  /// stack of GitHub the run links
  async fn open_pull_request(&mut self, branch: String, text: PullRequestText) -> Result<CreatedPullRequest, EffectStatus> {
    let (github, repository) = self.before_write().await?;
    let params = CreatePullRequestParams {
      repository_path: self.services.repository_path.clone(),
      branch_name: branch.clone(),
      head: text.head,
      base: text.base,
      title: text.title,
      body: text.body,
      reviewers: text.reviewers,
      assign_self: text.assign_self,
      wait_for_mirror: self.pushed.is_some(),
    };
    match create_pull_request(&github, &repository, &params).await {
      Ok(created) => {
        let write = PullRequestWrite::Created {
          pull_request: Box::new(created.pull_request.clone()),
        };
        self.version = self.host.write_pull_request(write).await.or(self.version);
        self.planned.stack_pull_requests.insert(branch, created.pull_request.clone());
        Ok(created)
      }
      Err(error) => Err(self.fail(crate::robot_start::github_failure(error))),
    }
  }

  /// The stack of GitHub of `chain` is brought in line with it (`domain_policy::github_stack::stack_writes`) over its
  /// open pull requests as the plan read them and the run opened them: each write in its order, GitHub's refusal of one
  /// fails the effect, the pull requests stay as they are
  async fn link_stack(&mut self, chain: &[String]) -> EffectStatus {
    let (github, repository) = match self.before_write().await {
      Ok(github) => github,
      Err(status) => return status,
    };
    let chain: Vec<&str> = chain.iter().map(String::as_str).collect();
    let open = &self.planned.stack_pull_requests;
    let writes = stack_writes(
      &chain,
      |branch| open.get(branch).filter(|pull_request| pull_request.state == PullRequestState::Open),
      &self.services.target,
      &self.services.prefix,
    );
    let path = self.services.repository_path.clone();
    for write in writes {
      let done = match &write {
        StackWrite::Unstack { stack } => unstack(&github, &path, &repository, *stack).await.map(|_| ()),
        StackWrite::Retarget { number, to, .. } => crate::pull_request::retarget(&github, &path, &repository, *number, to).await,
        StackWrite::Create { pull_requests } => create_stack(&github, &path, &repository, pull_requests).await.map(|_| ()),
        StackWrite::Add { stack, pull_requests } => add_to_stack(&github, &path, &repository, *stack, pull_requests).await.map(|_| ()),
      };
      if let Err(error) = done {
        debug!(?write, "GitHub refused a write of the stack");
        return self.fail(crate::robot_start::github_failure(error));
      }
    }
    EffectStatus::Done
  }

  /// GitHub has the pushed `branch` (a mirror copies it a moment after the push), asked again for a while: GitHub's form
  /// of a new pull request shows nothing of a branch it doesn't have
  async fn wait_for_mirror(&mut self, branch: &str) -> EffectStatus {
    let (github, repository) = match self.before_write().await {
      Ok(github) => github,
      Err(status) => return status,
    };
    match wait_for_branch(&github, &self.services.repository_path, &repository, branch, MIRROR_WAIT).await {
      Ok(true) => EffectStatus::Done,
      Ok(false) => self.fail(ApiError::new(
        ErrorCode::NetworkError,
        format!("GitHub doesn't have {branch} yet: a mirror copies a pushed branch a moment later; try again"),
      )),
      Err(failure) => self.fail(ApiError::new(ErrorCode::NetworkError, failure_message(&failure))),
    }
  }

  /// `reviewers` are asked to review the pull request `number`; whom GitHub asked goes into the facts. GitHub asking
  /// nobody (it refused each: the author, a user without access) fails the effect with each reason
  async fn request_reviewers(&mut self, number: u32, reviewers: &[String]) -> EffectStatus {
    let (github, repository) = match self.before_write().await {
      Ok(github) => github,
      Err(status) => return status,
    };
    let requested = match request_pull_request_reviewers(&github, &self.services.repository_path, &repository, number, reviewers).await {
      Ok(requested) => requested,
      Err(GitHubWriteError::Refused(message)) => return self.fail(ApiError::new(ErrorCode::InvalidArguments, message)),
      Err(error) => return self.fail(crate::robot_start::github_failure(error)),
    };
    let write = PullRequestWrite::ReviewersRequested {
      number,
      requested: requested.requested.clone(),
    };
    self.version = self.host.write_pull_request(write).await.or(self.version);
    let nobody = requested.requested.is_empty();
    let refused: Vec<String> = requested.refused.iter().map(|refused| format!("{}: {}", refused.login, refused.reason)).collect();
    self.requested = Some(requested);
    if nobody {
      return self.fail(ApiError::new(
        ErrorCode::ReviewRefused,
        format!("GitHub asked nobody to review pull request #{number}: {}", refused.join("; ")),
      ));
    }
    EffectStatus::Done
  }

  /// The run as its outcome tells it, with what its effects answered
  fn started(self, op: OperationId) -> Started {
    let subject = self.planned.plan_subject_name().to_string();
    let mode = self.planned.mode;
    let result = match &self.planned.plan.intent {
      Intent::Push(_) => OperationResult::Push(Box::new(PushResult {
        pushed_over: self.pushed.as_ref().and_then(|sent| sent.over.as_ref()).map_or_default(|over| over.statuses.clone()),
        restarted: self.robot.clone(),
      })),
      Intent::RestartRobot { .. } => OperationResult::Robot(Box::new(RobotResult {
        robot: self.robot.clone(),
        canceled: self.canceled.clone(),
        already_finished: false,
      })),
      Intent::CreatePullRequest(_) => self.created.clone().map_or_default(|created| OperationResult::PullRequest(Box::new(created))),
      Intent::RequestReview { .. } => self.requested.clone().map_or_default(|requested| OperationResult::Reviewers(Box::new(requested))),
      _ => OperationResult::Land(Box::new(LandResult {
        robot: self.robot.clone(),
        canceled: self.canceled.clone(),
        comment: self.commented.as_ref().map(|commented| StartComment {
          number: commented.number,
          url: commented.comment_url.clone(),
          accepted: commented.accepted,
        }),
        notes: self.planned.notes.clone(),
        lost: self.lost.clone(),
      })),
    };
    // the event of the robot of the subject: a pull request or a review request has no toast of a robot to replace
    let news_key = match &self.planned.plan.intent {
      Intent::CreatePullRequest(_) | Intent::RequestReview { .. } => None,
      // the toast of the pushes of the branch (of the stack branch of a whole stack, `stack:<root>`): a later push of it
      // takes its place
      Intent::Push(push) => Some(NewsKey::Push { branch: push.subject.key() }),
      _ => Some(NewsKey::RobotSubject { subject, mode }),
    };
    let outcome = Outcome {
      op,
      effects: self.records,
      news_key,
      version_after: self.version,
      result,
      undo: None,
      // what it cost is the registry's to fill: `RunningGuard::finish` of the app, the entry of the run of `bd`
      stats: OperationStats::default(),
    };
    Started {
      plan: self.planned.plan,
      outcome,
      retargets: self.retargets,
      canceled: self.canceled,
      pushed: self.pushed,
      robot: self.robot,
      restarted: self.restarted,
      commented: self.commented,
      created: self.created,
      requested: self.requested,
    }
  }
}

#[cfg(test)]
#[path = "land_execute_test.rs"]
pub(crate) mod land_execute_test;
