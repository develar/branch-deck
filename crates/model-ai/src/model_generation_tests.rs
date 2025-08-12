use crate::test_utils::{CommitDiff, FileDiff, convert_to_raw_git_format};
use anyhow::Result;
use git_ops::model::CommitInfo;
use insta::{assert_yaml_snapshot, with_settings};
use model_core::config::ModelConfig;
use model_core::prompt::MAX_BRANCH_NAME_LENGTH;
use model_core::quantized_qwen3::QuantizedQwen3BranchGenerator;
use model_core::qwen25::Qwen25BranchGenerator;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

// Test configuration
const MAX_NEW_TOKENS: usize = 1000; // Increased to allow for thinking tags and complete generation

/// Helper to get or download test model paths
async fn get_test_model_paths(model: &ModelConfig) -> Result<PathBuf> {
  // Build the model path using the home directory
  let home_dir = std::env::var("HOME")
    .or_else(|_| std::env::var("USERPROFILE")) // Windows fallback
    .map_err(|_| anyhow::anyhow!("Could not determine home directory"))?;

  let model_dir = match model {
    ModelConfig::Qwen25Coder15B => "qwen25-coder-15b",
    ModelConfig::Qwen25Coder3B => "qwen25-coder-3b",
    ModelConfig::Qwen3_17B => "qwen3-17b",
  };

  let model_path = PathBuf::from(home_dir).join("Library/Caches/branch-deck/models").join(model_dir);

  if !model_path.exists() {
    tracing::debug!(path = %model_path.display(), ?model, "Test model not found. Skipping generation tests");
    return Err(anyhow::anyhow!("Test model not found"));
  }
  Ok(model_path)
}

/// Enum for different generator types
enum BranchGenerator {
  Qwen25(Qwen25BranchGenerator),
  QuantizedQwen3(QuantizedQwen3BranchGenerator),
}

impl BranchGenerator {
  async fn generate_branch_name(&mut self, prompt: &str, max_tokens: usize, is_alternative: bool) -> Result<model_core::BranchNameResult> {
    match self {
      BranchGenerator::Qwen25(generator) => generator.generate_branch_name(prompt, max_tokens, is_alternative).await,
      BranchGenerator::QuantizedQwen3(generator) => generator.generate_branch_name(prompt, max_tokens, is_alternative).await,
    }
  }

  fn create_prompt(&self, git_output: &str) -> Result<String> {
    match self {
      BranchGenerator::Qwen25(generator) => generator.create_prompt(git_output),
      BranchGenerator::QuantizedQwen3(generator) => generator.create_prompt(git_output),
    }
  }
}

