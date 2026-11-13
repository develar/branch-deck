//! The plans of the reviews read ahead after a rewrite (openspec/specs/branch/review-plan/spec.md, "In the App";
//! docs/design/change-review.md, "Prefetch after a rewrite"): a sync that commits the table of a repository
//! (`SessionHost::table_committed`) reads again, in the background, the plans of the branches a review window asked
//! since the app started whose head the sync moved, so that the window that opens their review again finds its plan in
//! the memory of the plans, and accio its store of the new tree warm. A rewrite of Branch Deck (`brd update`, a pick, a
//! fold) replaces every commit of a branch, and the first plan after it is the cold one.
//!
//! - **What a window asked** (`Prefetch::asked`): the last plan of a scope it asked of a branch, by the name the window
//!   asked it, its base as the stack parent whose head it is (`ResolvedSubject::stack_parent`) or else the baseline,
//!   "Since Your Review", and the head it was asked at: the same subject resolved after the rewrite is the key the window
//!   asks. A range, a commit and a kept branch name commits a rewrite replaces: none is read ahead. At most the
//!   `WATCHED` branches of a repository asked last: a branch asked again is the newest, a fifth one forgets the one
//!   asked longest ago. `brd update` moves every branch at once, and the reviews opened again are mostly the recent
//!   ones, so a day of reviews read again on each update (15 branches, about 17 min of the processors at nice 19 on
//!   the IntelliJ monorepo) would cost more than it gives.
//! - **What a commit reads** (`Prefetch::committed`): the branches whose head the table moved, but not one a window
//!   shows now (it reads its own plan once it sees the new head, at the priority of the app), the most recently asked
//!   first, one at a time. A branch that left the table, or that no longer resolves, is forgotten. Nothing without the
//!   flag of the repository or an index on the machine: a plan without its usages is not kept.
//! - **Below the app** (`ChildPriority::Background`): its queries of accio run at nice 19 (below normal on Windows); its
//!   git runs as the app's.
//! - **A newer commit of the repository supersedes** the read ahead in flight: its queries are killed, and the newer one
//!   reads what is left to read.
//! - **It yields to a window**: a window that asks the plan being read ahead joins that reading, which then ends
//!   without a plan, its queries killed, and the window reads the plan itself at the priority of the app
//!   (`PlanReader::read`); accio keeps what the killed query resolved. The read ahead goes on with its next branch.
//! - **No memory of its own**: the plans go into the memory of the plans (`ReviewPlans`), the only one of Branch Deck
//!   (docs/design/change-review.md, decision 16): the speed comes from accio.
//!
//! The waiting requests of Your Turn are read ahead too (`read_ahead_requests`; openspec/specs/repository/review-requests/spec.md,
//! "Read ahead"): each listing of the review requests of a session names the waiting requests it has no last reading of
//! (`SessionHost::review_requests_listed`), and what each leaves to read goes to the session
//! (`Sessions::review_readings`), whose rows show it before any window opens.
//!
//! - **One read for all of them**, a phase a process: the requests whose commits are here are paired (one `cat-file`,
//!   one `rev-parse` of the merge bases, `sync_core::review_fetch::local_review_pairs`) and those memory or the store of
//!   the readings has are told at once, the others `Checking`; then one fetch of every head that is missing (two for a
//!   mirror, `fetch_review_pairs`) and one read of the diffs of all of them (`review_plan::reading::ahead_readings`: one
//!   `diff-tree --stdin`, one `cat-file --batch`). Never a read per request: on the IntelliJ monorepo one read of 20
//!   pull requests ran 2 git processes warm, one per request 20 to 41.
//! - **Without the index**: a query of accio of a tree far from its index costs tens of seconds (JetBrains/accio#777),
//!   so a rename that is not local waits for it (`checking renames`) until a review window of the request asks the index
//!   itself; such a reading is asked again by the next listing, and memory then has what the window read. It waits only
//!   for an index of this repository (`range_readings::repository_indexed`, `accio stats`): without one the rename is not
//!   proven, the reading final and kept, and no listing reads it again.
//! - **One read of a repository at a time**: a listing that comes while one runs leaves its requests for the next read,
//!   which runs once the one in flight ends, so two never fetch into the same refs; nothing is killed. Only a repository
//!   that reads review plans (`branchdeck.reviewExperimental`) is read; the others' requests are told `Unread` and show
//!   nothing.

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use crate::reads::range_readings::{is_final, keep_readings, kept_readings, known_indexed, repository_indexed};
use crate::reads::review_plan::{Asked, Canceled, PlanReader};
use crate::transport::WindowId;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ApiError;
use domain_types::review_request::{FetchReviewHeadParams, ReviewRequest};
use git_executor::git_command_executor::ProgressStop;
use operations::rewrite::RewriteHost;
use repository_model::branch_table::BranchTable;
use repository_model::review_readings::{ReadAheadAnswer, ReadAheadState};
use repository_model::session::{SessionKey, Sessions};
use review_plan::PlanSources;
use review_plan::concerns::ConcernEdges;
use review_plan::mark_reviewed::{NoCommonHistory, UnknownReviewBase, UnknownReviewBranch};
use review_plan::reading::ahead_readings;
use review_plan::{ReviewPlanParams, ReviewSubject};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use sync_core::review_fetch::{AheadPair, REVIEW_FETCH_IDLE_TIMEOUT, fetch_review_pairs, local_review_pairs};
use sync_utils::child_process::ChildPriority;
use tracing::{Instrument, debug, info_span};

