use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::CommitInfo;
use model_ai::generator::ModelBasedBranchGenerator as CoreGenerator;
use model_ai::path_provider::ModelPathProvider;
use model_core::config::ModelConfig;
use model_core::quantized_qwen3::GenerationCancelled;
use model_core::utils::clean_branch_name;
use model_types::{BranchSuggestion, SuggestionProgress};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;
use tracing::{info, instrument};

/// Branch name generator for the Tauri commands.
///
/// All methods are blocking (git, model loading and inference); call them from a blocking thread.
#[derive(Debug)]
pub struct ModelBasedBranchGenerator {
  core: CoreGenerator,
}

impl ModelBasedBranchGenerator {
  pub fn new() -> Result<Self> {
    Ok(Self { core: CoreGenerator::new()? })
  }

  pub fn with_config(model_config: ModelConfig) -> Result<Self> {
    Ok(Self {
      core: CoreGenerator::with_config(model_config)?,
    })
  }

  pub fn get_model_config(&self) -> ModelConfig {
    self.core.get_model_config()
  }

  pub(crate) fn get_git_output_for_commits<'a>(git_executor: &GitCommandExecutor, commits: impl IntoIterator<Item = &'a CommitInfo>, repo_path: &str) -> Result<String> {
    // Collect all commit hashes for batch processing
    let commit_hashes: Vec<&str> = commits.into_iter().map(|c| c.hash.as_str()).collect();
    if commit_hashes.is_empty() {
      return Ok(String::new());
    }
    let commit_count = commit_hashes.len();

    // Single git command for all commits
    let mut args = vec![
      "--no-pager",
      "show",
      "--format=%s%n%b", // Subject and body (similar to git log format)
      "--name-status",   // Show file status changes (A/M/D + filename)
    ];

    // Add all commit hashes to the single command
    args.extend(commit_hashes);

    let git_output = git_executor.execute_command(&args, repo_path)?;

    info!(
      "Generated git output with {} characters from {} commits in single batch git command",
      git_output.len(),
      commit_count
    );

    Ok(git_output)
  }

  /// See [`CoreGenerator::ensure_model_loaded`]; fails with [`model_ai::generator::ModelNotDownloaded`] when a model
  /// file is missing.
  pub fn ensure_model_loaded(&mut self, provider: &dyn ModelPathProvider) -> Result<()> {
    self.core.ensure_model_loaded(provider)
  }

  pub fn is_loaded(&self) -> bool {
    self.core.is_loaded()
  }

  /// Generates the primary and the alternative suggestion and streams them to `progress`.
  ///
  /// Sends the terminal `Completed` event, or `Cancelled` as soon as `current_generation_id` no longer equals
  /// `my_generation_id` (a newer request superseded this one). Errors are returned, not sent.
  #[instrument(skip_all, fields(commits = commits.len(), my_generation_id))]
  pub fn generate_branch_names_stream(
    &mut self,
    git_executor: &GitCommandExecutor,
    commits: &[CommitInfo],
    repository_path: &str,
    progress: &Channel<SuggestionProgress>,
    current_generation_id: &AtomicU64,
    my_generation_id: u64,
  ) -> Result<()> {
    let should_cancel = || my_generation_id != current_generation_id.load(Ordering::SeqCst);
    let send_cancelled = || -> Result<()> {
      info!("Generation {} cancelled, newer generation exists", my_generation_id);
      progress.send(SuggestionProgress::Cancelled).ok();
      Ok(())
    };

    // Validate commits have non-empty hashes first, before checking model
    let valid_commits: Vec<&CommitInfo> = commits.iter().filter(|c| !c.hash.is_empty()).collect();

    if valid_commits.is_empty() {
      return Err(anyhow::anyhow!("No valid commits provided (all have empty hashes)"));
    }

    if !self.core.is_loaded() {
      return Err(anyhow::anyhow!("Model not loaded"));
    }

    // Get git output once and reuse it
    let git_output = Self::get_git_output_for_commits(git_executor, valid_commits, repository_path)?;

    if should_cancel() {
      return send_cancelled();
    }

    // Generate primary suggestion
    let result = match self.core.generate_branch_name(&git_output, None, &should_cancel) {
      Ok(result) => result,
      Err(e) if e.downcast_ref::<GenerationCancelled>().is_some() => return send_cancelled(),
      Err(e) => return Err(e),
    };

    let cleaned_name = clean_branch_name(&result.name)?;

    // Send primary suggestion immediately
    progress
      .send(SuggestionProgress::SuggestionReady {
        suggestion: BranchSuggestion {
          name: cleaned_name.clone(),
          reason: Some(format!("AI-generated in {}ms", result.generation_time_ms)),
        },
        index: 0,
      })
      .map_err(|e| anyhow::anyhow!("Failed to send primary suggestion: {e}"))?;

    // Generate alternative suggestion if primary was successful
    if !cleaned_name.is_empty() {
      if should_cancel() {
        return send_cancelled();
      }

      // Generate alternative using the same git output but with context of the first suggestion; its failure is not
      // an error because the primary suggestion is already there
      match self.core.generate_branch_name(&git_output, Some(&cleaned_name), &should_cancel) {
        Ok(fallback_result) => {
          if let Ok(fallback_name) = clean_branch_name(&fallback_result.name)
            && fallback_name != cleaned_name
          {
            progress
              .send(SuggestionProgress::SuggestionReady {
                suggestion: BranchSuggestion {
                  name: fallback_name,
                  reason: Some("Alternative suggestion".to_string()),
                },
                index: 1,
              })
              .map_err(|e| anyhow::anyhow!("Failed to send alternative suggestion: {e}"))?;
          }
        }
        Err(e) if e.downcast_ref::<GenerationCancelled>().is_some() => return send_cancelled(),
        Err(e) => info!("Alternative suggestion failed: {e:#}"),
      }
    }

    progress
      .send(SuggestionProgress::Completed)
      .map_err(|e| anyhow::anyhow!("Failed to send completion: {e}"))?;
    Ok(())
  }
}

// State wrapper for Tauri
pub struct ModelGeneratorState {
  /// Blocking mutex: the generator is only used from blocking threads, which hold it for the whole model load or
  /// inference.
  pub generator: Arc<Mutex<ModelBasedBranchGenerator>>,
  /// Copy of the generator's config, readable without waiting for a running generation
  pub model_config: ModelConfig,
  /// Id of the latest suggestion request. Kept outside the generator mutex so a new request can
  /// supersede a running generation without waiting for its lock.
  pub current_generation_id: Arc<AtomicU64>,
  pub download_cancelled: Arc<AtomicBool>,
}

impl ModelGeneratorState {
  pub fn new(generator: ModelBasedBranchGenerator) -> Self {
    Self {
      model_config: generator.get_model_config(),
      generator: Arc::new(Mutex::new(generator)),
      current_generation_id: Arc::new(AtomicU64::new(0)),
      download_cancelled: Arc::new(AtomicBool::new(false)),
    }
  }
}
