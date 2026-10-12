use crate::generator::model_dir;
use crate::path_provider::ModelPathProvider;
use anyhow::Result;
use futures_util::StreamExt;
use model_core::config::ModelConfig;
use reqwest::StatusCode;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
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
      println!("  ↓ Downloading {} ({:.2} MB)...", file_name, f64::from(size) / 1_000_000.0);
    } else {
      println!("  ↓ Downloading {file_name}...");
    }
    Ok(())
  }

  fn report_progress(&self, file_name: &str, downloaded: u32, total: u32, bytes_per_second: Option<u32>, seconds_remaining: Option<u32>) -> Result<()> {
    use std::io::Write;
    let percent = approx_ratio_u32(u64::from(downloaded) * 100, f64::from(total));
    print!("\r  ↓ {file_name} - {percent}% ");

    if let Some(bps) = bytes_per_second {
      print!("({:.1} MB/s) ", f64::from(bps) / 1_000_000.0);
    }

    if let Some(secs) = seconds_remaining
      && secs > 0
    {
      print!("- {} remaining", format_duration(secs));
    }

    std::io::stdout().flush().ok();
    Ok(())
  }

  fn report_file_completed(&self, file_name: &str) -> Result<()> {
    println!("\r  ✓ {file_name} downloaded");
    Ok(())
  }

  fn report_completed(&self) -> Result<()> {
    tracing::info!("✅ All model files downloaded successfully");
    Ok(())
  }

  fn report_error(&self, message: &str) -> Result<()> {
    tracing::error!(message, "❌ Error");
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
  // No overall request timeout: a 1.3 GB download on a slow connection legitimately takes longer than any fixed
  // limit. A stalled transfer is caught by the read timeout instead.
  let reqwest_client = reqwest::Client::builder()
    .user_agent("branch-deck/0.1.0")
    .connect_timeout(Duration::from_secs(30))
    .read_timeout(Duration::from_secs(60))
    .redirect(reqwest::redirect::Policy::limited(10))
    .pool_max_idle_per_host(2)
    .pool_idle_timeout(Duration::from_secs(90))
    .build()
    .map_err(|e| {
      error!("Failed to create HTTP client: {}", e);
      anyhow::anyhow!("Failed to create HTTP client: {e}")
    })?;

  // Create retry policy: 3 retries with exponential backoff
  let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

  // Wrap with retry middleware
  let client = ClientBuilder::new(reqwest_client).with(RetryTransientMiddleware::new_with_policy(retry_policy)).build();

  Ok(client)
}

/// Logs `msg`, reports it to the frontend and turns it into an error
fn download_error(progress: &dyn ProgressReporter, msg: String) -> anyhow::Error {
  error!("{}", msg);
  let _ = progress.report_error(&msg);
  anyhow::anyhow!(msg)
}

/// Path of the partial download of `file_path`
fn partial_path(file_path: &Path) -> PathBuf {
  file_path.with_extension("download")
}

/// Size of an existing partial download, `0` if there is none
async fn partial_size(temp_path: &Path, filename: &str) -> u64 {
  match tokio::fs::metadata(temp_path).await {
    Ok(metadata) => {
      let size = metadata.len();
      if size > 0 {
        info!("Found partial download for {}, resuming from byte {}", filename, size);
      }
      size
    }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
    Err(e) => {
      // Download from scratch; the full (200) response truncates the partial file
      warn!("Failed to check partial download metadata: {}", e);
      0
    }
  }
}

