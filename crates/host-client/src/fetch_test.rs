use super::*;
use crate::record::RunFiles;
use crate::testkit::{ALIAS, FakeProcesses, Setup, calm};
use pretty_assertions::assert_eq;

fn fetch() -> Vec<String> {
  vec!["out/bt-runs".to_owned()]
}

/// An agent whose fetch sends `files`: each a path, a kind, the size that it names, and the bytes that it sends
fn agent_sending(files: Vec<(&'static str, FileKind, u64, Vec<u8>)>) -> Setup {
  Setup::new(move |request, channel| {
    if let Request::Fetch { .. } = request {
      for (path, kind, size, bytes) in &files {
        let _ = channel.send(&Reply::File {
          path: (*path).to_owned(),
          size: *size,
          executable: false,
          kind: kind.clone(),
        });
        for chunk in bytes.chunks(64 * 1024) {
          let _ = channel.send_bytes(chunk);
        }
        let _ = channel.send_end();
      }
      let _ = channel.send(&Reply::Fetched { files: 0, bytes: 0 });
    } else {
      let _ = channel.send(&Reply::refused(Code::BadRequest, "not in this script"));
    }
  })
}

/// An agent that stages, for each fetch, one file inside `out/bt-runs` and one outside, a file of the output `src`,
/// and the changes
fn agent_of_one_run() -> Setup {
  let one = |path: &'static str, kind: FileKind| (path, kind, 1, b"x".to_vec());
  agent_sending(vec![
    one("out/bt-runs/1/a.log", FileKind::Staged),
    one("scripts/hook.sh", FileKind::Staged),
    one("src/main.rs", FileKind::Output { root: "src".to_owned() }),
    one("changes.patch", FileKind::Changes),
  ])
}

/// The run files of the checkout of `setup`
fn run_files(setup: &Setup) -> RunFiles {
  RunFiles::new(&setup.repo.path().join(".git/branch-deck/host/runs")).unwrap()
}

/// Fetches the run `1-1` with `record`, in the checkout of `setup` that `local` changes first
fn fetch_with(setup: &Setup, record: Option<&RunRecord>, fetch: &[String], local: impl FnOnce(&Path)) -> Result<Vec<String>, Refusal> {
  local(setup.repo.path());
  if let Some(record) = record {
    run_files(setup).write("1-1", record)?;
  }
  let processes = FakeProcesses::default();
  setup.client(&processes, &|_| {}, &calm).fetch(ALIAS, "1-1", setup.repo.path(), fetch)
}

/// A local `src/main.rs`, which a fetch must not replace unless the record asks for the output `src`
fn with_src(root: &Path) {
  std::fs::create_dir_all(root.join("src")).unwrap();
  std::fs::write(root.join("src/main.rs"), "mine").unwrap();
}

/// A fetch takes only what the record of the run names: the staged files below its directories, the outputs that it
/// names and its changes. A path outside fails the fetch with `fetch_failed`
#[test]
fn a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link() {
  let setup = agent_of_one_run();
  let record = RunRecord {
    commit: "c".to_owned(),
    fetch: fetch(),
    outputs: vec!["results".to_owned()],
    changes: true,
  };
  let refusal = fetch_with(&setup, Some(&record), &[], with_src).unwrap_err();
  let root = setup.repo.path();
  assert_eq!(refusal.code, Code::FetchFailed);
  assert!(
    refusal.message.contains("2 of 4 files") && refusal.message.contains("scripts/hook.sh, src/main.rs"),
    "{}",
    refusal.message
  );
  assert!(root.join("out/bt-runs/1/a.log").is_file());
  assert!(!root.join("scripts/hook.sh").exists());
  assert_eq!(std::fs::read_to_string(root.join("src/main.rs")).unwrap(), "mine");
  assert_eq!(std::fs::read_to_string(run_files(&setup).patch("1-1")).unwrap(), "x");
}

/// Without a record, an explicit `--fetch` takes the staged files only, and no `--fetch` refuses before a connection
#[test]
fn a_fetch_without_a_record_takes_the_staged_files_of_its_directories_only() {
  let setup = agent_of_one_run();
  let refusal = fetch_with(&setup, None, &[], with_src).unwrap_err();
  assert_eq!(refusal.code, Code::Usage);
  assert!(refusal.message.contains("has no record of the run 1-1"), "{}", refusal.message);
  assert!(setup.agent.requests().is_empty(), "{:?}", setup.agent.requests());

  let refusal = fetch_with(&setup, None, &fetch(), |_| {}).unwrap_err();
  assert!(refusal.message.contains("3 of 4 files"), "{}", refusal.message);
  assert!(setup.repo.path().join("out/bt-runs/1/a.log").is_file());
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("src/main.rs")).unwrap(), "mine");
  assert!(!run_files(&setup).patch("1-1").exists());

  let processes = FakeProcesses::default();
  let refusal = setup.client(&processes, &|_| {}, &calm).fetch(ALIAS, "../1-1", setup.repo.path(), &fetch()).unwrap_err();
  assert_eq!(refusal.code, Code::Usage);
}

