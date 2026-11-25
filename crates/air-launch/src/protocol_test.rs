use crate::protocol::{
  Activity, ENVELOPE_FRAME_BYTES, Frame, HandshakeLine, HandshakeRefusal, LaunchRequest, Notification, OPENER_LINE, Refusal, RpcError, SessionSummary, StartKind, SummaryChanges,
  Target, envelope_line, handshake_line, parse_frame, request,
};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn every_refusal_code_of_handshake_v2_reads_back_as_its_text() {
  for (code, refusal) in [
    ("BAD_OPENER", HandshakeRefusal::BadOpener),
    ("BAD_NONCE", HandshakeRefusal::BadNonce),
    ("PEER_CHANGED", HandshakeRefusal::PeerChanged),
  ] {
    assert_eq!(HandshakeRefusal::from(code.to_string()), refusal);
    assert_eq!(refusal.as_str(), code);
  }
  assert_eq!(HandshakeRefusal::from("LATER".to_string()), HandshakeRefusal::Other("LATER".to_string()));
}

#[test]
fn a_line_of_local_peer_is_a_greeting_or_a_refusal_and_any_other_line_is_the_applications() {
  let line = |value: serde_json::Value| handshake_line(value.to_string().as_bytes());
  assert_eq!(
    line(json!({"proto": "local-peer", "v": 2, "nonce": "q1Zx-Ab_9kTn0sLw4eRt2Y", "later": 1})),
    HandshakeLine::Greeting {
      nonce: "q1Zx-Ab_9kTn0sLw4eRt2Y".to_string()
    }
  );
  assert_eq!(
    line(json!({"proto": "local-peer", "v": 2, "error": "PEER_CHANGED"})),
    HandshakeLine::Refused(HandshakeRefusal::PeerChanged)
  );
  for greeting in [
    json!({"proto": "local-peer", "v": 1, "nonce": "abc"}),
    json!({"proto": "local-peer", "nonce": "abc"}),
    json!({"proto": "local-peer", "v": 2, "nonce": ""}),
    json!({"proto": "local-peer", "v": 2, "nonce": "abc="}),
    json!({"proto": "local-peer", "v": 2, "nonce": 7}),
  ] {
    assert!(matches!(line(greeting.clone()), HandshakeLine::BadGreeting(_)), "{greeting}");
  }
  assert_eq!(line(json!({"jsonrpc": "2.0", "id": 1, "result": {}})), HandshakeLine::Application);
  assert_eq!(line(json!({"proto": "other", "error": "X"})), HandshakeLine::Application);
  assert_eq!(handshake_line(b"not json"), HandshakeLine::Application);
}

#[test]
fn the_envelope_holds_the_echo_of_the_nonce_and_the_request_object_on_one_line() {
  let request = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientId":"branch-\"deck\""}}"#;
  let line = envelope_line("q1Zx-Ab_9kTn0sLw4eRt2Y", request);
  assert!(line.len() <= request.len() + ENVELOPE_FRAME_BYTES);
  let (last, body) = line.split_last().unwrap();
  assert_eq!(*last, b'\n');
  assert!(!body.contains(&b'\n'));
  assert_eq!(
    serde_json::from_slice::<serde_json::Value>(body).unwrap(),
    json!({"nonce": "q1Zx-Ab_9kTn0sLw4eRt2Y", "request": {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientId": "branch-\"deck\""}}})
  );
  assert_eq!(OPENER_LINE, format!("{}\n", json!({"proto": "local-peer", "v": 2})).as_bytes());
}

#[test]
fn a_target_without_lists_loading_has_its_lists() {
  let target = |extra: serde_json::Value| {
    let mut value = json!({"agentId": "codex", "launchTargetId": "codex", "interactionSurfaceId": "native", "name": "Codex"});
    value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value::<Target>(value).unwrap().lists_loading
  };
  assert!(!target(json!({})));
  assert!(!target(json!({"listsLoading": false})));
  assert!(target(json!({"listsLoading": true})));
}

#[test]
fn a_request_names_the_channel_of_the_root_and_its_params_must_be_an_object() {
  assert_eq!(
    request(4, "listSessions", &()).unwrap(),
    json!({"jsonrpc": "2.0", "id": 4, "method": "listSessions", "params": {"channel": "ahp-root://"}})
  );
  assert_eq!(
    request(5, "x-air-ij/targets", &json!({"projectDirectory": "/work/app"})).unwrap(),
    json!({"jsonrpc": "2.0", "id": 5, "method": "x-air-ij/targets", "params": {"channel": "ahp-root://", "projectDirectory": "/work/app"}})
  );
  request(6, "x", &json!([1])).unwrap_err();
}

