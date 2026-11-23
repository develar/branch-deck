use crate::check_record::{CHECKS_FILE, CheckRecord, CheckState, check_tree, parse_check_records};
use crate::lease_clean::{PickCheck, parse_clean_rules, parse_pick_check};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

/// Only a green line of schema 1 counts: another schema, a red run, a line that does not parse or has no tree is left
/// out, and an unknown field is ignored
#[test]
fn only_green_records_of_schema_one_count() {
  let text = [
    r#"{"schema":1,"tree":"t1","head":"h1","tool":"bt","command":"build","patterns":["//plugins/air/..."],"exit":0,"at":5}"#,
    r#"{"schema":2,"tree":"t2","exit":0}"#,
    r#"{"schema":1,"tree":"t3","exit":3}"#,
    r#"{"schema":1,"tree":"t4","exit":0"#,
    r#"{"schema":1,"exit":0}"#,
    "",
    r#"{"schema":1,"tree":"t5","head":"h5","tool":"bt","command":"test","patterns":["--lane","fast"],"host":"studio","exit":0,"at":9,"new":true}"#,
  ]
  .join("\n");
  let records = parse_check_records(&text);
  assert_eq!(records.iter().map(|record| record.tree.as_str()).collect::<Vec<_>>(), ["t1", "t5"]);
  assert_eq!(records[0].describe(), "bt build //plugins/air/...");
  assert_eq!(records[1].describe(), "bt test --lane fast --host studio");
  assert_eq!(
    records[1],
    CheckRecord {
      tree: "t5".to_string(),
      head: "h5".to_string(),
      tool: "bt".to_string(),
      command: "test".to_string(),
      patterns: vec!["--lane".to_string(), "fast".to_string()],
      host: Some("studio".to_string()),
      at: 9,
    }
  );
}

/// `pick_requires_check` of `[lease]` is false without the key, and a value that is no boolean is an error of the file
#[test]
fn pick_requires_check_is_a_boolean_of_the_lease_table() {
  assert!(!parse_pick_check("").unwrap().required);
  assert!(!parse_pick_check("[lease]\nclean = []\n").unwrap().required);
  let text = "[lease]\npick_requires_check = true\nclean = [{ delete = \"x\" }]\n";
  assert!(parse_pick_check(text).unwrap().required);
  assert_eq!(parse_clean_rules(text).unwrap().len(), 1, "the clean rules read the file with the key");
  let error = format!("{:#}", parse_pick_check("[lease]\npick_requires_check = \"yes\"\n").unwrap_err());
  assert!(error.contains("not a boolean"), "{error}");
  let error = format!("{:#}", parse_pick_check("[lease]\npick_requires_checks = true\n").unwrap_err());
  assert!(error.contains("unknown key lease.pick_requires_checks"), "{error}");
}

/// `check_carries_over` of `[lease]` is true without the key, false turns the carry off, and a value that is no boolean
/// is an error of the file
#[test]
fn check_carries_over_is_a_boolean_true_by_default() {
  assert_eq!(
    parse_pick_check("[lease]\npick_requires_check = true\n").unwrap(),
    PickCheck {
      required: true,
      carries_over: true
    }
  );
  assert_eq!(
    parse_pick_check("[lease]\npick_requires_check = true\ncheck_carries_over = false\n").unwrap(),
    PickCheck {
      required: true,
      carries_over: false
    }
  );
  let error = format!("{:#}", parse_pick_check("[lease]\ncheck_carries_over = 0\n").unwrap_err());
  assert!(error.contains("`lease.check_carries_over` is not a boolean"), "{error}");
}

/// A repository whose `master` and `peer` forked from `base`, and a lane commit on `base`: `master` changed another
/// file, `peer` the line of `w.txt` the lane changed; the lane changed a binary file too
struct Carry {
  repo: TestRepo,
  base: String,
  lane: String,
  /// The lane commit replayed onto `master` by a clean update
  updated: String,
}

