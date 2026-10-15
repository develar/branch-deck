use crate::clustering::{Cluster, ClusterInput, Clustering, CommitFeatures, FileLines, Placement};
use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use crate::suggest::{SuggestPlan, plan_of};
use git_ops::rewrite_plan::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use sync_core::regroup::RegroupPlan;
use test_utils::git_test_utils::TestRepo;

fn short(id: &str) -> String {
  short_id(id).to_string()
}

/// Commit each `(subject, [(path, content)])` with increasing times
fn commits(repo: &TestRepo, commits: &[(&str, &[(&str, &str)])]) -> Vec<String> {
  commits
    .iter()
    .enumerate()
    .map(|(index, (subject, files))| {
      for (path, content) in *files {
        write(repo, path, content);
      }
      commit_all(repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100)
    })
    .collect()
}

/// The clusters as `name: (change, commit_count, via)`
fn clusters(output: &Value) -> BTreeMap<String, (String, u64, Option<String>)> {
  output["clusters"]
    .as_array()
    .unwrap()
    .iter()
    .map(|cluster| {
      let via = cluster.get("via").and_then(Value::as_str).map(str::to_string);
      let key = cluster["name"].as_str().unwrap().to_string();
      (key, (cluster["change"].as_str().unwrap().to_string(), cluster["commit_count"].as_u64().unwrap(), via))
    })
    .collect()
}

fn entry(change: &str, commit_count: u64, via: Option<&str>) -> (String, u64, Option<String>) {
  (change.to_string(), commit_count, via.map(str::to_string))
}

/// Apply the plan of a suggestion with `bd group`, and suggest again: the new suggestion has an empty plan
fn apply_and_suggest(repo: &TestRepo, output: &Value) -> Value {
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), output["plan"].to_string()).unwrap();
  let (exit_code, grouped) = run_in(repo, &["group", "--plan", plan.path().to_str().unwrap()]);
  assert_eq!(exit_code, 0, "{grouped}");
  let (exit_code, again) = run_in(repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{again}");
  assert_eq!(again["plan"], json!({ "branches": {}, "unassign": [] }));
  again
}

/// Refs and HEAD: a suggestion changes neither
fn state(repo: &TestRepo) -> (String, String) {
  (git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]), repo.head())
}

#[test_log::test]
fn suggests_a_checked_plan_that_is_stable_once_applied() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commits(
    &repo,
    &[
      ("setup vm: start the worker", &[("vm/worker.rs", "worker\n")]),
      ("setup vm: a lease skips a busy worker", &[("vm/lease.rs", "lease\n")]),
      ("ABC-1 parse lists", &[("parser/lists.rs", "lists\n")]),
      ("ABC-2 parse maps\n\nThe lists of ABC-1 hold maps too.", &[("parser/maps.rs", "maps\n")]),
      ("(docs) Tidy the guide", &[("docs/guide.md", "guide\n")]),
    ],
  );
  let before = state(&repo);
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output["plan"],
    json!({ "branches": { "vm": [short(&ids[0]), short(&ids[1])] }, "unassign": [], "issue_groups": { "parser-parse": ["ABC-1", "ABC-2"] } })
  );
  assert_eq!(
    clusters(&output),
    BTreeMap::from([
      ("vm".to_string(), entry("new", 2, None)),
      ("parser-parse".to_string(), entry("merged", 2, Some("issue_group"))),
      ("docs".to_string(), entry("kept", 1, None)),
    ])
  );
  let named = |name: &str| output["clusters"].as_array().unwrap().iter().find(|cluster| cluster["name"] == json!(name)).unwrap();
  let vm = named("vm");
  assert_eq!(
    (&vm["why"], &vm["from"], &vm["areas"], &vm["weak"]),
    (
      &json!("2 units; scope vm in 2 subjects; area vm"),
      &json!([]),
      &json!([{ "dir": "vm", "lines": 2 }]),
      &json!(false)
    )
  );
  let parser = named("parser-parse");
  assert_eq!(parser["why"], json!("2 issue units; ABC-1 named in 1 body; area parser"));
  assert_eq!(parser["issue_keys"], json!(["ABC-1", "ABC-2"]));
  assert_eq!(parser["from"], json!(["ABC-1", "ABC-2"]));
  // the issue group rewrites no subject
  assert_eq!(
    (&output["rewritten"], &output["check"]["ok"], &output["check"]["repair_rounds"]),
    (&json!(2), &json!(true), &json!(0))
  );
  let built: Vec<&Value> = output["check"]["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert!(built.contains(&&json!("parser-parse")), "{built:?}");
  assert_eq!(output.get("warnings"), None);
  assert_eq!(state(&repo), before, "a suggestion writes objects only");

  // --rewrite-issues gives the issue branches a prefix instead
  let (_, rewriting) = run_in(&repo, &["suggest", "--rewrite-issues"]);
  assert_eq!(rewriting["plan"]["branches"]["parser-parse"], json!([short(&ids[2]), short(&ids[3])]));
  assert_eq!((rewriting["plan"].get("issue_groups"), &rewriting["rewritten"]), (None, &json!(4)));

  // applied, the suggestion suggests nothing more
  let again = apply_and_suggest(&repo, &output);
  assert_eq!(again["rewritten"], json!(0));
  assert_eq!(clusters(&again)["vm"], entry("kept", 2, None));
  // the issue group is an anchor now, kept under its name
  assert_eq!(clusters(&again)["parser-parse"], entry("kept", 2, Some("issue_group")));
}

