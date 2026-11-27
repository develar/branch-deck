//! The answers of `air_open_draft` over a fake IDE: no IDE, the open of the New Session tab with the project and the
//! prompt alone, a refusal, and an error of the exchange

use super::{AirDraftAnswer, AirDraftParams, AirIde, open_draft};
use domain_types::error::ErrorCode;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

/// The absolute path of `path` on this platform: `/work/idea` stays on Unix, becomes `C:\work\idea` on Windows
fn absolute(path: &str) -> String {
  if cfg!(windows) { format!("C:{}", path.replace('/', "\\")) } else { path.to_owned() }
}

/// A fake IDE that hosts `/work/idea`: each connection runs the handshake and answers one request, `x-air-ij/hosts` with
/// the host, `x-air-ij/openDraft` with `draft` (a result, an error, or null for a close without an answer). The params
/// of each draft go to `drafts`
struct FakeIde {
  dir: tempfile::TempDir,
  drafts: mpsc::Receiver<Value>,
}

impl FakeIde {
  fn start(draft: Value) -> Self {
    let dir = tempfile::Builder::new().prefix("air").tempdir().unwrap();
    let socket = dir.path().join("ahp.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let file = json!({"schema": 2, "pid": std::process::id(), "product": "IU", "build": "IU-271.1", "socket": socket});
    std::fs::write(dir.path().join("ide.json"), file.to_string()).unwrap();
    let (sender, drafts) = mpsc::channel();
    std::thread::spawn(move || {
      while let Ok((stream, _)) = listener.accept() {
        serve(stream, &draft, &sender);
      }
    });
    Self { dir, drafts }
  }

  fn dir(&self) -> &Path {
    self.dir.path()
  }
}

fn serve(stream: UnixStream, draft: &Value, drafts: &mpsc::Sender<Value>) -> Option<()> {
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
  read(&mut reader)?;
  write(
    &mut reader,
    &json!({"jsonrpc": "2.0", "id": 1, "result": {"protocolVersion": "1.0.0", "serverSeq": 0, "snapshots": []}}),
  );
  let request = read(&mut reader)?;
  let mut answer = match request["method"].as_str() {
    Some("x-air-ij/hosts") => json!({"result": {"product": "IU", "build": "271.1", "hosts": [{"projectDirectory": absolute("/work/idea")}], "focusedAt": null}}),
    Some("x-air-ij/openDraft") => {
      let _ = drafts.send(request["params"].clone());
      if draft.is_null() {
        return None;
      }
      draft.clone()
    }
    other => panic!("a request of no local app: {other:?}"),
  };
  answer["jsonrpc"] = json!("2.0");
  answer["id"] = request["id"].clone();
  write(&mut reader, &answer);
  Some(())
}

fn params(project: &str) -> AirDraftParams {
  AirDraftParams {
    project: absolute(project),
    prompt: "Fix the Safe Push of auth".to_owned(),
  }
}

fn ide() -> AirIde {
  AirIde {
    product: "IU".to_owned(),
    build: "IU-271.1".to_owned(),
    project: absolute("/work/idea"),
  }
}

#[test]
fn the_ide_of_the_repository_opens_the_new_session_tab_with_the_project_and_the_prompt_alone() {
  let fake = FakeIde::start(json!({"result": {}}));
  assert_eq!(
    open_draft(Some(fake.dir()), &params("/work/idea/community")).unwrap(),
    AirDraftAnswer::Opened { ide: ide() }
  );
  assert_eq!(
    fake.drafts.try_recv().unwrap(),
    json!({"channel": "ahp-root://", "projectDirectory": absolute("/work/idea/community"), "prompt": "Fix the Safe Push of auth"})
  );
}

#[test]
fn no_directory_of_instance_files_and_no_ide_of_the_repository_are_no_ide() {
  assert_eq!(open_draft(None, &params("/work/idea")).unwrap(), AirDraftAnswer::NoIde);
  let fake = FakeIde::start(json!({"result": {}}));
  assert_eq!(open_draft(Some(fake.dir()), &params("/elsewhere")).unwrap(), AirDraftAnswer::NoIde);
  fake.drafts.try_recv().unwrap_err();
}

#[test]
fn a_refusal_of_the_ide_is_an_answer_and_a_close_without_an_answer_an_error() {
  let fake = FakeIde::start(json!({"error": {"code": -32603, "message": "No agent of the IDE can start a session.",
    "data": {"reason": "internal", "code": "UNAVAILABLE", "message": "No agent of the IDE can start a session.", "details": {}}}}));
  assert_eq!(
    open_draft(Some(fake.dir()), &params("/work/idea")).unwrap(),
    AirDraftAnswer::Refused {
      ide: ide(),
      code: "UNAVAILABLE".to_owned(),
      message: Some("No agent of the IDE can start a session.".to_owned()),
    }
  );

  // the IDE closes the connection without an answer
  let fake = FakeIde::start(Value::Null);
  let error = open_draft(Some(fake.dir()), &params("/work/idea")).unwrap_err();
  assert_eq!(error.code, ErrorCode::Internal, "{error:?}");
}
