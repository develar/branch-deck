use anyhow::Result;

/// Generic progress callback trait that git-ops functions can use
/// This allows the git-ops crate to be UI-framework agnostic
pub trait ProgressCallback: Send + Sync {
  /// Send branch status update
  fn send_branch_status(&self, branch_name: String, status: crate::model::BranchSyncStatus, error: Option<crate::model::BranchError>) -> Result<()>;
}

/// Progress context for cherry-pick operations
pub struct CherryPickProgress<'a> {
  pub callback: &'a dyn ProgressCallback,
  pub branch_name: &'a str,
}

impl std::fmt::Debug for CherryPickProgress<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CherryPickProgress").field("branch_name", &self.branch_name).finish()
  }
}

impl<'a> CherryPickProgress<'a> {
  pub fn new(callback: &'a dyn ProgressCallback, branch_name: &'a str) -> Self {
    Self { callback, branch_name }
  }

  pub fn send_status(&self, status: crate::model::BranchSyncStatus, error: Option<crate::model::BranchError>) -> Result<()> {
    self.callback.send_branch_status(self.branch_name.to_string(), status, error)
  }
}
