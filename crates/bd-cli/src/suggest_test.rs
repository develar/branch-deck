use crate::clustering::{Cluster, ClusterInput, Clustering, CommitFeatures, FileLines, Placement, Restack};
use crate::fixture_test::{
  ACP_HOST, commit_all, commit_changed_declaration, commit_dangling_reference, commit_override_of_overload, git, numbered_lines, repo_with_baseline, run_in, write,
};
use crate::suggest::{SuggestPlan, kept_and_moved, name_words, plan_of};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use sync_core::regroup::RegroupPlan;
use test_utils::git_test_utils::TestRepo;

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

/// `bd suggest`, whose check needed one repair round
fn suggest_after_one_repair(repo: &TestRepo) -> Value {
  let (exit_code, output) = run_in(repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["check"]["repair_rounds"], json!(1), "{output}");
  output
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
    json!({ "branches": { "vm": [short_id(&ids[0]), short_id(&ids[1])] }, "unassign": [], "issue_groups": { "parser-parse": ["ABC-1", "ABC-2"] } })
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
  // the commits of each cluster that stay and those that change branch, with the branch they leave
  let kept_and_moved = |output: &Value| {
    output["clusters"]
      .as_array()
      .unwrap()
      .iter()
      .map(|cluster| (cluster["name"].as_str().unwrap().to_string(), (cluster["kept"].clone(), cluster["moved"].clone())))
      .collect::<BTreeMap<_, _>>()
  };
  let moved = |id: &str, leaves: Option<&str>| json!({ "commit": short_id(id), "leaves": leaves });
  assert_eq!(
    kept_and_moved(&output),
    BTreeMap::from([
      ("vm".to_string(), (json!([]), json!([moved(&ids[0], None), moved(&ids[1], None)]))),
      (
        "parser-parse".to_string(),
        (json!([]), json!([moved(&ids[2], Some("ABC-1")), moved(&ids[3], Some("ABC-2"))]))
      ),
      ("docs".to_string(), (json!([short_id(&ids[4])]), json!([]))),
    ])
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
  assert_eq!(rewriting["plan"]["branches"]["parser-parse"], json!([short_id(&ids[2]), short_id(&ids[3])]));
  assert_eq!((rewriting["plan"].get("issue_groups"), &rewriting["rewritten"]), (None, &json!(4)));

  // applied, the suggestion suggests nothing more
  let again = apply_and_suggest(&repo, &output);
  assert_eq!(again["rewritten"], json!(0));
  assert_eq!(clusters(&again)["vm"], entry("kept", 2, None));
  // the issue group is an anchor now, kept under its name
  assert_eq!(clusters(&again)["parser-parse"], entry("kept", 2, Some("issue_group")));
  // and every commit stays in its branch
  let counts: Vec<(usize, Value)> = kept_and_moved(&again).into_values().map(|(kept, moved)| (kept.as_array().unwrap().len(), moved)).collect();
  assert_eq!(counts, [(1, json!([])), (2, json!([])), (2, json!([]))]);
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
  let output = suggest_after_one_repair(&repo);
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
  assert_eq!(rewriting["plan"]["branches"][name], json!([short_id(&ids[0]), short_id(&ids[1])]));

  // with --fresh a prefix joins nothing: two prefixed commits with one key become its issue branch
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commits(&repo, &[("(one) ABC-1 First", &[("x/a.rs", "a\n")]), ("(two) ABC-1 Second", &[("y/b.rs", "b\n")])]);
  let (_, kept) = run_in(&repo, &["suggest"]);
  assert_eq!(kept["plan"], json!({ "branches": {}, "unassign": [] }));
  let (exit_code, fresh) = run_in(&repo, &["suggest", "--fresh"]);
  assert_eq!(exit_code, 0, "{fresh}");
  assert_eq!(fresh["plan"], json!({ "branches": {}, "unassign": [short_id(&ids[0]), short_id(&ids[1])] }));
  assert_eq!(clusters(&fresh), BTreeMap::from([("ABC-1".to_string(), entry("merged", 2, None))]));
}

/// A conflict needs only what the lines of the conflicting commit name: the third commit changes the lock file line the
/// first added, so the repair joins the two; the second changes other lines of the same file and stays apart (by the
/// files alone the repair would have joined all three, as it took 42 commits for one conflict on the IntelliJ checkout)
#[test_log::test]
fn a_repair_joins_what_the_lines_of_the_conflict_need_not_every_commit_of_the_file() {
  let lock = numbered_lines(20);
  let repo = repo_with_baseline(&[("README.md", "readme\n"), ("Cargo.lock", &lock)]);
  let with = |changes: &[(&str, &str)]| changes.iter().fold(lock.clone(), |text, (from, to)| text.replace(from, to));
  commits(
    &repo,
    &[
      ("XA-1 Add foo", &[("Cargo.lock", &with(&[("line 2\n", "two\n")])), ("a/foo.rs", "foo\n")]),
      (
        "YB-1 Add bar",
        &[("Cargo.lock", &with(&[("line 2\n", "two\n"), ("line 18\n", "eighteen\n")])), ("b/bar.rs", "bar\n")],
      ),
      (
        "ZC-1 Change foo",
        &[("Cargo.lock", &with(&[("line 2\n", "TWO\n"), ("line 18\n", "eighteen\n")])), ("c/baz.rs", "baz\n")],
      ),
    ],
  );
  let output = suggest_after_one_repair(&repo);
  let mut keys: Vec<&Value> = output["clusters"].as_array().unwrap().iter().map(|cluster| &cluster["issue_keys"]).collect();
  keys.sort_by_key(ToString::to_string);
  assert_eq!(keys, [&json!(["XA-1", "ZC-1"]), &json!(["YB-1"])], "{output}");
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
  // `(b)` stays: the stack parents stack it on a
  assert_eq!(output["plan"], json!({ "branches": { "a/b": [], "c": [short_id(&ids[2])] }, "unassign": [] }));
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
fn a_stacked_plan_stacks_an_issue_group_and_rewrites_a_deeper_path() {
  // an issue group that needs another branch is stacked on it
  let (repo, ids) = stacked_repo(&[("(a) Add shared", &[("shared.txt", "one\n")]), ("XA-1 Change shared", &[("shared.txt", "two\n")])]);
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), json!({ "branches": {}, "unassign": [], "issue_groups": { "grp": ["XA-1"] } }).to_string()).unwrap();
  let (exit_code, grouped) = run_in(&repo, &["group", "--plan", plan.path().to_str().unwrap()]);
  assert_eq!(exit_code, 0, "{grouped}");
  // the issue group stays: the stack parents stack it on what it needs, and the trial rewrites nothing
  let (_, output) = run_in(&repo, &["suggest"]);
  assert_eq!(output["rewritten"], json!(0));
  suggest_checked_and_stable(&repo, &json!({ "branches": { "a/grp": [] }, "unassign": [] }));
  assert_eq!(repo.head(), ids[1]);

  // a path of three levels declares the parent of its parent too, so it becomes `parent/leaf`
  let (repo, ids) = stacked_repo(&[
    ("(a) Add a", &[("a.txt", "a\n")]),
    ("(x) Add shared", &[("shared.txt", "one\n")]),
    ("(a/b) Change shared", &[("shared.txt", "two\n")]),
    ("(a/b/c) Add c", &[("c.txt", "c\n")]),
  ]);
  suggest_checked_and_stable(&repo, &json!({ "branches": { "x/b": [short_id(&ids[2])], "b/c": [short_id(&ids[3])] }, "unassign": [] }));
  let (repo, ids) = stacked_repo(&[("(gone/a) Add a", &[("a.txt", "a\n")]), ("(gone/a/b) Add b", &[("b.txt", "b\n")])]);
  suggest_checked_and_stable(&repo, &json!({ "branches": { "a": [short_id(&ids[0])], "a/b": [short_id(&ids[1])] }, "unassign": [] }));
}