impl Carry {
  fn new() -> Self {
    let repo = TestRepo::new();
    let base = repo.create_commit_with_files("Base", &[("w.txt", "alpha\nbeta\n"), ("bin.dat", "\0\u{1}\u{2}")]);
    repo.git(&["branch", "peer"]);
    repo.create_commit("Master", "m.txt", "m\n");
    repo.git(&["checkout", "-q", "peer"]);
    repo.create_commit("Peer", "w.txt", "alpha\nBETA\n");
    repo.git(&["checkout", "-q", "--detach", &base]);
    let lane = repo.create_commit_with_files("Lane", &[("w.txt", "alpha\nBeta\n"), ("bin.dat", "\0\u{1}\u{3}"), ("l.txt", "l\n")]);
    repo.git(&["checkout", "-q", "--detach", "master"]);
    repo.git(&["cherry-pick", &lane]);
    let updated = repo.head();
    Self { repo, base, lane, updated }
  }

  /// A commit on `on` (detached) that writes `files`
  fn commit_on(&self, on: &str, files: &[(&str, &str)]) -> String {
    self.repo.git(&["checkout", "-q", "--detach", on]);
    self.repo.create_commit_with_files("Edit", files)
  }

  /// The state of the check of HEAD at `head` against one green record of `tree` on `checked_head`, carried onto
  /// `target`, and the commits it names untested
  fn check(&self, head: &str, checked_head: &str, tree: &str, target: Option<&str>) -> (CheckState, Option<u64>) {
    let repo = &self.repo;
    repo.git(&["checkout", "-q", "--detach", head]);
    let line = format!("{{\"schema\":1,\"tree\":\"{tree}\",\"head\":\"{checked_head}\",\"tool\":\"bt\",\"command\":\"build\",\"exit\":0}}\n");
    std::fs::write(repo.path().join(".git").join(CHECKS_FILE), line).unwrap();
    let check = check_tree(repo.git_executor(), repo.path_str(), || Ok(target.map(str::to_string))).unwrap();
    (check.state, check.untested_commits)
  }

  fn tree(&self, commit: &str) -> String {
    self.repo.git(&["rev-parse", &format!("{commit}^{{tree}}")])
  }
}

/// A record of an older tree carries over a clean update onto the target: the change of its tree since the base it was
/// built on, merged onto the base of HEAD, is `HEAD^{tree}` exactly (a binary file included), and the answer counts the
/// commits of the target in between. The record's tree may hold the changes that were uncommitted at its HEAD. An
/// update that needed a resolution, a binary byte or a whitespace changed after it, an edit on the same base, a record
/// without its HEAD, and a check without a target stay stale
#[test]
fn a_record_carries_over_a_clean_update_onto_the_target() {
  let carry = Carry::new();
  let lane_tree = carry.tree(&carry.lane);
  let resolved = carry.commit_on("peer", &[("w.txt", "alpha\nBEta\n"), ("bin.dat", "\0\u{1}\u{3}"), ("l.txt", "l\n")]);
  let binary = carry.commit_on(&carry.updated, &[("bin.dat", "\0\u{1}\u{4}")]);
  let whitespace = carry.commit_on(&carry.updated, &[("w.txt", "alpha \nBeta\n")]);
  let edited = carry.commit_on(&carry.lane, &[("l.txt", "l2\n")]);
  let master = Some("refs/heads/master");
  let cases = [
    (
      "a clean update",
      carry.check(&carry.updated, &carry.lane, &lane_tree, master),
      (CheckState::CarriedOver, Some(1)),
    ),
    (
      "changes uncommitted at the record's HEAD",
      carry.check(&carry.updated, &carry.base, &lane_tree, master),
      (CheckState::CarriedOver, Some(1)),
    ),
    (
      "an update that needed a resolution",
      carry.check(&resolved, &carry.lane, &lane_tree, Some("refs/heads/peer")),
      (CheckState::Stale, None),
    ),
    (
      "a binary byte changed after the update",
      carry.check(&binary, &carry.lane, &lane_tree, master),
      (CheckState::Stale, None),
    ),
    (
      "a whitespace changed after the update",
      carry.check(&whitespace, &carry.lane, &lane_tree, master),
      (CheckState::Stale, None),
    ),
    ("an edit on the same base", carry.check(&edited, &carry.lane, &lane_tree, master), (CheckState::Stale, None)),
    ("a record without its HEAD", carry.check(&carry.updated, "", &lane_tree, master), (CheckState::Stale, None)),
    (
      "the carry turned off",
      carry.check(&carry.updated, &carry.lane, &lane_tree, None),
      (CheckState::Stale, None),
    ),
  ];
  for (case, got, expected) in cases {
    assert_eq!(got, expected, "{case}");
  }
}
