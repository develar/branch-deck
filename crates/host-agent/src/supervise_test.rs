use super::*;
use crate::run::Runs;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;
use sync_utils::fs_retry;
use test_utils::git_test_utils::TestRepo;

fn lines(input: &[u8]) -> Vec<String> {
  let mut lines = Vec::new();
  read_lines(input, &mut |line| lines.push(line.to_owned()));
  lines
}

#[test]
fn lines_lose_their_ends_and_a_last_line_without_an_end_comes_too() {
  assert_eq!(lines(b"one\ntwo\r\n\nthree"), vec!["one", "two", "", "three"]);
  assert_eq!(lines(b""), Vec::<String>::new());
}

#[test]
fn a_long_line_comes_in_pieces() {
  let mut input = vec![b'x'; MAX_EVENT_TEXT * 2 + 5];
  input.push(b'\n');
  input.extend_from_slice(b"next\n");
  let pieces = lines(&input);
  assert_eq!(pieces.iter().map(String::len).collect::<Vec<_>>(), vec![MAX_EVENT_TEXT, MAX_EVENT_TEXT, 5, 4]);
  let exact = [vec![b'y'; MAX_EVENT_TEXT], b"\n".to_vec()].concat();
  assert_eq!(lines(&exact).len(), 1, "a line of exactly the limit is one event");
}

#[test]
fn an_exit_code_of_a_signal_is_128_plus_its_number() {
  let status = Command::new("/bin/sh").args(["-c", "kill -TERM $$"]).status().expect("ran");
  assert_eq!(exit_code(status), 128 + libc::SIGTERM);
  let status = Command::new("/bin/sh").args(["-c", "exit 4"]).status().expect("ran");
  assert_eq!(exit_code(status), 4);
}

#[test]
fn only_the_new_entries_of_a_fetch_directory_go_to_the_stage() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let slot = dir.path().join("slot");
  let runs = slot.join("out/bt-runs");
  fs::create_dir_all(runs.join("old")).expect("an old run");
  let fetch = vec!["out/bt-runs".to_owned(), "out/missing".to_owned()];
  let before: Vec<_> = fetch.iter().map(|dir| entries(&slot.join(dir))).collect();
  fs::create_dir_all(runs.join("new/sub")).expect("a new run");
  fs::write(runs.join("new/sub/report.xml"), "<x/>").expect("a report");
  fs::write(runs.join("loose.txt"), "t").expect("a file");
  let stage = dir.path().join("stage");
  assert_eq!(stage_new_entries(&slot, &fetch, &before, &stage), Vec::<String>::new());
  assert_eq!(fs::read_to_string(stage.join("out/bt-runs/new/sub/report.xml")).expect("staged"), "<x/>");
  assert!(stage.join("out/bt-runs/loose.txt").is_file());
  assert!(!stage.join("out/bt-runs/old").exists());
  assert!(runs.join("old").is_dir());
  assert!(!runs.join("new").exists());
  assert!(!stage.join("out/missing").exists());
}

#[test]
fn a_tree_copy_keeps_files_dirs_and_links() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let source = dir.path().join("source");
  fs::create_dir_all(source.join("a")).expect("a directory");
  fs::write(source.join("a/f"), "f").expect("a file");
  std::os::unix::fs::symlink("a/f", source.join("link")).expect("a link");
  let target: PathBuf = dir.path().join("target");
  copy_tree(&source, &target).expect("copied");
  assert_eq!(fs::read_to_string(target.join("a/f")).expect("the file"), "f");
  assert_eq!(fs::read_link(target.join("link")).expect("the link"), PathBuf::from("a/f"));
}

fn spec_of(commit: &str) -> RunSpec {
  RunSpec {
    run: "1-1".to_owned(),
    repo: "~/repo".to_owned(),
    affinity: "h".to_owned(),
    commit: commit.to_owned(),
    command: vec!["true".to_owned()],
    fetch: vec![],
    ..RunSpec::default()
  }
}

