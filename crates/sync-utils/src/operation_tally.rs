//! What one operation of the remote cost (docs/design/git-operations.md, "Journal"): its git processes by subcommand,
//! its HTTP requests and its time since its admission, wherever they ran. The tally of the operation is the current one
//! of a thread while the operation works on it: during each poll of its future (`OperationTally::counting`), inside a
//! blocking step of it (`scope`), and inside every closure it hands to another thread, which `span::in_current_span`
//! carries with the span. `GitCommandExecutor` counts each git process into the current tally (`git_process`), the
//! clients of Patronus, GitHub and YouTrack each request (`http_request`); without a current tally nothing is counted.
//! Work an operation leaves to another task (the actor of its session, the scheduler of the facts) counts into that
//! task's, none.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Instant;

thread_local! {
  /// The tally of the operation that works on this thread now
  static CURRENT: RefCell<Option<Arc<OperationTally>>> = const { RefCell::new(None) };
}

/// The cost of one operation so far
#[derive(Debug)]
pub struct OperationTally {
  began: Instant,
  git: Mutex<BTreeMap<String, u32>>,
  http: AtomicU32,
}

impl OperationTally {
  /// The tally of an operation admitted now
  #[must_use]
  pub fn begin() -> Arc<Self> {
    Arc::new(Self {
      began: Instant::now(),
      git: Mutex::new(BTreeMap::new()),
      http: AtomicU32::new(0),
    })
  }

  /// `work` as a future that counts into this tally while it is polled
  pub fn counting<F: Future>(self: &Arc<Self>, work: F) -> Counting<F> {
    Counting {
      tally: Arc::clone(self),
      work: Box::pin(work),
    }
  }

  /// Runs `work` on this thread counting into this tally
  pub fn scope<T>(self: &Arc<Self>, work: impl FnOnce() -> T) -> T {
    within(Some(self), work)
  }

  /// The git processes counted so far, by subcommand
  pub fn git_processes(&self) -> BTreeMap<String, u32> {
    self.git.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// The HTTP requests counted so far
  pub fn http_requests(&self) -> u32 {
    self.http.load(Ordering::Relaxed)
  }

  /// The milliseconds since the tally began
  pub fn millis(&self) -> u32 {
    u32::try_from(self.began.elapsed().as_millis()).unwrap_or(u32::MAX)
  }
}

/// The tally current on this thread, which a closure handed to another thread takes along (`span::in_current_span`)
pub fn current() -> Option<Arc<OperationTally>> {
  CURRENT.with(|current| current.borrow().clone())
}

/// Runs `work` with `tally` as the current one of this thread, the one before it current again after it, a panic
/// included
pub fn within<T>(tally: Option<&Arc<OperationTally>>, work: impl FnOnce() -> T) -> T {
  let _restore = Restore(CURRENT.with(|current| current.replace(tally.cloned())));
  work()
}

/// A git process of `subcommand` started, counted into the current tally
pub fn git_process(subcommand: &str) {
  CURRENT.with(|current| {
    if let Some(tally) = current.borrow().as_ref() {
      *tally.git.lock().unwrap_or_else(PoisonError::into_inner).entry(subcommand.to_string()).or_default() += 1;
    }
  });
}

/// An HTTP request is sent, counted into the current tally
pub fn http_request() {
  CURRENT.with(|current| {
    if let Some(tally) = current.borrow().as_ref() {
      tally.http.fetch_add(1, Ordering::Relaxed);
    }
  });
}

/// Puts the tally that was current back when dropped
struct Restore(Option<Arc<OperationTally>>);

impl Drop for Restore {
  fn drop(&mut self) {
    let previous = self.0.take();
    CURRENT.with(|current| *current.borrow_mut() = previous);
  }
}

/// A future that counts into its tally while it is polled (`OperationTally::counting`)
pub struct Counting<F> {
  tally: Arc<OperationTally>,
  work: Pin<Box<F>>,
}

impl<F: Future> Future for Counting<F> {
  type Output = F::Output;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    within(Some(&this.tally), || this.work.as_mut().poll(cx))
  }
}
