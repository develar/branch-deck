use sync_types::{ProgressReporter, SyncEvent};
use tauri::ipc::Channel;

// Implement ProgressReporter for Tauri Channel
#[derive(Clone)]
pub struct TauriProgressReporter {
  channel: Channel<SyncEvent>,
}

impl TauriProgressReporter {
  pub const fn new(channel: Channel<SyncEvent>) -> Self {
    Self { channel }
  }
}

impl ProgressReporter for TauriProgressReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    self.channel.send(event)?;
    Ok(())
  }
}
