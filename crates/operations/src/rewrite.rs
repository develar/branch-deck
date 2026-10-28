//! The family `Rewrite` of the engine (docs/design/git-operations.md, "Rewrite in the engine"; docs/design/operations.md,
//! decision 3): a rewrite of the checked out branch or of the refs of Branch Deck, or its undo, as an operation of the
//! session, run by its core as `bd` and the commands of the app run it.
//!
//! A rewrite holds `Resource::Head`: the rewrites of a process run one after the other, in the order they came, each on
//! what the ones before it left (a second click of the same rewrite joins the first); between processes the repository
//! lock orders them, and a wait for another holder is the step `WaitingForLock` with its holder, which
//! `Registry::cancel` stops (`canceled`, nothing changed). A rewrite never waits for a sync: the sync of the app writes no
//! HEAD.
//!
//! An intent names its commits and branches on the table its window showed (`TableWitness`). Once admitted, the rewrite
//! reads what it names now (one `for-each-ref` of the checked out branch and the three stores, `read_witness`): HEAD is
//! the one of the witness, so its ids are the ones of HEAD; or a rewrite of this process moved HEAD since, and each id
//! goes through the new ids its core reported (`TableState::rewrites`); or the session's table is brought to HEAD (the
//! sync that runs, `SyncReason::Open`) and each id is the commit of the same author, author time and subject without
//! the prefix (`CommitKey`, read off the table when the request came). A commit none of them finds changes the plan
//! (`plan_changed`), and nothing is written. The core then runs on the blocking pool with that HEAD expected
//! (`RewriteOptions::expect_head`), under the repository lock it takes itself; the operation ends once it asked the sync
//! that shows what it changed (`SyncReason::AfterRewrite`), not when that sync ends, with what the core answered and its
//! undo (`Outcome::undo`); the request answers once that sync ended, with the version of its last patch
//! (`Outcome::version_after`), which the toast of the window waits for. An undo names no table: its token moves each ref
//! back with compare-and-swap.

use crate::engine::{Hosts, outcome_of, run};
use crate::events::EventSink;
use crate::registry::{Answer, OperationRequest, Registry, RunningGuard, Stepper};
use crate::witness::{Witness, plan_changed, read_refs};
use domain_types::branch_names::{BranchKind, qualified_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectRecord, EffectStatus, Intent, OperationResult, OperationStep, Outcome, Plan, Resource};
use domain_types::rewrite::{AppliedConflictFix, MoveCommitsResult, RewriteIntent, RewriteResult, TableWitness, UndoKind, UndoToken};
use domain_types::session::{StoreTips, SyncReason, Version};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::landed_pairing::CommitKey;
use git_ops::revs::short_id;
use history_rewrite::autosquash::{SquashFixupsParams, squash_fixup_commits_core};
use history_rewrite::issue_groups::ISSUE_GROUPS_REF;
use history_rewrite::move_commits::{MoveCommitsParams, move_commits_to_branch_core, move_result};
use history_rewrite::parked::{PARKED_REF, ParkLine, ParkOutcome, park_core, unpark_core};
use history_rewrite::regroup::{RegroupOptions, UndoToken as RegroupUndoToken, rename_branch_core, restack_branch_core, undo_regroup_core};
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, LockHolder, LockWait, LockWaiter, RewriteOptions};
use history_rewrite::reword::{RewordRequest, reword_request_core};
use history_rewrite::squash::{RestoreBranchHeadParams, SquashCommitsParams, restore_branch_head_core, squash_commits_core};
use history_rewrite::stack_parents::STACK_PARENTS_REF;
use object_cache::cache::ObjectCache;
use repository_model::session::{SessionKey, Sessions, SyncTicket};
use repository_model::table::{RewriteLink, TableState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::add_issue_reference::{AddIssueReferenceParams, add_issue_reference_to_commits_core};
use sync_core::amend_to_branch::{AmendUncommittedToBranchParams, amend_uncommitted_to_branch_core};
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core, delete_unapplied_branch_core};
use sync_core::parked::park_result;
use sync_core::reapply_branch::{ReapplyBranchParams, ReapplyUndoToken, reapply_branch_core, undo_reapply_core};
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyUndoToken, unapply_branch_core, undo_unapply_core};
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};
use trial_check::conflict_fix::{ApplyConflictFixParams, apply_conflict_fix_core};
use update_core::pick::{PickRemoteCommitsParams, UndoPickRemoteCommitsParams, pick_remote_commits_core, undo_pick_remote_commits_core};
use update_core::update_project::{UpdateUndoToken, undo_update_core};

/// What the rewrites of a host run with besides the services of its reads (`ServiceHost`: the git, the object cache):
/// the diffs a pick reads, and a core's failure as the commands of the host answer it
pub trait RewriteHost: Send + Sync {
  /// The diffs of the commits a pick of remote commits reads; the app keeps one cache for every window
  fn file_diffs(&self) -> FileDiffs {
    FileDiffs::default()
  }

