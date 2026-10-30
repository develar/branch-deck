//! The link of the agent: the deadline of a handshake and the cap of handshakes of one peer, the heartbeats of a follow
//! whose thread is held, and the push of a new event to a follow

use super::{World, events, eventually, wait_for};
use host_link::channel::{Channel, ClientTls};
use host_link::proto::{Reply, Request, Stream};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::fs_retry;

#[test]
fn a_slow_handshake_is_cut_at_the_deadline_and_a_socket_over_the_cap_closes_at_once() {
  let deadline = Duration::from_millis(1500);
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_HANDSHAKE_MS", "1500")]);
  // the end of the connection, and the time since `since`
  let read_end = |stream: &mut TcpStream, since: Instant| {
    stream.set_read_timeout(Some(Duration::from_secs(10))).expect("a deadline");
    let mut byte = [0u8; 1];
    let read = stream.read(&mut byte);
    assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    since.elapsed()
  };

  // 8 handshakes that send nothing fill the cap of one peer, and its next socket is closed at once. The agent accepts
  // in the order of the connects, so the 8 hold the cap when the next one comes, and its log names the closed socket
  let idle: Vec<TcpStream> = (0..8).map(|_| TcpStream::connect(agent.address).expect("connected")).collect();
  let mut over = TcpStream::connect(agent.address).expect("connected");
  read_end(&mut over, Instant::now());
  assert!(
    world.serve_log().contains("closed connections over a cap of handshakes: 1 since the last line"),
    "{}",
    world.serve_log()
  );
  drop(idle);
  let tls = ClientTls::new(agent.fingerprint, &world.client).expect("the TLS setup");
  eventually("a handshake gets in again", || Channel::connect(agent.address, &tls, Duration::from_secs(10)).is_ok());

  // a client that sends a TLS record a byte at a time loses the connection at the total deadline, not before. A byte
  // every 20 ms keeps each read of the handshake alive, so a deadline of each read alone would let it go on for 10 s
  let connected = Instant::now();
  let mut slow = TcpStream::connect(agent.address).expect("connected");
  slow.write_all(&[0x16, 0x03, 0x01, 0x02, 0x00]).expect("a record header");
  let mut writer = slow.try_clone().expect("a clone");
  let trickle = thread::spawn(move || {
    for _ in 0..500 {
      if writer.write_all(&[0]).is_err() {
        return;
      }
      thread::sleep(Duration::from_millis(20));
    }
  });
  let cut = read_end(&mut slow, connected);
  assert!(cut >= deadline && cut < Duration::from_secs(5), "{cut:?}");
  trickle.join().expect("the trickle");

  // the agent still serves
  assert!(matches!(agent.reply(&Request::Runs), Reply::Runs { .. }));
}

#[test]
fn heartbeats_go_on_while_the_follow_thread_is_held_in_a_file_read() {
  let world = World::new(1, &[]);
  let hold = world.home.path().join("hold");
  fs::write(&hold, "").expect("the hold");
  let agent = world.serve_with(&[("BD_HOST_TEST_HEARTBEAT_MS", "100"), ("BD_HOST_TEST_FOLLOW_HOLD", hold.to_str().expect("a text path"))]);
  let gate = world.home.path().join("gate");
  agent.start(world.spec("23-1", &format!("echo ready; {}", wait_for(&gate))));
  eventually("the first line", || events(&world, "23-1").contains("ready"));

  let mut channel = agent.ask(&Request::Follow {
    run: "23-1".to_owned(),
    offset: 0,
  });
  channel.set_read_deadline(Duration::from_secs(2)).expect("a deadline");
  let held = Instant::now();
  let mut heartbeats = 0;
  while held.elapsed() < Duration::from_secs(1) {
    match channel.recv::<Reply>() {
      Ok(Reply::Heartbeat { .. }) => heartbeats += 1,
      other => panic!("only heartbeats come while the follow is held: {other:?}"),
    }
  }
  assert!(heartbeats >= 5, "{heartbeats} heartbeats in a second");

  fs_retry::remove_file(&hold).expect("the hold released");
  let first = loop {
    match channel.recv::<Reply>().expect("a reply") {
      Reply::Heartbeat { .. } => {}
      other => break other,
    }
  };
  assert!(matches!(&first, Reply::Output { .. }), "{first:?}");
  fs::write(&gate, "").expect("the gate");
  drop(channel);
  assert_eq!(agent.follow("23-1").end, Reply::Exit { code: 0 });
}

/// macOS pushes a change of the run files to a follow (a kqueue), elsewhere the follow polls them. Here the poll and the
/// recheck of the follow take a minute and the read of the client 10 s, so on macOS each event comes only when it was
/// pushed; elsewhere a poll of 20 ms brings it
#[test]
fn a_new_event_is_pushed_to_the_follow_on_macos_and_polled_elsewhere() {
  let world = World::new(1, &[]);
  let wait = if cfg!(target_os = "macos") { "60000" } else { "20" };
  let agent = world.serve_with(&[("BD_HOST_TEST_WATCH_POLL_MS", wait), ("BD_HOST_TEST_FOLLOW_RECHECK_MS", wait)]);
  let gate = world.home.path().join("gate");
  agent.start(world.spec("24-1", &format!("echo ready; {}", wait_for(&gate))));
  eventually("the first line", || events(&world, "24-1").lines().any(|line| line.contains("\"ready\"")));

  let events = world.run_dir("24-1").join("events.log");
  let mut channel = agent.ask(&Request::Follow {
    run: "24-1".to_owned(),
    offset: fs::metadata(&events).expect("the log").len(),
  });
  channel.set_read_deadline(Duration::from_secs(10)).expect("a deadline");
  // the command waits for its gate, so the log is quiet; the test appends an event as the supervisor would, and the next
  // one after the follow sent the last: a change during a read ends the next wait too
  let mut log = fs::OpenOptions::new().append(true).open(&events).expect("the log");
  for index in 0..3 {
    let text = format!("event-{index}");
    let line = format!("{}\n", json!({"stream": Stream::Stdout, "text": text}));
    log.write_all(line.as_bytes()).expect("appended");
    loop {
      match channel.recv::<Reply>() {
        Ok(Reply::Output { text: got, .. }) if got == text => break,
        Ok(Reply::Heartbeat { .. }) => {}
        other => panic!("{text} did not come before the read deadline, so it waited for a poll: {other:?}"),
      }
    }
  }
  fs::write(&gate, "").expect("the gate");
  drop(channel);
  assert_eq!(agent.follow("24-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("24-1"));
}