/// The branches of a repository read ahead at most: those a window asked last
const WATCHED: usize = 4;

/// What the windows asked of the branches, for the read ahead
#[derive(Default)]
pub(crate) struct Prefetch(Mutex<Watches>);

#[derive(Default)]
struct Watches {
  /// By repository (`ReviewPlanParams::repository_path`, the worktree of its session), the `WATCHED` branches its
  /// windows asked last, by full name
  asked: HashMap<String, HashMap<String, Watch>>,
  /// The branch each window shows now, by repository and full name: of its last plan of a scope; none for a range
  shown: HashMap<WindowId, (String, String)>,
  /// The number of the newest commit of the table of each repository: a read ahead of an older one is superseded
  commits: HashMap<String, Arc<AtomicU64>>,
  /// The number of the last ask
  last: u64,
  /// The read ahead of the waiting requests of each repository, by worktree
  requests: HashMap<String, AheadQueue>,
}

/// The read ahead of the waiting requests of a repository: one runs at a time, and the requests a listing names meanwhile
/// wait for the next
#[derive(Default)]
struct AheadQueue {
  running: bool,
  /// The session and the requests of the next read, the last listing's of a number
  next: Option<(SessionKey, Vec<ReviewRequest>)>,
}

/// A branch a window asked the plan of, as it asked it
struct Watch {
  /// As the window named it (`ReviewSubject::Branch::name`)
  name: String,
  /// The stack parent its base is the head of, by its name; none on the baseline
  parent: Option<String>,
  since_reviewed: bool,
  /// The head its plan was asked or read ahead at: a commit that leaves the branch there reads nothing
  head: String,
  /// The number of the ask: the most recent is read first
  number: u64,
}

/// A branch to read ahead after a commit
struct Job {
  /// The full name
  branch: String,
  /// The number of the ask it reads
  number: u64,
  params: ReviewPlanParams,
}

/// The read ahead after one commit of the table of a repository: superseded by the next commit
struct Ticket {
  counter: Arc<AtomicU64>,
  number: u64,
}

impl Ticket {
  fn is_superseded(&self) -> bool {
    self.counter.load(Ordering::SeqCst) != self.number
  }
}

