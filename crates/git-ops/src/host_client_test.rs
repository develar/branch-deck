use crate::host_client::{FARTHEST, Scan, apply_patch, candidates, commit_count, pack_commit_until, repository_name, snapshot as snapshot_of, working_tree_commit};
use crate::host_objects::ingest_pack;
use crate::lease_slot::LeaseConfig;
use crate::worktree_snapshot::{WorktreeCommit, local_changes, read_head_commit, write_tree_patch};
use pretty_assertions::assert_eq;
use std::path::Path;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;
use test_utils::process_budget::{assert_processes, counting};

fn snapshot(repo: &TestRepo) -> WorktreeCommit {
  let head = read_head_commit(repo.git_executor(), repo.path_str()).unwrap();
  working_tree_commit(repo.git_executor(), repo.path_str(), &repo.path().join(".git"), &head, &LeaseConfig::default()).unwrap()
}

/// A clean working tree ships HEAD after one status; a changed one a commit on HEAD of what `git add --all` stages, by
/// the fixed author and committer, dated as the committer of HEAD, so the same tree is the same commit; the index and
/// the refs stay
#[test]
fn the_commit_of_the_working_tree_is_head_or_the_same_commit_on_head_for_the_same_tree() {
  let repo = TestRepo::new();
  let head = repo.create_commit_with_files("Add files", &[(".gitignore", "out/\n"), ("a.txt", "a\n")]);
  let git = counting();
  let read = read_head_commit(&git, repo.path_str()).unwrap();
  let clean = working_tree_commit(&git, repo.path_str(), &repo.path().join(".git"), &read, &LeaseConfig::default()).unwrap();
  assert_processes(&git, &[("log", 1), ("status", 1)]);
  assert_eq!((clean.commit, clean.entries), (head.clone(), Vec::new()));

  std::fs::write(repo.path().join("a.txt"), "a\nchanged\n").unwrap();
  std::fs::write(repo.path().join("new.txt"), "new\n").unwrap();
  std::fs::create_dir_all(repo.path().join("out")).unwrap();
  std::fs::write(repo.path().join("out/ignored.txt"), "x\n").unwrap();
  let index = std::fs::read(repo.path().join(".git/index")).unwrap();
  let first = snapshot(&repo);
  let second = snapshot(&repo);
  assert_eq!(first.commit, second.commit, "the same tree on the same HEAD is the same commit");
  assert_eq!(std::fs::read(repo.path().join(".git/index")).unwrap(), index);
  assert_eq!(repo.head(), head);
  let lines: Vec<String> = first.entries.iter().map(super::worktree_snapshot::StatusEntry::line).collect();
  assert_eq!(lines, [" M a.txt", "?? new.txt"]);
  let described = repo.git(&["log", "-1", "--format=%P|%an <%ae>|%cn <%ce>|%ct|%s", &first.commit]);
  let committed = repo.git(&["log", "-1", "--format=%ct", &head]);
  assert_eq!(
    described,
    format!(
      "{head}|bd-host <bd-host@bd-host.invalid>|bd-host <bd-host@bd-host.invalid>|{committed}|bd-host: the working tree on {}",
      &head[..12]
    )
  );
  assert_eq!(repo.git(&["ls-tree", "-r", "--name-only", &first.commit]), ".gitignore\na.txt\nnew.txt");
}

/// The candidates are HEAD and its first-parent ancestors at the powers of two up to the farthest distance, then the
/// merge bases with origin/master and master, each once; the base is origin/master, else master
#[test]
fn the_candidates_are_the_powers_of_two_and_the_merge_bases() {
  let commits: Vec<HistoryCommit> = (0..12).map(|n| HistoryCommit::new(format!("Commit {n}"), &[("n.txt", &format!("{n}\n"))])).collect();
  let (repo, ids) = TestRepo::with_history(&commits);
  let head = ids[11].clone();
  repo.git(&["update-ref", "refs/remotes/origin/master", &ids[1]]);
  repo.git(&["branch", "-f", "side", &ids[0]]);
  let git = counting();
  let found = candidates(&git, repo.path_str(), &head, 8).unwrap();
  assert_processes(&git, &[("for-each-ref", 1), ("merge-base", 2), ("rev-list", 1)]);
  assert_eq!(found.commits, [11, 10, 9, 7, 3, 1].map(|n| ids[n].clone()));
  assert_eq!(found.base.as_deref(), Some(ids[1].as_str()));
  assert_eq!(candidates(repo.git_executor(), repo.path_str(), &head, FARTHEST).unwrap().commits.len(), 6);

  let lone = TestRepo::new();
  let only = lone.create_commit("Only", "a.txt", "a\n");
  lone.git(&["branch", "-m", "trunk"]);
  let found = candidates(lone.git_executor(), lone.path_str(), &only, FARTHEST).unwrap();
  assert_eq!((found.commits, found.base), (vec![only], None), "a missing base ref adds nothing");
}

