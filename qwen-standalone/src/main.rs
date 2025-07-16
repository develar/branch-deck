use anyhow::Result;
use clap::Parser;
use hf_hub::{api::sync::Api, Repo, RepoType};
use model_core::{GeneratorType, Qwen25BranchGenerator, Qwen3BranchGenerator, QuantizedQwen3BranchGenerator};
use model_core::test_utils::{convert_to_raw_git_format, CommitInfo, CommitDiff, FileDiff};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

// Compile-time switch to choose between models
const USE_QWEN3: bool = true;
const USE_QWEN3_17B: bool = true; // Use quantized 1.7B model instead of 0.6B

// GeneratorType is now imported from model_core

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// The temperature used to generate samples
  #[arg(long, default_value_t = 0.7)]
  temperature: f64,

  /// The length of the sample to generate (in tokens)
  #[arg(short = 'n', long, default_value_t = 30)]
  sample_len: usize,

  /// The seed to use when generating random samples
  #[arg(long, default_value_t = 299792458)]
  seed: u64,

  /// Custom prompt (if not provided, will test branch name generation)
  #[arg(long)]
  prompt: Option<String>,

  /// Run performance benchmark with multiple iterations
  #[arg(long)]
  benchmark: bool,
}

fn main() -> Result<()> {
  // Initialize tracing
  tracing_subscriber::fmt::init();

  let args = Args::parse();

  let model_name = if USE_QWEN3 {
    if USE_QWEN3_17B { "Qwen3-1.7B-GGUF" } else { "Qwen3-0.6B" }
  } else { 
    "Qwen2.5-Coder-0.5B" 
  };
  println!("🚀 {} Branch Name Generator", model_name);
  println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

  // Create async runtime
  let rt = Runtime::new()?;
  
  rt.block_on(async {
    run_async(args).await
  })
}

async fn run_async(args: Args) -> Result<()> {
  // Test prompts - production-style format using proper git format
  let test_data = if args.prompt.is_some() {
    vec![]
  } else {
    // Create test data using proper structures
    vec![
      // Test: Complex refactoring scenario
      (
        vec![
          CommitInfo {
            hash: "abc123".to_string(),
            message: "refactor(api): extract validation logic to separate module".to_string(),
          },
          CommitInfo {
            hash: "def456".to_string(),
            message: "refactor(api): consolidate error handling patterns".to_string(),
          },
          CommitInfo {
            hash: "ghi789".to_string(),
            message: "test(api): update tests for new validation structure".to_string(),
          },
        ],
        vec![
          CommitDiff {
            commit_hash: "abc123".to_string(),
            files: vec![
              FileDiff {
                path: "src/api/handlers/user.rs".to_string(),
                additions: 10,
                deletions: 50,
                patch: None,
              },
              FileDiff {
                path: "src/validators/user.rs".to_string(),
                additions: 45,
                deletions: 0,
                patch: None,
              },
              FileDiff {
                path: "src/validators/mod.rs".to_string(),
                additions: 5,
                deletions: 0,
                patch: None,
              },
            ],
          },
        ],
      ),
    ]
  };

  // Convert test data to git format or use custom prompt
  let test_prompts: Vec<String> = if let Some(ref custom_prompt) = args.prompt {
    vec![custom_prompt.clone()]
  } else {
    test_data.iter()
      .map(|(commits, diffs)| convert_to_raw_git_format(commits, diffs))
      .collect()
  };

  if args.benchmark {
    run_benchmark(&test_prompts, &args).await?;
  } else {
    // run_tests(&test_prompts, &args).await?;
    let x = r#"refactor(api): extract validation logic to separate module
M	src/api/handlers/user.rs
A	src/validators/user.rs
A	src/validators/mod.rs

refactor(api): consolidate error handling patterns

test(api): update tests for new validation structure"#;
    run_tests(&[x.to_string()], &args).await?;
  }

  Ok(())
}

