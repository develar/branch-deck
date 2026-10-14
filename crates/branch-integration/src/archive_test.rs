use crate::archive::RefNames;

fn names(list: &[&str]) -> RefNames {
  let mut names = RefNames::default();
  for name in list {
    names.insert(name);
  }
  names
}

#[test]
fn a_name_conflicts_with_the_same_name_and_with_names_below_or_above_it() {
  let taken = names(&["auth", "stack/login"]);
  assert!(!taken.is_free("auth"));
  // `auth` is a file, so `auth/x` can't be created
  assert!(!taken.is_free("auth/login"));
  // `stack` is a directory, so a ref `stack` can't be created
  assert!(!taken.is_free("stack"));
  assert!(!taken.is_free("stack/login"));
  assert!(taken.is_free("stack/logout"));
  assert!(taken.is_free("auth-1"));
  assert!(taken.is_free("authx/login"));
}

#[test]
fn suffixes_the_level_that_conflicts() {
  let taken = names(&["auth", "auth-1", "stack/login", "docs/x"]);
  assert_eq!(taken.unique("api", 10).as_deref(), Some("api"));
  assert_eq!(taken.unique("auth", 10).as_deref(), Some("auth-2"));
  // the file `auth` is above it: a suffix at the end would never help
  assert_eq!(taken.unique("auth/login", 10).as_deref(), Some("auth-2/login"));
  // `docs` is a directory
  assert_eq!(taken.unique("docs", 10).as_deref(), Some("docs-1"));
  assert_eq!(taken.unique("stack/login", 10).as_deref(), Some("stack/login-1"));
  assert_eq!(names(&["a", "a-1"]).unique("a", 1), None);
}