#[test]
fn the_name_of_a_repository_is_the_directory_of_its_main_worktree() {
  assert_eq!(repository_name(Path::new("/p/idea/.git")).as_deref(), Some("idea"));
  assert_eq!(repository_name(Path::new("/p/mirror.git")).as_deref(), Some("mirror"));
  assert_eq!(repository_name(Path::new("/p/idea/.git/")).as_deref(), Some("idea"));
}

/// The thin pack lands on a host that has its base, from a private file, by one `pack-objects`; git's progress arrives
/// line by line; a pack file of a day ago goes at the next pack; the count is of the commits the host lacks, by one
/// `rev-list`
#[test]
fn the_thin_pack_is_a_private_file_that_the_host_takes() {
  let (client, host) = TestRepo::upstream_and_clone(&[("a.txt", "a\n")]);
  let base = client.head();
  let commit = client.create_commit("Change", "b.txt", "b\n");
  let dir = tempfile::tempdir().unwrap();
  let packs = dir.path().join("packs");
  std::fs::create_dir_all(&packs).unwrap();
  let old = packs.join("pack-old.pack");
  std::fs::write(&old, "old").unwrap();
  std::fs::File::options()
    .write(true)
    .open(&old)
    .unwrap()
    .set_modified(std::time::SystemTime::now() - std::time::Duration::from_hours(25))
    .unwrap();

  let mut lines = Vec::new();
  let mut on_line = |line: &str| lines.push(line.to_string());
  let git = counting();
  let pack = pack_commit_until(&git, client.path_str(), &commit, std::slice::from_ref(&base), &packs, &mut on_line, &|| false).unwrap();
  assert_processes(&git, &[("pack-objects", 1)]);
  assert!(!old.exists(), "a pack file of a day ago goes");
  assert!(!lines.is_empty(), "the progress of git");
  assert_eq!(pack.bytes, std::fs::metadata(&pack.path).unwrap().len());
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&pack.path).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::metadata(&packs).unwrap().permissions().mode() & 0o777, 0o700);
  }
  ingest_pack(host.git_executor(), host.path_str(), &mut std::fs::File::open(&pack.path).unwrap(), 100).unwrap();
  assert_eq!(host.git(&["cat-file", "-t", &commit]), "commit");
  let git = counting();
  assert_eq!(commit_count(&git, client.path_str(), &commit, &[base]).unwrap(), 1);
  assert_processes(&git, &[("rev-list", 1)]);
  assert!(
    pack_commit_until(client.git_executor(), client.path_str(), "HEAD", &[], &packs, &mut |_| {}, &|| false).is_err(),
    "a name that is no id"
  );
}