async fn run_tests(test_prompts: &[String], args: &Args) -> Result<()> {
  let mut total_time = Duration::ZERO;
  let mut success_count = 0;

  for (i, prompt) in test_prompts.iter().enumerate() {
    println!("\n🔬 Test {} of {}", i + 1, test_prompts.len());
    println!("─────────────────────────────────────────────");
    println!("{}", prompt);

    // Show only the actual task, not the examples
    if let Some(pos) = prompt.rfind("Generate a git branch name") {
      let task_preview = &prompt[pos..].lines().take(5).collect::<Vec<_>>().join("\n");
      println!("Task preview:\n{}", task_preview);
    }

    // Load model for each test to avoid state issues
    let model_path = download_or_load_model_path().await?;
    let mut generator = if USE_QWEN3 {
        if USE_QWEN3_17B {
            let generator = QuantizedQwen3BranchGenerator::new();
            let mut generator = GeneratorType::QuantizedQwen3(generator);
            generator.load_model(model_path).await?;
            generator
        } else {
            let generator = Qwen3BranchGenerator::new();
            let mut generator = GeneratorType::Qwen3(generator);
            generator.load_model(model_path).await?;
            generator
        }
    } else {
        let generator = Qwen25BranchGenerator::new();
        let mut generator = GeneratorType::Qwen25(generator);
        generator.load_model(model_path).await?;
        generator
    };

    match generate_with_core(&mut generator, prompt, args).await {
      Ok((branch_name, duration, token_count)) => {
        total_time += duration;
        
        println!("\n📌 Result:");
        println!("   Branch name: '{}'", branch_name);
        println!("   Length: {} chars", branch_name.len());
        println!("   Time: {:.2}s", duration.as_secs_f64());
        println!("   Tokens: ~{}", token_count);
        println!("   Speed: ~{:.1} tokens/sec", token_count as f64 / duration.as_secs_f64());

        // Validation
        let is_valid = validate_branch_name(&branch_name);
        if is_valid && !branch_name.is_empty() && duration.as_secs() < 10 {
          println!("\n✅ SUCCESS: Valid branch name generated efficiently!");
          success_count += 1;
        } else if branch_name.is_empty() {
          println!("\n❌ FAILED: Empty result");
        } else if !is_valid {
          println!("\n❌ FAILED: Invalid branch name format");
        } else {
          println!("\n⚠️  WARNING: Slow generation (>10s)");
        }
      }
      Err(e) => {
        println!("\n❌ ERROR: {}", e);
        println!("   Debug: {:?}", e);
      }
    }
  }

  // Summary
  println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  println!("📊 Summary:");
  println!("   Tests passed: {}/{}", success_count, test_prompts.len());
  println!("   Total time: {:.2}s", total_time.as_secs_f64());
  println!("   Avg time/test: {:.2}s", total_time.as_secs_f64() / test_prompts.len() as f64);

  Ok(())
}

async fn run_benchmark(test_prompts: &[String], args: &Args) -> Result<()> {
  println!("\n🏃 Running performance benchmark...");
  println!("─────────────────────────────────────────────");
  
  let iterations = 5;
  let mut all_times = vec![];

  for (i, prompt) in test_prompts.iter().enumerate() {
    println!("\nBenchmarking prompt {}: {prompt}", i + 1);
    let mut times = vec![];

    for iter in 0..iterations {
      print!("  Iteration {}/{}... ", iter + 1, iterations);
      use std::io::Write;
      std::io::stdout().flush()?;

      let model_path = download_or_load_model_path().await?;
      let mut generator = if USE_QWEN3 {
          if USE_QWEN3_17B {
              let generator = QuantizedQwen3BranchGenerator::new();
              let mut generator = GeneratorType::QuantizedQwen3(generator);
              generator.load_model(model_path).await?;
              generator
          } else {
              let generator = Qwen3BranchGenerator::new();
              let mut generator = GeneratorType::Qwen3(generator);
              generator.load_model(model_path).await?;
              generator
          }
      } else {
          let generator = Qwen25BranchGenerator::new();
          let mut generator = GeneratorType::Qwen25(generator);
          generator.load_model(model_path).await?;
          generator
      };
      
      let start = Instant::now();
      
      match generate_with_core(&mut generator, prompt, args).await {
        Ok((name, _dur, tokens)) => {
          let elapsed = start.elapsed();
          times.push(elapsed);
          println!("{:.2}s ({} tokens, result: '{}')", elapsed.as_secs_f64(), tokens, name);
        }
        Err(e) => {
          println!("ERROR: {}", e);
        }
      }
    }

    if !times.is_empty() {
      let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
      let min_time = times.iter().min().unwrap();
      let max_time = times.iter().max().unwrap();
      
      println!("  Average: {:.2}s, Min: {:.2}s, Max: {:.2}s", 
        avg_time.as_secs_f64(), min_time.as_secs_f64(), max_time.as_secs_f64());
      
      all_times.extend(times);
    }
  }

  if !all_times.is_empty() {
    let overall_avg = all_times.iter().sum::<Duration>() / all_times.len() as u32;
    println!("\n📊 Overall benchmark results:");
    println!("   Total runs: {}", all_times.len());
    println!("   Average time: {:.2}s", overall_avg.as_secs_f64());
  }

  Ok(())
}

