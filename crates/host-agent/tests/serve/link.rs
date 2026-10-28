//! The link of the agent: the deadline of a handshake and the cap of handshakes of one peer, the heartbeats of a follow
//! whose thread is held, and the time until a new event reaches a follow

use super::{World, events, eventually, wait_for};
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
  let world = World::new(1, &[]);
  let agent = world.serve_with(&[("BD_HOST_TEST_HANDSHAKE_MS", "2000")]);
  let read_end = |stream: &mut TcpStream| {
    stream.set_read_timeout(Some(Duration::from_secs(10))).expect("a deadline");
    let started = Instant::now();
    let mut byte = [0u8; 1];
    let read = stream.read(&mut byte);
    assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    started.elapsed()
  };

  // 8 handshakes that send nothing fill the cap of one peer, and its next socket is closed at once
  let idle: Vec<TcpStream> = (0..8).map(|_| TcpStream::connect(agent.address).expect("connected")).collect();
  thread::sleep(Duration::from_millis(300));
  let mut over = TcpStream::connect(agent.address).expect("connected");
  let waited = read_end(&mut over);
  assert!(waited < Duration::from_millis(1500), "{waited:?}");
  drop(idle);
  thread::sleep(Duration::from_millis(500));

  // a client that sends a TLS record a byte at a time loses the connection at the total deadline, not before
  let mut slow = TcpStream::connect(agent.address).expect("connected");
  slow.write_all(&[0x16, 0x03, 0x01, 0x02, 0x00]).expect("a record header");
  let mut writer = slow.try_clone().expect("a clone");
  let trickle = thread::spawn(move || {
    for _ in 0..40 {
      if writer.write_all(&[0]).is_err() {
        return;
      }
      thread::sleep(Duration::from_millis(250));
    }
  });
  let cut = read_end(&mut slow);
  assert!(cut >= Duration::from_millis(1500) && cut < Duration::from_secs(5), "{cut:?}");
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

#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "only macOS pushes the changes of the run files; Linux polls")]
fn a_new_event_reaches_the_follow_in_less_than_50_ms() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let gate = world.home.path().join("gate");
  agent.start(world.spec("24-1", &format!("echo ready; {}", wait_for(&gate))));
  eventually("the first line", || events(&world, "24-1").lines().any(|line| line.contains("\"ready\"")));

  let events = world.run_dir("24-1").join("events.log");
  let mut channel = agent.ask(&Request::Follow {
    run: "24-1".to_owned(),
    offset: fs::metadata(&events).expect("the log").len(),
  });
  // the command waits for its gate, so the log is quiet; the test appends an event as the supervisor would
  let mut log = fs::OpenOptions::new().append(true).open(&events).expect("the log");
  let mut latencies = Vec::new();
  for index in 0..7 {
    thread::sleep(Duration::from_millis(300));
    let text = format!("event-{index}");
    let line = format!("{}\n", json!({"stream": Stream::Stdout, "text": text}));
    let written = Instant::now();
    log.write_all(line.as_bytes()).expect("appended");
    loop {
      match channel.recv::<Reply>().expect("a reply") {
        Reply::Output { text: got, .. } if got == text => break,
        Reply::Heartbeat { .. } => {}
        other => panic!("{other:?}"),
      }
    }
    latencies.push(written.elapsed());
  }
  // the median: one event on a loaded host can wait for the scheduler, while a poll of 200 ms delays each event
  latencies.sort();
  assert!(latencies[3] < Duration::from_millis(50), "the median is over 50 ms: {latencies:?}");
  fs::write(&gate, "").expect("the gate");
  drop(channel);
  assert_eq!(agent.follow("24-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("24-1"));
}
