use super::*;
use crate::record::RunFiles;
use crate::testkit::{ALIAS, FakeProcesses, Setup, calm, leak};
use git_ops::lease_slot::STATE_DIR;
use pretty_assertions::assert_eq;

fn fetch() -> Vec<String> {
  vec!["out/bt-runs".to_owned()]
}

/// An agent whose fetch sends `files`: each a path, a kind, the size that it names, and the bytes that it sends
fn agent_sending(files: Vec<(&'static str, FileKind, u64, Vec<u8>)>) -> Setup {
  let files: &'static [(&'static str, FileKind, u64, Vec<u8>)] = leak(files);
  Setup::new(move |request, channel| {
    let fetch = matches!(request, Request::Fetch { .. });
    Box::pin(async move {
      if !fetch {
        let _ = channel.send(&Reply::refused(Code::BadRequest, "not in this script")).await;
        return;
      }
      for (path, kind, size, bytes) in files {
        let _ = channel
          .send(&Reply::File {
            path: (*path).to_owned(),
            size: *size,
            executable: false,
            kind: kind.clone(),
          })
          .await;
        for chunk in bytes.chunks(64 * 1024) {
          let _ = channel.send_bytes(chunk).await;
        }
        let _ = channel.send_end().await;
      }
      let _ = channel.send(&Reply::Fetched { files: 0, bytes: 0 }).await;
    })
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
  RunFiles::new(&setup.repo.path().join(".git").join(STATE_DIR).join("host/runs")).unwrap()
}

/// Fetches the run `1-1` with `record`, in the checkout of `setup` that `local` changes first
async fn fetch_with(setup: &Setup, record: Option<&RunRecord>, fetch: &[String], local: impl FnOnce(&Path)) -> Result<Vec<String>, Refusal> {
  local(setup.repo.path());
  if let Some(record) = record {
    run_files(setup).write("1-1", record)?;
  }
  let processes = FakeProcesses::default();
  setup.client(&processes, &|_| {}, &calm).fetch(ALIAS, "1-1", setup.repo.path(), fetch).await
}

/// A local `src/main.rs`, which a fetch must not replace unless the record asks for the output `src`
fn with_src(root: &Path) {
  std::fs::create_dir_all(root.join("src")).unwrap();
  std::fs::write(root.join("src/main.rs"), "mine").unwrap();
}

/// A fetch takes only what the record of the run names: the staged files below its directories, the outputs that it
/// names and its changes. A path outside fails the fetch with `fetch_failed`
#[tokio::test(start_paused = true)]
async fn a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link() {
  let setup = agent_of_one_run();
  let record = RunRecord {
    commit: "c".to_owned(),
    fetch: fetch(),
    outputs: vec!["results".to_owned()],
    changes: true,
  };
  let refusal = fetch_with(&setup, Some(&record), &[], with_src).await.unwrap_err();
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
#[tokio::test(start_paused = true)]
async fn a_fetch_without_a_record_takes_the_staged_files_of_its_directories_only() {
  let setup = agent_of_one_run();
  let refusal = fetch_with(&setup, None, &[], with_src).await.unwrap_err();
  assert_eq!(refusal.code, Code::Usage);
  assert!(refusal.message.contains("has no record of the run 1-1"), "{}", refusal.message);
  assert!(setup.agent.requests().is_empty(), "{:?}", setup.agent.requests());

  let refusal = fetch_with(&setup, None, &fetch(), |_| {}).await.unwrap_err();
  assert!(refusal.message.contains("3 of 4 files"), "{}", refusal.message);
  assert!(setup.repo.path().join("out/bt-runs/1/a.log").is_file());
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("src/main.rs")).unwrap(), "mine");
  assert!(!run_files(&setup).patch("1-1").exists());

  let processes = FakeProcesses::default();
  let refusal = setup
    .client(&processes, &|_| {}, &calm)
    .fetch(ALIAS, "../1-1", setup.repo.path(), &fetch())
    .await
    .unwrap_err();
  assert_eq!(refusal.code, Code::Usage);
}

/// The first file of an output removes its local copy, and the second one keeps the first
#[tokio::test(start_paused = true)]
async fn the_first_file_of_an_output_replaces_its_local_copy() {
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
  .await
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
#[tokio::test(start_paused = true)]
async fn a_link_in_a_parent_directory_takes_no_file_and_removes_nothing() {
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
  .await
  .unwrap_err();
  assert!(refusal.message.contains("2 of 2 files"), "{}", refusal.message);
  assert_eq!(std::fs::read_to_string(outside.path().join("results/keep.txt")).unwrap(), "keep");
  assert!(!outside.path().join("bt-runs").exists());
  assert!(!outside.path().join("results/new.txt").exists());
}

/// A part that a link names would write through the link. The fetch removes the link, and the file it names stays
#[tokio::test(start_paused = true)]
async fn a_planted_part_link_is_removed_and_its_target_stays() {
  let outside = tempfile::tempdir().unwrap();
  let victim = outside.path().join("victim");
  std::fs::write(&victim, "victim").unwrap();
  let setup = agent_sending(vec![("out/bt-runs/1/a.log", FileKind::Staged, 3, b"new".to_vec())]);
  let fetched = fetch_with(&setup, Some(&staged_record()), &[], |root| {
    let dir = root.join("out/bt-runs/1");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&victim, dir.join("a.log.bd-host-part")).unwrap();
  })
  .await
  .unwrap();
  let root = setup.repo.path();
  assert_eq!(fetched, ["out/bt-runs/1/a.log"]);
  assert_eq!(std::fs::read_to_string(&victim).unwrap(), "victim");
  assert_eq!(std::fs::read_to_string(root.join("out/bt-runs/1/a.log")).unwrap(), "new");
  let part = std::fs::symlink_metadata(root.join("out/bt-runs/1/a.log.bd-host-part"));
  assert_eq!(part.map_err(|error| error.kind()).err(), Some(io::ErrorKind::NotFound));
}

