//! The plan of a review in its window (openspec/specs/branch/review-plan/spec.md, Status: Experimental): what changed since the
//! review of the branch, the units of its diff, the API it changes with the usages of each declaration (updated in the
//! change or left outside), the facts of each file, the links between the files, the dependency changes of the build
//! files and the dependency order of the files (`review_plan::review_plan_core`, the one entry of `brd review-plan`
//! too). The usages ask the index the host has (`SymbolIndexes`: accio in the app, the fixture of a test repository in
//! the e2e server).
//!
//! - **On the channel** of the command: `disabled` when the repository turns the review plan off
//!   (`branchdeck.reviewExperimental`), else the plan without the usages while the index is asked (`pending`), then the
//!   plan with them; the page replaces what it shows with each, and orders nothing itself.
//! - **One plan per window and slot** (`Windows::begin_plan`): a newer one of the window or its close supersedes the one
//!   in flight, whose queries of the index are killed, and it answers `canceled`; the plan of a commit of the column
//!   (`PlanSlot::Commit`) and the plan of its scope go on beside each other, and a symbol search beside both.
//! - **The memory of the plans** (`ReviewPlans`): a plan whose every query answered is kept by what it is of, so the
//!   window that opens it again, or another window of it, reads no git past the resolve of its subject and asks the
//!   index nothing
//! - **One reading of a plan** (`PlanReader::read`): a window that asks the plan another window or the read ahead reads
//!   now joins that reading and gets its plan whole, in one event, without the plan of its pending usages; a reading
//!   whose caller went away (superseded, canceled) ends without a plan, and the first caller that waited for it reads
//!   it itself
//! - **Read ahead after a rewrite** (`review_prefetch`): the branch a window asked the plan of is read again, below the
//!   app, once a sync moved it

use crate::api_error::api_error;
use crate::context::{AppContext, PrefixInputs};
use crate::operation::{operation, run_blocking};
use crate::reads::review_prefetch::Prefetch;
use crate::reads::symbols::SymbolIndexes;
use crate::repository_state::RepositoryStateCache;
use crate::session::windows::{PlanSlot, Windows};
use crate::transport::{Call, Sink, WindowId};
use domain_types::error::ApiError;
use file_diffs::accio::SearchCanceled;
use file_diffs::service::FileDiffs;
use moka::sync::Cache;
use review_plan::{PlanSources, ResolvedSubject, ReviewPlan, ReviewPlanEvent, ReviewPlanParams, ReviewSubject, resolve_subject, review_plan_of};
use std::sync::Arc;
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_utils::child_process::ChildPriority;
use sync_utils::single_flight::{Begin, Join, SingleFlight};
use tracing::{debug, instrument};

/// What a plan is of: the repository, the prefix of its branches, the subject resolved and how it is read
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlanKey {
  repository_path: String,
  branch_prefix: String,
  branch: Option<String>,
  base: String,
  head: String,
  /// The reviewed pair of the branch, which its interdiff compares with
  reviewed: Option<(String, String)>,
  /// The reviewed commit a commit is read with, whose message its story compares with
  reviewed_commit: Option<String>,
  since_reviewed: bool,
}

impl PlanKey {
  fn of(params: &ReviewPlanParams, branch_prefix: &str, subject: &ResolvedSubject) -> Self {
    Self {
      repository_path: params.repository_path.clone(),
      branch_prefix: branch_prefix.to_string(),
      branch: subject.branch.clone(),
      base: subject.base.clone(),
      head: subject.head.clone(),
      reviewed: subject
        .interdiff
        .as_ref()
        .map(|interdiff| (interdiff.reviewed_base.clone(), interdiff.reviewed_tip.clone())),
      reviewed_commit: subject.reviewed_commit.clone(),
      since_reviewed: params.since_reviewed,
    }
  }
}

/// The plans of the reviews: those whose every query of the index answered, by what each is of (`PlanKey`): its
/// commits and its reviewed pair fix it, so it is never stale; weighed by the size of its JSON, at most `MAX_BYTES`,
/// and gone after `IDLE` unread. Beside them the readings in flight, by key, and what the windows asked, for the read
/// ahead (`Prefetch`)
#[derive(Clone)]
pub struct ReviewPlans {
  kept: Cache<PlanKey, Arc<ReviewPlan>>,
  flights: Arc<SingleFlight<PlanKey, Arc<ReviewPlan>>>,
  pub(crate) prefetch: Arc<Prefetch>,
}

/// The memory of the plans at most
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// A plan not read for this long is dropped: an index built since may know more usages
const IDLE: Duration = Duration::from_secs(600);
/// How often a caller that waits for the reading of another asks whether its own caller went away
const CANCEL_POLL: Duration = Duration::from_millis(20);

