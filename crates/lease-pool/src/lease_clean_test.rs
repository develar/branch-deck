#[cfg(unix)]
use crate::lease_clean::{CleanMode, Trash, apply_clean_rules};
use crate::lease_clean::{CleanRule, describe_size, parse_clean_rules, parse_size};
#[cfg(unix)]
use git_executor::git_command_executor::GitCommandExecutor;
#[cfg(unix)]
use std::path::Path;
use std::time::Duration;
#[cfg(unix)]
use std::time::SystemTime;

#[test]
fn the_rules_are_cargo_and_delete_with_their_defaults() {
  let rules =
    parse_clean_rules(
      "[lease]\nclean = [\n  { cargo = \"./target/\", keep = 3, age = \"30m\" },\n  { cargo = \"nested/target\" },\n  { delete = \"tests//results\" },\n  { delete = \"out/dev-run\", except = [\"jar-cache\"] },\n]\n",
    )
    .unwrap();
  assert_eq!(
    rules,
    [
      Ok(CleanRule::Cargo {
        dir: "target".to_string(),
        keep: 3,
        age: Duration::from_mins(30),
      }),
      Ok(CleanRule::Cargo {
        dir: "nested/target".to_string(),
        keep: 8,
        age: Duration::from_hours(1),
      }),
      Ok(CleanRule::Delete {
        path: "tests/results".to_string(),
        except: Vec::new(),
      }),
      Ok(CleanRule::Delete {
        path: "out/dev-run".to_string(),
        except: vec!["jar-cache".to_string()],
      }),
    ]
  );
  assert_eq!(parse_clean_rules("").unwrap(), []);
  assert_eq!(parse_clean_rules("[lease]\n").unwrap(), []);
}

/// An unknown key or kind, a wrong type and a bad age are errors of the whole file
#[test]
fn unknown_keys_and_kinds_are_errors() {
  for (text, says) in [
    ("[other]\n", "unknown key \"other\""),
    ("[lease]\nprune = []\n", "unknown key lease.prune"),
    ("[lease]\nclean = [{ command = \"rm -rf x\" }]\n", "unknown kind"),
    ("[lease]\nclean = [{ delete = \"x\", force = true }]\n", "unknown key \"force\""),
    ("[lease]\nclean = [{ cargo = \"target\", age = \"1 week\" }]\n", "`age`"),
    ("[lease]\nclean = [{ cargo = \"target\", keep = -1 }]\n", "negative"),
    ("[lease]\nclean = [\"target\"]\n", "a rule is a table"),
    ("[lease]\nclean = [{ delete = \"out\", except = \"x\" }]\n", "`except` is not an array"),
    ("[lease]\nclean = [{ cargo = \"target\", except = [\"x\"] }]\n", "unknown key \"except\""),
    ("[lease\n", "TOML"),
  ] {
    let error = format!("{:#}", parse_clean_rules(text).unwrap_err());
    assert!(error.contains(says) || (says == "TOML" && !error.is_empty()), "{text}: {error}");
  }
}

/// A path out of the checkout, an absolute one, one with `.git` and the checkout itself are reported, never applied
#[test]
fn an_unsafe_path_is_a_rule_that_is_reported() {
  let rules =
    parse_clean_rules("[lease]\nclean = [{ delete = \"../outside\" }, { delete = \"/etc\" }, { delete = \"a/.git/x\" }, { cargo = \".\" }, { delete = \"a\\\\b\" }]\n").unwrap();
  let reasons: Vec<String> = rules.iter().map(|rule| rule.clone().unwrap_err().1).collect();
  for (reason, says) in reasons.iter().zip(["..", "absolute", ".git", "the checkout itself", "backslash"]) {
    assert!(reason.contains(says), "{reason}");
  }
}

/// An `except` entry is the name of one entry, and a `delete` that would remove the dir of a cargo rule without keeping
/// the entry on its way is reported, never applied
#[test]
fn a_delete_names_its_kept_entries_and_keeps_the_dirs_of_cargo_rules() {
  let rules = parse_clean_rules(
    "[lease]\nclean = [\n  { cargo = \"out/cargo-target\" },\n  { delete = \"out\" },\n  { delete = \"out\", except = [\"cargo-target\"] },\n  { delete = \"out/cargo-target\" },\n  { delete = \"x\", except = [\"a/b\"] },\n  { delete = \"y\", except = [\"..\"] },\n]\n",
  )
  .unwrap();
  let outcome: Vec<Result<String, String>> = rules.into_iter().map(|rule| rule.map(|rule| rule.describe()).map_err(|(_, reason)| reason)).collect();
  assert!(outcome[0].is_ok() && outcome[2] == Ok("delete out except cargo-target".to_string()), "{outcome:?}");
  for index in [1, 3] {
    assert!(outcome[index].as_ref().unwrap_err().contains("out/cargo-target, the dir of a cargo rule"), "{outcome:?}");
  }
  for index in [4, 5] {
    assert!(outcome[index].as_ref().unwrap_err().contains("not the name of an entry"), "{outcome:?}");
  }
}