/// Create realistic test data for different scenarios
fn create_test_scenarios() -> Vec<(&'static str, Vec<CommitInfo>, Vec<CommitDiff>)> {
  vec![
    // Scenario 1: JWT Authentication Feature
    (
      "jwt_auth_feature",
      vec![
        CommitInfo {
          hash: "a1b2c3d4".to_string(),
          message: "feat(auth): implement JWT token validation middleware".to_string(),
        },
        CommitInfo {
          hash: "b2c3d4e5".to_string(),
          message: "test(auth): add comprehensive JWT validation tests".to_string(),
        },
        CommitInfo {
          hash: "c3d4e5f6".to_string(),
          message: "docs(auth): update API documentation for JWT endpoints".to_string(),
        },
      ],
      create_jwt_auth_diffs(),
    ),
    // Scenario 2: Redis Memory Leak Fix
    (
      "redis_memory_fix",
      vec![
        CommitInfo {
          hash: "e5f6g7h8".to_string(),
          message: "fix(cache): resolve memory leak in Redis connection pool".to_string(),
        },
        CommitInfo {
          hash: "f6g7h8i9".to_string(),
          message: "perf(cache): optimize connection pooling strategy".to_string(),
        },
      ],
      create_redis_fix_diffs(),
    ),
    // Scenario 3: API Validation Refactoring
    (
      "validation_refactor",
      vec![
        CommitInfo {
          hash: "i9j0k1l2".to_string(),
          message: "refactor(api): extract validation logic to separate module".to_string(),
        },
        CommitInfo {
          hash: "j0k1l2m3".to_string(),
          message: "refactor(api): consolidate error handling patterns".to_string(),
        },
        CommitInfo {
          hash: "k1l2m3n4".to_string(),
          message: "test(api): update tests for new validation structure".to_string(),
        },
      ],
      create_validation_refactor_diffs(),
    ),
    // Scenario 4: Large Feature Development
    (
      "large_feature",
      vec![
        CommitInfo {
          hash: "feat001".to_string(),
          message: "PROJ-1234 feat: initial user management module setup".to_string(),
        },
        CommitInfo {
          hash: "feat002".to_string(),
          message: "PROJ-1234 feat: add user CRUD operations".to_string(),
        },
        CommitInfo {
          hash: "feat003".to_string(),
          message: "PROJ-1234 feat: implement role-based access control".to_string(),
        },
        CommitInfo {
          hash: "feat004".to_string(),
          message: "PROJ-1234 test: comprehensive test suite for user management".to_string(),
        },
        CommitInfo {
          hash: "feat005".to_string(),
          message: "PROJ-1234 docs: API documentation and migration guide".to_string(),
        },
      ],
      create_large_feature_diffs(),
    ),
    // Scenario 5: Non-conventional commits
    (
      "non_conventional",
      vec![
        CommitInfo {
          hash: "nc001".to_string(),
          message: "Update dependencies to latest versions".to_string(),
        },
        CommitInfo {
          hash: "nc002".to_string(),
          message: "Fix broken tests in user module".to_string(),
        },
        CommitInfo {
          hash: "nc003".to_string(),
          message: "Add error handling for edge cases".to_string(),
        },
      ],
      create_non_conventional_diffs(),
    ),
    // Scenario 6: Mixed format commits
    (
      "mixed_format",
      vec![
        CommitInfo {
          hash: "mix001".to_string(),
          message: "feat: add dark mode support".to_string(),
        },
        CommitInfo {
          hash: "mix002".to_string(),
          message: "Fix CSS issues in mobile view".to_string(),
        },
        CommitInfo {
          hash: "mix003".to_string(),
          message: "Update README with new instructions".to_string(),
        },
        CommitInfo {
          hash: "mix004".to_string(),
          message: "refactor: simplify theme switching logic".to_string(),
        },
      ],
      create_mixed_format_diffs(),
    ),
    // Scenario 7: Casual developer commits
    (
      "casual_commits",
      vec![
        CommitInfo {
          hash: "cas001".to_string(),
          message: "WIP: working on search feature".to_string(),
        },
        CommitInfo {
          hash: "cas002".to_string(),
          message: "Fix typo in variable name".to_string(),
        },
        CommitInfo {
          hash: "cas003".to_string(),
          message: "Code cleanup and formatting".to_string(),
        },
      ],
      create_casual_commits_diffs(),
    ),
  ]
}

fn create_jwt_auth_diffs() -> Vec<CommitDiff> {
  vec![
    CommitDiff {
      commit_hash: "a1b2c3d4".to_string(),
      files: vec![
        FileDiff {
          path: "src/middleware/auth.rs".to_string(),
          additions: 87,
          deletions: 12,
          patch: Some(JWT_AUTH_MIDDLEWARE_PATCH.to_string()),
        },
        FileDiff {
          path: "src/middleware/mod.rs".to_string(),
          additions: 2,
          deletions: 0,
          patch: Some(JWT_AUTH_MOD_PATCH.to_string()),
        },
      ],
    },
    CommitDiff {
      commit_hash: "b2c3d4e5".to_string(),
      files: vec![FileDiff {
        path: "tests/auth_test.rs".to_string(),
        additions: 156,
        deletions: 0,
        patch: Some(JWT_AUTH_TESTS_PATCH.to_string()),
      }],
    },
    CommitDiff {
      commit_hash: "c3d4e5f6".to_string(),
      files: vec![FileDiff {
        path: "docs/api/auth.md".to_string(),
        additions: 45,
        deletions: 5,
        patch: None, // Documentation, less important for branch naming
      }],
    },
  ]
}

fn create_redis_fix_diffs() -> Vec<CommitDiff> {
  vec![CommitDiff {
    commit_hash: "e5f6g7h8".to_string(),
    files: vec![FileDiff {
      path: "src/cache/redis_pool.rs".to_string(),
      additions: 45,
      deletions: 28,
      patch: Some(REDIS_POOL_FIX_PATCH.to_string()),
    }],
  }]
}

fn create_validation_refactor_diffs() -> Vec<CommitDiff> {
  vec![CommitDiff {
    commit_hash: "i9j0k1l2".to_string(),
    files: vec![
      FileDiff {
        path: "src/api/handlers/user.rs".to_string(),
        additions: 15,
        deletions: 48,
        patch: Some(USER_HANDLER_REFACTOR_PATCH.to_string()),
      },
      FileDiff {
        path: "src/validators/user.rs".to_string(),
        additions: 52,
        deletions: 0,
        patch: Some(USER_VALIDATOR_PATCH.to_string()),
      },
      FileDiff {
        path: "src/validators/mod.rs".to_string(),
        additions: 3,
        deletions: 0,
        patch: Some(VALIDATORS_MOD_PATCH.to_string()),
      },
    ],
  }]
}