#[test]
fn a_frame_is_an_answer_a_notice_or_a_request_of_the_ide() {
  let frame = |value: serde_json::Value| parse_frame(value.to_string().as_bytes());
  assert_eq!(
    frame(json!({"jsonrpc": "2.0", "id": 2, "result": {"a": 1}})).unwrap(),
    Frame::Response {
      id: json!(2),
      outcome: Ok(json!({"a": 1}))
    }
  );
  let Frame::Response { outcome: Err(error), .. } = frame(json!({"jsonrpc": "2.0", "id": 2, "error": {"code": -32603, "message": "m"}})).unwrap() else {
    panic!("an error expected");
  };
  assert_eq!(error.code, -32603);
  assert_eq!(
    frame(json!({"jsonrpc": "2.0", "method": "root/sessionRemoved", "params": {"session": "s"}})).unwrap(),
    Frame::Notification(Notification {
      method: "root/sessionRemoved".to_owned(),
      params: json!({"session": "s"}),
    })
  );
  assert_eq!(
    frame(json!({"jsonrpc": "2.0", "id": 9, "method": "x-air-ide/ask", "params": {}})).unwrap(),
    Frame::Request {
      method: "x-air-ide/ask".to_owned()
    }
  );
  frame(json!({"jsonrpc": "2.0"})).unwrap_err();
  parse_frame(b"not json").unwrap_err();
}

#[test]
fn a_refusal_reads_the_code_message_and_hint_of_the_data_else_the_code_of_the_number() {
  let error = |value: serde_json::Value| serde_json::from_value::<RpcError>(value).unwrap().refusal();
  assert_eq!(
    error(json!({"code": -32010, "message": "outer", "data": {"reason": "denied", "code": "DENIED", "message": "inner", "hint": "Allow it", "details": {}}})),
    Refusal {
      code: "DENIED".to_owned(),
      message: Some("inner".to_owned()),
      hint: Some("Allow it".to_owned()),
    }
  );
  assert_eq!(
    error(json!({"code": -32601, "message": "no method"})),
    Refusal {
      code: "UNKNOWN_METHOD".to_owned(),
      message: Some("no method".to_owned()),
      hint: None,
    }
  );
  assert_eq!(error(json!({"code": -32600, "message": ""})).message, None);
  assert_eq!(error(json!({"code": -32602, "message": "m", "data": {"reason": "badParams"}})).code, "BAD_PARAMS");
  assert_eq!(error(json!({"code": 7, "message": "m"})).code, "ERROR_7");
}

#[test]
fn the_activity_of_a_row_is_read_in_any_case_with_or_without_underscores() {
  for (name, activity) in [
    ("needsInput", Activity::NeedsInput),
    ("NEEDS_INPUT", Activity::NeedsInput),
    ("needs_input", Activity::NeedsInput),
    ("processing", Activity::Processing),
    ("READY", Activity::Ready),
    ("reviewing", Activity::Reviewing),
    ("unread", Activity::Unread),
    ("DRAFT", Activity::Draft),
    ("later", Activity::Other),
  ] {
    assert_eq!(Activity::of(Some(&json!(name))), activity, "{name}");
  }
  assert_eq!(Activity::of(None), Activity::Other);
  assert_eq!(Activity::of(Some(&json!({"kind": "ready"}))), Activity::Other);
}

#[test]
fn a_row_names_its_activity_in_the_facts_of_air_of_its_meta() {
  let row = |value: serde_json::Value| serde_json::from_value::<SessionSummary>(value).unwrap().activity();
  let facts = |activity: &str| json!({"x-air-ij": {"projectDirectory": "/work/app", "threadId": "t-1", "launchTargetId": "codex", "updatedAt": 1, "activity": activity, "archivable": false}});
  assert_eq!(
    row(json!({"resource": "s1", "title": "t", "activity": "ready", "_meta": facts("NEEDS_INPUT")})),
    Activity::NeedsInput
  );
  // the top-level activity is not the one of Air
  assert_eq!(row(json!({"resource": "s1", "title": "t", "activity": "ready"})), Activity::Other);
  let changes = |value: serde_json::Value| serde_json::from_value::<SummaryChanges>(value).unwrap().activity();
  assert_eq!(changes(json!({"_meta": facts("UNREAD")})), Some(Activity::Unread));
  assert_eq!(changes(json!({"_meta": {}})), Some(Activity::Other));
  assert_eq!(changes(json!({"title": "t", "modifiedAt": "2026-11-18T10:00:00.000Z"})), None);
}

#[test]
fn the_params_of_a_launch_send_an_option_only_when_set_and_a_surface_only_with_a_target() {
  let request = LaunchRequest {
    project: "/work/app".to_owned(),
    launch_target_id: None,
    interaction_surface_id: Some("native".to_owned()),
    model: None,
    effort: None,
    mode: None,
    plan: false,
    plan_effort: Some("high".to_owned()),
    prompt: None,
    title: Some("t".to_owned()),
    start: StartKind::Draft,
  };
  assert_eq!(serde_json::to_value(request.params()).unwrap(), json!({"projectDirectory": "/work/app", "prompt": ""}));
  let run = LaunchRequest {
    launch_target_id: Some("codex".to_owned()),
    plan: true,
    prompt: Some("Fix it".to_owned()),
    start: StartKind::Run,
    ..request
  };
  assert_eq!(
    serde_json::to_value(run.params()).unwrap(),
    json!({"projectDirectory": "/work/app", "prompt": "Fix it", "launchTargetId": "codex", "interactionSurfaceId": "native", "plan": true, "planEffort": "high", "title": "t"})
  );
}