/// A size is a number and a binary unit, as `df -h` counts; a message names it in GiB
#[test]
fn a_size_is_a_number_and_a_binary_unit() {
  for (text, bytes) in [
    ("0", 0),
    ("512", 512),
    ("200G", 200 << 30),
    ("200GiB", 200 << 30),
    ("1.5G", u64::MAX),
    ("3T", 3 << 40),
    ("1P", 1 << 50),
    ("10KB", 10 << 10),
  ] {
    assert_eq!(parse_size(text).unwrap_or(u64::MAX), bytes, "{text}");
  }
  for text in ["", "G", "200X", "-1G"] {
    assert!(parse_size(text).is_err(), "{text}");
  }
  assert_eq!(describe_size(200 << 30), "200.0 GiB");
}

/// Set the mtime of `path`, a file or a directory, to `age` ago
#[cfg(unix)]
fn age(path: &Path, age: Duration) {
  std::fs::File::open(path).unwrap().set_modified(SystemTime::now() - age).unwrap();
}

/// A unit dir with a fingerprint file, built `built` ago
#[cfg(unix)]
fn unit(dir: &Path, built: Duration) {
  std::fs::create_dir_all(dir.join("fingerprint")).unwrap();
  std::fs::write(dir.join("fingerprint/dep"), vec![0u8; 4096]).unwrap();
  age(&dir.join("fingerprint/dep"), built);
  age(&dir.join("fingerprint"), built);
  age(dir, built);
}

/// An empty repository to clean, and the git of the rules
#[cfg(unix)]
fn git_checkout() -> (tempfile::TempDir, GitCommandExecutor) {
  let checkout = tempfile::tempdir().unwrap();
  let git = GitCommandExecutor::new();
  git.execute_command(&["init", "-q"], checkout.path().to_str().unwrap()).unwrap();
  (checkout, git)
}

/// In every profile dir (`.cargo-lock`, also two levels down), the cargo rule keeps the `keep` newest units of each
/// group and the young ones, and removes the rest; a dry run measures the same and deletes nothing
#[cfg(unix)]
#[test]
fn the_cargo_rule_keeps_the_newest_and_the_young_units() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let hour = Duration::from_hours(1);
  for profile in ["target/debug", "target/aarch64-apple-darwin/release"] {
    let profile = root.join(profile);
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join(".cargo-lock"), "").unwrap();
    // five units of `git-ops`: the two newest stay by `keep`, `h2` by its age, `h0` and `h1` go
    let minutes = |minutes: u64| Duration::from_secs(minutes * 60);
    for (hash, built) in [("h0", 10 * hour), ("h1", 9 * hour), ("h2", minutes(30)), ("h3", minutes(20)), ("h4", minutes(10))] {
      unit(&profile.join("build/git-ops").join(hash), built);
    }
    // a crate name with `-`: grouped by what comes before the last `-`
    for (name, built) in [("git_ops-old", 5 * hour), ("git_ops-mid", 4 * hour), ("git_ops-new", 3 * hour)] {
      unit(&profile.join("incremental").join(name), built);
    }
  }
  let rules = parse_clean_rules("[lease]\nclean = [{ cargo = \"target\", keep = 2, age = \"1h\" }]\n").unwrap();
  let checkout = root.to_str().unwrap();
  let dry_run = apply_clean_rules(&git, checkout, &rules, CleanMode::Measure);
  assert!(dry_run[0].freed_bytes > 0 && dry_run[0].error.is_none(), "{dry_run:?}");
  assert!(root.join("target/debug/build/git-ops/h0").exists(), "a dry run deletes nothing");

  let cleaned = apply_clean_rules(&git, checkout, &rules, CleanMode::Delete);
  assert_eq!((cleaned[0].freed_bytes, &cleaned[0].error), (dry_run[0].freed_bytes, &None));
  for profile in ["target/debug", "target/aarch64-apple-darwin/release"] {
    let left = |dir: &str| -> Vec<String> {
      let mut names: Vec<String> = std::fs::read_dir(root.join(profile).join(dir))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
      names.sort();
      names
    };
    assert_eq!(left("build/git-ops"), ["h2", "h3", "h4"], "{profile}");
    assert_eq!(left("incremental"), ["git_ops-mid", "git_ops-new"], "{profile}");
  }
}

