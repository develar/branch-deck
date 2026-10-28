use crate::proto::{DEFAULT_PORT, feature, features};
use pretty_assertions::assert_eq;

#[test]
fn an_agent_on_macos_has_the_upgrade_and_elsewhere_no_feature() {
  if cfg!(target_os = "macos") {
    assert_eq!(features(), [feature::UPGRADE]);
  } else {
    assert_eq!(features(), Vec::<&str>::new());
  }
  // yonderd keeps 7741, so both agents serve one host side by side
  assert_eq!(DEFAULT_PORT, 7742);
}
