use crate::commands::check_model_files_exist;
use model_core::config::ModelConfig;
use std::fs;
use tempfile::TempDir;
use test_log::test;

#[test]
fn test_model_status_detection_for_gguf_model() {
  // Create a temporary directory for the test
  let temp_dir = TempDir::new().unwrap();
  let model_path = temp_dir.path();

  let model_config = ModelConfig::Qwen3_17B;

  // Initially, no files exist
  let (available, status) = check_model_files_exist(model_config, model_path);
  assert!(!available);
  assert!(!status.model);
  assert!(!status.tokenizer);

  // Create every file the model needs
  for file in model_config.files() {
    fs::write(model_path.join(file.name), "dummy content").unwrap();
  }

  // Now check again - all files should exist
  let (available, status) = check_model_files_exist(model_config, model_path);
  assert!(available);
  assert!(status.model);
  assert!(status.tokenizer);
}

#[test]
fn test_model_status_qwen3_17b_specific_filename() {
  // This test ensures we correctly detect the Qwen3-1.7B-Q8_0.gguf file
  // instead of looking for a generic "model.gguf"
  let temp_dir = TempDir::new().unwrap();
  let model_path = temp_dir.path();

  let model_config = ModelConfig::Qwen3_17B;

  // Create a generic model.gguf file (wrong file)
  fs::write(model_path.join("model.gguf"), "wrong file").unwrap();
  fs::write(model_path.join("tokenizer.json"), "{}").unwrap();

  // Should NOT detect as available because it's looking for the wrong filename
  let (available, status) = check_model_files_exist(model_config, model_path);
  assert!(!available);
  assert!(!status.model, "Should not detect generic model.gguf as the model file");
  assert!(status.tokenizer);

  // Now create the correct file
  fs::write(model_path.join("Qwen3-1.7B-Q8_0.gguf"), "correct model").unwrap();

  // Should now detect as available
  let (available, status) = check_model_files_exist(model_config, model_path);
  assert!(available);
  assert!(status.model, "Should detect Qwen3-1.7B-Q8_0.gguf as the model file");
  assert!(status.tokenizer);
}