/// Download model from HuggingFace or return local path
async fn download_or_load_model_path() -> Result<PathBuf> {
  // Try to use local files first (from our main project)
  let home_dir = std::env::var("HOME").expect("HOME not set");
  let model_dir = if USE_QWEN3 {
    if USE_QWEN3_17B { "qwen3-17b" } else { "qwen3-06b" }
  } else { 
    "qwen25-coder-05b" 
  };
  let local_model_path = PathBuf::from(&home_dir)
      .join("Library/Caches/branch-deck/models")
      .join(model_dir);

  if local_model_path.exists() {
    let tokenizer_path = local_model_path.join("tokenizer.json");
    
    // Check for different model file types
    let model_file_exists = if USE_QWEN3 && USE_QWEN3_17B {
      local_model_path.join("Qwen3-1.7B-Q8_0.gguf").exists()
    } else {
      let config_path = local_model_path.join("config.json");
      let model_path = local_model_path.join("model.safetensors");
      config_path.exists() && model_path.exists()
    };

    if tokenizer_path.exists() && model_file_exists {
      return Ok(local_model_path);
    }
  }

  // Fall back to downloading from HuggingFace
  let (model_id, gguf_file) = if USE_QWEN3 {
    if USE_QWEN3_17B {
("Qwen/Qwen3-1.7B-GGUF", Some("Qwen3-1.7B-Q8_0.gguf"))
    } else {
      ("Qwen/Qwen3-0.6B", None)
    }
  } else {
    ("Qwen/Qwen2.5-Coder-0.5B", None)
  };
  
  println!("📥 Downloading {} from HuggingFace...", model_id);
  let api = Api::new()?;
  let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

  // Create cache directory if it doesn't exist
  std::fs::create_dir_all(&local_model_path)?;

  // Download tokenizer (always needed)
  let tokenizer_file = if USE_QWEN3 && USE_QWEN3_17B {
    // For GGUF model, get tokenizer from main Qwen3-1.7B repo
    let tokenizer_repo = api.repo(Repo::new("Qwen/Qwen3-1.7B".to_string(), RepoType::Model));
    tokenizer_repo.get("tokenizer.json")?
  } else {
    repo.get("tokenizer.json")?
  };
  std::fs::copy(&tokenizer_file, local_model_path.join("tokenizer.json"))?;

  if let Some(gguf_filename) = gguf_file {
    // Download GGUF model
    let model_file = repo.get(gguf_filename)?;
    std::fs::copy(&model_file, local_model_path.join(gguf_filename))?;
  } else {
    // Download SafeTensors model and config
    let config_file = repo.get("config.json")?;
    let model_file = repo.get("model.safetensors")?;
    std::fs::copy(&config_file, local_model_path.join("config.json"))?;
    std::fs::copy(&model_file, local_model_path.join("model.safetensors"))?;
    
    // Download optional files if they exist
    if let Ok(tokenizer_config_file) = repo.get("tokenizer_config.json") {
      std::fs::copy(&tokenizer_config_file, local_model_path.join("tokenizer_config.json"))?;
    }
    if let Ok(merges_file) = repo.get("merges.txt") {
      std::fs::copy(&merges_file, local_model_path.join("merges.txt"))?;
    }
  }

  println!("✅ Model cached locally for future runs");
  Ok(local_model_path)
}

/// Generate text using model-core
async fn generate_with_core(
  generator: &mut GeneratorType,
  git_output: &str,
  args: &Args
) -> Result<(String, Duration, usize)> {
  let start = Instant::now();
  
  // Use the generator's create_prompt method to format the prompt correctly
  let prompt = generator.create_prompt(git_output)?;
  
  let result = generator.generate_branch_name(
    &prompt,
    args.sample_len,
    args.temperature
  ).await?;
  
  let duration = start.elapsed();
  
  // Estimate token count (model-core doesn't expose this directly)
  let token_count = if let Some(count) = generator.count_tokens(&result.name) {
    count
  } else {
    result.name.len() / 4 // Rough estimate
  };
  
  Ok((result.name, duration, token_count))
}

/// Validate that a branch name follows Git conventions
fn validate_branch_name(name: &str) -> bool {
  if name.is_empty() || name.len() > 50 {
    return false;
  }

  // Check for valid characters
  name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.')
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_validate_branch_name() {
    assert!(validate_branch_name("fix-bug"));
    assert!(validate_branch_name("feature/oauth2"));
    assert!(validate_branch_name("jira-123-fix"));
    assert!(!validate_branch_name(""));
    assert!(!validate_branch_name("invalid branch name"));
    assert!(!validate_branch_name("too-long-branch-name-that-exceeds-the-fifty-character-limit"));
  }
}