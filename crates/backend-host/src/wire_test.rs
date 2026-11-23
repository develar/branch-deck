use crate::wire::{Frame, Incoming, negotiate, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A line is a request, a notification or an answer; no JSON, a batch, no `jsonrpc` and params without a channel are
/// failures of the protocol with the id the line names
#[test]
fn a_line_is_a_request_a_notification_or_an_answer() {
  assert_eq!(
    read(r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{"channel":"ahp-root://"}}"#),
    Ok(Incoming::Request {
      id: json!(1),
      method: "ping".to_owned(),
      params: json!({ "channel": "ahp-root://" })
    })
  );
  assert_eq!(
    read(r#"{"jsonrpc":"2.0","id":"h-1","result":{"window":"review:1"}}"#),
    Ok(Incoming::Response {
      id: json!("h-1"),
      answer: Ok(json!({ "window": "review:1" }))
    })
  );
  let reason = |line: &str| read(line).err().map(|(id, failure)| (id, failure.code, failure.reason));
  assert_eq!(reason("{not json"), Some((Value::Null, -32700, "parse")));
  assert_eq!(reason(r#"[{"jsonrpc":"2.0","id":5,"method":"ping"}]"#), Some((Value::Null, -32600, "invalidRequest")));
  assert_eq!(
    reason(r#"{"id":6,"method":"ping","params":{"channel":"ahp-root://"}}"#),
    Some((json!(6), -32600, "invalidRequest"))
  );
  assert_eq!(reason(r#"{"jsonrpc":"2.0","id":8,"method":"ping","params":{}}"#), Some((json!(8), -32602, "badParams")));
}

/// The host takes the highest offered version of its major
#[test]
fn the_host_takes_the_highest_offered_version_of_its_major() {
  assert_eq!(negotiate(&[json!("1.0.0"), json!("1.2.0"), json!("2.0.0")]).as_deref(), Some("1.2.0"));
  assert_eq!(negotiate(&[json!("2.0.0")]), None);
  assert_eq!(negotiate(&[json!("1.x")]), None);
}

/// The bytes of a raw answer are base64 on a socket
#[test]
fn the_bytes_of_a_raw_answer_are_base64_on_a_socket() {
  let frame = Frame::Bytes {
    id: json!(3),
    bytes: vec![0, 1, 2],
  };
  let text: Value = serde_json::from_str(&frame.into_text()).unwrap();
  assert_eq!(text, json!({ "jsonrpc": "2.0", "id": 3, "result": { "bytes": "AAEC" } }));
}