/// Fetches `notes.local/run.log` of the run `1-1` into the checkout of a fresh setup, whose `notes.local` and lease
/// notes `local` makes first: it gets the root and the notes of the leases of the repository. Answers the outcome and the
/// setup
async fn fetch_into_notes(local: impl FnOnce(&Path, &Path)) -> (Result<Vec<String>, Refusal>, Setup) {
  let setup = agent_sending(vec![("notes.local/run.log", FileKind::Staged, 3, b"log".to_vec())]);
  let notes = git_ops::lease_slot::legacy_notes_root(&setup.repo.path().join(".git"));
  record_notes(&notes.join("lease-1"), setup.repo.path());
  let fetched = fetch_with(&setup, None, &["notes.local".to_owned()], |root| local(root, &notes)).await;
  (fetched, setup)
}

/// `notes.local` of a lease links to its notes (here in the place of a brd before, in the state), and the logs of a run
/// go there by `--fetch notes.local`: the write goes through that link, as through no other
#[tokio::test(start_paused = true)]
async fn a_fetch_writes_through_notes_local_into_the_notes_of_a_lease_of_its_repository() {
  let mut notes_dir = PathBuf::new();
  let (fetched, _setup) = fetch_into_notes(|root, notes| {
    notes_dir = notes.join("lease-1");
    std::os::unix::fs::symlink(&notes_dir, root.join("notes.local")).unwrap();
  })
  .await;
  assert_eq!(fetched.unwrap(), ["notes.local/run.log"]);
  assert_eq!(std::fs::read_to_string(notes_dir.join("run.log")).unwrap(), "log");
}

/// The record of the notes `dir` of a lease, beside it, naming `slot` as the slot of the lease
fn record_notes(dir: &Path, slot: &Path) {
  std::fs::create_dir_all(dir).unwrap();
  let record = serde_json::json!({ "lease_id": "lease-1", "holder": "agent", "slot": slot, "leased_at": 1, "linked": true });
  std::fs::write(dir.with_extension("json"), serde_json::to_vec(&record).unwrap()).unwrap();
}

/// The notes of a lease in the pool directory (`<pool dir>/notes/<lease id>`) take the write of a fetch through
/// `notes.local` while their record names this checkout as the slot of the lease, and none while it names another
#[tokio::test(start_paused = true)]
async fn a_fetch_writes_through_notes_local_only_into_the_notes_of_the_lease_of_its_checkout() {
  let pool = tempfile::tempdir().unwrap();
  let notes = std::fs::canonicalize(pool.path()).unwrap().join("notes/lease-1");
  let link = |checkout: &Path| std::os::unix::fs::symlink(&notes, checkout.join("notes.local")).unwrap();

  let setup = agent_sending(vec![("notes.local/run.log", FileKind::Staged, 3, b"log".to_vec())]);
  record_notes(&notes, pool.path());
  let refusal = fetch_with(&setup, None, &["notes.local".to_owned()], link).await.unwrap_err();
  assert!(refusal.message.contains("1 of 1 files"), "{}", refusal.message);
  assert!(!notes.join("run.log").exists());

  let setup = agent_sending(vec![("notes.local/run.log", FileKind::Staged, 3, b"log".to_vec())]);
  record_notes(&notes, setup.repo.path());
  let fetched = fetch_with(&setup, None, &["notes.local".to_owned()], link).await;
  assert_eq!(fetched.unwrap(), ["notes.local/run.log"]);
  assert_eq!(std::fs::read_to_string(notes.join("run.log")).unwrap(), "log");
}

/// What makes `notes.local` of a case: it gets the root of the checkout and the notes of the leases of its repository
type MakeLink<'a> = dyn FnOnce(&Path, &Path) + 'a;

