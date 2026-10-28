use crate::host_ref_name::is_host_ref_name;

#[test]
fn a_host_ref_name_is_one_safe_component() {
  for name in ["run-1", "1792-81.a_b", "client", &"a".repeat(128)] {
    assert!(is_host_ref_name(name), "{name}");
  }
  for name in ["", "-run", ".run", "a..b", "a/b", "a.lock", "a.LOCK", "a b", "a:b", &"a".repeat(129)] {
    assert!(!is_host_ref_name(name), "{name}");
  }
}
