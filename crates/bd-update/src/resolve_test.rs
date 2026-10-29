use bd_cli::fixture_test::{assert_error, canonical, git, hold_the_lock, run_in, write};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use test_utils::git_test_utils::TestRepo;
#[cfg(unix)]
use test_utils::history::HistoryCommit;
use test_utils::history::numbered_lines;

const SHARED: &str = "one\ntwo\nthree\n";

/// An upstream with `shared.txt` and its clone, the clone's `(auth)` commit and the upstream's change on line 2
fn conflicting_clone() -> (TestRepo, TestRepo) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED), ("other.txt", "other\n")]);
  clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  upstream.create_commit("Change line 2 upstream", "shared.txt", "one\ntheirs\nthree\n");
  (upstream, clone)
}

fn read(path: &str) -> String {
  std::fs::read_to_string(path).unwrap()
}

fn file_of<'a>(output: &'a Value, path: &str) -> &'a Value {
  output["session"]["files"]
    .as_array()
    .unwrap()
    .iter()
    .find(|file| file["path"] == json!(path))
    .unwrap_or_else(|| panic!("{path} in {output}"))
}

fn edit_of(output: &Value, path: &str) -> String {
  file_of(output, path)["edit"].as_str().unwrap().to_string()
}

fn worktree_status(repo: &TestRepo) -> String {
  git(repo, &["status", "--porcelain", "--untracked-files=all"])
}

#[test_log::test]
fn a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update() {
  let (upstream, clone) = conflicting_clone();
  let head = clone.head();

  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 1, "{round}");
  assert_eq!(
    (&round["state"], &round["session"]["round"], &round["session"]["replayed"], &round["session"]["total"]),
    (&json!("conflict"), &json!(1), &json!(0), &json!(1))
  );
  assert_eq!(round["session"]["commit"]["subject"], json!("(auth) Change line 2"));
  let file = file_of(&round, "shared.txt");
  assert_eq!((&file["kind"], &file["markers"]), (&json!("content"), &json!(1)), "{round}");
  assert_eq!(read(file["onto"].as_str().unwrap()), "one\ntheirs\nthree\n");
  assert_eq!(read(file["commit"].as_str().unwrap()), "one\nmine\nthree\n");
  assert_eq!(read(file["before"].as_str().unwrap()), SHARED);
  assert!(read(round["session"]["commit"]["patch"].as_str().unwrap()).contains("+mine"));
  assert_eq!(file["upstream_commits"][0]["subject"], json!("Change line 2 upstream"));
  // nothing moved, the working tree is as it was
  assert_eq!(clone.head(), head);
  assert_eq!(worktree_status(&clone), "");
  assert!(git(&clone, &["for-each-ref", "--format=%(refname)", "refs/branch-deck/resolve/"]).contains("refs/branch-deck/resolve/main"));

  // a rerun prints the same round
  let (exit_code, again) = run_in(&clone, &["update"]);
  assert_eq!(
    (exit_code, &again["session"]["round"], edit_of(&again, "shared.txt")),
    (1, &json!(1), edit_of(&round, "shared.txt")),
    "{again}"
  );

  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs and mine\nthree\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["--full-ids", "update", "--continue", "--round", "1"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(done["state"], json!("rebase"));
  assert_eq!(done["resolutions"][0]["files"][0], json!({ "path": "shared.txt", "how": "edited" }));
  assert_eq!(done["verify"], json!({ "paths": ["shared.txt"] }), "an edited file is built and tested: {done}");
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), upstream.head());
  assert_eq!(std::fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\ntheirs and mine\nthree\n");
  assert_eq!(worktree_status(&clone), "");
  assert_eq!(git(&clone, &["log", "-1", "--format=%s"]), "(auth) Change line 2");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "");
  assert!(git(&clone, &["notes", "--ref=refs/notes/branch-deck/resolve", "show", "HEAD"]).contains("edited\tshared.txt"));
  assert!(!Path::new(&edit_of(&round, "shared.txt")).exists(), "the copies are removed");
  assert!(!clone.path().join(".git/branch-deck/resolve").exists());

  // the undo of the update takes it back
  let (exit_code, undo) = run_in(&clone, &["undo", done["undo_token"].as_str().unwrap()]);
  assert_eq!(exit_code, 0, "{undo}");
  assert_eq!(clone.head(), head);
}

/// bd with a git wrapper: `arms` are `case " $* " in` arms that run before the real git, with `{dir}` replaced by the
/// directory of the wrapper (its log and marks)
#[cfg(unix)]
struct Wrapped {
  directory: tempfile::TempDir,
  environment: bd_core::context::Environment,
}

#[cfg(unix)]
impl Wrapped {
  fn new(repo: &TestRepo, arms: &str) -> Self {
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("git");
    let arms = arms.replace("{dir}", directory.path().to_str().unwrap());
    let text = format!(
      "#!/bin/sh\necho \"$*\" >> '{}/log'\ncase \" $* \" in {arms}\nesac\nexec git \"$@\"\n",
      directory.path().display()
    );
    std::fs::write(&script, text).unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut environment = bd_cli::fixture_test::environment(repo.path());
    environment.git = Some(script.to_str().unwrap().to_string());
    Self { directory, environment }
  }

  fn run(&self, args: &[&str]) -> (i32, Value) {
    let mut all = vec!["--prefix", bd_cli::fixture_test::PREFIX];
    all.extend_from_slice(args);
    let _ = std::fs::remove_file(self.directory.path().join("log"));
    bd_cli::fixture_test::run_json(&self.environment, &all)
  }

  /// The git commands of the last run that start with `subcommand`
  fn count(&self, subcommand: &str) -> usize {
    let log = std::fs::read_to_string(self.directory.path().join("log")).unwrap_or_default();
    log
      .lines()
      .filter(|line| line.split(' ').find(|word| !word.starts_with('-') && !word.contains('=')) == Some(subcommand))
      .count()
  }

  fn mark(&self, name: &str) {
    std::fs::write(self.directory.path().join(name), "").unwrap();
  }
}

/// The wrapper arms that fail the files of the finish and then its rollback, once: the branch moved, the files did not
#[cfg(unix)]
const FILES_AND_ROLLBACK_FAIL: &str = "*\" read-tree -m -u --no-recurse-submodules \"*) if [ -f '{dir}/fail' ]; then exit 1; fi;;\n\
     *\" update-ref -m branch-deck: update project rolled back \"*) if [ -f '{dir}/fail' ]; then rm -f '{dir}/fail'; exit 1; fi;;";

/// Open the round of `conflicting_clone` through `wrapped` and resolve it in its copy
#[cfg(unix)]
fn resolve_round(wrapped: &Wrapped) {
  let (_, round) = wrapped.run(&["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
}

/// The subjects of the commits on top of the upstream, newest first
fn local_subjects(clone: &TestRepo, upstream: &TestRepo) -> String {
  git(clone, &["log", "--format=%s", &format!("{}..HEAD", upstream.head())])
}

/// Resolve `shared.txt` of `round`, the last one, and finish: `replayed` local commits on the upstream
fn resolve_the_last_round(clone: &TestRepo, round: &Value, content: &str, replayed: usize) {
  std::fs::write(edit_of(round, "shared.txt"), content).unwrap();
  let (exit_code, done) = run_in(clone, &["update", "--continue"]);
  assert_eq!((exit_code, &done["replayed"]), (0, &json!(replayed)), "{done}");
}

/// `--continue` with the wrapper's `fail` mark: the finish fails after the branch moved onto `upstream`
#[cfg(unix)]
fn fail_the_finish(wrapped: &Wrapped, clone: &TestRepo, upstream: &TestRepo) {
  wrapped.mark("fail");
  let (exit_code, output) = wrapped.run(&["update", "--continue"]);
  assert_eq!(exit_code, 5, "{output}");
  assert_eq!(git(clone, &["rev-parse", "HEAD~1"]), upstream.head(), "the branch moved");
}

/// A linked worktree `wt` of `clone` on the new branch `other`; the directory lives as long as the guard
fn linked_worktree(clone: &TestRepo) -> (tempfile::TempDir, std::path::PathBuf) {
  let linked = tempfile::tempdir().unwrap();
  let linked_path = linked.path().join("wt");
  git(clone, &["worktree", "add", "-q", "-b", "other", linked_path.to_str().unwrap()]);
  (linked, linked_path)
}

/// Run bd in the directory `path`
fn run_at(path: &Path, args: &[&str]) -> (i32, Value) {
  bd_cli::fixture_test::run_json(&bd_cli::fixture_test::environment(path), &[&["--prefix", bd_cli::fixture_test::PREFIX][..], args].concat())
}

/// `n` local commits, each adding `f<i>.txt`; the one at `conflict_at` (1-based) changes line 2 of `shared.txt`, which
/// the upstream changes too
#[cfg(unix)]
fn clone_with_commits(n: usize, conflict_at: usize) -> (TestRepo, TestRepo) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED)]);
  // the local commits in one `fast-import`
  let commits: Vec<HistoryCommit> = (1..=n)
    .map(|i| {
      let subject = format!("(auth) Commit {i}");
      if i == conflict_at {
        HistoryCommit::new(subject, &[("shared.txt", "one\nmine\nthree\n")])
      } else {
        HistoryCommit::new(subject, &[(&format!("f{i}.txt"), &format!("{i}\n"))])
      }
    })
    .collect();
  clone.commit_history("refs/heads/master", &commits);
  upstream.create_commit("Change line 2 upstream", "shared.txt", "one\ntheirs\nthree\n");
  (upstream, clone)
}

