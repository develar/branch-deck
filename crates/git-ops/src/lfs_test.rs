use crate::lease_slot::lease_config;
use crate::lfs::{ChangedPointer, fetch_config, include_lists, include_pattern, pointer_oid, pull_lfs_files, skips_download_errors};
use crate::remote_command::{LFS_DEADLINE, execute_remote_command_until};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use test_utils::env_vars::EnvVars;
use test_utils::fake_executable::{executor_of_fake_git, fake_executable};
use test_utils::git_test_utils::TestRepo;
use test_utils::hermetic_git::HermeticGit;

/// A repository whose local config a test writes, with no system config and an empty global one: the filter of a
/// `git lfs install` of the developer or of the runner stays out
struct IsolatedConfig {
  repo: TestRepo,
  config: PathBuf,
  local: String,
  _hermetic: HermeticGit,
}

impl IsolatedConfig {
  fn new() -> Self {
    let hermetic = HermeticGit::new();
    let repo = TestRepo::new();
    let config = repo.path().join(".git").join("config");
    let local = std::fs::read_to_string(&config).unwrap();
    Self {
      repo,
      config,
      local,
      _hermetic: hermetic,
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

/// Each path is a pattern of the `--include` of Git LFS that matches it: a character of a pattern and a leading `!` or
/// `#` escaped, a comma and a blank at an end (which Git LFS splits at or trims) a `?`
#[test]
fn a_path_is_a_pattern_of_the_include_that_matches_it() {
  for (path, pattern) in [
    ("data/big file.bin", "data/big file.bin"),
    ("a,b.bin", "a?b.bin"),
    ("*.bin", "\\*.bin"),
    ("a?[b]\\c.bin", "a\\?\\[b\\]\\\\c.bin"),
    ("!a.bin", "\\!a.bin"),
    ("#a.bin", "\\#a.bin"),
    ("x/!a#.bin", "x/!a#.bin"),
    (" lead.bin", "?lead.bin"),
    ("trail.bin ", "trail.bin?"),
  ] {
    assert_eq!(include_pattern(path), pattern, "{path}");
  }
}

/// The paths of a fetch are lists of at most 8 KiB each, so a long list is several fetches, never one without
/// `--include`, which could pull every object of a commit the store lacks
#[test]
fn the_paths_of_a_fetch_are_lists_of_at_most_8_kib() {
  let pointers: Vec<ChangedPointer> = (0..1000)
    .map(|n| ChangedPointer {
      path: format!("assets/textures/a-long-directory-name/model-{n:04}.bin"),
      oid: String::new(),
    })
    .collect();
  let lists = include_lists(&pointers.iter().collect::<Vec<_>>());
  assert!(lists.len() > 1, "{}", lists.len());
  assert!(lists.iter().all(|list| list.len() <= 8 * 1024));
  let paths: Vec<&str> = lists.iter().flat_map(|list| list.split(',')).collect();
  assert_eq!(paths, pointers.iter().map(|pointer| pointer.path.as_str()).collect::<Vec<_>>());
  let one = ChangedPointer {
    path: "model.bin".to_owned(),
    oid: String::new(),
  };
  assert_eq!(include_lists(&[&one]), ["model.bin"]);
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

/// A repository lives on the pointers of what Git LFS can not download when `lfs.skipdownloaderrors` is true, a key
/// without a value too, or `GIT_LFS_SKIP_DOWNLOAD_ERRORS` is
#[test]
fn a_repository_skips_download_errors_by_its_config_or_the_environment() {
  let isolated = IsolatedConfig::new();
  let skips = |text: &str| {
    isolated.write(text);
    skips_download_errors(isolated.repo.git_executor(), isolated.repo.path_str())
  };
  assert!(!skips(""));
  for (text, expected) in [
    ("[lfs]\n\tskipdownloaderrors = true\n", true),
    ("[lfs]\n\tskipdownloaderrors\n", true),
    ("[lfs]\n\tskipdownloaderrors = false\n", false),
  ] {
    assert_eq!(skips(text), expected, "{text}");
  }
  let _skip = EnvVars::set(&[("GIT_LFS_SKIP_DOWNLOAD_ERRORS", "1")]);
  assert!(skips(""));
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

/// A `git lfs pull` that hangs is killed at the deadline of the LFS commands (`LFS_DEADLINE`, a test's
/// `BD_TEST_LFS_DEADLINE_MS`), with the processes it started: git-lfs does its own HTTP, which `http.lowSpeedLimit` does
/// not reach
#[test]
fn a_hung_lfs_pull_is_killed_at_its_deadline() {
  let fake = tempfile::tempdir().unwrap();
  let fake_git = fake_executable(fake.path(), "git", "case \" $* \" in *\" lfs pull \"*) sleep 60; exit 0;; esac\nexec git \"$@\"");
  let git = executor_of_fake_git(&fake_git);
  let repo = TestRepo::new();
  let _deadline = EnvVars::set(&[("BD_TEST_LFS_DEADLINE_MS", "300")]);
  let started = std::time::Instant::now();
  let error = pull_lfs_files(&git, repo.path_str()).unwrap_err();
  assert!(format!("{error:#}").contains("did not finish within"), "{error:#}");
  assert!(started.elapsed() < std::time::Duration::from_secs(30), "{:?}", started.elapsed());
}

/// A fetch before a move ends as its operation is canceled (the Stop Waiting of the app), with the processes it started
#[test]
fn a_fetch_ends_as_its_operation_is_canceled() {
  let fake = tempfile::tempdir().unwrap();
  let fake_git = fake_executable(fake.path(), "git", "case \" $* \" in *\" lfs fetch \"*) sleep 60; exit 0;; esac\nexec git \"$@\"");
  let git = executor_of_fake_git(&fake_git);
  let repo = TestRepo::new();
  let started = std::time::Instant::now();
  let error = execute_remote_command_until(&git, &["lfs", "fetch"], repo.path_str(), LFS_DEADLINE, &|| true).unwrap_err();
  assert!(format!("{error:#}").contains("canceled"), "{error:#}");
  assert!(started.elapsed() < std::time::Duration::from_secs(30), "{:?}", started.elapsed());
}