impl Default for ReviewPlans {
  fn default() -> Self {
    let weigh = |_: &PlanKey, plan: &Arc<ReviewPlan>| serde_json::to_vec(plan.as_ref()).map_or(u32::MAX, |json| u32::try_from(json.len()).unwrap_or(u32::MAX));
    Self {
      kept: Cache::builder().max_capacity(MAX_BYTES).weigher(weigh).time_to_idle(IDLE).build(),
      flights: Arc::default(),
      prefetch: Arc::default(),
    }
  }
}

impl ReviewPlans {
  /// `window` closed: the branch it showed is read ahead again after a rewrite, as the window no longer asks it
  pub fn forget_window(&self, window: &WindowId) {
    self.prefetch.forget_window(window);
  }
}

/// Whether the caller of a reading went away: a newer plan of its window, a newer commit of the table it reads ahead of
pub(crate) type Canceled = Arc<dyn Fn() -> bool + Send + Sync>;

/// A subject resolved, as a reading takes it: the params, the subject and its key
#[derive(Clone)]
pub(crate) struct Asked {
  pub(crate) params: ReviewPlanParams,
  pub(crate) subject: ResolvedSubject,
  key: PlanKey,
}

/// What this caller of a reading did: it read the plan (or found it in the memory), or found the reading of another
pub(crate) enum Turn {
  Read(Arc<ReviewPlan>),
  Joined(Join<Arc<ReviewPlan>>),
}

/// What a plan is read with beside its params, owned for the blocking steps: git and the prefix of the repository, its
/// caches, the index of the host and the plans
#[derive(Clone)]
pub(crate) struct PlanReader {
  inputs: Arc<PrefixInputs>,
  repository_state: RepositoryStateCache,
  file_diffs: FileDiffs,
  pub(crate) indexes: Arc<dyn SymbolIndexes>,
  pub(crate) plans: ReviewPlans,
}

impl PlanReader {
  /// The reader of the context, with the settings as they are now
  pub(crate) fn of(ctx: &AppContext) -> Self {
    Self::new(
      ctx.prefix_inputs(),
      ctx.repository_state.clone(),
      ctx.file_diffs.clone(),
      Arc::clone(&ctx.symbol_indexes),
      ctx.review_plans.clone(),
    )
  }

  pub(crate) fn new(inputs: PrefixInputs, repository_state: RepositoryStateCache, file_diffs: FileDiffs, indexes: Arc<dyn SymbolIndexes>, plans: ReviewPlans) -> Self {
    Self {
      inputs: Arc::new(inputs),
      repository_state,
      file_diffs,
      indexes,
      plans,
    }
  }

  /// Whether the repository of `repository_path` reads review plans (`branchdeck.reviewExperimental`). Blocking
  pub(crate) fn enabled(&self, repository_path: &str) -> anyhow::Result<bool> {
    Ok(ConfigSnapshot::read(&self.inputs.git, repository_path)?.review_experimental())
  }

  /// The subject of `params` resolved with the prefix of its repository, and its key. Blocking
  pub(crate) fn resolve(&self, params: ReviewPlanParams) -> anyhow::Result<Asked> {
    let branch_prefix = self.inputs.branch_prefix(&params.repository_path)?;
    let subject = resolve_subject(&self.inputs.git, &branch_prefix, &params)?;
    let key = PlanKey::of(&params, &branch_prefix, &subject);
    Ok(Asked { params, subject, key })
  }

  /// The plan of `asked`: of the memory; else of the reading of its key in flight, joined; else read now by this caller,
  /// its queries of the index at `priority` and killed once `canceled` turns true, the plan of its pending usages on
  /// `pending`. A joined reading that ended without a plan (its caller went away) is read again, by the first of its
  /// joiners; `canceled` answers `SearchCanceled`
  pub(crate) async fn read(&self, asked: &Asked, canceled: &Canceled, priority: ChildPriority, pending: Option<&Sink<ReviewPlanEvent>>) -> Result<Arc<ReviewPlan>, ApiError> {
    loop {
      let turn = {
        let (reader, asked, canceled, pending) = (self.clone(), asked.clone(), Arc::clone(canceled), pending.cloned());
        run_blocking(move || reader.lead_or_join(&asked, &canceled, priority, pending.as_ref())).await?
      };
      match turn {
        Turn::Read(plan) => return Ok(plan),
        Turn::Joined(join) => {
          if let Some(plan) = joined(join, canceled).await {
            return Ok(plan);
          }
          if canceled() {
            return Err(api_error(SearchCanceled.into()));
          }
          debug!("the reading of the plan this one waited for ended without it: read again");
        }
      }
    }
  }