impl Prefetch {
  fn watches(&self) -> MutexGuard<'_, Watches> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// `window` asked the plan of a scope, resolved as `asked`: of a branch, what the window shows now, kept for the read
  /// ahead as the newest watch of its repository, past `WATCHED` in place of the one asked longest ago; a range shows
  /// none
  pub(crate) fn asked(&self, window: &WindowId, asked: &Asked) {
    let mut watches = self.watches();
    let (ReviewSubject::Branch { name, .. }, Some(branch)) = (&asked.params.subject, &asked.subject.branch) else {
      watches.shown.remove(window);
      return;
    };
    let repository = asked.params.repository_path.clone();
    watches.last += 1;
    let watch = Watch {
      name: name.clone(),
      parent: asked.subject.stack_parent.clone(),
      since_reviewed: asked.params.since_reviewed,
      head: asked.subject.head.clone(),
      number: watches.last,
    };
    let branches = watches.asked.entry(repository.clone()).or_default();
    branches.insert(branch.clone(), watch);
    if branches.len() > WATCHED
      && let Some(oldest) = branches.iter().min_by_key(|(_, watch)| watch.number).map(|(branch, _)| branch.clone())
    {
      branches.remove(&oldest);
    }
    watches.shown.insert(window.clone(), (repository, branch.clone()));
  }

  /// `window` closed: the branch it showed is read ahead from now on
  pub(crate) fn forget_window(&self, window: &WindowId) {
    self.watches().shown.remove(window);
  }

  /// A sync of `key` committed `table`: the read ahead of the repository in flight is superseded, and the branches to
  /// read now are those whose head the table moved and no window shows, the most recently asked first. A branch the
  /// table no longer has is forgotten; one whose stack parent left it is on the baseline now
  fn committed(&self, key: &SessionKey, table: &BranchTable) -> (Ticket, Vec<Job>) {
    let mut watches = self.watches();
    let Watches { asked, shown, commits, .. } = &mut *watches;
    let counter = Arc::clone(commits.entry(key.worktree.clone()).or_default());
    let number = counter.fetch_add(1, Ordering::SeqCst) + 1;
    let ticket = Ticket { counter, number };
    let Some(branches) = asked.get_mut(&key.worktree) else {
      return (ticket, Vec::new());
    };
    branches.retain(|branch, _| table.tips.contains_key(branch));
    let in_table = |name: &str| to_final_branch_name(&key.prefix, name).is_ok_and(|full| table.tips.contains_key(&full));
    let mut jobs: Vec<Job> = branches
      .iter()
      .filter(|(branch, watch)| table.tips.get(*branch).is_some_and(|tip| *tip != watch.head))
      .filter(|(branch, _)| !shown.values().any(|(repository, shown)| *repository == key.worktree && shown == *branch))
      .map(|(branch, watch)| {
        let parent = watch.parent.clone().filter(|parent| in_table(parent));
        let baseline_branch = parent.is_none().then(|| table.baseline.clone());
        Job {
          branch: branch.clone(),
          number: watch.number,
          params: ReviewPlanParams {
            repository_path: key.worktree.clone(),
            subject: ReviewSubject::Branch {
              name: watch.name.clone(),
              base: parent,
              baseline_branch,
            },
            since_reviewed: watch.since_reviewed,
            concern_edges: ConcernEdges::default(),
          },
        }
      })
      .collect();
    jobs.sort_by_key(|job| std::cmp::Reverse(job.number));
    (ticket, jobs)
  }

  /// The plan of the ask `number` of `branch` was read at `head`: a commit that leaves the branch there reads nothing.
  /// A newer ask of the branch keeps its own head
  fn read_at(&self, repository: &str, branch: &str, number: u64, head: &str) {
    if let Some(watch) = self.watches().asked.get_mut(repository).and_then(|branches| branches.get_mut(branch))
      && watch.number == number
    {
      head.clone_into(&mut watch.head);
    }
  }

  /// The listing of `key` leaves `requests` to read ahead: they join the next read of the repository; whether a read is
  /// to start now (none runs)
  fn queue_requests(&self, key: &SessionKey, requests: Vec<ReviewRequest>) -> bool {
    let mut watches = self.watches();
    let queue = watches.requests.entry(key.worktree.clone()).or_default();
    let next = queue.next.get_or_insert_with(|| (key.clone(), Vec::new()));
    next.0 = key.clone();
    for request in requests {
      next.1.retain(|known| known.number != request.number);
      next.1.push(request);
    }
    !std::mem::replace(&mut queue.running, true)
  }

