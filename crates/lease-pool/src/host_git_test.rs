use crate::host_git::{floor_text, meets_floor};

#[test]
fn a_version_meets_the_floor_from_its_release_on() {
  assert_eq!(floor_text(), "2.56.0");
  for version in ["2.56.0", "2.56.1", "2.57.0-rc1", "3.0.0", "2.56.0.windows.1"] {
    assert!(meets_floor(version), "{version}");
  }
  for version in ["2.55.9", "2.56.0-rc2", "2.56.0.rc1", "2.50.1 (Apple Git-155)", "garbage"] {
    assert!(!meets_floor(version), "{version}");
  }
}
