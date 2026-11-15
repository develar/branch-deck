use crate::lease::Pool;
use crate::lease_clean::{CleanMode, CleanRule, Removed, Trash, apply_clean_rules, describe_size, holds_gone_runs, parse_clean_rules, remove_measured_until};
use git_executor::git_command_executor::GitCommandExecutor;
use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(unix)]
use std::time::SystemTime;
use test_utils::git_in_directory::GitInDirectory;
use test_utils::links::directory_link;

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

/// `[host]` belongs to the remote runs of brd-host, so the rules and the pick check read past it, whatever it holds
#[test]
fn the_host_table_of_the_remote_runs_stays_unread() {
  let text = "[lease]\npick_requires_check = true\nclean = [{ delete = \"out\" }]\n\n[host]\noutputs = [\"out/x\"]\nunknown = 1\n";
  assert_eq!(parse_clean_rules(text).unwrap().len(), 1);
  assert!(crate::lease_clean::parse_pick_requires_check(text).unwrap());
}

/// A table that a later version adds (`[commit]`, `[other]`) leaves the rules of `[lease]` in force
#[test]
fn an_unknown_table_leaves_the_lease_rules_in_force() {
  let text = "[other]\nkey = 1\n\n[lease]\npick_requires_check = true\nclean = [{ delete = \"out\" }]\n\n[commit]\ncheck = 5\n";
  assert_eq!(parse_clean_rules(text).unwrap().len(), 1);
  assert!(crate::lease_clean::parse_pick_requires_check(text).unwrap());
  assert_eq!(parse_clean_rules("[other]\n").unwrap(), []);
}

