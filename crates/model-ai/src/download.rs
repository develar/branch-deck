use crate::path_provider::ModelPathProvider;
use anyhow::Result;
use futures_util::StreamExt;
use model_core::ModelConfig;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

/// Trait for reporting download progress
pub trait ProgressReporter: Send + Sync {
  fn report_started(&self, total_files: u32) -> Result<()>;
  fn report_file_started(&self, file_name: &str, file_size: Option<u32>) -> Result<()>;
  fn report_progress(&self, file_name: &str, downloaded: u32, total: u32, bytes_per_second: Option<u32>, seconds_remaining: Option<u32>) -> Result<()>;
  fn report_file_completed(&self, file_name: &str) -> Result<()>;
  fn report_completed(&self) -> Result<()>;
  fn report_error(&self, message: &str) -> Result<()>;
}

/// Console progress reporter for tests and CLI usage
pub struct ConsoleProgressReporter;

impl ProgressReporter for ConsoleProgressReporter {
  fn report_started(&self, total_files: u32) -> Result<()> {
    println!("📥 Starting download of {total_files} files");
    Ok(())
  }

  fn report_file_started(&self, file_name: &str, file_size: Option<u32>) -> Result<()> {
    if let Some(size) = file_size {
      println!("  ↓ Downloading {} ({:.2} MB)...", file_name, size as f64 / 1_000_000.0);
    } else {
      println!("  ↓ Downloading {file_name}...");
    }
    Ok(())
  }

  fn report_progress(&self, file_name: &str, downloaded: u32, total: u32, bytes_per_second: Option<u32>, seconds_remaining: Option<u32>) -> Result<()> {
    let percent = (downloaded as f64 / total as f64 * 100.0) as u32;
    print!("\r  ↓ {file_name} - {percent}% ");

    if let Some(bps) = bytes_per_second {
      print!("({:.1} MB/s) ", bps as f64 / 1_000_000.0);
    }

    if let Some(secs) = seconds_remaining {
      if secs > 0 {
        print!("- {} remaining", format_duration(secs));
      }
    }

    use std::io::Write;
    std::io::stdout().flush().ok();
    Ok(())
  }

  fn report_file_completed(&self, file_name: &str) -> Result<()> {
    println!("\r  ✓ {file_name} downloaded");
    Ok(())
  }

  fn report_completed(&self) -> Result<()> {
    println!("✅ All model files downloaded successfully");
    Ok(())
  }

  fn report_error(&self, message: &str) -> Result<()> {
    eprintln!("❌ Error: {message}");
    Ok(())
  }
}

fn format_duration(seconds: u32) -> String {
  if seconds < 60 {
    format!("{seconds}s")
  } else if seconds < 3600 {
    format!("{}m {}s", seconds / 60, seconds % 60)
  } else {
    format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
  }
}

/// Creates an HTTP client with retry middleware
fn create_http_client() -> Result<ClientWithMiddleware> {
  // Create base reqwest client with connection pooling
  let reqwest_client = reqwest::Client::builder()
    .user_agent("branch-deck/0.1.0")
    .timeout(Duration::from_secs(300))
    .redirect(reqwest::redirect::Policy::limited(10))
    .pool_max_idle_per_host(2)
    .pool_idle_timeout(Duration::from_secs(90))
    .build()
    .map_err(|e| {
      error!("Failed to create HTTP client: {}", e);
      anyhow::anyhow!("Failed to create HTTP client: {}", e)
    })?;

  // Create retry policy: 3 retries with exponential backoff
  let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

  // Wrap with retry middleware
  let client = ClientBuilder::new(reqwest_client).with(RetryTransientMiddleware::new_with_policy(retry_policy)).build();

  Ok(client)
}

