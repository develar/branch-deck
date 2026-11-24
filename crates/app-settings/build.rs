//! Compiles the identifier of the app into the crate (`identity::APP_IDENTIFIER`), from the configuration tauri-build
//! reads: the binary of a build with another identifier (a check copy) finds its own data directory, in the app, in
//! `branch-deck ide` and in `brd` alike

#[path = "src/identity_source.rs"]
mod identity_source;

fn main() {
  let config = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tauri.conf.json");
  println!("cargo:rerun-if-changed={}", config.display());
  println!("cargo:rerun-if-env-changed=TAURI_CONFIG");
  let base = std::fs::read_to_string(&config).unwrap_or_else(|e| panic!("cannot read {}: {e}", config.display()));
  let overlay = std::env::var("TAURI_CONFIG").ok();
  let identifier = identity_source::identifier_of(&base, overlay.as_deref()).unwrap_or_else(|| panic!("{} names no identifier", config.display()));
  println!("cargo:rustc-env=BRANCH_DECK_APP_IDENTIFIER={identifier}");
}
