//! What a follow and a fetch send: a follow from an offset in the middle of a run, and the TLS records of a fetch of many
//! small files

use super::{ALL_PHASES, Agent, DEADLINE, World, wait_for};
use host_link::channel::{Channel, ClientTls};
use host_link::proto::{Reply, Request, Stream};
use pretty_assertions::assert_eq;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

impl Agent<'_> {
  /// Follows a run from `offset` until the line `text`, and answers each line with the offset after it
  fn lines_until(&self, run: &str, offset: u64, text: &str) -> Vec<(Stream, String, u64)> {
    let mut channel = self.ask(&Request::Follow { run: run.to_owned(), offset });
    let mut lines = Vec::new();
    loop {
      match channel.recv::<Reply>().unwrap_or_else(|error| panic!("{error}; {}", self.dump(run))) {
        Reply::Output { stream, text: line, end } => {
          let seen = line == text;
          lines.push((stream, line, end));
          if seen {
            return lines;
          }
        }
        Reply::Heartbeat { .. } | Reply::Phases { .. } => {}
        other => panic!("{other:?} before {text}; {}", self.dump(run)),
      }
    }
  }
}

/// A follow from the offset after a line in the middle of a run goes on from the next line. The steps before the
/// command have ended while it runs, a follow after the end gets the steps too, and an offset inside a line refuses
/// with `bad_request`
#[test]
fn a_follow_from_an_offset_in_the_middle_of_a_run_goes_on_from_there() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let gate = world.home.path().join("gate");
  agent.start(world.spec("3-1", &format!("echo first; echo second; {}; echo third", wait_for(&gate))));
  let seen = agent.lines_until("3-1", 0, "second");
  let after_first = seen.iter().find(|(_, text, _)| text == "first").expect("the first line").2;
  // The command goes on, so the steps before it have ended
  let Reply::Runs { runs } = agent.reply(&Request::Runs) else {
    panic!("not runs");
  };
  let names: Vec<&str> = runs[0].phases.iter().map(|phase| phase.name.as_str()).collect();
  assert_eq!(names, ALL_PHASES[..4]);
  assert_eq!(runs[0].exit, None);

  fs::write(&gate, "").expect("the gate");
  let followed = agent.follow_from("3-1", after_first);
  assert_eq!(followed.of(Stream::Stdout), ["second", "third"]);
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("3-1"));
  assert_eq!(followed.phase_names(), ALL_PHASES);

  // A follow that comes after the end gets the steps too
  let again = agent.follow("3-1");
  assert_eq!(again.of(Stream::Stdout), ["first", "second", "third"]);
  assert_eq!((again.phases, again.end), (followed.phases, Reply::Exit { code: 0 }));

  let refused = agent.reply(&Request::Follow {
    run: "3-1".to_owned(),
    offset: after_first - 1,
  });
  assert!(matches!(&refused, Reply::Refused { code, .. } if code == "bad_request"), "{refused:?}");
}

/// A TCP proxy on a loopback port to `target` that counts the TLS records from `target` to the client. A TLS 1.3
/// record has a header of 5 bytes, with the length in the last two
fn record_counter(target: SocketAddr) -> (SocketAddr, Arc<AtomicUsize>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
  let address = listener.local_addr().expect("the address");
  let records = Arc::new(AtomicUsize::new(0));
  let counted = Arc::clone(&records);
  thread::spawn(move || {
    let (mut client, _) = listener.accept().expect("the client");
    let mut agent = TcpStream::connect(target).expect("the agent");
    let (mut to_agent, mut from_client) = (agent.try_clone().expect("a clone"), client.try_clone().expect("a clone"));
    thread::spawn(move || {
      let _ = std::io::copy(&mut from_client, &mut to_agent);
      let _ = to_agent.shutdown(Shutdown::Write);
    });
    let mut head = [0u8; 5];
    while agent.read_exact(&mut head).is_ok() {
      let mut body = vec![0u8; usize::from(u16::from_be_bytes([head[3], head[4]]))];
      if agent.read_exact(&mut body).is_err() {
        break;
      }
      counted.fetch_add(1, Ordering::SeqCst);
      if client.write_all(&head).and_then(|()| client.write_all(&body)).is_err() {
        break;
      }
    }
    let _ = client.shutdown(Shutdown::Write);
  });
  (address, records)
}

/// The files of a fetch go into the buffer of the channel, so 300 small files take a few TLS records and not three per
/// file
#[test]
fn a_fetch_of_many_small_files_takes_few_tls_records() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let script = "mkdir -p out/bt-runs && i=0 && while [ $i -lt 300 ]; do echo $i >out/bt-runs/$i.txt; i=$((i+1)); done";
  agent.start(world.spec("31-1", script));
  assert_eq!(agent.follow("31-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("31-1"));

  let (proxy, records) = record_counter(agent.address);
  let tls = ClientTls::new(agent.fingerprint, &world.client).expect("the TLS setup");
  let mut channel = Channel::connect(proxy, &tls, DEADLINE).expect("connected");
  channel.send(&Request::hello("test")).expect("the hello");
  assert!(matches!(channel.recv::<Reply>().expect("the hello reply"), Reply::Hello { .. }));
  let before = records.load(Ordering::SeqCst);
  channel.send(&Request::Fetch { run: "31-1".to_owned() }).expect("the request");
  let mut files = 0;
  loop {
    match channel.recv::<Reply>().expect("a reply") {
      Reply::File { .. } => {
        channel.recv_stream(&mut std::io::sink()).expect("the bytes");
        files += 1;
      }
      Reply::Fetched { files: count, .. } => {
        assert_eq!(count, 300);
        break;
      }
      other => panic!("{other:?}"),
    }
  }
  assert_eq!(files, 300);
  let fetched = records.load(Ordering::SeqCst) - before;
  assert!(fetched <= 10, "the fetch took {fetched} TLS records");
}