#[test]
fn a_classic_unit_is_its_name_and_its_hash() {
  let unit = |name: &str| crate::lease_clean::classic_unit_name(name).map(|(group, hash)| format!("{group} {hash}"));
  let hash = "23097ec89eae1564";
  for (file, expected) in [
    (format!("libaho_corasick-{hash}.rlib"), Some(format!("aho_corasick {hash}"))),
    (format!("libaho_corasick-{hash}.rmeta"), Some(format!("aho_corasick {hash}"))),
    (format!("aho_corasick-{hash}.d"), Some(format!("aho_corasick {hash}"))),
    (format!("aho_corasick-{hash}"), Some(format!("aho_corasick {hash}"))),
    (format!("aho_corasick-{hash}.dSYM"), Some(format!("aho_corasick {hash}"))),
    (
      format!("aho_corasick-{hash}.aho_corasick.4f50d93f3af7ef9a-cgu.00.rcgu.o"),
      Some(format!("aho_corasick {hash}")),
    ),
    (format!("aho-corasick-{hash}"), Some(format!("aho_corasick {hash}"))),
    // a crate named `libc` keeps its name, without the `lib` of the library file
    (format!("liblibc-{hash}.rlib"), Some(format!("libc {hash}"))),
    (format!("libc-{hash}.d"), Some(format!("libc {hash}"))),
    (format!("avl_vm-{hash}.exe"), Some(format!("avl_vm {hash}"))),
    ("libavl_vm.rlib".to_string(), None),
    ("air-trace".to_string(), None),
    (".cargo-lock".to_string(), None),
    ("aho_corasick-23097ec89eae156".to_string(), None),
  ] {
    assert_eq!(unit(&file), expected, "{file}");
  }
}

/// Every file of a classic unit, built `built` ago: in `deps/` the library, its metadata, the dep file, an object file,
/// the test executable and its `.dSYM`; the fingerprint dir and the build dir of the build script
#[cfg(unix)]
fn classic_unit(profile: &Path, hash: &str, built: Duration) {
  let files = [
    format!("deps/libaho_corasick-{hash}.rlib"),
    format!("deps/libaho_corasick-{hash}.rmeta"),
    format!("deps/aho_corasick-{hash}.d"),
    format!("deps/aho_corasick-{hash}.aho_corasick.4f50d93f3af7ef9a-cgu.0.rcgu.o"),
    format!("deps/aho_corasick-{hash}"),
    format!("deps/aho_corasick-{hash}.dSYM/Contents/Info.plist"),
    format!(".fingerprint/aho-corasick-{hash}/lib-aho_corasick"),
    format!("build/aho-corasick-{hash}/out/generated.rs"),
  ];
  for file in &files {
    let path = profile.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![0u8; 4096]).unwrap();
    // the file and every dir up to the profile's own
    let mut current = path.as_path();
    while current != profile {
      age(current, built);
      current = current.parent().unwrap();
    }
  }
}

/// The classic layout: three hashes of one crate across `deps/`, `.fingerprint/` and `build/`, keep 1: the old units go
/// whole, the newest stays whole, the uplifted artifacts and `.cargo-lock` stay. Beside it a profile dir of the
/// build-dir layout in the same target dir and a classic cross-target profile dir
#[cfg(unix)]
#[test]
fn the_cargo_rule_prunes_the_classic_layout_by_units_of_name_and_hash() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let hour = Duration::from_hours(1);
  let (old, older, newest) = ("1111111111111111", "2222222222222222", "3333333333333333");
  let classic = ["target/debug", "target/aarch64-unknown-linux-musl/debug"];
  for profile in classic {
    let profile = root.join(profile);
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join(".cargo-lock"), "").unwrap();
    std::fs::write(profile.join("libaho_corasick.rlib"), "uplifted").unwrap();
    std::fs::write(profile.join("aho-corasick"), "uplifted").unwrap();
    age(&profile.join("libaho_corasick.rlib"), 20 * hour);
    classic_unit(&profile, old, 10 * hour);
    classic_unit(&profile, older, 9 * hour);
    classic_unit(&profile, newest, 2 * hour);
  }
  // the build-dir layout of the release profile
  let release = root.join("target/release");
  std::fs::create_dir_all(&release).unwrap();
  std::fs::write(release.join(".cargo-lock"), "").unwrap();
  unit(&release.join("build/git-ops/h0"), 10 * hour);
  unit(&release.join("build/git-ops/h1"), 2 * hour);

  let rules = parse_clean_rules("[lease]\nclean = [{ cargo = \"target\", keep = 1, age = \"1h\" }]\n").unwrap();
  let cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Delete);
  assert!(cleaned[0].freed_bytes > 0 && cleaned[0].error.is_none(), "{cleaned:?}");
  let names = |dir: &Path| -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
      .unwrap()
      .flatten()
      .map(|entry| entry.file_name().to_string_lossy().into_owned())
      .collect();
    names.sort();
    names
  };
  for profile in classic {
    let profile = root.join(profile);
    assert_eq!(
      names(&profile.join("deps")),
      [
        format!("aho_corasick-{newest}"),
        format!("aho_corasick-{newest}.aho_corasick.4f50d93f3af7ef9a-cgu.0.rcgu.o"),
        format!("aho_corasick-{newest}.d"),
        format!("aho_corasick-{newest}.dSYM"),
        format!("libaho_corasick-{newest}.rlib"),
        format!("libaho_corasick-{newest}.rmeta"),
      ],
      "{}",
      profile.display()
    );
    assert_eq!(names(&profile.join(".fingerprint")), [format!("aho-corasick-{newest}")]);
    assert_eq!(names(&profile.join("build")), [format!("aho-corasick-{newest}")]);
    assert!(profile.join(format!("build/aho-corasick-{newest}/out/generated.rs")).exists());
    for kept in [".cargo-lock", "libaho_corasick.rlib", "aho-corasick"] {
      assert!(profile.join(kept).exists(), "{kept}");
    }
  }
  assert_eq!(names(&release.join("build/git-ops")), ["h1"]);
}

