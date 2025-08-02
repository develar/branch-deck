use anyhow::Result;
use model_ai::download::ProgressReporter;
use model_ai::types::DownloadProgress;
use tauri::ipc::Channel;

macro_rules! send_progress {
  ($channel:expr, $progress:expr) => {
    $channel.send($progress).map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))
  };
}

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
    send_progress!(self.channel, DownloadProgress::Started { total_files })
  }

  fn report_file_started(&self, file_name: &str, file_size: Option<u32>) -> Result<()> {
    send_progress!(
      self.channel,
      DownloadProgress::FileStarted {
        file_name: file_name.to_string(),
        file_size,
      }
    )
  }

  fn report_progress(&self, file_name: &str, downloaded: u32, total: u32, bytes_per_second: Option<u32>, seconds_remaining: Option<u32>) -> Result<()> {
    send_progress!(
      self.channel,
      DownloadProgress::Progress {
        file_name: file_name.to_string(),
        downloaded,
        total,
        bytes_per_second,
        seconds_remaining,
      }
    )
  }

  fn report_file_completed(&self, file_name: &str) -> Result<()> {
    send_progress!(self.channel, DownloadProgress::FileCompleted { file_name: file_name.to_string() })
  }

  fn report_completed(&self) -> Result<()> {
    send_progress!(self.channel, DownloadProgress::Completed)
  }

  fn report_error(&self, message: &str) -> Result<()> {
    send_progress!(self.channel, DownloadProgress::Error { message: message.to_string() })
  }
}
