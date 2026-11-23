//! The commit of the build id (`identity::build_id`, docs/design/backend-protocol.md, "The wire"): `BRANCH_DECK_COMMIT`
//! as the build that runs cargo sets it (`scripts/install-app.sh`: the commit and `-dirty` for a tree with changes),
//! else `GITHUB_SHA` of the job of GitHub Actions that builds it (CI and the release, `build-tauri.yml`), else `dev`
//! (a dev build, whose executable path gives it a backend of its own anyway). No git runs here: the commit is the
//! builder's word.

fn main() {
  println!("cargo:rerun-if-env-changed=BRANCH_DECK_COMMIT");
  println!("cargo:rerun-if-env-changed=GITHUB_SHA");
  let commit = ["BRANCH_DECK_COMMIT", "GITHUB_SHA"]
    .iter()
    .filter_map(|name| std::env::var(name).ok())
    .map(|value| value.trim().to_owned())
    .find(|value| !value.is_empty())
    .unwrap_or_else(|| "dev".to_owned());
  println!("cargo:rustc-env=BACKEND_BUILD_COMMIT={commit}");
}
