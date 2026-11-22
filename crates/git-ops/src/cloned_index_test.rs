use crate::cloned_index::{IndexPatch, patch_cloned_index};
use crate::lease_slot::{UntrackedFiles, linked_worktree, refresh_cloned_index, register_slot, slot_changes};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::fs::{File, FileTimes};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use test_utils::git_test_utils::TestRepo;
use test_utils::links::set_link_mtime;

/// The mtime of the files of a fixture, in 2020: older than any index the test writes, so no entry is racy
const PAST: Duration = Duration::from_secs(1_600_000_000);

/// A template with committed files and symbolic links whose mtimes are in 2020, and its index refreshed after that
struct Template {
  repo: TestRepo,
  head: String,
}

/// A slot registered beside the template, with a copy of each tracked file and symbolic link of the template (a new inode
/// and ctime, the same mtime and size) and a copy of its index, as an APFS clone gets them
struct Slot {
  _dir: tempfile::TempDir,
  path: PathBuf,
  git_dir: PathBuf,
  /// The mtime of the index of the template before its copy
  template_written: SystemTime,
}

fn set_mtime(path: &Path, mtime: SystemTime) {
  File::options().write(true).open(path).unwrap().set_times(FileTimes::new().set_modified(mtime)).unwrap();
}

impl Template {
  fn new(files: &[(&str, &str)]) -> Self {
    Self::in_repo(TestRepo::new(), files, &[])
  }

  /// `files` (path, content) and the symbolic links `links` (path, target) committed in `repo`
  fn in_repo(repo: TestRepo, files: &[(&str, &str)], links: &[(&str, &str)]) -> Self {
    for (name, target) in links {
      std::os::unix::fs::symlink(target, repo.path().join(name)).unwrap();
      repo.git(&["add", "--", name]);
    }
    let head = repo.create_commit_with_files("Add files", files);
    for (name, _) in files {
      set_mtime(&repo.path().join(name), UNIX_EPOCH + PAST);
    }
    for (name, _) in links {
      set_link_mtime(&repo.path().join(name), UNIX_EPOCH + PAST);
    }
    repo.git(&["update-index", "-q", "--refresh"]);
    Self { repo, head }
  }

  fn clone_slot(&self) -> Slot {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("slot");
    register_slot(self.repo.git_executor(), self.repo.path_str(), &path, &self.head, false).unwrap();
    for name in self.repo.git(&["ls-files"]).lines() {
      let (from, to) = (self.repo.path().join(name), path.join(name));
      std::fs::create_dir_all(to.parent().unwrap()).unwrap();
      let metadata = from.symlink_metadata().unwrap();
      if metadata.is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(&from).unwrap(), &to).unwrap();
        set_link_mtime(&to, metadata.modified().unwrap());
      } else {
        std::fs::write(&to, std::fs::read(&from).unwrap()).unwrap();
        std::fs::set_permissions(&to, metadata.permissions()).unwrap();
        set_mtime(&to, metadata.modified().unwrap());
      }
    }
    let git_dir = linked_worktree(&path).unwrap().git_dir;
    let template_index = self.repo.path().join(".git/index");
    let template_written = template_index.metadata().unwrap().modified().unwrap();
    std::fs::copy(&template_index, git_dir.join("index")).unwrap();
    Slot {
      _dir: dir,
      path,
      git_dir,
      template_written,
    }
  }
}

impl Slot {
  fn path_str(&self) -> &str {
    self.path.to_str().unwrap()
  }

  fn git(&self, args: &[&str]) -> String {
    GitCommandExecutor::new()
      .execute_command(args, self.path_str())
      .unwrap_or_else(|error| panic!("git {args:?} in the slot: {error}"))
  }

  fn patch(&self, head: &str) -> IndexPatch {
    patch_cloned_index(&self.git_dir, &self.path, self.template_written, head).unwrap()
  }

  /// The inode that the index holds for `name`, from `ls-files --debug`
  fn indexed_inode(&self, name: &str) -> u64 {
    let debug = self.git(&["ls-files", "--debug", "--", name]);
    let inode = debug.split("ino: ").nth(1).unwrap_or_else(|| panic!("no inode in {debug}"));
    inode.split_whitespace().next().unwrap().parse().unwrap()
  }

  fn inode(&self, name: &str) -> u64 {
    self.path.join(name).symlink_metadata().unwrap().ino() & u64::from(u32::MAX)
  }

  fn index_bytes(&self) -> Vec<u8> {
    std::fs::read(self.git_dir.join("index")).unwrap()
  }

  /// The index holds the inode of the file of each of `names`, and a `status` that may write the index (`slot_changes`)
  /// finds `changes` and writes nothing: no entry needed a hash
  fn assert_patched(&self, names: &[&str], changes: &[&str], context: &str) {
    for name in names {
      assert_eq!(self.indexed_inode(name), self.inode(name), "{context}: {name}");
    }
    let before = self.index_bytes();
    assert_eq!(
      slot_changes(&GitCommandExecutor::new(), self.path_str(), UntrackedFiles::Normal).unwrap(),
      changes,
      "{context}"
    );
    assert!(self.index_bytes() == before, "{context}: the status refreshed and wrote the index");
  }
}