/// Downloads a single file with progress tracking
#[instrument(skip(client, progress, cancelled), fields(filename = %filename, url = %url))]
async fn download_file(
  client: &ClientWithMiddleware,
  url: &str,
  file_path: &std::path::Path,
  filename: &str,
  expected_size: Option<u32>,
  progress: &dyn ProgressReporter,
  cancelled: Option<Arc<AtomicBool>>,
) -> Result<()> {
  // Check if we can resume a partial download
  let mut resume_from = 0u64;
  let temp_path = file_path.with_extension("download");

  if temp_path.exists() {
    match tokio::fs::metadata(&temp_path).await {
      Ok(metadata) => {
        resume_from = metadata.len();
        info!("Found partial download for {}, resuming from byte {}", filename, resume_from);
      }
      Err(e) => {
        warn!("Failed to check partial download metadata: {}", e);
        // Delete corrupt partial file
        let _ = tokio::fs::remove_file(&temp_path).await;
      }
    }
  }

  // Build request with optional range header for resume
  let mut request = client.get(url);
  if resume_from > 0 {
    request = request.header("Range", format!("bytes={resume_from}-"));
  }

  // Execute request
  let response = request.send().await.map_err(|e| {
    let msg = format!("Failed to download {filename}: {e}");
    error!("{}", msg);
    let _ = progress.report_error(&msg);
    anyhow::anyhow!("{}", msg)
  })?;

  // Check response status
  let status = response.status();
  if !status.is_success() && status.as_u16() != 206 {
    // 206 is Partial Content for resumed downloads
    let msg = format!("Failed to download {filename}: HTTP {status}");
    error!("{}", msg);
    let _ = progress.report_error(&msg);
    return Err(anyhow::anyhow!("{}", msg));
  }

  // Get content length
  let content_length = response.content_length().unwrap_or(0);
  let total_size = if resume_from > 0 && status.as_u16() == 206 {
    // For resumed downloads, add the already downloaded bytes
    content_length + resume_from
  } else {
    content_length
  };

  // For large files, download with streaming progress
  if expected_size.is_some() && total_size > 0 {
    // Open file for writing (append if resuming)
    let mut file = if resume_from > 0 {
      tokio::fs::OpenOptions::new().write(true).append(true).open(&temp_path).await
    } else {
      tokio::fs::File::create(&temp_path).await
    }
    .map_err(|e| {
      let msg = format!("Failed to create file {filename}: {e}");
      error!("{}", msg);
      let _ = progress.report_error(&msg);
      anyhow::anyhow!("{}", msg)
    })?;

    // Get response stream
    let mut stream = response.bytes_stream();
    let mut downloaded = resume_from;
    let mut last_progress_update = std::time::Instant::now();
    let download_start = std::time::Instant::now();
    let mut last_percentage = if total_size > 0 { (resume_from * 100 / total_size) as u32 } else { 0 };

    // Send initial progress if resuming
    if resume_from > 0 {
      let _ = progress.report_progress(filename, downloaded.min(u32::MAX as u64) as u32, total_size.min(u32::MAX as u64) as u32, None, None);
    }

    use tokio::io::AsyncWriteExt;

    // Stream download with progress updates
    while let Some(chunk_result) = stream.next().await {
      // Check for cancellation before processing each chunk
      if let Some(ref cancel_flag) = cancelled {
        if cancel_flag.load(Ordering::Relaxed) {
          info!("Download cancelled for {}", filename);
          return Err(anyhow::anyhow!("Download cancelled"));
        }
      }

      let chunk = chunk_result.map_err(|e| {
        let msg = format!("Failed to download chunk for {filename}: {e}");
        error!("{}", msg);
        let _ = progress.report_error(&msg);
        anyhow::anyhow!("{}", msg)
      })?;

      // Write chunk to file
      file.write_all(&chunk).await.map_err(|e| {
        let msg = format!("Failed to write chunk for {filename}: {e}");
        error!("{}", msg);
        let _ = progress.report_error(&msg);
        anyhow::anyhow!("{}", msg)
      })?;

      downloaded += chunk.len() as u64;

      // Calculate current percentage
      let current_percentage = if total_size > 0 { (downloaded * 100 / total_size) as u32 } else { 0 };

      // Send progress update every 1000ms and when percentage changes by at least 1%
      if last_progress_update.elapsed() > Duration::from_millis(1000) && (current_percentage > last_percentage || current_percentage == 100) {
        last_percentage = current_percentage;

        // Calculate speed and time remaining
        let elapsed_secs = download_start.elapsed().as_secs_f64();
        let bytes_downloaded_in_session = downloaded - resume_from;
        let bytes_per_second = if elapsed_secs > 0.0 {
          Some((bytes_downloaded_in_session as f64 / elapsed_secs) as u32)
        } else {
          None
        };

        let seconds_remaining = if let Some(bps) = bytes_per_second {
          if bps > 0 {
            let remaining_bytes = total_size.saturating_sub(downloaded);
            Some((remaining_bytes as f64 / bps as f64) as u32)
          } else {
            None
          }
        } else {
          None
        };

        let _ = progress.report_progress(
          filename,
          downloaded.min(u32::MAX as u64) as u32,
          total_size.min(u32::MAX as u64) as u32,
          bytes_per_second,
          seconds_remaining,
        );
        last_progress_update = std::time::Instant::now();
      }
    }

    // Final progress update
    let _ = progress.report_progress(filename, downloaded.min(u32::MAX as u64) as u32, total_size.min(u32::MAX as u64) as u32, None, Some(0));

    // Flush file
    file.flush().await.map_err(|e| {
      let msg = format!("Failed to flush file {filename}: {e}");
      error!("{}", msg);
      let _ = progress.report_error(&msg);
      anyhow::anyhow!("{}", msg)
    })?;

    // Move temp file to final location
    tokio::fs::rename(&temp_path, file_path).await.map_err(|e| {
      let msg = format!("Failed to move downloaded file {filename}: {e}");
      error!("{}", msg);
      let _ = progress.report_error(&msg);
      anyhow::anyhow!("{}", msg)
    })?;

    let _ = progress.report_file_completed(filename);
  } else {
    // For small files, download without progress tracking
    let bytes = response.bytes().await.map_err(|e| {
      let msg = format!("Failed to read response for {filename}: {e}");
      error!("{}", msg);
      let _ = progress.report_error(&msg);
      anyhow::anyhow!("{}", msg)
    })?;

    tokio::fs::write(file_path, &bytes).await.map_err(|e| {
      let msg = format!("Failed to write {filename}: {e}");
      error!("{}", msg);
      let _ = progress.report_error(&msg);
      anyhow::anyhow!("{}", msg)
    })?;
  }

  Ok(())
}