/// An unknown key or kind, a wrong type and a bad age are errors of the whole file
#[test]
fn unknown_keys_and_kinds_are_errors() {
  for (text, says) in [
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

/// A `delete` of the dir of a cargo rule itself is a whitelist of that dir: with `except` it is a rule, without (no key
/// or an empty array) it would delete what the cargo rule keeps and is reported, never applied
#[test]
fn a_delete_of_the_dir_of_a_cargo_rule_needs_its_kept_entries() {
  let rules = parse_clean_rules(
    "[lease]\nclean = [\n  { cargo = \"target\" },\n  { delete = \"target\", except = [\"debug\", \"release\", \"CACHEDIR.TAG\"] },\n  { delete = \"target\" },\n  { delete = \"./target/\", except = [] },\n]\n",
  )
  .unwrap();
  let outcome: Vec<Result<String, String>> = rules.into_iter().map(|rule| rule.map(|rule| rule.describe()).map_err(|(_, reason)| reason)).collect();
  assert_eq!(
    outcome[..2],
    [Ok("cargo target".to_string()), Ok("delete target except debug, release, CACHEDIR.TAG".to_string())]
  );
  for index in [2, 3] {
    let reason = outcome[index].as_ref().unwrap_err();
    assert!(
      reason.contains("target, the dir of a cargo rule") && reason.contains("name the entries it keeps"),
      "{outcome:?}"
    );
  }
}

/// A path has one `delete`: a later one of the same path (normalized) is reported, never applied, so a dry run measures
/// each entry once, and a clone and a cargo rule of the dir see the `except` of the first
#[test]
fn a_second_delete_of_a_path_is_reported() {
  let rules = parse_clean_rules(
    "[lease]\nclean = [\n  { cargo = \"target\" },\n  { delete = \"target\", except = [\"debug\", \"release\"] },\n  { delete = \"./target/\", except = [\"debug\"] },\n  { delete = \"out\" },\n  { delete = \"out\", except = [\"a\"] },\n  { delete = \"out/a\" },\n]\n",
  )
  .unwrap();
  let outcome: Vec<Result<String, String>> = rules.iter().cloned().map(|rule| rule.map(|rule| rule.describe()).map_err(|(_, reason)| reason)).collect();
  for index in [0, 1, 3, 5] {
    assert!(outcome[index].is_ok(), "{outcome:?}");
  }
  assert!(outcome[2].as_ref().unwrap_err().contains("an earlier rule deletes target"), "{outcome:?}");
  assert!(outcome[4].as_ref().unwrap_err().contains("an earlier rule deletes out"), "{outcome:?}");
  assert_eq!(
    crate::lease_clean::clone_skips(&rules),
    [
      ("target".to_string(), vec!["debug".to_string(), "release".to_string()]),
      ("out".to_string(), Vec::new()),
      ("out/a".to_string(), Vec::new())
    ]
  );
}

/// The free space a message names is in GiB
/// A delete that may stop asks before its first entry and then once every 256: stopped at its second ask, it deleted 256
/// entries and leaves the rest where it was; asked to go on, it deletes the rest
#[test]
fn a_delete_stops_at_the_ask_its_stop_holds_and_leaves_the_rest() {
  let dir = tempfile::tempdir().unwrap();
  let tree = dir.path().join("target");
  std::fs::create_dir_all(&tree).unwrap();
  for file in 0..600 {
    std::fs::write(tree.join(file.to_string()), "x").unwrap();
  }
  let asks = std::cell::Cell::new(0);
  let stop = || {
    asks.set(asks.get() + 1);
    asks.get() == 2
  };
  let removed = remove_measured_until(&tree, &stop).unwrap();
  assert!(!removed.complete && removed.freed > 0, "{removed:?}");
  // the directory is the first entry, then 255 files before the second ask
  assert_eq!(std::fs::read_dir(&tree).unwrap().count(), 600 - 255);
  assert!(remove_measured_until(&tree, &|| false).unwrap().complete);
  assert!(!tree.exists());
  assert_eq!(remove_measured_until(&tree, &|| false).unwrap(), Removed { freed: 0, complete: true });
}

/// A trash whose delete stopped is left to the sweep of any run (`left-…`), which a get below the minimum finds and
/// deletes before it shrinks anything
#[test]
fn a_trash_whose_delete_stopped_is_left_to_the_next_sweep() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("trash");
  let output = dir.path().join("target");
  std::fs::create_dir_all(&output).unwrap();
  for file in 0..300 {
    std::fs::write(output.join(file.to_string()), "x").unwrap();
  }
  let trash = Trash::new(&root);
  let moved = trash.move_in(&output).unwrap();
  let mut cleaned = vec![crate::lease_clean::Cleaned {
    rule: "cargo target".to_string(),
    freed_bytes: 0,
    elapsed: Duration::ZERO,
    error: None,
    trashed: vec![moved],
    deferred: Vec::new(),
    profiles: crate::lease_clean::Profiles::default(),
  }];
  assert!(!holds_gone_runs(&root));
  let (left, complete) = trash.empty_until(&mut cleaned, &|| true);
  assert_eq!((left, complete), (Vec::<String>::new(), false));
  // what the delete left is in a directory any run sweeps
  let names: Vec<String> = std::fs::read_dir(&root)
    .unwrap()
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  assert!(names.len() == 1 && names[0].starts_with("left-"), "{names:?}");
  assert!(holds_gone_runs(&root));
  Trash::new(&root).sweep();
  assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
}

#[test]
fn a_free_space_is_named_in_gib() {
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
fn git_checkout() -> (tempfile::TempDir, GitCommandExecutor) {
  let checkout = tempfile::tempdir().unwrap();
  let git = GitCommandExecutor::new();
  git.execute_command(&["init", "-q"], checkout.path().to_str().unwrap()).unwrap();
  (checkout, git)
}

/// The names of the entries of `dir`, sorted
#[cfg(unix)]
fn entry_names(dir: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(dir)
    .unwrap()
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
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
    let left = |dir: &str| entry_names(&root.join(profile).join(dir));
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
  for path in classic_unit_files(profile, hash) {
    // the file and every dir up to the profile's own
    let mut current = path.as_path();
    while current != profile {
      age(current, built);
      current = current.parent().unwrap();
    }
  }
}

/// The files of a classic unit (`classic_unit`), built now
fn classic_unit_files(profile: &Path, hash: &str) -> Vec<PathBuf> {
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
  files
    .iter()
    .map(|file| {
      let path = profile.join(file);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(&path, vec![0u8; 4096]).unwrap();
      path
    })
    .collect()
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
  for profile in classic {
    let profile = root.join(profile);
    assert_eq!(
      entry_names(&profile.join("deps")),
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
    assert_eq!(entry_names(&profile.join(".fingerprint")), [format!("aho-corasick-{newest}")]);
    assert_eq!(entry_names(&profile.join("build")), [format!("aho-corasick-{newest}")]);
    assert!(profile.join(format!("build/aho-corasick-{newest}/out/generated.rs")).exists());
    for kept in [".cargo-lock", "libaho_corasick.rlib", "aho-corasick"] {
      assert!(profile.join(kept).exists(), "{kept}");
    }
  }
  assert_eq!(entry_names(&release.join("build/git-ops")), ["h1"]);
}

/// A `delete` with `except` removes every entry of its directory but the kept ones, a symlink at its end as the link (the
/// target stays, the link is deleted in place and never goes into the trash), and refuses a symlink on its way. Into
/// the trash, the entries leave the checkout at once and the trash counts their bytes once it deletes them; the trash of
/// a run whose process is gone goes too
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
  assert!(cleaned[1].error.is_none() && cleaned[1].trashed.is_empty(), "the link went into the trash: {cleaned:?}");
  let left = trash.empty(&mut cleaned);
  assert_eq!(left, Vec::<String>::new());
  assert!(cleaned[0].freed_bytes >= 2 * 8192 && cleaned[0].trashed.is_empty(), "{cleaned:?}");
  assert_eq!(std::fs::read_dir(trash_root.path()).unwrap().count(), 0, "the trash of this run and of the gone one");
}

/// The output base of a Bazel build out of the checkout, with a file in each of the dirs its `out/bazel-*` links lead
/// to
fn output_base(links: &[&str]) -> tempfile::TempDir {
  let output_base = tempfile::tempdir().unwrap();
  for link in links {
    let dir = output_base.path().join("execroot/_main").join(link);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("output"), "output").unwrap();
  }
  output_base
}

/// Bazel's `out/bazel-*` links (junctions on Windows, which issue #159 saw stuck in the trash with "Access is denied")
/// are deleted in place as links by a `delete` through the trash: none goes into the trash, no rule fails, what they
/// lead to stays, and the kept entry and the trash of the other entries are as without them
#[test]
fn a_delete_through_the_trash_deletes_bazel_links_in_place_and_keeps_their_targets() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let links = ["bazel-bin", "bazel-out"];
  let output_base = output_base(&links);
  for file in ["out/dev-run/kept", "out/notes/handoff.md"] {
    std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
    std::fs::write(root.join(file), vec![1u8; 8192]).unwrap();
  }
  for link in links {
    directory_link(&output_base.path().join("execroot/_main").join(link), &root.join("out").join(link));
    assert!(root.join("out").join(link).join("output").exists(), "the link leads to the output base");
  }
  let rules = parse_clean_rules("[lease]\nclean = [{ delete = \"out\", except = [\"dev-run\"] }]\n").unwrap();
  let trash_root = tempfile::tempdir_in(root.parent().unwrap()).unwrap();
  let trash = Trash::new(trash_root.path());

  let mut cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Trash(&trash));

  assert!(cleaned[0].error.is_none(), "{cleaned:?}");
  for link in links {
    assert!(root.join("out").join(link).symlink_metadata().is_err(), "{link} stays");
    assert_eq!(
      std::fs::read_to_string(output_base.path().join("execroot/_main").join(link).join("output")).unwrap(),
      "output"
    );
  }
  assert_eq!(cleaned[0].trashed.len(), 1, "only out/notes goes into the trash: {cleaned:?}");
  assert!(cleaned[0].trashed[0].join("handoff.md").exists(), "{cleaned:?}");
  assert!(!root.join("out/notes").exists() && root.join("out/dev-run/kept").exists());
  assert_eq!(trash.empty(&mut cleaned), Vec::<String>::new());
  assert!(cleaned[0].error.is_none() && cleaned[0].freed_bytes >= 8192, "{cleaned:?}");
  assert_eq!(std::fs::read_dir(trash_root.path()).unwrap().count(), 0, "the trash of this run");
}

/// A link that an earlier version moved into the trash (a junction on Windows, which it could not delete there) is
/// deleted as the link by the sweep of the trash of a run that is gone, what it leads to kept
#[test]
fn the_sweep_deletes_a_link_left_in_the_trash_of_a_gone_run() {
  let output_base = output_base(&["bazel-out"]);
  let trash_root = tempfile::tempdir().unwrap();
  // the trash of a run that is gone: no process has the pid 0 with the start time 1
  std::fs::create_dir_all(trash_root.path().join("0-1-gone")).unwrap();
  directory_link(&output_base.path().join("execroot/_main/bazel-out"), &trash_root.path().join("0-1-gone/0"));
  assert!(trash_root.path().join("0-1-gone/0/output").exists(), "the link leads to the output base");

  Trash::new(trash_root.path()).sweep();

  assert_eq!(std::fs::read_dir(trash_root.path()).unwrap().count(), 0, "the trash of the gone run");
  assert_eq!(std::fs::read_to_string(output_base.path().join("execroot/_main/bazel-out/output")).unwrap(), "output");
}