  /// The next read of the waiting requests of `worktree`, `None` once none waits: no read runs from then on
  fn next_requests(&self, worktree: &str) -> Option<(SessionKey, Vec<ReviewRequest>)> {
    let mut watches = self.watches();
    let queue = watches.requests.get_mut(worktree)?;
    let next = queue.next.take();
    if next.is_none() {
      watches.requests.remove(worktree);
    }
    next
  }

  /// The ask `number` of `branch` no longer resolves (the branch or its base went): forgotten, unless asked again since
  fn forget(&self, repository: &str, branch: &str, number: u64) {
    if let Some(branches) = self.watches().asked.get_mut(repository)
      && branches.get(branch).is_some_and(|watch| watch.number == number)
    {
      branches.remove(branch);
    }
  }
}

/// Reads ahead after the commit of `table` by a sync of `key` (module doc), on the runtime of the app; returns at once
pub fn prefetch_after_commit(ctx: &AppContext, key: &SessionKey, table: &BranchTable) {
  if let Some(reading) = read_ahead(PlanReader::of(ctx), key, table) {
    // its span is a child of the span of the session that committed the table
    ctx.runtime.spawn(reading.in_current_span());
  }
}

/// The read ahead after the commit of `table` by a sync of `key`, which supersedes the one in flight of the repository:
/// `None` when it has no branch to read
pub(crate) fn read_ahead(reader: PlanReader, key: &SessionKey, table: &BranchTable) -> Option<impl Future<Output = ()> + Send + use<>> {
  let (ticket, jobs) = reader.plans.prefetch.committed(key, table);
  if jobs.is_empty() {
    return None;
  }
  let repository = key.worktree.clone();
  let span = info_span!("review_prefetch", repository_path = %repository, branches = jobs.len());
  Some(
    async move {
      if let Err(error) = operation(read_jobs(reader, repository, ticket, jobs)).await {
        debug!("the plans were not read ahead: {}", error.message);
      }
    }
    .instrument(span),
  )
}

/// Reads the plan of each of `jobs` in turn, below the app, until `ticket` is superseded
async fn read_jobs(reader: PlanReader, repository: String, ticket: Ticket, jobs: Vec<Job>) -> Result<(), ApiError> {
  let canceled: Canceled = Arc::new(move || ticket.is_superseded());
  // a plan without its usages is not kept: of no use read ahead
  let enabled = {
    let (reader, repository) = (reader.clone(), repository.clone());
    run_blocking(move || reader.enabled(&repository)).await?
  };
  if !enabled || reader.indexes.search(&|| false, ChildPriority::Background).is_none() {
    return Ok(());
  }
  for job in jobs {
    if canceled() {
      return Ok(());
    }
    let resolved = {
      let (reader, params) = (reader.clone(), job.params.clone());
      run_blocking(move || match reader.resolve(params) {
        Ok(asked) => Ok(Some(asked)),
        Err(error) if is_gone(&error) => Ok(None),
        Err(error) => Err(error),
      })
      .await
    };
    let asked = match resolved {
      Ok(Some(asked)) => asked,
      Ok(None) => {
        reader.plans.prefetch.forget(&repository, &job.branch, job.number);
        continue;
      }
      Err(error) => {
        debug!(branch = %job.branch, "Cannot resolve the branch to read its plan ahead: {}", error.message);
        continue;
      }
    };
    match reader.read(&asked, &canceled, ChildPriority::Background, None).await {
      Ok(plan) => reader.plans.prefetch.read_at(&repository, &job.branch, job.number, &plan.head),
      Err(_) if canceled() => return Ok(()),
      // a failure, or a window asked the plan meanwhile and reads it itself (`Stage::yields`)
      Err(error) => debug!(branch = %job.branch, "The plan was not read ahead: {}", error.message),
    }
  }
  Ok(())
}

