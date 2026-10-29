use crate::api::{ApiChange, ApiChangeKind, FanIn};
use crate::risk::{FileRisk, file_risks, test_subject};
use object_cache::commit_diff::FileChange;
use pretty_assertions::assert_eq;
use trial_check::diff_scan::CommitDiff;

/// A change of `path` with the `status` of `--raw`; `old_path` for a rename
fn file(status: char, path: &str, old_path: Option<&str>) -> FileChange {
  FileChange {
    status,
    path: path.to_string(),
    old_path: old_path.map(str::to_string),
    additions: 1,
    deletions: 0,
    binary: false,
    old_blob: None,
    new_blob: None,
    hunks: Vec::new(),
  }
}

fn commit(files: Vec<FileChange>) -> CommitDiff {
  CommitDiff { id: "c".to_string(), files }
}

/// The paths of the risks that are untested
fn untested(risks: &[FileRisk]) -> Vec<&str> {
  risks.iter().filter(|risk| risk.untested).map(|risk| risk.path.as_str()).collect()
}

#[test]
fn a_test_names_its_subject_by_its_path() {
  let subjects: Vec<Option<String>> = [
    "src/test/kotlin/a/SessionTest.kt",
    "src/a/SessionTests.java",
    "crates/a/src/session_test.rs",
    "crates/a/tests/session.rs",
    "src/lib/session.test.ts",
    "src/lib/session.spec.tsx",
    "src/__tests__/session.ts",
    "scripts/test_session.py",
    "src/a/Session.kt",
    "src/a/Test.kt",
  ]
  .iter()
  .map(|path| test_subject(path))
  .collect();
  let session = Some("session".to_string());
  assert_eq!(
    subjects,
    [
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session.clone(),
      session,
      None,
      None
    ]
  );
}

/// A source file changed without its test is untested; one changed with its test, a test and a file of no known
/// language are not; a deleted file is not listed, a file deleted and added again is
#[test]
fn a_source_file_changed_without_its_test_is_untested() {
  let diffs = [
    commit(vec![
      file('M', "src/a/Session.kt", None),
      file('M', "src/a/Login.kt", None),
      file('A', "README.md", None),
      file('D', "src/a/Back.kt", None),
    ]),
    commit(vec![
      file('M', "src/test/kotlin/a/LoginTest.kt", None),
      file('D', "src/a/Gone.kt", None),
      file('A', "src/a/Back.kt", None),
    ]),
  ];
  let risks = file_risks(&diffs, &[]);
  assert_eq!(untested(&risks), ["src/a/Back.kt", "src/a/Session.kt"]);
  let paths: Vec<&str> = risks.iter().map(|risk| risk.path.as_str()).collect();
  assert_eq!(
    paths,
    ["src/a/Back.kt", "src/a/Session.kt", "README.md", "src/a/Login.kt", "src/test/kotlin/a/LoginTest.kt"]
  );
}

/// A renamed file is risked at its new path only, also after a second move, and a Rust module file names the directory
/// its test names
#[test]
fn a_renamed_file_is_risked_at_its_new_path() {
  let diffs = [
    commit(vec![
      file('R', "crates/auth/src/session/mod.rs", Some("crates/auth/src/old.rs")),
      file('A', "crates/auth/tests/session.rs", None),
      file('M', "crates/auth/src/token.rs", None),
    ]),
    commit(vec![file('R', "crates/auth/src/token/mod.rs", Some("crates/auth/src/token.rs"))]),
  ];
  let risks = file_risks(&diffs, &[]);
  let paths: Vec<(&str, bool)> = risks.iter().map(|risk| (risk.path.as_str(), risk.untested)).collect();
  assert_eq!(
    paths,
    [
      ("crates/auth/src/token/mod.rs", true),
      ("crates/auth/src/session/mod.rs", false),
      ("crates/auth/tests/session.rs", false)
    ]
  );
}

/// The highest fan-in of a file's removed or changed API raises it above an untested file
#[test]
fn the_highest_fan_in_of_a_file_ranks_it_first() {
  let diffs = [commit(vec![
    file('M', "src/a/Session.kt", None),
    file('M', "src/a/Api.kt", None),
    file('M', "src/test/a/ApiTest.kt", None),
  ])];
  let risks = file_risks(&diffs, &[api_change("src/a/Api.kt", "few", 2), api_change("src/a/Api.kt", "many", 12)]);
  assert_eq!(
    risks,
    [
      FileRisk {
        path: "src/a/Api.kt".to_string(),
        max_fan_in: Some(12),
        untested: false,
        risky: true,
      },
      FileRisk {
        path: "src/a/Session.kt".to_string(),
        max_fan_in: None,
        untested: true,
        risky: true,
      },
      FileRisk {
        path: "src/test/a/ApiTest.kt".to_string(),
        max_fan_in: None,
        untested: false,
        risky: false,
      },
    ]
  );
}

/// A changed API that nothing uses does not make its file risky: an untested file ranks above it
#[test]
fn an_untested_file_ranks_above_an_api_without_usages() {
  let diffs = [commit(vec![
    file('M', "src/a/Api.kt", None),
    file('M', "src/test/a/ApiTest.kt", None),
    file('M', "src/a/Session.kt", None),
  ])];
  let risks = file_risks(&diffs, &[api_change("src/a/Api.kt", "unused", 0)]);
  let ranked: Vec<(&str, Option<u32>, bool)> = risks.iter().map(|risk| (risk.path.as_str(), risk.max_fan_in, risk.risky)).collect();
  assert_eq!(
    ranked,
    [("src/a/Session.kt", None, true), ("src/a/Api.kt", Some(0), false), ("src/test/a/ApiTest.kt", None, false)]
  );
}

/// A changed declaration `name` of `path` with `usages`
fn api_change(path: &str, name: &str, usages: u32) -> ApiChange {
  ApiChange {
    path: path.to_string(),
    line: 1,
    name: name.to_string(),
    key: format!("a.{name}"),
    kind: ApiChangeKind::Changed,
    changes: Vec::new(),
    before: None,
    after: None,
    fan_in: Some(FanIn { usages, complete: true }),
    file: path.to_string(),
  }
}
