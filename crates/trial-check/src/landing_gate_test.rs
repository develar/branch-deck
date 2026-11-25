//! What the Safe Push of each branch needs that a branch outside it adds (`landing_needs`): the refusals the start's gate
//! would give, by one trial check of every branch

use crate::check::CheckSession;
use crate::landing_gate::{check_landing_gate, landing_needs, landing_needs_of};
use domain_types::landing::{LandingFix, LandingRefusal, Subject};
use domain_types::repository_view::RobotOf;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashSet};
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::repo_on_origin;

const PREFIX: &str = "test";

/// On `origin/master`: `util` adds `controlUrl`, `flow` uses it, `flow/ui` is stacked on `flow`;
/// returns the repository and the commit of `util`
pub(crate) fn repo() -> (TestRepo, String) {
  let repo = repo_on_origin();
  let declares = repo.create_commit_with_files("(util) Add controlUrl", &[("src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n")]);
  repo.create_commit_with_files(
    "(flow) Start with controlUrl",
    &[("src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n")],
  );
  repo.create_commit_with_files("(flow/ui) Show the start", &[("src/c/Ui.kt", "package c\n\nfun show() = 1\n")]);
  (repo, declares)
}

fn needs_of(repo: &TestRepo, landed: &HashSet<String>) -> BTreeMap<String, Vec<LandingRefusal>> {
  landing_needs_of(&GitCommandExecutor::new(), &ObjectCache::disabled(), repo.path_str(), PREFIX, landed)
    .unwrap()
    .refusals
}

/// The branch the start needs and its fix, of each refusal; the gate builds no declared need, which has neither
fn needed(refusals: &[LandingRefusal]) -> Vec<(&str, &LandingFix)> {
  refusals
    .iter()
    .map(|refusal| {
      let needed = refusal.needed().unwrap_or_else(|| panic!("{refusal:?}"));
      (needed.branch.as_str(), refusal.fix().unwrap_or_else(|| panic!("{refusal:?}")))
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
/// beside it fails all the same. The trial check names no robot (`landing_in` stays `None`): the model names the robot
/// of the branch that adds the name (`domain_policy::landing_needs::name_landing_robots`, tested there), and the
/// reason says it
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
  let LandingRefusal::MissingDeclaration { landing_in, declared_by, .. } = &mut refusals[0] else {
    panic!("{refusals:?}")
  };
  assert_eq!((landing_in.as_ref(), declared_by.branch.as_str()), (None, "util"));
  *landing_in = Some(robot);
  assert_eq!(
    refusals[0].reason(),
    "\"Start with controlUrl\" of flow uses controlUrl in src/b/Use.kt, which \"Add controlUrl\" of util adds; util is landing in its Safe Push, which tests it without this start"
  );
}

/// `keep` drops `fresh`, a use of a property and of an import, then `drop` drops their last use and them: HEAD builds.
/// The branch `drop` is built without `keep`, and its copy keeps `fresh` without the bindings: its Safe Push would fail
/// to compile. The dangling reference of the property names the commit that fixes it; the import, which no line search
/// sees, is a lost binding: the original on HEAD uses neither, so it is what the build of the branch brought
#[test]
fn a_commit_whose_branch_keeps_a_use_of_what_it_removes_refuses_the_start() {
  let repo = repo_on_origin();
  repo.create_commit_with_files("Add Session", &[("src/a/Session.kt", SESSION)]);
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  let without_fresh = SESSION.replace("  fun fresh() = try { backend() } catch (t: Throwable) { rethrow(t) }\n\n", "");
  repo.create_commit_with_files("(keep) Drop fresh", &[("src/a/Session.kt", &without_fresh)]);
  let dropped = without_fresh
    .replace("import b.rethrow\n\n", "")
    .replace("  private val backend: () -> Int,\n", "")
    .replace("  fun old() = try { backend() } catch (t: Throwable) { rethrow(t) }\n\n", "");
  let drop = repo.create_commit_with_files("(drop) Drop old", &[("src/a/Session.kt", &dropped)]);

  let needs = needs_of(&repo, &HashSet::new());
  assert_eq!(needs.keys().collect::<Vec<_>>(), ["drop"]);
  // the property is a dangling reference, with the commit that fixes it; the import only a lost binding
  let refusals: Vec<(&str, &str, u32, &str)> = needs["drop"]
    .iter()
    .filter_map(|refusal| match refusal {
      LandingRefusal::DanglingReference { identifier, file, line, .. } => Some(("dangling", identifier.as_str(), *line, file.as_str())),
      LandingRefusal::LostBinding {
        identifier,
        file,
        line,
        removed_by,
        ..
      } => {
        assert_eq!(removed_by.id, drop);
        Some(("lost", identifier.as_str(), *line, file.as_str()))
      }
      _ => None,
    })
    .collect();
  assert_eq!(refusals, [("dangling", "backend", 7, "src/a/Session.kt"), ("lost", "rethrow", 7, "src/a/Session.kt")]);
  let lost = needs["drop"].iter().find(|refusal| matches!(refusal, LandingRefusal::LostBinding { .. })).unwrap();
  assert_eq!(lost.reason(), "\"Drop old\" of drop removes the import of rethrow, which src/a/Session.kt:7 still uses");
}

const SESSION: &str = "package a

import b.rethrow

class Session(
  private val backend: () -> Int,
) {
  fun old() = try { backend() } catch (t: Throwable) { rethrow(t) }

  fun keep1() = 1

  fun fresh() = try { backend() } catch (t: Throwable) { rethrow(t) }

  fun keep2() = 2
}
";
