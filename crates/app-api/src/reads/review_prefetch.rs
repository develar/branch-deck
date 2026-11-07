//! The plans of the reviews read ahead after a rewrite (spec/branch/review-plan.spec.md, "In the App";
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
//! - **One reading**: a window that asks the plan being read ahead joins that reading (`PlanReader::read`).
//! - **No memory of its own**: the plans go into the memory of the plans (`ReviewPlans`), the only one of Branch Deck
//!   (docs/design/change-review.md, decision 16): the speed comes from accio.

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use crate::reads::review_plan::{Asked, Canceled, PlanReader};
use crate::transport::WindowId;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ApiError;
use repository_model::branch_table::BranchTable;
use repository_model::session::SessionKey;
use review_plan::concerns::ConcernEdges;
use review_plan::mark_reviewed::{NoCommonHistory, UnknownReviewBase, UnknownReviewBranch};
use review_plan::{ReviewPlanParams, ReviewSubject};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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
      Err(error) => debug!(branch = %job.branch, "Cannot read the plan ahead: {}", error.message),
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
