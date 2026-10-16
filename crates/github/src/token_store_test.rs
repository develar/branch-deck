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

#[test]
fn a_token_is_written_for_the_user_only_and_read_back() {
  let dir = tempfile::tempdir().unwrap();
  let path = token_file(&dir.path().join("org.develar.branch-deck"));
  assert_eq!(read_token(&path), None);
  write_token(&path, &stored()).unwrap();
  assert_eq!(read_token(&path), Some(stored()));
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
  }
  // no temporary file is left
  assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
  remove_token(&path).unwrap();
  remove_token(&path).unwrap();
  assert_eq!(read_token(&path), None);
}

#[cfg(unix)]
#[test]
fn a_token_file_others_may_read_is_not_used() {
  use std::os::unix::fs::PermissionsExt;
  let dir = tempfile::tempdir().unwrap();
  let path = token_file(dir.path());
  write_token(&path, &stored()).unwrap();
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
  assert_eq!(read_token(&path), None);
}

#[test]
fn the_debug_form_hides_the_token() {
  assert!(!format!("{:?}", stored()).contains("gho_secret"));
}
