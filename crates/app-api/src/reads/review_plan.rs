//! The plan of a review in its window (spec/branch/review-plan.spec.md, Status: Experimental): what changed since the
//! review of the branch, the units of its diff, the API it changes with the usages of each declaration (updated in the
//! change or left outside), the facts of each file, the links between the files, the dependency changes of the build
//! files and the dependency order of the files (`review_plan::review_plan_core`, the one entry of `bd review-plan`
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

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::session::windows::PlanSlot;
use crate::transport::{Call, Sink};
use domain_types::error::ApiError;
use file_diffs::accio::SearchCanceled;
use moka::sync::Cache;
use review_plan::{PlanSources, ResolvedSubject, ReviewPlan, ReviewPlanEvent, ReviewPlanParams, ReviewSubject, resolve_subject, review_plan_of};
use std::sync::Arc;
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
use tracing::{debug, instrument};

/// What a plan is of: the repository, the prefix of its branches, the subject resolved and how it is read
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PlanKey {
  repository_path: String,
  branch_prefix: String,
  branch: Option<String>,
  base: String,
  head: String,
  /// The reviewed pair of the branch, which its interdiff compares with
  reviewed: Option<(String, String)>,
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
      since_reviewed: params.since_reviewed,
    }
  }
}

/// The plans the windows read whose every query of the index answered, by what each is of (`PlanKey`): its commits
/// and its reviewed pair fix it, so it is never stale; weighed by the size of its JSON, at most `MAX_BYTES`, and gone
/// after `IDLE` unread
#[derive(Clone)]
pub struct ReviewPlans(Cache<PlanKey, Arc<ReviewPlan>>);

/// The memory of the plans at most
const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// A plan not read for this long is dropped: an index built since may know more usages
const IDLE: Duration = Duration::from_secs(600);

impl Default for ReviewPlans {
  fn default() -> Self {
    let weigh = |_: &PlanKey, plan: &Arc<ReviewPlan>| serde_json::to_vec(plan.as_ref()).map_or(u32::MAX, |json| u32::try_from(json.len()).unwrap_or(u32::MAX));
    Self(Cache::builder().max_capacity(MAX_BYTES).weigher(weigh).time_to_idle(IDLE).build())
  }
}

/// The plan of `params.subject` on `events` (module doc), with the branches named by the prefix Rust resolves for the
/// repository (`PrefixInputs`), through its object cache, as the plan of the window of `call`
#[instrument(skip_all, fields(repository_path = %params.repository_path, subject = ?params.subject, since_reviewed = params.since_reviewed))]
pub async fn get_review_plan(ctx: &AppContext, call: &Call, params: ReviewPlanParams, events: Sink<ReviewPlanEvent>) -> Result<(), ApiError> {
  let slot = if matches!(params.subject, ReviewSubject::Commit { .. }) {
    PlanSlot::Commit
  } else {
    PlanSlot::Scope
  };
  let request = ctx.windows.begin_plan(&call.window, slot);
  let (inputs, repository_state, file_diffs, indexes, plans) = (
    ctx.prefix_inputs(),
    ctx.repository_state.clone(),
    ctx.file_diffs.clone(),
    Arc::clone(&ctx.symbol_indexes),
    ctx.review_plans.clone(),
  );
  run_blocking(move || -> anyhow::Result<()> {
    let canceled = || request.is_superseded();
    let send = |event: ReviewPlanEvent| {
      if events.send(event).is_err() {
        debug!("the page of the review plan went away");
      }
    };
    if !ConfigSnapshot::read(&inputs.git, &params.repository_path)?.review_experimental() {
      send(ReviewPlanEvent::Disabled);
      return Ok(());
    }
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let subject = resolve_subject(&inputs.git, &branch_prefix, &params)?;
    let key = PlanKey::of(&params, &branch_prefix, &subject);
    if let Some(plan) = plans.0.get(&key) {
      send(ReviewPlanEvent::Plan {
        plan: Box::new(plan.as_ref().clone()),
        pending: false,
      });
      return Ok(());
    }
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    let index = indexes.search(&canceled);
    let sources = PlanSources {
      cache: &cache,
      file_diffs: &file_diffs,
      index: index.as_deref(),
    };
    let pending = |plan: &ReviewPlan| {
      if !canceled() {
        send(ReviewPlanEvent::Plan {
          plan: Box::new(plan.clone()),
          pending: true,
        });
      }
    };
    let plan = review_plan_of(&inputs.git, sources, &params, subject, pending)?;
    // a plan the window asked again meanwhile is of no use, with or without an index to kill
    if canceled() {
      return Err(SearchCanceled.into());
    }
    if plan.index_note.is_none() {
      plans.0.insert(key, Arc::new(plan.clone()));
    }
    send(ReviewPlanEvent::Plan {
      plan: Box::new(plan),
      pending: false,
    });
    Ok(())
  })
  .await
}
