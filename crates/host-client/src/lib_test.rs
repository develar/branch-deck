use crate::{affinity, request_exit, shell_quote, short};
use host_link::refusal::{Code, Exit, Refusal};
use pretty_assertions::assert_eq;
use std::path::Path;

/// One affinity per checkout: the same path keeps its slot, and another path gets its own
#[test]
fn each_checkout_has_its_own_stable_affinity() {
  let first = affinity(Path::new("/pool/1/idea-1"));
  assert_eq!(first, affinity(Path::new("/pool/1/idea-1")));
  assert_ne!(first, affinity(Path::new("/pool/2/idea-1")));
  assert!(first.starts_with("checkout-"), "{first}");
  assert_eq!(first.len(), "checkout-".len() + 12);
}

/// A request that is no run exits 2 for `usage`, 130 for `interrupted`, else 255
#[test]
fn a_refusal_of_a_request_exits_by_its_code() {
  let exit = |code| request_exit(&Refusal::new(code, "x"));
  assert_eq!(
    [exit(Code::Usage), exit(Code::Interrupted), exit(Code::Unreachable), exit(Code::Config)],
    [Exit::Usage, Exit::Interrupted, Exit::Refused, Exit::Refused]
  );
}

#[test]
fn a_quoted_value_keeps_its_own_quotes_and_an_id_is_short() {
  assert_eq!(shell_quote("it's"), r"'it'\''s'");
  assert_eq!((short("0123456789abcdef"), short("ab")), ("0123456789ab", "ab"));
}
