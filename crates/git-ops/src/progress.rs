use anyhow::Result;

/// Generic progress callback trait that git-ops functions can use
/// This allows the git-ops crate to be UI-framework agnostic
pub trait ProgressCallback {
  /// Send branch status update
  fn send_branch_status(&self, branch_name: String, status: crate::model::BranchSyncStatus, error: Option<crate::model::BranchError>) -> Result<()>;
}

/// A no-op implementation for when progress isn't needed
pub struct NoOpProgress;

impl ProgressCallback for NoOpProgress {
  fn send_branch_status(&self, _branch_name: String, _status: crate::model::BranchSyncStatus, _error: Option<crate::model::BranchError>) -> Result<()> {
    Ok(())
  }
}

/// Progress context for cherry-pick operations
pub struct CherryPickProgress<'a> {
  pub callback: &'a dyn ProgressCallback,
  pub branch_name: &'a str,
  pub task_index: i16,
}

impl<'a> std::fmt::Debug for CherryPickProgress<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CherryPickProgress")
      .field("branch_name", &self.branch_name)
      .field("task_index", &self.task_index)
      .finish()
  }
}

impl<'a> CherryPickProgress<'a> {
  pub fn new(callback: &'a dyn ProgressCallback, branch_name: &'a str, task_index: i16) -> Self {
    Self {
      callback,
      branch_name,
      task_index,
    }
  }

  pub fn send_status(&self, status: crate::model::BranchSyncStatus, error: Option<crate::model::BranchError>) -> Result<()> {
    self.callback.send_branch_status(self.branch_name.to_string(), status, error)
  }
}