#[test_log::test]
fn a_conflict_in_a_lock_file_joins_the_commits_and_fresh_ignores_prefixes() {
  let repo = repo_with_baseline(&[("README.md", "readme\n"), ("Cargo.lock", "line 1\nline 2\nline 3\n")]);
  // lock files link no commits, so only the check sees that the second needs the first
  let ids = commits(
    &repo,
    &[
      ("XA-1 Add foo", &[("Cargo.lock", "line 1\nfoo\nline 2\nline 3\n"), ("a/foo.rs", "foo\n")]),
      ("YB-1 Add bar", &[("Cargo.lock", "line 1\nfoo\nbar\nline 2\nline 3\n"), ("b/bar.rs", "bar\n")]),
    ],
  );
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["check"]["repair_rounds"], json!(1));
  let cluster = &output["clusters"][0];
  // two issue branches joined by the repair: an issue group builds them together
  assert_eq!(
    (&cluster["commit_count"], &cluster["change"], cluster.get("via")),
    (&json!(2), &json!("merged"), Some(&json!("issue_group")))
  );
  assert_eq!(cluster["why"], json!("1 unit; joined by repair round 1 after a conflict; area a"));
  let name = cluster["name"].as_str().unwrap();
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [], "issue_groups": { name: ["XA-1", "YB-1"] } }));
  assert_eq!((&output["check"]["ok"], &output["rewritten"]), (&json!(true), &json!(0)));
  let (_, rewriting) = run_in(&repo, &["suggest", "--rewrite-issues"]);
  assert_eq!(rewriting["plan"]["branches"][name], json!([short(&ids[0]), short(&ids[1])]));

  // with --fresh a prefix joins nothing: two prefixed commits with one key become its issue branch
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commits(&repo, &[("(one) ABC-1 First", &[("x/a.rs", "a\n")]), ("(two) ABC-1 Second", &[("y/b.rs", "b\n")])]);
  let (_, kept) = run_in(&repo, &["suggest"]);
  assert_eq!(kept["plan"], json!({ "branches": {}, "unassign": [] }));
  let (exit_code, fresh) = run_in(&repo, &["suggest", "--fresh"]);
  assert_eq!(exit_code, 0, "{fresh}");
  assert_eq!(fresh["plan"], json!({ "branches": {}, "unassign": [short(&ids[0]), short(&ids[1])] }));
  assert_eq!(clusters(&fresh), BTreeMap::from([("ABC-1".to_string(), entry("merged", 2, None))]));
}

