use super::*;
use host_link::proto::{RunCores, RunRefusal};
use pretty_assertions::assert_eq;
use sync_utils::cores::Cores;

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
    "brd-host: phases: pack 3.1s, upload 2.0s, slot 9m02s, pack_wait 0.0s, command 25.1s, fetch 0.4s, total 1h05m09s"
  );
  assert_eq!(phases_short(&phases), "pack 3.1s, upload 2.0s, slot 9m02s, command 25.1s, fetch 0.4s, total 1h05m09s");
  assert_eq!(phases_short(&[step("pack_wait", 99)]), "");
  assert_eq!(phase("fetch", Duration::from_millis(1500)), step("fetch", 1500));
}

fn run_info(exit: Option<i32>, ended_ms: Option<u64>, forgotten: bool) -> RunInfo {
  RunInfo {
    run: "1794817711000-42".to_owned(),
    affinity: "checkout-0123456789ab".to_owned(),
    commit: "86551e413c53".repeat(3),
    command: vec!["cargo".to_owned(), "test".to_owned()],
    // 2026-11-16 08:28:31 UTC
    started_ms: 1_794_817_711_000,
    ended_ms,
    exit,
    phases: vec![step("slot", 50), step("command", 801_000)],
    fetch: Vec::new(),
    forgotten,
    refused: None,
    cores: None,
  }
}

/// A run of `brd-host runs` says whether it runs or finished, its start in the zone of the client, and for a finished
/// run its exit, how long it took, and whether its files are still on the host
#[test]
fn a_run_line_says_its_state_its_start_and_how_long_a_finished_run_took() {
  let utc = TimeZone::UTC;
  assert_eq!(
    run_line(&run_info(None, None, false), &utc),
    "1794817711000-42  running, started 2026-11-16 08:28:31  checkout-0123456789ab  86551e413c53  cargo test  (command 13m21s)"
  );
  assert_eq!(
    run_line(&run_info(Some(0), Some(1_794_817_711_000 + 801_400), true), &utc),
    "1794817711000-42  finished, exit 0, started 2026-11-16 08:28:31, took 13m21s  checkout-0123456789ab  86551e413c53  cargo test  (command 13m21s)"
  );
  assert_eq!(
    run_line(&run_info(Some(3), Some(1_794_817_711_000 + 4_000), false), &utc),
    "1794817711000-42  finished, exit 3, started 2026-11-16 08:28:31, took 4.0s, its files kept  checkout-0123456789ab  86551e413c53  cargo test  (command 13m21s)"
  );
  // an older agent sends no end
  assert!(run_line(&run_info(Some(0), None, true), &utc).contains("finished, exit 0, started 2026-11-16 08:28:31  checkout-"));
}

/// A run says the cores of its command: so far while it runs, its mean and its peak once it ended
#[test]
fn a_run_line_says_the_mean_and_the_peak_cores_of_its_command() {
  let utc = TimeZone::UTC;
  let cores = Some(RunCores {
    mean: Cores(75),
    peak: Cores(140),
  });
  let finished = RunInfo {
    cores,
    ..run_info(Some(0), Some(1_794_817_711_000 + 801_400), true)
  };
  assert_eq!(
    run_line(&finished, &utc),
    "1794817711000-42  finished, exit 0, started 2026-11-16 08:28:31, took 13m21s, cores 7.5 mean, 14.0 peak  checkout-0123456789ab  86551e413c53  cargo test  (command 13m21s)"
  );
  let running = RunInfo {
    cores,
    ..run_info(None, None, false)
  };
  assert!(run_line(&running, &utc).contains("running, started 2026-11-16 08:28:31, cores 7.5 mean, 14.0 peak so far  checkout-"));
}

/// A run that a refusal ended says the code instead of its exit, and the next line its message with the alias of the
/// host: a superseded run names the run that replaced it and its follow
#[test]
fn a_refused_run_says_its_code_and_its_message_with_the_alias() {
  let superseded = RunInfo {
    exit: Some(255),
    refused: Some(RunRefusal {
      code: "superseded".to_owned(),
      message: "run 1794817711000-42 was superseded by run 1794817712000-43 of the same checkout and command before it started: nothing ran; follow 1794817712000-43: `brd-host follow <host> 1794817712000-43`".to_owned(),
    }),
    phases: vec![step("heavy", 801_000)],
    ..run_info(Some(255), Some(1_794_817_711_000 + 801_400), true)
  };
  assert_eq!(
    run_lines_in(&[superseded], "studio", &TimeZone::UTC),
    [
      "1794817711000-42  finished, refused superseded, started 2026-11-16 08:28:31, took 13m21s  checkout-0123456789ab  86551e413c53  cargo test  (heavy 13m21s)",
      "    superseded: run 1794817711000-42 was superseded by run 1794817712000-43 of the same checkout and command before it started: nothing ran; follow 1794817712000-43: `brd-host follow studio 1794817712000-43`",
    ]
  );
}
