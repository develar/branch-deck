use crate::commands::download_model::DownloadProgress;
use anyhow::Result;
use model_ai::download::ProgressReporter;
use tauri::ipc::Channel;

/// Tauri progress reporter using channel
pub struct TauriProgressReporter {
  channel: Channel<DownloadProgress>,
}

impl TauriProgressReporter {
  pub fn new(channel: Channel<DownloadProgress>) -> Self {
    Self { channel }
  }
}

impl ProgressReporter for TauriProgressReporter {
  fn report_started(&self, total_files: u32) -> Result<()> {
    self
      .channel
      .send(DownloadProgress::Started { total_files })
      .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  }

  fn report_file_started(&self, file_name: &str, file_size: Option<u32>) -> Result<()> {
    self
      .channel
      .send(DownloadProgress::FileStarted {
        file_name: file_name.to_string(),
        file_size,
      })
      .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  }

  fn report_progress(&self, file_name: &str, downloaded: u32, total: u32, bytes_per_second: Option<u32>, seconds_remaining: Option<u32>) -> Result<()> {
    self
      .channel
      .send(DownloadProgress::Progress {
        file_name: file_name.to_string(),
        downloaded,
        total,
        bytes_per_second,
        seconds_remaining,
      })
      .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  }

  fn report_file_completed(&self, file_name: &str) -> Result<()> {
    self
      .channel
      .send(DownloadProgress::FileCompleted { file_name: file_name.to_string() })
      .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  }

  fn report_completed(&self) -> Result<()> {
    self
      .channel
      .send(DownloadProgress::Completed)
      .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  }

  fn report_error(&self, message: &str) -> Result<()> {
    self
      .channel
      .send(DownloadProgress::Error { message: message.to_string() })
      .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  }
}

// Re-export download_model_files from model-ai
pub use model_ai::download::download_model_files;
