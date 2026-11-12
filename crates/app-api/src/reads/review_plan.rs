//! The plan of a review in its window (openspec/specs/branch/review-plan/spec.md, Status: Experimental): what changed since the
//! review of the branch, the units of its diff, the API it changes with the usages of each declaration (updated in the
//! change or left outside), the facts of each file, the links between the files, the dependency changes of the build
//! files and the dependency order of the files (`review_plan::review_plan_core`, the one entry of `brd review-plan`
//! too). The usages ask the index the host has (`SymbolIndexes`: accio in the app, the fixture of a test repository in
//! the e2e server).
//!
//! - **On the channel** of the command: `disabled` when the repository turns the review plan off
//!   (`branchdeck.reviewExperimental`), else the plan as it stands while the index is asked, each with what it still
//!   waits for (`PlanWait`: every answer of the index, then the tests of the steps alone), then the plan whole; the page
//!   replaces what it shows with each, and orders nothing itself.
//! - **One plan per window and slot** (`Windows::begin_plan`): a newer one of the window or its close supersedes the one
//!   in flight, whose queries of the index are killed, and it answers `canceled`; the plan of a commit of the column
//!   (`PlanSlot::Commit`) and the plan of its scope go on beside each other, and a symbol search beside both.
//! - **The memory of the plans** (`ReviewPlans`): a plan whose every query answered, none cut short
//!   (`ReviewPlan::is_final`), is kept by what it is of, so the
//!   window that opens it again, or another window of it, reads no git past the resolve of its subject and asks the
//!   index nothing
//! - **One reading of a plan** (`PlanReader::read`): a window that asks the plan another window or the read ahead reads
//!   now joins that reading. The reading publishes each stage of its plan beside its flight (`Staging`), so the window
//!   that joined gets the latest one at once and each later one as it comes, then the plan whole; a reading whose
//!   caller went away (superseded, canceled) ends without a plan, and the first caller that waited for it reads it
//!   itself
//! - **The read ahead yields to a window** (`Stage::yields`): a window that joins a reading led below the app ends
//!   it, its queries of the index killed, and reads the plan itself at the priority of the app
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
use review_plan::{PlanSources, PlanWait, ResolvedSubject, ReviewPlan, ReviewPlanEvent, ReviewPlanParams, ReviewSubject, resolve_subject, review_plan_of};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
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
/// and gone after `IDLE` unread. Beside them the readings in flight, by key, with what each published of its plan so
/// far (`Stage`), and what the windows asked, for the read ahead (`Prefetch`)
#[derive(Clone)]
pub struct ReviewPlans {
  kept: Cache<PlanKey, Arc<ReviewPlan>>,
  flights: Arc<SingleFlight<PlanKey, Arc<ReviewPlan>>>,
  /// The stage of each reading in flight, there from its start until it ends, however it ends (`Staging`)
  stages: Arc<Mutex<HashMap<PlanKey, Arc<Stage>>>>,
  pub(crate) prefetch: Arc<Prefetch>,
}

/// What a reading in flight shows the callers that joined it, beside its flight: the plan as it stands, and whether it
/// gives way to a window. Never a plan of the memory: a staged plan is no answer of a reading
struct Stage {
  /// The priority of the queries of its leader
  priority: ChildPriority,
  /// The plan as its leader published it last, with what it still waits for and the number of the stage, from 1
  published: Mutex<Option<(u32, Arc<ReviewPlan>, PlanWait)>>,
  /// A window waits for this reading led below the app: its leader ends it without a plan (`yields`)
  yields: AtomicBool,
}

impl Stage {
  /// The stage published after the stage `seen`, if any
  fn published_after(&self, seen: u32) -> Option<(u32, Arc<ReviewPlan>, PlanWait)> {
    self.published.lock().unwrap_or_else(PoisonError::into_inner).clone().filter(|(number, ..)| *number > seen)
  }

  fn publish(&self, plan: Arc<ReviewPlan>, waits: PlanWait) {
    let mut published = self.published.lock().unwrap_or_else(PoisonError::into_inner);
    let number = published.as_ref().map_or(1, |(number, ..)| number + 1);
    *published = Some((number, plan, waits));
  }
}

/// The stage of the reading this caller leads, in the stages of the plans until it is dropped: before its flight ends,
/// so a caller that joins the next flight of the key never finds it
struct Staging<'a> {
  plans: &'a ReviewPlans,
  key: &'a PlanKey,
  stage: Arc<Stage>,
}

impl Drop for Staging<'_> {
  fn drop(&mut self) {
    let mut stages = self.plans.stages();
    if stages.get(self.key).is_some_and(|stage| Arc::ptr_eq(stage, &self.stage)) {
      stages.remove(self.key);
    }
  }
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
      stages: Arc::default(),
      prefetch: Arc::default(),
    }
  }
}

impl ReviewPlans {
  /// `window` closed: the branch it showed is read ahead again after a rewrite, as the window no longer asks it
  pub fn forget_window(&self, window: &WindowId) {
    self.prefetch.forget_window(window);
  }

  /// The stages of the readings in flight: held for a look or a change of the map, never across a reading
  fn stages(&self) -> MutexGuard<'_, HashMap<PlanKey, Arc<Stage>>> {
    self.stages.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The stage of the reading of `key` this caller leads at `priority`, there until the `Staging` is dropped
  fn stage<'a>(&'a self, key: &'a PlanKey, priority: ChildPriority) -> Staging<'a> {
    let stage = Arc::new(Stage {
      priority,
      published: Mutex::default(),
      yields: AtomicBool::new(false),
    });
    self.stages().insert(key.clone(), Arc::clone(&stage));
    Staging { plans: self, key, stage }
  }

