use crate::lease_test::{Fixture, path_of};
use bd_cli::fixture_test::{canonical, git};
use bd_core::bd_command::shell_word;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

fn workspace_in(fixture: &Fixture, cwd: &Path) -> Value {
  let (exit_code, output) = fixture.run_in(cwd, &["workspace"]);
  assert_eq!(exit_code, 0, "{output}");
  output
}

fn display(path: &Path) -> String {
  canonical(path).to_string_lossy().into_owned()
}

/// By default an agent leases a worktree from the shared checkout and writes the subject as the repository does; inside
/// the lease it picks each commit into the shared checkout and returns the lease, whose local changes it does not have
#[test_log::test]
fn by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it() {
  let fixture = Fixture::new();
  let shared = display(fixture.repo.path());
  std::fs::create_dir_all(fixture.repo.path().join("crates/x")).unwrap();
  std::fs::write(fixture.repo.path().join("local.txt"), "the user's\n").unwrap();

  let output = workspace_in(&fixture, &fixture.repo.path().join("crates/x"));
  assert_eq!(
    (&output["kind"], &output["lease"], &output["subject_branch"], &output["stacks"]),
    (
      &json!("main"),
      &json!({ "on": true, "source": "default" }),
      &json!({ "on": true, "source": "default" }),
      &json!(false)
    )
  );
  assert_eq!(
    (&output["top_level"], &output["subdir"], &output["shared_checkout"], &output["local_changes"]),
    (&json!(shared), &json!("crates/x"), &json!(shared), &json!(1))
  );
  assert_eq!(output["next"], json!([format!("bd lease get --holder <your name> -C {}", shell_word(&shared))]));
  assert_eq!(output["active_leases"], json!([]));

  let lease = fixture.get(&["--holder", "lane-1"]);
  let slot = path_of(&lease);
  let output = workspace_in(&fixture, Path::new(&slot));
  assert_eq!(
    (&output["kind"], &output["lease"]["on"], &output["shared_checkout"], &output["lease_id"], &output["holder"]),
    (&json!("lease"), &json!(true), &json!(shared), &lease["lease_id"], &json!("lane-1"))
  );
  assert_eq!(output["local_changes"], json!(1), "the shared checkout's, which the lease lacks");
  assert_eq!(output["home"], json!({ "checkout": shared, "branch": "master" }));
  assert_eq!(
    output["next"],
    json!([
      format!("bd -C {} lease pick", shell_word(&slot)),
      format!("bd lease return --lease-id {}", lease["lease_id"].as_str().unwrap()),
    ])
  );
  assert_eq!(
    output["active_leases"],
    json!([{ "lease_id": lease["lease_id"], "holder": "lane-1", "path": slot, "holder_alive": true }])
  );
  // the shared checkout names the lease too: a session that lost its path finds it by its holder
  assert_eq!(workspace_in(&fixture, fixture.repo.path())["active_leases"], output["active_leases"]);
}

/// `branchdeck.agentLease false` and `branchdeck.agentSubjectBranch false` in the global config turn both rules off
/// everywhere, a repository's own value wins
#[test_log::test]
fn the_global_config_turns_a_rule_off_and_a_repository_keeps_its_own() {
  let fixture = Fixture::new();
  std::fs::write(fixture.global_config.path(), "[branchdeck]\n\tagentLease = false\n\tagentSubjectBranch = false\n").unwrap();

  let output = workspace_in(&fixture, fixture.repo.path());
  assert_eq!(
    (&output["lease"], &output["subject_branch"], &output["next"]),
    (&json!({ "on": false, "source": "global" }), &json!({ "on": false, "source": "global" }), &json!([]))
  );

  fixture.repo.set_config("branchdeck.agentLease", "true").unwrap();
  fixture.repo.set_config("branchdeck.agentSubjectBranch", "maybe").unwrap();
  fixture.repo.set_config("branchdeck.stacks", "true").unwrap();
  let output = workspace_in(&fixture, fixture.repo.path());
  assert_eq!(
    (&output["lease"], &output["subject_branch"], &output["stacks"]),
    (
      &json!({ "on": true, "source": "local" }),
      &json!({ "on": true, "source": "local", "invalid": "maybe is not a boolean (true or false)" }),
      &json!(true)
    )
  );
}

/// A linked worktree outside the pool is a checkout of its own: no lease, no pick; outside a working tree there is
/// nothing to edit, and that is an answer, not an error
#[test_log::test]
fn a_worktree_of_its_own_needs_no_lease_and_no_working_tree_is_an_answer() {
  let fixture = Fixture::new();
  let linked = tempfile::tempdir().unwrap();
  let linked_path = linked.path().join("linked");
  git(&fixture.repo, &["worktree", "add", "-q", "--detach", linked_path.to_str().unwrap()]);

  let output = workspace_in(&fixture, &linked_path);
  assert_eq!(
    (&output["kind"], &output["lease"]["on"], &output["next"], output.get("shared_checkout")),
    (&json!("linked"), &json!(false), &json!([]), None)
  );
  assert!(output["lease"]["reason"].as_str().unwrap().starts_with("A worktree of its own"), "{output}");

  let nowhere = tempfile::tempdir().unwrap();
  let output = workspace_in(&fixture, nowhere.path());
  assert_eq!((&output["kind"], &output["lease"]["on"], &output["next"]), (&json!("none"), &json!(false), &json!([])));
}

/// A pool that can't be written, as in a sandbox, turns the lease off with the reason: a get would fail there
#[cfg(unix)]
#[test_log::test]
fn a_pool_that_cant_be_written_turns_the_lease_off() {
  use std::os::unix::fs::PermissionsExt;
  let fixture = Fixture::new();
  let pool = tempfile::tempdir().unwrap();
  std::fs::set_permissions(pool.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
  fixture.repo.set_config("branchdeck.leasePool", pool.path().join("slots").to_str().unwrap()).unwrap();

  let output = workspace_in(&fixture, fixture.repo.path());
  std::fs::set_permissions(pool.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
  assert_eq!((&output["lease"]["on"], &output["next"]), (&json!(false), &json!([])));
  let reason = output["lease"]["reason"].as_str().unwrap();
  assert!(reason.contains(&display(pool.path())) && reason.contains("can't be written"), "{reason}");
}