/// ⌈log₂ n⌉
#[cfg(unix)]
fn search_replays(n: usize) -> usize {
  usize::try_from(n.next_power_of_two().trailing_zeros()).unwrap()
}

#[cfg(unix)]
#[test_log::test]
fn the_first_conflict_is_found_with_a_binary_search() {
  for (n, conflict_at) in [(1, 1), (10, 7), (10, 1), (10, 10)] {
    let (upstream, clone) = clone_with_commits(n, conflict_at);
    // the replays of the session alone: the check replays the commits once more to resolve them by words
    git(&clone, &["config", "branchdeck.resolveByWords", "false"]);
    let wrapped = Wrapped::new(&clone, "");
    let (exit_code, round) = wrapped.run(&["update"]);
    assert_eq!(exit_code, 1, "{round}");
    assert_eq!((&round["session"]["replayed"], &round["session"]["total"]), (&json!(conflict_at - 1), &json!(n)), "{round}");
    assert_eq!(round["session"]["commit"]["subject"], json!(format!("(auth) Commit {conflict_at}")));
    // the replay of the check and its search for the first conflict, then one replay of the commits before it in the
    // session, which opens the round there without a search of its own
    assert!(wrapped.count("replay") <= 2 + search_replays(n), "{n}/{conflict_at}: {} replays", wrapped.count("replay"));

    std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs\nmine\nthree\n").unwrap();
    let (exit_code, done) = wrapped.run(&["update", "--continue"]);
    assert_eq!((exit_code, &done["state"], &done["replayed"]), (0, &json!("rebase"), &json!(n)), "{done}");
    // one replay of the rest, none after the last commit
    assert_eq!(wrapped.count("replay"), usize::from(conflict_at < n));
    assert_eq!(git(&clone, &["rev-list", "--count", &format!("{}..HEAD", upstream.head())]), n.to_string());
    assert_eq!(worktree_status(&clone), "");
  }
}

/// The second local commit changes the line again: after the first round, it conflicts with the resolution
#[test_log::test]
fn a_second_conflict_opens_a_second_round_with_its_own_copies() {
  let (_upstream, clone) = conflicting_clone();
  // the words of the second conflict resolve it: a round of its own needs the resolve by words off
  git(&clone, &["config", "branchdeck.resolveByWords", "false"]);
  clone.create_commit("(auth) Change line 2 again", "shared.txt", "one\nmine again\nthree\n");
  let (_, round1) = run_in(&clone, &["update"]);
  std::fs::write(edit_of(&round1, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();

  let (exit_code, round2) = run_in(&clone, &["update", "--continue", "--round", "1"]);
  assert_eq!(
    (exit_code, &round2["session"]["round"], &round2["session"]["replayed"]),
    (1, &json!(2), &json!(1)),
    "{round2}"
  );
  // a round doesn't repeat the earlier rounds: a long session would print each of them again and again; the finish and
  // `--status` list them
  assert_eq!(round2["session"].get("resolutions"), None, "{round2}");
  let file = file_of(&round2, "shared.txt");
  assert_eq!(read(file["onto"].as_str().unwrap()), "one\ntheirs mine\nthree\n");
  assert_ne!(edit_of(&round2, "shared.txt"), edit_of(&round1, "shared.txt"));
  assert!(!Path::new(&edit_of(&round1, "shared.txt")).exists(), "the copies of round 1 are gone");
  // the round of the next step
  let (exit_code, stale) = run_in(&clone, &["update", "--continue", "--round", "1"]);
  // a stale read: `bd update` prints the open round, and the same command for it succeeds
  assert_error(&stale, exit_code, (3, "round_moved"));
  assert_eq!(stale["error"]["retryable"], json!(true));

  std::fs::write(edit_of(&round2, "shared.txt"), "one\ntheirs mine again\nthree\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue", "--round", "2"]);
  assert_eq!((exit_code, done["resolutions"].as_array().map(Vec::len)), (0, Some(2)), "{done}");
  assert_eq!(std::fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\ntheirs mine again\nthree\n");
  assert_eq!(git(&clone, &["log", "--format=%s", "-2"]), "(auth) Change line 2 again\n(auth) Change line 2");
}

#[test_log::test]
fn markers_an_edit_outside_a_block_and_a_missing_file_are_refused() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  let edit = edit_of(&round, "shared.txt");
  let merged = read(&edit);
  let session = || git(&clone, &["rev-parse", "refs/branch-deck/resolve/main"]);
  let before = session();

  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "unresolved_markers"));
  assert_eq!(output["error"]["paths"], json!(["shared.txt"]));

  std::fs::write(&edit, "ONE\ntheirs mine\nthree\n").unwrap();
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "resolution_outside_conflict"));

  std::fs::remove_file(&edit).unwrap();
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "missing_resolution"));
  assert_eq!(session(), before, "a refusal moves nothing");

  // `--status --materialize` brings the missing copy back
  let (exit_code, status) = run_in(&clone, &["update", "--status", "--materialize"]);
  assert_eq!((exit_code, &status["session"]["round"]), (0, &json!(1)), "{status}");
  assert_eq!(read(&edit), merged);
  std::fs::write(&edit, "edited\n").unwrap();
  run_in(&clone, &["update", "--status", "--materialize"]);
  assert_eq!(read(&edit), "edited\n", "--materialize writes only the missing copies");
  run_in(&clone, &["update", "--status", "--materialize", "--force"]);
  assert_eq!(read(&edit), merged);
}

/// `Set-Content -Encoding UTF8` of Windows PowerShell 5.1 adds a byte order mark, `Out-File` writes UTF-16: the copy is
/// refused with the way to write it
#[test_log::test]
fn an_edit_copy_with_a_byte_order_mark_or_in_utf16_is_refused() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  let edit = edit_of(&round, "shared.txt");
  let resolved = "one\ntheirs mine\nthree\n";
  let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain(resolved.encode_utf16().flat_map(u16::to_le_bytes)).collect();
  for content in [format!("\u{feff}{resolved}").into_bytes(), utf16] {
    std::fs::write(&edit, content).unwrap();
    let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
    assert_error(&output, exit_code, (2, "resolution_outside_conflict"));
    let message = output["error"]["message"].as_str().unwrap();
    assert!(message.contains("byte order mark") && message.contains("[IO.File]::WriteAllText"), "{message}");
  }
  std::fs::write(&edit, resolved).unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0, "{done}");
}

#[cfg(unix)]
#[test_log::test]
fn a_symlink_at_edit_is_never_followed() {
  let (_upstream, clone) = conflicting_clone();
  let secret = clone.path().join("secret.txt");
  std::fs::write(&secret, "secret\n").unwrap();
  let (_, round) = run_in(&clone, &["update"]);
  let edit = edit_of(&round, "shared.txt");
  std::fs::remove_file(&edit).unwrap();
  std::os::unix::fs::symlink(&secret, &edit).unwrap();

  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "missing_resolution"));
  // --force replaces the link instead of writing through it
  run_in(&clone, &["update", "--status", "--materialize", "--force"]);
  assert!(!std::fs::symlink_metadata(&edit).unwrap().file_type().is_symlink());
  assert_eq!(std::fs::read_to_string(&secret).unwrap(), "secret\n");
}

/// The upstream deletes `gone.txt` and changes `kept.txt`; the local commit changes `gone.txt` and deletes `kept.txt`
fn modify_delete_clone() -> (TestRepo, TestRepo) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("gone.txt", "gone\n"), ("kept.txt", "kept\n"), ("other.txt", "other\n")]);
  git(&clone, &["rm", "-q", "kept.txt"]);
  clone.create_commit("(auth) Change gone, delete kept", "gone.txt", "gone, changed\n");
  git(&upstream, &["rm", "-q", "gone.txt"]);
  upstream.create_commit("Change kept, delete gone", "kept.txt", "kept, changed\n");
  (upstream, clone)
}