#[test_log::test]
fn a_stored_parent_without_need_is_reported_and_kept() {
  let (repo, _) = stacked_repo(&[("(a) Add a", &[("a.txt", "a\n")]), ("(b) Add b", &[("b.txt", "b\n")])]);
  let (exit_code, restacked) = run_in(&repo, &["restack", "b", "a"]);
  assert_eq!(exit_code, 0, "{restacked}");
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  // the plan changes nothing: a stack parent may be deliberate
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [] }));
  let b = output["clusters"].as_array().unwrap().iter().find(|cluster| cluster["name"] == json!("a/b")).unwrap();
  assert_eq!(
    (&b["parent"], &b["stacked_without_need"], b.get("restacked")),
    (&json!("a"), &json!(true), None),
    "{output}"
  );
  let a = output["clusters"].as_array().unwrap().iter().find(|cluster| cluster["name"] == json!("a")).unwrap();
  assert_eq!(a.get("stacked_without_need"), None);

  // once b needs a, it is stacked with need
  write(&repo, "a.txt", "a for b\n");
  commit_all(&repo, "(b) Use a", 1_700_000_900);
  let (_, output) = run_in(&repo, &["suggest"]);
  let b = output["clusters"].as_array().unwrap().iter().find(|cluster| cluster["name"] == json!("a/b")).unwrap();
  assert_eq!(b.get("stacked_without_need"), None, "{output}");
}