fn create_large_feature_diffs() -> Vec<CommitDiff> {
  // Create a large number of file changes
  let mut diffs = vec![];
  for i in 0..5 {
    diffs.push(CommitDiff {
      commit_hash: format!("feat00{}", i + 1),
      files: (0..10)
        .map(|j| FileDiff {
          path: format!("src/users/module_{i}/file_{j}.rs"),
          additions: 50 + j * 10,
          deletions: 10 + j * 2,
          patch: if j < 3 {
            Some(format!("// Large feature file {i}/{j} content"))
          } else {
            None // Simulate files-only mode for some files
          },
        })
        .collect(),
    });
  }
  diffs
}

fn create_non_conventional_diffs() -> Vec<CommitDiff> {
  vec![
    CommitDiff {
      commit_hash: "nc001".to_string(),
      files: vec![
        FileDiff {
          path: "package.json".to_string(),
          additions: 15,
          deletions: 15,
          patch: None,
        },
        FileDiff {
          path: "Cargo.lock".to_string(),
          additions: 200,
          deletions: 180,
          patch: None,
        },
      ],
    },
    CommitDiff {
      commit_hash: "nc002".to_string(),
      files: vec![FileDiff {
        path: "tests/user_test.rs".to_string(),
        additions: 25,
        deletions: 10,
        patch: None,
      }],
    },
    CommitDiff {
      commit_hash: "nc003".to_string(),
      files: vec![
        FileDiff {
          path: "src/handlers/error.rs".to_string(),
          additions: 35,
          deletions: 5,
          patch: None,
        },
        FileDiff {
          path: "src/utils/validation.rs".to_string(),
          additions: 20,
          deletions: 0,
          patch: None,
        },
      ],
    },
  ]
}

fn create_mixed_format_diffs() -> Vec<CommitDiff> {
  vec![
    CommitDiff {
      commit_hash: "mix001".to_string(),
      files: vec![
        FileDiff {
          path: "src/components/theme.tsx".to_string(),
          additions: 120,
          deletions: 30,
          patch: None,
        },
        FileDiff {
          path: "styles/dark.css".to_string(),
          additions: 200,
          deletions: 0,
          patch: None,
        },
      ],
    },
    CommitDiff {
      commit_hash: "mix002".to_string(),
      files: vec![FileDiff {
        path: "styles/mobile.css".to_string(),
        additions: 45,
        deletions: 32,
        patch: None,
      }],
    },
    CommitDiff {
      commit_hash: "mix003".to_string(),
      files: vec![FileDiff {
        path: "README.md".to_string(),
        additions: 30,
        deletions: 10,
        patch: None,
      }],
    },
    CommitDiff {
      commit_hash: "mix004".to_string(),
      files: vec![FileDiff {
        path: "src/hooks/useTheme.ts".to_string(),
        additions: 25,
        deletions: 40,
        patch: None,
      }],
    },
  ]
}

fn create_casual_commits_diffs() -> Vec<CommitDiff> {
  vec![
    CommitDiff {
      commit_hash: "cas001".to_string(),
      files: vec![
        FileDiff {
          path: "src/search/index.js".to_string(),
          additions: 80,
          deletions: 0,
          patch: None,
        },
        FileDiff {
          path: "src/search/filter.js".to_string(),
          additions: 45,
          deletions: 0,
          patch: None,
        },
      ],
    },
    CommitDiff {
      commit_hash: "cas002".to_string(),
      files: vec![FileDiff {
        path: "src/utils/helpers.rs".to_string(),
        additions: 2,
        deletions: 2,
        patch: None,
      }],
    },
    CommitDiff {
      commit_hash: "cas003".to_string(),
      files: vec![
        FileDiff {
          path: "src/lib.rs".to_string(),
          additions: 15,
          deletions: 25,
          patch: None,
        },
        FileDiff {
          path: "src/main.rs".to_string(),
          additions: 10,
          deletions: 15,
          patch: None,
        },
      ],
    },
  ]
}