#[test_log::test]
fn modify_delete_is_resolved_with_take_and_delete() {
  let (_upstream, clone) = modify_delete_clone();
  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 1, "{round}");
  for path in ["gone.txt", "kept.txt"] {
    assert_eq!(file_of(&round, path)["kind"], json!("modify_delete"), "{round}");
  }
  assert_eq!(file_of(&round, "gone.txt").get("onto"), None);
  assert_eq!(file_of(&round, "kept.txt").get("commit"), None);
  // the version left in the tree, unchanged, is no resolution
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "missing_resolution"));
  assert_eq!(output["error"]["paths"], json!(["gone.txt", "kept.txt"]));
  let (exit_code, output) = run_in(&clone, &["update", "--continue", "--take", "upstream:gone.txt"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&clone, &["update", "--continue", "--delete", "nope.txt"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let (exit_code, done) = run_in(&clone, &["update", "--continue", "--take", "commit:gone.txt", "--delete", "kept.txt"]);
  assert_eq!(exit_code, 0, "{done}");
  let hows: Vec<(&Value, &Value)> = done["resolutions"][0]["files"]
    .as_array()
    .unwrap()
    .iter()
    .map(|file| (&file["path"], &file["how"]))
    .collect();
  assert_eq!(hows, [(&json!("gone.txt"), &json!("commit")), (&json!("kept.txt"), &json!("deleted"))]);
  assert_eq!(std::fs::read_to_string(clone.path().join("gone.txt")).unwrap(), "gone, changed\n");
  assert!(!clone.path().join("kept.txt").exists());
  assert_eq!(worktree_status(&clone), "");
}

#[test_log::test]
fn binary_and_file_directory_conflicts_are_refused_and_start_no_session() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("bin.dat", "\0one"), ("df", "file\n")]);
  clone.create_commit("(auth) Change the binary", "bin.dat", "\0mine");
  upstream.create_commit("Change the binary upstream", "bin.dat", "\0theirs");
  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_error(&output, exit_code, (4, "unsupported_conflict"));
  assert_eq!(output["error"]["paths"], json!(["bin.dat"]));
  // the refusal names the commit
  assert!(output["error"]["message"].as_str().unwrap().contains("(auth) Change the binary"), "{output}");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/resolve/"]), "");

  git(&clone, &["reset", "-q", "--hard", "origin/master"]);
  clone.create_commit("(auth) Change df", "df", "mine\n");
  git(&upstream, &["rm", "-q", "df"]);
  upstream.create_commit("df becomes a directory", "df/inner.txt", "inner\n");
  let (exit_code, output) = run_in(&clone, &["update"]);
  assert_error(&output, exit_code, (4, "unsupported_conflict"));
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/resolve/"]), "");
}

#[test_log::test]
fn a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it() {
  let (upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  assert_eq!(round["session"]["commit"].get("hint"), None);
  let commit = round["session"]["commit"]["commit"].as_str().unwrap().to_string();
  let session = git(&clone, &["rev-parse", "refs/branch-deck/resolve/main"]);
  let (exit_code, output) = run_in(&clone, &["update", "--continue", "--take", "onto:shared.txt"]);
  assert_error(&output, exit_code, (2, "empty_resolution"));
  assert!(output["error"]["message"].as_str().unwrap().contains(&format!("--drop-empty {commit}")), "{output}");
  assert_eq!(output["error"]["ask_user"], json!(true), "{output}");
  assert_eq!(git(&clone, &["rev-parse", "refs/branch-deck/resolve/main"]), session, "a refusal moves nothing");
  let (exit_code, output) = run_in(&clone, &["update", "--continue", "--take", "onto:shared.txt", "--drop-empty", "0000"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  // kept: an empty commit on the upstream
  let (exit_code, kept) = run_in(&clone, &["update", "--continue", "--take", "onto:shared.txt", "--allow-empty", "--dry-run"]);
  assert_eq!((exit_code, &kept["resolutions"][0]["empty"]), (0, &json!("kept")), "{kept}");

  let (exit_code, done) = run_in(&clone, &["--full-ids", "update", "--continue", "--take", "onto:shared.txt", "--drop-empty", &commit]);
  assert_eq!(exit_code, 0, "{done}");
  let local = git(&clone, &["rev-parse", &commit]);
  assert_eq!(
    (&done["replayed"], &done["dropped_count"], &done["dropped"]),
    (&json!(0), &json!(1), &json!([{ "commit": local, "subject": "(auth) Change line 2", "proof": "user" }])),
    "{done}"
  );
  assert_eq!(
    (&done["resolutions"][0]["new_id"], done["resolutions"][0].get("empty"), done.get("verify")),
    (&Value::Null, None, None),
    "a dropped round changed nothing to verify: {done}"
  );
  assert_eq!(done.get("landed"), None, "a dropped commit without a counterpart upstream did not land: {done}");
  assert_eq!(clone.head(), upstream.head());
  assert_eq!(git(&clone, &["notes", "--ref=refs/notes/branch-deck/resolve", "list"]), "", "a dropped commit gets no note");
}

#[test_log::test]
fn peers_commits_on_top_become_more_rounds() {
  let (upstream, clone) = conflicting_clone();
  // the words of the peer's conflict resolve it: a round of its own needs the resolve by words off
  git(&clone, &["config", "branchdeck.resolveByWords", "false"]);
  let (_, round) = run_in(&clone, &["update"]);
  // a peer commits in the same tree while the agent resolves: one clean commit, one that conflicts again
  clone.create_commit("(peer) Add peer", "peer.txt", "peer\n");
  clone.create_commit("(peer) Change line 2 too", "shared.txt", "one\nmine and peer\nthree\n");
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();

  let (exit_code, round2) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(
    (exit_code, &round2["session"]["round"], &round2["session"]["replayed"], &round2["session"]["total"]),
    (1, &json!(2), &json!(2), &json!(3)),
    "{round2}"
  );
  assert_eq!(round2["session"]["commit"]["subject"], json!("(peer) Change line 2 too"));
  resolve_the_last_round(&clone, &round2, "one\ntheirs mine and peer\nthree\n", 3);
  assert_eq!(local_subjects(&clone, &upstream), "(peer) Change line 2 too\n(peer) Add peer\n(auth) Change line 2");
  assert_eq!(worktree_status(&clone), "");
}

#[test_log::test]
fn a_rewritten_head_makes_the_session_stale_and_abort_removes_it() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  git(&clone, &["commit", "-q", "--amend", "-m", "(auth) Reworded"]);
  let head = clone.head();

  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (4, "session_stale"));
  assert_eq!(output["error"]["retryable"], json!(false));
  assert_eq!(clone.head(), head);

  let (exit_code, aborted) = run_in(&clone, &["update", "--abort"]);
  assert_eq!((exit_code, &aborted["worktree"], &aborted["round"]), (0, &json!("main"), &json!(1)), "{aborted}");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/", "refs/notes/"]), "", "no session ref, no notes");
  assert!(!Path::new(&edit_of(&round, "shared.txt")).exists());
  assert!(!clone.path().join(".git/branch-deck/resolve").exists());
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "no_session"));
}

#[test_log::test]
fn a_session_belongs_to_its_worktree_and_survives_a_gc_in_another() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  let (_linked, linked_path) = linked_worktree(&clone);
  let linked_repo = |args: &[&str]| {
    bd_cli::fixture_test::run_json(
      &bd_cli::fixture_test::environment(&linked_path),
      &[&["--prefix", bd_cli::fixture_test::PREFIX][..], args].concat(),
    )
  };

  let (exit_code, output) = linked_repo(&["update", "--continue"]);
  assert_error(&output, exit_code, (2, "no_session"));
  let (exit_code, status) = linked_repo(&["update", "--status"]);
  assert_eq!(
    (exit_code, status.get("session"), &status["sessions"][0]["worktree"], &status["sessions"][0]["own"]),
    (0, None, &json!("main"), &json!(false)),
    "{status}"
  );

  // a gc in the other worktree, with every reflog expired, keeps what the session needs
  let linked_git = |args: &[&str]| clone.git_executor().execute_command(args, linked_path.to_str().unwrap()).unwrap();
  linked_git(&["reflog", "expire", "--expire=now", "--all"]);
  linked_git(&["gc", "-q", "--prune=now"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0, "{done}");
}

#[test_log::test]
fn an_orphaned_session_is_listed_and_aborted_by_its_worktree() {
  let (_upstream, clone) = conflicting_clone();
  let (_linked, linked_path) = linked_worktree(&clone);
  let (exit_code, round) = run_at(&linked_path, &["update"]);
  assert_eq!((exit_code, &round["session"]["worktree"]), (1, &json!("wt")), "{round}");
  git(&clone, &["worktree", "remove", "--force", linked_path.to_str().unwrap()]);

  let (_, status) = run_in(&clone, &["update", "--status"]);
  assert_eq!(
    (&status["sessions"][0]["worktree"], &status["sessions"][0]["orphaned"]),
    (&json!("wt"), &json!(true)),
    "{status}"
  );
  let (exit_code, aborted) = run_in(&clone, &["update", "--abort", "--worktree", "wt"]);
  assert_eq!((exit_code, &aborted["worktree"]), (0, &json!("wt")), "{aborted}");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "");
}

#[test_log::test]
fn a_dry_run_of_continue_checks_and_gives_the_ids_of_the_real_one() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let session = git(&clone, &["rev-parse", "refs/branch-deck/resolve/main"]);

  let (exit_code, checked) = run_in(&clone, &["--full-ids", "update", "--continue", "--dry-run"]);
  assert_eq!((exit_code, &checked["dry_run"]), (0, &json!(true)), "{checked}");
  assert_eq!(git(&clone, &["rev-parse", "refs/branch-deck/resolve/main"]), session, "a dry run moves nothing");
  let resolved = checked["resolutions"][0]["new_id"].clone();
  assert!(resolved.is_string(), "{checked}");
  let (_, again) = run_in(&clone, &["--full-ids", "update", "--continue", "--dry-run"]);
  assert_eq!(again["resolutions"][0]["new_id"], resolved);

  let (exit_code, done) = run_in(&clone, &["--full-ids", "update", "--continue"]);
  assert_eq!((exit_code, &done["resolutions"][0]["new_id"], &done["new_head"]), (0, &resolved, &resolved), "{done}");
}

