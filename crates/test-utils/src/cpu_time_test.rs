use super::*;
use std::time::Instant;

/// A sleep of 200 ms uses next to none of the CPU time of its thread; a loop on the thread uses it up, and never more
/// than its wall time
#[test]
fn the_cpu_time_of_a_thread_grows_with_its_work_and_not_with_its_sleep() {
  let before = thread_cpu_time();
  std::thread::sleep(Duration::from_millis(200));
  let slept = thread_cpu_time().saturating_sub(before);
  assert!(slept < Duration::from_millis(100), "a sleep used {slept:?} of CPU time");

  let before = thread_cpu_time();
  let started = Instant::now();
  let mut used = Duration::ZERO;
  while used < Duration::from_millis(200) && started.elapsed() < Duration::from_mins(1) {
    std::hint::black_box(used);
    used = thread_cpu_time().saturating_sub(before);
  }
  assert!(used >= Duration::from_millis(200), "a loop of a minute used {used:?} of CPU time");
  // the clock of Windows counts whole ticks of 15.6 ms, so its count can pass the wall time by one
  let tick = if cfg!(windows) { Duration::from_millis(16) } else { Duration::ZERO };
  assert!(used <= started.elapsed() + tick, "{used:?} of CPU time in {:?}", started.elapsed());
}

/// The work count of a thread grows with a loop on it and stays about put through a sleep
#[test]
fn the_work_of_a_thread_grows_with_a_loop_and_not_with_a_sleep() {
  let before = thread_work();
  std::thread::sleep(Duration::from_millis(100));
  let slept = thread_work() - before;
  let before = thread_work();
  let started = Instant::now();
  while started.elapsed() < Duration::from_millis(100) {
    std::hint::black_box(started);
  }
  let worked = thread_work() - before;
  assert!(worked > 10 * slept.max(1), "a loop of 100 ms counted {worked}, a sleep of 100 ms {slept}");
}
