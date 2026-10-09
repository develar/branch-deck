use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::CommitInfo;
use model_ai::generator::ModelBasedBranchGenerator as CoreGenerator;
use model_ai::path_provider::ModelPathProvider;
use model_core::config::ModelConfig;
use model_core::utils::clean_branch_name;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use tokio::sync::Mutex;
use tracing::info;

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

  pub fn get_model_path(&mut self, provider: &dyn ModelPathProvider) -> Result<std::path::PathBuf> {
    self.core.get_model_path(provider)
  }

  pub fn get_model_config(&self) -> ModelConfig {
    self.core.get_model_config()
  }

  pub async fn set_model_config(&mut self, config: ModelConfig) -> Result<()> {
    self.core.set_model_config(config).await
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

  pub async fn ensure_model_loaded(&mut self, provider: &dyn ModelPathProvider) -> Result<()> {
    self.core.ensure_model_loaded(provider).await
  }

  pub fn is_loaded(&self) -> bool {
    self.core.is_loaded()
  }

  pub async fn generate_branch_names_stream(
    &mut self,
    git_executor: &GitCommandExecutor,
    commits: &[CommitInfo],
    repository_path: &str,
    progress: &tauri::ipc::Channel<model_ai::types::SuggestionProgress>,
    current_generation_id: &AtomicU64,
    my_generation_id: u64,
  ) -> Result<()> {
    use model_ai::types::{BranchSuggestion, SuggestionProgress};

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

    // Check if we should continue with this generation
    if my_generation_id != current_generation_id.load(std::sync::atomic::Ordering::SeqCst) {
      info!("Generation {} cancelled, newer generation exists", my_generation_id);
      progress.send(SuggestionProgress::Cancelled).ok();
      return Ok(());
    }

    // Generate primary suggestion
    let result = self.core.generate_branch_name(&git_output, None).await?;

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
      // Check again before generating alternative
      if my_generation_id != current_generation_id.load(std::sync::atomic::Ordering::SeqCst) {
        info!("Generation {} cancelled before alternative, newer generation exists", my_generation_id);
        return Ok(());
      }

      // Generate alternative using the same git output but with context of the first suggestion
      let fallback_result = self.core.generate_branch_name(&git_output, Some(&cleaned_name)).await;

      if let Ok(fallback_result) = fallback_result
        && let Ok(fallback_name) = clean_branch_name(&fallback_result.name)
        && fallback_name != cleaned_name
      {
        // Send alternative suggestion
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

    Ok(())
  }
}

// State wrapper for Tauri
pub struct ModelGeneratorState {
  pub generator: Mutex<ModelBasedBranchGenerator>,
  /// Id of the latest suggestion request. Kept outside the generator mutex so a new request can
  /// supersede a running generation without waiting for its lock.
  pub current_generation_id: AtomicU64,
  pub download_cancelled: Arc<AtomicBool>,
}

impl ModelGeneratorState {
  pub fn new(generator: ModelBasedBranchGenerator) -> Self {
    Self {
      generator: Mutex::new(generator),
      current_generation_id: AtomicU64::new(0),
      download_cancelled: Arc::new(AtomicBool::new(false)),
    }
  }
}