/// A `delete` with `except` removes every entry of its directory but the kept ones, a symlink at its end as the link (the
/// target stays), and refuses a symlink on its way. Into the trash, the entries leave the checkout at once and the
/// trash counts their bytes once it deletes them; the trash of a run whose process is gone goes too
#[cfg(unix)]
#[test]
fn a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let outside = tempfile::tempdir().unwrap();
  std::fs::write(outside.path().join("target-file"), "kept").unwrap();
  for dir in ["out/dev-run/jar-cache", "out/dev-run/idea/lib", "out/notes", "real"] {
    std::fs::create_dir_all(root.join(dir)).unwrap();
  }
  for file in ["out/dev-run/jar-cache/j", "out/dev-run/idea/lib/a.jar", "out/dev-run/x.txt", "out/notes/handoff.md"] {
    std::fs::write(root.join(file), vec![1u8; 8192]).unwrap();
  }
  std::os::unix::fs::symlink(outside.path(), root.join("out/bazel-bin")).unwrap();
  std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();
  let rules =
    parse_clean_rules("[lease]\nclean = [\n  { delete = \"out/dev-run\", except = [\"jar-cache\"] },\n  { delete = \"out/bazel-bin\" },\n  { delete = \"link/x\" },\n]\n").unwrap();
  let trash_root = tempfile::tempdir_in(root.parent().unwrap()).unwrap();
  // the trash of a run that is gone: no process has the pid 0 with the start time 1
  std::fs::create_dir_all(trash_root.path().join("0-1-gone/0")).unwrap();
  let trash = Trash::new(trash_root.path());
  let mut cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Trash(&trash));
  assert!(!root.join("out/dev-run/idea").exists() && !root.join("out/dev-run/x.txt").exists());
  assert!(root.join("out/dev-run/jar-cache/j").exists() && root.join("out/notes/handoff.md").exists());
  assert!(root.join("out/bazel-bin").symlink_metadata().is_err() && outside.path().join("target-file").exists());
  assert!(cleaned[2].error.as_ref().unwrap().contains("is a symlink"), "{cleaned:?}");
  assert_eq!((cleaned[0].freed_bytes, cleaned[0].trashed.len()), (0, 2));
  let left = trash.empty(&mut cleaned);
  assert_eq!(left, Vec::<String>::new());
  assert!(cleaned[0].freed_bytes >= 2 * 8192 && cleaned[0].trashed.is_empty(), "{cleaned:?}");
  assert_eq!(std::fs::read_dir(trash_root.path()).unwrap().count(), 0, "the trash of this run and of the gone one");
}

/// A whitelist of a directory: `out` but for `dev-run` and a mark, and `out/dev-run` but for `jar-cache`, leave only
/// those; a new entry of `out` goes without an edit of the rules
#[cfg(unix)]
#[test]
fn nested_deletes_keep_only_the_whitelist() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  for file in ["out/dev-run/jar-cache/j", "out/dev-run/idea/lib.jar", "out/.mark", "out/handoff.md", "out/new-tool/x"] {
    std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
    std::fs::write(root.join(file), "x").unwrap();
  }
  let rules =
    parse_clean_rules("[lease]\nclean = [\n  { delete = \"out\", except = [\"dev-run\", \".mark\"] },\n  { delete = \"out/dev-run\", except = [\"jar-cache\"] },\n]\n").unwrap();
  let cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Delete);
  assert!(cleaned.iter().all(|rule| rule.error.is_none()), "{cleaned:?}");
  let mut left: Vec<String> = std::fs::read_dir(root.join("out"))
    .unwrap()
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  left.sort();
  assert_eq!(left, [".mark", "dev-run"]);
  assert_eq!(std::fs::read_dir(root.join("out/dev-run")).unwrap().count(), 1);
  assert!(root.join("out/dev-run/jar-cache/j").exists());
}