#[test]
fn an_eperm_in_an_app_bundle_names_finder_removal_in_trash_left() {
  let root = tempfile::tempdir().unwrap();
  let bundle = root.path().join("cef_server.app");
  let path = bundle.join("Contents/MacOS/cef_server");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, "signed app binary").unwrap();
  let error = crate::lease_clean::removal_error(&path, std::io::Error::from_raw_os_error(1));
  let left = crate::lease_clean::trash_left_path(&path, &error);

  if cfg!(target_os = "macos") {
    let message = error.to_string();
    assert!(message.contains(path.to_str().unwrap()), "{message}");
    assert!(message.contains(bundle.to_str().unwrap()), "{message}");
    assert!(message.contains("Finder"), "{message}");
    assert_eq!(left, message);
  } else {
    assert_eq!(left, path.display().to_string());
    assert!(!error.to_string().contains("Finder"));
  }

  let ordinary = crate::lease_clean::removal_error(&path, std::io::Error::from_raw_os_error(13));
  assert_eq!(crate::lease_clean::trash_left_path(&path, &ordinary), path.display().to_string());
}

/// Bazel takes the write bit from every directory of an output it wrote (`dr-xr-xr-x`), and without it no entry of
/// such a directory goes ("Permission denied"), which left the trash of gone runs of a Bazel repository lying: the
/// walk widens a directory it deletes and tries it again, and counts the bytes of what went before the refusal once
#[cfg(unix)]
#[test]
fn a_read_only_directory_of_a_bazel_output_goes_with_the_bytes_it_freed() {
  use std::io::Write;
  use std::os::unix::fs::PermissionsExt;
  let output_base = output_base(&["bazel-out"]);
  let execroot = output_base.path().join("execroot");
  let bin = execroot.join("_main/bazel-out/bin");
  std::fs::create_dir_all(&bin).unwrap();
  // on APFS the blocks of a file are its own only once it is on disk
  let mut file = std::fs::File::create(bin.join("output")).unwrap();
  file.write_all(&vec![1u8; 8192]).unwrap();
  file.sync_all().unwrap();
  drop(file);
  for dir in [bin.as_path(), bin.parent().unwrap(), execroot.as_path()] {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
  }
  let on_disk = crate::lease_clean::disk_size(&execroot);
  assert!(on_disk >= 8192, "the bytes of the tree: {on_disk}");

  let freed = crate::lease_clean::remove_measured(&execroot).unwrap();

  assert!(!execroot.exists(), "the read-only tree of the output base");
  assert_eq!(freed, on_disk, "the bytes of the tree, those of the try before the widening counted once");
}

/// The same tree in the trash of a run that is gone: the sweep deletes it whole, where it reported "Permission denied"
/// and kept it
#[cfg(unix)]
#[test]
fn the_sweep_deletes_the_read_only_trash_of_a_gone_run() {
  use std::os::unix::fs::PermissionsExt;
  let trash_root = tempfile::tempdir().unwrap();
  // the trash of a run that is gone: no process has the pid 0 with the start time 1
  let output = trash_root.path().join("0-1-gone/0/execroot/_main/bazel-out");
  std::fs::create_dir_all(&output).unwrap();
  std::fs::write(output.join("output"), "output").unwrap();
  std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o555)).unwrap();

  Trash::new(trash_root.path()).sweep();

  assert_eq!(std::fs::read_dir(trash_root.path()).unwrap().count(), 0, "the trash of the gone run");
}

#[test]
fn trash_bytes_measure_each_run_with_the_shared_walk() {
  let root = tempfile::tempdir().unwrap();
  let trash = root.path().join("branch-deck/trash");
  let first = trash.join("first");
  let second = trash.join("second");
  std::fs::create_dir_all(&first).unwrap();
  std::fs::create_dir_all(&second).unwrap();
  std::fs::write(first.join("output"), vec![1u8; 8192]).unwrap();
  std::fs::write(second.join("output"), vec![2u8; 4096]).unwrap();
  let expected = fs_walk::tree_size(&first) + fs_walk::tree_size(&second);

  assert_eq!(Pool::new(root.path()).trash_bytes(), expected);
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
  assert_eq!(entry_names(&root.join("out")), [".mark", "dev-run"]);
  assert_eq!(std::fs::read_dir(root.join("out/dev-run")).unwrap().count(), 1);
  assert!(root.join("out/dev-run/jar-cache/j").exists());
}

/// Every path under `dir`, sorted, symlinks not followed
#[cfg(unix)]
fn tree(dir: &Path) -> Vec<PathBuf> {
  let mut paths = Vec::new();
  for entry in std::fs::read_dir(dir).unwrap().flatten() {
    paths.push(entry.path());
    if entry.file_type().unwrap().is_dir() {
      paths.extend(tree(&entry.path()));
    }
  }
  paths.sort();
  paths
}

/// What one rule of a pair did in a clean through the trash
#[cfg(unix)]
struct RuleRun {
  freed_bytes: u64,
  /// The paths it moved into the trash
  trashed: usize,
}

/// The rules `delete` and `cargo` in either order, each time on a new checkout that `setup` fills: a dry run changes
/// nothing and measures per rule what a clean through the trash then frees, byte for byte, without an error; `check`
/// reads the checkout after the clean and what the delete and the cargo rule did
#[cfg(unix)]
fn clean_in_either_order(delete: &str, cargo: &str, setup: impl Fn(&Path), check: impl Fn(&Path, &RuleRun, &RuleRun)) {
  for order in [[delete, cargo], [cargo, delete]] {
    let (checkout, git) = git_checkout();
    let root = checkout.path();
    setup(root);
    let rules = parse_clean_rules(&format!("[lease]\nclean = [{}, {}]\n", order[0], order[1])).unwrap();
    assert!(rules.iter().all(Result::is_ok), "{rules:?}");
    let checkout = root.to_str().unwrap();
    let before = tree(root);
    let dry_run = apply_clean_rules(&git, checkout, &rules, CleanMode::Measure);
    assert_eq!(tree(root), before, "{order:?}: a dry run deletes nothing");
    let trash_root = tempfile::tempdir_in(root.parent().unwrap()).unwrap();
    let trash = Trash::new(trash_root.path());
    let mut cleaned = apply_clean_rules(&git, checkout, &rules, CleanMode::Trash(&trash));
    let trashed: Vec<usize> = cleaned.iter().map(|rule| rule.trashed.len()).collect();
    assert_eq!(trash.empty(&mut cleaned), Vec::<String>::new());
    assert!(cleaned.iter().all(|rule| rule.error.is_none()), "{order:?}: {cleaned:?}");
    let freed = |outcome: &[crate::lease_clean::Cleaned]| outcome.iter().map(|rule| rule.freed_bytes).collect::<Vec<_>>();
    assert_eq!(freed(&dry_run), freed(&cleaned), "{order:?}: each rule frees what its dry run measured");
    let run = |index: usize| RuleRun {
      freed_bytes: cleaned[index].freed_bytes,
      trashed: trashed[index],
    };
    let (delete_index, cargo_index) = if order[0] == delete { (0, 1) } else { (1, 0) };
    check(root, &run(delete_index), &run(cargo_index));
  }
}