  /// The failure of a core as the commands of the host answer it: the code of its typed error
  /// (`app_api::api_error::api_error`)
  fn rewrite_error(&self, error: anyhow::Error) -> ApiError;

  /// The failure of an undo as the commands of the host answer it (`app_api::api_error::undo_error`)
  fn undo_error(&self, error: anyhow::Error) -> ApiError;
}

/// The plan of a rewrite over the table of its session, and what its run resolves the ids it names by
struct RewritePlan {
  plan: Plan,
  /// The commits it names, by original id, as the table has them: what finds each of them after a rewrite of another
  /// process or one the session has no new ids of
  keys: HashMap<String, CommitKey>,
}

/// The plan of `rewrite` over the table of the session of `key` (`engine::prepare`): its core rewrites what it names,
/// then the sync shows it (none after a delete, which leaves the table at once); `None` for a session that is not open
pub async fn prepare_rewrite(sessions: &Sessions, key: &SessionKey, rewrite: RewriteIntent) -> Option<Plan> {
  sessions.read(key, move |state, _| rewrite_plan(state, &rewrite).plan).await
}

/// `prepare_rewrite` on the table state `state`
fn rewrite_plan(state: &TableState, rewrite: &RewriteIntent) -> RewritePlan {
  let commits = named_commits(state, rewrite);
  let keys = state
    .commits()
    .filter(|commit| commits.contains(&commit.id))
    .map(|commit| (commit.id.clone(), CommitKey::from_subject(&commit.author_email, commit.author_timestamp, &commit.subject)))
    .collect();
  let mut effects = vec![PlannedEffect::Rewrite {
    commits,
    branches: named_branches(rewrite),
  }];
  if !is_delete(rewrite) {
    effects.push(PlannedEffect::Sync);
  }
  let intent = Intent::Rewrite(Box::new(rewrite.clone()));
  let fingerprint = Witness::default().fingerprint(&(&intent, state.header().witness()));
  let plan = Plan {
    intent,
    fingerprint,
    lands: Vec::new(),
    effects,
    confirmations: Vec::new(),
    blockers: Vec::new(),
    waits: Vec::new(),
    alternatives: None,
    pull_request: None,
    approvals_skipped_by: None,
  };
  RewritePlan { plan, keys }
}

/// The commits `rewrite` names, original ids: those of its request; of an ungroup every commit of its branch as `state`
/// has it
fn named_commits(state: &TableState, rewrite: &RewriteIntent) -> Vec<String> {
  match rewrite {
    RewriteIntent::Move { commit_ids, .. } | RewriteIntent::Squash { commit_ids, .. } => commit_ids.clone(),
    RewriteIntent::Reword { rewords, .. } => rewords.iter().map(|reword| reword.commit.clone()).collect(),
    RewriteIntent::AddIssueReference { commits, .. } => commits.iter().map(|commit| commit.hash.clone()).collect(),
    RewriteIntent::Amend { original_commit_id, .. } => vec![original_commit_id.clone()],
    RewriteIntent::Ungroup { branch_name, .. } => state
      .branch(branch_name)
      .map_or_default(|branch| branch.commits.iter().map(|commit| commit.id.clone()).collect()),
    _ => Vec::new(),
  }
}

/// The branches `rewrite` names
fn named_branches(rewrite: &RewriteIntent) -> Vec<String> {
  match rewrite {
    RewriteIntent::Move { target_branch, .. } => target_branch.iter().cloned().collect(),
    RewriteIntent::Rename { branch_name, new_name, .. } => vec![branch_name.clone(), new_name.clone()],
    RewriteIntent::Restack { branch_name, parent, .. } => vec![branch_name.clone(), parent.clone()],
    RewriteIntent::Park { branches, .. } => branches.iter().map(|line| line.branch_name.clone()).collect(),
    RewriteIntent::Unpark { branch_names, .. } => branch_names.clone(),
    _ => rewrite.subject().map(str::to_owned).into_iter().collect(),
  }
}

/// A delete of an archived or an unapplied branch: the table drops it at once, no sync runs after it
const fn is_delete(rewrite: &RewriteIntent) -> bool {
  matches!(rewrite, RewriteIntent::DeleteArchived { .. } | RewriteIntent::DeleteUnapplied { .. })
}

/// What `rewrite` in a session of `prefix` asks the registry for: HEAD, but for a delete, which moves no HEAD, and the
/// kept branch a reapply or a delete moves (so a push or a delete of it waits); no virtual branch, whose push runs beside
/// a rewrite and waits for its sync itself
pub(crate) fn rewrite_request(prefix: &str, rewrite: &RewriteIntent) -> OperationRequest {
  let mut resources = Vec::new();
  if !is_delete(rewrite) {
    resources.push(Resource::Head);
  }
  let kept = match rewrite {
    RewriteIntent::DeleteArchived { branch_name, .. } | RewriteIntent::DeleteUnapplied { branch_name, .. } => Some(branch_name.clone()),
    RewriteIntent::Reapply { branch_name, .. } => qualified_branch_name(prefix, BranchKind::Unapplied, branch_name).ok(),
    _ => None,
  };
  resources.extend(kept.map(|name| Resource::Ref { name }));
  OperationRequest {
    intent: Intent::Rewrite(Box::new(rewrite.clone())),
    resources,
    step: OperationStep::Checking,
    moves: Vec::new(),
  }
}

