use crate::session::windows::{WindowRequests, WindowSubscriptions, Windows};
use crate::transport::WindowId;
use domain_types::operation::OperationId;
use repository_model::session::SubscriptionId;

/// A subscription is kept for the page that asked for it while that page is shown; one that answers after the page went
/// away (a reload or a close while the command awaited) is not kept, so the caller ends it, and a page of the same label
/// shown later is another page
#[test]
fn a_subscription_that_answers_after_its_page_went_away_is_not_kept() {
  let (main, review) = (WindowId::new("main"), WindowId::new("review-auth"));
  let mut windows = WindowSubscriptions::default();
  let page = windows.page(&main);
  assert_eq!(windows.page(&main), page, "the same page while it is shown");
  assert!(windows.keep(&main, page, SubscriptionId(1)));

  // the page loads again while a second subscription of it is on its way
  let asked = windows.page(&main);
  assert_eq!(windows.forget(&main), [SubscriptionId(1)]);
  assert!(!windows.keep(&main, asked, SubscriptionId(2)), "the old page's subscription is not kept");
  let reloaded = windows.page(&main);
  assert_ne!(reloaded, asked);
  assert!(!windows.keep(&main, asked, SubscriptionId(3)), "nor once the new page asked for its own");
  assert!(windows.keep(&main, reloaded, SubscriptionId(4)));

  // a closed window: its page is gone, a later one under its label is another
  let closed = windows.page(&review);
  assert_eq!(windows.forget(&review), []);
  assert_ne!(windows.page(&review), closed);
  assert!(!windows.keep(&review, closed, SubscriptionId(5)));

  windows.drop_subscription(&main, SubscriptionId(4));
  assert_eq!(windows.forget(&main), []);
}

/// The call of a command takes the page its window shows when it comes: a subscription it answers after a reload is
/// not kept. A review fetch of a window is known until it ends or the window closes, and a window with either is open
#[test]
fn a_call_names_the_page_of_its_window_and_a_closed_window_answers_its_fetches() {
  let windows = Windows::default();
  let call = windows.call("main");
  assert_eq!(call.window.as_str(), "main");
  assert!(windows.keep(&call, SubscriptionId(1)));
  assert_eq!(windows.forget_page(&call.window), [SubscriptionId(1)]);
  assert!(!windows.keep(&call, SubscriptionId(2)), "the call came before the reload");

  let review = WindowId::new("review-7");
  let (first, second) = (OperationId("op-1".to_owned()), OperationId("op-2".to_owned()));
  windows.fetch_started(&review, &first);
  windows.fetch_started(&review, &first);
  windows.fetch_started(&review, &second);
  windows.fetch_ended(&review, &first);
  assert_eq!(windows.open(), std::slice::from_ref(&review));
  assert_eq!(windows.forget_fetches(&review), [second]);
  assert_eq!(windows.forget_fetches(&review), []);
  assert_eq!(windows.open(), [] as [WindowId; 0]);
}

#[test]
fn a_window_supersedes_only_its_own_request() {
  let (main, review) = (WindowId::new("main"), WindowId::new("pull-request-review"));
  let requests = WindowRequests::default();
  let first = requests.begin(&main);
  let window = requests.begin(&review);

  // a newer request of the window, then a cancel of main: each supersedes only its own window's request
  let newer = requests.begin(&review);
  assert!(window.is_superseded());
  assert!(!first.is_superseded());
  requests.cancel(&main);
  assert!(first.is_superseded());
  assert!(!newer.is_superseded());
}

#[test]
fn a_closed_window_supersedes_its_request() {
  let review = WindowId::new("pull-request-review");
  let windows = Windows::default();
  let search = windows.begin_search(&review);
  windows.forget_reads(&review);
  assert!(search.is_superseded());
  // a window opened again under the label starts afresh
  let again = windows.begin_search(&review);
  assert!(!again.is_superseded());
  windows.cancel_search(&review);
  assert!(again.is_superseded());
}

#[test]
fn a_review_plan_and_a_symbol_search_of_a_window_never_supersede_each_other() {
  let review = WindowId::new("branch-review");
  let windows = Windows::default();
  let plan = windows.begin_plan(&review);
  // Go to Declaration while the plan reads, and its cancel: the plan goes on
  let search = windows.begin_search(&review);
  windows.cancel_search(&review);
  assert!(search.is_superseded());
  assert!(!plan.is_superseded());
  // a newer plan of the window supersedes the one before, not the search after it
  let search = windows.begin_search(&review);
  let newer = windows.begin_plan(&review);
  assert!(plan.is_superseded());
  assert!(!search.is_superseded());
  // the close (`AppContext::forget_window`) supersedes the plan and the search
  windows.forget_reads(&review);
  assert!(newer.is_superseded());
  assert!(search.is_superseded());
}
