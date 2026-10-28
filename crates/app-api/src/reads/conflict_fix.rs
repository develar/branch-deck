//! The fix of the conflicts of the last sync: the regroup after which the branches that conflict apply cleanly, found by
//! trial builds (`trial_check::conflict_fix`; objects and the object cache only, no repository lock). One search of the
//! app at a time (`ConflictFixSearch`): a new one or `cancel_conflict_fix` (the page asks it before a sync) stops the
//! one in flight between two trials, which then answers `canceled`

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::Call;
use domain_types::error::ApiError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tracing::instrument;
use trial_check::conflict_fix::{ConflictFix, FindConflictFixParams, find_conflict_fix_core};

/// The cancel flag of the search in flight
#[derive(Default)]
pub struct ConflictFixSearch(Mutex<Option<Arc<AtomicBool>>>);

impl ConflictFixSearch {
  fn running(&self) -> MutexGuard<'_, Option<Arc<AtomicBool>>> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// A new search, which stops the one in flight: its cancel flag
  pub fn begin(&self) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let before = self.running().replace(Arc::clone(&flag));
    if let Some(before) = before {
      before.store(true, Ordering::Relaxed);
    }
    flag
  }

  /// Stops the search in flight, if any
  pub fn cancel(&self) {
    if let Some(running) = self.running().as_ref() {
      running.store(true, Ordering::Relaxed);
    }
  }
}

/// The fix of the branches named with the prefix Rust resolves for the repository (`PrefixInputs`), through its object
/// cache (`find_conflict_fix_core`)
#[instrument(skip(ctx))]
pub async fn find_conflict_fix(ctx: &AppContext, params: FindConflictFixParams) -> Result<ConflictFix, ApiError> {
  let (inputs, repository_state) = (ctx.prefix_inputs(), ctx.repository_state.clone());
  let canceled = ctx.conflict_fix.begin();
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    find_conflict_fix_core(&inputs.git, &cache, &params, &branch_prefix, &canceled)
  })
  .await
}

/// Stops the search in flight (`ConflictFixSearch::cancel`)
#[instrument(skip_all)]
pub fn cancel_conflict_fix(ctx: &AppContext, _call: &Call) {
  ctx.conflict_fix.cancel();
}
