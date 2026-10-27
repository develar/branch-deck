use crate::lease_test::{Fixture, path_of, slot_git};
use bd_cli::fixture_test::{PREFIX, assert_covered, assert_error, canonical, git};
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

/// The conflicting copy of `path` in the open round of `update` (the `update` of `bd lease pick`, or a `bd update`)
fn edit_of(update: &Value, path: &str) -> String {
  update["session"]["files"]
    .as_array()
    .unwrap()
    .iter()
    .find(|file| file["path"] == json!(path))
    .and_then(|file| file["edit"].as_str())
    .unwrap_or_else(|| panic!("{path} in {update}"))
    .to_string()
}

/// A lease whose commit conflicts with what another session landed on master since the get, and the lease
fn conflicting_lease() -> (Fixture, String) {
  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Change b", "b.txt");
  fixture.repo.create_commit("(peer) Change b", "b.txt", "peer\n");
  (fixture, path)
}

/// A conflict lands nothing and comes back to the lease: the lease is updated onto the tip of master, the update opens
/// a resolve session there, and the lease stays where it was until `bd update --continue` applies it; then `bd lease
/// pick` lands the resolved commit. A return and a pick while the session is open are refused
#[test_log::test]
fn a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it() {
  let (fixture, path) = conflicting_lease();
  let (home, lease) = (fixture.repo.head(), slot_git(&path, &["rev-parse", "HEAD"]));

  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!(
    (exit_code, &output["state"], &output["update"]["state"]),
    (1, &json!("conflict"), &json!("conflict")),
    "{output}"
  );
  assert_eq!((fixture.repo.head(), slot_git(&path, &["rev-parse", "HEAD"])), (home.clone(), lease), "nothing moved");
  assert_eq!(output["update"]["upstream"], json!("master"));
  let next = output["conflict"]["next"].as_str().unwrap();
  assert!(next.contains("update --continue") && next.contains("lease pick` again"), "{next}");

  let (exit_code, refused) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&refused, exit_code, (4, "resolve_in_progress"));
  let (exit_code, refused) = lease_pick(&fixture, &path, &[]);
  assert_error(&refused, exit_code, (4, "resolve_in_progress"));
  let (_, schema) = fixture.run(&["schema", "--all"]);
  let outputs = &schema["outputs"];
  assert_covered(&outputs["lease_pick"], &outputs["lease_pick"], &output, "lease_pick");

  std::fs::write(edit_of(&output["update"], "b.txt"), "peer and lane\n").unwrap();
  let (exit_code, done) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "update", "--continue", "--round", "1"]);
  assert_eq!((exit_code, &done["state"]), (0, &json!("rebase")), "{done}");
  // the update names the lease by its top level, which has forward slashes on Windows
  let repository = done["repository"].as_str().unwrap();
  assert_eq!(display(Path::new(repository)), display(Path::new(&path)));
  assert_eq!(done["next"], json!([format!("bd -C {} lease pick", shell_word(repository))]), "{done}");
  assert_eq!(slot_git(&path, &["rev-parse", "HEAD~1"]), home, "the lease is on the tip of master");

  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!((exit_code, &output["state"], output.get("update")), (0, &json!("picked"), None), "{output}");
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("b.txt")).unwrap(), "peer and lane\n");
  fixture.give_back(&path);
}

/// In a lease `bd update` onto the baseline is refused, it would move the lease past its home; a dry run and the
/// steps of a resolve session go on, and `bd update --abort` closes the session that a pick opened
#[test_log::test]
fn an_update_in_a_lease_is_refused_and_its_session_is_closed_with_abort() {
  let (fixture, path) = conflicting_lease();
  let (exit_code, output) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "update"]);
  assert_error(&output, exit_code, (4, "lease_follows_home"));
  assert!(output["error"]["message"].as_str().unwrap().contains("lease pick"), "{output}");
  // a dry run reads only: not the lease refuses it, the check of the baseline does, as on any detached HEAD
  let (exit_code, output) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "update", "--dry-run"]);
  assert_error(&output, exit_code, (4, "detached_head"));

  let (exit_code, _) = lease_pick(&fixture, &path, &[]);
  assert_eq!(exit_code, 1);
  let (exit_code, status) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "update", "--status"]);
  assert_eq!(exit_code, 0, "{status}");
  let (exit_code, aborted) = fixture.run_in(Path::new(&path), &["--prefix", PREFIX, "update", "--abort"]);
  assert_eq!((exit_code, &aborted["branch"]), (0, &json!("HEAD")), "{aborted}");
  // the session is gone: the return refuses only the commit that did not land
  let (exit_code, refused) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&refused, exit_code, (4, "unlanded_commits"));
}

