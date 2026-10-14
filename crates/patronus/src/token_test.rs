use crate::token::{TOKEN_FILE, TokenLookup, TokenStamp, find_token};
use std::path::Path;

const NOW: u32 = 1_791_970_200; // 2026-10-14T09:30:00Z

fn write_token_file(home: &Path, content: &str) {
  let path = home.join(TOKEN_FILE);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, content).unwrap();
}

#[test]
fn reads_the_token_of_the_safe_push_cli() {
  let home = tempfile::tempdir().unwrap();
  // the CLI writes Go's time.Time: local offset and nanoseconds
  write_token_file(
    home.path(),
    r#"{"name":"PatronusSafePushCli-mac-1a2b","token":"secret","expires":"2026-10-21T11:30:00.123456789+02:00"}"#,
  );
  let TokenLookup::Found(token) = find_token(Some(home.path()), Some("env".to_string()), NOW) else {
    panic!("the file token is found first");
  };
  assert_eq!(token.value, "secret");
  assert!(matches!(token.stamp, TokenStamp::File(Some(_))));
}

#[test]
fn never_sends_an_expired_token() {
  let home = tempfile::tempdir().unwrap();
  write_token_file(home.path(), r#"{"name":"n","token":"secret","expires":"2026-10-14T09:29:59Z"}"#);
  assert!(matches!(find_token(Some(home.path()), None, NOW), TokenLookup::Expired(TokenStamp::File(_))));
}

#[test]
fn falls_back_to_the_environment_variable() {
  let home = tempfile::tempdir().unwrap();
  assert_eq!(find_token(Some(home.path()), None, NOW), TokenLookup::Missing);
  assert_eq!(find_token(Some(home.path()), Some("  ".to_string()), NOW), TokenLookup::Missing);
  let TokenLookup::Found(token) = find_token(Some(home.path()), Some(" env-token\n".to_string()), NOW) else {
    panic!("the variable is used without the file");
  };
  assert_eq!((token.value.as_str(), token.stamp), ("env-token", TokenStamp::Env));

  write_token_file(home.path(), "not json");
  assert!(matches!(find_token(Some(home.path()), Some("env-token".to_string()), NOW), TokenLookup::Found(_)));
}
