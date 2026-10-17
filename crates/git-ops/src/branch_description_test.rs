use crate::branch_description::{Description, clear_description, read_descriptions, write_description};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_utils::git_test_utils::TestRepo;

#[test]
fn reads_every_description_by_its_full_ref() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "readme");
  let path = repo.path().to_str().unwrap();
  assert_eq!(read_descriptions(repo.git_executor(), path).unwrap(), HashMap::new());

  let login = Description {
    title: "Add login".to_string(),
    body: "Adds the login form.\n\nWith tests.".to_string(),
  };
  repo.set_config("branch.user/virtual/auth.description", &login.to_text()).unwrap();
  repo.set_config("branch.feature/x.description", "Only a title\n").unwrap();
  assert_eq!(
    read_descriptions(repo.git_executor(), path).unwrap(),
    HashMap::from([
      ("refs/heads/user/virtual/auth".to_string(), login),
      (
        "refs/heads/feature/x".to_string(),
        Description {
          title: "Only a title".to_string(),
          body: String::new(),
        }
      ),
    ])
  );
}

/// A write in git's format reads back the same; a clear removes it, and clearing one that is not set is no error
#[test]
fn writes_and_clears_a_description() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "readme");
  let (git, path) = (repo.git_executor(), repo.path().to_str().unwrap());
  let description = Description {
    title: "Add login".to_string(),
    body: "Gr\u{f6}\u{df}e 🦀".to_string(),
  };
  write_description(git, path, "user/virtual/auth", &description).unwrap();
  assert_eq!(repo.git(&["config", "branch.user/virtual/auth.description"]), "Add login\n\nGr\u{f6}\u{df}e 🦀");
  assert_eq!(
    read_descriptions(git, path).unwrap(),
    HashMap::from([("refs/heads/user/virtual/auth".to_string(), description)])
  );

  clear_description(git, path, "user/virtual/auth").unwrap();
  assert_eq!(read_descriptions(git, path).unwrap(), HashMap::new());
  clear_description(git, path, "user/virtual/auth").unwrap();
}
