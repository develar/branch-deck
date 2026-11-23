use crate::app::life::{BridgeConnections, WINDOWLESS_GRACE, WindowsClosed, exit_deadline, is_background_start, restart_args, shows_main, when_windows_close};
use std::ffi::OsString;
use std::time::{Duration, Instant};

#[test]
fn the_windows_close_and_the_process_stays_only_while_a_bridge_is_connected() {
  for (connections, expected) in [(0, WindowsClosed::Exit), (1, WindowsClosed::StayWindowless), (3, WindowsClosed::StayWindowless)] {
    assert_eq!(when_windows_close(connections), expected, "{connections} connections");
  }
}

#[test]
fn without_a_window_and_a_connection_the_process_exits_after_the_grace_of_the_later_of_its_windowless_start_and_its_last_connection() {
  let start = Instant::now();
  let later = start + Duration::from_secs(100);
  let earlier = start.checked_sub(Duration::from_secs(100));
  // (windowless since, connections now, the end of the last connection, the deadline)
  let cases = [
    // a window is open: no deadline, with a connection or without
    (None, 0, None, None),
    (None, 2, Some(start), None),
    // a start `--background` that no bridge joined yet
    (Some(start), 0, None, Some(start + WINDOWLESS_GRACE)),
    // a bridge is connected
    (Some(start), 1, None, None),
    (Some(start), 1, Some(later), None),
    // the last connection ended after the windowless start
    (Some(start), 0, Some(later), Some(later + WINDOWLESS_GRACE)),
    // the last connection ended before the last window closed (a connection of a window open then)
    (Some(start), 0, earlier, Some(start + WINDOWLESS_GRACE)),
  ];
  for (windowless_since, connections, last_disconnect, expected) in cases {
    assert_eq!(
      exit_deadline(windowless_since, connections, last_disconnect),
      expected,
      "{windowless_since:?} {connections} {last_disconnect:?}"
    );
  }
}

#[test]
fn only_the_background_argument_starts_without_a_window_and_a_second_start_with_it_shows_no_main_window() {
  for (args, background) in [
    (&[][..], false),
    (&["--background"][..], true),
    (&["--flag", "--background"][..], true),
    (&["background"][..], false),
    (&["--background=1"][..], false),
  ] {
    assert_eq!(is_background_start(args), background, "{args:?}");
    assert_eq!(shows_main(args), !background, "{args:?}");
  }
}

#[test]
fn a_restart_starts_with_a_window_when_one_is_open_and_without_one_when_none_is() {
  let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
  // (the arguments of the process, a window open, the arguments of the restart)
  let cases = [
    (args(&["app", "--background"]), true, args(&["app"])),
    (args(&["app", "--background"]), false, args(&["app", "--background"])),
    (args(&["app"]), true, args(&["app"])),
    (args(&["app"]), false, args(&["app", "--background"])),
    (args(&["app", "--x", "--background"]), true, args(&["app", "--x"])),
    // the program itself is never taken for the argument
    (args(&["--background"]), true, args(&["--background"])),
  ];
  for (process, window_open, expected) in cases {
    assert_eq!(restart_args(&process, window_open), expected, "{process:?} {window_open}");
  }
}

#[tokio::test]
async fn the_count_of_the_connections_is_what_the_host_set_last_and_none_never_changes() {
  let (sender, mut connections) = BridgeConnections::channel();
  assert_eq!(connections.now(), 0);
  sender.send(2).unwrap();
  assert_eq!(connections.changed().await, Some(2));
  assert_eq!(connections.now(), 2);
  drop(sender);
  assert_eq!(connections.changed().await, None);

  let mut none = BridgeConnections::none();
  assert_eq!((none.now(), none.changed().await), (0, None));
}
