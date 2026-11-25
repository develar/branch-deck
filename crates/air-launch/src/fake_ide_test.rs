//! The fake local AHP socket of an IDE for the tests of the client, of the watch and of the discovery: it takes the
//! connections one after another, runs the handshake, and plays the steps of each connection: a frame it reads and
//! checks, a frame it writes, a pause, the close. `FakeIde::replay` plays a fixture of `fixtures/`, in the form of the
//! corpus of the IDE (`localConnect`, `local`, `toLocal`)

use crate::client::Client;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

/// The nonce the fake IDE greets with
pub(crate) const NONCE: &str = "q1Zx-Ab_9kTn0sLw4eRt2Y";

/// The value of a fixture that matches any value; the fake IDE writes 0 for it
const ANY: &str = "${any}";

/// One step of a connection of the fake IDE
#[derive(Clone, Debug)]
pub(crate) enum Step {
  /// The frame the client sends; the first one of a connection is the request of the envelope. `${any}` matches any
  /// value
  Read(Value),
  /// A frame of the IDE; `${any}` becomes 0
  Write(Value),
  /// A line of the IDE as it is, also one that is not JSON
  WriteLine(String),
  /// Bytes of the IDE without a line end
  Raw(Vec<u8>),
  Pause(Duration),
  /// The close of the connection by the IDE
  Close,
}

/// How the fake IDE runs the handshake of a connection
pub(crate) enum Handshake {
  /// Handshake v2: the opener, the greeting with `NONCE`, the envelope with its echo
  V2,
  /// Handshake v2, but the server expects another nonce than it sent: it refuses the envelope with `BAD_NONCE`
  WrongNonce,
  /// The greeting, then the refusal `code` after the envelope (`PEER_CHANGED`)
  RefuseEnvelope(&'static str),
  /// The refusal `code` instead of the greeting (`BAD_OPENER`)
  RefuseOpener(&'static str),
  /// `line` instead of the greeting
  Greet(String),
  /// The opener, then no greeting until the client closes the connection
  Silent,
}

/// The fake socket of an IDE
pub(crate) struct FakeIde {
  _dir: TempDir,
  pub(crate) socket: PathBuf,
  /// Each line the server read, as JSON
  read: mpsc::Receiver<Value>,
  server: JoinHandle<()>,
}

/// The frame `initialize` of the client
pub(crate) fn initialize() -> Value {
  json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {"channel": "ahp-root://", "protocolVersions": ["1.0.0"], "clientId": "branch-deck"}})
}

/// The result of `initialize` of the IDE
pub(crate) fn initialized() -> Value {
  json!({"jsonrpc": "2.0", "id": 1, "result": {"protocolVersion": "1.0.0", "serverSeq": 0, "snapshots": [],
    "serverInfo": {"name": "air-ide"}, "_meta": {"x-air-ij": {"methods": ["x-air-ij/hosts", "listSessions", "createSession", "x-air-ij/targets", "x-air-ij/openDraft"]}}}})
}

/// The request `id` of `method` with `params` and the channel of the root
pub(crate) fn request(id: u64, method: &str, params: &Value) -> Value {
  let mut params = params.clone();
  params.as_object_mut().unwrap().insert("channel".to_owned(), json!("ahp-root://"));
  json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// The result `result` of the request `id`
pub(crate) fn result(id: u64, result: &Value) -> Value {
  json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// The error of the request `id` with the code `code` of its data, as the IDE refuses
pub(crate) fn refusal(id: u64, code: &str, message: &str) -> Value {
  json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32010, "message": message,
    "data": {"reason": code.to_ascii_lowercase(), "code": code, "message": message, "hint": "A hint.", "details": {}}}})
}

/// A connection that asks `method` with `params` once and gets `answer`
pub(crate) fn exchange(method: &str, params: &Value, answer: Value) -> Vec<Step> {
  vec![
    Step::Read(initialize()),
    Step::Write(initialized()),
    Step::Read(request(2, method, params)),
    Step::Write(answer),
  ]
}

/// Whether `actual` is `expected`, where `${any}` of `expected` matches any value
fn matches(expected: &Value, actual: &Value) -> bool {
  match (expected, actual) {
    (Value::String(any), _) if any == ANY => true,
    (Value::Object(expected), Value::Object(actual)) => expected.len() == actual.len() && expected.iter().all(|(key, value)| actual.get(key).is_some_and(|it| matches(value, it))),
    (Value::Array(expected), Value::Array(actual)) => expected.len() == actual.len() && expected.iter().zip(actual).all(|(a, b)| matches(a, b)),
    _ => expected == actual,
  }
}

/// `frame` with 0 for each `${any}`
fn concrete(frame: &Value) -> Value {
  match frame {
    Value::String(any) if any == ANY => json!(0),
    Value::Object(fields) => Value::Object(fields.iter().map(|(key, value)| (key.clone(), concrete(value))).collect()),
    Value::Array(items) => Value::Array(items.iter().map(concrete).collect()),
    other => other.clone(),
  }
}

fn refusal_line(code: &str) -> String {
  json!({"proto": "local-peer", "v": 2, "error": code}).to_string()
}

impl FakeIde {
  /// The connections `connections` one after another, each with handshake v2
  pub(crate) fn serve(connections: Vec<Vec<Step>>) -> Self {
    Self::start(connections.into_iter().map(|steps| (Handshake::V2, steps)).collect())
  }