  /// One turn of `read`: the plan of the memory, the reading in flight of another caller, else the plan read now, kept
  /// before its joiners get it. A failed or canceled reading ends without a plan, so its joiners read it themselves.
  /// Blocking
  pub(crate) fn lead_or_join(&self, asked: &Asked, canceled: &Canceled, priority: ChildPriority, pending: Option<&Sink<ReviewPlanEvent>>) -> anyhow::Result<Turn> {
    if let Some(plan) = self.plans.kept.get(&asked.key) {
      return Ok(Turn::Read(plan));
    }
    let lead = match self.plans.flights.begin(asked.key.clone()) {
      Begin::Lead(lead) => lead,
      Begin::Join(join) => return Ok(Turn::Joined(join)),
    };
    // a reading that ended between the look into the memory and the begin kept its plan before it left the flights
    if let Some(plan) = self.plans.kept.get(&asked.key) {
      lead.finish(Arc::clone(&plan));
      return Ok(Turn::Read(plan));
    }
    let repository_path = asked.params.repository_path.as_str();
    let cache = self.repository_state.object_cache(&self.inputs.git, repository_path);
    let index = self.indexes.search(&**canceled, priority);
    let sources = PlanSources {
      cache: &cache,
      file_diffs: &self.file_diffs,
      index: index.as_deref(),
    };
    let send_pending = |plan: &ReviewPlan| {
      if let Some(pending) = pending
        && !canceled()
      {
        send(
          pending,
          ReviewPlanEvent::Plan {
            plan: Box::new(plan.clone()),
            pending: true,
          },
        );
      }
    };
    let plan = Arc::new(review_plan_of(&self.inputs.git, sources, &asked.params, asked.subject.clone(), send_pending)?);
    if plan.index_note.is_none() {
      self.plans.kept.insert(asked.key.clone(), Arc::clone(&plan));
    }
    lead.finish(Arc::clone(&plan));
    Ok(Turn::Read(plan))
  }
}

/// The plan of the reading `join` waits for; `None` when that reading ended without one, or `canceled` turned true
/// first
pub(crate) async fn joined(join: Join<Arc<ReviewPlan>>, canceled: &Canceled) -> Option<Arc<ReviewPlan>> {
  let mut join = std::pin::pin!(join);
  loop {
    if let Ok(answer) = tokio::time::timeout(CANCEL_POLL, join.as_mut()).await {
      return answer;
    }
    if canceled() {
      return None;
    }
  }
}

/// Sends `event` to the page of the plan, which may have gone away
fn send(events: &Sink<ReviewPlanEvent>, event: ReviewPlanEvent) {
  if events.send(event).is_err() {
    debug!("the page of the review plan went away");
  }
}

/// The plan of `params.subject` on `events` (module doc), with the branches named by the prefix Rust resolves for the
/// repository (`PrefixInputs`), through its object cache, as the plan of the window of `call`
#[instrument(skip_all, fields(repository_path = %params.repository_path, subject = ?params.subject, since_reviewed = params.since_reviewed))]
pub async fn get_review_plan(ctx: &AppContext, call: &Call, params: ReviewPlanParams, events: Sink<ReviewPlanEvent>) -> Result<(), ApiError> {
  operation(async move { ask_plan(PlanReader::of(ctx), &ctx.windows, &call.window, params, events).await }).await
}

/// `get_review_plan` of the window `window` with `reader`: the plan of a scope a window asks is what it shows now, which
/// the read ahead leaves to it (`Prefetch::asked`)
pub(crate) async fn ask_plan(reader: PlanReader, windows: &Windows, window: &WindowId, params: ReviewPlanParams, events: Sink<ReviewPlanEvent>) -> Result<(), ApiError> {
  let slot = if matches!(params.subject, ReviewSubject::Commit { .. }) {
    PlanSlot::Commit
  } else {
    PlanSlot::Scope
  };
  let request = windows.begin_plan(window, slot);
  let asked = {
    let (reader, events, window) = (reader.clone(), Arc::clone(&events), window.clone());
    run_blocking(move || -> anyhow::Result<Option<Asked>> {
      if !reader.enabled(&params.repository_path)? {
        send(&events, ReviewPlanEvent::Disabled);
        return Ok(None);
      }
      let asked = reader.resolve(params)?;
      if slot == PlanSlot::Scope {
        reader.plans.prefetch.asked(&window, &asked);
      }
      Ok(Some(asked))
    })
    .await?
  };
  let Some(asked) = asked else { return Ok(()) };
  let canceled: Canceled = Arc::new(move || request.is_superseded());
  let plan = reader.read(&asked, &canceled, ChildPriority::Normal, Some(&events)).await?;
  // a plan the window asked again meanwhile is of no use to it; kept, it is of use to the next one
  if canceled() {
    return Err(api_error(SearchCanceled.into()));
  }
  send(
    &events,
    ReviewPlanEvent::Plan {
      plan: Box::new(plan.as_ref().clone()),
      pending: false,
    },
  );
  Ok(())
}

#[cfg(test)]
impl ReviewPlans {
  /// Whether the memory keeps the plan of `asked`
  pub(crate) fn keeps(&self, asked: &Asked) -> bool {
    self.kept.contains_key(&asked.key)
  }
}
