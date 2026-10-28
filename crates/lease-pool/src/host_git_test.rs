use crate::host_git::{FoundGit, GitPlan, HOST_MIN_VERSION, Version, is_homebrew_lock, plan, with_first};
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[test]
fn a_version_parses_releases_candidates_and_the_apple_git() {
  assert_eq!(HOST_MIN_VERSION.to_string(), "2.56.0");
  assert_eq!(Version::parse("2.50.1 (Apple Git-155)"), Some(Version::new(2, 50, 1)));
  assert_eq!(Version::parse("2.45.1.windows.1"), Some(Version::new(2, 45, 1)));
  assert_eq!(Version::parse("2.56").map(|version| version.to_string()).as_deref(), Some("2.56.0"));
  assert_eq!(Version::parse("2.57.0-rc1").map(|version| version.to_string()).as_deref(), Some("2.57.0-rc1"));
  assert_eq!(Version::parse("garbage"), None);
  let floor = HOST_MIN_VERSION;
  for meets in ["2.56.0", "2.56.1", "2.57.0-rc1", "3.0.0", "2.56.0.windows.1"] {
    assert!(Version::parse(meets).unwrap() >= floor, "{meets}");
  }
  for below in ["2.55.9", "2.56.0-rc2", "2.56.0.rc1", "2.50.1 (Apple Git-155)"] {
    assert!(Version::parse(below).unwrap() < floor, "{below}");
  }
}

#[test]
fn the_bin_of_homebrew_comes_before_usr_bin() {
  let bin = Path::new("/opt/homebrew/bin");
  let first = |path: &str| with_first(OsStr::new(path), bin).to_string_lossy().into_owned();
  assert_eq!(first("/usr/bin:/bin"), "/opt/homebrew/bin:/usr/bin:/bin");
  assert_eq!(first("/a:/usr/bin:/opt/homebrew/bin"), "/a:/opt/homebrew/bin:/usr/bin");
  assert_eq!(first("/opt/homebrew/bin:/usr/bin"), "/opt/homebrew/bin:/usr/bin");
  assert_eq!(first("/a"), "/a:/opt/homebrew/bin");
  assert_eq!(first(""), "/opt/homebrew/bin");
}

/// Ready at the floor; below it an install when Homebrew has no git, an upgrade when it has one, a refusal with the hint
/// of the platform without Homebrew, and a refusal when another git comes before the git of Homebrew
#[test]
fn the_plan_installs_upgrades_or_refuses_with_a_hint() {
  let found = |path: &str, version: &str| FoundGit {
    path: PathBuf::from(path),
    version: Version::parse(version),
    text: version.to_string(),
  };
  let brew = Path::new("/opt/homebrew/bin/brew");
  let floor = HOST_MIN_VERSION;
  assert_eq!(plan(Some(&found("/opt/homebrew/bin/git", "2.56.0")), floor, Some(brew), true, true), GitPlan::Ready);
  assert_eq!(
    plan(Some(&found("/usr/bin/git", "2.50.1 (Apple Git-155)")), floor, Some(brew), false, true),
    GitPlan::Install
  );
  assert_eq!(plan(None, floor, Some(brew), false, true), GitPlan::Install);
  assert_eq!(plan(Some(&found("/opt/homebrew/bin/git", "2.55.0")), floor, Some(brew), true, true), GitPlan::Upgrade);
  let refused = |plan: GitPlan| match plan {
    GitPlan::Refuse(message) => message,
    other => panic!("{other:?}"),
  };
  assert!(refused(plan(Some(&found("/usr/bin/git", "2.50.1")), floor, None, false, true)).contains("Install Homebrew"));
  assert!(refused(plan(Some(&found("/usr/bin/git", "2.43.0")), floor, None, false, false)).contains("git-core PPA"));
  assert!(refused(plan(Some(&found("/usr/local/bin/git", "2.40.0")), floor, Some(brew), true, true)).contains("before the git of Homebrew"));
}

#[test]
fn a_lock_of_another_homebrew_process_is_told_by_its_words() {
  assert!(is_homebrew_lock("Error: A `brew upgrade git` process has already locked /opt/homebrew/Cellar/git"));
  assert!(is_homebrew_lock("Error: Another `brew update` process is already in progress"));
  assert!(!is_homebrew_lock("Error: No such keg"));
}