/// Outside a leased slot there is no home: the shared checkout itself refuses
#[test_log::test]
fn a_lease_pick_outside_a_lease_is_refused() {
  let fixture = Fixture::new();
  let (exit_code, output) = fixture.run(&["--prefix", PREFIX, "lease", "pick"]);
  assert_error(&output, exit_code, (2, "no_lease"));
}

/// A fixture whose home checkout commits `pick_requires_check = true` before the get, so the lease has it too
fn gated_fixture() -> Fixture {
  let fixture = Fixture::new();
  fixture.repo.create_commit("Require a check", ".branchdeck.toml", "[lease]\npick_requires_check = true\n");
  fixture
}

/// The tree of the lease's HEAD
fn tree_of(path: &str) -> String {
  slot_git(path, &["rev-parse", "HEAD^{tree}"])
}

/// Appends a line to `bd-checks.jsonl` of the lease, as a check tool writes it
fn record(path: &str, tree: &str, exit: i32) {
  let file = slot_git(path, &["rev-parse", "--path-format=absolute", "--git-path", "bd-checks.jsonl"]);
  let line = format!(
    r#"{{"schema":1,"tree":"{tree}","head":"{}","tool":"bt","command":"build","patterns":["//plugins/air/..."],"exit":{exit},"at":1793108792000}}"#,
    slot_git(path, &["rev-parse", "HEAD"])
  );
  let mut text = std::fs::read_to_string(&file).unwrap_or_default();
  text.push_str(&line);
  text.push('\n');
  std::fs::write(file, text).unwrap();
}

/// The message of the `unchecked_tree` refusal of `bd lease pick` in the lease at `path`
fn unchecked_message(fixture: &Fixture, path: &str) -> String {
  let (exit_code, output) = lease_pick(fixture, path, &[]);
  assert_error(&output, exit_code, (4, "unchecked_tree"));
  output["error"]["message"].as_str().unwrap().to_string()
}

/// `bd lease pick` that must exit with `exit_code`, and the given field of its `check`
fn checked_pick(fixture: &Fixture, path: &str, args: &[&str], exit_code: i32, field: &str) -> (Value, Value) {
  let (exit, output) = lease_pick(fixture, path, args);
  assert_eq!(exit, exit_code, "{output}");
  let field = output["check"][field].clone();
  (output, field)
}

