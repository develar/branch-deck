use crate::branch_description::{Description, read_descriptions};
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
