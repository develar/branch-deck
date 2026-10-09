//! Static file serving functionality for the test server.
//!
//! This module serves the prebuilt Vite output from the `dist` directory
//! with Single Page Application (SPA) routing support.

use tower_http::services::{ServeDir, ServeFile};

/// Creates a `ServeDir` service that serves static files from the `dist` directory
/// with SPA fallback support.
///
/// This function:
/// 1. Locates the project root by looking for `Cargo.toml` and `dist/index.html`
/// 2. Serves static files from `dist`
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
/// - Cannot find the project root with a built `dist` directory (in production mode); run `pnpm build`
///
/// # Test Mode Behavior
///
/// In test mode (`#[cfg(test)]`), if the `dist` directory cannot be found, this function
/// will create a minimal fallback `ServeDir` service using a temporary directory instead of panicking.
/// This allows unit tests to run without requiring the frontend build output.
pub fn serve_static_files() -> ServeDir<ServeFile> {
  // Find the project root by looking for Cargo.toml
  let current_exe = std::env::current_exe().expect("Failed to get current executable path");
  let project_root_opt = current_exe
    .ancestors()
    .find(|p| p.join("Cargo.toml").exists() && p.join("dist").join("index.html").exists());

  let static_dir = match project_root_opt {
    Some(project_root) => {
      let static_dir = project_root.join("dist");
      tracing::info!("Serving static files from: {:?}", static_dir);
      static_dir
    }
    None => {
      #[cfg(test)]
      {
        // In test mode, create a temporary directory to avoid panicking
        let temp_dir = std::env::temp_dir().join("branch-deck-test-static");
        std::fs::create_dir_all(&temp_dir).ok(); // Ignore errors
        tracing::warn!("Could not find dist directory during tests, using fallback: {:?}", temp_dir);
        temp_dir
      }
      #[cfg(not(test))]
      {
        panic!("Failed to find project root with dist/index.html, run `pnpm build` first");
      }
    }
  };

  // Serve static files and fall back to index.html for SPA routing
  let index_html = static_dir.join("index.html");
  ServeDir::new(static_dir).fallback(ServeFile::new(index_html))
}
