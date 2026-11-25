//! The watches of the sessions of Start in Air over a fake IDE: what each row tells the page, which end each watch gets,
//! and that the watches of one IDE share one connection (`AirWatches`)

use super::{AirSessionStatus, AirWatchEnd, AirWatches, WatchOutcome};
use crate::transport::{EventSink, SinkGone, WindowId};
use air_launch::client::Client;
use air_launch::protocol::Activity;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

/// The longest wait of a test for a status or an end: a hang fails the test, not a slow machine
const WAIT: Duration = Duration::from_secs(30);

/// A page that hears each status; `gone`: the page went away, a send fails
struct Page {
  heard: Mutex<mpsc::Sender<AirSessionStatus>>,
  gone: bool,
}

impl EventSink<AirSessionStatus> for Page {
  fn send(&self, event: AirSessionStatus) -> Result<(), SinkGone> {
    if self.gone {
      return Err(SinkGone);
    }
    self.heard.lock().map_err(|_poisoned| SinkGone)?.send(event).map_err(|_gone| SinkGone)
  }
}

/// A page and what it hears
fn page(gone: bool) -> (Arc<Page>, mpsc::Receiver<AirSessionStatus>) {
  let (sender, heard) = mpsc::channel();
  (Arc::new(Page { heard: Mutex::new(sender), gone }), heard)
}

fn status(activity: Activity, title: Option<&str>) -> AirSessionStatus {
  AirSessionStatus {
    title: title.map(str::to_owned),
    activity,
  }
}

/// The facts of Air of a row (`_meta`), which name its activity
fn facts(activity: &str) -> Value {
  json!({"x-air-ij": {"projectDirectory": "/work/idea", "threadId": "t-1", "launchTargetId": "codex", "updatedAt": 1,
    "activity": activity, "archivable": false}})
}

fn row(session: &str, title: &str, activity: &str) -> Value {
  json!({"resource": session, "provider": "codex", "title": title, "status": 1, "_meta": facts(activity),
    "createdAt": "2026-11-18T09:00:00.000Z", "modifiedAt": "2026-11-18T10:00:00.000Z", "project": "project-a"})
}

fn notice(method: &str, params: &Value) -> Value {
  let mut params = params.clone();
  params.as_object_mut().unwrap().insert("channel".to_owned(), json!("ahp-root://"));
  json!({"jsonrpc": "2.0", "method": method, "params": params})
}

fn changed(session: &str, changes: &Value) -> Value {
  notice("root/sessionSummaryChanged", &json!({"session": session, "changes": changes}))
}

fn removed(session: &str) -> Value {
  notice("root/sessionRemoved", &json!({"session": session}))
}

/// A fake IDE: each connection runs the handshake, answers `initialize`, `subscribe` and `listSessions` (with `rows`, or
/// with the refusal `refusal`). Each frame the test sends goes to the connection that listed last, and a `None` closes it
struct FakeIde {
  _dir: tempfile::TempDir,
  socket: PathBuf,
  frames: mpsc::Sender<Option<Value>>,
  connections: Arc<AtomicUsize>,
}