/// A `delete` of a target dir with `except` and the cargo rule of that dir, in either order: the stray entries of the
/// target dir go whole (a nested target dir with a profile dir of its own, scratch dirs, a patch, a link as the link),
/// the kept entries stay, and the cargo rule prunes only the profile dirs it keeps. The two never take the same path,
/// so a dry run measures what each rule then frees through the trash, byte for byte
#[cfg(unix)]
#[test]
fn a_delete_of_a_target_dir_keeps_its_profile_dirs_for_the_cargo_rule() {
  let hour = Duration::from_hours(1);
  let outside = tempfile::tempdir().unwrap();
  std::fs::write(outside.path().join("kept"), "kept").unwrap();
  let setup = |root: &Path| {
    let target = root.join("target");
    for (profile, units) in [
      ("debug", [("h0", 10 * hour), ("h1", 9 * hour), ("h2", hour / 6)].as_slice()),
      ("release", [("h0", 10 * hour), ("h1", 2 * hour)].as_slice()),
      ("base40-target/debug", [("h0", 10 * hour), ("h1", 9 * hour)].as_slice()),
    ] {
      let profile = target.join(profile);
      std::fs::create_dir_all(&profile).unwrap();
      std::fs::write(profile.join(".cargo-lock"), "").unwrap();
      for (hash, built) in units {
        unit(&profile.join("build/git-ops").join(hash), *built);
      }
    }
    for file in ["CACHEDIR.TAG", ".rustc_info.json", "ab1/notes.md", "issue-138.patch"] {
      std::fs::create_dir_all(target.join(file).parent().unwrap()).unwrap();
      std::fs::write(target.join(file), vec![1u8; 8192]).unwrap();
    }
    std::os::unix::fs::symlink(outside.path(), target.join("scratch")).unwrap();
  };
  clean_in_either_order(
    "{ delete = \"target\", except = [\"debug\", \"release\", \"tools\", \"CACHEDIR.TAG\"] }",
    "{ cargo = \"target\", keep = 1, age = \"1h\" }",
    setup,
    |root, delete, cargo| {
      let target = root.join("target");
      assert_eq!(cargo.trashed, 3, "the stale units of debug and release alone");
      assert!(cargo.freed_bytes > 0 && delete.freed_bytes >= 3 * 8192 + 2 * 4096);
      assert_eq!(entry_names(&target), ["CACHEDIR.TAG", "debug", "release"]);
      assert_eq!(entry_names(&target.join("debug/build/git-ops")), ["h2"]);
      assert_eq!(entry_names(&target.join("release/build/git-ops")), ["h1"]);
      assert!(target.join("debug/.cargo-lock").exists() && outside.path().join("kept").exists());
    },
  );
}

/// An `except` entry of that delete names an entry byte for byte, for the cargo rule as for the delete: `Debug` keeps no
/// `debug`, also on a volume that ignores case (APFS, NTFS), so the delete removes `debug` whole and the cargo rule
/// prunes nothing of it, and a dry run measures it once
#[cfg(unix)]
#[test]
fn a_kept_entry_of_a_target_dir_is_its_exact_name() {
  let hour = Duration::from_hours(1);
  let setup = |root: &Path| {
    for profile in ["debug", "release"] {
      let profile = root.join("target").join(profile);
      std::fs::create_dir_all(&profile).unwrap();
      std::fs::write(profile.join(".cargo-lock"), "").unwrap();
      unit(&profile.join("build/git-ops/h0"), 10 * hour);
      unit(&profile.join("build/git-ops/h1"), 9 * hour);
    }
  };
  clean_in_either_order(
    "{ delete = \"target\", except = [\"Debug\", \"release\"] }",
    "{ cargo = \"target\", keep = 1, age = \"1h\" }",
    setup,
    |root, delete, cargo| {
      assert_eq!(cargo.trashed, 1, "the stale unit of release alone");
      assert!(delete.freed_bytes >= 2 * 4096);
      assert_eq!(entry_names(&root.join("target")), ["release"]);
      assert_eq!(entry_names(&root.join("target/release/build/git-ops")), ["h1"]);
    },
  );
}

/// The dir of the cargo rule may be a profile dir itself (`target/debug`), its delete a whitelist that drops the
/// uplifted artifacts: the cargo rule prunes the units of the kept dirs alone (`deps`, `build`), the delete takes the
/// rest whole (`.fingerprint`, which it does not keep, the uplifted files), in either order, a dry run measuring each
#[cfg(unix)]
#[test]
fn a_delete_of_a_profile_dir_keeps_its_unit_dirs_for_the_cargo_rule() {
  let hour = Duration::from_hours(1);
  let (old, newest) = ("1111111111111111", "3333333333333333");
  let setup = |root: &Path| {
    let profile = root.join("target/debug");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join(".cargo-lock"), "").unwrap();
    std::fs::write(profile.join("aho-corasick"), vec![1u8; 8192]).unwrap();
    classic_unit(&profile, old, 10 * hour);
    classic_unit(&profile, newest, 2 * hour);
  };
  clean_in_either_order(
    "{ delete = \"target/debug\", except = [\"deps\", \"build\", \"incremental\", \".cargo-lock\"] }",
    "{ cargo = \"target/debug\", keep = 1, age = \"1h\" }",
    setup,
    |root, delete, cargo| {
      let profile = root.join("target/debug");
      assert!(delete.freed_bytes >= 8192 && cargo.freed_bytes >= 6 * 4096, "the old unit in deps and build");
      assert_eq!(entry_names(&profile), [".cargo-lock", "build", "deps"]);
      assert!(entry_names(&profile.join("deps")).iter().all(|name| name.contains(newest)));
      assert_eq!(entry_names(&profile.join("build")), [format!("aho-corasick-{newest}")]);
    },
  );
}

