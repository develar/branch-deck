use crate::stack::{leaf_of, stack_of, stack_path};

fn names(list: &[&str]) -> Vec<String> {
  list.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn stack_prefixes_split_into_sanitized_levels() {
  assert_eq!(stack_of("auth/login"), Ok(("login".to_string(), Some("auth".to_string()))));
  assert_eq!(stack_of("login"), Ok(("login".to_string(), None)));
  assert_eq!(stack_path("auth/login/oauth"), Ok(names(&["auth", "login", "oauth"])));
  assert_eq!(stack_path(" my auth / log in "), Ok(names(&["my-auth", "log-in"])));
  // empty levels at the ends are dropped
  assert_eq!(stack_of("auth/"), Ok(("auth".to_string(), None)));
  assert_eq!(stack_of("/login"), Ok(("login".to_string(), None)));
  assert_eq!(leaf_of("auth/login"), Some("login".to_string()));
}

#[test]
fn an_empty_stack_level_is_a_syntax_error() {
  let error = stack_of("a//b").unwrap_err();
  assert_eq!(error.leaf.as_deref(), Some("b"));
  assert_eq!(error.to_string(), "(a//b) has an empty stack level");
  // a level that sanitizes to nothing is empty too
  assert_eq!(stack_path("a/~/b").unwrap_err().leaf.as_deref(), Some("b"));
  assert_eq!(stack_path("/").unwrap_err().leaf, None);
  assert_eq!(leaf_of("a//b"), None);
}
