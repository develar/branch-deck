//! What the app keeps to answer `get_repository_view` (and the E2E server, with the same code): the branch table of the
//! last sync of each repository and branch prefix, and the memo of what its views asked by content. A sync that fails
//! keeps the table of the one before: a view is of the last table a sync finished.

use crate::branch_table::BranchTable;
use crate::patronus_view::{ContentMemo, patronus_view};
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use sync_types::repository_view::{GetRepositoryViewParams, PatronusView, RepositoryView};
use tracing::instrument;

/// How many repositories and branch prefixes the app keeps the views of
const KEPT: u64 = 32;

/// The table a sync finished, numbered
struct SyncedTable {
  generation: u32,
  table: BranchTable,
}

/// The tables and the memos, by repository path and branch prefix. Cloning shares them
#[derive(Clone)]
pub struct RepositoryViews {
  tables: Cache<(String, String), Arc<SyncedTable>>,
  memos: Cache<(String, String), Arc<Mutex<ContentMemo>>>,
  generation: Arc<AtomicU32>,
}

impl Default for RepositoryViews {
  fn default() -> Self {
    Self::with_capacity(KEPT)
  }
}

impl RepositoryViews {
  /// Keeps the views of at most `repositories` repositories and branch prefixes: the E2E server keeps those of every
  /// repository of a run, many more than the app opens
  pub fn with_capacity(repositories: u64) -> Self {
    Self {
      tables: Cache::new(repositories),
      memos: Cache::new(repositories),
      generation: Arc::new(AtomicU32::new(0)),
    }
  }

  /// Keeps the table a sync of `repository_path` under `branch_prefix` finished
  pub fn keep(&self, repository_path: &str, branch_prefix: &str, table: BranchTable) {
    let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
    self
      .tables
      .insert((repository_path.to_string(), branch_prefix.to_string()), Arc::new(SyncedTable { generation, table }));
  }

  /// The view of the kept table with the asker's robots, matches and remote facts; empty before the first sync. Runs
  /// git for the robots the hashes and the memo don't answer: call from a blocking context
  #[instrument(skip_all, fields(repository_path = %params.repository_path, revision = params.revision))]
  pub fn view(&self, git_executor: &GitCommandExecutor, params: &GetRepositoryViewParams) -> RepositoryView {
    let key = (params.repository_path.clone(), params.branch_prefix.clone());
    let Some(synced) = self.tables.get(&key) else {
      return RepositoryView {
        revision: params.revision,
        table_generation: 0,
        patronus: PatronusView::default(),
      };
    };
    let memo = self.memos.get_with(key, || Arc::new(Mutex::new(ContentMemo::default())));
    // a poisoned memo (a panic in an earlier view) starts again
    let mut memo = memo.lock().unwrap_or_else(|poisoned| {
      memo.clear_poison();
      poisoned.into_inner()
    });
    let patronus = patronus_view(
      git_executor,
      &params.repository_path,
      &synced.table,
      &params.remote,
      &params.robots,
      &params.matched,
      &mut memo,
    );
    RepositoryView {
      revision: params.revision,
      table_generation: synced.generation,
      patronus,
    }
  }
}

#[cfg(test)]
#[path = "views_test.rs"]
mod views_test;