/// With `pick_requires_check`, a tree that no green build or test recorded lands nothing (`unchecked_tree`): no record,
/// a red one, or a green one of an older tree, which the refusal names with the files changed since; --dry-run reports
/// the check and never refuses; a green record of the tree lands it
#[test_log::test]
fn a_lease_pick_refuses_a_tree_that_no_green_check_recorded() {
  let fixture = gated_fixture();
  let path = path_of(&fixture.get(&[]));
  let before = tree_of(&path);
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  let tree = tree_of(&path);
  let home = fixture.repo.head();

  let message = unchecked_message(&fixture, &path);
  assert!(message.contains("no green build or test ran") && message.contains("--unchecked '<reason>'"), "{message}");
  assert!(!message.contains("The last check"), "{message}");
  let (dry, state) = checked_pick(&fixture, &path, &["--dry-run"], 0, "state");
  assert_eq!((&dry["state"], state), (&json!("picked"), json!("missing")), "{dry}");

  record(&path, &tree, 3);
  record(&path, &before, 0);
  let message = unchecked_message(&fixture, &path);
  assert!(
    message.contains("The last check ran on tree")
      && message.contains("(`bt build //plugins/air/...`), and 1 file changed since, which no check saw as they are in HEAD: lane-a.txt."),
    "{message}"
  );
  let (dry, last) = checked_pick(&fixture, &path, &["--dry-run"], 0, "record");
  assert_eq!(
    (&dry["check"]["state"], &last["changed_files"], &last["changed_paths"]),
    (&json!("stale"), &json!(1), &json!(["lane-a.txt"])),
    "{dry}"
  );
  assert_eq!(fixture.repo.head(), home, "nothing landed");

  record(&path, &tree, 0);
  let (dry, checked) = checked_pick(&fixture, &path, &["--dry-run"], 0, "tree");
  assert_eq!((&dry["check"]["state"], checked), (&json!("ok"), json!(tree)), "{dry}");
  let (output, state) = checked_pick(&fixture, &path, &[], 0, "state");
  assert_eq!((&output["state"], state), (&json!("picked"), json!("ok")), "{output}");
  let (_, schema) = fixture.run(&["schema", "--all"]);
  assert_covered(&schema["outputs"]["lease_pick"], &schema["outputs"]["lease_pick"], &dry, "lease_pick");
  fixture.give_back(&path);
}

/// `--unchecked` lands a tree without a check and the output repeats the reason; a blank reason is refused. Without
/// `pick_requires_check` the pick reads no record and prints no `check`
#[test_log::test]
fn unchecked_lands_without_a_check_and_says_why() {
  let fixture = gated_fixture();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  let (exit_code, output) = lease_pick(&fixture, &path, &["--unchecked", " "]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = lease_pick(&fixture, &path, &["--unchecked", "docs only"]);
  assert_eq!(
    (exit_code, &output["state"], &output["unchecked"], output.get("check")),
    (0, &json!("picked"), &json!("docs only"), None),
    "{output}"
  );
  fixture.give_back(&path);

  let fixture = Fixture::new();
  let path = path_of(&fixture.get(&[]));
  commit_in(&path, "(lane) Add a", "lane-a.txt");
  let (exit_code, output) = lease_pick(&fixture, &path, &[]);
  assert_eq!((exit_code, &output["state"], output.get("check")), (0, &json!("picked"), None), "{output}");
}

/// A conflict whose update applies at once moves the lease onto a new tree: with `pick_requires_check` the pick does
/// not land it before a build, the conflict says so, and once a green record names the new tree the pick lands it
#[test_log::test]
fn an_update_that_applies_after_a_conflict_waits_for_a_check_of_the_new_tree() {
  let fixture = gated_fixture();
  fixture.repo.create_commit("Add words", "w.txt", "alpha beta gamma delta\n");
  let path = path_of(&fixture.get(&[]));
  std::fs::write(Path::new(&path).join("w.txt"), "ALPHA beta gamma delta\n").unwrap();
  slot_git(&path, &["commit", "-q", "-a", "-m", "(lane) Change alpha"]);
  fixture.repo.create_commit("(peer) Change delta", "w.txt", "alpha beta gamma DELTA\n");
  record(&path, &tree_of(&path), 0);

  let (output, state) = checked_pick(&fixture, &path, &[], 1, "state");
  assert_eq!(
    (&output["state"], &output["update"]["state"], state),
    (&json!("conflict"), &json!("rebase"), json!("stale")),
    "{output}"
  );
  let next = output["conflict"]["next"].as_str().unwrap();
  assert!(next.contains("no build or test ran on") && next.contains("lease pick` again"), "{next}");
  assert_eq!(std::fs::read_to_string(Path::new(&path).join("w.txt")).unwrap(), "ALPHA beta gamma DELTA\n");

  record(&path, &tree_of(&path), 0);
  let (output, state) = checked_pick(&fixture, &path, &[], 0, "state");
  assert_eq!((&output["state"], state), (&json!("picked"), json!("ok")), "{output}");
  fixture.give_back(&path);
}