/// The snapshot reads no status for `Scan::Head`, the config and one status for `Scan::Status`, and ships a commit of the
/// working tree for `Scan::WorkingTree` by `bd-host <bd-host@bd-host.invalid>` (the blobs, the commit and the delete of
/// its temporary ref); each with the candidates of HEAD (one `rev-list`, one `for-each-ref`, a `merge-base` per base ref)
#[test]
fn a_snapshot_of_bd_host_reads_what_its_scan_names_and_ships_its_own_commit() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Add a", "a.txt", "a\n");
  std::fs::write(repo.path().join("a.txt"), "a\nchanged\n").unwrap();
  let git_dir = repo.path().join(".git");
  let git = counting();
  let quick = snapshot_of(&git, repo.path_str(), &git_dir, Scan::Head, 16).unwrap();
  assert_processes(&git, &[("for-each-ref", 1), ("log", 1), ("merge-base", 1), ("rev-list", 1)]);
  assert_eq!((quick.commit.as_str(), quick.head.commit.as_str(), quick.changed), (head.as_str(), head.as_str(), None));
  assert_eq!(quick.candidates.commits, std::slice::from_ref(&head));

  let git = counting();
  let status = snapshot_of(&git, repo.path_str(), &git_dir, Scan::Status, 16).unwrap();
  assert_processes(&git, &[("config", 1), ("for-each-ref", 1), ("log", 1), ("merge-base", 1), ("rev-list", 1), ("status", 1)]);
  let lines: Vec<String> = status.changed.unwrap().iter().map(super::worktree_snapshot::StatusEntry::line).collect();
  assert_eq!((status.commit.as_str(), lines), (head.as_str(), vec![" M a.txt".to_string()]));

  let git = counting();
  let shipped = snapshot_of(&git, repo.path_str(), &git_dir, Scan::WorkingTree, 16).unwrap();
  assert_processes(
    &git,
    &[
      ("config", 1),
      ("fast-import", 1),
      ("for-each-ref", 1),
      ("hash-object", 1),
      ("log", 1),
      ("merge-base", 1),
      ("rev-list", 1),
      ("status", 1),
      ("update-ref", 1),
    ],
  );
  assert_ne!(shipped.commit, head);
  assert_eq!(
    repo.git(&["log", "-1", "--format=%P|%an <%ae>|%s", &shipped.commit]),
    format!("{head}|bd-host <bd-host@bd-host.invalid>|bd-host: the working tree on {}", &head[..12])
  );
  assert_eq!(repo.head(), head, "no ref moves");
}

/// A pack that its caller stops fails as cancelled and leaves no file
#[test]
fn a_cancelled_pack_fails_and_leaves_no_file() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Base", "a.txt", "a\n");
  let commit = repo.create_commit("Change", "b.txt", "b\n");
  let dir = tempfile::tempdir().unwrap();
  let error = pack_commit_until(repo.git_executor(), repo.path_str(), &commit, &[base], dir.path(), &mut |_| {}, &|| true).unwrap_err();
  assert_eq!(
    error.downcast_ref::<git_executor::git_command_executor::StoppedCommand>(),
    Some(&git_executor::git_command_executor::StoppedCommand::Cancelled)
  );
  assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

/// The patch of a run applies to the files, not the index, all or nothing: one that does not apply changes nothing
#[test]
fn the_patch_of_a_run_applies_to_the_files_all_or_nothing() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Add files", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
  let before = repo.git(&["rev-parse", "HEAD^{tree}"]);
  let dir = tempfile::tempdir().unwrap();
  let changed = TestRepo::new_empty();
  changed.clone_from(repo.path()).unwrap();
  std::fs::write(changed.path().join("a.txt"), "a\nby the run\n").unwrap();
  std::fs::write(changed.path().join("c.txt"), "c\n").unwrap();
  let after = snapshot(&changed).tree;
  let patch = dir.path().join("run.patch");
  write_tree_patch(changed.git_executor(), changed.path_str(), &before, &after, &patch).unwrap();

  std::fs::write(repo.path().join("b.txt"), "b\nlocal\n").unwrap();
  assert_eq!(apply_patch(repo.git_executor(), repo.path_str(), &patch).unwrap(), 2);
  assert_eq!(std::fs::read_to_string(repo.path().join("a.txt")).unwrap(), "a\nby the run\n");
  let changes: Vec<String> = local_changes(repo.git_executor(), repo.path_str(), &LeaseConfig::default())
    .unwrap()
    .iter()
    .map(super::worktree_snapshot::StatusEntry::line)
    .collect();
  assert_eq!(changes, [" M a.txt", " M b.txt", "?? c.txt"], "the index stays");

  std::fs::write(repo.path().join("a.txt"), "a\nmoved on\n").unwrap();
  std::fs::remove_file(repo.path().join("c.txt")).unwrap();
  apply_patch(repo.git_executor(), repo.path_str(), &patch).unwrap_err();
  assert!(!repo.path().join("c.txt").exists(), "nothing applied");
  assert!(apply_patch(repo.git_executor(), repo.path_str(), Path::new("run.patch")).is_err(), "a relative path");
}
