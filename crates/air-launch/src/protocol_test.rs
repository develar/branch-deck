use crate::protocol::RefusalCode;
use pretty_assertions::assert_eq;

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
  ];
  for code in codes {
    let parsed = RefusalCode::from(code.to_string());
    assert!(!matches!(parsed, RefusalCode::Other(_)), "{code} is a code of protocol v1");
    assert_eq!(parsed.as_str(), code);
  }
  assert_eq!(RefusalCode::from("denied".to_string()), RefusalCode::Other("denied".to_string()));
}
