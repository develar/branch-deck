use super::*;
use pretty_assertions::assert_eq;

fn step(name: &str, ms: u64) -> Phase {
  Phase { name: name.to_owned(), ms }
}

#[test]
fn a_duration_reads_in_seconds_then_minutes_then_hours() {
  assert_eq!(duration_text(0), "0.0s");
  assert_eq!(duration_text(3149), "3.1s");
  assert_eq!(duration_text(41_000), "41.0s");
  assert_eq!(duration_text(59_949), "59.9s");
  assert_eq!(duration_text(59_950), "1m00s");
  assert_eq!(duration_text(542_000), "9m02s");
  assert_eq!(duration_text(3_599_499), "59m59s");
  assert_eq!(duration_text(3_599_500), "1h00m00s");
  assert_eq!(duration_text(3_909_000), "1h05m09s");
}

/// The steps of the client wrap the steps of the host in their order, and the short form leaves out the steps under
/// 0.1 s
#[test]
fn the_phases_are_the_steps_of_both_ends_in_their_order() {
  let phases = [
    step("pack", 3100),
    step("upload", 2000),
    step("slot", 542_000),
    step("pack_wait", 0),
    step("command", 25_100),
    step("fetch", 400),
    step("total", 3_909_000),
  ];
  assert_eq!(
    phases_line(&phases),
    "bd-host: phases: pack 3.1s, upload 2.0s, slot 9m02s, pack_wait 0.0s, command 25.1s, fetch 0.4s, total 1h05m09s"
  );
  assert_eq!(phases_short(&phases), "pack 3.1s, upload 2.0s, slot 9m02s, command 25.1s, fetch 0.4s, total 1h05m09s");
  assert_eq!(phases_short(&[step("pack_wait", 99)]), "");
  assert_eq!(phase("fetch", Duration::from_millis(1500)), step("fetch", 1500));
}