/// Runs `rewrite` in the session of `key` over the services of `host` (the module's header): admitted on HEAD (a second
/// click joins it, another rewrite waits in the queue), what it names resolved on HEAD as it is now, its core on the
/// blocking pool with that HEAD expected and the lock waited for as long as its holder lives (`WaitingForLock`, stopped
/// by `Registry::cancel`), then the sync asked; its steps in the view, its outcome to `events`. The operation ends with
/// the request of that sync, so a rewrite queued behind it runs beside the sync; the answer comes once the sync ended,
/// with the version of its last patch (`Outcome::version_after`), so the window that asked shows its toast beside what
/// it changed. A request that joined the operation answers as it ended, without that version
///
/// # Errors
/// What its core refused, with the code of its typed error; `plan_changed` for a commit no longer there; `canceled`
/// after a stopped wait for the lock: nothing was written
#[instrument(skip_all, fields(subject = ?rewrite.subject()))]
pub async fn execute_rewrite(registry: &Registry, key: &SessionKey, rewrite: RewriteIntent, hosts: Hosts<'_>, events: Arc<dyn EventSink>) -> Answer {
  let sessions = registry.sessions();
  let request = rewrite_request(&key.prefix, &rewrite);
  // what it names, read off the table of the click before a rewrite before it changes the table
  let intent = rewrite.clone();
  let planned = sessions.read(key, move |state, _| rewrite_plan(state, &intent)).await;
  let asked = Mutex::new(None);
  let mut answer = run(registry, key, request, events, async |guard| {
    // no session was open: what the intent itself names
    let planned = planned.unwrap_or_else(|| rewrite_plan(&TableState::new(), &rewrite));
    let (resolved, expect_head) = resolve(sessions, key, hosts, &rewrite, &planned).await?;
    let ran = run_core(guard, key, hosts, &resolved, expect_head.clone()).await?;
    let (outcome, sync) = finish(sessions, key, guard, &planned.plan, &resolved, expect_head, ran).await;
    *asked.lock().unwrap_or_else(PoisonError::into_inner) = sync;
    Ok(outcome)
  })
  .await;
  let sync = asked.into_inner().unwrap_or_else(PoisonError::into_inner);
  if let (Ok(outcome), Some(sync)) = (&mut answer, sync) {
    // a sync that failed or was deferred shows nothing new: the toast comes at once
    outcome.version_after = sync.wait().await.ok().map(|answer| answer.version);
  }
  answer
}

/// HEAD and the stores as the table of the session of `key` knows them, and the rewrites of this process since
struct Seen {
  /// The checked out branch of the last grouping; empty before it
  branch: String,
  header: TableWitness,
  links: Vec<RewriteLink>,
}

/// What the session knows of HEAD now
async fn seen(sessions: &Sessions, key: &SessionKey) -> Option<Seen> {
  sessions
    .read(key, |state, _| Seen {
      branch: state.header().branch.clone(),
      header: state.header().witness(),
      links: state.rewrites().to_vec(),
    })
    .await
}

