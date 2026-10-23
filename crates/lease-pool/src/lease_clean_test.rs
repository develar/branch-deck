#[cfg(unix)]
use crate::lease_clean::apply_clean_rules;
use crate::lease_clean::{CleanRule, parse_clean_rules};
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
    parse_clean_rules("[lease]\nclean = [\n  { cargo = \"./target/\", keep = 3, age = \"30m\" },\n  { cargo = \"nested/target\" },\n  { delete = \"tests//results\" },\n]\n")
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
  let dry_run = apply_clean_rules(&git, checkout, &rules, false);
  assert!(dry_run[0].freed_bytes > 0 && dry_run[0].error.is_none(), "{dry_run:?}");
  assert!(root.join("target/debug/build/git-ops/h0").exists(), "a dry run deletes nothing");

  let cleaned = apply_clean_rules(&git, checkout, &rules, true);
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
  let cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, true);
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
