use crate::concurrency::ProcessPermits;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Threads of two parts of an operation that take their slots from one `ProcessPermits` never run more tasks at a time
/// than it has slots
#[test]
fn two_parts_share_the_slots() {
  let permits = ProcessPermits::new(3);
  let (running, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
  std::thread::scope(|scope| {
    for _ in 0..12 {
      scope.spawn(|| {
        let _permit = permits.acquire();
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        most.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(5));
        running.fetch_sub(1, Ordering::SeqCst);
      });
    }
  });
  let most = most.load(Ordering::SeqCst);
  assert!((1..=3).contains(&most), "{most} tasks ran at a time");
  assert_eq!(permits.capacity(), 3);
  // all slots are free again: three are held at once without waiting
  let held = [permits.acquire(), permits.acquire(), permits.acquire()];
  drop(held);
}
