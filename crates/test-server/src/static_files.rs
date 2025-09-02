//! Static file serving functionality for the test server.
//!
//! This module handles serving prebuilt Nuxt output from `.output/public` directory
//! with Single Page Application (SPA) routing support.

use tower_http::services::ServeDir;

/// Creates a ServeDir service that serves static files from the `.output/public` directory
/// with SPA fallback support.
///
/// This function:
/// 1. Locates the project root by looking for `Cargo.toml` and `.output` directory
/// 2. Serves static files from `.output/public`
/// 3. Provides SPA routing fallback by serving `index.html` for directory requests
///
/// # Returns
///
/// A configured `ServeDir` service ready to be used as a fallback service in Axum router.
///
/// # Panics
///
/// Panics if:
/// - Cannot determine the current executable path
/// - Cannot find the project root with required directories
pub fn serve_static_files() -> ServeDir {
  // Find the project root by looking for Cargo.toml
  let current_exe = std::env::current_exe().expect("Failed to get current executable path");
  let project_root = current_exe
    .ancestors()
    .find(|p| p.join("Cargo.toml").exists() && p.join(".output").exists())
    .expect("Failed to find project root with .output directory");

  let static_dir = project_root.join(".output").join("public");

  tracing::info!("Serving static files from: {:?}", static_dir);

  // Create ServeDir that will serve static files and fall back to index.html for SPA routing
  ServeDir::new(static_dir).append_index_html_on_directories(true)
}
