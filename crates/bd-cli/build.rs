//! `BD_VERSION`: the version of the app, which release-please keeps in `src-tauri/tauri.conf.json`. The workspace
//! version of the crates stays 0.1.0, so `CARGO_PKG_VERSION` is not the product version

const TAURI_CONFIG: &str = "../../src-tauri/tauri.conf.json";

fn main() {
  println!("cargo::rerun-if-changed={TAURI_CONFIG}");
  let text = std::fs::read_to_string(TAURI_CONFIG).unwrap_or_else(|e| panic!("cannot read {TAURI_CONFIG}: {e}"));
  let config: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {TAURI_CONFIG}: {e}"));
  let version = config["version"].as_str().unwrap_or_else(|| panic!("no version in {TAURI_CONFIG}"));
  println!("cargo::rustc-env=BD_VERSION={version}");
}