/// A process that works in a stale unit or in an entry of a `delete` (a shell or a build of an agent) keeps it: on Windows
/// no rename moves it, so the unit (its files moved before put back) and the entry stay whole in the checkout, the error
/// of the rule names them and says that a process holds them, and the rest goes through the trash; once the processes
/// exited, the next clean takes them. Elsewhere the rename moves them from under the process, and everything goes at once
#[test]
fn a_held_unit_and_a_held_entry_stay_whole_and_the_next_clean_takes_them() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = root.join("target/debug");
  std::fs::create_dir_all(&profile).unwrap();
  std::fs::write(profile.join(".cargo-lock"), "").unwrap();
  let (held, free) = ("1111111111111111", "2222222222222222");
  let held_files = classic_unit_files(&profile, held);
  let free_files = classic_unit_files(&profile, free);
  for file in ["out/kept/k", "out/held/h", "out/free/f"] {
    std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
    std::fs::write(root.join(file), "x").unwrap();
  }
  // the fingerprint dir of the unit comes after its files in `deps/`, which go back
  let holders = [
    GitInDirectory::start(&profile.join(format!(".fingerprint/aho-corasick-{held}"))),
    GitInDirectory::start(&root.join("out/held")),
  ];
  let rules = parse_clean_rules("[lease]\nclean = [\n  { cargo = \"target\", keep = 0, age = \"0s\" },\n  { delete = \"out\", except = [\"kept\"] },\n]\n").unwrap();
  let trash_root = tempfile::tempdir_in(root.parent().unwrap()).unwrap();
  let clean = || {
    let trash = Trash::new(trash_root.path());
    let mut cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Trash(&trash));
    assert_eq!(trash.empty(&mut cleaned), Vec::<String>::new());
    cleaned
  };

  let cleaned = clean();

  assert!(free_files.iter().all(|file| !file.exists()) && !root.join("out/free").exists(), "{cleaned:?}");
  assert!(root.join("out/kept/k").exists());
  if cfg!(windows) {
    assert!(held_files.iter().all(|file| file.exists()), "the held unit is not whole: {cleaned:?}");
    assert!(root.join("out/held/h").exists());
    let (unit, entry) = (cleaned[0].error.clone().unwrap_or_default(), cleaned[1].error.clone().unwrap_or_default());
    for (error, held) in [(&unit, format!("aho-corasick-{held}")), (&entry, "held".to_string())] {
      assert!(error.contains("cannot move") && error.contains(&held) && error.contains("a process holds it"), "{error}");
    }
    drop(holders);
    let again = clean();
    assert!(held_files.iter().all(|file| !file.exists()) && !root.join("out/held").exists(), "{again:?}");
    assert!(again.iter().all(|rule| rule.error.is_none() && rule.freed_bytes > 0), "{again:?}");
  } else {
    drop(holders);
    assert!(held_files.iter().all(|file| !file.exists()) && !root.join("out/held").exists(), "{cleaned:?}");
    assert!(cleaned.iter().all(|rule| rule.error.is_none()), "{cleaned:?}");
  }
  assert!(cleaned.iter().all(|rule| rule.freed_bytes > 0), "{cleaned:?}");
}

/// The `.branchdeck.toml` of this repository, with its `[host]` table, gives its clean rules: the stale units of
/// `target/` and the two directories of the e2e artifacts; the notes of a lease are no path of its worktree
/// (`lease_notes`), so no rule deletes them
#[test]
fn the_rules_of_this_repository_parse() {
  assert_eq!(parse_clean_rules(include_str!("../../../.branchdeck.toml")).unwrap().len(), 3);
}

/// An APFS clone shares its blocks with its template: the delete of the clone frees near nothing, and the delete of the
/// template after it frees every block, which is what the two deletes free on the volume
#[cfg(target_os = "macos")]
#[test]
fn a_clone_frees_only_the_blocks_it_shares_with_no_other_file() {
  use std::io::Write;
  const SIZE: u64 = 4 * 1024 * 1024;
  let dir = tempfile::tempdir().unwrap();
  let template = dir.path().join("template");
  std::fs::create_dir_all(template.join("0")).unwrap();
  // on APFS the blocks of a file are its own only once it is on disk
  let mut file = std::fs::File::create(template.join("0/0.bin")).unwrap();
  file.write_all(&vec![1u8; usize::try_from(SIZE).unwrap()]).unwrap();
  file.sync_all().unwrap();
  drop(file);
  let clone = dir.path().join("clone");
  crate::clone_dir::clone_directory(&template, &clone, &crate::clone_dir::CloneSkips::default()).unwrap();

  let clone_freed = crate::lease_clean::remove_measured(&clone).unwrap();
  let template_freed = crate::lease_clean::remove_measured(&template).unwrap();

  assert!(clone_freed < SIZE / 4, "the clone shares its blocks: {clone_freed}");
  assert!((SIZE..2 * SIZE).contains(&template_freed), "the template holds them alone at its delete: {template_freed}");
}

/// A file with two hard links frees its blocks with its last link only: the delete of a tree that holds one of them
/// frees nothing of it, and the delete of the other frees all
#[cfg(unix)]
#[test]
fn a_hard_link_frees_nothing_while_another_link_keeps_the_file() {
  use std::io::Write;
  const SIZE: u64 = 1024 * 1024;
  let dir = tempfile::tempdir().unwrap();
  let (deps, profile) = (dir.path().join("deps"), dir.path().join("profile"));
  std::fs::create_dir_all(&deps).unwrap();
  std::fs::create_dir_all(&profile).unwrap();
  let mut file = std::fs::File::create(deps.join("lib.rlib")).unwrap();
  file.write_all(&vec![1u8; usize::try_from(SIZE).unwrap()]).unwrap();
  file.sync_all().unwrap();
  drop(file);
  std::fs::hard_link(deps.join("lib.rlib"), profile.join("lib.rlib")).unwrap();

  let profile_freed = crate::lease_clean::remove_measured(&profile).unwrap();
  let deps_freed = crate::lease_clean::remove_measured(&deps).unwrap();

  assert!(profile_freed < SIZE / 4, "the link in deps keeps the file: {profile_freed}");
  assert!((SIZE..2 * SIZE).contains(&deps_freed), "the last link frees the file: {deps_freed}");
}

/// A profile dir at `target/debug` of `root` with the lock files of cargo
#[cfg(unix)]
fn profile_dir(root: &Path) -> PathBuf {
  let profile = root.join("target/debug");
  std::fs::create_dir_all(&profile).unwrap();
  for lock in [".cargo-lock", ".cargo-build-lock"] {
    std::fs::write(profile.join(lock), "").unwrap();
  }
  profile
}

/// A file of 8 KiB at `path`, written `old` ago
#[cfg(unix)]
fn output(path: &Path, old: Duration) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, vec![7u8; 8192]).unwrap();
  age(path, old);
}

/// The cargo rule of the object tests: the 8 newest units of a group and the ones of the last hour stay
#[cfg(unix)]
const OBJECT_RULES: &str = "[lease]\nclean = [{ cargo = \"target\", keep = 8, age = \"1h\" }]\n";

/// Apply `OBJECT_RULES` to `root`: the delete frees one object of 8 KiB, without an error
#[cfg(unix)]
fn prune_one_object(git: &GitCommandExecutor, root: &Path) {
  let cleaned = apply_clean_rules(git, root.to_str().unwrap(), &parse_clean_rules(OBJECT_RULES).unwrap(), CleanMode::Delete);
  assert_eq!((cleaned[0].freed_bytes, &cleaned[0].error), (8192, &None), "{cleaned:?}");
}

