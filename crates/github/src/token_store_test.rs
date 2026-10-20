use crate::token_store::{StoredToken, read_token, remove_token, token_file, write_token};
use pretty_assertions::assert_eq;

fn stored() -> StoredToken {
  StoredToken {
    token: "gho_secret".to_string(),
    login: "octocat".to_string(),
    scopes: "read:org,repo".to_string(),
    created_at: 1_800_000_000,
  }
}

// the 0600 file itself: `sync_utils::private_file_test`
#[test]
fn a_token_is_written_and_read_back() {
  let dir = tempfile::tempdir().unwrap();
  let path = token_file(&dir.path().join("org.develar.branch-deck"));
  assert_eq!(read_token(&path), None);
  write_token(&path, &stored()).unwrap();
  assert_eq!(read_token(&path), Some(stored()));
  remove_token(&path).unwrap();
  assert_eq!(read_token(&path), None);
}

#[test]
fn the_debug_form_hides_the_token() {
  assert!(!format!("{:?}", stored()).contains("gho_secret"));
}