/// A repository with one commit, and a later commit whose object is hidden, as if its pack still travelled. Answers
/// the directory, the repository, the hidden commit, its object path and its hiding place
fn repo_with_a_travelling_commit() -> (tempfile::TempDir, TestRepo, String, PathBuf, PathBuf) {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let repo = TestRepo::new();
  let first = repo.create_commit("first", "a.txt", "a");
  let tree = repo.git(&["rev-parse", &format!("{first}^{{tree}}")]);
  let later = repo.git(&["commit-tree", tree.trim(), "-p", &first, "-m", "later"]).trim().to_owned();
  let object = repo.path().join(".git/objects").join(&later[..2]).join(&later[2..]);
  let hidden = dir.path().join("hidden");
  fs_retry::rename(&object, &hidden).expect("the object hidden");
  (dir, repo, later, object, hidden)
}

/// Whether a commit is in `repo`: a loose object of the commit
fn has(repo: &Path) -> impl Fn(&str) -> Result<bool, Refusal> {
  let objects = repo.join(".git/objects");
  move |commit| Ok(objects.join(&commit[..2]).join(&commit[2..]).is_file())
}

fn limits(deadline_ms: u64, stall_ms: u64) -> PackLimits {
  PackLimits {
    deadline: Duration::from_millis(deadline_ms),
    stall: Duration::from_millis(stall_ms),
  }
}

#[test]
fn the_pack_wait_ends_at_once_for_a_present_commit_at_its_deadline_at_a_stall_or_at_a_stop() {
  let (dir, repo, missing, _, _) = repo_with_a_travelling_commit();
  let present = repo.head();
  let (run, _lock) = Runs::new(dir.path().join("runs")).create(&spec_of(&missing)).expect("a run");
  let mut said = Vec::new();
  let mut wait = |commit: &str, limits: PackLimits, stop: &dyn Fn() -> bool| {
    wait_for_pack(&run, repo.path(), &spec_of(commit), limits, stop, &mut |line| said.push(line.to_owned()), &has(repo.path()))
  };

  // a commit that the host has already needs no pack.
  let started = Instant::now();
  assert!(wait(&present, limits(30_000, 30_000), &|| false).expect("no refusal"));
  assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());

  for (limits, failure) in [
    (limits(300, 30_000), "did not come in 0.3 s"),
    (limits(30_000, 300), "sent no chunk for 0.3 s, so its client is gone"),
  ] {
    let refusal = wait(&missing, limits, &|| false).expect_err(failure);
    assert_eq!(refusal.code, Code::PackMissing, "{failure}");
    assert!(refusal.message.starts_with(&format!("the pack of the run 1-1 {failure}")), "{failure}: {}", refusal.message);
  }

  assert!(!wait(&missing, limits(30_000, 30_000), &|| true).expect("no refusal"));
  assert_eq!(said, Vec::<String>::new(), "a short wait says nothing");
}

/// The chunks of a pack keep the wait alive past its stall time, and the mark of the landed pack ends it. A commit that
/// landed without its mark ends it at the stall too
#[test]
fn the_chunks_of_a_pack_keep_the_wait_alive_and_its_mark_ends_it() {
  let (dir, repo, later, object, hidden) = repo_with_a_travelling_commit();
  let (run, _lock) = Runs::new(dir.path().join("runs")).create(&spec_of(&later)).expect("a run");
  let mut said = Vec::new();
  let started = Instant::now();
  let waited = thread::scope(|scope| {
    scope.spawn(|| {
      while started.elapsed() < Duration::from_millis(1500) {
        run.note_pack_progress().expect("a chunk noted");
        thread::sleep(Duration::from_millis(100));
      }
      run.mark_packed().expect("the pack landed");
    });
    wait_for_pack(
      &run,
      repo.path(),
      &spec_of(&later),
      limits(30_000, 500),
      &|| false,
      &mut |line| said.push(line.to_owned()),
      &has(repo.path()),
    )
  });
  assert!(waited.expect("no refusal"));
  assert!(started.elapsed() >= Duration::from_millis(1500), "{:?}", started.elapsed());
  assert_eq!(said, vec![format!("waiting for the pack of {later}")]);

  let (other, _lock) = Runs::new(dir.path().join("runs"))
    .create(&RunSpec {
      run: "1-2".to_owned(),
      ..spec_of(&later)
    })
    .expect("another run");
  let restore = thread::spawn(move || {
    thread::sleep(Duration::from_millis(100));
    fs_retry::rename(&hidden, &object).expect("the object back");
  });
  let waited = wait_for_pack(&other, repo.path(), &spec_of(&later), limits(30_000, 400), &|| false, &mut |_| {}, &has(repo.path()));
  restore.join().expect("the restore");
  assert!(waited.expect("the commit came without a mark"));
}