/// Download model files with optional cancellation support
#[instrument(skip(provider, progress, cancelled), fields(model = %model_config.model_name()))]
pub async fn download_model_files(model_config: &ModelConfig, provider: &dyn ModelPathProvider, progress: &dyn ProgressReporter, cancelled: Option<Arc<AtomicBool>>) -> Result<()> {
  let cache_dir = provider.get_cache_dir()?;
  let model_path = cache_dir.join("models").join(model_config.model_id());

  // Get download URLs based on model config
  let files = model_config.download_urls();

  // Send start event with correct file count
  progress.report_started(files.len() as u32)?;

  // Create model directory if it doesn't exist
  tokio::fs::create_dir_all(&model_path).await.map_err(|e| {
    let msg = format!("Failed to create model directory: {e}");
    error!("{}", msg);
    let _ = progress.report_error(&msg);
    anyhow::anyhow!("{}", msg)
  })?;

  // Create HTTP client with retry middleware
  let client = create_http_client()?;

  for (filename, url, expected_size) in files {
    let file_path = model_path.join(filename);

    // Skip if file already exists
    if file_path.exists() {
      info!("File already exists: {:?}", file_path);
      continue;
    }

    // Only send file started event for the large file
    if expected_size.is_some() {
      progress.report_file_started(filename, expected_size)?;
    }

    info!("Downloading {}...", filename);

    // Check for cancellation before starting each file
    if let Some(ref cancel_flag) = cancelled {
      if cancel_flag.load(Ordering::Relaxed) {
        info!("Download cancelled before starting {}", filename);
        return Err(anyhow::anyhow!("Download cancelled"));
      }
    }

    // Download file with retry and resume support
    match download_file(&client, url, &file_path, filename, expected_size, progress, cancelled.clone()).await {
      Ok(_) => {
        info!("Successfully downloaded {}", filename);
      }
      Err(e) => {
        // Clean up partial downloads on failure
        let temp_path = file_path.with_extension("download");
        if temp_path.exists() {
          warn!("Cleaning up partial download for {}", filename);
          let _ = tokio::fs::remove_file(&temp_path).await;
        }
        return Err(e);
      }
    }
  }

  progress.report_completed()?;
  info!("All model files downloaded successfully");
  Ok(())
}