/// Each copied file keeps the mtime and the size of the template and gets a new inode and ctime. The patch writes its
/// stat data into the index for index version 2 and 4 and for an executable file, so a `status` with the default check
/// that may write the index (`slot_changes`) finds nothing and writes nothing: no entry needed a hash. Without the patch
/// the same `status` hashes every file and writes the index. An intent-to-add entry (index version 3) is left
#[test]
fn the_stat_data_of_the_copied_files_goes_into_the_index_without_a_hash() {
  for version in ["2", "4"] {
    let mut template = Template::new(&[("a.txt", "a\n"), ("dir/b.txt", "b\n"), ("run.sh", "#!/bin/sh\n")]);
    std::fs::set_permissions(template.repo.path().join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    template.repo.git(&["add", "run.sh"]);
    template.repo.git(&["commit", "-q", "-m", "Executable"]);
    template.repo.git(&["update-index", "-q", "--refresh"]);
    template.head = template.repo.head();
    let head = template.head.clone();
    template.repo.git(&["update-index", "--index-version", version]);
    let unpatched = template.clone_slot();
    let before = unpatched.index_bytes();
    assert_eq!(
      slot_changes(template.repo.git_executor(), unpatched.path_str(), UntrackedFiles::Normal).unwrap(),
      Vec::<String>::new()
    );
    assert!(unpatched.index_bytes() != before, "version {version}: without the patch the status refreshes the index");
    let slot = template.clone_slot();
    assert_ne!(
      slot.indexed_inode("a.txt"),
      slot.inode("a.txt"),
      "version {version}: the index holds the inode of the template"
    );

    assert_eq!(slot.patch(&head), IndexPatch::Written { patched: 3, left: 0 }, "version {version}");

    slot.assert_patched(&["a.txt", "dir/b.txt", "run.sh"], &[], &format!("version {version}"));
    assert!(!slot.git_dir.join("index.lock").exists());
  }

  let template = Template::new(&[("a.txt", "a\n")]);
  std::fs::write(template.repo.path().join("new.txt"), "new\n").unwrap();
  template.repo.git(&["add", "-N", "new.txt"]);
  let slot = template.clone_slot();
  assert_eq!(
    slot.patch(&template.head),
    IndexPatch::Written { patched: 1, left: 0 },
    "the intent-to-add entry is no candidate"
  );
}

/// An entry whose mtime is not older than the index of the template is racy: the template may have changed its file in
/// the same second. A file of another size is changed. The patch leaves both to the refresh, which hashes them: the
/// unchanged racy file is clean, the changed one is a change
#[test]
fn a_racy_entry_and_a_changed_file_are_left_to_the_refresh() {
  let template = Template::new(&[("a.txt", "a\n"), ("racy.txt", "r\n"), ("changed.txt", "c\n")]);
  let mut slot = template.clone_slot();
  let racy_mtime = UNIX_EPOCH + PAST + Duration::from_secs(10);
  set_mtime(&template.repo.path().join("racy.txt"), racy_mtime);
  template.repo.git(&["update-index", "-q", "--refresh"]);
  std::fs::copy(template.repo.path().join(".git/index"), slot.git_dir.join("index")).unwrap();
  set_mtime(&slot.path.join("racy.txt"), racy_mtime);
  // the template wrote its index in the second of the mtime of racy.txt
  slot.template_written = racy_mtime;
  std::fs::write(slot.path.join("changed.txt"), "changed\n").unwrap();

  assert_eq!(slot.patch(&template.head), IndexPatch::Written { patched: 1, left: 2 });
  assert_ne!(slot.indexed_inode("racy.txt"), slot.inode("racy.txt"));

  refresh_cloned_index(template.repo.git_executor(), slot.path_str(), &slot.git_dir, None, &template.head).unwrap();
  assert_eq!(slot.git(&["status", "--porcelain"]), "M changed.txt");
  assert_eq!(slot.indexed_inode("racy.txt"), slot.inode("racy.txt"), "the refresh proved it unchanged by its hash");
}

/// The extensions that cache stat data (`UNTR`) go, and git builds them again; a split index and an `index.lock` of
/// another process leave the index as it is, for the refresh to check every entry
#[test]
fn a_stat_cache_goes_and_an_index_it_does_not_know_stays() {
  let template = Template::new(&[("a.txt", "a\n")]);
  template.repo.git(&["-c", "core.untrackedCache=true", "update-index", "--untracked-cache"]);
  template.repo.git(&["-c", "core.untrackedCache=true", "status", "--porcelain"]);
  let slot = template.clone_slot();
  assert!(slot.index_bytes().windows(4).any(|window| window == b"UNTR"), "the template has an untracked cache");
  assert_eq!(slot.patch(&template.head), IndexPatch::Written { patched: 1, left: 0 });
  assert!(!slot.index_bytes().windows(4).any(|window| window == b"UNTR"));
  assert_eq!(slot.git(&["status", "--porcelain"]), "");

  let slot = template.clone_slot();
  std::fs::write(slot.git_dir.join("index.lock"), "").unwrap();
  assert_eq!(slot.patch(&template.head), IndexPatch::Kept("another process holds index.lock".to_string()));
  assert!(slot.git_dir.join("index.lock").exists(), "the lock of another process stays");
  std::fs::remove_file(slot.git_dir.join("index.lock")).unwrap();

  template.repo.git(&["update-index", "--split-index"]);
  let slot = template.clone_slot();
  let before = slot.index_bytes();
  assert_eq!(slot.patch(&template.head), IndexPatch::Kept("a split index".to_string()));
  assert!(slot.index_bytes() == before);
}

/// A symbolic link takes the stat data of the link itself (`lstat`), as git's `fill_stat_data` does: its own inode, and
/// its size is the length of its target. A link to a tracked file and a link whose target is gone are both written. A
/// link of the index that is a file of the same size and mtime in the slot is left to the refresh, which finds the type
/// change
#[test]
fn a_symbolic_link_takes_the_stat_data_of_the_link_and_not_of_its_target() {
  let template = Template::in_repo(
    TestRepo::new(),
    &[("a.txt", "a\n")],
    &[("link", "a.txt"), ("dangling", "gone/target"), ("swapped", "a.txt")],
  );
  let slot = template.clone_slot();
  std::fs::remove_file(slot.path.join("swapped")).unwrap();
  std::fs::write(slot.path.join("swapped"), "a.txt").unwrap();
  set_mtime(&slot.path.join("swapped"), UNIX_EPOCH + PAST);

  assert_eq!(slot.patch(&template.head), IndexPatch::Written { patched: 3, left: 1 });

  for name in ["a.txt", "link", "dangling"] {
    assert_eq!(slot.indexed_inode(name), slot.inode(name), "{name}");
  }
  assert_ne!(slot.inode("link"), slot.inode("a.txt"), "the link and its target are two inodes");
  assert_ne!(slot.indexed_inode("swapped"), slot.inode("swapped"));
  let before = slot.index_bytes();
  assert_eq!(slot.git(&["diff-files", "--name-only", "--", "a.txt", "link", "dangling"]), "");
  assert!(slot.index_bytes() == before, "diff-files does not write the index");

  refresh_cloned_index(template.repo.git_executor(), slot.path_str(), &slot.git_dir, None, &template.head).unwrap();
  assert_eq!(slot.git(&["status", "--porcelain"]), "T swapped");
}

/// In a SHA-256 repository the object id of an entry is 32 bytes, so the fixed part of each entry and the padding after
/// its path differ from SHA-1, and the checksum is a SHA-256. The paths of 1 to 8 bytes give each of the 8 paddings of
/// index version 3; an intent-to-add entry gives an extended entry of index version 3 and 4. The patch writes every
/// entry of a file and a link, a `status` that may write the index writes nothing, and `fsck` accepts the checksum of the
/// new index
#[test]
fn a_sha256_index_takes_the_stat_data_of_the_copied_files() {
  let files = [
    ("a", "1\n"),
    ("bb", "2\n"),
    ("ccc", "3\n"),
    ("dddd", "4\n"),
    ("eeeee", "5\n"),
    ("ffffff", "6\n"),
    ("ggggggg", "7\n"),
    ("hhhhhhhh", "8\n"),
    ("dir/nested.txt", "9\n"),
  ];
  for version in ["2", "4"] {
    let template = Template::in_repo(TestRepo::new_sha256(), &files, &[("link", "a")]);
    assert_eq!(template.head.len(), 64);
    std::fs::write(template.repo.path().join("new.txt"), "new\n").unwrap();
    template.repo.git(&["add", "-N", "new.txt"]);
    template.repo.git(&["update-index", "--index-version", version]);
    let slot = template.clone_slot();
    // the intent-to-add entry is extended, so git writes index version 2 as version 3
    let written_version = if version == "2" { 3 } else { 4 };
    assert_eq!(slot.index_bytes()[4..8], u32::to_be_bytes(written_version), "version {version}");
    assert_ne!(slot.indexed_inode("a"), slot.inode("a"), "version {version}: the index holds the inode of the template");

    assert_eq!(slot.patch(&template.head), IndexPatch::Written { patched: 10, left: 0 }, "version {version}");

    let mut names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();
    names.push("link");
    slot.assert_patched(&names, &["new.txt"], &format!("version {version}"));
    slot.git(&["fsck", "--no-progress", "--no-dangling"]);
  }
}
