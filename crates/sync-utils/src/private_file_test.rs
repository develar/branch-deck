use crate::private_file::{read_private, remove_private, write_private};

#[test]
fn a_file_is_written_for_the_user_only_and_read_back() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("org.develar.branch-deck").join("token.json");
  assert_eq!(read_private(&path, "the token file"), None);
  write_private(&path, b"first").unwrap();
  write_private(&path, b"second").unwrap();
  assert_eq!(read_private(&path, "the token file").as_deref(), Some("second"));
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
  }
  // no temporary file is left
  assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
  remove_private(&path).unwrap();
  remove_private(&path).unwrap();
  assert_eq!(read_private(&path, "the token file"), None);
}

#[cfg(unix)]
#[test]
fn a_file_others_may_read_is_not_used() {
  use std::os::unix::fs::PermissionsExt;
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("token.json");
  write_private(&path, b"secret").unwrap();
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
  assert_eq!(read_private(&path, "the token file"), None);
}
