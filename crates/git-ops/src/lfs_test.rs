use crate::lease_slot::lease_config;
use crate::lfs::{ChangedPointer, fetch_config, include_of, pointer_oid};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use test_utils::env_vars::EnvVars;
use test_utils::git_test_utils::TestRepo;

/// A repository whose local config a test writes, with no system config and an empty global one: the filter of a
/// `git lfs install` of the developer or of the runner stays out
struct IsolatedConfig {
  repo: TestRepo,
  config: PathBuf,
  local: String,
  _isolated: EnvVars,
  _global: tempfile::NamedTempFile,
}

impl IsolatedConfig {
  fn new() -> Self {
    let global = tempfile::NamedTempFile::new().unwrap();
    let isolated = EnvVars::set(&[("GIT_CONFIG_NOSYSTEM", "1".as_ref()), ("GIT_CONFIG_GLOBAL", global.path().as_os_str())]);
    let repo = TestRepo::new();
    let config = repo.path().join(".git").join("config");
    let local = std::fs::read_to_string(&config).unwrap();
    Self {
      repo,
      config,
      local,
      _isolated: isolated,
      _global: global,
    }
  }

  /// The local config of the repository as it was made, then `text`
  fn write(&self, text: &str) {
    std::fs::write(&self.config, format!("{}{text}", self.local)).unwrap();
  }
}

/// A pointer file of Git LFS names its object by the line `oid sha256:` after its version line; anything else is content
#[test]
fn a_pointer_names_its_object() {
  let oid = "Ab".repeat(32);
  let pointer = format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 12\n");
  assert_eq!(pointer_oid(pointer.as_bytes()), Some(oid.to_ascii_lowercase()));
  for content in [
    format!("oid sha256:{oid}\nsize 12\n"),
    format!("version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 12\n", &oid[1..]),
    format!("version https://git-lfs.github.com/spec/v1\noid sha256:{}\n", "xy".repeat(32)),
    "a text\n".to_string(),
  ] {
    assert_eq!(pointer_oid(content.as_bytes()), None, "{content}");
  }
  assert_eq!(pointer_oid(&[0xff, 0xfe]), None);
}

/// The fetch includes the paths it needs, comma separated; a path that Git LFS would read as a pattern or as two, or a
/// list past 8 KiB, fetches every object of the commit the store lacks instead
#[test]
fn the_fetch_includes_the_paths_it_needs_unless_one_reads_as_a_pattern() {
  let pointers = |paths: &[&str]| {
    paths
      .iter()
      .map(|path| ChangedPointer {
        path: (*path).to_string(),
        oid: String::new(),
      })
      .collect::<Vec<_>>()
  };
  let include = |paths: &[&str]| {
    let pointers = pointers(paths);
    include_of(&pointers.iter().collect::<Vec<_>>())
  };
  assert_eq!(include(&["model.bin", "data/big file.bin"]).as_deref(), Some("model.bin,data/big file.bin"));
  for odd in ["a,b.bin", "*.bin", "a?.bin", "[a].bin", "a\\b.bin", "!a.bin", "#a.bin"] {
    assert_eq!(include(&["model.bin", odd]), None, "{odd}");
  }
  let long = "d/".repeat(5000);
  assert_eq!(include(&[long.as_str()]), None);
}

/// The fetch takes the remote Git LFS takes by default: the remote of the branch, else `remote.lfsdefault`, else the
/// one remote, else `origin`; the keys of the filter and of the store come with it
#[test]
fn the_fetch_takes_the_remote_git_lfs_takes_by_default() {
  let isolated = IsolatedConfig::new();
  let remote = |text: &str, branch: Option<&str>| {
    isolated.write(text);
    fetch_config(isolated.repo.git_executor(), isolated.repo.path_str(), branch).unwrap().remote
  };
  let one = "[remote \"upstream\"]\n\turl = https://example.com/one\n";
  let two = format!("{one}[remote \"fork\"]\n\turl = https://example.com/two\n");
  assert_eq!(remote("", None), "origin");
  assert_eq!(remote(one, None), "upstream");
  assert_eq!(remote(&two, None), "origin");
  assert_eq!(remote(&format!("{two}[remote]\n\tlfsdefault = fork\n"), None), "fork");
  let tracked = format!("{two}[remote]\n\tlfsdefault = fork\n[branch \"Feature\"]\n\tremote = upstream\n");
  assert_eq!(remote(&tracked, Some("Feature")), "upstream");
  assert_eq!(remote(&tracked, Some("other")), "fork");
  assert_eq!(remote(&format!("{two}[branch \"Feature\"]\n\tremote = .\n"), Some("Feature")), "origin");

  isolated.write("[filter \"lfs\"]\n\tprocess = git-lfs filter-process\n[lfs]\n\tstorage = store\n");
  let read = fetch_config(isolated.repo.git_executor(), isolated.repo.path_str(), None).unwrap();
  assert_eq!((read.lfs.smudge, read.lfs.storage.as_deref()), (true, Some("store")));
}

/// The smudge filter of Git LFS downloads when `filter.lfs.process`, else `filter.lfs.smudge`, names a command without
/// `--skip` and `GIT_LFS_SKIP_SMUDGE` is not true: a new slot is then checked out with the pointers and pulled after the
/// clones lock (`LfsConfig::smudge`). A `process` that is set, empty too, hides the `smudge`, as in git
#[test]
fn the_smudge_of_git_lfs_downloads_unless_it_skips() {
  let isolated = IsolatedConfig::new();
  let smudges = |section: &str| {
    isolated.write(&format!("[filter \"lfs\"]\n{section}"));
    lease_config(isolated.repo.git_executor(), isolated.repo.path_str()).unwrap().lfs.smudge
  };
  let cases = [
    ("\trequired = true\n", false),
    ("\tprocess = git-lfs filter-process\n", true),
    ("\tprocess = git-lfs filter-process --skip\n", false),
    ("\tsmudge = git-lfs smudge -- %f\n", true),
    ("\tsmudge = git-lfs smudge --skip -- %f\n", false),
    ("\tsmudge = git-lfs smudge -- %f\n\tprocess = git-lfs filter-process --skip\n", false),
    ("\tprocess = git-lfs filter-process --skip\n\tprocess = git-lfs filter-process\n", true),
    ("\tsmudge = git-lfs smudge -- %f\n\tprocess =\n", false),
  ];
  for (section, expected) in cases {
    assert_eq!(smudges(section), expected, "{section}");
  }
  let downloading = "\tprocess = git-lfs filter-process\n";
  let _skip = EnvVars::set(&[("GIT_LFS_SKIP_SMUDGE", "1")]);
  assert!(!smudges(downloading), "GIT_LFS_SKIP_SMUDGE=1");
  let _no_skip = EnvVars::set(&[("GIT_LFS_SKIP_SMUDGE", "0")]);
  assert!(smudges(downloading), "GIT_LFS_SKIP_SMUDGE=0");
}

/// The objects of Git LFS are in `lfs/objects` of the common directory, or in `objects` of `lfs.storage`, relative to it
#[test]
fn the_objects_of_git_lfs_are_where_lfs_storage_puts_them() {
  let isolated = IsolatedConfig::new();
  let common = PathBuf::from("/common");
  let objects = || lease_config(isolated.repo.git_executor(), isolated.repo.path_str()).unwrap().lfs.objects(&common);
  assert_eq!(objects(), common.join("lfs").join("objects"));
  isolated.write("[lfs]\n\tstorage = store\n");
  assert_eq!(objects(), common.join("store").join("objects"));
}
