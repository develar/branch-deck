use crate::commands::check_model_files_exist;
use model_core::ModelConfig;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_model_status_detection_for_gguf_model() {
  // Create a temporary directory for the test
  let temp_dir = TempDir::new().unwrap();
  let model_path = temp_dir.path();

  // Test with Qwen3_17B which uses GGUF format
  let model_config = ModelConfig::Qwen3_17B;
  let download_urls = model_config.download_urls();

  // Initially, no files exist
  let (_config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(!model_exists);
  assert!(!tokenizer_exists);

  // Create the expected files based on download URLs
  for (filename, _, _) in &download_urls {
    fs::write(model_path.join(filename), "dummy content").unwrap();
  }

  // Now check again - all files should exist
  let (config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(config_exists); // GGUF doesn't need config
  assert!(model_exists);
  assert!(tokenizer_exists);
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
  let (_config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(!model_exists, "Should not detect generic model.gguf as the model file");
  assert!(tokenizer_exists);

  // Now create the correct file
  fs::write(model_path.join("Qwen3-1.7B-Q8_0.gguf"), "correct model").unwrap();

  // Should now detect as available
  let (_config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(model_exists, "Should detect Qwen3-1.7B-Q8_0.gguf as the model file");
  assert!(tokenizer_exists);
}

#[test]
fn test_model_status_detection_for_safetensors_model() {
  // Create a temporary directory for the test
  let temp_dir = TempDir::new().unwrap();
  let model_path = temp_dir.path();

  // Test with Qwen25Coder15B which uses SafeTensors format
  let model_config = ModelConfig::Qwen25Coder15B;

  // Initially, no files exist
  let (config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(!config_exists);
  assert!(!model_exists);
  assert!(!tokenizer_exists);

  // Create only some files
  fs::write(model_path.join("config.json"), "{}").unwrap();
  fs::write(model_path.join("tokenizer.json"), "{}").unwrap();

  // Model file is still missing
  let (config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(config_exists);
  assert!(!model_exists);
  assert!(tokenizer_exists);

  // Create the model file
  fs::write(model_path.join("model.safetensors"), "dummy").unwrap();

  // Now all files should exist
  let (config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, model_path);
  assert!(config_exists);
  assert!(model_exists);
  assert!(tokenizer_exists);
}
