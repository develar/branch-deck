use crate::client::{Client, ClientError, MAX_FRAME_BYTES, MAX_REQUEST_BYTES};
use crate::fake_ide_test::{FakeIde, Handshake, NONCE, Step, exchange, initialize, initialized, refusal, request, result};
use crate::protocol::{DraftParams, Host, HostsResult, Refusal};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

fn refused(error: ClientError) -> Refusal {
  match error {
    ClientError::Refused(refusal) => refusal,
    other => panic!("a refusal expected: {other:?}"),
  }
}

/// The params of the draft the tests send
fn draft() -> Value {
  json!({"projectDirectory": "/work/app", "prompt": "Fix it"})
}

#[test]
fn the_local_app_side_of_the_corpus_of_the_ide_replays() {
  // the frames of `initialize`, the hosts and a draft with the project and the prompt alone
  let ide = FakeIde::replay_only("local-ahp-apps.ndjson", &[("x-air-ij/hosts", &[]), ("x-air-ij/openDraft", &["projectDirectory", "prompt"])]);
  let client = ide.client();

  // before the holder gives the local apps
  let unauthorized = refused(client.connect().unwrap_err());
  assert_eq!(unauthorized.code, "UNAUTHORIZED");
  assert_eq!(
    unauthorized.hint.as_deref(),
    Some("Run air-ide from a session that the IDE started, with its AIR_CLI_ACCESS.")
  );

  let mut connection = client.connect().unwrap();
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
  let draft = |project_directory, prompt| DraftParams { project_directory, prompt };
  assert_eq!(
    connection.call::<Value>("x-air-ij/openDraft", &draft("/work/app/src", "Review the change")).unwrap(),
    json!({})
  );
  // the client sends the prompt as it is: the IDE drops its control characters
  assert_eq!(
    connection.call::<Value>("x-air-ij/openDraft", &draft("/work/app", "Fix\u{1b}[201~ the\r\nbuild")).unwrap(),
    json!({})
  );
  let no_project = refused(connection.call::<Value>("x-air-ij/openDraft", &draft("/elsewhere", "Review the change")).unwrap_err());
  assert_eq!(no_project.code, "NO_PROJECT");
  assert_eq!(no_project.message.as_deref(), Some("No open project of the IDE holds the directory."));
  drop(connection);
  ide.finish();
}

#[test]
fn a_draft_names_only_the_project_and_the_prompt_and_reads_each_refusal() {
  let ide = FakeIde::replay("local-ahp-draft.ndjson");
  let client = ide.client();
  client.open_draft("/work/app", "Fix the build").unwrap();
  assert_eq!(
    refused(client.open_draft("/work/app", "Fix the build").unwrap_err()),
    Refusal {
      code: "DENIED".to_string(),
      message: Some("The user denied the local app.".to_string()),
      hint: Some("Allow Branch Deck in the IDE.".to_string()),
    }
  );
  assert_eq!(refused(client.open_draft("/work/app", "Fix the build").unwrap_err()).code, "UNAVAILABLE");
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
fn a_notice_the_answer_of_another_id_and_a_request_of_the_ide_before_the_answer_are_skipped() {
  let ide = FakeIde::serve(vec![vec![
    Step::Read(initialize()),
    Step::Write(initialized()),
    Step::Read(request(2, "x-air-ij/openDraft", &draft())),
    Step::Write(result(99, &json!({"not": "this one"}))),
    Step::Write(json!({"jsonrpc": "2.0", "id": 7, "method": "x-air-ide/ask", "params": {}})),
    Step::Write(json!({"jsonrpc": "2.0", "method": "root/sessionRemoved", "params": {"channel": "ahp-root://", "session": "s-1"}})),
    Step::Write(result(2, &json!({}))),
    Step::Close,
  ]]);
  ide.client().open_draft("/work/app", "Fix it").unwrap();
  ide.finish();
}

#[test]
fn an_error_without_a_code_of_its_data_reads_the_code_of_its_number() {
  let error = |code: i64| json!({"jsonrpc": "2.0", "id": 2, "error": {"code": code, "message": "no such method"}});
  let ide = FakeIde::serve(vec![
    exchange("x-air-ij/openDraft", &draft(), error(-32601)),
    exchange("x-air-ij/openDraft", &draft(), error(-32602)),
    exchange("x-air-ij/openDraft", &draft(), error(-32099)),
    exchange("x-air-ij/openDraft", &draft(), refusal(2, "BUSY", "Too many calls wait")),
  ]);
  let code = || refused(ide.client().open_draft("/work/app", "Fix it").unwrap_err());
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

/// A connection that gets the result of `initialize`, then reads the request of the hosts and plays `then`
fn after_hosts(then: Vec<Step>) -> Vec<Step> {
  let mut steps = vec![Step::Read(initialize()), Step::Write(initialized()), Step::Read(request(2, "x-air-ij/hosts", &json!({})))];
  steps.extend(then);
  steps
}

#[test]
fn an_ide_that_does_not_answer_in_time_is_a_timeout() {
  let ide = FakeIde::serve(vec![after_hosts(Vec::new())]);
  let client = ide.client().with_timeout(Duration::from_millis(200));
  let error = client.hosts().unwrap_err();
  assert!(matches!(error, ClientError::Timeout(_)), "{error:?}");
  ide.finish();
}

#[test]
fn a_frame_longer_than_the_limit_is_refused() {
  let ide = FakeIde::serve(vec![after_hosts(vec![Step::Raw(vec![b'a'; MAX_FRAME_BYTES + 2]), Step::Close])]);
  let error = ide.client().hosts().unwrap_err();
  assert!(matches!(error, ClientError::ResponseTooLarge), "{error:?}");
  ide.finish();
}

#[test]
fn a_garbled_frame_or_a_result_of_another_shape_is_a_bad_response() {
  let ide = FakeIde::serve(vec![
    after_hosts(vec![Step::WriteLine("not json".to_string())]),
    after_hosts(vec![Step::Write(result(2, &json!({"later": []})))]),
    after_hosts(vec![Step::Write(json!({"jsonrpc": "2.0"}))]),
  ]);
  for _ in 0..3 {
    let error = ide.client().hosts().unwrap_err();
    assert!(matches!(error, ClientError::BadResponse(_)), "{error:?}");
  }
  ide.finish();
}

#[test]
fn a_close_without_an_answer_is_closed_and_a_frame_without_its_end_is_read_whole() {
  let ide = FakeIde::serve(vec![
    after_hosts(vec![Step::Close]),
    after_hosts(vec![Step::Raw(result(2, &json!({"hosts": []})).to_string().into_bytes()), Step::Close]),
  ]);
  let error = ide.client().hosts().unwrap_err();
  assert!(matches!(error, ClientError::Closed), "{error:?}");
  assert_eq!(ide.client().hosts().unwrap().hosts, Vec::new());
  ide.finish();
}

#[test]
fn a_request_longer_than_the_limit_is_not_sent() {
  let ide = FakeIde::serve(vec![vec![Step::Read(initialize()), Step::Write(initialized())]]);
  let error = ide.client().open_draft("/work/app", &"a".repeat(MAX_REQUEST_BYTES)).unwrap_err();
  assert!(matches!(error, ClientError::RequestTooLarge), "{error:?}");
  // the opener and the envelope, no request
  assert_eq!(ide.finish().len(), 2);
}

#[test]
fn a_socket_nobody_listens_on_is_a_connect_error() {
  let dir = tempfile::tempdir().unwrap();
  let error = Client::new(dir.path().join("gone.sock")).hosts().unwrap_err();
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
