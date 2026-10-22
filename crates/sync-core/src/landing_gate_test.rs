//! What the Safe Push of each branch needs that a branch outside it adds (`landing_needs`): the refusals the start's gate
//! would give, by one trial check of every branch

use crate::check::CheckSession;
use crate::landing_gate::{check_landing_gate, landing_needs, landing_needs_of, name_landing_robots};
use crate::process_budget_test::repo_on_origin;
use domain_types::landing::{LandingFix, LandingRefusal, Subject};
use domain_types::repository_view::RobotOf;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashSet};
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test";

/// Stacked branches on `origin/master`: `util` adds `controlUrl`, `flow` uses it, `flow/ui` is stacked on `flow`;
/// returns the repository and the commit of `util`
pub(crate) fn repo() -> (TestRepo, String) {
  let repo = repo_on_origin();
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let declares = repo.create_commit_with_files("(util) Add controlUrl", &[("src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n")]);
  repo.create_commit_with_files(
    "(flow) Start with controlUrl",
    &[("src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n")],
  );
  repo.create_commit_with_files("(flow/ui) Show the start", &[("src/c/Ui.kt", "package c\n\nfun show() = 1\n")]);
  (repo, declares)
}

fn needs_of(repo: &TestRepo, landed: &HashSet<String>) -> BTreeMap<String, Vec<LandingRefusal>> {
  landing_needs_of(&GitCommandExecutor::new(), &ObjectCache::disabled(), repo.path_str(), PREFIX, landed).unwrap()
}

/// The branch the start needs and its fix, of each refusal
fn needed(refusals: &[LandingRefusal]) -> Vec<(&str, &LandingFix)> {
  refusals
    .iter()
    .map(|refusal| match refusal {
      LandingRefusal::MissingDeclaration { declared_by, fix, .. } | LandingRefusal::DanglingReference { fixed_by: declared_by, fix, .. } => (declared_by.branch.as_str(), fix),
    })
    .collect()
}

/// `flow` uses a name `util` adds, and `flow/ui` lands `flow`: both need `util`, the fix a restack of `flow`, the root of
/// what lands, on `util`; `util` needs nothing
#[test]
fn a_branch_that_uses_a_name_of_a_branch_outside_it_needs_that_branch_and_so_does_a_branch_built_on_it() {
  let (repo, _) = repo();
  let needs = needs_of(&repo, &HashSet::new());
  let restack = LandingFix::Restack {
    branch: "flow".to_string(),
    onto: "util".to_string(),
  };
  assert_eq!(needs.keys().collect::<Vec<_>>(), ["flow", "ui"]);
  assert_eq!(needed(&needs["flow"]), [("util", &restack)]);
  assert_eq!(needed(&needs["ui"]), [("util", &restack)]);
  let LandingRefusal::MissingDeclaration { identifier, file, line, .. } = &needs["flow"][0] else {
    panic!("{needs:?}")
  };
  assert_eq!((identifier.as_str(), file.as_str(), *line), ("controlUrl", "src/b/Use.kt", 3));
}

/// What `landing_needs` gives a branch is what the gate of its start refuses, one rule
#[test]
fn the_needs_of_a_branch_are_the_refusals_of_its_start() {
  let (repo, _) = repo();
  let git = GitCommandExecutor::new();
  let cache = ObjectCache::disabled();
  let session = CheckSession::open(&git, repo.path_str(), PREFIX, &cache).unwrap();
  let needs = landing_needs(&session, &HashSet::new()).unwrap();
  for name in ["flow", "ui", "util"] {
    let gate = check_landing_gate(&session, &Subject::Branch { name: name.to_string() }, &HashSet::new()).unwrap();
    assert_eq!(gate.refusals(&session), needs.get(name).cloned().unwrap_or_default(), "{name}");
  }
}

/// A merged Safe Push landed the commit that adds the name: the target has it, nothing is needed
#[test]
fn a_landed_declaration_is_needed_by_nobody() {
  let (repo, declares) = repo();
  assert_eq!(needs_of(&repo, &HashSet::from([declares])), BTreeMap::new());
}

/// While a Safe Push lands `util`, the refusal names it: its robot tests `util` without `flow`, so a start of `flow`
/// beside it fails all the same
#[test]
fn a_running_safe_push_of_the_branch_that_adds_the_name_is_named_in_the_reason() {
  let (repo, _) = repo();
  let mut needs = needs_of(&repo, &HashSet::new());
  let robot = RobotOf {
    branch: "util".to_string(),
    robot_id: "robot-1".to_string(),
    whole_stack: false,
  };
  let refusals = needs.get_mut("flow").unwrap();
  name_landing_robots(refusals, |branch| (branch == "util").then(|| robot.clone()));
  let LandingRefusal::MissingDeclaration { landing_in, .. } = &refusals[0] else {
    panic!("{refusals:?}")
  };
  assert_eq!(landing_in.as_ref(), Some(&robot));
  assert_eq!(
    refusals[0].reason(),
    "\"Start with controlUrl\" of flow uses controlUrl in src/b/Use.kt, which \"Add controlUrl\" of util adds; util is landing in its Safe Push, which tests it without this start"
  );
}
