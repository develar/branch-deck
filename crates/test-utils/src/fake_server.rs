//! A fake of an HTTP service on a loopback port of its own: the one way the fakes of the tests (`patronus::fake_patronus`,
//! `github::fake_github`, `youtrack::fake_youtrack`) and the E2E test server (`remote_host.rs`) serve a router.

use tracing::Instrument;

/// A router served on a loopback port of its own; the server stops when this is dropped, unless detached
pub struct FakeServer {
  /// `http://127.0.0.1:{port}`, the base URL of a client
  pub url: String,
  task: Option<tokio::task::JoinHandle<()>>,
}

impl FakeServer {
  /// Serves `router` on a new loopback port; call inside a Tokio runtime
  pub fn serve(router: axum::Router) -> Self {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port for the fake");
    listener.set_nonblocking(true).expect("a non-blocking listener");
    Self::serve_on(tokio::net::TcpListener::from_std(listener).expect("a Tokio listener"), router)
  }

  /// `serve`, bound without blocking the runtime
  pub async fn start(router: axum::Router) -> Self {
    Self::serve_on(tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("a loopback port for the fake"), router)
  }

  /// The server runs in the span of the caller, so its log lines belong to the test
  fn serve_on(listener: tokio::net::TcpListener, router: axum::Router) -> Self {
    let url = format!("http://{}", listener.local_addr().expect("the address of the fake"));
    let serving = async move { axum::serve(listener, router).await.expect("the fake server") };
    let task = tokio::spawn(serving.in_current_span());
    Self { url, task: Some(task) }
  }

  /// Keeps the server running until the runtime ends; its URL
  pub fn detach(mut self) -> String {
    self.task.take();
    std::mem::take(&mut self.url)
  }
}

impl Drop for FakeServer {
  fn drop(&mut self) {
    if let Some(task) = self.task.take() {
      task.abort();
    }
  }
}
