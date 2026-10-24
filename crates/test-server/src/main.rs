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
    () = ctrl_c => {},
    () = terminate => {},
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
  // TEST_SERVER_LOG before RUST_LOG: .cargo/config.toml forces RUST_LOG=debug on every `cargo run`, and Playwright
  // starts the server with `cargo run`
  let filter = std::env::var("TEST_SERVER_LOG")
    .ok()
    .and_then(|directives| tracing_subscriber::EnvFilter::try_new(directives).ok())
    .or_else(|| tracing_subscriber::EnvFilter::try_from_default_env().ok())
    .unwrap_or_else(|| "test_server=debug,tower_http=debug".into());
  tracing_subscriber::registry().with(filter).with(tracing_subscriber::fmt::layer()).init();

  // Create and run the app (this returns both the app and the state)
  let (app, state) = test_server::create_test_app_with_state();

  // E2E_PORT lets a second worktree run its own server
  let port = std::env::var("E2E_PORT").ok().and_then(|port| port.parse::<u16>().ok()).unwrap_or(3030);
  let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();

  tracing::info!("Test server listening on http://{}", listener.local_addr().unwrap());

  // Run server with graceful shutdown
  axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(state)).await.unwrap();
}