/// `rewrite` with the ids it names on HEAD as it is now (an ungroup as the move back of the commits of its branch), and
/// the HEAD its core expects (the module's header): an undo as it is; a rewrite whose session never grouped as it was
/// asked, with the HEAD of its witness
///
/// # Errors
/// `plan_changed` for a commit that is gone; a failed read of git
async fn resolve(sessions: &Sessions, key: &SessionKey, hosts: Hosts<'_>, rewrite: &RewriteIntent, planned: &RewritePlan) -> Result<(RewriteIntent, Option<String>), ApiError> {
  let Some(asked) = rewrite.witness() else {
    return Ok((rewrite.clone(), None));
  };
  let Some(seen) = seen(sessions, key).await.filter(|seen| !seen.branch.is_empty()) else {
    return Ok((ungrouped(rewrite, planned_commits(&planned.plan)), Some(asked.head.clone())));
  };
  let (git, path, branch) = (hosts.services.git(), key.worktree.clone(), seen.branch.clone());
  let now = blocking(move || read_witness(&git, &path, &branch).map_err(|error| ApiError::new(ErrorCode::GitError, format!("{error:#}")))).await?;
  if now.head == asked.head {
    return Ok((ungrouped(rewrite, planned_commits(&planned.plan)), Some(now.head)));
  }
  let named = named_commits_of(rewrite);
  // a branch goes by its name
  if named.is_empty() && !matches!(rewrite, RewriteIntent::Ungroup { .. }) {
    return Ok((rewrite.clone(), Some(now.head)));
  }
  if !named.is_empty()
    && let Some(ids) = through_links(&seen.links, &asked.head, &now.head, &named)
  {
    debug!(from = %short_id(&asked.head), to = %short_id(&now.head), "the ids go through the rewrites of this process");
    return Ok((with_commits(rewrite, &ids), Some(now.head)));
  }
  // the table of the session at HEAD: the sync in flight (the one a rewrite before asked), or one that reads it
  let mut table = seen.header;
  for _ in 0..SYNCS_TO_HEAD {
    if table.head == now.head {
      break;
    }
    if let Err(error) = sessions.sync(key, SyncReason::Open).wait().await {
      debug!(error = %error.message, "the sync before the rewrite failed");
    }
    table = seen_header(sessions, key).await.unwrap_or_default();
  }
  let intent = Intent::Rewrite(Box::new(rewrite.clone()));
  if table.head != now.head {
    return Err(plan_changed(&intent, Some("HEAD moved, and no sync shows it yet")));
  }
  let (named, keys, of) = (named.clone(), planned.keys.clone(), rewrite.clone());
  let found = sessions
    .read(key, move |state, _| {
      if matches!(of, RewriteIntent::Ungroup { .. }) {
        // its commits are the ones of its branch now
        return Ok(named_commits(state, &of));
      }
      // a key two commits share finds neither
      let mut by_key: HashMap<CommitKey, Option<&str>> = HashMap::new();
      for commit in state.commits() {
        let key = CommitKey::from_subject(&commit.author_email, commit.author_timestamp, &commit.subject);
        by_key.entry(key).and_modify(|found| *found = None).or_insert(Some(commit.id.as_str()));
      }
      named
        .iter()
        .map(|id| keys.get(id).and_then(|key| by_key.get(key).copied().flatten()).map(str::to_owned).ok_or_else(|| id.clone()))
        .collect::<Result<Vec<String>, String>>()
    })
    .await
    .unwrap_or_else(|| Err(String::new()));
  match found {
    Ok(ids) if ids.is_empty() => Err(plan_changed(&intent, Some(&format!("{} has no commits", rewrite.subject().unwrap_or_default())))),
    Ok(ids) => Ok((with_commits(&ungrouped(rewrite, ids.clone()), &ids), Some(now.head))),
    Err(gone) => Err(plan_changed(&intent, Some(&format!("{} is no commit ahead of the baseline any more", short_id(&gone))))),
  }
}

/// The commits of the rewrite effect of `plan`
fn planned_commits(plan: &Plan) -> Vec<String> {
  plan
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::Rewrite { commits, .. } => Some(commits.clone()),
      _ => None,
    })
    .unwrap_or_default()
}

/// An ungroup as the move back of `commits`, the commits of its branch; any other rewrite as it is
fn ungrouped(rewrite: &RewriteIntent, commits: Vec<String>) -> RewriteIntent {
  match rewrite {
    RewriteIntent::Ungroup { witness, .. } => RewriteIntent::Move {
      commit_ids: commits,
      target_branch: None,
      witness: witness.clone(),
    },
    _ => rewrite.clone(),
  }
}

/// The commits `rewrite` itself names (an ungroup names a branch)
fn named_commits_of(rewrite: &RewriteIntent) -> Vec<String> {
  named_commits(&TableState::new(), rewrite)
}

/// `rewrite` naming `ids` in place of the commits it named, in their order
fn with_commits(rewrite: &RewriteIntent, ids: &[String]) -> RewriteIntent {
  let mut rewrite = rewrite.clone();
  match &mut rewrite {
    RewriteIntent::Move { commit_ids, .. } | RewriteIntent::Squash { commit_ids, .. } => commit_ids.clone_from(&ids.to_vec()),
    RewriteIntent::Reword { rewords, .. } => rewords.iter_mut().zip(ids).for_each(|(reword, id)| reword.commit.clone_from(id)),
    RewriteIntent::AddIssueReference { commits, .. } => commits.iter_mut().zip(ids).for_each(|(commit, id)| commit.hash.clone_from(id)),
    RewriteIntent::Amend { original_commit_id, .. } => {
      if let Some(id) = ids.first() {
        original_commit_id.clone_from(id);
      }
    }
    _ => {}
  }
  rewrite
}

/// How many syncs a rewrite whose HEAD moved waits for until the table shows that HEAD: the one in flight may have read
/// the HEAD before
const SYNCS_TO_HEAD: usize = 3;

/// HEAD and the stores of the table of the session of `key`
async fn seen_header(sessions: &Sessions, key: &SessionKey) -> Option<TableWitness> {
  sessions.read(key, |state, _| state.header().witness()).await
}

/// HEAD (the tip of `branch`, the checked out branch) and the tips of the stores now: one `for-each-ref`
fn read_witness(git: &GitCommandExecutor, path: &str, branch: &str) -> anyhow::Result<TableWitness> {
  let head_ref = format!("refs/heads/{branch}");
  let refs = [head_ref.clone(), ISSUE_GROUPS_REF.to_owned(), STACK_PARENTS_REF.to_owned(), PARKED_REF.to_owned()];
  let mut tips = read_refs(git, path, &refs)?;
  let mut tip = |name: &str| tips.remove(name).flatten();
  Ok(TableWitness {
    head: tip(&head_ref).unwrap_or_default(),
    store_tips: StoreTips {
      issue_groups: tip(ISSUE_GROUPS_REF),
      stack_parents: tip(STACK_PARENTS_REF),
      parked: tip(PARKED_REF),
    },
  })
}

