use crate::presentation::{Presentation, presented_within};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep};

const BUDGET: Duration = Duration::from_secs(1);

#[tokio::test(start_paused = true)]
async fn a_window_due_to_show_waits_for_the_frame_its_webview_presents() {
  let (presented, answer) = oneshot::channel();
  let start = Instant::now();
  let (presentation, ()) = tokio::join!(presented_within(BUDGET, answer), async {
    // the frame of the settled rows, committed to the app's process
    sleep(Duration::from_millis(40)).await;
    presented.send(true).unwrap();
  });
  assert_eq!(presentation, Presentation::Presented);
  assert_eq!(start.elapsed(), Duration::from_millis(40));
}

#[tokio::test(start_paused = true)]
async fn a_webview_that_presents_nothing_shows_its_window_once_the_budget_passed() {
  let (_presented, answer) = oneshot::channel::<bool>();
  let start = Instant::now();
  assert_eq!(presented_within(BUDGET, answer).await, Presentation::TimedOut);
  assert_eq!(start.elapsed(), BUDGET);
}

#[tokio::test(start_paused = true)]
async fn a_webview_that_tells_no_presentation_shows_its_window_at_once() {
  let start = Instant::now();
  let (presented, answer) = oneshot::channel();
  presented.send(false).unwrap();
  assert_eq!(presented_within(BUDGET, answer).await, Presentation::Unknown);
  // the webview went with its window before it answered
  let (presented, answer) = oneshot::channel::<bool>();
  drop(presented);
  assert_eq!(presented_within(BUDGET, answer).await, Presentation::Unknown);
  assert_eq!(start.elapsed(), Duration::ZERO);
}