/// Downloads a single file into `<file_path>.download` with progress tracking, then moves it to `file_path`.
///
/// An existing partial download is resumed with a `Range` request. The partial file is left in place on error and on
/// cancellation (pause), so the next attempt can resume it. It is discarded only when the server rejects the range
/// (416) or answers a resume request with the full content (200, truncate and restart).
#[instrument(skip(client, progress, cancelled), fields(filename = %filename, url = %url))]
async fn download_file(client: &ClientWithMiddleware, url: &str, file_path: &Path, filename: &str, progress: &dyn ProgressReporter, cancelled: Option<&AtomicBool>) -> Result<()> {
  let temp_path = partial_path(file_path);
  let resume_from = partial_size(&temp_path, filename).await;

  let mut request = client.get(url);
  if resume_from > 0 {
    request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
  }

  let response = request.send().await.map_err(|e| download_error(progress, format!("Failed to download {filename}: {e}")))?;

  let status = response.status();
  let content_length = response.content_length();
  // `downloaded` counts the bytes already in the partial file
  let (open_result, mut downloaded) = match status {
    // The server honoured the range: append to the partial file
    StatusCode::PARTIAL_CONTENT => (tokio::fs::OpenOptions::new().create(true).append(true).open(&temp_path).await, resume_from),
    // Full content (no partial file, or the server ignored the range): start over
    StatusCode::OK => (tokio::fs::File::create(&temp_path).await, 0),
    StatusCode::RANGE_NOT_SATISFIABLE => {
      // The partial file does not match the remote file (e.g., it is complete or the file changed); the next attempt
      // starts from scratch
      let _ = tokio::fs::remove_file(&temp_path).await;
      return Err(download_error(progress, format!("Failed to resume {filename}: HTTP {status}, partial download discarded")));
    }
    _ => return Err(download_error(progress, format!("Failed to download {filename}: HTTP {status}"))),
  };
  let mut file = open_result.map_err(|e| download_error(progress, format!("Failed to create file {filename}: {e}")))?;
  let session_start = downloaded;
  // 0 when the server sends no length
  let total_size = content_length.map_or(0, |length| length + downloaded);

  let mut stream = response.bytes_stream();
  let mut last_progress_update = std::time::Instant::now();
  let download_start = std::time::Instant::now();
  let mut last_percentage = saturating_u32((downloaded * 100).checked_div(total_size).unwrap_or(0));

  // Send initial progress if resuming
  if downloaded > 0 {
    let _ = progress.report_progress(filename, saturating_u32(downloaded), saturating_u32(total_size), None, None);
  }

  while let Some(chunk_result) = stream.next().await {
    // Check for cancellation before processing each chunk
    if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
      info!("Download cancelled for {}", filename);
      return Err(anyhow::anyhow!("Download cancelled"));
    }

    let chunk = chunk_result.map_err(|e| download_error(progress, format!("Failed to download chunk for {filename}: {e}")))?;
    file
      .write_all(&chunk)
      .await
      .map_err(|e| download_error(progress, format!("Failed to write chunk for {filename}: {e}")))?;
    downloaded += chunk.len() as u64;

    let current_percentage = saturating_u32((downloaded * 100).checked_div(total_size).unwrap_or(0));

    // Send progress update every 1000ms and when percentage changes by at least 1%
    if last_progress_update.elapsed() > Duration::from_millis(1000) && (current_percentage > last_percentage || current_percentage == 100) {
      last_percentage = current_percentage;

      // Calculate speed and time remaining
      let elapsed_secs = download_start.elapsed().as_secs_f64();
      let bytes_per_second = (elapsed_secs > 0.0).then(|| approx_ratio_u32(downloaded - session_start, elapsed_secs));
      let seconds_remaining = bytes_per_second
        .filter(|bps| *bps > 0)
        .map(|bps| approx_ratio_u32(total_size.saturating_sub(downloaded), f64::from(bps)));

      let _ = progress.report_progress(filename, saturating_u32(downloaded), saturating_u32(total_size), bytes_per_second, seconds_remaining);
      last_progress_update = std::time::Instant::now();
    }
  }

  file.flush().await.map_err(|e| download_error(progress, format!("Failed to flush file {filename}: {e}")))?;
  drop(file);

  if total_size > 0 && downloaded != total_size {
    return Err(download_error(
      progress,
      format!("Incomplete download of {filename}: got {downloaded} of {total_size} bytes"),
    ));
  }

  // Final progress update
  let _ = progress.report_progress(filename, saturating_u32(downloaded), saturating_u32(total_size), None, Some(0));

  // Move temp file to final location
  tokio::fs::rename(&temp_path, file_path)
    .await
    .map_err(|e| download_error(progress, format!("Failed to move downloaded file {filename}: {e}")))?;

  let _ = progress.report_file_completed(filename);
  Ok(())
}

fn saturating_u32(value: u64) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

/// Approximate `numerator / denominator` for progress statistics.
#[expect(
  clippy::cast_precision_loss,
  clippy::cast_possible_truncation,
  clippy::cast_sign_loss,
  reason = "approximate progress statistics; float-to-int `as` saturates and maps NaN to 0"
)]
fn approx_ratio_u32(numerator: u64, denominator: f64) -> u32 {
  (numerator as f64 / denominator) as u32
}

/// Download model files with optional cancellation support.
///
/// A failed or cancelled (paused) download keeps its partial file so the next call resumes it.
#[instrument(skip(provider, progress, cancelled), fields(model = %model_config.model_name()))]
pub async fn download_model_files(model_config: &ModelConfig, provider: &dyn ModelPathProvider, progress: &dyn ProgressReporter, cancelled: Option<Arc<AtomicBool>>) -> Result<()> {
  let model_path = model_dir(provider, *model_config)?;
  let files = model_config.files();
  let cancelled = cancelled.as_deref();

  // Send start event with correct file count
  progress.report_started(u32::try_from(files.len()).unwrap_or(u32::MAX))?;

  // Create model directory if it doesn't exist
  tokio::fs::create_dir_all(&model_path)
    .await
    .map_err(|e| download_error(progress, format!("Failed to create model directory: {e}")))?;

  // Create HTTP client with retry middleware
  let client = create_http_client()?;

  for model_file in files {
    let filename = model_file.name;
    let file_path = model_path.join(filename);

    // Skip if file already exists
    if file_path.exists() {
      info!("File already exists: {:?}", file_path);
      continue;
    }

    // Check for cancellation before starting each file
    if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
      info!("Download cancelled before starting {}", filename);
      return Err(anyhow::anyhow!("Download cancelled"));
    }

    progress.report_file_started(filename, model_file.approx_size)?;
    info!("Downloading {}...", filename);

    download_file(&client, model_file.url, &file_path, filename, progress, cancelled).await?;
    info!("Successfully downloaded {}", filename);
  }

  progress.report_completed()?;
  info!("All model files downloaded successfully");
  Ok(())
}