/// An agent checks its resolution in a loop while the app's Update Project or a rewrite holds the lock: the check
/// answers at once, and only the real `--continue` waits for the lock
#[test_log::test]
fn a_dry_run_of_continue_answers_while_another_process_holds_the_lock() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let held = hold_the_lock(&clone);

  let (exit_code, checked) = run_in(&clone, &["update", "--continue", "--dry-run"]);
  assert_eq!((exit_code, &checked["dry_run"]), (0, &json!(true)), "{checked}");
  assert!(checked["resolutions"][0]["new_id"].is_string(), "{checked}");
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (3, "locked"));

  drop(held);
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0, "{done}");
}

#[test_log::test]
fn the_copies_go_to_dir_only_outside_every_worktree() {
  let (_upstream, clone) = conflicting_clone();
  let inside = clone.path().join("copies");
  let (exit_code, output) = run_in(&clone, &["update", "--dir", inside.to_str().unwrap()]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(!inside.exists(), "a refused --dir that bd created is removed");

  let outside = tempfile::tempdir().unwrap();
  std::fs::write(outside.path().join("unrelated.txt"), "x").unwrap();
  let (exit_code, output) = run_in(&clone, &["update", "--dir", outside.path().to_str().unwrap()]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  #[cfg(unix)]
  {
    let link = outside.path().join("link");
    std::os::unix::fs::symlink(clone.path(), &link).unwrap();
    let (exit_code, output) = run_in(&clone, &["update", "--dir", link.to_str().unwrap()]);
    assert_error(&output, exit_code, (2, "invalid_arguments"));
  }

  let empty = tempfile::tempdir().unwrap();
  let (exit_code, round) = run_in(&clone, &["update", "--dir", empty.path().to_str().unwrap()]);
  assert_eq!(exit_code, 1, "{round}");
  let edit = edit_of(&round, "shared.txt");
  assert!(Path::new(&edit).starts_with(canonical(empty.path())), "{edit}");
  std::fs::write(&edit, "one\ntheirs mine\nthree\n").unwrap();
  let (exit_code, _) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0);
  assert!(empty.path().exists(), "bd removes only what it wrote");
  assert_eq!(std::fs::read_dir(empty.path()).unwrap().count(), 0);
}

/// The finish fails right at its move once (as a crash between the `applying` state and the move would leave it), and
/// a `git switch` before the move refuses it: both leave the session to finish later
#[cfg(unix)]
#[test_log::test]
fn an_interrupted_finish_is_completed_by_the_next_continue() {
  let (upstream, clone) = conflicting_clone();
  let wrapped = Wrapped::new(
    &clone,
    "*\" update-ref -m branch-deck: update project \"*) if [ -f '{dir}/fail' ]; then rm -f '{dir}/fail'; exit 1; fi;;\n\
     *\" update-ref \"*\"--stdin\"*) if [ -f '{dir}/switch' ]; then rm -f '{dir}/switch'; git symbolic-ref HEAD refs/heads/peer; fi;;",
  );
  let (_, round) = wrapped.run(&["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let head = clone.head();

  wrapped.mark("fail");
  let (exit_code, output) = wrapped.run(&["update", "--continue"]);
  assert_eq!(exit_code, 5, "{output}");
  assert_eq!(clone.head(), head);
  let (_, status) = run_in(&clone, &["update", "--status"]);
  assert_eq!(status["session"]["state"], json!("applying"), "{status}");

  git(&clone, &["branch", "peer"]);
  wrapped.mark("switch");
  let (exit_code, output) = wrapped.run(&["update", "--continue"]);
  assert_error(&output, exit_code, (3, "head_moved"));
  git(&clone, &["symbolic-ref", "HEAD", "refs/heads/master"]);
  assert_eq!(git(&clone, &["rev-parse", "master", "peer"]), format!("{head}\n{head}"));

  let (exit_code, done) = wrapped.run(&["update", "--continue"]);
  assert_eq!((exit_code, &done["state"]), (0, &json!("rebase")), "{done}");
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), upstream.head());
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "");
  assert_eq!(worktree_status(&clone), "");
}

/// The move commits, but the files fail to follow and the rollback fails too: HEAD stays at the new tip with the index
/// and the files of the old one (`applied`), and the next `--continue` lets the files follow
#[cfg(unix)]
#[test_log::test]
fn a_finish_killed_before_the_files_followed_is_recovered() {
  let (upstream, clone) = conflicting_clone();
  let wrapped = Wrapped::new(&clone, FILES_AND_ROLLBACK_FAIL);
  resolve_round(&wrapped);

  fail_the_finish(&wrapped, &clone, &upstream);
  assert_eq!(std::fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\nmine\nthree\n");
  let (_, status) = run_in(&clone, &["update", "--status"]);
  assert_eq!(status["session"]["state"], json!("applied"), "{status}");
  let (exit_code, output) = run_in(&clone, &["update", "--abort"]);
  assert_error(&output, exit_code, (4, "finish_pending"));

  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!((exit_code, &done["state"]), (0, &json!("rebase")), "{done}");
  assert_eq!(std::fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\ntheirs mine\nthree\n");
  assert_eq!(worktree_status(&clone), "");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "");
}

#[test_log::test]
fn local_changes_on_the_finish_keep_the_session_ready() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  std::fs::write(clone.path().join("shared.txt"), "a peer's edit\n").unwrap();

  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (4, "uncommitted_overlap"));
  assert_eq!(output["error"]["paths"], json!(["shared.txt"]));
  let (_, status) = run_in(&clone, &["update", "--status"]);
  assert_eq!(
    (&status["session"]["state"], &status["session"]["resolutions"][0]["files"][0]["how"]),
    (&json!("ready"), &json!("edited")),
    "{status}"
  );
  let (exit_code, pending) = run_in(&clone, &["update"]);
  assert_eq!(
    (exit_code, &pending["state"], &pending["session"]["next"]),
    (1, &json!("pending"), &json!("bd update --continue")),
    "{pending}"
  );

  std::fs::write(clone.path().join("shared.txt"), "one\nmine\nthree\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!((exit_code, done["resolutions"].as_array().map(Vec::len)), (0, Some(1)), "{done}");
}

/// A commit of `files` at one author time, so a commit of the same subject in another repository has its identity
fn commit_at(repo: &TestRepo, subject: &str, files: &[(&str, &str)]) {
  for (path, content) in files {
    std::fs::write(repo.path().join(path), content).unwrap();
    repo.git_executor().execute_command(&["add", path], repo.path_str()).unwrap();
  }
  repo
    .git_executor()
    .cmd(&["commit", "-q", "-m", subject], repo.path_str())
    .env(&[("GIT_AUTHOR_DATE", "1700000000 +0000")])
    .text()
    .unwrap();
}

/// Runs the `next` of `output` with `extra`, with full ids
fn run_next(clone: &TestRepo, output: &Value, extra: &[&str]) -> (i32, Value) {
  let next = output["session"]["next"].as_str().unwrap_or_else(|| panic!("{output}"));
  let mut args = vec!["--full-ids"];
  args.extend(next.split(' ').skip(1));
  args.extend_from_slice(extra);
  run_in(clone, &args)
}

