//! Static file serving functionality for the test server.
//!
//! This module serves the prebuilt Vite output from the `dist` directory
//! with Single Page Application (SPA) routing support.

use tower_http::services::{ServeDir, ServeFile};

/// Creates a `ServeDir` service that serves static files from the `dist` directory
/// with SPA fallback support.
///
/// This function:
/// 1. Locates the workspace root: the first ancestor of the executable whose `Cargo.toml` has a `[workspace]` table.
///    The search stops there, so a git worktree under `.claude/worktrees/` never serves the main checkout's `dist`.
/// 2. Serves static files from `<root>/dist`, read per request: Playwright starts this server before its global setup
///    has built the frontend, so a missing `dist` is only logged
/// 3. Serves `dist/index.html` for any path that is not a file, so client-side routes
///    such as `/conflicting-files` and `/missing-commits` resolve to the SPA
///
/// # Returns
///
/// A configured `ServeDir` service ready to be used as a fallback service in Axum router.
///
/// # Panics
///
/// Panics if:
/// - Cannot determine the current executable path
/// - Cannot find the workspace root (in production mode)
///
/// # Test Mode Behavior
///
/// In test mode (`#[cfg(test)]`), if the workspace root cannot be found, this function
/// will create a minimal fallback `ServeDir` service using a temporary directory instead of panicking.
/// This allows unit tests to run without requiring the frontend build output.
pub fn serve_static_files() -> ServeDir<ServeFile> {
  let static_dir = if let Some(workspace_root) = workspace_root() {
    let static_dir = workspace_root.join("dist");
    if static_dir.join("index.html").exists() {
      tracing::info!("Serving static files from: {:?}", static_dir);
    } else {
      tracing::warn!(
        "{:?} has no index.html yet, run `pnpm build` in {:?}; requests fail until it exists",
        static_dir,
        workspace_root
      );
    }
    static_dir
  } else if cfg!(test) {
    // the unit tests of the server serve no app: an empty temporary directory
    let temp_dir = std::env::temp_dir().join("branch-deck-test-static");
    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
      tracing::warn!(error = %e, "Cannot create the fallback static directory {}", temp_dir.display());
    }
    tracing::warn!("Could not find the workspace root during tests, using fallback: {:?}", temp_dir);
    temp_dir
  } else {
    panic!("Failed to find the workspace root (a Cargo.toml with [workspace]) above the test-server executable");
  };

  // Serve static files and fall back to index.html for SPA routing
  let index_html = static_dir.join("index.html");
  ServeDir::new(static_dir).fallback(ServeFile::new(index_html))
}

/// The checkout of this server: the first ancestor of the executable whose `Cargo.toml` has a `[workspace]` table. The
/// E2E global setup compares it with its own checkout (`/health`), so a run never tests another worktree's server.
pub fn workspace_root() -> Option<std::path::PathBuf> {
  let current_exe = std::env::current_exe().expect("Failed to get current executable path");
  current_exe.ancestors().find(|p| is_workspace_root(p)).map(std::path::Path::to_path_buf)
}

/// Whether `dir` has a `Cargo.toml` with a `[workspace]` table
fn is_workspace_root(dir: &std::path::Path) -> bool {
  std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|manifest| manifest.lines().any(|line| line.trim() == "[workspace]"))
}