/// A kept unit that links only an rlib: each old object of an incremental compile that no session holds goes, also two
/// names of one inode; a session-held, a young and a non-incremental object stay. A dry run counts what the delete frees,
/// the shared inode once
#[cfg(unix)]
#[test]
fn the_cargo_rule_deletes_the_objects_of_a_unit_that_links_only_an_rlib() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = profile_dir(root);
  let out = profile.join("build/app/h1/out");
  let hours = |hours: u64| Duration::from_hours(hours);
  for file in ["libapp-h1.rlib", "libapp-h1.rmeta", "app-h1.d"] {
    output(&out.join(file), hours(2));
  }
  let object = |name: &str| out.join(format!("app-h1.{name}.rcgu.o"));
  output(&object("held.0aaaaaa"), hours(3));
  let session = profile.join("incremental/app-x/s-1-abc");
  std::fs::create_dir_all(&session).unwrap();
  std::fs::hard_link(object("held.0aaaaaa"), session.join("held.o")).unwrap();
  output(&object("shared.0aaaaaa"), hours(3));
  std::fs::hard_link(object("shared.0aaaaaa"), object("shared.0bbbbbb")).unwrap();
  output(&object("single.0aaaaaa"), hours(3));
  output(&object("young.0bbbbbb"), Duration::from_mins(5));
  output(&out.join("app-h1.app.4f50d93f3af7ef9a-cgu.0.rcgu.o"), hours(3));
  unit(&profile.join("build/app/h1"), hours(2));

  let rules = parse_clean_rules(OBJECT_RULES).unwrap();
  let checkout = root.to_str().unwrap();
  let dry_run = apply_clean_rules(&git, checkout, &rules, CleanMode::Measure);
  assert_eq!((dry_run[0].freed_bytes > 0, &dry_run[0].error), (true, &None), "{dry_run:?}");
  assert!(object("single.0aaaaaa").exists(), "a dry run deletes nothing");
  let cleaned = apply_clean_rules(&git, checkout, &rules, CleanMode::Delete);
  assert_eq!((cleaned[0].freed_bytes, &cleaned[0].error), (dry_run[0].freed_bytes, &None));
  assert_eq!(
    entry_names(&out),
    [
      "app-h1.app.4f50d93f3af7ef9a-cgu.0.rcgu.o",
      "app-h1.d",
      "app-h1.held.0aaaaaa.rcgu.o",
      "app-h1.young.0bbbbbb.rcgu.o",
      "libapp-h1.rlib",
      "libapp-h1.rmeta",
    ]
  );
  assert!(session.join("held.o").exists());
}

/// A kept executable names its own objects: the objects of the invocation that wrote the newest object stay, and so does
/// an old object whose inode that invocation names; the other old ones go
#[cfg(unix)]
#[test]
fn the_cargo_rule_keeps_the_objects_of_the_newest_invocation_of_an_executable() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = profile_dir(root);
  let out = profile.join("build/app/h2/out");
  let hours = |hours: u64| Duration::from_hours(hours);
  output(&out.join("app-h2"), hours(2));
  output(&out.join("app-h2.d"), hours(2));
  let object = |name: &str| out.join(format!("app-h2.{name}.rcgu.o"));
  output(&object("cgu0.0aaaaaa"), hours(4));
  output(&object("cgu1.0aaaaaa"), hours(4));
  output(&object("cgu0.0bbbbbb"), hours(2));
  std::fs::hard_link(object("cgu1.0aaaaaa"), object("cgu1.0bbbbbb")).unwrap();
  unit(&profile.join("build/app/h2"), hours(2));

  prune_one_object(&git, root);
  assert_eq!(
    entry_names(&out),
    [
      "app-h2",
      "app-h2.cgu0.0bbbbbb.rcgu.o",
      "app-h2.cgu1.0aaaaaa.rcgu.o",
      "app-h2.cgu1.0bbbbbb.rcgu.o",
      "app-h2.d",
    ]
  );
}

/// Two units of one crate share one incremental dir, whose session holds a few objects of the first and every object of
/// the second: the newest invocation of each keeps all its objects, also the ones no session holds
#[cfg(unix)]
#[test]
fn sibling_units_of_one_incremental_dir_keep_their_newest_objects() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = profile_dir(root);
  let session = profile.join("incremental/app-x/s-2-def");
  std::fs::create_dir_all(&session).unwrap();
  let hours = |hours: u64| Duration::from_hours(hours);
  for (unit_hash, invocation, held) in [("u1", "0cccccc", 2), ("u2", "0eeeeee", 3)] {
    let out = profile.join("build/app").join(unit_hash).join("out");
    output(&out.join(format!("app-{unit_hash}")), hours(2));
    for index in 0..3 {
      let object = out.join(format!("app-{unit_hash}.cgu{index}.{invocation}.rcgu.o"));
      output(&object, hours(2));
      if index < held {
        std::fs::hard_link(&object, session.join(format!("{unit_hash}-{index}.o"))).unwrap();
      }
    }
    unit(&profile.join("build/app").join(unit_hash), hours(2));
  }
  // the newest object of u1 is the one no session holds; an older invocation of u1 goes
  let u1 = profile.join("build/app/u1/out");
  age(&u1.join("app-u1.cgu2.0cccccc.rcgu.o"), Duration::from_mins(90));
  output(&u1.join("app-u1.cgu0.0dddddd.rcgu.o"), hours(4));

  prune_one_object(&git, root);
  assert_eq!(
    entry_names(&u1),
    ["app-u1", "app-u1.cgu0.0cccccc.rcgu.o", "app-u1.cgu1.0cccccc.rcgu.o", "app-u1.cgu2.0cccccc.rcgu.o"]
  );
  assert_eq!(entry_names(&profile.join("build/app/u2/out")).len(), 4);
}

/// The classic layout: the objects of a unit are in `deps/`, beside its executable; the old invocation goes, the newest
/// stays
#[cfg(unix)]
#[test]
fn the_cargo_rule_deletes_the_stale_objects_of_a_classic_unit() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = profile_dir(root);
  let hash = "4444444444444444";
  let hours = |hours: u64| Duration::from_hours(hours);
  let deps = profile.join("deps");
  output(&deps.join(format!("app-{hash}")), hours(2));
  output(&deps.join(format!("app-{hash}.d")), hours(2));
  output(&deps.join(format!("app-{hash}.cgu0.0aaaaaa.rcgu.o")), hours(4));
  output(&deps.join(format!("app-{hash}.cgu0.0bbbbbb.rcgu.o")), hours(2));
  output(&profile.join(format!(".fingerprint/app-{hash}/test-bin-app")), hours(2));

  prune_one_object(&git, root);
  assert_eq!(
    entry_names(&deps),
    [format!("app-{hash}"), format!("app-{hash}.cgu0.0bbbbbb.rcgu.o"), format!("app-{hash}.d")]
  );
}