impl FakeIde {
  fn start(rows: Vec<Value>, refusal: Option<&'static str>) -> Self {
    let dir = tempfile::Builder::new().prefix("air").tempdir().unwrap();
    let socket = dir.path().join("ahp.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (frames, sent) = mpsc::channel::<Option<Value>>();
    let connections = Arc::new(AtomicUsize::new(0));
    let latest: Arc<Mutex<Option<UnixStream>>> = Arc::default();
    let accepted = Arc::clone(&connections);
    let listed = Arc::clone(&latest);
    std::thread::spawn(move || {
      while let Ok((stream, _)) = listener.accept() {
        accepted.fetch_add(1, Ordering::SeqCst);
        let rows = rows.clone();
        let listed = Arc::clone(&listed);
        std::thread::spawn(move || serve(stream, &rows, refusal, &listed));
      }
    });
    std::thread::spawn(move || {
      while let Ok(frame) = sent.recv() {
        let mut latest = latest.lock().unwrap();
        let Some(stream) = latest.as_mut() else {
          continue;
        };
        match frame {
          Some(frame) => {
            let _ = stream.write_all(format!("{frame}\n").as_bytes());
          }
          None => {
            let _ = stream.shutdown(std::net::Shutdown::Both);
          }
        }
      }
    });
    Self {
      _dir: dir,
      socket,
      frames,
      connections,
    }
  }

  fn client(&self) -> Client {
    Client::new(&self.socket)
  }

  fn send(&self, frame: Value) {
    self.frames.send(Some(frame)).unwrap();
  }

  fn close(&self) {
    self.frames.send(None).unwrap();
  }
}

/// One connection up to the answer of `listSessions`; then its stream takes the frames of the test
fn serve(stream: UnixStream, rows: &[Value], refusal: Option<&str>, latest: &Mutex<Option<UnixStream>>) -> Option<()> {
  let mut reader = BufReader::new(stream);
  let read = |reader: &mut BufReader<UnixStream>| {
    let mut line = String::new();
    reader.read_line(&mut line).ok().filter(|read| *read > 0)?;
    serde_json::from_str::<Value>(&line).ok()
  };
  let write = |reader: &mut BufReader<UnixStream>, frame: &Value| {
    let _ = reader.get_mut().write_all(format!("{frame}\n").as_bytes());
  };
  read(&mut reader)?;
  write(&mut reader, &json!({"proto": "local-peer", "v": 2, "nonce": "q1Zx-Ab_9kTn0sLw4eRt2Y"}));
  let envelope = read(&mut reader)?;
  assert_eq!(envelope["request"]["method"], "initialize");
  write(
    &mut reader,
    &json!({"jsonrpc": "2.0", "id": 1, "result": {"protocolVersion": "1.0.0", "serverSeq": 0, "snapshots": []}}),
  );
  for method in ["subscribe", "listSessions"] {
    let request = read(&mut reader)?;
    assert_eq!(request["method"], method);
    let answer = match (method, refusal) {
      ("listSessions", Some(code)) => json!({"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32010, "message": "no",
        "data": {"reason": "denied", "code": code, "message": "no", "hint": "h", "details": {}}}}),
      ("listSessions", None) => json!({"jsonrpc": "2.0", "id": request["id"], "result": {"items": rows}}),
      _ => json!({"jsonrpc": "2.0", "id": request["id"], "result": {"snapshot": {}}}),
    };
    if method == "listSessions" {
      // the frames of the test come after the list
      let mut latest = latest.lock().unwrap();
      write(&mut reader, &answer);
      *latest = Some(reader.get_ref().try_clone().unwrap());
    } else {
      write(&mut reader, &answer);
    }
  }
  // until the client closes the connection
  let mut rest = Vec::new();
  let _ = std::io::Read::read_to_end(&mut reader, &mut rest);
  Some(())
}

/// Watches `session` for `window` with `page`, and the receiver of its end
fn watch(watches: &Arc<AirWatches>, ide: &FakeIde, window: &WindowId, session: &str, page: Arc<Page>) -> mpsc::Receiver<WatchOutcome> {
  watches.watch(&ide.client(), window, session, page).unwrap()
}

fn heard(statuses: &mpsc::Receiver<AirSessionStatus>) -> AirSessionStatus {
  statuses.recv_timeout(WAIT).unwrap()
}

fn ended(end: &mpsc::Receiver<WatchOutcome>) -> AirWatchEnd {
  end.recv_timeout(WAIT).unwrap().unwrap()
}

/// The watches of the IDEs whose connection runs now
fn open_ides(watches: &AirWatches) -> usize {
  watches.ides().values().filter(|ide| !ide.state().ended).count()
}

#[test]
fn a_watch_tells_the_page_the_status_of_its_row_now_and_after_each_change_then_that_it_is_gone() {
  let ide = FakeIde::start(vec![row("s1", "Fix it", "PROCESSING"), row("s9", "Other", "READY")], None);
  let watches = Arc::new(AirWatches::default());
  let (page, statuses) = page(false);
  let end = watch(&watches, &ide, &WindowId::main(), "s1", page);
  assert_eq!(heard(&statuses), status(Activity::Processing, Some("Fix it")));

  ide.send(changed("s1", &json!({"_meta": facts("NEEDS_INPUT"), "modifiedAt": "2026-11-18T10:05:00.000Z"})));
  // a change of another row, and of another field of the row, tell the page nothing
  ide.send(changed("s9", &json!({"_meta": facts("PROCESSING")})));
  ide.send(changed("s1", &json!({"modifiedAt": "2026-11-18T10:06:00.000Z"})));
  ide.send(changed("s1", &json!({"title": ""})));
  ide.send(changed("s1", &json!({"_meta": facts("LATER")})));
  ide.send(removed("s1"));
  assert_eq!(heard(&statuses), status(Activity::NeedsInput, Some("Fix it")));
  assert_eq!(heard(&statuses), status(Activity::NeedsInput, None));
  assert_eq!(heard(&statuses), status(Activity::Other, None));
  assert_eq!(ended(&end), AirWatchEnd::Gone);
  statuses.try_recv().unwrap_err();
  // the last watch that ended closed the connection of the IDE
  assert_eq!(open_ides(&watches), 0);
}