#[test_log::test]
fn a_follow_up_that_joins_an_issue_branch_stays_there_once_applied() {
  let (repo, ids) = stacked_repo(&[
    ("K-1 Add parser", &[("shared.txt", "one\n"), ("parser/a.rs", "a\n")]),
    ("Tune shared", &[("shared.txt", "two\n"), ("ui/b.rs", "b\n")]),
  ]);
  suggest_checked_and_stable(&repo, &json!({ "branches": { "K-1": [short_id(&ids[1])] }, "unassign": [] }));
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
  // per cluster the commits that stay and those that change branch, with the branch they leave: the fixup that the
  // plan leaves out moves with its target, and so do the issue branches the issue group takes in
  let kept_and_moved_of = |placements: &[Placement]| {
    clustering
      .clusters
      .iter()
      .zip(placements)
      .map(|(cluster, placement)| kept_and_moved(&input, cluster, &placement.path))
      .collect::<Vec<_>>()
  };
  assert_eq!(
    kept_and_moved_of(&placements),
    [
      (vec![0], vec![(1, None), (2, None)]),
      (vec![4], vec![(3, Some("old"))]),
      (Vec::new(), vec![(5, Some("XYZ-1")), (6, Some("XYZ-2"))]),
    ]
  );
  let plan = plan_of(&input, &clustering, &placements, true);
  assert_eq!((plan.branches["xyz"].as_slice(), plan.issue_groups.len()), ([5, 6].as_slice(), 0));
  // a stacked issue group is named by its leaf, and its path writes the stack parent
  placements[2] = Placement {
    path: "feat/xyz".to_string(),
    parent: Some("feat".to_string()),
    restack: Some(Restack::Needs),
  };
  let plan = plan_of(&input, &clustering, &placements, false);
  assert_eq!(plan.branches["feat/xyz"], Vec::<usize>::new());
  assert_eq!(plan.issue_groups, BTreeMap::from([("xyz".to_string(), vec!["XYZ-1".to_string(), "XYZ-2".to_string()])]));
  assert_eq!(kept_and_moved_of(&placements)[2], (Vec::new(), vec![(5, Some("XYZ-1")), (6, Some("XYZ-2"))]));

  // the printed plan is what `bd group` and `bd check` read
  let plan = SuggestPlan {
    branches: BTreeMap::from([("feat".to_string(), vec!["0123abcd".to_string()])]),
    unassign: vec!["4567".to_string()],
    issue_groups: BTreeMap::from([("xyz".to_string(), vec!["XYZ-1".to_string()])]),
    ungroup_issues: vec!["ABC-1".to_string()],
    unstack: vec!["old".to_string()],
  };
  let parsed: RegroupPlan = serde_json::from_value(serde_json::to_value(&plan).unwrap()).unwrap();
  assert_eq!(
    (parsed.branches, parsed.unassign, parsed.issue_groups, parsed.ungroup_issues, parsed.unstack),
    (plan.branches, plan.unassign, plan.issue_groups, plan.ungroup_issues, plan.unstack)
  );
}

#[test]
fn an_issue_key_is_cut_where_it_is_not_inside_another_word() {
  assert_eq!(name_words("XABC-1x ABC-1 cache"), name_words("XABC-1x cache"));
  assert_eq!(name_words("ABC-1 cache ABC-2"), name_words("cache"));
}

/// The commit that stops using a function another branch removes: with stacked branches the remover stacks on it
#[test_log::test]
fn a_dangling_reference_stacks_the_branch_on_its_fix() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  write(&repo, "src/util/Util.kt", "package util\n\nfun controlUrl(): String = \"\"\n");
  write(&repo, "src/flow/Start.kt", "package flow\n\nimport util.controlUrl\n\nfun start() = controlUrl()\n");
  let base = commit_all(&repo, "Add util", 1_700_000_050);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &base]);
  commits(
    &repo,
    &[
      ("(cleanup) Drop controlUrl", &[("src/util/Util.kt", "package util\n")]),
      ("(flow) Start without it", &[("src/flow/Start.kt", "package flow\n\nfun start() = 1\n")]),
    ],
  );
  let (_, output) = run_in(&repo, &["check"]);
  assert_eq!(output["ok"], json!(false), "{output}");

  suggest_checked_and_stable(&repo, &json!({ "branches": { "flow/cleanup": [] }, "unassign": [] }));
}

