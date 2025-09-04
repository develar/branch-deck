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
/// - Cannot find the project root with required directories (in production mode)
///
/// # Test Mode Behavior
///
/// In test mode (`#[cfg(test)]`), if the `.output` directory cannot be found, this function
/// will create a minimal fallback ServeDir service using a temporary directory instead of panicking.
/// This allows unit tests to run without requiring the full Nuxt build output.
pub fn serve_static_files() -> ServeDir {
  // Find the project root by looking for Cargo.toml
  let current_exe = std::env::current_exe().expect("Failed to get current executable path");
  let project_root_opt = current_exe.ancestors().find(|p| p.join("Cargo.toml").exists() && p.join(".output").exists());

  let static_dir = match project_root_opt {
    Some(project_root) => {
      let static_dir = project_root.join(".output").join("public");
      tracing::info!("Serving static files from: {:?}", static_dir);
      static_dir
    }
    None => {
      #[cfg(test)]
      {
        // In test mode, create a temporary directory to avoid panicking
        let temp_dir = std::env::temp_dir().join("branch-deck-test-static");
        std::fs::create_dir_all(&temp_dir).ok(); // Ignore errors
        tracing::warn!("Could not find .output directory during tests, using fallback: {:?}", temp_dir);
        temp_dir
      }
      #[cfg(not(test))]
      {
        panic!("Failed to find project root with .output directory");
      }
    }
  };

  // Create ServeDir that will serve static files and fall back to index.html for SPA routing
  ServeDir::new(static_dir).append_index_html_on_directories(true)
}