#[test]
fn the_watches_of_one_ide_share_one_connection_and_a_row_that_comes_later_is_followed() {
  let ide = FakeIde::start(vec![row("s1", "First", "PROCESSING")], None);
  let watches = Arc::new(AirWatches::default());
  let (first_page, first) = page(false);
  let first_end = watch(&watches, &ide, &WindowId::main(), "s1", first_page);
  assert_eq!(heard(&first), status(Activity::Processing, Some("First")));
  // the row of the second session is not there yet: its watch waits for it
  let (second_page, second) = page(false);
  let second_end = watch(&watches, &ide, &WindowId::new("review-1"), "s2", second_page);
  ide.send(notice("root/sessionAdded", &json!({"summary": row("s2", "Second", "REVIEWING")})));
  assert_eq!(heard(&second), status(Activity::Reviewing, Some("Second")));
  assert_eq!(ide.connections.load(Ordering::SeqCst), 1);

  ide.send(removed("s1"));
  assert_eq!(ended(&first_end), AirWatchEnd::Gone);
  ide.send(changed("s2", &json!({"_meta": facts("UNREAD")})));
  assert_eq!(heard(&second), status(Activity::Unread, Some("Second")));
  ide.close();
  assert_eq!(ended(&second_end), AirWatchEnd::Closed);
}

#[test]
fn a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window() {
  let ide = FakeIde::start(vec![row("s1", "First", "PROCESSING"), row("s2", "Second", "PROCESSING")], None);
  let watches = Arc::new(AirWatches::default());
  let main = WindowId::main();
  let review = WindowId::new("review-1");

  let (page_one, heard_one) = page(false);
  let stopped = watch(&watches, &ide, &main, "s1", page_one);
  heard(&heard_one);
  watches.stop(&main, "s1");
  assert_eq!(ended(&stopped), AirWatchEnd::Stopped);

  // a newer watch of the same session stops the one before and keeps its connection
  let (older_page, older_heard) = page(false);
  let older = watch(&watches, &ide, &main, "s2", older_page);
  heard(&older_heard);
  let (newer_page, newer_heard) = page(false);
  let newer = watch(&watches, &ide, &main, "s2", newer_page);
  assert_eq!(heard(&newer_heard), status(Activity::Processing, Some("Second")));
  assert_eq!(ended(&older), AirWatchEnd::Stopped);

  // the page of the main window goes away: its watches stop, those of another window go on
  let (other_page, other_heard) = page(false);
  let elsewhere = watch(&watches, &ide, &review, "s1", other_page);
  heard(&other_heard);
  watches.forget_window(&main);
  assert_eq!(ended(&newer), AirWatchEnd::Stopped);
  ide.send(changed("s1", &json!({"_meta": facts("READY")})));
  assert_eq!(heard(&other_heard), status(Activity::Ready, Some("First")));

  // a page that went away stops its watch at its first status
  let (gone_page, _) = page(true);
  let gone = watch(&watches, &ide, &main, "s1", gone_page);
  assert_eq!(ended(&gone), AirWatchEnd::Stopped);

  watches.stop(&review, "s1");
  assert_eq!(ended(&elsewhere), AirWatchEnd::Stopped);
  assert_eq!(open_ides(&watches), 0);
  assert_eq!(ide.connections.load(Ordering::SeqCst), 2, "the stop of the first watch closed its connection");
}

#[test]
fn a_refusal_of_the_list_ends_each_watch_refused() {
  let ide = FakeIde::start(Vec::new(), Some("DENIED"));
  let watches = Arc::new(AirWatches::default());
  let (page, _) = page(false);
  let end = watch(&watches, &ide, &WindowId::main(), "s1", page);
  assert_eq!(
    ended(&end),
    AirWatchEnd::Refused {
      code: "DENIED".to_owned(),
      message: Some("no".to_owned()),
    }
  );
}
