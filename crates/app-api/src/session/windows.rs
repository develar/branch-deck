//! What each window of the app holds open, by its label: the subscriptions of the page it shows, the review fetches it
//! runs and its symbol search. A subscription ends with `unsubscribe_view`, with its window (`AppContext::forget_window`, on
//! `Destroyed`) and with a page load of its window (`AppContext::forget_page`, a reload): a Tauri channel of a page
//! that went away never fails its send, so the session would keep it. A subscription is recorded under the page that
//! asked for it, so one that answers after its page went away (the command awaited the prefix and the session
//! meanwhile) ends at once. A review fetch (`Intent::FetchReview`) stops with its window: the fetch of a pull request no
//! window shows is of no use; every other operation of a closed window goes on. A symbol search (Go to Declaration, Find
//! Usages) is superseded by a newer one of its window, its cancel and its close (`WindowRequests`): windows search apart,
//! one never cancels the search of another. The read of a review plan (`get_review_plan`) is a request of its own kind,
//! superseded by a newer plan of its window and by its close: a symbol search and a plan never cancel each other.

use crate::transport::{Call, PageId, WindowId};
use domain_types::operation::OperationId;
use repository_model::session::SubscriptionId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The windows of an app: their subscriptions, their review fetches, their symbol searches and their review plans
#[derive(Default)]
pub struct Windows {
  subscriptions: Mutex<WindowSubscriptions>,
  fetches: WindowFetches,
  searches: WindowRequests,
  plans: WindowRequests,
}

/// The review fetches each window runs (`Intent::FetchReview`), by its label
#[derive(Default)]
pub struct WindowFetches(Mutex<HashMap<WindowId, Vec<OperationId>>>);

impl Windows {
  fn subscriptions(&self) -> MutexGuard<'_, WindowSubscriptions> {
    self.subscriptions.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn fetches(&self) -> MutexGuard<'_, HashMap<WindowId, Vec<OperationId>>> {
    self.fetches.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The call of a command of the window `label`, with the page it shows now
  pub fn call(&self, label: &str) -> Call {
    let window = WindowId::new(label);
    let page = self.subscriptions().page(&window);
    Call { window, page }
  }

  /// Keeps `subscription` for the page of `call`; false when that page went away meanwhile, so the caller ends it
  pub fn keep(&self, call: &Call, subscription: SubscriptionId) -> bool {
    self.subscriptions().keep(&call.window, call.page, subscription)
  }

  /// Drops `subscription` of the window of `call` (its page ended it)
  pub fn drop_subscription(&self, call: &Call, subscription: SubscriptionId) {
    self.subscriptions().drop_subscription(&call.window, subscription);
  }

  /// Forgets the page `window` shows (it closed, or loads again): its subscriptions, which the caller ends
  pub fn forget_page(&self, window: &WindowId) -> Vec<SubscriptionId> {
    self.subscriptions().forget(window)
  }

  /// The review fetch `op` of `window` runs: its close stops it
  pub fn fetch_started(&self, window: &WindowId, op: &OperationId) {
    let mut fetches = self.fetches();
    let ops = fetches.entry(window.clone()).or_default();
    if !ops.contains(op) {
      ops.push(op.clone());
    }
  }

  /// The review fetch `op` of `window` ended
  pub fn fetch_ended(&self, window: &WindowId, op: &OperationId) {
    if let Some(ops) = self.fetches().get_mut(window) {
      ops.retain(|kept| kept != op);
    }
  }

  /// The review fetches of `window` until now: it closed, the caller stops them
  pub fn forget_fetches(&self, window: &WindowId) -> Vec<OperationId> {
    self.fetches().remove(window).unwrap_or_default()
  }

  /// Starts a symbol search of `window`, which supersedes the one before
  pub fn begin_search(&self, window: &WindowId) -> WindowRequest {
    self.searches.begin(window)
  }

  /// Supersedes the symbol search of `window` in flight, if any
  pub fn cancel_search(&self, window: &WindowId) {
    self.searches.cancel(window);
  }