/// The ids of `named` after the rewrites of `links` that led HEAD from `from` to `to`, each id through the new ids of
/// each rewrite (one that only added commits keeps the others); `None` when no chain of them leads there, or one of
/// them may have given an id another id it does not name
fn through_links(links: &[RewriteLink], from: &str, to: &str, named: &[String]) -> Option<Vec<String>> {
  let mut head = from;
  let mut ids = named.to_vec();
  for link in links {
    if head == to {
      break;
    }
    if link.from != head {
      continue;
    }
    ids = ids
      .into_iter()
      .map(|id| link.rewritten.get(&id).cloned().or_else(|| link.appends.then_some(id)))
      .collect::<Option<Vec<_>>>()?;
    head = &link.to;
  }
  (head == to).then_some(ids)
}

/// What the core of a rewrite answered, and how its outcome is undone
struct Ran {
  result: RewriteResult,
  undo: Option<UndoToken>,
}

impl Ran {
  /// `result` undone by the token `token` of `kind`, when the core printed one
  fn of(result: RewriteResult, kind: UndoKind, token: Option<String>) -> Self {
    Self {
      result,
      undo: token.map(|token| UndoToken { kind, token }),
    }
  }
}

/// How a core failed: a rewrite or an undo, worded apart (`RewriteHost`)
enum Failure {
  Rewrite(anyhow::Error),
  Undo(anyhow::Error),
}

/// Runs the core of `rewrite` (resolved: ids on HEAD, an ungroup as a move) for the operation of `guard` on the blocking
/// pool, with `expect_head` and the lock waited for as long as its holder lives (`StepWaiter`)
///
/// # Errors
/// The core's failure as the host words it; `canceled` once the user stopped the wait for the lock
async fn run_core(guard: &RunningGuard, key: &SessionKey, hosts: Hosts<'_>, rewrite: &RewriteIntent, expect_head: Option<String>) -> Result<Ran, ApiError> {
  guard.step(OperationStep::Rewriting);
  let cache = match rewrite {
    RewriteIntent::PickRemote { .. } => Some(hosts.services.object_cache(key).await),
    _ => None,
  };
  let core = Core {
    git: hosts.services.git(),
    path: key.worktree.clone(),
    prefix: key.prefix.clone(),
    expect_head,
    cache,
    file_diffs: hosts.rewrites.file_diffs(),
  };
  let waiter = Arc::new(StepWaiter::new(guard, OperationStep::Rewriting));
  let (resolved, core_waiter) = (rewrite.clone(), Arc::clone(&waiter));
  let ran = blocking(move || Ok(core.run(&resolved, &*core_waiter))).await?;
  match ran {
    Ok(ran) => Ok(ran),
    // nothing was read or written: whatever the core answered after the wait
    Err(_) if waiter.ended_by_cancel() => Err(wait_stopped()),
    Err(Failure::Rewrite(error)) => Err(hosts.rewrites.rewrite_error(error)),
    Err(Failure::Undo(error)) => Err(hosts.rewrites.undo_error(error)),
  }
}

/// The wait of a rewrite or an update for the repository lock, told as the step of its operation: `WaitingForLock`
/// with its holder, then the step it goes on at once it ended (`then`); stopped by `Registry::cancel` of the operation
pub(crate) struct StepWaiter {
  stepper: Stepper,
  canceled: Arc<AtomicBool>,
  /// The wait ended because the user stopped it (a stop after the lock was taken ends nothing)
  ended_by_cancel: AtomicBool,
  then: OperationStep,
}

impl StepWaiter {
  /// The waiter of the operation of `guard`, which goes on at `then` after its wait
  pub(crate) fn new(guard: &RunningGuard, then: OperationStep) -> Self {
    Self {
      stepper: guard.stepper(),
      canceled: guard.canceled(),
      ended_by_cancel: AtomicBool::new(false),
      then,
    }
  }

  /// Whether the wait ended because the user stopped it: nothing was read or written after it
  pub(crate) fn ended_by_cancel(&self) -> bool {
    self.ended_by_cancel.load(Ordering::SeqCst)
  }
}

/// The error of an operation whose wait for the repository lock the user stopped: nothing changed
pub(crate) fn wait_stopped() -> ApiError {
  ApiError::new(ErrorCode::Canceled, "The wait for the repository lock was stopped; nothing changed")
}

impl LockWaiter for StepWaiter {
  fn holder(&self, holder: Option<&LockHolder>) {
    self.stepper.step(match holder {
      Some(holder) => OperationStep::WaitingForLock { holder: Some(holder.info()) },
      None => self.then.clone(),
    });
  }