/// The robot landed the commit with a fixup folded in: its twin lacks the line of the commit, so a drop loses it. The
/// first output asks the user once, `--landed` refuses without `--user`, and drops with it as landed
#[test_log::test]
fn a_twin_that_differs_asks_the_user_and_drops_after_the_yes() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED)]);
  commit_at(&clone, "(auth) Change line 2", &[("shared.txt", "one\nmine\nthree\n")]);
  commit_at(&upstream, "Change line 2", &[("shared.txt", "one\nmine, fixed\nthree\n")]);
  let (local, twin) = (clone.head(), upstream.head());
  let (exit_code, round) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!(exit_code, 1, "{round}");
  let landed_as = json!({
    "commit": twin,
    "subject": "Change line 2",
    "local_only": [{ "path": "shared.txt", "lines": ["+mine"] }],
    "upstream_only": [{ "path": "shared.txt", "lines": ["+mine, fixed"] }],
  });
  assert_eq!(
    (&round["ask_user"], &round["reasons"][0]["code"], &round["session"]["landed_differs"]),
    (
      &json!(true),
      &json!("landed_differs"),
      &json!([{ "commit": local, "subject": "(auth) Change line 2", "landed_as": landed_as }])
    ),
    "{round}"
  );
  assert_eq!(round["session"]["commit"]["landed_as"], landed_as, "{round}");
  assert!(round["session"]["next"].as_str().unwrap().ends_with(&format!("--round 1 --landed {local}")), "{round}");
  assert!(file_of(&round, "shared.txt")["hint"].as_str().unwrap().contains("session.commit.hint"), "{round}");
  assert_eq!(round.get("dropped"), None, "the identity alone proves nothing: {round}");

  // `next` waits for the user's yes
  let (exit_code, output) = run_next(&clone, &round, &[]);
  assert_error(&output, exit_code, (2, "landed_differs"));
  assert_eq!(output["error"]["ask_user"], json!(true), "{output}");
  let (exit_code, done) = run_next(&clone, &round, &["--user"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(
    done["dropped"],
    json!([{ "commit": local, "subject": "(auth) Change line 2", "upstream": twin, "proof": "landed" }]),
    "{done}"
  );
  assert_eq!(
    (&done["resolutions"][0]["landed_as"], &done["resolutions"][0]["local_only"]),
    (&json!(twin), &json!([{ "path": "shared.txt", "lines": ["+mine"] }])),
    "{done}"
  );
  assert_eq!(
    (&done["landed"][0]["commits"][0]["commit"], &done["landed"][0]["commits"][0]["upstream"]),
    (&json!(local), &json!(twin)),
    "a drop as landed lands as the twin: {done}"
  );
  assert_eq!(clone.head(), upstream.head());
}

/// The twin has every line of the commit and more (the upstream changed a line beside it): a drop loses nothing, so the
/// round's `next` drops it as landed without a question
#[test_log::test]
fn a_twin_with_every_line_of_the_commit_drops_without_a_question() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED)]);
  commit_at(&clone, "(auth) Change line 2", &[("shared.txt", "one\nmine\nthree\n")]);
  commit_at(&upstream, "Change line 2", &[("shared.txt", "one\nmine\nTHREE\n")]);
  let twin = upstream.head();
  upstream.create_commit("Change line 2 again", "shared.txt", "one\ntheirs\nTHREE\n");
  let local = clone.head();
  let (exit_code, round) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!(exit_code, 1, "{round}");
  assert_eq!(
    (round.get("ask_user"), round["session"].get("landed_differs")),
    (None, None),
    "a drop that loses nothing needs no question: {round}"
  );
  assert_eq!(
    round["session"]["commit"]["landed_as"],
    json!({ "commit": twin, "subject": "Change line 2", "upstream_only": [{ "path": "shared.txt", "lines": ["-three", "+THREE"] }] }),
    "{round}"
  );
  let (exit_code, done) = run_next(&clone, &round, &[]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(
    (&done["dropped"], &done["landed"][0]["commits"][0]["upstream"]),
    (
      &json!([{ "commit": local, "subject": "(auth) Change line 2", "upstream": twin, "proof": "landed" }]),
      &json!(twin)
    ),
    "{done}"
  );
  assert_eq!(done.get("verify"), None, "nothing of the drop is built: {done}");
  assert_eq!(clone.head(), upstream.head());
}

/// `--landed` drops only the commit of a twin round, and only when `onto` for every file leaves it without a change
#[test_log::test]
fn landed_refuses_a_round_without_a_twin_and_a_commit_that_keeps_a_change() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  let commit = round["session"]["commit"]["commit"].as_str().unwrap().to_string();
  let (exit_code, output) = run_in(&clone, &["update", "--continue", "--landed", &commit]);
  assert_error(&output, exit_code, (2, "not_landed_round"));
  assert_eq!(output["error"].get("ask_user"), None, "{output}");

  // the commit changes a file that its twin does not: even after the user's yes, `onto` leaves that change
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED), ("other.txt", "other\n")]);
  commit_at(&clone, "(auth) Change line 2", &[("shared.txt", "one\nmine\nthree\n"), ("other.txt", "other, mine\n")]);
  commit_at(&upstream, "Change line 2", &[("shared.txt", "one\nmine, fixed\nthree\n")]);
  let (_, round) = run_in(&clone, &["update"]);
  assert_eq!(
    round["session"]["commit"]["landed_as"]["local_only"],
    json!([{ "path": "other.txt", "lines": ["-other", "+other, mine"] }, { "path": "shared.txt", "lines": ["+mine"] }]),
    "{round}"
  );
  let (exit_code, output) = run_next(&clone, &round, &[]);
  assert_error(&output, exit_code, (2, "landed_differs"));
  let (exit_code, output) = run_next(&clone, &round, &["--user"]);
  assert_error(&output, exit_code, (2, "landed_not_empty"));
  assert_eq!(output["error"]["paths"], json!(["other.txt"]), "{output}");
  assert_eq!(round["session"]["round"], json!(1), "a refusal keeps the round");
}

