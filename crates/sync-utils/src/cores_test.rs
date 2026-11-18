use super::Cores;
use pretty_assertions::assert_eq;
use std::time::Duration;

/// Cores are tenths inside, a number with one decimal in JSON and in a line, and the mean of a CPU time over a wall time
#[test]
fn cores_read_and_write_with_one_decimal_and_are_the_mean_of_a_cpu_time() {
  assert_eq!(Cores::of(Duration::from_secs(150), Duration::from_secs(20)), Cores(75));
  assert_eq!(Cores::of(Duration::from_millis(1), Duration::ZERO), Cores(0));
  assert_eq!(Cores::whole(20), Cores(200));
  assert_eq!(
    (Cores(75).to_string(), Cores(120).to_string(), Cores(3).to_string()),
    ("7.5".to_owned(), "12.0".to_owned(), "0.3".to_owned())
  );
  assert_eq!(serde_json::to_string(&Cores(75)).unwrap(), "7.5");
  assert_eq!(serde_json::from_str::<Cores>("12").unwrap(), Cores(120));
  assert_eq!(serde_json::from_str::<Cores>("0.26").unwrap(), Cores(3));
  serde_json::from_str::<Cores>("-1").unwrap_err();
  assert_eq!([Cores(75), Cores(25)].into_iter().sum::<Cores>(), Cores(100));
}
