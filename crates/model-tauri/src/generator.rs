use anyhow::Result;
use git_ops::git_command::GitCommandExecutor;
use git_ops::model::CommitInfo;
use model_ai::generator::{ModelBasedBranchGenerator as CoreGenerator, ALTERNATIVE_CONFIDENCE, PRIMARY_CONFIDENCE};
use model_ai::path_provider::ModelPathProvider;
use model_core::utils::clean_branch_name;
use model_core::ModelConfig;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug)]
pub struct ModelBasedBranchGenerator {
  core: CoreGenerator,
  current_generation_id: Arc<AtomicU64>,
}

impl ModelBasedBranchGenerator {
  pub fn new() -> Result<Self> {
    Ok(Self {
      core: CoreGenerator::new()?,
      current_generation_id: Arc::new(AtomicU64::new(0)),
    })
  }

  pub fn with_config(model_config: ModelConfig) -> Result<Self> {
    Ok(Self {
      core: CoreGenerator::with_config(model_config)?,
      current_generation_id: Arc::new(AtomicU64::new(0)),
    })
  }

  pub fn get_model_path(&mut self, provider: &dyn ModelPathProvider) -> Result<std::path::PathBuf> {
    self.core.get_model_path(provider)
  }

  pub fn get_current_generation_id(&self) -> Arc<AtomicU64> {
    Arc::clone(&self.current_generation_id)
  }

  pub fn get_model_config(&self) -> ModelConfig {
    self.core.get_model_config()
  }

  pub async fn set_model_config(&mut self, config: ModelConfig) -> Result<()> {
    self.core.set_model_config(config).await
  }

  async fn create_enhanced_prompt(&self, git_output: &str) -> Result<String> {
    self.core.create_enhanced_prompt(git_output).await
  }

  fn get_git_output_for_commits(&self, git_executor: &GitCommandExecutor, commits: &[CommitInfo], repo_path: &str) -> Result<String> {
    if commits.is_empty() {
      return Ok(String::new());
    }

    // Collect all commit hashes for batch processing
    let commit_hashes: Vec<&str> = commits.iter().map(|c| c.hash.as_str()).collect();

    // Single git command for all commits
    let mut args = vec![
      "--no-pager",
      "show",
      "--format=%s%n%b", // Subject and body (similar to git log format)
      "--name-status",   // Show file status changes (A/M/D + filename)
      "--no-patch",      // Don't show patch content, just metadata
    ];

    // Add all commit hashes to the single command
    args.extend(commit_hashes);

    let git_output = git_executor.execute_command(&args, repo_path)?;

    info!(
      "Generated git output with {} characters from {} commits in single batch git command",
      git_output.len(),
      commits.len()
    );

    Ok(git_output)
  }

  pub async fn ensure_model_loaded(&mut self, provider: &dyn ModelPathProvider) -> Result<()> {
    self.core.ensure_model_loaded(provider).await
  }

  pub fn is_loaded(&self) -> bool {
    self.core.is_loaded()
  }

  // Removed old complex loading methods - now using model-core directly

  pub async fn generate_branch_names_stream(
    &mut self,
    git_executor: &GitCommandExecutor,
    commits: &[CommitInfo],
    repository_path: &str,
    progress: &tauri::ipc::Channel<crate::types::SuggestionProgress>,
    my_generation_id: u64,
  ) -> Result<()> {
    use crate::types::{BranchSuggestion, SuggestionProgress};

    // Validate commits have non-empty hashes first, before checking model
    let valid_commits: Vec<&CommitInfo> = commits.iter().filter(|c| !c.hash.is_empty()).collect();

    if valid_commits.is_empty() {
      return Err(anyhow::anyhow!("No valid commits provided (all have empty hashes)"));
    }

    if !self.core.is_loaded() {
      return Err(anyhow::anyhow!("Model not loaded"));
    }

    // Get git output once and reuse it
    let git_output = self.get_git_output_for_commits(git_executor, commits, repository_path)?;
    let prompt = self.create_enhanced_prompt(&git_output).await?;
    debug!("Generated prompt with {} chars", prompt.len());

    // Check if we should continue with this generation
    if my_generation_id != self.current_generation_id.load(std::sync::atomic::Ordering::SeqCst) {
      info!("Generation {} cancelled, newer generation exists", my_generation_id);
      progress.send(SuggestionProgress::Cancelled).ok();
      return Ok(());
    }

    // Generate primary suggestion
    let result = self.core.generate_branch_name(&prompt).await?;

    let cleaned_name = clean_branch_name(&result.name)?;

    // Send primary suggestion immediately
    progress
      .send(SuggestionProgress::SuggestionReady {
        suggestion: BranchSuggestion {
          name: cleaned_name.clone(),
          confidence: result.confidence.max(PRIMARY_CONFIDENCE),
          reason: Some(format!("AI-generated in {}ms (confidence: {:.1}%)", result.generation_time_ms, result.confidence * 100.0)),
        },
        index: 0,
      })
      .map_err(|e| anyhow::anyhow!("Failed to send primary suggestion: {e}"))?;

    // Generate alternative suggestion if primary was successful
    if !cleaned_name.is_empty() {
      // Check again before generating alternative
      if my_generation_id != self.current_generation_id.load(std::sync::atomic::Ordering::SeqCst) {
        info!("Generation {} cancelled before alternative, newer generation exists", my_generation_id);
        return Ok(());
      }

      let fallback_result = self.core.generate_alternative_branch_name(&prompt).await;

      if let Ok(fallback_result) = fallback_result {
        if let Ok(fallback_name) = clean_branch_name(&fallback_result.name) {
          if fallback_name != cleaned_name {
            // Send alternative suggestion
            progress
              .send(SuggestionProgress::SuggestionReady {
                suggestion: BranchSuggestion {
                  name: fallback_name,
                  confidence: ALTERNATIVE_CONFIDENCE,
                  reason: Some("Alternative suggestion".to_string()),
                },
                index: 1,
              })
              .map_err(|e| anyhow::anyhow!("Failed to send alternative suggestion: {e}"))?;
          }
        }
      }
    }

    Ok(())
  }
}

// State wrapper for Tauri
pub struct ModelGeneratorState(pub Mutex<ModelBasedBranchGenerator>);