#[test_log::test]
fn session_outputs_match_the_schema() {
  let (_upstream, clone) = conflicting_clone();
  let (_, schema) = bd_cli::fixture_test::run_json(&bd_cli::fixture_test::environment(clone.path()), &["schema", "--all"]);
  let outputs = &schema["outputs"];
  let (_, round) = run_in(&clone, &["update"]);
  bd_cli::fixture_test::assert_covered(&outputs["update"], &outputs["update"], &round, "round");
  let (_, status) = run_in(&clone, &["update", "--status"]);
  bd_cli::fixture_test::assert_covered(&outputs["update_status"], &outputs["update_status"], &status, "status");
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let (_, checked) = run_in(&clone, &["update", "--continue", "--dry-run"]);
  bd_cli::fixture_test::assert_covered(&outputs["update"], &outputs["update"], &checked, "checked");
  let (_, aborted) = run_in(&clone, &["update", "--abort"]);
  bd_cli::fixture_test::assert_covered(&outputs["update_abort"], &outputs["update_abort"], &aborted, "abort");
  let (_, round) = run_in(&clone, &["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let (_, done) = run_in(&clone, &["update", "--continue"]);
  bd_cli::fixture_test::assert_covered(&outputs["update"], &outputs["update"], &done, "finish");
}

/// The finish dies before it deletes the session ref (step 4): the branch moved and the files followed. A peer's commit
/// on top must not strand the session: `--abort` and `--continue` both end it
#[cfg(unix)]
#[test_log::test]
fn a_session_whose_finish_died_after_the_files_followed_ends_after_a_peer_commit() {
  for end_with in ["--continue", "--abort"] {
    let (upstream, clone) = conflicting_clone();
    let wrapped = Wrapped::new(
      &clone,
      "*\" update-ref -m branch-deck: resolve session finished \"*) if [ -f '{dir}/fail' ]; then rm -f '{dir}/fail'; exit 1; fi;;",
    );
    resolve_round(&wrapped);
    fail_the_finish(&wrapped, &clone, &upstream);
    assert_eq!(std::fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\ntheirs mine\nthree\n");
    clone.create_commit("(peer) After the update", "peer.txt", "peer\n");

    let (exit_code, output) = run_in(&clone, &["update", end_with]);
    assert_eq!(exit_code, 0, "{end_with}: {output}");
    assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/"]), "", "{end_with}");
    assert_eq!(git(&clone, &["log", "-1", "--format=%s"]), "(peer) After the update");
  }
}

/// The files did not follow, and a peer committed the old files on top: bd can't finish, and says so
#[cfg(unix)]
#[test_log::test]
fn a_moved_branch_without_its_files_is_finish_pending() {
  let (_upstream, clone) = conflicting_clone();
  let wrapped = Wrapped::new(&clone, FILES_AND_ROLLBACK_FAIL);
  resolve_round(&wrapped);
  wrapped.mark("fail");
  wrapped.run(&["update", "--continue"]);
  git(&clone, &["commit", "-q", "-m", "(peer) Commit of the old index"]);

  for args in [&["update", "--continue"][..], &["update", "--abort"]] {
    let (exit_code, output) = run_in(&clone, args);
    assert_error(&output, exit_code, (4, "finish_pending"));
    assert!(
      output["error"]["message"].as_str().unwrap().contains("git update-ref -d refs/branch-deck/resolve/main"),
      "{output}"
    );
  }
}

/// Markdown setext headings (`=======`) outside the block are text, and `conflict-marker-size` makes longer markers
#[test_log::test]
fn markers_are_found_by_their_labels() {
  let guide = "Setup\n=======\n\none\ntwo\nthree\n\nUsage\n=======\n";
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("guide.md", guide), ("c.txt", SHARED), (".gitattributes", "*.txt conflict-marker-size=10\n")]);
  clone.create_commit_with_files("(docs) Mine", &[("guide.md", &guide.replace("two", "mine")), ("c.txt", "one\nmine\nthree\n")]);
  upstream.create_commit_with_files("Theirs", &[("guide.md", &guide.replace("two", "theirs")), ("c.txt", "one\ntheirs\nthree\n")]);
  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 1, "{round}");
  let labels = &round["session"]["marker_labels"];
  let long_edit = read(&edit_of(&round, "c.txt"));
  assert!(long_edit.contains(&format!("<<<<<<<<<< {}", labels["onto"].as_str().unwrap())), "{long_edit}");
  assert_eq!((&file_of(&round, "c.txt")["markers"], &file_of(&round, "guide.md")["markers"]), (&json!(1), &json!(1)));

  // an unchanged copy with long markers is refused, never committed
  std::fs::write(edit_of(&round, "guide.md"), guide.replace("two", "theirs mine")).unwrap();
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "unresolved_markers"));
  assert_eq!(output["error"]["paths"], json!(["c.txt"]));
  std::fs::write(edit_of(&round, "c.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(std::fs::read_to_string(clone.path().join("guide.md")).unwrap(), guide.replace("two", "theirs mine"));
}

/// The commonest mistake: the agent edits the file in the working tree; the error names the copy to edit
#[test_log::test]
fn an_edit_in_the_working_tree_is_pointed_to_the_copy() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  std::fs::write(clone.path().join("shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "unresolved_markers"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains(&edit_of(&round, "shared.txt")), "{message}");
  assert!(message.contains("shared.txt changed in the working tree"), "{message}");

  // an edit outside a block says where such a change goes
  std::fs::write(edit_of(&round, "shared.txt"), "ONE\ntheirs mine\nthree\n").unwrap();
  let (_, output) = run_in(&clone, &["update", "--continue"]);
  assert!(output["error"]["message"].as_str().unwrap().contains("`fixup! (auth) Change line 2`"), "{output}");
}

#[test_log::test]
fn local_changes_that_will_block_the_finish_are_named_from_the_start() {
  let (_upstream, clone) = conflicting_clone();
  write(&clone, "shared.txt", "a local edit\n");
  // the working tree holds the local edit; the round still opens, with the warning
  git(&clone, &["stash", "-q"]);
  let (exit_code, round) = run_in(&clone, &["update"]);
  git(&clone, &["stash", "pop", "-q"]);
  assert_eq!(exit_code, 1, "{round}");
  let (_, again) = run_in(&clone, &["update"]);
  assert_eq!(again["session"]["blocked_paths"], json!(["shared.txt"]), "{again}");
  assert_eq!((&again["ask_user"], &again["reasons"][0]["code"]), (&json!(true), &json!("blocked_paths")), "{again}");
  // a round of --continue lists them again, without a new question
  let (_, checked) = run_in(&clone, &["update", "--continue", "--take", "onto:shared.txt", "--allow-empty", "--dry-run"]);
  assert_eq!(checked.get("ask_user"), None, "{checked}");
}

#[test_log::test]
fn the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read() {
  let (_upstream, clone) = conflicting_clone();
  let shared = tempfile::tempdir().unwrap();
  let (exit_code, round) = run_in(&clone, &["update", "--dir", shared.path().to_str().unwrap()]);
  assert_eq!(exit_code, 1, "{round}");
  let (_linked, linked_path) = linked_worktree(&clone);
  let (exit_code, output) = run_at(&linked_path, &["update", "--dir", shared.path().to_str().unwrap()]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(Path::new(&edit_of(&round, "shared.txt")).exists(), "the other worktree's copies stay");

  // `r1/edit` replaced by a symlink to a directory outside: never read, never deleted through
  #[cfg(unix)]
  {
    let outside = tempfile::tempdir().unwrap();
    let edit = edit_of(&round, "shared.txt");
    let edit_dir = Path::new(&edit).parent().unwrap().to_path_buf();
    std::fs::write(outside.path().join(Path::new(&edit).file_name().unwrap()), "one\nfrom outside\nthree\n").unwrap();
    std::fs::remove_dir_all(&edit_dir).unwrap();
    std::os::unix::fs::symlink(outside.path(), &edit_dir).unwrap();
    let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
    assert_error(&output, exit_code, (2, "missing_resolution"));
    let (exit_code, _) = run_in(&clone, &["update", "--abort"]);
    assert_eq!(exit_code, 0);
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1, "nothing deleted through the symlink");
  }
}

#[test_log::test]
fn aborting_the_live_session_of_another_worktree_needs_force() {
  let (_upstream, clone) = conflicting_clone();
  let (_linked, linked_path) = linked_worktree(&clone);
  run_at(&linked_path, &["update"]);

  let (exit_code, output) = run_in(&clone, &["update", "--abort", "--worktree", "wt"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&clone, &["update", "--abort", "--worktree", "w t"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&clone, &["update", "--abort", "--worktree", "wt", "--force"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(git(&clone, &["for-each-ref", "refs/branch-deck/resolve/"]), "");
}

#[test_log::test]
fn stale_temporary_refs_are_swept_when_a_session_starts() {
  let (_upstream, clone) = conflicting_clone();
  // a ref of a process killed an hour ago, and one of now
  let head = clone.head();
  let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
  git(&clone, &["update-ref", "refs/branch-deck/tmp/1-1000-0", &head]);
  git(&clone, &["update-ref", &format!("refs/branch-deck/tmp/1-{now}-0"), &head]);
  run_in(&clone, &["update"]);
  assert_eq!(
    git(&clone, &["for-each-ref", "--format=%(refname)", "refs/branch-deck/tmp/"]),
    format!("refs/branch-deck/tmp/1-{now}-0")
  );
}

/// A rename on either side, or of the whole directory, with a change of the same line on the other side: merge-ort
/// writes `<label>:<path>` after the markers, and the round resolves with an edit
#[test_log::test]
fn a_renamed_file_with_a_conflict_resolves_with_an_edit() {
  // (upstream, local): each a commit as `(message, rename, file)`, a rename `(from, to)` done with `git mv`
  type Change = (&'static str, Option<(&'static str, &'static str)>, &'static str);
  let cases: [(&str, Change, Change); 3] = [
    (
      "upstream rename",
      ("Rename c upstream", Some(("c.txt", "d.txt")), "d.txt"),
      ("(auth) Change c", None, "c.txt"),
    ),
    ("local rename", ("Change c upstream", None, "c.txt"), ("(auth) Rename c", Some(("c.txt", "d.txt")), "d.txt")),
    (
      "directory rename",
      ("Move src to lib", Some(("src", "lib")), "lib/c.txt"),
      ("(auth) Change src/c", None, "src/c.txt"),
    ),
  ];
  for (name, (up_message, up_rename, up_file), (local_message, local_rename, local_file)) in cases {
    let base_file = if name == "directory rename" { "src/c.txt" } else { "c.txt" };
    let (upstream, clone) = TestRepo::upstream_and_clone(&[(base_file, SHARED), ("other.txt", "other\n")]);
    let change = |repo: &TestRepo, message: &str, rename: Option<(&str, &str)>, file: &str, line: &str| {
      if let Some((from, to)) = rename {
        git(repo, &["mv", from, to]);
      }
      repo.create_commit(message, file, &format!("one\n{line}\nthree\n"));
    };
    change(&upstream, up_message, up_rename, up_file, "theirs");
    change(&clone, local_message, local_rename, local_file, "mine");
    let (exit_code, round) = run_in(&clone, &["update"]);
    assert_eq!(exit_code, 1, "{name}: {round}");
    let file = &round["session"]["files"][0];
    assert_eq!((&file["kind"], &file["markers"]), (&json!("content"), &json!(1)), "{name}: {round}");
    let path = file["path"].as_str().unwrap().to_string();
    std::fs::write(file["edit"].as_str().unwrap(), "one\ntheirs mine\nthree\n").unwrap();
    let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
    assert_eq!(exit_code, 0, "{name}: {done}");
    assert_eq!(std::fs::read_to_string(clone.path().join(&path)).unwrap(), "one\ntheirs mine\nthree\n", "{name}");
    assert_eq!(worktree_status(&clone), "", "{name}");
  }
}

/// A take reports what it drops of the other side
#[test_log::test]
fn a_take_reports_the_lines_of_the_other_side_it_drops() {
  let (_upstream, clone) = conflicting_clone();
  run_in(&clone, &["update"]);
  let (exit_code, checked) = run_in(&clone, &["update", "--continue", "--take", "commit:shared.txt", "--dry-run"]);
  assert_eq!(exit_code, 0, "{checked}");
  assert_eq!(checked["resolutions"][0]["files"][0]["dropped_upstream_lines"], json!(["theirs"]), "{checked}");
  assert_eq!(checked["resolutions"][0]["files"][0].get("dropped_commit_lines"), None, "{checked}");
}

/// A local commit that the upstream cherry-picked and then changed further: as `git rebase` does, bd leaves it out by
/// its patch id instead of opening a round for it
#[test_log::test]
fn a_commit_the_upstream_has_with_the_same_patch_is_left_out() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED), ("other.txt", "other\n")]);
  clone.create_commit("(auth) Add a", "a.txt", "a\n");
  let landed = clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  clone.create_commit("(auth) Add b", "b.txt", "b\n");
  let picked = upstream.create_commit("Change line 2 (picked)", "shared.txt", "one\nmine\nthree\n");
  upstream.create_commit("Change line 2 further", "shared.txt", "one\nmine, further\nthree\n");

  let (exit_code, done) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(done["state"], json!("rebase"));
  assert_eq!(
    done["dropped"],
    json!([{ "commit": landed, "subject": "(auth) Change line 2", "upstream": picked, "proof": "patch_id" }]),
    "{done}"
  );
  assert_eq!((&done["replayed"], &done["dropped_count"]), (&json!(2), &json!(1)), "{done}");
  assert_eq!(local_subjects(&clone, &upstream), "(auth) Add b\n(auth) Add a");
  assert_eq!(std::fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\nmine, further\nthree\n");
  assert_eq!(worktree_status(&clone), "");
}

/// The twins of `upstream_and_clone_with_twins` land, each left out by its proof; the local fixup of the first replays
/// on its own, and the only round is the real conflict of `other.txt`
#[test_log::test]
fn twins_in_another_order_are_left_out_by_their_proof_and_land() {
  let (upstream, clone, local, twins) = TestRepo::upstream_and_clone_with_twins(true);
  let (exit_code, round) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!(
    (exit_code, &round["session"]["round"], &round["session"]["commit"]["subject"]),
    (1, &json!(1), &json!("(ui) Change other")),
    "{round}"
  );
  let proven = json!([
    { "commit": local[0], "subject": "(auth) Change line 5", "upstream": twins[1], "proof": "replayed" },
    { "commit": local[2], "subject": "(auth) Change line 5 of g", "upstream": twins[3], "proof": "changed_lines" },
  ]);
  assert_eq!((&round["dropped"], &round["dropped_count"]), (&proven, &json!(2)), "{round}");
  assert_eq!(round.get("ask_user"), None, "a proof needs no question: {round}");
  std::fs::write(edit_of(&round, "other.txt"), "theirs and mine\n").unwrap();
  // a round of --continue counts the dropped commits; the first output listed them
  let (_, checked) = run_in(&clone, &["--full-ids", "update", "--continue", "--dry-run"]);
  assert_eq!((checked.get("dropped"), &checked["dropped_count"]), (None, &json!(2)), "{checked}");
  let (exit_code, done) = run_in(&clone, &["--full-ids", "update", "--continue"]);
  assert_eq!((exit_code, &done["replayed"], &done["dropped"]), (0, &json!(2), &proven), "{done}");
  let landed: Vec<(&Value, &Value)> = done["landed"][0]["commits"]
    .as_array()
    .unwrap_or_else(|| panic!("{done}"))
    .iter()
    .map(|commit| (&commit["commit"], &commit["upstream"]))
    .collect();
  assert_eq!(landed, [(&json!(local[0]), &json!(twins[1])), (&json!(local[2]), &json!(twins[3]))], "{done}");
  assert_eq!(local_subjects(&clone, &upstream), "(ui) Change other\nfixup! (auth) Change line 5");
  assert_eq!(read(clone.path().join("f.txt").to_str().unwrap()), numbered_lines(&[(2, "two"), (5, "five, fixed")]));
}

/// A left-out commit splits the local commits into runs; the conflict after it still opens its round
#[test_log::test]
fn a_conflict_after_a_left_out_commit_opens_its_round() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", SHARED), ("other.txt", "other\n")]);
  clone.create_commit("(auth) Change other", "other.txt", "other, mine\n");
  clone.create_commit("(auth) Add a", "a.txt", "a\n");
  clone.create_commit("(auth) Change line 2", "shared.txt", "one\nmine\nthree\n");
  clone.create_commit("(auth) Add b", "b.txt", "b\n");
  upstream.create_commit("Change other (picked)", "other.txt", "other, mine\n");
  upstream.create_commit("Change other further", "other.txt", "other, mine, further\n");
  upstream.create_commit("Change line 2 upstream", "shared.txt", "one\ntheirs\nthree\n");

  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 1, "{round}");
  assert_eq!(round["session"]["commit"]["subject"], json!("(auth) Change line 2"));
  assert_eq!((&round["session"]["replayed"], &round["session"]["total"]), (&json!(1), &json!(3)), "{round}");
  assert_eq!(round["dropped"][0]["subject"], json!("(auth) Change other"));
  resolve_the_last_round(&clone, &round, "one\ntheirs mine\nthree\n", 3);
  assert_eq!(local_subjects(&clone, &upstream), "(auth) Add b\n(auth) Change line 2\n(auth) Add a");
}

/// The same rename on both sides merges cleanly; a rename on one side and a delete on the other is a round that a
/// take of either path resolves
#[test_log::test]
fn a_rename_and_a_delete_resolve_with_a_take() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("x.txt", "1\n2\n3\n4\n5\n6\n7\n8\n"), ("other.txt", "other\n")]);
  git(&upstream, &["mv", "x.txt", "y.txt"]);
  upstream.create_commit("Rename x", "other.txt", "other, upstream\n");
  git(&clone, &["mv", "x.txt", "y.txt"]);
  clone.create_commit("(auth) Rename x and change it", "y.txt", "1\nmine\n3\n4\n5\n6\n7\n8\n");
  let (exit_code, done) = run_in(&clone, &["update"]);
  assert_eq!((exit_code, &done["state"]), (0, &json!("rebase")), "{done}");

  for (take, kept) in [("onto:x.txt", true), ("commit:y.txt", false)] {
    let (upstream, clone) = TestRepo::upstream_and_clone(&[("x.txt", "1\n2\n3\n4\n5\n6\n7\n8\n"), ("other.txt", "other\n")]);
    git(&upstream, &["mv", "x.txt", "y.txt"]);
    upstream.create_commit("Rename x", "other.txt", "other, upstream\n");
    git(&clone, &["rm", "-q", "x.txt"]);
    clone.create_commit("(auth) Delete x", "other.txt", "other, mine\n");
    let (exit_code, round) = run_in(&clone, &["update"]);
    assert_eq!(exit_code, 1, "{round}");
    let file = file_of(&round, "y.txt");
    assert_eq!((&file["kind"], &file["renamed_from"]), (&json!("rename_delete"), &json!("x.txt")), "{round}");
    // other.txt conflicts too
    std::fs::write(edit_of(&round, "other.txt"), "other, upstream and mine\n").unwrap();
    let (exit_code, done) = run_in(&clone, &["update", "--continue", "--take", take]);
    assert_eq!(exit_code, 0, "{take}: {done}");
    assert_eq!(clone.path().join("y.txt").exists(), kept, "{take}");
    assert!(!clone.path().join("x.txt").exists(), "{take}");
  }
}