/// A registry build script named `build_script_build`, as the build scripts of the workspace in `incremental/`, a
/// registry library and a release executable: their objects are of a compile without incremental, and every one stays
#[cfg(unix)]
#[test]
fn the_cargo_rule_keeps_the_objects_of_units_with_other_names() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = profile_dir(root);
  let hours = |hours: u64| Duration::from_hours(hours);
  std::fs::create_dir_all(profile.join("incremental/build_script_build-y/s-1-abc")).unwrap();
  let units = [
    ("serde/h3", "build_script_build", "build_script_build-h3"),
    ("serde/h4", "libserde-h4.rlib", "serde-h4"),
    ("tool/h5", "tool-h5", "tool-h5"),
  ];
  for (unit_dir, artifact, stem) in units {
    let out = profile.join("build").join(unit_dir).join("out");
    output(&out.join(artifact), hours(3));
    output(&out.join(format!("{stem}.{}.0123456789abcdef-cgu.0.rcgu.o", stem.split('-').next().unwrap())), hours(3));
    unit(&profile.join("build").join(unit_dir), hours(3));
  }
  let cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &parse_clean_rules(OBJECT_RULES).unwrap(), CleanMode::Delete);
  assert_eq!((cleaned[0].freed_bytes, &cleaned[0].error), (0, &None), "{cleaned:?}");
  for (unit_dir, ..) in units {
    assert_eq!(entry_names(&profile.join("build").join(unit_dir).join("out")).len(), 2, "{unit_dir}");
  }
}

/// A process holds `.cargo-build-lock` of one profile dir, then `.cargo-lock`: that dir stays whole and the error names
/// it, the other dir is pruned, and the lock files stay, free once the rule ends
#[cfg(unix)]
#[test]
fn a_profile_dir_whose_cargo_lock_is_held_stays_whole() {
  for held in [".cargo-build-lock", ".cargo-lock"] {
    let (checkout, git) = git_checkout();
    let root = checkout.path();
    for profile in ["target/debug", "target/release"] {
      let profile = root.join(profile);
      std::fs::create_dir_all(&profile).unwrap();
      for lock in [".cargo-lock", ".cargo-build-lock"] {
        std::fs::write(profile.join(lock), "").unwrap();
      }
      unit(&profile.join("build/app/old"), Duration::from_hours(3));
      unit(&profile.join("build/app/new"), Duration::from_hours(2));
    }
    let lock = std::fs::File::open(root.join("target/debug").join(held)).unwrap();
    lock.try_lock().unwrap();
    let rules = parse_clean_rules("[lease]\nclean = [{ cargo = \"target\", keep = 1, age = \"1h\" }]\n").unwrap();
    let cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Delete);
    assert_eq!(
      cleaned[0].error.as_deref(),
      Some("the lock of cargo is held (a build or another clean): target/debug"),
      "{held}"
    );
    assert_eq!(entry_names(&root.join("target/debug/build/app")), ["new", "old"], "{held}");
    assert_eq!(entry_names(&root.join("target/release/build/app")), ["new"], "{held}");
    drop(lock);
    for lock in [".cargo-lock", ".cargo-build-lock"] {
      let file = std::fs::File::open(root.join("target/release").join(lock)).unwrap();
      file.try_lock().unwrap_or_else(|error| panic!("{lock} stays held after the rule: {error}"));
    }
  }
}

/// A lock that another holder releases during the wait is taken; without a wait a held one is refused at once, and a
/// profile dir of a cargo before 1.91, which has only `.cargo-lock`, is locked by it alone
#[test]
fn a_cargo_lock_that_is_released_during_the_wait_is_taken() {
  let dir = tempfile::tempdir().unwrap();
  let profile = dir.path();
  std::fs::write(profile.join(".cargo-lock"), "").unwrap();
  crate::lease_clean::CargoLock::take(profile, Duration::ZERO).unwrap();
  std::fs::write(profile.join(".cargo-build-lock"), "").unwrap();
  let held = std::fs::File::open(profile.join(".cargo-build-lock")).unwrap();
  held.try_lock().unwrap();
  let (release, released) = std::sync::mpsc::channel::<()>();
  let holder = std::thread::spawn(move || {
    released.recv().unwrap();
    drop(held);
  });
  let refused = crate::lease_clean::CargoLock::take(profile, Duration::ZERO).err().map(|error| error.to_string());
  assert_eq!(refused.as_deref(), Some("the lock of cargo is held (a build or another clean)"));
  release.send(()).unwrap();
  let taken = crate::lease_clean::CargoLock::take(profile, Duration::from_secs(10));
  holder.join().unwrap();
  assert!(taken.is_ok(), "{:?}", taken.err());
}

/// A profile dir at `target/debug` of `root` with two units of `app`: `old`, built 3 hours ago, and `new`, 2 hours ago
#[cfg(unix)]
fn old_and_new_units(root: &Path) -> PathBuf {
  let profile = profile_dir(root);
  unit(&profile.join("build/app/old"), Duration::from_hours(3));
  unit(&profile.join("build/app/new"), Duration::from_hours(2));
  profile
}

/// The trash of the run is on another volume: the stale units go into `.brd-trash/` of their profile dir under the
/// locks, and its delete comes with the trash, after the release; a `.brd-trash/` dir of a gone run is swept
#[cfg(unix)]
#[test]
fn a_cargo_rule_on_another_volume_moves_its_units_beside_them() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = old_and_new_units(root);
  let gone = profile.join(".brd-trash/999999999-1-gone");
  output(&gone.join("0/fingerprint/dep"), Duration::from_hours(5));
  let trash_root = root.join("trash");
  let trash = Trash::on_another_volume(&trash_root);
  let rules = parse_clean_rules("[lease]\nclean = [{ cargo = \"target\", keep = 1, age = \"1h\" }]\n").unwrap();
  let mut cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Trash(&trash));
  assert_eq!((cleaned[0].trashed.len(), &cleaned[0].error), (0, &None), "{cleaned:?}");
  assert_eq!(entry_names(&profile.join("build/app")), ["new"]);
  let [own] = cleaned[0].deferred.as_slice() else { panic!("{cleaned:?}") };
  assert!(own.starts_with(profile.join(".brd-trash")) && own.join("0/fingerprint/dep").exists(), "{own:?}");
  assert!(!gone.exists(), "the sweep deletes the dir of a gone run");
  let own = own.clone();
  assert_eq!(trash.empty(&mut cleaned), Vec::<String>::new());
  assert!(cleaned[0].freed_bytes > 0 && !own.exists(), "{cleaned:?}");
  assert!(!trash_root.exists());
}