  /// One connection with the handshake `handshake`
  pub(crate) fn with_handshake(handshake: Handshake, steps: Vec<Step>) -> Self {
    Self::start(vec![(handshake, steps)])
  }

  /// The connections of the fixture `name` of `fixtures/`: a `localConnect` opens one, a `local` is its frame of the
  /// client, a `toLocal` its frame of the IDE, or the close
  pub(crate) fn replay(name: &str) -> Self {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut clients: Vec<String> = Vec::new();
    let mut connections: Vec<Vec<Step>> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
      let value: Value = serde_json::from_str(line).unwrap();
      let (kind, body) = value.as_object().unwrap().iter().next().unwrap();
      let client = body["client"].as_str().map(str::to_owned);
      let index = |client: Option<String>| clients.iter().position(|it| Some(it) == client.as_ref()).unwrap();
      match kind.as_str() {
        "comment" => {}
        "localConnect" => {
          clients.push(client.unwrap());
          connections.push(vec![Step::Read(body["request"].clone())]);
        }
        "local" => connections[index(client)].push(Step::Read(body["frame"].clone())),
        "toLocal" if body["frame"].is_null() => connections[index(client)].push(Step::Close),
        "toLocal" => connections[index(client)].push(Step::Write(body["frame"].clone())),
        other => panic!("{}: a step {other} of no local app", path.display()),
      }
    }
    Self::serve(connections)
  }

  fn start(connections: Vec<(Handshake, Vec<Step>)>) -> Self {
    let dir = tempfile::Builder::new().prefix("air").tempdir().unwrap();
    let socket = dir.path().join("ahp.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (sender, read) = mpsc::channel();
    let server = std::thread::spawn(move || {
      for (handshake, steps) in connections {
        let (stream, _) = listener.accept().unwrap();
        play(stream, handshake, steps, &sender);
      }
    });
    Self { _dir: dir, socket, read, server }
  }

  pub(crate) fn client(&self) -> Client {
    Client::new(&self.socket)
  }

  /// Each line the server read, once it played every step: the opener, the envelope and the frames after it
  pub(crate) fn finish(self) -> Vec<Value> {
    self.server.join().unwrap();
    self.read.try_iter().collect()
  }
}

/// Plays one connection
fn play(stream: UnixStream, handshake: Handshake, steps: Vec<Step>, sender: &mpsc::Sender<Value>) {
  let mut reader = BufReader::new(stream);
  // the line the client sent, or none once it closed the connection
  let read_line = |reader: &mut BufReader<UnixStream>| {
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
      return None;
    }
    let value: Value = serde_json::from_str(&line).unwrap();
    sender.send(value.clone()).unwrap();
    Some(value)
  };
  let write = |reader: &mut BufReader<UnixStream>, line: &str| {
    // the client may close as soon as it has enough
    let _ = reader.get_mut().write_all(format!("{line}\n").as_bytes());
  };
  let hold = |reader: &mut BufReader<UnixStream>| {
    let mut rest = Vec::new();
    let _ = std::io::Read::read_to_end(reader, &mut rest);
    rest
  };
  let opener = read_line(&mut reader).unwrap();
  // the request of the envelope: the first frame the client sent
  let mut envelope_request = match handshake {
    Handshake::RefuseOpener(code) => {
      write(&mut reader, &refusal_line(code));
      return;
    }
    Handshake::Silent => {
      assert!(hold(&mut reader).is_empty(), "the client sends nothing before the greeting");
      return;
    }
    Handshake::Greet(line) => {
      write(&mut reader, &line);
      // the client closes the connection without an envelope
      assert!(hold(&mut reader).is_empty(), "the client sends no envelope after a bad greeting");
      return;
    }
    Handshake::V2 | Handshake::WrongNonce | Handshake::RefuseEnvelope(_) => {
      assert_eq!(opener, json!({"proto": "local-peer", "v": 2}));
      write(&mut reader, &json!({"proto": "local-peer", "v": 2, "nonce": NONCE}).to_string());
      let envelope = read_line(&mut reader).unwrap();
      let expected = if matches!(handshake, Handshake::WrongNonce) { "another" } else { NONCE };
      if envelope["nonce"] != expected {
        write(&mut reader, &refusal_line("BAD_NONCE"));
        return;
      }
      if let Handshake::RefuseEnvelope(code) = handshake {
        write(&mut reader, &refusal_line(code));
        return;
      }
      assert_eq!(
        envelope.as_object().unwrap().len(),
        2,
        "the envelope holds the nonce and the request, no access: {envelope}"
      );
      Some(envelope["request"].clone())
    }
  };
  for step in steps {
    match step {
      Step::Read(expected) => {
        let actual = envelope_request.take().or_else(|| read_line(&mut reader));
        let actual = actual.unwrap_or_else(|| panic!("the client closed the connection before it sent {expected}"));
        assert!(matches(&expected, &actual), "the client sent\n{actual}\ninstead of\n{expected}");
      }
      Step::Write(frame) => write(&mut reader, &concrete(&frame).to_string()),
      Step::WriteLine(line) => write(&mut reader, &line),
      Step::Raw(bytes) => {
        let _ = reader.get_mut().write_all(&bytes);
      }
      Step::Pause(pause) => std::thread::sleep(pause),
      Step::Close => return,
    }
  }
  // no more frames until the client closes the connection
  hold(&mut reader);
}
