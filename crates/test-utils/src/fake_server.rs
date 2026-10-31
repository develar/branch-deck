//! A fake of an HTTP service on a loopback port of its own: the one way the fakes of the tests (`patronus::fake_patronus`,
//! `github::fake_github`, `youtrack::fake_youtrack`) and the E2E test server (`remote_host.rs`) serve a router; and the
//! gate a fake holds a request at (`FakeGate`).

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

/// `route` (`METHOD /path`) is the one `pattern` names: segment by segment, a `*` segment matching any one; the route
/// of a hold or a failure of a fake (`FakePatronus::hold`, `FakeGitHub::hold`)
#[must_use]
pub fn route_matches(pattern: &str, route: &str) -> bool {
  let mut segments = route.split('/');
  pattern
    .split('/')
    .all(|expected| segments.next().is_some_and(|segment| expected == "*" || expected == segment))
    && segments.next().is_none()
}

/// A gate that holds the requests a fake routes to it until `open`: a barrier for a test of what runs while a request is
/// in flight (`FakeGitHub::hold`, `FakePatronus::hold`, the held answers of the E2E test server)
pub struct FakeGate {
  /// No permit is ever added: a held request waits until the semaphore is closed
  semaphore: tokio::sync::Semaphore,
  arrivals: std::sync::atomic::AtomicUsize,
  arrived: tokio::sync::Notify,
}

impl Default for FakeGate {
  fn default() -> Self {
    Self {
      semaphore: tokio::sync::Semaphore::new(0),
      arrivals: std::sync::atomic::AtomicUsize::new(0),
      arrived: tokio::sync::Notify::new(),
    }
  }
}

impl FakeGate {
  /// Lets every held request and every later one through
  pub fn open(&self) {
    self.semaphore.close();
  }

  /// A request reached the gate (held now, or let through since)
  pub fn has_arrived(&self) -> bool {
    self.arrivals.load(std::sync::atomic::Ordering::SeqCst) > 0
  }

  /// Resolves once a request reached the gate
  pub async fn arrived(&self) {
    self.arrived_times(1).await;
  }

  /// Resolves once `count` requests reached the gate
  pub async fn arrived_times(&self, count: usize) {
    loop {
      let notified = self.arrived.notified();
      if self.arrivals.load(std::sync::atomic::Ordering::SeqCst) >= count {
        return;
      }
      notified.await;
    }
  }

  /// Waits here until the gate opens: the fake calls it before it answers a held request
  pub async fn pass(&self) {
    self.arrivals.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    self.arrived.notify_waiters();
    // an error once the gate is open: the only way through
    let _ = self.semaphore.acquire().await;
  }
}