#[test_log::test]
fn whole_issue_branches_become_an_issue_group() {
  let repo = repo_with_baseline(&[("README.md", "readme\n"), ("Cargo.lock", "line 1\nline 2\nline 3\n")]);
  // the mention makes one cluster of two issue branches; apart, the second would conflict in the lock file
  let ids = commits(
    &repo,
    &[
      ("XA-1 Add foo", &[("Cargo.lock", "line 1\nfoo\nline 2\nline 3\n"), ("a/foo.rs", "foo\n")]),
      (
        "XA-2 Add bar\n\nAfter XA-1.",
        &[("Cargo.lock", "line 1\nfoo\nbar\nline 2\nline 3\n"), ("b/bar.rs", "bar\n")],
      ),
    ],
  );
  let before = git(&repo, &["log", "--format=%H %s"]);
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  // the issue group builds both in one branch, so nothing conflicts
  assert_eq!(
    (&output["check"]["ok"], &output["check"]["repair_rounds"], &output["rewritten"]),
    (&json!(true), &json!(0), &json!(0))
  );
  let cluster = &output["clusters"][0];
  assert_eq!((&cluster["commit_count"], cluster.get("via")), (&json!(2), Some(&json!("issue_group"))));
  let name = cluster["name"].as_str().unwrap();
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [], "issue_groups": { name: ["XA-1", "XA-2"] } }));

  // applied: no commit changes, the store maps both keys, and the suggestion suggests nothing more
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), output["plan"].to_string()).unwrap();
  let (exit_code, grouped) = run_in(
    &repo,
    &["group", "--expect-head", output["head"].as_str().unwrap(), "--plan", plan.path().to_str().unwrap()],
  );
  assert_eq!(exit_code, 0, "{grouped}");
  assert_eq!((&grouped["rewritten"], &grouped["issue_groups"]), (&json!(0), &json!({ name: ["XA-1", "XA-2"] })));
  assert!(grouped["undo_token"].as_str().unwrap().starts_with("groups:"), "{grouped}");
  assert_eq!(git(&repo, &["log", "--format=%H %s"]), before);
  let (_, again) = run_in(&repo, &["suggest"]);
  assert_eq!(again["plan"], json!({ "branches": {}, "unassign": [] }));
  assert_eq!(clusters(&again)[name], entry("kept", 2, Some("issue_group")));
  assert_eq!(ids.len(), 2);

  // --fresh ignores the issue group as it ignores a prefix, and finds the same cluster: nothing changes
  let (_, fresh) = run_in(&repo, &["suggest", "--fresh"]);
  assert_eq!(fresh["plan"], json!({ "branches": {}, "unassign": [] }));
}

#[test_log::test]
fn fresh_keeps_the_name_of_a_branch_that_stays_whole() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commits(&repo, &[("(feat) Add x", &[("x/a.rs", "a\n")]), ("(feat) Change x", &[("x/a.rs", "b\n")])]);
  let (exit_code, output) = run_in(&repo, &["suggest", "--fresh"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [] }));
  assert_eq!(clusters(&output), BTreeMap::from([("feat".to_string(), entry("kept", 2, None))]));
}

#[test_log::test]
fn stacked_branches_and_an_empty_range() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let (exit_code, empty) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{empty}");
  assert_eq!(
    (&empty["plan"], &empty["clusters"], &empty["check"]["ok"]),
    (&json!({ "branches": {}, "unassign": [] }), &json!([]), &json!(true))
  );

  repo.set_config("branchdeck.stacks", "true").unwrap();
  commits(&repo, &[("(a) Add x", &[("x/a.rs", "a\n")]), ("(a/b) Add y", &[("y/b.rs", "b\n")])]);
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [] }));
  assert_eq!(
    clusters(&output),
    BTreeMap::from([("a".to_string(), entry("kept", 1, None)), ("a/b".to_string(), entry("kept", 1, None))])
  );
  let b = output["check"]["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!("b")).unwrap();
  assert_eq!((&b["parent"], &b["result"]), (&json!("a"), &json!("ok")));
}

#[test_log::test]
fn prefixes_that_depend_on_each_other_stay_apart_and_exit_with_1() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commits(&repo, &[("(a) Add shared", &[("shared.txt", "one\n")]), ("(b) Change shared", &[("shared.txt", "two\n")])]);
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [] }));
  assert_eq!(output["check"]["ok"], json!(false));
  let warnings: Vec<(&str, &Value)> = output["warnings"]
    .as_array()
    .unwrap()
    .iter()
    .map(|warning| (warning["kind"].as_str().unwrap(), &warning["branches"]))
    .collect();
  assert_eq!(warnings, [("anchor_dependency", &json!(["b", "a"])), ("unresolved_conflict", &json!(["b"]))]);
  // without stacked branches the warning names the setting that stacks them
  let message = output["warnings"][0]["message"].as_str().unwrap();
  assert!(message.contains("git config branchdeck.stacks true"), "{message}");
}