#[tokio::test]
async fn test_model_generation_with_snapshots() {
  let scenarios = create_test_scenarios();
  let mut enabled_models = vec![ModelConfig::Qwen3_17B, ModelConfig::Qwen25Coder15B];
  enabled_models.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
  let modes = vec![(true, "files_only"), (false, "full_diff")];

  // Load all models upfront
  let mut generators: HashMap<ModelConfig, BranchGenerator> = HashMap::new();

  for model in &enabled_models {
    match get_test_model_paths(model).await {
      Ok(model_path) => {
        let generator = match model {
          ModelConfig::Qwen25Coder15B | ModelConfig::Qwen25Coder3B => {
            let mut qwen25_gen = Qwen25BranchGenerator::new();
            if let Err(e) = qwen25_gen.load_model(model_path).await {
              tracing::warn!(error = %e, "Failed to load Qwen2.5 model. Skipping");
              continue;
            }
            BranchGenerator::Qwen25(qwen25_gen)
          }
          ModelConfig::Qwen3_17B => {
            let mut qwen3_gen = QuantizedQwen3BranchGenerator::new();
            if let Err(e) = qwen3_gen.load_model(model_path).await {
              tracing::warn!(error = %e, "Failed to load Quantized Qwen3 model. Skipping");
              continue;
            }
            BranchGenerator::QuantizedQwen3(qwen3_gen)
          }
        };
        generators.insert(*model, generator);
      }
      Err(_) => {
        tracing::debug!(?model, "Model not found, skipping");
      }
    }
  }

  if generators.is_empty() {
    tracing::warn!(
      "No models loaded. Skipping tests. \
       To run these tests, ensure models are downloaded to: \
       ~/Library/Caches/branch-deck/models/qwen25-coder-05b or \
       ~/Library/Caches/branch-deck/models/qwen3-17b"
    );
    return;
  }

  for (scenario_name, commits, diffs) in scenarios {
    for (use_files_only, mode_name) in &modes {
      let test_name = format!("{scenario_name}_{mode_name}");
      println!("\n=== Testing {test_name} ===");

      // Filter diffs based on mode
      let filtered_diffs: Vec<CommitDiff> = if *use_files_only {
        diffs
          .iter()
          .map(|d| CommitDiff {
            commit_hash: d.commit_hash.clone(),
            files: d
              .files
              .iter()
              .map(|f| FileDiff {
                path: f.path.clone(),
                additions: f.additions,
                deletions: f.deletions,
                patch: None, // Remove patches in files-only mode
              })
              .collect(),
          })
          .collect()
      } else {
        diffs.clone()
      };

      // Create git output for prompt generation
      let git_output = convert_to_raw_git_format(&commits, &filtered_diffs);

      // Generate results for all available models
      #[derive(serde::Serialize)]
      struct ModelResult {
        model: String,
        prompt: String, // Store the actual prompt used by each model
        generated_branch_name: String,
        generation_time_ms: u64,
      }

      let mut model_results = Vec::new();

      // Sort models by name to ensure stable iteration order
      let mut sorted_generators: Vec<_> = generators.iter_mut().collect();
      sorted_generators.sort_by(|a, b| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)));

      for (model_config, generator) in sorted_generators {
        println!("  Testing with model: {model_config:?}");

        // Generate model-specific prompt using the generator
        let model_prompt = generator.create_prompt(&git_output).unwrap();

        // Generate branch name
        let start = Instant::now();
        match generator.generate_branch_name(&model_prompt, MAX_NEW_TOKENS, false).await {
          Ok(generation_result) => {
            let generation_time = start.elapsed().as_millis() as u64;

            println!("    Generated: '{}'", generation_result.name);
            println!("    Time: {generation_time}ms");

            // Verify generated name
            assert!(!generation_result.name.is_empty(), "Generated branch name should not be empty");
            assert!(
              generation_result.name.len() <= MAX_BRANCH_NAME_LENGTH,
              "Branch name too long: {} > {}",
              generation_result.name.len(),
              MAX_BRANCH_NAME_LENGTH
            );

            model_results.push(ModelResult {
              model: format!("{model_config:?}"),
              prompt: model_prompt.clone(),
              generated_branch_name: generation_result.name,
              generation_time_ms: generation_time.div_ceil(2000) * 2000, // Round up to nearest 2000ms
            });

            // Test prompt characteristics
            verify_prompt_characteristics(model_config, &model_prompt, *use_files_only, scenario_name);
          }
          Err(e) => {
            tracing::warn!(?model_config, error = %e, "Failed to generate branch name");
          }
        }
      }

      // Create unified snapshot with all model results
      #[derive(serde::Serialize)]
      struct UnifiedSnapshotData<'a> {
        scenario: &'a str,
        mode: &'a str,
        commits_count: usize,
        files_count: usize,
        model_results: Vec<ModelResult>,
      }

      // Model results are already in sorted order due to stable iteration above

      let snapshot_data = UnifiedSnapshotData {
        scenario: scenario_name,
        mode: mode_name,
        commits_count: commits.len(),
        files_count: filtered_diffs.iter().map(|d| d.files.len()).sum::<usize>(),
        model_results,
      };

      with_settings!({
          description => format!("Generated branch names for {scenario_name} in {mode_name} mode").as_str(),
          omit_expression => true
      }, {
          assert_yaml_snapshot!(test_name, snapshot_data);
      });
    }
  }
}

