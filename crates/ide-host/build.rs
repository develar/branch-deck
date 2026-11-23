//! The files of the Air build of the page (`vite build --mode air`: `dist-air`, a link to the newest build, or the
//! directory `BRANCH_DECK_AIR_DIST` names) built into `brd mcp`: `resources.rs` of `OUT_DIR` lists each with its bytes,
//! from copies in `OUT_DIR` (`build/air_build.rs`). Without a build the list is empty, and `brd mcp` serves a stub
//! page; a new build of the page touches `air-build.stamp`, which builds the list again.

use std::fmt::Write;
use std::path::PathBuf;
use std::time::Duration;

#[path = "build/air_build.rs"]
mod air_build;

/// How long the copy waits for a build that a publish of the page moves in (two steps on Windows)
const PATIENCE: Duration = Duration::from_secs(2);

fn main() {
  println!("cargo::rerun-if-env-changed=BRANCH_DECK_AIR_DIST");
  let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
  let dist = std::env::var_os("BRANCH_DECK_AIR_DIST").map_or_else(|| manifest.join("../../dist-air"), PathBuf::from);
  let builds = manifest.join("../../target/dist-air.builds");
  for path in air_build::rerun_paths(&dist, &manifest.join("air-build.stamp")) {
    println!("cargo::rerun-if-changed={}", path.display());
  }
  let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
  let files = air_build::copy_build(&dist, &builds, &out.join("air"), PATIENCE).unwrap_or_else(|error| panic!("{error}"));
  let mut source = String::from("/// The files of the Air build of the page, by their path in it\npub static FILES: &[(&str, &[u8])] = &[\n");
  for (path, file) in &files {
    let _ = writeln!(source, "  ({path:?}, include_bytes!({:?})),", file.display().to_string());
  }
  source.push_str("];\n");
  std::fs::write(out.join("resources.rs"), source).expect("OUT_DIR is writable");
}