#[test_log::test]
fn with_stacked_branches_a_branch_stacks_on_the_branch_it_needs() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let ids = commits(
    &repo,
    &[
      ("(a) Add shared", &[("shared.txt", "one\n")]),
      ("(b) Change shared", &[("shared.txt", "two\n")]),
      ("(gone/c) Add other", &[("other.txt", "other\n")]),
    ],
  );
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["plan"], json!({ "branches": { "a/b": [short(&ids[1])], "c": [short(&ids[2])] }, "unassign": [] }));
  let named = |name: &str| {
    output["clusters"]
      .as_array()
      .unwrap()
      .iter()
      .find(|cluster| cluster["name"] == json!(name))
      .unwrap()
      .clone()
  };
  let b = named("a/b");
  assert_eq!((&b["change"], &b["parent"], &b["restacked"]), (&json!("kept"), &json!("a"), &json!(true)));
  assert_eq!(b["why"], json!("prefix (b); 1 unit; area .; stacked on a, whose commits it needs"));
  assert_eq!(named("c")["why"], json!("prefix (c); 1 unit; area .; unstacked, because its parent has no commits"));
  assert_eq!((&output["check"]["ok"], output.get("warnings")), (&json!(true), None));

  // applied, the suggestion suggests nothing more
  let again = apply_and_suggest(&repo, &output);
  assert_eq!(
    clusters(&again),
    BTreeMap::from([
      ("a".to_string(), entry("kept", 1, None)),
      ("a/b".to_string(), entry("kept", 1, None)),
      ("c".to_string(), entry("kept", 1, None)),
    ])
  );
  assert!(again["clusters"].as_array().unwrap().iter().all(|cluster| cluster.get("restacked").is_none()), "{again}");
}

/// A repository with stacked branches on and these commits
fn stacked_repo(subjects: &[(&str, &[(&str, &str)])]) -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let ids = commits(&repo, subjects);
  (repo, ids)
}

/// Suggest, expect a checked plan, apply it and expect that the next suggestion changes nothing
fn suggest_checked_and_stable(repo: &TestRepo, plan: &Value) {
  let (exit_code, output) = run_in(repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["plan"], &output["check"]["ok"]), (plan, &json!(true)), "{output}");
  apply_and_suggest(repo, &output);
}

#[test_log::test]
fn a_stacked_plan_ungroups_an_issue_group_and_rewrites_a_deeper_path() {
  // an issue group that needs another branch leaves the group for a stacked prefix
  let (repo, ids) = stacked_repo(&[("(a) Add shared", &[("shared.txt", "one\n")]), ("XA-1 Change shared", &[("shared.txt", "two\n")])]);
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), json!({ "branches": {}, "unassign": [], "issue_groups": { "grp": ["XA-1"] } }).to_string()).unwrap();
  let (exit_code, grouped) = run_in(&repo, &["group", "--plan", plan.path().to_str().unwrap()]);
  assert_eq!(exit_code, 0, "{grouped}");
  suggest_checked_and_stable(&repo, &json!({ "branches": { "a/grp": [short(&ids[1])] }, "unassign": [], "ungroup_issues": ["XA-1"] }));

  // a path of three levels declares the parent of its parent too, so it becomes `parent/leaf`
  let (repo, ids) = stacked_repo(&[
    ("(a) Add a", &[("a.txt", "a\n")]),
    ("(x) Add shared", &[("shared.txt", "one\n")]),
    ("(a/b) Change shared", &[("shared.txt", "two\n")]),
    ("(a/b/c) Add c", &[("c.txt", "c\n")]),
  ]);
  suggest_checked_and_stable(&repo, &json!({ "branches": { "x/b": [short(&ids[2])], "b/c": [short(&ids[3])] }, "unassign": [] }));
  let (repo, ids) = stacked_repo(&[("(gone/a) Add a", &[("a.txt", "a\n")]), ("(gone/a/b) Add b", &[("b.txt", "b\n")])]);
  suggest_checked_and_stable(&repo, &json!({ "branches": { "a": [short(&ids[0])], "a/b": [short(&ids[1])] }, "unassign": [] }));
}

#[test_log::test]
fn a_follow_up_that_joins_an_issue_branch_stays_there_once_applied() {
  let (repo, ids) = stacked_repo(&[
    ("K-1 Add parser", &[("shared.txt", "one\n"), ("parser/a.rs", "a\n")]),
    ("Tune shared", &[("shared.txt", "two\n"), ("ui/b.rs", "b\n")]),
  ]);
  suggest_checked_and_stable(&repo, &json!({ "branches": { "K-1": [short(&ids[1])] }, "unassign": [] }));
  let (_, again) = run_in(&repo, &["suggest"]);
  assert_eq!(clusters(&again), BTreeMap::from([("K-1".to_string(), entry("kept", 2, None))]));
}