  fn canceled(&self) -> bool {
    let canceled = self.canceled.load(Ordering::SeqCst);
    if canceled {
      self.ended_by_cancel.store(true, Ordering::SeqCst);
    }
    canceled
  }
}

/// What a core runs on
struct Core {
  git: GitCommandExecutor,
  path: String,
  prefix: String,
  /// The HEAD the core expects, as the rewrite resolved it; `None` for an undo, whose token holds its own
  expect_head: Option<String>,
  /// The object cache of a pick
  cache: Option<ObjectCache>,
  file_diffs: FileDiffs,
}

/// `work` on the blocking pool, in the span of the operation
pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, ApiError> + Send + 'static) -> Result<T, ApiError> {
  match tokio::task::spawn_blocking(in_current_span(work)).await {
    Ok(answer) => answer,
    Err(error) => Err(ApiError::internal(format!("The operation stopped: {error}"))),
  }
}

impl Core {
  /// The core of `rewrite`, its lock waited for by `waiter`
  fn run(&self, rewrite: &RewriteIntent, waiter: &dyn LockWaiter) -> Result<Ran, Failure> {
    let lock_wait = LockWait::WhileHeld(waiter);
    let options = RewriteOptions {
      expect_head: self.expect_head.clone(),
      lock_wait,
      held_lock: None,
    };
    let regroup = RegroupOptions {
      expect_head: self.expect_head.clone(),
      lock_wait,
      ..RegroupOptions::default()
    };
    let (git, path) = (&self.git, self.path.as_str());
    let failed = |error: anyhow::Error| Failure::Rewrite(error);
    let moved = |result: MoveCommitsResult| {
      let token = result.undo_token.clone();
      Ran::of(RewriteResult::Moved(result), UndoKind::Regroup, token)
    };
    Ok(match rewrite {
      RewriteIntent::Move { commit_ids, target_branch, .. } => {
        let params = MoveCommitsParams {
          expect_head: self.expect_head.clone(),
          ..MoveCommitsParams::new(path, commit_ids.clone(), target_branch.as_deref())
        };
        moved(move_commits_to_branch_core(git, &params, lock_wait).map_err(failed)?)
      }
      RewriteIntent::Rename { branch_name, new_name, .. } => {
        let outcome = rename_branch_core(git, path, branch_name, new_name, &regroup).map_err(|error| failed(error.into()))?;
        moved(move_result(&outcome, &[], Some(new_name.clone())))
      }
      RewriteIntent::Restack { branch_name, parent, .. } => {
        let outcome = restack_branch_core(git, path, branch_name, Some(parent), &regroup).map_err(|error| failed(error.into()))?;
        moved(move_result(&outcome, &[], Some(branch_name.clone())))
      }
      RewriteIntent::Unstack { branch_name, .. } => {
        let outcome = restack_branch_core(git, path, branch_name, None, &regroup).map_err(|error| failed(error.into()))?;
        moved(move_result(&outcome, &[], Some(branch_name.clone())))
      }
      RewriteIntent::Park { branches, .. } => {
        let lines: Vec<ParkLine<'_>> = branches
          .iter()
          .map(|line| ParkLine {
            branch: &line.branch_name,
            note: &line.note,
          })
          .collect();
        parked(park_core(git, path, &lines, &options).map_err(|error| failed(error.into()))?)
      }
      RewriteIntent::Unpark { branch_names, .. } => {
        let names: Vec<&str> = branch_names.iter().map(String::as_str).collect();
        parked(unpark_core(git, path, &names, &options).map_err(|error| failed(error.into()))?)
      }
      RewriteIntent::Unapply {
        branch_name, original_commit_ids, ..
      } => {
        let params = UnapplyBranchParams {
          repository_path: path.to_owned(),
          branch_name: branch_name.clone(),
          original_commit_ids: original_commit_ids.clone(),
        };
        let result = unapply_branch_core(git, params, &self.prefix, &options).map_err(failed)?;
        let token = result.undo_token.clone();
        Ran::of(RewriteResult::Unapplied(result), UndoKind::Unapply, Some(token))
      }
      RewriteIntent::Reapply { branch_name, .. } => {
        let params = ReapplyBranchParams {
          repository_path: path.to_owned(),
          branch_name: branch_name.clone(),
          dry_run: false,
        };
        let result = reapply_branch_core(git, &params, &self.prefix, &options).map_err(failed)?;
        let token = result.undo_token.clone();
        Ran::of(RewriteResult::Reapplied(result), UndoKind::Reapply, token)
      }
      RewriteIntent::Squash { commit_ids, message, .. } => {
        let params = SquashCommitsParams {
          repository_path: path.to_owned(),
          commit_ids: commit_ids.clone(),
          message: message.clone(),
        };
        let result = squash_commits_core(git, &params, &options).map_err(failed)?;
        let token = head_token(&result.previous_head, &result.new_head);
        Ran::of(RewriteResult::Squashed(result), UndoKind::Squash, token)
      }
      RewriteIntent::SquashFixups { scope, .. } => {
        let params = SquashFixupsParams {
          repository_path: path.to_owned(),
          expected_head: self.expect_head.clone().unwrap_or_default(),
          scope: scope.clone(),
        };
        let result = squash_fixup_commits_core(git, &params, &options).map_err(failed)?;
        let token = head_token(&result.previous_head, &result.new_head);
        Ran::of(RewriteResult::FixupsSquashed(result), UndoKind::Squash, token)
      }
      RewriteIntent::Reword { rewords, .. } => {
        let request = RewordRequest {
          repository_path: path.to_owned(),
          rewords: rewords.clone(),
        };
        let result = reword_request_core(git, &request, &regroup).map_err(failed)?;
        let token = result.undo_token.clone();
        Ran::of(RewriteResult::Reworded(result), UndoKind::Regroup, token)
      }
      RewriteIntent::Amend {
        branch_name,
        original_commit_id,
        files,
        ..
      } => {
        let params = AmendUncommittedToBranchParams {
          repository_path: path.to_owned(),
          branch_name: branch_name.clone(),
          original_commit_id: original_commit_id.clone(),
          files: files.clone(),
        };
        let result = amend_uncommitted_to_branch_core(git, params, &options).map_err(|error| failed(anyhow::anyhow!(error)))?;
        Ran::of(RewriteResult::Amended(result), UndoKind::Regroup, None)
      }
      RewriteIntent::AddIssueReference {
        branch_name,
        commits,
        issue_reference,
        ..
      } => {
        let params = AddIssueReferenceParams {
          repository_path: path.to_owned(),
          branch_name: branch_name.clone(),
          commits: commits.clone(),
          issue_reference: issue_reference.clone(),
        };
        let result = add_issue_reference_to_commits_core(git, params, &options).map_err(|error| failed(anyhow::anyhow!(error)))?;
        Ran::of(RewriteResult::IssueReferenceAdded(result), UndoKind::Regroup, None)
      }
      RewriteIntent::ApplyConflictFix { fingerprint, plan, .. } => {
        let params = ApplyConflictFixParams {
          repository_path: path.to_owned(),
          fingerprint: fingerprint.clone(),
          plan: plan.clone(),
        };
        let applied = apply_conflict_fix_core(git, &params, lock_wait).map_err(failed)?;
        let token = match &applied {
          AppliedConflictFix::Applied { result } => result.undo_token.clone(),
          AppliedConflictFix::Stale => None,
        };
        Ran::of(RewriteResult::ConflictFixApplied(applied), UndoKind::Regroup, token)
      }
      RewriteIntent::DeleteArchived { branch_name, .. } => {
        delete_archived_branch_core(git, kept(path, branch_name), &self.prefix).map_err(failed)?;
        Ran::of(RewriteResult::Deleted, UndoKind::Regroup, None)
      }
      RewriteIntent::DeleteUnapplied { branch_name, .. } => {
        delete_unapplied_branch_core(git, kept(path, branch_name), &self.prefix).map_err(failed)?;
        Ran::of(RewriteResult::Deleted, UndoKind::Regroup, None)
      }
      RewriteIntent::PickRemote {
        branch_name,
        parent_branch,
        my_email,
        expected_remote,
        ..
      } => {
        let params = PickRemoteCommitsParams {
          repository_path: path.to_owned(),
          branch_name: branch_name.clone(),
          parent_branch: parent_branch.clone(),
          my_email: my_email.clone(),
          expected_remote: expected_remote.clone(),
        };
        let cache = self.cache.clone().unwrap_or_else(ObjectCache::disabled);
        let result = pick_remote_commits_core(git, &params, &self.prefix, &options, &cache, &self.file_diffs).map_err(failed)?;
        let token = result.undo_token.clone();
        Ran::of(RewriteResult::Picked(result), UndoKind::Pick, token)
      }
      // resolved into a move back of the commits of its branch
      RewriteIntent::Ungroup { .. } => return Err(failed(anyhow::anyhow!("An ungroup runs as the move back of its commits"))),
      RewriteIntent::Undo { token } => self.undo(token, lock_wait).map_err(Failure::Undo)?,
    })
  }