/// The Safe Push of IJAI-4185: the branch that makes a function suspend stacks on the one that deleted its call
#[test_log::test]
fn a_changed_declaration_stacks_the_branch_on_its_fix() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  commit_changed_declaration(&repo);
  let (_, output) = run_in(&repo, &["check"]);
  assert_eq!(output["ok"], json!(false), "{output}");

  suggest_checked_and_stable(&repo, &json!({ "branches": { "journey/suspend": [] }, "unassign": [] }));
}

/// The incident: the fix uses the class the branch adds, so no stack builds both; the suggestion says to move it. The
/// symbols stack the fix on the branch whose class it uses; on report only the plan keeps them apart
#[test_log::test]
fn a_mutual_dangling_reference_suggests_moving_the_change() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (_, fixed_by) = commit_dangling_reference(&repo);
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 1, "{output}");
  assert_eq!(output["plan"], json!({ "branches": { "fake-agent/lane-document": [] }, "unassign": [] }), "{output}");
  assert_eq!(
    output["warnings"],
    json!([{
      "kind": "unresolved_conflict",
      "branches": ["fake-agent"],
      "message": format!(
        "fake-agent is dangling_reference after 0 repair rounds: {fixed} in {ACP_HOST} changes the use of CONTROL_URL_ENV with what the branch adds, so no stack builds both: move {fixed}, which changes only {ACP_HOST}, into fake-agent",
        fixed = short_id(&fixed_by),
      ),
    }])
  );

  git(&repo, &["config", "branchdeck.symbols", "report"]);
  let (_, output) = run_in(&repo, &["suggest"]);
  assert_eq!(output["plan"], json!({ "branches": {}, "unassign": [] }), "{output}");
}

/// With the symbols on, `flow` uses a function `util` adds and no line links them: without stacked branches the plan
/// keeps them apart and warns; with them it stacks `flow` on `util`
#[test_log::test]
fn with_the_symbols_on_a_use_of_another_branch_warns_or_stacks() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  commits(
    &repo,
    &[
      ("(util) Add controlUrl", &[("src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n")]),
      (
        "(flow) Start with controlUrl",
        &[("src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n")],
      ),
    ],
  );
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  let warning = output["warnings"]
    .as_array()
    .unwrap()
    .iter()
    .find(|warning| warning["kind"] == json!("symbol_dependency"))
    .unwrap();
  assert_eq!(warning["branches"], json!(["flow", "util"]), "{output}");
  assert!(
    warning["message"].as_str().unwrap().starts_with("flow uses controlUrl, which util adds (src/b/Use.kt:3)"),
    "{warning}"
  );

  git(&repo, &["config", "branchdeck.stacks", "true"]);
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  let flow = output["clusters"].as_array().unwrap().iter().find(|cluster| cluster["name"] == json!("util/flow")).unwrap();
  assert_eq!(flow["parent"], json!("util"), "{output}");
  assert!(
    output
      .get("warnings")
      .is_none_or(|warnings| warnings.as_array().unwrap().iter().all(|warning| warning["kind"] != json!("symbol_dependency"))),
    "{output}"
  );
}

/// The Safe Push failure of 2026-10-17: `xyz` overrides an overload that `abc` adds and no line links them; with the
/// symbols on and stacked branches the plan stacks `xyz` on `abc`, on report only it keeps them apart
#[test_log::test]
fn with_the_symbols_on_an_override_of_another_branch_stacks() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  commit_override_of_overload(&repo);
  let suggest = || {
    let (exit_code, output) = run_in(&repo, &["suggest"]);
    assert_eq!(exit_code, 0, "{output}");
    let clusters = output["clusters"].as_array().unwrap().iter();
    clusters.map(|cluster| (cluster["name"].clone(), cluster["parent"].clone())).collect::<Vec<_>>()
  };

  assert_eq!(suggest(), [(json!("abc"), Value::Null), (json!("abc/xyz"), json!("abc"))]);
  git(&repo, &["config", "branchdeck.symbols", "report"]);
  assert_eq!(suggest(), [(json!("abc"), Value::Null), (json!("xyz"), Value::Null)]);
}

/// The plan of a suggestion as commit -> the path it goes to (`None`: unassigned), and the commits its clusters keep,
/// move and give a new path
struct PlanLists {
  plan: BTreeMap<String, Option<String>>,
  kept: Vec<String>,
  moved: Vec<String>,
  new_path: Vec<String>,
}