#[test_log::test]
fn the_dump_hides_every_name() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commits(
    &repo,
    &[
      (
        "(secret-team) PROJ-12 Add the widget\n\nSee PROJ-7.",
        &[("product/widget/src/com/acme/Widget.kt", "widget\n")],
      ),
      ("setup vm: tidy the lease", &[("tools/vm/lease.rs", "lease\n")]),
    ],
  );
  let (exit_code, output) = run_in(&repo, &["suggest", "--dump-input"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["schema"], json!(1));
  let input: ClusterInput = serde_json::from_value(output["input"].clone()).unwrap();
  assert_eq!(input.commits.len(), 2);
  let text = output.to_string();
  for name in ["secret", "team", "PROJ", "widget", "Widget", "acme", "product", "src", "tools", "vm", "lease", "tidy"] {
    assert!(!text.contains(name), "{name} in {text}");
  }
}

#[test]
fn the_plan_moves_only_what_changes_and_is_a_regroup_plan() {
  let commit = |prefix: Option<&str>, key: Option<&str>, fixup_of: Option<usize>| CommitFeatures {
    prefix: prefix.map(str::to_string),
    group: prefix.or(key).map(str::to_string),
    key: key.map(str::to_string),
    fixup_of,
    files: vec![FileLines {
      path: "a.txt".to_string(),
      lines: 1,
    }],
    ..CommitFeatures::default()
  };
  let input = ClusterInput {
    commits: vec![
      // in place already
      commit(Some("feat"), None, None),
      // moves, and its fixup follows it
      commit(None, None, None),
      commit(None, None, Some(1)),
      // a prefixed commit that goes to its own issue branch loses its prefix
      commit(Some("old"), Some("ABC-1"), None),
      commit(None, Some("ABC-1"), None),
      // whole issue branches become an issue group
      commit(None, Some("XYZ-1"), None),
      commit(None, Some("XYZ-2"), None),
    ],
    ..ClusterInput::default()
  };
  let cluster = |commits: Vec<usize>, keys: &[&str], units: usize, issue_units: usize| Cluster {
    commits,
    anchor: None,
    keys: keys.iter().map(|key| (*key).to_string()).collect(),
    units,
    issue_units,
    links: Vec::new(),
    weak: false,
    repair_rounds: Vec::new(),
  };
  let clustering = Clustering {
    clusters: vec![
      cluster(vec![0, 1, 2], &[], 2, 0),
      cluster(vec![3, 4], &["ABC-1"], 2, 1),
      cluster(vec![5, 6], &["XYZ-1", "XYZ-2"], 2, 2),
    ],
    warnings: Vec::new(),
    unit_of: Vec::new(),
    unit_anchors: Vec::new(),
    dependencies: Vec::new(),
  };
  let mut placements = vec![Placement::root("feat"), Placement::root("ABC-1"), Placement::root("xyz")];
  let plan = plan_of(&input, &clustering, &placements, false);
  assert_eq!(
    (plan.branches, plan.unassign, plan.issue_groups),
    (
      BTreeMap::from([("feat".to_string(), vec![1])]),
      vec![3],
      BTreeMap::from([("xyz".to_string(), vec!["XYZ-1".to_string(), "XYZ-2".to_string()])])
    )
  );
  let plan = plan_of(&input, &clustering, &placements, true);
  assert_eq!((plan.branches["xyz"].as_slice(), plan.issue_groups.len()), ([5, 6].as_slice(), 0));
  // a stacked branch is named by its path, which an issue group can't hold
  placements[2].path = "feat/xyz".to_string();
  let plan = plan_of(&input, &clustering, &placements, false);
  assert_eq!((plan.branches["feat/xyz"].as_slice(), plan.issue_groups.len()), ([5, 6].as_slice(), 0));

  // the printed plan is what `bd group` and `bd check` read
  let plan = SuggestPlan {
    branches: BTreeMap::from([("feat".to_string(), vec!["0123abcd".to_string()])]),
    unassign: vec!["4567".to_string()],
    issue_groups: BTreeMap::from([("xyz".to_string(), vec!["XYZ-1".to_string()])]),
    ungroup_issues: vec!["ABC-1".to_string()],
  };
  let parsed: RegroupPlan = serde_json::from_value(serde_json::to_value(&plan).unwrap()).unwrap();
  assert_eq!(
    (parsed.branches, parsed.unassign, parsed.issue_groups, parsed.ungroup_issues),
    (plan.branches, plan.unassign, plan.issue_groups, plan.ungroup_issues)
  );
}
