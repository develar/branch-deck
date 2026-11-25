use crate::client::{Client, ClientError, MAX_FRAME_BYTES, MAX_REQUEST_BYTES};
use crate::fake_ide_test::{FakeIde, Handshake, NONCE, Step, exchange, initialize, initialized, refusal, request, result};
use crate::protocol::{
  Activity, CreateSessionResult, Host, HostParams, HostsResult, LaunchRequest, ListSessionsParams, ListSessionsResult, Mode, ModeKind, Model, Refusal, RootChange, SessionIdentity,
  SessionSummary, StartKind, Target, TargetsResult,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

fn launch_request(start: StartKind) -> LaunchRequest {
  LaunchRequest {
    project: "/work/app".to_string(),
    launch_target_id: None,
    interaction_surface_id: None,
    model: None,
    effort: None,
    mode: None,
    plan: false,
    plan_effort: None,
    prompt: Some("Fix the build".to_string()),
    title: Some("Fix the Safe Push of auth".to_string()),
    start,
  }
}

fn refused(error: ClientError) -> Refusal {
  match error {
    ClientError::Refused(refusal) => refusal,
    other => panic!("a refusal expected: {other:?}"),
  }
}

#[test]
fn the_local_app_side_of_the_corpus_of_the_ide_replays() {
  let ide = FakeIde::replay("local-ahp-apps.ndjson");
  let client = ide.client();

  // before the holder gives the local apps
  let unauthorized = refused(client.connect().unwrap_err());
  assert_eq!(unauthorized.code, "UNAUTHORIZED");
  assert_eq!(
    unauthorized.hint.as_deref(),
    Some("Run air-ide from a session that the IDE started, with its AIR_CLI_ACCESS.")
  );

  let mut connection = client.connect().unwrap();
  connection.call::<Value>("subscribe", &()).unwrap();
  let hosts: HostsResult = connection.call("x-air-ij/hosts", &()).unwrap();
  assert_eq!(
    hosts,
    HostsResult {
      product: "IU".to_string(),
      build: "271.1".to_string(),
      hosts: vec![Host {
        project_directory: PathBuf::from("/work/app"),
      }],
      focused_at: Some(1_795_593_688_000),
    }
  );
  let rows: ListSessionsResult = connection.call("listSessions", &ListSessionsParams { cursor: None }).unwrap();
  assert_eq!(rows.items.iter().map(|row| row.resource.as_str()).collect::<Vec<_>>(), vec!["air-session:/s1"]);
  assert_eq!(rows.next_cursor, None);

  // the notice that comes before the answer of the targets waits
  let targets: TargetsResult = connection
    .call(
      "x-air-ij/targets",
      &HostParams {
        project_directory: "/work/app/src",
      },
    )
    .unwrap();
  assert_eq!(
    targets.targets,
    vec![
      Target {
        agent_id: "codex".to_string(),
        launch_target_id: "codex".to_string(),
        interaction_surface_id: "native".to_string(),
        name: "Codex".to_string(),
        models: vec![
          Model {
            id: "gpt-sol".to_string(),
            name: "Sol".to_string(),
            qualifier: None,
            is_default: true,
            reasoning_efforts: vec!["low".to_string(), "high".to_string()],
          },
          Model {
            id: "gpt-astra".to_string(),
            name: "Astra".to_string(),
            qualifier: Some("1M".to_string()),
            is_default: false,
            reasoning_efforts: vec!["high".to_string(), "xhigh".to_string()],
          },
        ],
        modes: vec![
          Mode {
            id: "read-only".to_string(),
            name: "Read Only".to_string(),
            kind: ModeKind::Standard,
          },
          Mode {
            id: "plan".to_string(),
            name: "Plan".to_string(),
            kind: ModeKind::Plan,
          },
        ],
        plan: true,
        lists_loading: false,
      },
      Target {
        agent_id: "claude".to_string(),
        launch_target_id: "terminal.claude".to_string(),
        interaction_surface_id: "terminal".to_string(),
        name: "Claude".to_string(),
        models: Vec::new(),
        modes: Vec::new(),
        plan: false,
        lists_loading: true,
      },
    ]
  );

  // no target: the default of the New Session button; a surface without a target and a plan effort without Plan are
  // not sent
  let default = LaunchRequest {
    interaction_surface_id: Some("native".to_string()),
    plan_effort: Some("low".to_string()),
    title: Some("Build fix".to_string()),
    ..launch_request(StartKind::Run)
  };
  let started: CreateSessionResult = connection.call("createSession", &default.params()).unwrap();
  assert_eq!(
    started,
    CreateSessionResult {
      session: SessionIdentity {
        project_directory: "/work/app".to_string(),
        local_session_id: "local-3".to_string(),
      },
      agent_session_id: Some("019a-agent-3".to_string()),
    }
  );

  // the client never sends a surface without a target: the corpus pins the refusal of the IDE
  let bad_params = refused(
    connection
      .call::<Value>(
        "createSession",
        &json!({"projectDirectory": "/work/app", "prompt": "Fix the build", "interactionSurfaceId": "native"}),
      )
      .unwrap_err(),
  );
  assert_eq!(bad_params.code, "BAD_PARAMS");
  assert_eq!(bad_params.message.as_deref(), Some("interactionSurfaceId needs launchTargetId."));

  // a draft on a route with every option of the corpus, and no title
  let draft = LaunchRequest {
    project: "/work/app/src".to_string(),
    launch_target_id: Some("codex".to_string()),
    interaction_surface_id: Some("native".to_string()),
    model: Some("gpt-sol".to_string()),
    effort: Some("high".to_string()),
    plan: true,
    plan_effort: Some("low".to_string()),
    prompt: Some("Review the change".to_string()),
    ..launch_request(StartKind::Draft)
  };
  assert_eq!(connection.call::<Value>("x-air-ij/openDraft", &draft.params()).unwrap(), json!({}));

  let elsewhere = LaunchRequest {
    project: "/elsewhere".to_string(),
    title: None,
    ..launch_request(StartKind::Run)
  };
  let no_project = refused(connection.call::<Value>("createSession", &elsewhere.params()).unwrap_err());
  assert_eq!(no_project.code, "NO_PROJECT");
  assert_eq!(no_project.message.as_deref(), Some("No open project of the IDE holds the directory."));

  // the rows of the notices that came before the answers, in their order: the row of the new session has the local
  // session id of the result
  let mut added = Vec::new();
  while let Some(notice) = connection.next_notification() {
    let notice = notice.unwrap();
    let Some(RootChange::Added(row)) = RootChange::of(&notice).unwrap() else {
      panic!("a row added: {notice:?}");
    };
    added.push((row.resource, row.title));
  }
  // `localAhpApps` null ends the connection
  assert_eq!(
    added,
    vec![
      ("air-session:/s2".to_string(), Some("Second".to_string())),
      ("local-3".to_string(), Some("Build fix".to_string())),
    ]
  );
  drop(connection);
  ide.finish();
}

#[test]
fn the_launch_of_the_wire_starts_a_route_with_every_field_reads_each_refusal_and_watches_the_rows() {
  let ide = FakeIde::replay("local-ahp-launch.ndjson");
  let client = ide.client();

  let named = LaunchRequest {
    launch_target_id: Some("codex".to_string()),
    interaction_surface_id: Some("native".to_string()),
    model: Some("gpt-5.5".to_string()),
    effort: Some("high".to_string()),
    mode: Some("auto".to_string()),
    plan: true,
    plan_effort: Some("low".to_string()),
    ..launch_request(StartKind::Run)
  };
  let started = client.launch(&named).unwrap().unwrap();
  assert_eq!(started.session.local_session_id, "air-session:/s4");
  assert_eq!(started.agent_session_id.as_deref(), Some("019a-codex-thread"));

  let denied = refused(client.launch(&launch_request(StartKind::Run)).unwrap_err());
  assert_eq!(
    denied,
    Refusal {
      code: "DENIED".to_string(),
      message: Some("The user denied the local app.".to_string()),
      hint: Some("Allow Branch Deck in the IDE.".to_string()),
    }
  );
  assert_eq!(refused(client.launch(&launch_request(StartKind::Run)).unwrap_err()).code, "NO_DEFAULT");

  let mut watch = client.watch().unwrap();
  let rows = watch.list_sessions().unwrap();
  assert_eq!(rows.iter().map(|row| row.resource.as_str()).collect::<Vec<_>>(), vec!["air-session:/s3", "air-session:/s1"]);
  assert_eq!(rows.iter().map(SessionSummary::activity).collect::<Vec<_>>(), vec![Activity::Processing, Activity::Ready]);
  // the IDE sets no top-level activity: a row names it in `_meta["x-air-ij"].activity`
  let changes: Vec<RootChange> = watch.map(Result::unwrap).collect();
  let read: Vec<_> = changes
    .iter()
    .map(|change| match change {
      RootChange::Changed { session, changes } => (session.as_str(), changes.title.as_deref(), changes.activity()),
      RootChange::Removed { session } => (session.as_str(), None, None),
      RootChange::Added(row) => panic!("no row added: {row:?}"),
    })
    .collect();
  assert_eq!(
    read,
    vec![
      ("air-session:/s3", None, Some(Activity::NeedsInput)),
      ("air-session:/s1", Some("First, renamed"), None),
      ("air-session:/s3", None, None),
    ]
  );
  assert!(matches!(changes[2], RootChange::Removed { .. }));
  ide.finish();
}

#[test]
fn the_handshake_writes_the_opener_then_the_envelope_with_the_echo_of_the_nonce_and_initialize_without_access() {
  let ide = FakeIde::serve(vec![exchange(
    "x-air-ij/hosts",
    &json!({}),
    result(2, &json!({"product": "IU", "build": "271.1", "hosts": [], "focusedAt": null})),
  )]);
  assert_eq!(ide.client().hosts().unwrap().hosts, Vec::new());
  assert_eq!(
    ide.finish(),
    vec![
      json!({"proto": "local-peer", "v": 2}),
      json!({"nonce": NONCE, "request": initialize()}),
      request(2, "x-air-ij/hosts", &json!({})),
    ]
  );
}

#[test]
fn a_notice_before_the_answer_waits_and_the_answer_of_another_id_and_a_request_of_the_ide_are_skipped() {
  let notice = json!({"jsonrpc": "2.0", "method": "root/sessionRemoved", "params": {"channel": "ahp-root://", "session": "s-1"}});
  let ide = FakeIde::serve(vec![vec![
    Step::Read(initialize()),
    Step::Write(initialized()),
    Step::Read(request(2, "x-air-ij/targets", &json!({"projectDirectory": "/work/app"}))),
    Step::Write(result(99, &json!({"targets": "not this one"}))),
    Step::Write(json!({"jsonrpc": "2.0", "id": 7, "method": "x-air-ide/ask", "params": {}})),
    Step::Write(notice.clone()),
    Step::Write(result(2, &json!({"targets": []}))),
    Step::Close,
  ]]);
  let mut connection = ide.client().connect().unwrap();
  let targets: Value = connection.call("x-air-ij/targets", &json!({"projectDirectory": "/work/app"})).unwrap();
  assert_eq!(targets, json!({"targets": []}));
  let waited = connection.next_notification().unwrap().unwrap();
  assert_eq!((waited.method.as_str(), &waited.params), ("root/sessionRemoved", &notice["params"]));
  assert!(connection.next_notification().is_none());
  drop(connection);
  ide.finish();
}

#[test]
fn an_error_without_a_code_of_its_data_reads_the_code_of_its_number() {
  let error = |code: i64| json!({"jsonrpc": "2.0", "id": 2, "error": {"code": code, "message": "no such method"}});
  let ide = FakeIde::serve(vec![
    exchange("x-air-ij/targets", &json!({"projectDirectory": "/work/app"}), error(-32601)),
    exchange("x-air-ij/targets", &json!({"projectDirectory": "/work/app"}), error(-32602)),
    exchange("x-air-ij/targets", &json!({"projectDirectory": "/work/app"}), error(-32099)),
    exchange("x-air-ij/targets", &json!({"projectDirectory": "/work/app"}), refusal(2, "BUSY", "Too many calls wait")),
  ]);
  let code = || refused(ide.client().targets("/work/app").unwrap_err());
  assert_eq!(
    code(),
    Refusal {
      code: "UNKNOWN_METHOD".to_string(),
      message: Some("no such method".to_string()),
      hint: None,
    }
  );
  assert_eq!(code().code, "BAD_PARAMS");
  assert_eq!(code().code, "ERROR_-32099");
  assert_eq!(code().code, "BUSY");
  ide.finish();
}

/// A connection that gets the result of `initialize`, then reads the request of the targets and plays `then`
fn after_targets(then: Vec<Step>) -> Vec<Step> {
  let mut steps = vec![
    Step::Read(initialize()),
    Step::Write(initialized()),
    Step::Read(request(2, "x-air-ij/targets", &json!({"projectDirectory": "/work/app"}))),
  ];
  steps.extend(then);
  steps
}

#[test]
fn an_ide_that_does_not_answer_in_time_is_a_timeout() {
  let ide = FakeIde::serve(vec![after_targets(Vec::new())]);
  let client = ide.client().with_timeout(Duration::from_millis(200));
  let error = client.targets("/work/app").unwrap_err();
  assert!(matches!(error, ClientError::Timeout(_)), "{error:?}");
  ide.finish();
}

#[test]
fn a_frame_longer_than_the_limit_is_refused() {
  let ide = FakeIde::serve(vec![after_targets(vec![Step::Raw(vec![b'a'; MAX_FRAME_BYTES + 2]), Step::Close])]);
  let error = ide.client().targets("/work/app").unwrap_err();
  assert!(matches!(error, ClientError::ResponseTooLarge), "{error:?}");
  ide.finish();
}

#[test]
fn a_garbled_frame_or_a_result_of_another_shape_is_a_bad_response() {
  let ide = FakeIde::serve(vec![
    after_targets(vec![Step::WriteLine("not json".to_string())]),
    after_targets(vec![Step::Write(result(2, &json!({"later": []})))]),
    after_targets(vec![Step::Write(json!({"jsonrpc": "2.0"}))]),
  ]);
  for _ in 0..3 {
    let error = ide.client().targets("/work/app").unwrap_err();
    assert!(matches!(error, ClientError::BadResponse(_)), "{error:?}");
  }
  ide.finish();
}

#[test]
fn a_close_without_an_answer_is_closed_and_a_frame_without_its_end_is_read_whole() {
  let ide = FakeIde::serve(vec![
    after_targets(vec![Step::Close]),
    after_targets(vec![Step::Raw(result(2, &json!({"targets": []})).to_string().into_bytes()), Step::Close]),
  ]);
  let error = ide.client().targets("/work/app").unwrap_err();
  assert!(matches!(error, ClientError::Closed), "{error:?}");
  assert_eq!(ide.client().targets("/work/app").unwrap(), Vec::new());
  ide.finish();
}

#[test]
fn a_request_longer_than_the_limit_is_not_sent() {
  let ide = FakeIde::serve(vec![vec![Step::Read(initialize()), Step::Write(initialized())]]);
  let request = LaunchRequest {
    prompt: Some("a".repeat(MAX_REQUEST_BYTES)),
    ..launch_request(StartKind::Run)
  };
  let error = ide.client().launch(&request).unwrap_err();
  assert!(matches!(error, ClientError::RequestTooLarge), "{error:?}");
  // the opener and the envelope, no request
  assert_eq!(ide.finish().len(), 2);
}

#[test]
fn a_socket_nobody_listens_on_is_a_connect_error() {
  let dir = tempfile::tempdir().unwrap();
  let error = Client::new(dir.path().join("gone.sock")).targets("/work/app").unwrap_err();
  assert!(matches!(error, ClientError::Connect { .. }), "{error:?}");
}

#[test]
fn a_refusal_of_the_handshake_is_a_typed_error() {
  use crate::protocol::HandshakeRefusal;
  for (handshake, code) in [
    (Handshake::RefuseOpener("BAD_OPENER"), HandshakeRefusal::BadOpener),
    (Handshake::WrongNonce, HandshakeRefusal::BadNonce),
    (Handshake::RefuseEnvelope("PEER_CHANGED"), HandshakeRefusal::PeerChanged),
    (Handshake::RefuseEnvelope("LATER_CODE"), HandshakeRefusal::Other("LATER_CODE".to_string())),
  ] {
    let ide = FakeIde::with_handshake(handshake, Vec::new());
    let error = ide.client().connect().unwrap_err();
    assert!(matches!(&error, ClientError::Handshake(refused) if *refused == code), "{error:?}");
    ide.finish();
  }
}

#[test]
fn a_greeting_of_another_version_without_a_nonce_or_of_no_handshake_is_a_bad_response_and_no_greeting_in_time_is_a_timeout() {
  for greeting in [
    json!({"proto": "local-peer", "v": 3, "nonce": NONCE}),
    json!({"proto": "local-peer", "v": 2}),
    json!({"proto": "local-peer", "v": 2, "nonce": "not base64url!"}),
    json!({"proto": "local-peer", "v": 2, "nonce": "a".repeat(65)}),
    // a line of the application instead of the greeting
    json!({"status": "refused", "code": "UNSUPPORTED_VERSION"}),
  ] {
    let ide = FakeIde::with_handshake(Handshake::Greet(greeting.to_string()), Vec::new());
    let error = ide.client().connect().unwrap_err();
    assert!(matches!(error, ClientError::BadResponse(_)), "{greeting}: {error:?}");
    ide.finish();
  }

  let ide = FakeIde::with_handshake(Handshake::Silent, Vec::new());
  let error = ide.client().with_timeout(Duration::from_millis(200)).connect().unwrap_err();
  assert!(matches!(error, ClientError::Timeout(_)), "{error:?}");
  assert_eq!(ide.finish(), vec![json!({"proto": "local-peer", "v": 2})]);
}

/// The steps of a watch: `initialize`, the root `subscribe` and one page of `listSessions` without rows
fn watched(then: Vec<Step>) -> Vec<Step> {
  let mut steps = vec![
    Step::Read(initialize()),
    Step::Write(initialized()),
    Step::Read(request(2, "subscribe", &json!({}))),
    Step::Write(result(2, &json!({"snapshot": {"resource": "ahp-root://", "state": {"agents": []}, "fromSeq": 0}}))),
    Step::Read(request(3, "listSessions", &json!({}))),
    Step::Write(result(3, &json!({"items": []}))),
  ];
  steps.extend(then);
  steps
}

fn removed(session: &str) -> Value {
  json!({"jsonrpc": "2.0", "method": "root/sessionRemoved", "params": {"channel": "ahp-root://", "session": session}})
}

#[test]
fn a_watch_waits_without_a_timeout_skips_a_notice_of_another_method_and_its_stopper_ends_it_from_another_thread() {
  // each notice comes later than the timeout of an answer
  let ide = FakeIde::serve(vec![watched(vec![
    Step::Pause(Duration::from_millis(300)),
    Step::Write(json!({"jsonrpc": "2.0", "method": "root/later", "params": {"channel": "ahp-root://"}})),
    Step::Write(removed("s-1")),
    Step::Pause(Duration::from_millis(300)),
    Step::Write(removed("s-2")),
  ])]);
  let mut watch = ide.client().with_timeout(Duration::from_millis(250)).watch().unwrap();
  assert_eq!(watch.list_sessions().unwrap(), Vec::new());
  assert_eq!(watch.next().unwrap().unwrap(), RootChange::Removed { session: "s-1".to_string() });
  assert_eq!(watch.next().unwrap().unwrap(), RootChange::Removed { session: "s-2".to_string() });
  let stopper = watch.stopper().unwrap();
  let reader = std::thread::spawn(move || watch.next().map(|change| change.map_err(|error| error.to_string())));
  stopper.stop();
  assert_eq!(reader.join().unwrap(), None);
  ide.finish();
}

#[test]
fn a_garbled_notice_of_a_watch_is_a_bad_response_and_ends_it() {
  let ide = FakeIde::serve(vec![watched(vec![
    Step::Write(json!({"jsonrpc": "2.0", "method": "root/sessionRemoved", "params": {"channel": "ahp-root://"}})),
    Step::Write(removed("s-1")),
  ])]);
  let mut watch = ide.client().watch().unwrap();
  watch.list_sessions().unwrap();
  let error = watch.next().unwrap().unwrap_err();
  assert!(matches!(error, ClientError::BadResponse(_)), "{error:?}");
  assert!(watch.next().is_none());
  drop(watch);
  ide.finish();
}

#[test]
fn a_refusal_of_the_list_of_a_watch_reads_its_code() {
  let ide = FakeIde::serve(vec![vec![
    Step::Read(initialize()),
    Step::Write(initialized()),
    Step::Read(request(2, "subscribe", &json!({}))),
    Step::Write(result(2, &json!({"snapshot": {}}))),
    Step::Read(request(3, "listSessions", &json!({}))),
    Step::Write(refusal(3, "UNAVAILABLE", "The IDE cannot ask")),
  ]]);
  let mut watch = ide.client().watch().unwrap();
  assert_eq!(refused(watch.list_sessions().unwrap_err()).code, "UNAVAILABLE");
  drop(watch);
  ide.finish();
}