/// Helper function to verify prompt characteristics
fn verify_prompt_characteristics(model: &ModelConfig, prompt: &str, use_files_only: bool, scenario: &str) {
  println!("Verifying {} prompt for scenario '{}' (files_only: {})", model.model_name(), scenario, use_files_only);

  // Length constraints (common for all models)
  assert!(prompt.len() > 50, "Prompt should have meaningful content");
  assert!(prompt.len() < 10000, "Prompt should be reasonably sized for model context");

  // Model-specific prompt format assertions
  match model {
    ModelConfig::Qwen3_17B => {
      // ChatML format assertions (specifically for quantized Qwen3)
      assert!(prompt.contains("<|im_start|>system"), "ChatML format should have system section");
      assert!(prompt.contains("<|im_start|>user"), "ChatML format should have user section");
      assert!(prompt.contains("/no_think"), "ChatML format should have assistant section with think tags");
      assert!(prompt.contains("Maximum 50 characters"), "Should specify length constraint");
    }
    _ => {
      // Generic format assertions (for Qwen2.5 and other models)
      assert!(prompt.contains("Create one branch name"), "Generic prompt should ask for branch name creation");
      assert!(prompt.contains("Your turn:"), "Generic prompt should have user prompt section");
    }
  }

  // Model-specific validations
  if model == &ModelConfig::Qwen25Coder15B {
    // Qwen models are optimized for code understanding
    assert!(prompt.contains("max 50 characters"), "Should specify branch name length limit");
  }

  // The current implementation doesn't include diffs in the prompt
  assert!(!prompt.contains("@@"), "Current implementation doesn't include diff markers");
}

// Test data patches
const JWT_AUTH_MIDDLEWARE_PATCH: &str = r#"@@ -15,12 +15,87 @@
 pub struct AuthMiddleware {
     secret: String,
+    validation: Validation,
+    encoding_key: EncodingKey,
+    decoding_key: DecodingKey,
 }"#;

const JWT_AUTH_MOD_PATCH: &str = r#"@@ -3,3 +3,5 @@
 pub mod cors;
 pub mod logging;
+pub mod auth;
+
+pub use auth::AuthMiddleware;"#;

const JWT_AUTH_TESTS_PATCH: &str = r#"@@ -0,0 +156,156 @@
+#[cfg(test)]
+mod tests {
+    use super::*;
+    
+    #[test]
+    fn test_jwt_validation() {
+        // Test implementation
+    }
+}"#;

const REDIS_POOL_FIX_PATCH: &str = r#"@@ -23,28 +23,45 @@
-        let conn = self.pool.get().await?;
-        Ok(conn)
+        // Fix: Add timeout to prevent indefinite waiting
+        let conn = timeout(Duration::from_secs(5), self.pool.get())
+            .await
+            .map_err(|_| CacheError::ConnectionTimeout)?
+            .map_err(|e| CacheError::PoolError(e))?;"#;

const USER_HANDLER_REFACTOR_PATCH: &str = r#"@@ -10,48 +10,15 @@
-    // Validate email format
-    if !payload.email.contains('@') {
-        return Err(ApiError::ValidationError("Invalid email format".to_string()));
-    }
+    // Refactored: Use centralized validator
+    UserValidator::validate_create_request(&payload)?;"#;

const USER_VALIDATOR_PATCH: &str = r#"@@ -0,0 +52,52 @@
+pub struct UserValidator;
+
+impl UserValidator {
+    pub fn validate_create_request(req: &CreateUserRequest) -> Result<(), ValidationError> {
+        Self::validate_email(&req.email)?;
+        Self::validate_password(&req.password)?;
+        Self::validate_username(&req.username)?;
+        Ok(())
+    }
+}"#;

const VALIDATORS_MOD_PATCH: &str = r#"@@ -1,2 +1,5 @@
 pub mod common;
+pub mod user;
+
+pub use user::UserValidator;"#;
