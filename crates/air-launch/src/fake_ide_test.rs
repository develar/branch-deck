//! The fake socket of an IDE for the tests of the client and of the discovery: one connection, the handshake and the
//! reply a test names

use crate::client::Client;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
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

/// The fake socket of an IDE: it takes one connection, runs the handshake as `handshake` says, reads the request and
/// answers as `reply` says
pub(crate) struct FakeIde {
  _dir: TempDir,
  pub(crate) socket: PathBuf,
  /// Each line the server read, as JSON
  read: mpsc::Receiver<Value>,
  server: JoinHandle<()>,
}

/// How the fake IDE runs the handshake
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
  /// An IDE before handshake v2: it reads the opener as the request and answers it at once
  Legacy,
  /// The opener, then no greeting until the client closes the connection
  Silent,
}

pub(crate) enum Reply {
  Line(String),
  /// Lines, then the close
  Lines(Vec<String>),
  /// Lines with a pause before each, then no more until the client closes the connection
  Paced(Vec<String>, Duration),
  /// Bytes without a line end, then the close
  Raw(Vec<u8>),
  /// No answer until the client gives up (the connection closes from its side)
  Never,
}

fn refusal_line(code: &str) -> String {
  json!({"proto": "local-peer", "v": 2, "error": code}).to_string()
}

impl FakeIde {
  pub(crate) fn start(reply: Reply) -> Self {
    Self::with_handshake(Handshake::V2, reply)
  }

  pub(crate) fn with_handshake(handshake: Handshake, reply: Reply) -> Self {
    let dir = tempfile::Builder::new().prefix("air").tempdir().unwrap();
    let socket = dir.path().join("launch.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (sender, read) = mpsc::channel();
    let server = std::thread::spawn(move || {
      let (stream, _) = listener.accept().unwrap();
      let mut reader = BufReader::new(stream);
      let read_line = |reader: &mut BufReader<UnixStream>| {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let value: Value = serde_json::from_str(&line).unwrap();
        sender.send(value.clone()).unwrap();
        value
      };
      let opener = read_line(&mut reader);
      let write = |reader: &mut BufReader<UnixStream>, line: &str| reader.get_mut().write_all(format!("{line}\n").as_bytes()).unwrap();
      match handshake {
        Handshake::Legacy => {}
        Handshake::RefuseOpener(code) => {
          write(&mut reader, &refusal_line(code));
          return;
        }
        Handshake::Silent => {
          let mut rest = Vec::new();
          let _ = std::io::Read::read_to_end(&mut reader, &mut rest);
          assert!(rest.is_empty(), "the client sends nothing before the greeting");
          return;
        }
        Handshake::Greet(line) => {
          write(&mut reader, &line);
          // the client closes the connection without an envelope
          let mut rest = Vec::new();
          let _ = std::io::Read::read_to_end(&mut reader, &mut rest);
          return;
        }
        Handshake::V2 | Handshake::WrongNonce | Handshake::RefuseEnvelope(_) => {
          assert_eq!(opener, json!({"proto": "local-peer", "v": 2}));
          write(&mut reader, &json!({"proto": "local-peer", "v": 2, "nonce": NONCE}).to_string());
          let envelope = read_line(&mut reader);
          let expected = if matches!(handshake, Handshake::WrongNonce) { "another" } else { NONCE };
          if envelope["nonce"] != expected {
            write(&mut reader, &refusal_line("BAD_NONCE"));
            return;
          }
          if let Handshake::RefuseEnvelope(code) = handshake {
            write(&mut reader, &refusal_line(code));
            return;
          }
        }
      }
      let mut stream = reader.into_inner();
      match reply {
        Reply::Line(line) => stream.write_all(format!("{line}\n").as_bytes()).unwrap(),
        // all lines in one write: the client splits them
        Reply::Lines(lines) => stream.write_all(format!("{}\n", lines.join("\n")).as_bytes()).unwrap(),
        Reply::Paced(lines, pause) => {
          for line in lines {
            std::thread::sleep(pause);
            stream.write_all(format!("{line}\n").as_bytes()).unwrap();
          }
          let mut rest = Vec::new();
          let _ = std::io::Read::read_to_end(&mut stream, &mut rest);
        }
        Reply::Raw(bytes) => {
          // the client may close as soon as it has enough
          let _ = stream.write_all(&bytes);
        }
        Reply::Never => {
          let mut rest = Vec::new();
          let _ = std::io::Read::read_to_end(&mut stream, &mut rest);
        }
      }
    });
    Self { _dir: dir, socket, read, server }
  }

  pub(crate) fn client(&self) -> Client {
    Client::new(&self.socket)
  }

  /// Each line the server read, once it answered
  pub(crate) fn finish_lines(self) -> Vec<Value> {
    self.server.join().unwrap();
    self.read.try_iter().collect()
  }

  /// The request object of the envelope the server read, once it answered
  pub(crate) fn finish(self) -> Value {
    let lines = self.finish_lines();
    assert_eq!(lines.len(), 2, "the opener and the envelope: {lines:?}");
    assert_eq!(lines[1]["nonce"], NONCE, "the envelope echoes the nonce of the greeting");
    assert_eq!(lines[1].as_object().unwrap().len(), 2, "the envelope holds the nonce and the request: {}", lines[1]);
    lines[1]["request"].clone()
  }
}