  /// The stage of the reading of `key` in flight, once its leader opened it
  fn stage_of(&self, key: &PlanKey) -> Option<Arc<Stage>> {
    self.stages().get(key).cloned()
  }
}

/// A caller that waits for the reading of another (`joined`): what it does on each look at that reading
pub(crate) struct Follower<'a> {
  plans: &'a ReviewPlans,
  key: &'a PlanKey,
  /// The priority this caller would read at
  priority: ChildPriority,
  /// The page of the window that waits, if a window does
  events: Option<&'a Sink<ReviewPlanEvent>>,
  canceled: &'a Canceled,
  /// The number of the last stage it sent its page
  seen: u32,
}

impl<'a> Follower<'a> {
  pub(crate) fn new(plans: &'a ReviewPlans, asked: &'a Asked, priority: ChildPriority, events: Option<&'a Sink<ReviewPlanEvent>>, canceled: &'a Canceled) -> Self {
    Self {
      plans,
      key: &asked.key,
      priority,
      events,
      canceled,
      seen: 0,
    }
  }

  /// One look at the reading it waits for: a window that waits for a reading led below the app makes it yield, and the
  /// stage that reading published since the last look goes to the page of the window, unless its request was superseded
  fn look(&mut self) {
    let Some(stage) = self.plans.stage_of(self.key) else { return };
    if self.priority == ChildPriority::Normal && stage.priority == ChildPriority::Background {
      stage.yields.store(true, Ordering::SeqCst);
    }
    let Some(events) = self.events else { return };
    if let Some((number, plan, waits)) = stage.published_after(self.seen)
      && !(self.canceled)()
    {
      self.seen = number;
      send(
        events,
        ReviewPlanEvent::Plan {
          plan: Box::new(plan.as_ref().clone()),
          waits: Some(waits),
        },
      );
    }
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
  /// its queries of the index at `priority` and killed once `canceled` turns true. Each stage of the plan (`PlanWait`)
  /// goes to `events`, of the reading it leads as of the one it joined. A joined reading that ended without a plan (its
  /// caller went away, or it yielded to this window) is read again, by the first of its joiners; `canceled` answers
  /// `SearchCanceled`
  pub(crate) async fn read(&self, asked: &Asked, canceled: &Canceled, priority: ChildPriority, events: Option<&Sink<ReviewPlanEvent>>) -> Result<Arc<ReviewPlan>, ApiError> {
    loop {
      let turn = {
        let (reader, asked, canceled, events) = (self.clone(), asked.clone(), Arc::clone(canceled), events.cloned());
        run_blocking(move || reader.lead_or_join(&asked, &canceled, priority, events.as_ref())).await?
      };
      match turn {
        Turn::Read(plan) => return Ok(plan),
        Turn::Joined(join) => {
          if let Some(plan) = joined(join, Follower::new(&self.plans, asked, priority, events, canceled)).await {
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

  /// One turn of `read`: the plan of the memory, the reading in flight of another caller, else the plan read now, each
  /// stage of it published for its joiners and sent to `events`, and the plan kept before its joiners get it. Only the
  /// plan the reading answers is kept, never a stage. A failed or canceled reading, and one led below the app that a
  /// window joined (`Stage::yields`), ends without a plan, so its joiners read it themselves. Blocking
  pub(crate) fn lead_or_join(&self, asked: &Asked, canceled: &Canceled, priority: ChildPriority, events: Option<&Sink<ReviewPlanEvent>>) -> anyhow::Result<Turn> {
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
    // declared after the lead: dropped before it on every way out, so the stage is gone before the flight ends
    let staging = self.plans.stage(&asked.key, priority);
    let stage = Arc::clone(&staging.stage);
    // the queries of a reading that yields to a window are killed as those of one whose caller went away
    let gone = || canceled() || stage.yields.load(Ordering::SeqCst);
    let repository_path = asked.params.repository_path.as_str();
    let cache = self.repository_state.object_cache(&self.inputs.git, repository_path);
    let index = self.indexes.search(&gone, priority);
    let sources = PlanSources {
      cache: &cache,
      file_diffs: &self.file_diffs,
      index: index.as_deref(),
    };
    let publish = |plan: &ReviewPlan, waits: PlanWait| {
      stage.publish(Arc::new(plan.clone()), waits);
      if let Some(events) = events
        && !canceled()
      {
        send(
          events,
          ReviewPlanEvent::Plan {
            plan: Box::new(plan.clone()),
            waits: Some(waits),
          },
        );
      }
    };
    let plan = Arc::new(review_plan_of(&self.inputs.git, sources, &asked.params, asked.subject.clone(), publish)?);
    if plan.is_final() {
      self.plans.kept.insert(asked.key.clone(), Arc::clone(&plan));
    }
    drop(index);
    drop(staging);
    lead.finish(Arc::clone(&plan));
    Ok(Turn::Read(plan))
  }
}

/// The plan of the reading `join` waits for; `None` when that reading ended without one, or the caller of `follower`
/// went away first. The follower looks at the reading at once and every `CANCEL_POLL` (`Follower::look`)
pub(crate) async fn joined(join: Join<Arc<ReviewPlan>>, mut follower: Follower<'_>) -> Option<Arc<ReviewPlan>> {
  let mut join = std::pin::pin!(join);
  loop {
    follower.look();
    if let Ok(answer) = tokio::time::timeout(CANCEL_POLL, join.as_mut()).await {
      return answer;
    }
    if (follower.canceled)() {
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
      waits: None,
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
