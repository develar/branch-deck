use anyhow::Result;
use git_ops::model::{BranchError, BranchSyncStatus};
use git_ops::progress::ProgressCallback;
use tauri::ipc::Channel;

pub use sync_types::{ProgressReporter, SyncEvent};

// Implement ProgressReporter for Tauri Channel
#[derive(Clone)]
pub struct TauriProgressReporter {
  channel: Channel<SyncEvent>,
}

impl TauriProgressReporter {
  pub fn new(channel: Channel<SyncEvent>) -> Self {
    Self { channel }
  }
}

impl ProgressReporter for TauriProgressReporter {
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
