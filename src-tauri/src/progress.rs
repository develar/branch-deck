use anyhow::Result;
use git_ops::model::{BranchError, BranchSyncStatus};
use git_ops::progress::ProgressCallback;
use tauri::ipc::Channel;

// Re-export types from branch-sync crate
pub use branch_sync::progress::{GroupedBranchInfo, ProgressReporter, SyncEvent};

// Implement ProgressReporter for Tauri Channel
pub struct TauriProgressReporter<'a> {
  channel: &'a Channel<SyncEvent>,
}

impl<'a> TauriProgressReporter<'a> {
  pub fn new(channel: &'a Channel<SyncEvent>) -> Self {
    Self { channel }
  }
}

impl<'a> ProgressReporter for TauriProgressReporter<'a> {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.channel.send(event)?;
    Ok(())
  }
}

/// Adapter that implements ProgressCallback for Tauri Channel
/// This allows git-ops to send progress updates through Tauri IPC
pub struct TauriChannelProgress<'a> {
  channel: &'a Channel<SyncEvent>,
}

impl<'a> TauriChannelProgress<'a> {
  pub fn new(channel: &'a Channel<SyncEvent>) -> Self {
    Self { channel }
  }
}

impl<'a> ProgressCallback for TauriChannelProgress<'a> {
  fn send_branch_status(&self, branch_name: String, status: BranchSyncStatus, error: Option<BranchError>) -> Result<()> {
    self.channel.send(SyncEvent::BranchStatusUpdate { branch_name, status, error })?;
    Ok(())
  }
}