/// Every refusal names the round's commit, and says the round and the copies are as they were
#[test_log::test]
fn a_refusal_names_the_commit_and_keeps_the_round() {
  let (_upstream, clone) = conflicting_clone();
  let (_, round) = run_in(&clone, &["update"]);
  let commit = round["session"]["commit"]["commit"].as_str().unwrap().to_string();
  std::fs::write(edit_of(&round, "shared.txt"), "ONE\ntheirs mine\nthree\n").unwrap();
  let (exit_code, output) = run_in(&clone, &["update", "--continue"]);
  assert_error(&output, exit_code, (2, "resolution_outside_conflict"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.starts_with(&format!("Round 1, commit {commit} (auth) Change line 2:")), "{message}");
  assert!(message.contains("Nothing changed: round 1 stays open, and the edit copies keep your edits"), "{message}");
  assert_eq!(output["error"]["commits"], json!([commit]));
  // the status says the copy holds the agent's edit
  let (_, status) = run_in(&clone, &["update", "--status"]);
  assert_eq!(
    (&status["session"]["round"], &status["session"]["files"][0]["edited"]),
    (&json!(1), &json!(true)),
    "{status}"
  );
  assert_eq!(read(&edit_of(&round, "shared.txt")), "ONE\ntheirs mine\nthree\n");
}

/// Synthetic cases shaped by a field run: a `Date:` line both sides changed, a deleted test whose upstream edit belongs
/// in another file now, and the same line added on both sides
#[test_log::test]
fn field_shaped_conflicts_resolve_as_the_hints_say() {
  // a spec whose `Date:` line both sides changed: a content round, resolved by an edit
  let spec = "# Spec\n\nStatus: Draft\nDate: 2026-01-01\n\nText.\n";
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("spec.md", spec)]);
  clone.create_commit("(spec) Update the spec", "spec.md", &spec.replace("2026-01-01", "2026-03-01"));
  upstream.create_commit("Update the spec upstream", "spec.md", &spec.replace("2026-01-01", "2026-02-01"));
  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 1, "{round}");
  assert_eq!(file_of(&round, "spec.md")["kind"], json!("content"), "{round}");
  std::fs::write(edit_of(&round, "spec.md"), spec.replace("2026-01-01", "2026-03-01")).unwrap();
  let (exit_code, done) = run_in(&clone, &["update", "--continue"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(done["resolutions"][0]["files"][0]["dropped_upstream_lines"], json!(["Date: 2026-02-01"]), "{done}");

  // the commit deletes a test that the upstream edited; the edit belongs in the new test of the commit
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("old_test.txt", "test one\ntest two\n")]);
  git(&clone, &["rm", "-q", "old_test.txt"]);
  // rewritten, so no rename is found: a rename would carry the upstream edit into the new file by itself
  clone.create_commit("(tests) Move the tests", "new_test.txt", "fn tests() {\n  check(1);\n  check(2);\n}\n");
  upstream.create_commit("Fix test two", "old_test.txt", "test one\ntest two, fixed\n");
  let (exit_code, round) = run_in(&clone, &["update"]);
  assert_eq!(exit_code, 1, "{round}");
  let hint = file_of(&round, "old_test.txt")["hint"].as_str().unwrap().to_string();
  assert!(hint.contains("--take commit:old_test.txt") && hint.contains("`fixup!` commit after the update"), "{hint}");
  let (exit_code, done) = run_in(&clone, &["update", "--continue", "--take", "commit:old_test.txt"]);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(done["resolutions"][0]["files"][0]["dropped_upstream_lines"], json!(["test two, fixed"]), "{done}");
  assert!(!clone.path().join("old_test.txt").exists());

  // the same line added on both sides merges cleanly: no round, no block of identical sides
  let build = "deps = [\n  \"a\",\n  \"c\",\n]\n";
  let added = build.replace("  \"c\",", "  \"b\",\n  \"c\",");
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("BUILD", build)]);
  clone.create_commit("(build) Add b", "BUILD", &added);
  upstream.create_commit("Add b upstream", "BUILD", &added);
  upstream.create_commit("Add d upstream", "d.txt", "d\n");
  let (exit_code, done) = run_in(&clone, &["update"]);
  assert_eq!((exit_code, &done["state"]), (0, &json!("rebase")), "{done}");
}