/// Any other link takes no write: one to a directory outside the notes of the repository, one to a link in them, one
/// whose target has `..`, one to the notes root itself, and one to the notes under another name than `notes.local` at
/// the root
#[tokio::test(start_paused = true)]
async fn a_link_that_is_no_notes_of_a_lease_of_the_repository_takes_no_write() {
  let outside = tempfile::tempdir().unwrap();
  let outside_dir = outside.path().to_path_buf();
  let cases: Vec<(&str, Box<MakeLink<'_>>)> = vec![
    (
      "another directory",
      Box::new(|root: &Path, _: &Path| std::os::unix::fs::symlink(&outside_dir, root.join("notes.local")).unwrap()),
    ),
    (
      "a link in the notes",
      Box::new(|root: &Path, notes: &Path| {
        std::os::unix::fs::symlink(notes.join("lease-1"), notes.join("lease-2")).unwrap();
        std::os::unix::fs::symlink(notes.join("lease-2"), root.join("notes.local")).unwrap();
      }),
    ),
    (
      "a target with ..",
      Box::new(|root: &Path, notes: &Path| std::os::unix::fs::symlink(notes.join("lease-1/../lease-1"), root.join("notes.local")).unwrap()),
    ),
    (
      "the notes root itself",
      Box::new(|root: &Path, notes: &Path| std::os::unix::fs::symlink(notes, root.join("notes.local")).unwrap()),
    ),
  ];
  for (case, local) in cases {
    let (fetched, setup) = fetch_into_notes(local).await;
    let refusal = fetched.unwrap_err();
    assert!(refusal.message.contains("1 of 1 files"), "{case}: {}", refusal.message);
    let notes = git_ops::lease_slot::legacy_notes_root(&setup.repo.path().join(".git"));
    assert!(!notes.join("lease-1/run.log").exists() && !notes.join("run.log").exists(), "{case}");
    assert!(!outside_dir.join("run.log").exists(), "{case}");
  }
  // the notes under another name: `logs` at the root, fetched as `logs`
  let setup = agent_sending(vec![("logs/run.log", FileKind::Staged, 3, b"log".to_vec())]);
  let notes = git_ops::lease_slot::legacy_notes_root(&setup.repo.path().join(".git"));
  record_notes(&notes.join("lease-1"), setup.repo.path());
  let refusal = fetch_with(&setup, None, &["logs".to_owned()], |root| {
    std::os::unix::fs::symlink(notes.join("lease-1"), root.join("logs")).unwrap();
  })
  .await
  .unwrap_err();
  assert!(refusal.message.contains("1 of 1 files"), "{}", refusal.message);
  assert!(!notes.join("lease-1/run.log").exists());
}

/// A link inside the notes of the lease, which an agent may make there, takes no write either: each directory below
/// `notes.local` is checked as below the root, so `notes.local/sub/run.log` with `sub` a link out of the notes is refused
#[tokio::test(start_paused = true)]
async fn a_link_inside_the_notes_of_a_lease_takes_no_write() {
  let outside = tempfile::tempdir().unwrap();
  let setup = agent_sending(vec![("notes.local/sub/run.log", FileKind::Staged, 3, b"log".to_vec())]);
  let notes = git_ops::lease_slot::legacy_notes_root(&setup.repo.path().join(".git"));
  record_notes(&notes.join("lease-1"), setup.repo.path());
  let refusal = fetch_with(&setup, None, &["notes.local".to_owned()], |root| {
    std::os::unix::fs::symlink(notes.join("lease-1"), root.join("notes.local")).unwrap();
    std::os::unix::fs::symlink(outside.path(), notes.join("lease-1/sub")).unwrap();
  })
  .await
  .unwrap_err();
  assert!(refusal.message.contains("1 of 1 files"), "{}", refusal.message);
  assert!(!outside.path().join("run.log").exists());
}

/// A relative target takes no write, though from the root of the checkout it names the notes of the lease: the link
/// that the pool makes is absolute, and a relative one is read by the directory of the link, not by the working
/// directory of the process. Here the working directory is the root, so the target resolves to the real notes
#[tokio::test(start_paused = true)]
async fn a_relative_link_to_the_notes_of_a_lease_takes_no_write() {
  let (fetched, setup) = fetch_into_notes(|root, _| {
    std::env::set_current_dir(root).unwrap();
    std::os::unix::fs::symlink(format!(".git/{STATE_DIR}/notes/lease-1"), root.join("notes.local")).unwrap();
  })
  .await;
  let refusal = fetched.unwrap_err();
  assert!(refusal.message.contains("1 of 1 files"), "{}", refusal.message);
  assert!(!git_ops::lease_slot::legacy_notes_root(&setup.repo.path().join(".git")).join("lease-1/run.log").exists());
}

/// A stream over its size writes no more than its size, and the next file of the fetch still comes
#[tokio::test(start_paused = true)]
async fn a_stream_over_its_size_writes_no_more_and_the_next_file_comes() {
  let setup = agent_sending(vec![
    ("out/bt-runs/1/big.log", FileKind::Staged, 4, vec![b'x'; 300 * 1024]),
    ("out/bt-runs/1/small.log", FileKind::Staged, 2, b"ok".to_vec()),
  ]);
  let refusal = fetch_with(&setup, Some(&staged_record()), &[], |_| {}).await.unwrap_err();
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
