use crate::protocol::{ENVELOPE_FRAME_BYTES, HandshakeLine, HandshakeRefusal, OPENER_LINE, RefusalCode, Target, envelope_line, handshake_line};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn every_refusal_code_of_protocol_v1_reads_back_as_its_text() {
  let codes = [
    "DENIED",
    "UNAVAILABLE",
    "BUSY",
    "RATE_LIMITED",
    "UNSUPPORTED_VERSION",
    "UNKNOWN_METHOD",
    "BAD_REQUEST",
    "NO_PROJECT",
    "UNTRUSTED_PROJECT",
    "TARGET_UNAVAILABLE",
    "MODEL_UNAVAILABLE",
    "MODEL_AMBIGUOUS",
    "INVALID_REASONING_EFFORT",
    "EMPTY_PROMPT",
    "LAUNCH_FAILED",
    "MODE_UNAVAILABLE",
    "MODE_CONFLICT",
    "PLAN_UNAVAILABLE",
    "INVALID_PLAN_REASONING_EFFORT",
    "NO_SESSION",
  ];
  for code in codes {
    let parsed = RefusalCode::from(code.to_string());
    assert!(!matches!(parsed, RefusalCode::Other(_)), "{code} is a code of protocol v1");
    assert_eq!(parsed.as_str(), code);
  }
  assert_eq!(RefusalCode::from("denied".to_string()), RefusalCode::Other("denied".to_string()));
}

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
  assert_eq!(line(json!({"status": "refused", "code": "UNSUPPORTED_VERSION"})), HandshakeLine::Application);
  assert_eq!(line(json!({"proto": "other", "error": "X"})), HandshakeLine::Application);
  assert_eq!(handshake_line(b"not json"), HandshakeLine::Application);
}

#[test]
fn the_envelope_holds_the_echo_of_the_nonce_and_the_request_object_on_one_line() {
  let request = br#"{"v":1,"method":"targets","project":"/work/\"idea\""}"#;
  let line = envelope_line("q1Zx-Ab_9kTn0sLw4eRt2Y", request);
  assert!(line.len() <= request.len() + ENVELOPE_FRAME_BYTES);
  let (last, body) = line.split_last().unwrap();
  assert_eq!(*last, b'\n');
  assert!(!body.contains(&b'\n'));
  assert_eq!(
    serde_json::from_slice::<serde_json::Value>(body).unwrap(),
    json!({"nonce": "q1Zx-Ab_9kTn0sLw4eRt2Y", "request": {"v": 1, "method": "targets", "project": "/work/\"idea\""}})
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
