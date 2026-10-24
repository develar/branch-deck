use crate::commands::session::WindowSubscriptions;
use repository_model::session::SubscriptionId;

/// A subscription is kept for the page that asked for it while that page is shown; one that answers after the page went
/// away (a reload or a close while the command awaited) is not kept, so the caller ends it, and a page of the same label
/// shown later is another page
#[test]
fn a_subscription_that_answers_after_its_page_went_away_is_not_kept() {
  let mut windows = WindowSubscriptions::default();
  let page = windows.page("main");
  assert_eq!(windows.page("main"), page, "the same page while it is shown");
  assert!(windows.keep("main", page, SubscriptionId(1)));

  // the page loads again while a second subscription of it is on its way
  let asked = windows.page("main");
  assert_eq!(windows.forget("main"), [SubscriptionId(1)]);
  assert!(!windows.keep("main", asked, SubscriptionId(2)), "the old page's subscription is not kept");
  let reloaded = windows.page("main");
  assert_ne!(reloaded, asked);
  assert!(!windows.keep("main", asked, SubscriptionId(3)), "nor once the new page asked for its own");
  assert!(windows.keep("main", reloaded, SubscriptionId(4)));

  // a closed window: its page is gone, a later one under its label is another
  let review = windows.page("review-auth");
  assert_eq!(windows.forget("review-auth"), []);
  assert_ne!(windows.page("review-auth"), review);
  assert!(!windows.keep("review-auth", review, SubscriptionId(5)));

  windows.drop_subscription("main", SubscriptionId(4));
  assert_eq!(windows.forget("main"), []);
}