/// The first file of an output removes its local copy, and the second one keeps the first
#[test]
fn the_first_file_of_an_output_replaces_its_local_copy() {
  let results = || FileKind::Output { root: "results".to_owned() };
  let setup = agent_sending(vec![
    ("results/new.txt", results(), 3, b"new".to_vec()),
    ("results/sub/deep.txt", results(), 4, b"deep".to_vec()),
  ]);
  let record = RunRecord {
    outputs: vec!["results".to_owned()],
    ..RunRecord::default()
  };
  let fetched = fetch_with(&setup, Some(&record), &[], |root| {
    std::fs::create_dir_all(root.join("results")).unwrap();
    std::fs::write(root.join("results/old.txt"), "old").unwrap();
  })
  .unwrap();
  let root = setup.repo.path();
  assert_eq!(fetched, ["results/new.txt", "results/sub/deep.txt"]);
  assert!(!root.join("results/old.txt").exists(), "the output of the run replaced the local one");
  assert_eq!(std::fs::read_to_string(root.join("results/new.txt")).unwrap(), "new");
  assert_eq!(std::fs::read_to_string(root.join("results/sub/deep.txt")).unwrap(), "deep");
}

fn staged_record() -> RunRecord {
  RunRecord {
    fetch: fetch(),
    outputs: vec!["out/results".to_owned()],
    ..RunRecord::default()
  }
}

/// A local link in a parent directory would take a write or the removal of an output outside the checkout, so the
/// fetch refuses the file. The directory that the link names stays as it was
#[cfg(unix)]
#[test]
fn a_link_in_a_parent_directory_takes_no_file_and_removes_nothing() {
  let outside = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(outside.path().join("results")).unwrap();
  std::fs::write(outside.path().join("results/keep.txt"), "keep").unwrap();
  let setup = agent_sending(vec![
    ("out/bt-runs/1/a.log", FileKind::Staged, 1, b"x".to_vec()),
    ("out/results/new.txt", FileKind::Output { root: "out/results".to_owned() }, 1, b"x".to_vec()),
  ]);
  let refusal = fetch_with(&setup, Some(&staged_record()), &[], |root| {
    std::os::unix::fs::symlink(outside.path(), root.join("out")).unwrap();
  })
  .unwrap_err();
  assert!(refusal.message.contains("2 of 2 files"), "{}", refusal.message);
  assert_eq!(std::fs::read_to_string(outside.path().join("results/keep.txt")).unwrap(), "keep");
  assert!(!outside.path().join("bt-runs").exists());
  assert!(!outside.path().join("results/new.txt").exists());
}