/// The branch of a job, or the base it is reviewed on, went: the branch is read ahead no more
fn is_gone(error: &anyhow::Error) -> bool {
  error
    .chain()
    .any(|cause| cause.is::<UnknownReviewBranch>() || cause.is::<UnknownReviewBase>() || cause.is::<NoCommonHistory>())
}

/// Reads ahead what `requests`, waiting in the listing of `key`, leave to read (module doc), on the runtime of the app;
/// returns at once
pub fn read_ahead_requests(ctx: &AppContext, key: &SessionKey, requests: Vec<ReviewRequest>) {
  let reader = PlanReader::of(ctx);
  if !reader.plans.prefetch.queue_requests(key, requests) {
    return;
  }
  let (sessions, rewrites, worktree) = (Arc::clone(&ctx.sessions), Arc::clone(&ctx.rewrites), key.worktree.clone());
  let span = info_span!("review_read_ahead", repository_path = %worktree);
  let reading = async move {
    while let Some((key, requests)) = reader.plans.prefetch.next_requests(&worktree) {
      let answers = match operation(read_requests(&reader, &sessions, &*rewrites, &key, &requests)).await {
        Ok(answers) => answers,
        Err(error) => {
          debug!("the waiting requests were not read ahead: {}", error.message);
          unread(&requests)
        }
      };
      sessions.review_readings(&key, answers).await;
    }
  }
  .instrument(span);
  // its span is a child of the span of the session that listed the requests
  ctx.runtime.spawn(reading.in_current_span());
}

/// What a request the read ahead could not read is told
fn unread(requests: &[ReviewRequest]) -> Vec<ReadAheadAnswer> {
  requests.iter().map(|request| answer(request, ReadAheadState::Unread)).collect()
}

/// `state` of `request` at the head and the base its listing names
fn answer(request: &ReviewRequest, state: ReadAheadState) -> ReadAheadAnswer {
  ReadAheadAnswer {
    number: request.number,
    head: request.head_ref_oid.clone(),
    base: request.base_ref_oid.clone(),
    state,
  }
}

