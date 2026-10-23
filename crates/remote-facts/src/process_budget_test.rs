//! The git processes of the reads of the remote family that run git: the text of a new pull request of a branch and
//! of the whole stack (AGENTS.md, "Adding a git operation": a test that pins its processes by subcommand).

use crate::pull_request_create::read_new_pull_request;
use crate::stack_pull_request::{GetNewStackPullRequestParams, StackMemberSubjects, read_new_stack_pull_request};
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{alpha_and_beta_on_origin, assert_processes, counting};

const PREFIX: &str = "test";

fn path(repo: &TestRepo) -> String {
  repo.path().to_str().unwrap().to_string()
}

/// The text of the "Create pull request" dialog of a branch without a description: its commits against the baseline,
/// a stacked branch's split from those of the branches it is built on by the decorations of the same `log`
#[test]
fn the_text_of_a_new_pull_request() {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.git(&["branch", "test/virtual/alpha"]);
  repo.git(&["branch", "test/virtual/root", "HEAD~1"]);
  let git = counting();
  let read = read_new_pull_request(&git, &path(&repo), "origin", "test/virtual/alpha", "master", &["test/virtual/root".to_string()]).unwrap();
  assert_eq!((read.description, read.commits.len(), read.ancestors[0].subjects.len()), (None, 1, 5));
  assert_processes(&git, &[("config", 1), ("log", 1)]);
}

/// The text of the pull request of the whole stack the app's form shows: the descriptions of the stack branch and of
/// its members from one `config`, the subjects as the rows show them
#[test]
fn the_text_of_a_new_pull_request_of_the_whole_stack() {
  let (repo, _) = alpha_and_beta_on_origin();
  repo.git(&["config", "branch.test/virtual/beta.description", "IJPL-2 Beta\n\nThe beta part."]);
  let member = |name: &str, parent: Option<&str>, subjects: &[&str]| StackMemberSubjects {
    name: name.to_string(),
    parent: parent.map(str::to_string),
    subjects: subjects.iter().map(|subject| (*subject).to_string()).collect(),
  };
  let params = GetNewStackPullRequestParams {
    repository_path: path(&repo),
    branch_prefix: PREFIX.to_string(),
    root: "alpha".to_string(),
    members: vec![
      member("alpha", None, &["Alpha"]),
      member("beta", Some("alpha"), &["Beta"]),
      member("gamma", Some("alpha"), &["Gamma"]),
    ],
    target: "master".to_string(),
  };
  let git = counting();
  let text = read_new_stack_pull_request(&git, &params).unwrap();
  assert_eq!(text.title, "Alpha, with the 2 branches built on it");
  assert!(text.body.contains("## beta: IJPL-2 Beta"), "{}", text.body);
  assert_processes(&git, &[("config", 1)]);
}
