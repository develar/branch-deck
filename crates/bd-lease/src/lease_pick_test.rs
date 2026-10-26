use crate::lease_test::{Fixture, path_of, slot_git};
use bd_cli::fixture_test::{PREFIX, assert_error, canonical, git};
use bd_core::bd_command::shell_word;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;

fn commit_in(path: &str, subject: &str, file: &str) {
  std::fs::write(Path::new(path).join(file), format!("{subject}\n")).unwrap();
  slot_git(path, &["add", file]);
  slot_git(path, &["commit", "-q", "-m", subject]);
}

fn display(path: &Path) -> String {
  canonical(path).to_string_lossy().into_owned()
}

/// `bd lease pick` in the slot at `path`
fn lease_pick(fixture: &Fixture, path: &str, args: &[&str]) -> (i32, Value) {
  fixture.run_in(Path::new(path), &[&["--prefix", PREFIX, "lease", "pick"][..], args].concat())
}

/// The get names where the commits of the lease land and what the session runs, before its first edit
#[test_log::test]
fn the_get_names_the_home_of_the_lease_and_the_commands_of_the_session() {
  let fixture = Fixture::new();
  let got = fixture.get(&["--holder", "lane-1"]);
  let path = path_of(&got);
  assert_eq!(got["home"], json!({ "checkout": display(fixture.repo.path()), "branch": "master" }));
  assert_eq!(
    got["next"],
    json!([
      format!("bd -C {} lease pick", shell_word(&path)),
      format!("bd lease return --lease-id {}", got["lease_id"].as_str().unwrap()),
    ])
  );
  // a source on a detached HEAD names no branch to land on: no pick in `next`
  fixture.give_back(&path);
  git(&fixture.repo, &["checkout", "-q", "--detach"]);
  let got = fixture.get(&[]);
  assert_eq!(got["home"], json!({ "checkout": display(fixture.repo.path()), "branch": null }));
  assert_eq!(got["next"], json!([format!("bd lease return --lease-id {}", got["lease_id"].as_str().unwrap())]));
  let (exit_code, output) = lease_pick(&fixture, &path_of(&got), &[]);
  assert_error(&output, exit_code, (4, "no_home_branch"));
}

/// The commits of the lease land on the branch it was cut from, onto a tip that moved since, and the lease follows; a
/// pick with nothing to land only moves the lease onto the tip of the home branch
#[test_log::test]
fn a_lease_pick_lands_the_commits_on_the_home_branch_and_the_lease_follows() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  fixture.repo.create_commit("(peer) Add p", "p.txt", "p\n");

  let (exit_code, dry) = lease_pick(&fixture, &path, &["--dry-run"]);
  assert_eq!((exit_code, &dry["state"], &dry["followed"]["state"]), (0, &json!("picked"), &json!("would_move")), "{dry}");
  let before = fixture.repo.head();
  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!(
    (exit_code, &output["state"], &output["branch"], &output["followed"]["state"]),
    (0, &json!("picked"), &json!("master"), &json!("moved")),
    "{output}"
  );
  assert_eq!(git(&fixture.repo, &["log", "-1", "--format=%s"]), "(lane) Add a");
  assert_eq!(git(&fixture.repo, &["rev-parse", "HEAD^"]), before, "on top of the peer's commit");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD"]), fixture.repo.head(), "the lease follows");

  // another session lands a commit: the lease only follows
  let peer = fixture.repo.create_commit("(peer) Add q", "q.txt", "q\n");
  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!(
    (exit_code, &output["state"], &output["followed"]["state"]),
    (0, &json!("up_to_date"), &json!("moved")),
    "{output}"
  );
  assert_eq!((fixture.repo.head(), slot_git(&path, &["rev-parse", "HEAD"])), (peer.clone(), peer));
  fixture.give_back(&path);
}

/// The home checkout switched to another branch since the get: the pick lands nothing and names it, and `bd workspace`
/// in the lease shows the switch; once the branch is back the pick lands
#[test_log::test]
fn a_lease_pick_refuses_a_home_that_switched_its_branch() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  git(&fixture.repo, &["switch", "-q", "-c", "other"]);
  let other = fixture.repo.head();

  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_error(&output, exit_code, (4, "home_branch_switched"));
  assert!(output["error"]["message"].as_str().unwrap().contains("other"), "{output}");
  assert_eq!(fixture.repo.head(), other, "nothing landed");
  let (exit_code, workspace) = fixture.run_in(Path::new(&path), &["workspace"]);
  assert_eq!(
    (exit_code, &workspace["home"]["branch"], &workspace["home"]["switched_to"]),
    (0, &json!("master"), &json!("other")),
    "{workspace}"
  );

  git(&fixture.repo, &["switch", "-q", "master"]);
  let (exit_code, workspace) = fixture.run_in(Path::new(&path), &["workspace"]);
  assert_eq!((exit_code, workspace["home"].get("switched_to")), (0, None), "{workspace}");
  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!((exit_code, &output["state"]), (0, &json!("picked")), "{output}");
}

/// A conflict lands nothing and names `bd lease pick` as the command to run again
#[test_log::test]
fn a_lease_pick_that_conflicts_changes_nothing_and_names_itself_again() {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Change b", "b.txt");
  let head = fixture.repo.create_commit("(peer) Change b", "b.txt", "peer\n");

  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!((exit_code, &output["state"]), (1, &json!("conflict")), "{output}");
  assert_eq!(fixture.repo.head(), head);
  let next = output["conflict"]["next"].as_str().unwrap();
  assert!(next.contains("lease pick` again") && next.contains(&shell_word(&path)), "{next}");
}

/// Outside a leased slot there is no home: the shared checkout itself refuses
#[test_log::test]
fn a_lease_pick_outside_a_lease_is_refused() {
  let fixture = Fixture::new();
  let (exit_code, output) = fixture.run(&["--prefix", PREFIX, "lease", "pick"]);
  assert_error(&output, exit_code, (2, "no_lease"));
}