  /// The undo of `token`, by the core of its kind
  fn undo(&self, token: &UndoToken, lock_wait: LockWait<'_>) -> anyhow::Result<Ran> {
    let (git, path) = (&self.git, self.path.as_str());
    let text = token.token.as_str();
    let not_a_token = || anyhow::anyhow!("Not an undo token of a {:?}: {text}", token.kind);
    let already_undone = match token.kind {
      UndoKind::Regroup => {
        undo_regroup_core(git, path, &RegroupUndoToken::parse(text).ok_or_else(not_a_token)?, lock_wait)?;
        false
      }
      UndoKind::Squash => {
        let (previous_head, expected_head) = text.split_once("..").ok_or_else(not_a_token)?;
        let params = RestoreBranchHeadParams {
          repository_path: path.to_owned(),
          expected_head: expected_head.to_owned(),
          previous_head: previous_head.to_owned(),
        };
        restore_branch_head_core(git, &params, lock_wait)?;
        false
      }
      UndoKind::Unapply => undo_unapply_core(git, path, &UnapplyUndoToken::parse(text).ok_or_else(not_a_token)?, lock_wait)?.already_undone,
      UndoKind::Reapply => undo_reapply_core(git, path, &ReapplyUndoToken::parse(text).ok_or_else(not_a_token)?, lock_wait)?.already_undone,
      UndoKind::Pick => {
        let params = UndoPickRemoteCommitsParams {
          repository_path: path.to_owned(),
          undo_token: text.to_owned(),
        };
        // the undo of a pick waits a bounded time, as `bd undo` does
        undo_pick_remote_commits_core(git, &params, DEFAULT_LOCK_TIMEOUT)?;
        false
      }
      // its short lock, as `bd undo` takes it
      UndoKind::Update => undo_update_core(git, path, &UpdateUndoToken::parse(text).ok_or_else(not_a_token)?, DEFAULT_LOCK_TIMEOUT)?.already_undone,
    };
    Ok(Ran::of(RewriteResult::Undone { already_undone }, token.kind, None))
  }
}