/// A part that a link names would write through the link. The fetch removes the link, and the file it names stays
#[cfg(unix)]
#[test]
fn a_planted_part_link_is_removed_and_its_target_stays() {
  let outside = tempfile::tempdir().unwrap();
  let victim = outside.path().join("victim");
  std::fs::write(&victim, "victim").unwrap();
  let setup = agent_sending(vec![("out/bt-runs/1/a.log", FileKind::Staged, 3, b"new".to_vec())]);
  let fetched = fetch_with(&setup, Some(&staged_record()), &[], |root| {
    let dir = root.join("out/bt-runs/1");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&victim, dir.join("a.log.bd-host-part")).unwrap();
  })
  .unwrap();
  let root = setup.repo.path();
  assert_eq!(fetched, ["out/bt-runs/1/a.log"]);
  assert_eq!(std::fs::read_to_string(&victim).unwrap(), "victim");
  assert_eq!(std::fs::read_to_string(root.join("out/bt-runs/1/a.log")).unwrap(), "new");
  let part = std::fs::symlink_metadata(root.join("out/bt-runs/1/a.log.bd-host-part"));
  assert_eq!(part.map_err(|error| error.kind()).err(), Some(io::ErrorKind::NotFound));
}

/// A stream over its size writes no more than its size, and the next file of the fetch still comes
#[test]
fn a_stream_over_its_size_writes_no_more_and_the_next_file_comes() {
  let setup = agent_sending(vec![
    ("out/bt-runs/1/big.log", FileKind::Staged, 4, vec![b'x'; 300 * 1024]),
    ("out/bt-runs/1/small.log", FileKind::Staged, 2, b"ok".to_vec()),
  ]);
  let refusal = fetch_with(&setup, Some(&staged_record()), &[], |_| {}).unwrap_err();
  let root = setup.repo.path();
  assert!(
    refusal.message.contains("1 of 2 files") && refusal.message.contains("out/bt-runs/1/big.log"),
    "{}",
    refusal.message
  );
  assert!(!root.join("out/bt-runs/1/big.log").exists());
  assert!(!root.join("out/bt-runs/1/big.log.bd-host-part").exists());
  assert_eq!(std::fs::read_to_string(root.join("out/bt-runs/1/small.log")).unwrap(), "ok");
}

#[test]
fn a_capped_writer_passes_its_size_and_drops_the_rest() {
  let mut capped = Capped { inner: Vec::new(), left: 5 };
  capped.write_all(b"abc").unwrap();
  capped.write_all(b"defgh").unwrap();
  assert_eq!(capped.inner, b"abcde");
}

#[test]
fn a_fetched_path_stays_below_a_fetch_directory_of_the_checkout() {
  let fetch = fetch();
  let root = Path::new("/c");
  assert_eq!(
    local_target(root, "out/bt-runs/1/a.log", &fetch).unwrap(),
    root.join("out").join("bt-runs").join("1").join("a.log")
  );
  for path in [
    "/etc/passwd",
    "out/bt-runs/../x",
    "out/bt-runs",
    "out/other/a",
    "out//bt-runs/a",
    "out/bt-runs/c:\\x",
    "out/bt-runs/1/a.log.bd-host-part",
  ] {
    local_target(root, path, &fetch).expect_err(path);
  }
}

/// A host that sends a hook or a git config would run code here, so no path inside a `.git` directory comes, in any
/// spelling that a file system of the client reads as `.git`. A path without fetch directories comes nowhere
#[test]
fn no_path_goes_into_a_git_directory_or_comes_without_fetch_directories() {
  let root = Path::new("/c");
  let everything = vec!["out".to_owned()];
  for path in [
    "out/.git/hooks/post-checkout",
    "out/.GIT/config",
    "out/.Git./config",
    "out/.git /config",
    "out/GIT~1/config",
    "out/sub/.git",
  ] {
    assert_eq!(local_target(root, path, &everything).expect_err(path), "the path is inside a .git directory", "{path}");
  }
  assert_eq!(
    local_target(root, "out/.github/workflow.yml", &everything).unwrap(),
    root.join("out").join(".github").join("workflow.yml")
  );
  local_target(root, "a/b.txt", &[]).unwrap_err();
  assert_eq!(part_path(Path::new("/c/a.log")), Path::new("/c/a.log.bd-host-part"));
}