/// A clone whose `(auth)` commit changes the first word of line 2 and an upstream that changes its last word: git
/// conflicts, the words resolve
fn words_clone() -> (TestRepo, TestRepo) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\nalpha beta gamma\nthree\n"), ("other.txt", "other\n")]);
  clone.create_commit("(auth) Change alpha", "shared.txt", "one\nALPHA beta gamma\nthree\n");
  upstream.create_commit("Change gamma upstream", "shared.txt", "one\nalpha beta GAMMA\nthree\n");
  (upstream, clone)
}

#[test_log::test]
fn an_update_whose_conflict_resolves_by_words_applies_and_names_the_commit() {
  let (upstream, clone) = words_clone();
  let local = clone.head();
  git(&clone, &["fetch", "-q"]);
  let (exit_code, preview) = run_in(&clone, &["--full-ids", "update", "--dry-run"]);
  assert_eq!((exit_code, &preview["state"]), (0, &json!("rebase")), "{preview}");
  assert_eq!(
    preview["resolved"],
    json!([{ "commit": local, "conflicts": 1, "file_count": 1, "files": [{ "path": "shared.txt", "conflicts": 1 }] }])
  );

  let (exit_code, done) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!((exit_code, &done["state"]), (0, &json!("rebase")), "{done}");
  let new_id = clone.head();
  assert_eq!(
    done["resolved"],
    json!([{ "commit": local, "new_id": new_id, "conflicts": 1, "file_count": 1, "files": [{ "path": "shared.txt", "conflicts": 1 }] }])
  );
  assert_eq!(done.get("session"), None, "{done}");
  assert_eq!(done["verify"], json!({ "paths": ["shared.txt"] }), "nobody saw the merge of the words: {done}");
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), upstream.head());
  assert_eq!(read(clone.path().join("shared.txt").to_str().unwrap()), "one\nALPHA beta GAMMA\nthree\n");
  assert!(git(&clone, &["notes", "--ref=refs/notes/branch-deck/resolve", "show", "HEAD"]).contains("words\tshared.txt"));
}

#[test_log::test]
fn a_round_that_resolves_by_words_is_named_in_the_next_round_the_status_and_the_finish() {
  let (upstream, clone) = words_clone();
  let words = clone.head();
  // a second commit with a conflict that the words leave
  clone.create_commit("(auth) Change other", "other.txt", "mine\n");
  upstream.create_commit("Change other upstream", "other.txt", "theirs\n");

  let (exit_code, round) = run_in(&clone, &["--full-ids", "update"]);
  assert_eq!((exit_code, &round["session"]["round"], &round["session"]["replayed"]), (1, &json!(2), &json!(1)), "{round}");
  assert_eq!(round["session"]["commit"]["subject"], json!("(auth) Change other"));
  let resolved = &round["session"]["resolved"];
  assert_eq!(
    (&resolved[0]["round"], &resolved[0]["commit"], &resolved[0]["conflicts"]),
    (&json!(1), &json!(words), &json!(1)),
    "{round}"
  );

  let (_, status) = run_in(&clone, &["--full-ids", "update", "--status"]);
  assert_eq!(status["session"]["resolved"], *resolved, "{status}");
  let first = &status["session"]["resolutions"][0];
  assert_eq!((&first["round"], &first["words"]), (&json!(1), &json!(1)), "{status}");
  assert_eq!(first["files"][0], json!({ "path": "shared.txt", "how": "words", "conflicts": 1 }));

  std::fs::write(edit_of(&round, "other.txt"), "theirs mine\n").unwrap();
  let (exit_code, done) = run_in(&clone, &["--full-ids", "update", "--continue", "--round", "2"]);
  assert_eq!((exit_code, done["resolutions"].as_array().map(Vec::len)), (0, Some(2)), "{done}");
  assert_eq!(done["resolved"][0]["round"], json!(1), "{done}");
  assert_eq!(done["resolutions"][1]["words"], Value::Null, "{done}");
  assert_eq!(read(clone.path().join("shared.txt").to_str().unwrap()), "one\nALPHA beta GAMMA\nthree\n");
}

/// `bd update` of `clone` (its round on `shared.txt`), the copy resolved, then `bd update --continue` under the git that
/// logs the state of the lock: the wrapper, the exit code and the output of the continue
#[cfg(unix)]
fn continue_logged(clone: &TestRepo) -> (bd_cli::fixture_test::LockSpanGit, i32, Value) {
  let (_, round) = run_in(clone, &["update"]);
  std::fs::write(edit_of(&round, "shared.txt"), "one\ntheirs mine\nthree\n").unwrap();
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(clone, "");
  let (exit_code, done) = wrapped.run(&["update", "--continue"]);
  (wrapped, exit_code, done)
}

/// The finish of a session releases the repository lock once the branch, the files and the session refs moved: what
/// the update landed is a report, read and written (landed.json) without the lock, as on the clean path of an update
#[cfg(unix)]
#[test_log::test]
fn the_landed_report_of_a_finished_session_runs_after_the_lock() {
  let (upstream, clone) = conflicting_clone();
  let (wrapped, exit_code, done) = continue_logged(&clone);
  assert_eq!((exit_code, &done["replayed"]), (0, &json!(1)), "{done}");
  assert_eq!(git(&clone, &["rev-parse", "HEAD~1"]), upstream.head());
  let commands = wrapped.commands();
  let finished = commands
    .iter()
    .position(|(_, args)| args.contains(" update-ref -m branch-deck: resolve session finished "))
    .unwrap_or_else(|| panic!("the finish moves the session refs: {commands:#?}"));
  assert!(commands[finished].0, "the finish under the lock: {commands:#?}");
  let report = &commands[finished + 1..];
  // the landed candidates of the report: its commits by their author in the upstream range (`find_landed`)
  assert!(
    report.iter().any(|(_, args)| args.contains(" --fixed-strings --author=")),
    "the landed report: {commands:#?}"
  );
  assert!(report.iter().all(|(locked, _)| !*locked), "nothing after the finish under the lock: {commands:#?}");
}

/// The subcommands of the last run of `wrapped` under the repository lock, sorted, each once
#[cfg(unix)]
fn under_the_lock(wrapped: &bd_cli::fixture_test::LockSpanGit) -> Vec<String> {
  let mut under: Vec<String> = wrapped.under_lock().iter().map(|args| bd_cli::fixture_test::subcommand(args).to_string()).collect();
  under.sort_unstable();
  under.dedup();
  under
}

/// What `bd update --continue` runs under the repository lock, as it is: the read of the session and of the copies, the
/// commit of the round, the replay of the rest and the finish (its exact check, the move of the branch and the files,
/// the session refs); only the landed report after it. The one core of bd that computes under the lock
/// (docs/design/git-operations.md, "Lock spans")
#[cfg(unix)]
#[test_log::test]
fn a_continue_of_a_resolve_session_holds_the_lock_for_its_round_and_its_finish() {
  let (upstream, clone) = conflicting_clone();
  // a commit above the conflict: the continue replays it
  clone.create_commit("(auth) Add more", "more.txt", "more\n");
  let (wrapped, exit_code, done) = continue_logged(&clone);
  assert_eq!(exit_code, 0, "{done}");
  assert_eq!(git(&clone, &["rev-parse", "HEAD~2"]), upstream.head());
  let round_and_finish = [
    "cat-file",
    "config",
    "diff-tree",
    "fast-import",
    "read-tree",
    "replay",
    "rev-parse",
    "symbolic-ref",
    "update-index",
    "update-ref",
  ];
  assert_eq!(under_the_lock(&wrapped), round_and_finish, "{:#?}", wrapped.commands());
  let replays = wrapped.locked_runs("replay");
  assert!(
    !replays.is_empty() && replays.iter().all(|locked| *locked),
    "the replay of the rest under the lock: {:#?}",
    wrapped.commands()
  );
}

/// `bd update --abort` holds the repository lock for the read of the session and its delete: the compare-and-swap of
/// the session ref and the sweep of stale temporary refs (docs/design/git-operations.md, "Lock spans")
#[cfg(unix)]
#[test_log::test]
fn an_abort_of_a_resolve_session_locks_only_the_read_of_the_session_and_its_delete() {
  let (_upstream, clone) = conflicting_clone();
  let head = clone.head();
  let (_, round) = run_in(&clone, &["update"]);
  assert!(round["session"].is_object(), "{round}");
  let wrapped = bd_cli::fixture_test::LockSpanGit::new(&clone, "");
  let (exit_code, aborted) = wrapped.run(&["update", "--abort"]);
  assert_eq!(exit_code, 0, "{aborted}");
  assert_eq!(clone.head(), head);
  assert_eq!(under_the_lock(&wrapped), ["cat-file", "for-each-ref", "update-ref"], "{:#?}", wrapped.commands());
}