/// Reads what `requests` of `key` leave to read: those memory or the store of the readings has told to the session at
/// once with the others `Checking`, then the answers of the others; every request `Unread` in a repository that reads no
/// review plans
async fn read_requests(
  reader: &PlanReader,
  sessions: &Sessions,
  rewrites: &dyn RewriteHost,
  key: &SessionKey,
  requests: &[ReviewRequest],
) -> Result<Vec<ReadAheadAnswer>, ApiError> {
  let repository = key.worktree.clone();
  let enabled = {
    let (reader, repository) = (reader.clone(), repository.clone());
    run_blocking(move || reader.enabled(&repository)).await?
  };
  if !enabled {
    return Ok(unread(requests));
  }
  // the head and the base GitHub names, as a review fetch takes them (the E2E server fills what a fixture leaves out)
  let params: Vec<FetchReviewHeadParams> = requests
    .iter()
    .map(|request| {
      rewrites.review_fetch(FetchReviewHeadParams {
        repository_path: repository.clone(),
        number: request.number,
        head_ref_name: request.head_ref_name.clone(),
        head_ref_oid: request.head_ref_oid.clone(),
        base_ref_name: request.base_ref_name.clone(),
        base_ref_oid: request.base_ref_oid.clone(),
        is_cross_repository: request.is_cross_repository,
      })
    })
    .collect::<Result<_, _>>()?;
  let local = {
    let (reader, repository, params) = (reader.clone(), repository.clone(), params.clone());
    run_blocking(move || {
      let pairs = local_review_pairs(reader.git(), &repository, &params)?;
      let ranges: Vec<(String, String)> = pairs.iter().filter_map(range_of).collect();
      let never = || false;
      let mut indexed = || {
        known_indexed(&reader.plans, &repository)
          .unwrap_or_else(|| repository_indexed(&reader.plans, reader.indexes.search(&never, ChildPriority::Background).as_deref(), &repository))
      };
      let mut kept = kept_readings(&reader.plans, &reader.object_cache(&repository), &repository, &ranges, &mut indexed).into_iter();
      let kept: Vec<Option<_>> = pairs.iter().map(|pair| range_of(pair).and_then(|_| kept.next().flatten())).collect();
      anyhow::Ok((pairs, kept))
    })
    .await?
  };
  let (mut pairs, kept) = local;
  let told: Vec<ReadAheadAnswer> = requests
    .iter()
    .zip(&kept)
    .map(|(request, kept)| match kept {
      Some(reading) => answer(
        request,
        ReadAheadState::Read {
          reading: reading.clone(),
          last: true,
        },
      ),
      None => answer(request, ReadAheadState::Checking),
    })
    .collect();
  sessions.review_readings(key, told).await;
  if kept.iter().all(Option::is_some) {
    return Ok(Vec::new());
  }

  let known: Vec<bool> = kept.iter().map(Option::is_some).collect();
  let (reader, repository) = (reader.clone(), repository.clone());
  let (read, indexed) = run_blocking(move || {
    // the heads that are missing, in one fetch
    let missing: Vec<usize> = (0..params.len()).filter(|&index| pairs[index] == AheadPair::Missing).collect();
    if !missing.is_empty() {
      let stop = ProgressStop {
        idle_timeout: Some(REVIEW_FETCH_IDLE_TIMEOUT),
        is_cancelled: &|| false,
      };
      let fetching: Vec<FetchReviewHeadParams> = missing.iter().map(|&index| params[index].clone()).collect();
      for (index, pair) in missing.into_iter().zip(fetch_review_pairs(reader.git(), &repository, &fetching, &stop)?) {
        pairs[index] = pair;
      }
    }
    // the diffs of those no memory had, in one read
    let unread: Vec<usize> = (0..pairs.len()).filter(|&index| !known[index] && range_of(&pairs[index]).is_some()).collect();
    let ranges: Vec<(String, String)> = unread.iter().filter_map(|&index| range_of(&pairs[index])).collect();
    // a rename waits for an index of this repository, never for accio alone: without one it is not proven, final
    let never = || false;
    let indexed = repository_indexed(&reader.plans, reader.indexes.search(&never, ChildPriority::Background).as_deref(), &repository);
    let cache = reader.object_cache(&repository);
    let sources = PlanSources {
      cache: &cache,
      file_diffs: reader.file_diffs(),
      index: None,
    };
    let mut readings = vec![None; ranges.len()];
    ahead_readings(reader.git(), sources, &repository, &ranges, indexed, |at, reading| readings[at] = Some(reading))?;
    let read: Vec<(&(String, String), &_)> = ranges.iter().zip(&readings).filter_map(|(range, reading)| Some((range, reading.as_ref()?))).collect();
    keep_readings(&reader.plans, &cache, &repository, &read, indexed);
    let mut by_place = vec![None; pairs.len()];
    for (index, reading) in unread.into_iter().zip(readings) {
      by_place[index] = reading;
    }
    anyhow::Ok((by_place, indexed))
  })
  .await?;
  Ok(
    requests
      .iter()
      .zip(kept.iter().zip(read))
      .filter_map(|(request, (kept, read))| match (kept, read) {
        (Some(_), _) => None,
        (None, Some(reading)) => {
          let last = is_final(&reading, indexed);
          Some(answer(request, ReadAheadState::Read { reading, last }))
        }
        (None, None) => Some(answer(request, ReadAheadState::Unread)),
      })
      .collect(),
  )
}

/// The range a pair is read by
fn range_of(pair: &AheadPair) -> Option<(String, String)> {
  match pair {
    AheadPair::Pair(pair) => Some((pair.base.clone(), pair.head.clone())),
    AheadPair::Missing | AheadPair::Unrelated => None,
  }
}