/// A kept unit with 300 stale objects, cleaned with the trash
#[cfg(unix)]
fn stale_objects_fixture() -> (tempfile::TempDir, GitCommandExecutor, PathBuf) {
  let (checkout, git) = git_checkout();
  let profile = profile_dir(checkout.path());
  let out = profile.join("build/app/h1/out");
  output(&out.join("libapp-h1.rlib"), Duration::from_hours(2));
  for index in 0..300 {
    output(&out.join(format!("app-h1.cgu{index}.0aaaaaa.rcgu.o")), Duration::from_hours(3));
  }
  unit(&profile.join("build/app/h1"), Duration::from_hours(2));
  (checkout, git, out)
}

/// The stale objects of a clean with the trash stay in place, never in the trash, until it is deleted; then they go in
/// place with their bytes counted, one count of entries for all of them, so the stop is asked twice for 300 objects. A
/// delete that stops leaves the rest for the next clean
#[cfg(unix)]
#[test]
fn a_clean_with_the_trash_deletes_the_stale_objects_in_place_once_it_empties() {
  let rules = parse_clean_rules(OBJECT_RULES).unwrap();
  let objects = |out: &Path| entry_names(out).iter().filter(|name| name.ends_with(".rcgu.o")).count();
  let (checkout, git, out) = stale_objects_fixture();
  let trash_root = checkout.path().join("trash");
  let trash = Trash::new(&trash_root);
  let mut cleaned = apply_clean_rules(&git, checkout.path().to_str().unwrap(), &rules, CleanMode::Trash(&trash));
  assert_eq!((objects(&out), cleaned[0].deferred.len(), cleaned[0].freed_bytes), (300, 300, 0), "{:?}", cleaned[0].error);
  assert!(!trash_root.exists(), "no object goes into the trash");
  let asks = std::sync::atomic::AtomicUsize::new(0);
  let (left, complete) = trash.empty_until(&mut cleaned, &|| {
    asks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    false
  });
  assert_eq!((left.len(), complete, objects(&out)), (0, true, 0));
  assert_eq!((asks.into_inner(), cleaned[0].freed_bytes), (2, 300 * 8192));

  let (checkout, git, out) = stale_objects_fixture();
  let trash = Trash::new(&checkout.path().join("trash"));
  let mut cleaned = apply_clean_rules(&git, checkout.path().to_str().unwrap(), &rules, CleanMode::Trash(&trash));
  let asks = std::sync::atomic::AtomicUsize::new(0);
  let (_, complete) = trash.empty_until(&mut cleaned, &|| asks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) > 0);
  assert_eq!((complete, objects(&out)), (false, 300 - 256));
  let mut again = apply_clean_rules(&git, checkout.path().to_str().unwrap(), &rules, CleanMode::Trash(&trash));
  assert_eq!(Trash::new(&checkout.path().join("trash")).empty(&mut again), Vec::<String>::new());
  assert_eq!(objects(&out), 0, "the next clean deletes the rest");
}

/// A package with more current targets than `keep`: each target its fingerprint names keeps its own `keep` newest
/// units, so the old test harness stays beside the newer libraries
#[cfg(unix)]
#[test]
fn each_target_of_a_package_keeps_its_own_newest_units() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = profile_dir(root);
  let hours = |hours: u64| Duration::from_hours(hours);
  for (hash, target, built) in [
    ("l1", "lib-app", hours(2)),
    ("l2", "lib-app", hours(3)),
    ("l3", "lib-app", hours(4)),
    ("t1", "test-lib-app", hours(5)),
  ] {
    let dir = profile.join("build/app").join(hash);
    unit(&dir, built);
    output(&dir.join("fingerprint").join(target), built);
    output(&dir.join("fingerprint").join(format!("{target}.json")), built);
    age(&dir.join("fingerprint"), built);
    age(&dir, built);
  }
  let rules = parse_clean_rules("[lease]\nclean = [{ cargo = \"target\", keep = 2, age = \"1h\" }]\n").unwrap();
  let cleaned = apply_clean_rules(&git, root.to_str().unwrap(), &rules, CleanMode::Delete);
  assert_eq!(cleaned[0].error, None);
  assert_eq!(entry_names(&profile.join("build/app")), ["l1", "l2", "t1"]);
}

/// The walk reads the units before the locks of cargo; under them a stale unit goes only while no build wrote it since:
/// a fingerprint rewritten after the read tells it
#[cfg(unix)]
#[test]
fn a_unit_that_a_build_wrote_after_the_walk_is_built_again() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("build/app/h1");
  unit(&path, Duration::from_hours(3));
  let mut read = crate::lease_clean::Unit::new("app".to_string());
  read.add(path.clone());
  assert!(!read.built_again());
  output(&path.join("fingerprint/lib-app"), Duration::ZERO);
  assert!(read.built_again());
}

/// The check of the lease comes under the locks of cargo, taken first: the check finds `.cargo-build-lock` held, and a
/// check that fails leaves every unit in place and marks the rule `changed`
#[cfg(unix)]
#[test]
fn a_lease_that_changed_once_the_locks_are_held_leaves_the_profile_dir_as_it_is() {
  let (checkout, git) = git_checkout();
  let root = checkout.path();
  let profile = old_and_new_units(root);
  let checked_under_the_lock = std::cell::Cell::new(None);
  let still_holds = || {
    let lock = std::fs::File::open(profile.join(".cargo-build-lock")).unwrap();
    checked_under_the_lock.set(Some(matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock))));
    false
  };
  let options = crate::lease_clean::CleanOptions {
    cargo_only: true,
    still_holds: Some(&still_holds),
    ..crate::lease_clean::CleanOptions::default()
  };
  let rules = parse_clean_rules("[lease]\nclean = [{ cargo = \"target\", keep = 1, age = \"1h\" }, { delete = \"notes.local\" }]\n").unwrap();
  std::fs::create_dir_all(root.join("notes.local")).unwrap();
  let cleaned = crate::lease_clean::apply_clean_rules_with(&git, root.to_str().unwrap(), &rules, CleanMode::Delete, options);
  assert_eq!(checked_under_the_lock.get(), Some(true));
  assert_eq!(
    cleaned.iter().map(|rule| (rule.rule.as_str(), rule.profiles.changed)).collect::<Vec<_>>(),
    [("cargo target", true)]
  );
  assert_eq!(entry_names(&profile.join("build/app")), ["new", "old"]);
  assert!(root.join("notes.local").exists(), "a cargo-only clean applies no delete rule");
}