fn plan_kept_and_moved(output: &Value) -> PlanLists {
  let ids = |value: &Value| value.as_array().unwrap().iter().map(|id| id.as_str().unwrap().to_string()).collect::<Vec<_>>();
  let mut plan: BTreeMap<String, Option<String>> = BTreeMap::new();
  for (path, commits) in output["plan"]["branches"].as_object().unwrap() {
    plan.extend(ids(commits).into_iter().map(|id| (id, Some(path.clone()))));
  }
  plan.extend(ids(&output["plan"]["unassign"]).into_iter().map(|id| (id, None)));
  let clusters = output["clusters"].as_array().unwrap();
  let kept = clusters.iter().flat_map(|cluster| ids(&cluster["kept"])).collect();
  let moved = clusters
    .iter()
    .flat_map(|cluster| cluster["moved"].as_array().unwrap())
    .map(|moved| moved["commit"].as_str().unwrap().to_string())
    .collect();
  let new_path = clusters.iter().flat_map(|cluster| ids(&cluster["new_path"])).collect();
  PlanLists { plan, kept, moved, new_path }
}

/// A commit with an explicit `(prefix)` stays in its branch: its own prefix is its anchor. Neither the plan nor
/// `--rewrite-issues` moves it, only an unprefixed commit, its fixup and whole issue branches move. With stacked
/// branches the one exception is the path a subject declares: the plan rewrites `(gone/a)` to `(a)`, and the commit
/// stays in its branch, in `new_path`
#[test_log::test]
fn a_plan_never_moves_a_commit_with_an_explicit_prefix() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commits(
    &repo,
    &[
      ("(feat) Add x", &[("x/a.rs", "a\n")]),
      ("Tune x", &[("x/a.rs", "b\n")]),
      ("(old) ABC-1 First", &[("y/one.rs", "one\n")]),
      ("ABC-1 Second", &[("y/two.rs", "two\n")]),
      ("XA-1 Add foo", &[("a/foo.rs", "foo\n")]),
      ("XA-2 Add bar\n\nAfter XA-1.", &[("b/bar.rs", "bar\n")]),
      ("(docs) Tidy the guide", &[("docs/guide.md", "guide\n")]),
      ("fixup! Tune x", &[("x/a.rs", "c\n")]),
      ("fixup! (feat) Add x", &[("x/a.rs", "d\n")]),
    ],
  );
  let short = |index: usize| short_id(&ids[index]).to_string();
  let prefixed = [0, 2, 6, 8].map(short);
  for args in [&["suggest"][..], &["suggest", "--rewrite-issues"]] {
    let (exit_code, output) = run_in(&repo, args);
    assert_eq!(exit_code, 0, "{output}");
    let PlanLists { plan, kept, moved, new_path } = plan_kept_and_moved(&output);
    assert_eq!(new_path, Vec::<String>::new(), "{args:?} {output}");
    // the unprefixed follow-up moves into the branch of what it needs, and its fixup follows it
    assert_eq!(plan.get(&short(1)), Some(&Some("feat".to_string())), "{args:?} {output}");
    assert!(moved.contains(&short(7)) && !plan.contains_key(&short(7)), "{args:?} {output}");
    for id in &prefixed {
      assert!(!plan.contains_key(id) && kept.contains(id) && !moved.contains(id), "{id} {args:?} {output}");
    }
  }
  // --rewrite-issues gives the whole issue branches a prefix, and still none to a commit that has one
  let (_, rewriting) = run_in(&repo, &["suggest", "--rewrite-issues"]);
  let plan = plan_kept_and_moved(&rewriting).plan;
  assert!(plan.contains_key(&short(4)) && plan.contains_key(&short(5)), "{rewriting}");

  let (repo, ids) = stacked_repo(&[
    ("(gone/a) Add a", &[("a.txt", "a\n")]),
    ("(gone/a/b) Add b", &[("b.txt", "b\n")]),
    ("(c) Add c", &[("c.txt", "c\n")]),
    ("Tune c", &[("c.txt", "c2\n")]),
  ]);
  let short = |index: usize| short_id(&ids[index]).to_string();
  let (exit_code, output) = run_in(&repo, &["suggest"]);
  assert_eq!(exit_code, 0, "{output}");
  let PlanLists { plan, kept, moved, new_path } = plan_kept_and_moved(&output);
  assert_eq!(
    plan,
    BTreeMap::from([(short(0), Some("a".to_string())), (short(1), Some("a/b".to_string())), (short(3), Some("c".to_string())),])
  );
  // the rewritten paths keep the leaf: the commits stay in their branches, with a new subject and a new id
  assert_eq!((kept, new_path, moved), (vec![short(2)], vec![short(0), short(1)], vec![short(3)]));
}
