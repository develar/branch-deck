//! The fix of the conflicts of the last sync: the regroup after which the branches that conflict apply cleanly, found by
//! trial builds (`trial_check::conflict_fix`; objects and the object cache only, no repository lock). One search of a
//! window at a time (`ConflictFixSearch`): a new one of the window, its `cancel_conflict_fix` (the page asks it before
//! a sync) or its close stops the one in flight between two trials, which then answers `canceled`; a window of another
//! connection searches apart

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::{Call, WindowId};
use domain_types::error::ApiError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tracing::instrument;
use trial_check::conflict_fix::{ConflictFix, FindConflictFixParams, find_conflict_fix_core};

/// The cancel flag of the search in flight of each window
#[derive(Default)]
pub struct ConflictFixSearch(Mutex<HashMap<WindowId, Arc<AtomicBool>>>);

impl ConflictFixSearch {
  fn running(&self) -> MutexGuard<'_, HashMap<WindowId, Arc<AtomicBool>>> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// A new search of `window`, which stops the one in flight of the window: its cancel flag
  pub fn begin(&self, window: &WindowId) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let before = self.running().insert(window.clone(), Arc::clone(&flag));
    if let Some(before) = before {
      before.store(true, Ordering::Relaxed);
    }
    flag
  }

  /// Stops the search in flight of `window`, if any
  pub fn cancel(&self, window: &WindowId) {
    if let Some(running) = self.running().get(window) {
      running.store(true, Ordering::Relaxed);
    }
  }

  /// `window` closed: its search stops, and is forgotten
  pub fn forget(&self, window: &WindowId) {
    let removed = self.running().remove(window);
    if let Some(running) = removed {
      running.store(true, Ordering::Relaxed);
    }
  }
}

/// The fix of the branches named with the prefix Rust resolves for the repository (`PrefixInputs`), through its object
/// cache (`find_conflict_fix_core`)
#[instrument(skip(ctx, call), fields(window = %call.window))]
pub async fn find_conflict_fix(ctx: &AppContext, call: &Call, params: FindConflictFixParams) -> Result<ConflictFix, ApiError> {
  let (inputs, repository_state) = (ctx.prefix_inputs(), ctx.repository_state.clone());
  let canceled = ctx.conflict_fix.begin(&call.window);
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    find_conflict_fix_core(&inputs.git, &cache, &params, &branch_prefix, &canceled)
  })
  .await
}

/// Stops the search in flight of the window (`ConflictFixSearch::cancel`)
#[instrument(skip_all, fields(window = %call.window))]
pub fn cancel_conflict_fix(ctx: &AppContext, call: &Call) {
  ctx.conflict_fix.cancel(&call.window);
}
