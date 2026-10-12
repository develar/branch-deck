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

  let model_config = ModelConfig::default();

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
fn test_model_status_requires_model_specific_weights_filename() {
  // This test ensures we detect the model-specific GGUF weights file (e.g., Qwen3-4B-Instruct-2507-Q4_K_M.gguf)
  // instead of looking for a generic "model.gguf"
  let temp_dir = TempDir::new().unwrap();
  let model_path = temp_dir.path();

  let model_config = ModelConfig::default();

  // Create a generic model.gguf file (wrong file)
  fs::write(model_path.join("model.gguf"), "wrong file").unwrap();
  fs::write(model_path.join(model_config.tokenizer_file().name), "{}").unwrap();

  // Should NOT detect as available because it's looking for the wrong filename
  let (available, status) = check_model_files_exist(model_config, model_path);
  assert!(!available);
  assert!(!status.model, "Should not detect generic model.gguf as the model file");
  assert!(status.tokenizer);

  // Now create the correct file
  fs::write(model_path.join(model_config.weights_file().name), "correct model").unwrap();

  // Should now detect as available
  let (available, status) = check_model_files_exist(model_config, model_path);
  assert!(available);
  assert!(status.model, "Should detect {} as the model file", model_config.weights_file().name);
  assert!(status.tokenizer);
}
