use crate::client::{Client, ClientError, MAX_LINE_BYTES};
use crate::protocol::{Activity, Launch, LaunchRequest, Mode, ModeKind, Model, Refusal, RefusalCode, Session, StartKind, Target, Targets, WatchLine};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(windows)]
use uds_windows::UnixListener;

/// The fake socket of an IDE: it takes one connection, reads its request line and answers as `reply` says
struct FakeIde {
  _dir: TempDir,
  socket: PathBuf,
  request: mpsc::Receiver<Value>,
  server: JoinHandle<()>,
}

enum Reply {
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

impl FakeIde {
  fn start(reply: Reply) -> Self {
    let dir = tempfile::Builder::new().prefix("air").tempdir().unwrap();
    let socket = dir.path().join("launch.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (sender, request) = mpsc::channel();
    let server = std::thread::spawn(move || {
      let (stream, _) = listener.accept().unwrap();
      let mut reader = BufReader::new(stream);
      let mut line = String::new();
      reader.read_line(&mut line).unwrap();
      sender.send(serde_json::from_str(&line).unwrap()).unwrap();
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
    Self {
      _dir: dir,
      socket,
      request,
      server,
    }
  }

  fn client(&self) -> Client {
    Client::new(&self.socket)
  }

  /// The request line the server read, once it answered
  fn finish(self) -> Value {
    self.server.join().unwrap();
    self.request.recv().unwrap()
  }
}

fn mode(id: &str, name: &str, kind: ModeKind) -> Mode {
  Mode {
    id: id.to_string(),
    name: name.to_string(),
    kind,
  }
}

fn launch_request(start: StartKind) -> LaunchRequest {
  LaunchRequest {
    project: "/work/idea".to_string(),
    launch_target_id: "codex".to_string(),
    interaction_surface_id: "native".to_string(),
    model: Some("gpt-5.5".to_string()),
    effort: Some("high".to_string()),
    mode: Some("auto".to_string()),
    plan: true,
    plan_effort: Some("xhigh".to_string()),
    prompt: Some("Fix the failing test".to_string()),
    title: Some("Fix the Safe Push of auth".to_string()),
    start,
  }
}

#[test]
fn the_targets_round_trip_names_the_project_and_reads_the_agents_with_their_models_and_modes() {
  let ide = FakeIde::start(Reply::Line(
    json!({"status": "ok", "targets": [
      {"agentId": "codex", "launchTargetId": "codex", "interactionSurfaceId": "native", "name": "Codex",
       "models": [{"id": "gpt-5.5", "name": "GPT-5.5", "qualifier": null, "isDefault": true, "reasoningEfforts": ["low", "high"]}],
       "modes": [{"id": "plan", "name": "Plan", "kind": "plan"}, {"id": "auto", "name": "Auto", "kind": "auto_review"},
                 {"id": "yolo", "name": "Full Access", "kind": "full_access"}, {"id": "later", "name": "Later", "kind": "later_kind"}],
       "plan": true},
      {"agentId": "claude", "launchTargetId": "claude-acp", "interactionSurfaceId": "acp", "name": "Claude", "models": [], "later": 1},
      {"agentId": "claude", "launchTargetId": "terminal.claude", "interactionSurfaceId": "terminal", "name": "Claude in Terminal"}
    ]})
    .to_string(),
  ));
  let targets = ide.client().targets("/work/idea").unwrap();
  assert_eq!(
    targets,
    Targets::Listed {
      targets: vec![
        Target {
          agent_id: "codex".to_string(),
          launch_target_id: "codex".to_string(),
          interaction_surface_id: "native".to_string(),
          name: "Codex".to_string(),
          models: vec![Model {
            id: "gpt-5.5".to_string(),
            name: "GPT-5.5".to_string(),
            qualifier: None,
            is_default: true,
            reasoning_efforts: vec!["low".to_string(), "high".to_string()],
          }],
          modes: vec![
            mode("plan", "Plan", ModeKind::Plan),
            mode("auto", "Auto", ModeKind::AutoReview),
            mode("yolo", "Full Access", ModeKind::FullAccess),
            mode("later", "Later", ModeKind::Other),
          ],
          plan: true,
        },
        Target {
          agent_id: "claude".to_string(),
          launch_target_id: "claude-acp".to_string(),
          interaction_surface_id: "acp".to_string(),
          name: "Claude".to_string(),
          models: Vec::new(),
          modes: Vec::new(),
          plan: false,
        },
        Target {
          agent_id: "claude".to_string(),
          launch_target_id: "terminal.claude".to_string(),
          interaction_surface_id: "terminal".to_string(),
          name: "Claude in Terminal".to_string(),
          models: Vec::new(),
          modes: Vec::new(),
          plan: false,
        },
      ],
    }
  );
  assert_eq!(ide.finish(), json!({"v": 1, "method": "targets", "project": "/work/idea"}));
}

#[test]
fn an_accepted_launch_sends_every_field_and_reads_the_session_id() {
  let ide = FakeIde::start(Reply::Line(r#"{"status":"accepted","sessionId":"s-1"}"#.to_string()));
  let launch = ide.client().launch(&launch_request(StartKind::Run)).unwrap();
  assert_eq!(
    launch,
    Launch::Accepted {
      session_id: Some("s-1".to_string())
    }
  );
  assert_eq!(
    ide.finish(),
    json!({"v": 1, "method": "launch", "project": "/work/idea", "launchTargetId": "codex", "interactionSurfaceId": "native",
      "model": "gpt-5.5", "effort": "high", "mode": "auto", "plan": true, "planEffort": "xhigh",
      "prompt": "Fix the failing test", "title": "Fix the Safe Push of auth", "start": "run"})
  );
}

#[test]
fn a_draft_without_a_model_an_effort_a_mode_plan_and_a_prompt_leaves_them_out() {
  let ide = FakeIde::start(Reply::Line(r#"{"status":"accepted","sessionId":null}"#.to_string()));
  let request = LaunchRequest {
    model: None,
    effort: None,
    mode: None,
    plan: false,
    plan_effort: None,
    prompt: None,
    title: None,
    ..launch_request(StartKind::Draft)
  };
  assert_eq!(ide.client().launch(&request).unwrap(), Launch::Accepted { session_id: None });
  assert_eq!(
    ide.finish(),
    json!({"v": 1, "method": "launch", "project": "/work/idea", "launchTargetId": "codex", "interactionSurfaceId": "native", "start": "draft"})
  );
}

#[test]
fn a_refusal_of_a_launch_or_of_the_targets_reads_its_code_and_message() {
  let ide = FakeIde::start(Reply::Line(r#"{"status":"refused","code":"MODEL_UNAVAILABLE","message":"no model gpt-9"}"#.to_string()));
  assert_eq!(
    ide.client().launch(&launch_request(StartKind::Run)).unwrap(),
    Launch::Refused(Refusal {
      code: RefusalCode::ModelUnavailable,
      message: Some("no model gpt-9".to_string()),
    })
  );
  ide.finish();

  let ide = FakeIde::start(Reply::Line(r#"{"status":"refused","code":"DENIED"}"#.to_string()));
  assert_eq!(
    ide.client().targets("/work/idea").unwrap(),
    Targets::Refused(Refusal {
      code: RefusalCode::Denied,
      message: None,
    })
  );
  ide.finish();
}

#[test]
fn a_refusal_code_of_a_later_protocol_stays_as_its_text() {
  let ide = FakeIde::start(Reply::Line(r#"{"status":"refused","code":"QUOTA_EXCEEDED","message":"later"}"#.to_string()));
  let Launch::Refused(refusal) = ide.client().launch(&launch_request(StartKind::Run)).unwrap() else {
    panic!("a refusal expected");
  };
  assert_eq!(refusal.code, RefusalCode::Other("QUOTA_EXCEEDED".to_string()));
  assert_eq!(refusal.code.as_str(), "QUOTA_EXCEEDED");
  ide.finish();
}

#[test]
fn an_ide_that_does_not_answer_in_time_is_a_timeout() {
  let ide = FakeIde::start(Reply::Never);
  let client = ide.client().with_timeout(Duration::from_millis(200));
  let error = client.targets("/work/idea").unwrap_err();
  assert!(matches!(error, ClientError::Timeout(_)), "{error:?}");
  drop(client);
  ide.finish();
}

#[test]
fn a_response_longer_than_the_limit_is_refused() {
  let ide = FakeIde::start(Reply::Raw(vec![b'a'; MAX_LINE_BYTES + 2]));
  let error = ide.client().targets("/work/idea").unwrap_err();
  assert!(matches!(error, ClientError::ResponseTooLarge), "{error:?}");
  ide.finish();
}

#[test]
fn a_garbled_response_or_an_unknown_status_is_a_bad_response() {
  let ide = FakeIde::start(Reply::Line("not json".to_string()));
  let error = ide.client().targets("/work/idea").unwrap_err();
  assert!(matches!(error, ClientError::BadResponse(_)), "{error:?}");
  ide.finish();

  let ide = FakeIde::start(Reply::Line(r#"{"status":"later"}"#.to_string()));
  let error = ide.client().launch(&launch_request(StartKind::Run)).unwrap_err();
  assert!(matches!(error, ClientError::BadResponse(_)), "{error:?}");
  ide.finish();
}

#[test]
fn a_close_without_an_answer_is_closed_and_a_line_without_its_end_is_read_whole() {
  let ide = FakeIde::start(Reply::Raw(Vec::new()));
  let error = ide.client().targets("/work/idea").unwrap_err();
  assert!(matches!(error, ClientError::Closed), "{error:?}");
  ide.finish();

  let ide = FakeIde::start(Reply::Raw(br#"{"status":"ok","targets":[]}"#.to_vec()));
  assert_eq!(ide.client().targets("/work/idea").unwrap(), Targets::Listed { targets: Vec::new() });
  ide.finish();
}

#[test]
fn a_request_longer_than_the_limit_is_not_sent() {
  let dir = tempfile::tempdir().unwrap();
  let request = LaunchRequest {
    prompt: Some("a".repeat(MAX_LINE_BYTES)),
    ..launch_request(StartKind::Run)
  };
  // no server: the client refuses before it connects
  let error = Client::new(dir.path().join("none.sock")).launch(&request).unwrap_err();
  assert!(matches!(error, ClientError::RequestTooLarge), "{error:?}");
}

#[test]
fn a_socket_nobody_listens_on_is_a_connect_error() {
  let dir = tempfile::tempdir().unwrap();
  let error = Client::new(dir.path().join("gone.sock")).targets("/work/idea").unwrap_err();
  assert!(matches!(error, ClientError::Connect { .. }), "{error:?}");
}

fn session(activity: Activity, title: Option<&str>, updated_at: Option<u64>) -> WatchLine {
  WatchLine::Session {
    session: Session {
      session_id: "s-1".to_string(),
      title: title.map(str::to_string),
      activity,
      updated_at,
    },
  }
}

/// The line of the status `activity` of the session `s-1`
fn status_line(activity: &str) -> String {
  json!({"status": "session", "session": {"sessionId": "s-1", "activity": activity}}).to_string()
}

/// The lines a watch of `s-1` reads from an IDE that answers `reply`, and the request it sent
fn watched(reply: Reply) -> (Vec<WatchLine>, Value) {
  let ide = FakeIde::start(reply);
  let lines = ide.client().watch("/work/idea", "s-1").unwrap().map(Result::unwrap).collect();
  (lines, ide.finish())
}

#[test]
fn a_watch_names_the_session_and_reads_each_status_until_the_end() {
  let (lines, request) = watched(Reply::Lines(vec![
    json!({"status": "session", "session": {"sessionId": "s-1", "title": "Fix it", "activity": "processing", "updatedAt": 1_000}}).to_string(),
    json!({"status": "session", "session": {"sessionId": "s-1", "title": null, "activity": "needs_input", "updatedAt": null}}).to_string(),
    json!({"status": "session", "session": {"sessionId": "s-1", "activity": "later_activity", "later": 1}}).to_string(),
    json!({"status": "end", "reason": "timeout"}).to_string(),
    // nothing is read after the end
    status_line("ready"),
  ]));
  assert_eq!(
    lines,
    vec![
      session(Activity::Processing, Some("Fix it"), Some(1_000)),
      session(Activity::NeedsInput, None, None),
      session(Activity::Other, None, None),
      WatchLine::End { reason: "timeout".to_string() },
    ]
  );
  assert_eq!(request, json!({"v": 1, "method": "watch", "project": "/work/idea", "sessionId": "s-1"}));
}

#[test]
fn a_watch_of_a_session_that_is_gone_ends_with_no_session_and_a_close_ends_it_without_a_line() {
  let refusal = |code: RefusalCode, message: Option<&str>| {
    WatchLine::Refused(Refusal {
      code,
      message: message.map(str::to_string),
    })
  };
  let (lines, _) = watched(Reply::Lines(vec![
    status_line("unread"),
    json!({"status": "refused", "code": "NO_SESSION", "message": "gone"}).to_string(),
  ]));
  assert_eq!(lines, vec![session(Activity::Unread, None, None), refusal(RefusalCode::NoSession, Some("gone"))]);

  let (lines, _) = watched(Reply::Lines(vec![status_line("ready")]));
  assert_eq!(lines, vec![session(Activity::Ready, None, None)]);

  let (lines, _) = watched(Reply::Line(r#"{"status":"refused","code":"BUSY"}"#.to_string()));
  assert_eq!(lines, vec![refusal(RefusalCode::Busy, None)]);
}

#[test]
fn a_watch_waits_without_a_timeout_and_its_stopper_ends_it_from_another_thread() {
  // each line comes later than the timeout of a response
  let ide = FakeIde::start(Reply::Paced(vec![status_line("processing"), status_line("needs_input")], Duration::from_millis(20)));
  let mut watch = ide.client().with_timeout(Duration::from_millis(1)).watch("/work/idea", "s-1").unwrap();
  assert_eq!(watch.next().unwrap().unwrap(), session(Activity::Processing, None, None));
  assert_eq!(watch.next().unwrap().unwrap(), session(Activity::NeedsInput, None, None));
  let stopper = watch.stopper().unwrap();
  let reader = std::thread::spawn(move || watch.next().map(|line| line.map_err(|error| error.to_string())));
  stopper.stop();
  assert_eq!(reader.join().unwrap(), None);
  ide.finish();
}

#[test]
fn a_garbled_line_of_a_watch_is_a_bad_response_and_ends_it() {
  let ide = FakeIde::start(Reply::Lines(vec!["not json".to_string(), r#"{"status":"end","reason":"timeout"}"#.to_string()]));
  let mut watch = ide.client().watch("/work/idea", "s-1").unwrap();
  let error = watch.next().unwrap().unwrap_err();
  assert!(matches!(error, ClientError::BadResponse(_)), "{error:?}");
  assert!(watch.next().is_none());
  drop(watch);
  ide.finish();
}
