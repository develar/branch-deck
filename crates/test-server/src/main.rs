use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

async fn shutdown_signal(state: Arc<test_server::state::AppState>) {
  // Wait for ctrl-c signal
  let ctrl_c = async {
    tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
  };

  // Also handle SIGTERM on Unix
  #[cfg(unix)]
  let terminate = async {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
      .expect("failed to install signal handler")
      .recv()
      .await;
  };

  #[cfg(not(unix))]
  let terminate = std::future::pending::<()>();

  // Wait for either signal
  tokio::select! {
    _ = ctrl_c => {},
    _ = terminate => {},
  }

  tracing::info!("Shutdown signal received, cleaning up...");

  // The temp directory will be cleaned up when state is dropped
  // But we can log the path for debugging
  tracing::info!("Cleaning up test directory: {:?}", state.test_root_dir.path());

  // Drop the state explicitly to ensure cleanup happens
  drop(state);
}

#[tokio::main]
async fn main() {
  // Initialize tracing
  tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "test_server=debug,tower_http=debug".into()))
    .with(tracing_subscriber::fmt::layer())
    .init();

  // Check if we should just regenerate templates and exit
  let args: Vec<String> = std::env::args().collect();
  if args.len() > 1 && args[1] == "--regenerate-templates" {
    tracing::info!("Regenerating test repository templates...");

    // Remove existing templates
    let test_repos_dir = test_server::get_test_repos_dir();
    if test_repos_dir.exists() {
      if let Err(e) = std::fs::remove_dir_all(&test_repos_dir) {
        tracing::warn!("Failed to remove existing templates: {}", e);
      }
    }

    // Recreate templates
    if let Err(e) = test_server::ensure_test_repos().await {
      tracing::error!("Failed to create test repositories: {}", e);
      std::process::exit(1);
    }

    tracing::info!("Test repository templates regenerated successfully!");
    std::process::exit(0);
  }

  // Create and run the app (this returns both the app and the state)
  let (app, state) = test_server::create_test_app_with_state().await;

  // Try to get listener from systemfd first (for hot reload)
  let mut listenfd = listenfd::ListenFd::from_env();
  let listener = if let Some(listener) = listenfd.take_tcp_listener(0).unwrap() {
    // Convert std listener to tokio listener
    tokio::net::TcpListener::from_std(listener).unwrap()
  } else {
    // No listener from systemfd, bind normally
    tokio::net::TcpListener::bind("127.0.0.1:3030").await.unwrap()
  };

  tracing::info!("Test server listening on http://127.0.0.1:3030");

  // Run server with graceful shutdown
  axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(state)).await.unwrap();
}