/// What a park or an unpark answered, and its undo: the store back to the commit it was written on, with
/// compare-and-swap (`parked:` of a regroup token); none when nothing changed
fn parked(outcome: ParkOutcome) -> Ran {
  let token = outcome.new_commit.clone().map(|new| {
    RegroupUndoToken {
      head: None,
      issue_groups: None,
      stack_parents: None,
      parked: Some((outcome.previous_commit.clone(), new)),
    }
    .to_string()
  });
  Ran::of(RewriteResult::Parked(park_result(outcome)), UndoKind::Regroup, token)
}

/// The undo token of a rewrite that moved HEAD from `previous` to `new` and nothing else (a squash): none when it moved
/// nothing
fn head_token(previous: &str, new: &str) -> Option<String> {
  (previous != new).then(|| format!("{previous}..{new}"))
}

/// The request of the delete of the kept branch `branch_name` (its full name) of the worktree `path`
fn kept(path: &str, branch_name: &str) -> DeleteArchivedBranchParams {
  DeleteArchivedBranchParams {
    repository_path: path.to_owned(),
    branch_name: branch_name.to_owned(),
  }
}

/// The end of the rewrite `resolved` of the operation of `guard` planned as `plan`, whose core answered `ran` on HEAD
/// `expect_head`: the link of HEAD it moved, for the rewrites queued behind it (`TableState::record_rewrite`), then the
/// sync that shows what it changed, asked and not waited for (a delete: the table drops the branch at once); its outcome
/// and the ticket of that sync
async fn finish(
  sessions: &Sessions,
  key: &SessionKey,
  guard: &RunningGuard,
  plan: &Plan,
  resolved: &RewriteIntent,
  expect_head: Option<String>,
  ran: Ran,
) -> (Outcome, Option<SyncTicket>) {
  if let (Some(from), Some(to)) = (expect_head, ran.result.head())
    && from != to
  {
    let link = RewriteLink {
      from,
      to: to.to_owned(),
      rewritten: ran.result.rewritten(),
      appends: matches!(ran.result, RewriteResult::Picked(_) | RewriteResult::Reapplied(_)),
    };
    drop(sessions.apply(key, move |state| state.record_rewrite(link)));
  }
  let rewrite = PlannedEffect::Rewrite {
    commits: if named_commits_of(resolved).is_empty() {
      planned_commits(plan)
    } else {
      named_commits_of(resolved)
    },
    branches: named_branches(resolved),
  };
  let mut effects = vec![EffectRecord::new(rewrite, EffectStatus::Done)];
  let (version_after, sync) = match resolved {
    RewriteIntent::DeleteArchived { branch_name, .. } => (removed(sessions, key, branch_name, TableState::remove_archived).await, None),
    RewriteIntent::DeleteUnapplied { branch_name, .. } => (removed(sessions, key, branch_name, TableState::remove_unapplied).await, None),
    _ => {
      // the operation ends with the request: the sync is the session's, and a rewrite after this one runs beside it
      let sync = ran.result.changes().then(|| sessions.sync(key, SyncReason::AfterRewrite));
      let status = if sync.is_some() { EffectStatus::Done } else { EffectStatus::Skipped { reason: None } };
      effects.push(EffectRecord::new(PlannedEffect::Sync, status));
      (None, sync)
    }
  };
  let outcome = Outcome {
    undo: ran.undo,
    ..outcome_of(guard.id().clone(), effects, None, version_after, OperationResult::Rewrite(Box::new(ran.result)))
  };
  (outcome, sync)
}

/// The kept branch `name` leaves the table of the session of `key` by `remove`: the version of the patch
async fn removed(sessions: &Sessions, key: &SessionKey, name: &str, remove: fn(&mut TableState, &str)) -> Option<Version> {
  let name = name.to_owned();
  sessions.apply(key, move |state| remove(state, &name)).version().await
}

#[cfg(test)]
#[path = "rewrite_test.rs"]
mod rewrite_test;
