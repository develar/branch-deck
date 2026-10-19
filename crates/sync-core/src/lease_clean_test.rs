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

/// In every profile dir (`.cargo-lock`, also two levels down), the cargo rule keeps the `keep` newest units of each
/// group and the young ones, and removes the rest; a dry run measures the same and deletes nothing
#[cfg(unix)]
#[test]
fn the_cargo_rule_keeps_the_newest_and_the_young_units() {
  let checkout = tempfile::tempdir().unwrap();
  let root = checkout.path();
  let git = GitCommandExecutor::new();
  git.execute_command(&["init", "-q"], root.to_str().unwrap()).unwrap();
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