  /// Starts a read of the review plan of `window`, which supersedes the one before; a symbol search of the window
  /// goes on
  pub fn begin_plan(&self, window: &WindowId) -> WindowRequest {
    self.plans.begin(window)
  }

  /// `window` closed: its symbol search and its read of a review plan are superseded and forgotten
  pub fn forget_reads(&self, window: &WindowId) {
    self.searches.forget(window);
    self.plans.forget(window);
  }

  /// Every window with a page or a fetch
  pub fn open(&self) -> Vec<WindowId> {
    let mut windows: Vec<WindowId> = self.subscriptions().pages.keys().cloned().collect();
    for window in self.fetches().keys() {
      if !windows.contains(window) {
        windows.push(window.clone());
      }
    }
    windows
  }
}

/// The subscriptions of the page each window shows
#[derive(Default)]
pub struct WindowSubscriptions {
  last_page: u64,
  pages: HashMap<WindowId, (PageId, Vec<SubscriptionId>)>,
}

impl WindowSubscriptions {
  /// The page `window` shows now, as a command of it starts
  pub fn page(&mut self, window: &WindowId) -> PageId {
    if let Some((page, _)) = self.pages.get(window) {
      return *page;
    }
    self.last_page += 1;
    let page = PageId(self.last_page);
    self.pages.insert(window.clone(), (page, Vec::new()));
    page
  }

  /// Keeps `subscription` for the page `page` of `window`; false when that page went away meanwhile, so the caller
  /// ends the subscription
  pub fn keep(&mut self, window: &WindowId, page: PageId, subscription: SubscriptionId) -> bool {
    match self.pages.get_mut(window) {
      Some((shown, subscriptions)) if *shown == page => {
        subscriptions.push(subscription);
        true
      }
      _ => false,
    }
  }

  /// Drops `subscription` of `window` (its page ended it)
  pub fn drop_subscription(&mut self, window: &WindowId, subscription: SubscriptionId) {
    if let Some((_, subscriptions)) = self.pages.get_mut(window) {
      subscriptions.retain(|kept| *kept != subscription);
    }
  }

  /// Forgets the page of `window` (it closed, or loads again) and answers its subscriptions, which the caller ends
  pub fn forget(&mut self, window: &WindowId) -> Vec<SubscriptionId> {
    self.pages.remove(window).map_or_default(|(_, subscriptions)| subscriptions)
  }
}

/// The latest request of one kind of each window: an older one of the same window still running is superseded, as is
/// the latest after a cancel of the window or when the window goes. Windows ask apart: one never cancels the request of
/// another
#[derive(Default)]
pub struct WindowRequests(Mutex<HashMap<WindowId, Arc<AtomicU64>>>);

/// A request of a window: superseded once the window starts a newer one, cancels it, or goes
pub struct WindowRequest {
  counter: Arc<AtomicU64>,
  request: u64,
}

impl WindowRequest {
  pub fn is_superseded(&self) -> bool {
    self.counter.load(Ordering::SeqCst) != self.request
  }
}

impl WindowRequests {
  fn counter(&self, window: &WindowId) -> Arc<AtomicU64> {
    let mut latest = self.0.lock().unwrap_or_else(PoisonError::into_inner);
    Arc::clone(latest.entry(window.clone()).or_default())
  }

  /// Starts a request of the window, which supersedes the one before
  pub fn begin(&self, window: &WindowId) -> WindowRequest {
    let counter = self.counter(window);
    let request = counter.fetch_add(1, Ordering::SeqCst) + 1;
    WindowRequest { counter, request }
  }

  /// Supersedes the request of the window in flight, if any
  pub fn cancel(&self, window: &WindowId) {
    self.counter(window).fetch_add(1, Ordering::SeqCst);
  }

  /// The window went: its request is superseded and forgotten
  pub fn forget(&self, window: &WindowId) {
    let removed = self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(window);
    if let Some(counter) = removed {
      counter.fetch_add(1, Ordering::SeqCst);
    }
  }
}
