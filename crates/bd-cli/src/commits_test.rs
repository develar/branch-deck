use crate::commits::resolve_ids;
use crate::error::ErrorCode;
use crate::fixture_test::{commit_all, git, numbered_lines, repo_with_baseline, run_in, write};
use git_ops::rewrite_plan::short_id;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

fn short(id: &str) -> String {
  short_id(id).to_string()
}

fn by_subject<'a>(output: &'a Value, subject: &str) -> &'a Value {
  output["commits"]
    .as_array()
    .unwrap()
    .iter()
    .find(|commit| commit["subject"] == json!(subject))
    .unwrap_or_else(|| panic!("no commit {subject:?} in {output}"))
}

#[test_log::test]
fn files_with_renames_binaries_and_deletions() {
  let repo = repo_with_baseline(&[("old.txt", &numbered_lines(10)), ("gone.txt", "bye\n"), ("keep.txt", "a\nb\nc\n")]);
  write(&repo, "image.bin", [0u8, 1, 2, 3]);
  write(&repo, "keep.txt", "a\nB\nc\nd\n");
  commit_all(&repo, "(assets) Add an image", 1_700_000_100);

  git(&repo, &["mv", "old.txt", "new name.txt"]);
  write(&repo, "new name.txt", numbered_lines(11));
  write(&repo, "image.bin", [0u8, 9, 9]);
  std::fs::remove_file(repo.path().join("gone.txt")).unwrap();
  commit_all(&repo, "Rename, change the image, delete", 1_700_000_200);

  let (exit_code, output) = run_in(&repo, &["commits", "--files"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    by_subject(&output, "(assets) Add an image")["files"],
    json!([
      { "path": "image.bin", "status": "A", "binary": true },
      { "path": "keep.txt", "status": "M", "additions": 2, "deletions": 1 },
    ])
  );
  assert_eq!(
    by_subject(&output, "Rename, change the image, delete")["files"],
    json!([
      { "path": "gone.txt", "status": "D", "additions": 0, "deletions": 1 },
      { "path": "image.bin", "status": "M", "binary": true },
      { "path": "new name.txt", "old_path": "old.txt", "status": "R", "additions": 1, "deletions": 0 },
    ])
  );

  // the counts are what `--numstat` reports
  let numstat = git(&repo, &["log", "-1", "-M", "--format=", "--numstat", "HEAD"]);
  assert_eq!(numstat, "0\t1\tgone.txt\n-\t-\timage.bin\n1\t0\told.txt => new name.txt");
}

#[test_log::test]
fn entries_groups_and_fixups() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "auth.rs", "fn login() {}\n");
  let login = commit_all(&repo, "(auth) Add login\n\nWith a body.", 1_700_000_100);
  write(&repo, "auth.rs", "fn login() { check() }\n");
  let fixup = commit_all(&repo, "fixup! (auth) Add login", 1_700_000_200);
  write(&repo, "other.rs", "x\n");
  let orphan = commit_all(&repo, "squash! Nothing like this", 1_700_000_300);

  let (exit_code, output) = run_in(&repo, &["commits"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["schema"], json!(1));
  assert_eq!(output["prefix"], json!("test-user"));
  assert_eq!(output["baseline"], json!("origin/master"));
  assert_eq!(output["head"], json!(short(&orphan)));
  assert_eq!(
    output["commits"],
    json!([
      {
        "id": short(&login),
        "subject": "(auth) Add login",
        "body": "With a body.",
        "author": "Test User <test@example.com>",
        "time": 1_700_000_100,
        "group": "auth",
      },
      {
        "id": short(&fixup),
        "subject": "fixup! (auth) Add login",
        "author": "Test User <test@example.com>",
        "time": 1_700_000_200,
        "group": "auth",
        "fixup": { "kind": "fixup", "target": short(&login), "target_subject": "(auth) Add login" },
      },
      {
        "id": short(&orphan),
        "subject": "squash! Nothing like this",
        "author": "Test User <test@example.com>",
        "time": 1_700_000_300,
        "group": null,
        "fixup": { "kind": "squash", "target": null, "target_subject": "Nothing like this" },
      },
    ])
  );
  assert_eq!(output["dependencies"], json!([{ "from": short(&fixup), "to": short(&login), "files": ["auth.rs"] }]));
}

#[test_log::test]
fn dependency_edges_and_cluster_overlap() {
  let repo = repo_with_baseline(&[("shared.txt", &numbered_lines(30))]);
  // a: adds lines after line 2; b: replaces a line `a` added (overlap); c: changes line 25, far from both
  let mut content = numbered_lines(30);
  content = content.replace("line 2\n", "line 2\nadded 1\nadded 2\n");
  write(&repo, "shared.txt", &content);
  let a = commit_all(&repo, "(first) Add lines", 1_700_000_100);
  write(&repo, "shared.txt", content.replace("added 2\n", "changed 2\n"));
  let b = commit_all(&repo, "(second) Change an added line", 1_700_000_200);
  content = content.replace("added 2\n", "changed 2\n").replace("line 25\n", "line twenty-five\n");
  write(&repo, "shared.txt", &content);
  let c = commit_all(&repo, "(third) Change a far line", 1_700_000_300);

  let (exit_code, output) = run_in(&repo, &["commits"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["dependencies"], json!([{ "from": short(&b), "to": short(&a), "files": ["shared.txt"] }]));
  assert!(output["commits"].as_array().unwrap().iter().all(|commit| commit.get("files").is_none()));
  assert_eq!(
    output["cluster_overlap"],
    json!([
      { "branches": ["first", "second"], "files": ["shared.txt"] },
      { "branches": ["first", "third"], "files": ["shared.txt"] },
      { "branches": ["second", "third"], "files": ["shared.txt"] },
    ])
  );

  // --branch keeps the edges and overlaps that involve the branch
  let (exit_code, output) = run_in(&repo, &["commits", "--branch", "third", "--patch"]);
  assert_eq!(exit_code, 0, "{output}");
  let commits = output["commits"].as_array().unwrap();
  assert_eq!(commits.len(), 1);
  assert_eq!(commits[0]["id"], json!(short(&c)));
  let patch = commits[0]["patch"].as_str().unwrap();
  assert!(patch.starts_with("diff --git a/shared.txt b/shared.txt\n"), "{patch}");
  assert!(patch.contains("-line 25\n+line twenty-five\n"), "{patch}");
  assert_eq!(output["dependencies"], json!([]));
  assert_eq!(output["cluster_overlap"].as_array().unwrap().len(), 2);
}

#[test_log::test]
fn generated_files_are_flagged_and_unlinked() {
  let repo = repo_with_baseline(&[
    (".gitattributes", "gen/** linguist-generated\n"),
    ("Cargo.lock", &numbered_lines(5)),
    ("gen/types.ts", &numbered_lines(5)),
  ]);
  for (name, line, timestamp) in [("one", "line 2\n", 1_700_000_100), ("two", "line 3\n", 1_700_000_200)] {
    for path in ["Cargo.lock", "gen/types.ts"] {
      let content = std::fs::read_to_string(repo.path().join(path)).unwrap();
      write(&repo, path, content.replace(line, &format!("{name}\n")));
    }
    commit_all(&repo, &format!("({name}) Regenerate"), timestamp);
  }

  let (exit_code, output) = run_in(&repo, &["commits", "--files"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    by_subject(&output, "(one) Regenerate")["files"],
    json!([
      { "path": "Cargo.lock", "status": "M", "additions": 1, "deletions": 1, "generated": true },
      { "path": "gen/types.ts", "status": "M", "additions": 1, "deletions": 1, "generated": true },
    ])
  );
  assert_eq!(output["dependencies"], json!([]));
  assert_eq!(output["cluster_overlap"], json!([]));

  let (exit_code, output) = run_in(&repo, &["commits", "--include-generated"]);
  assert_eq!(exit_code, 0, "{output}");
  let commits = output["commits"].as_array().unwrap();
  assert_eq!(
    output["dependencies"],
    json!([{ "from": commits[1]["id"], "to": commits[0]["id"], "files": ["Cargo.lock", "gen/types.ts"] }])
  );
  assert_eq!(output["cluster_overlap"], json!([{ "branches": ["one", "two"], "files": ["Cargo.lock", "gen/types.ts"] }]));
}

#[test_log::test]
fn cached_output_equals_uncached() {
  let repo = repo_with_baseline(&[("shared.txt", &numbered_lines(30)), ("Cargo.lock", "a\n")]);
  write(&repo, "shared.txt", numbered_lines(30).replace("line 2\n", "line 2\nadded\n"));
  let first = commit_all(&repo, "(first) Add a line", 1_700_000_100);
  write(&repo, "shared.txt", numbered_lines(30).replace("line 2\n", "line 2\nchanged\n"));
  write(&repo, "Cargo.lock", "b\n");
  write(&repo, "image.bin", [0u8, 1]);
  let second = commit_all(&repo, "(second) Change it", 1_700_000_200);
  let cache_dir = repo.path().join(".git/branch-deck/cache");

  let (_, uncached) = run_in(&repo, &["--no-cache", "commits", "--files"]);
  assert!(!cache_dir.exists(), "--no-cache creates no cache");
  let (_, cold) = run_in(&repo, &["commits", "--files"]);
  assert!(cache_dir.join("data.mdb").exists());
  let cached = ObjectCache::open(&repo.path().join(".git")).get_commit_diffs(&[first, second]);
  assert_eq!(cached.len(), 2, "the cold run filled the cache");
  let (_, warm) = run_in(&repo, &["commits", "--files"]);
  assert_eq!(cold, uncached);
  assert_eq!(warm, uncached);
  assert_ne!(warm["dependencies"], json!([]));

  // status --stats reads the same cache
  let (_, uncached) = run_in(&repo, &["--no-cache", "status", "--stats"]);
  let (_, warm) = run_in(&repo, &["status", "--stats"]);
  assert_eq!(warm, uncached);
}

/// A prefixed commit with its fixup, two issue commits where the newer depends on the older, and an unassigned one
fn brief_repo() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n"), ("src/parser/p.rs", &numbered_lines(10))]);
  let mut ids = Vec::new();
  write(&repo, "src/auth/login.rs", "fn login() {}\n");
  ids.push(commit_all(&repo, "(auth) Add login", 1_700_000_100));
  write(&repo, "src/auth/login.rs", "fn login() { check() }\n");
  ids.push(commit_all(&repo, "fixup! (auth) Add login", 1_700_000_200));
  write(&repo, "src/parser/p.rs", numbered_lines(10).replace("line 5\n", "parse\n"));
  write(&repo, "README.md", "readme\nparser\n");
  ids.push(commit_all(&repo, "ABC-1 Add parser", 1_700_000_300));
  write(&repo, "docs/notes.md", "notes\n");
  ids.push(commit_all(&repo, "Tidy up", 1_700_000_400));
  write(&repo, "src/parser/p.rs", numbered_lines(10).replace("line 5\n", "parse lists\n"));
  ids.push(commit_all(&repo, "ABC-1 Parse lists", 1_700_000_500));
  (repo, ids)
}

#[test_log::test]
fn brief_view_with_units_and_areas() {
  let (repo, ids) = brief_repo();
  let ids: Vec<String> = ids.iter().map(|id| short(id)).collect();
  let (exit_code, output) = run_in(&repo, &["commits", "--brief"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["head"], json!(ids[4]));
  assert_eq!(
    output["commits"],
    json!([
      // the subject without the prefix, which `group` names
      { "id": ids[0], "subject": "Add login", "group": "auth", "areas": [{ "dir": "src/auth", "lines": 1 }] },
      {
        "id": ids[1],
        "subject": "fixup! Add login",
        "group": "auth",
        "fixup": { "kind": "fixup", "target": ids[0] },
        "areas": [{ "dir": "src/auth", "lines": 2 }],
      },
      // README.md is a hub file; the issue key is the group
      { "id": ids[2], "subject": "ABC-1 Add parser", "group": "ABC-1", "areas": [{ "dir": "src/parser", "lines": 2 }] },
      { "id": ids[3], "subject": "Tidy up", "group": null, "areas": [{ "dir": "docs", "lines": 1 }] },
      { "id": ids[4], "subject": "ABC-1 Parse lists", "group": "ABC-1", "areas": [{ "dir": "src/parser", "lines": 2 }] },
    ])
  );
  assert_eq!(output["dependencies"], json!([{ "from": ids[1], "to": ids[0] }, { "from": ids[4], "to": ids[2] }]));
  assert_eq!(output["units"], json!([[ids[0], ids[1]], [ids[2], ids[4]]]));

  let (_, output) = run_in(&repo, &["commits", "--brief", "--depth", "1", "--branch", "auth"]);
  assert_eq!(output["commits"].as_array().unwrap().len(), 2);
  assert_eq!(output["commits"][0]["areas"], json!([{ "dir": "src", "lines": 1 }]));
  assert_eq!(output["dependencies"], json!([{ "from": ids[1], "to": ids[0] }]));
  assert_eq!(output["units"], json!([[ids[0], ids[1]]]));

  // --depth needs --brief, --brief excludes --files and --patch
  for args in [
    &["commits", "--depth", "2"][..],
    &["commits", "--brief", "--files"],
    &["commits", "--brief", "--depth", "0"],
  ] {
    let (exit_code, output) = run_in(&repo, args);
    assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")), "{args:?}");
  }
}

#[test_log::test]
fn commit_ids_and_unassigned_filter_the_listing() {
  let (repo, ids) = brief_repo();
  let short_ids: Vec<String> = ids.iter().map(|id| short(id)).collect();
  // a full id and one abbreviated to 6 characters; the edges are computed over every commit
  let (exit_code, output) = run_in(&repo, &["commits", &ids[4], &ids[1][..6]]);
  assert_eq!(exit_code, 0, "{output}");
  let listed: Vec<&Value> = output["commits"].as_array().unwrap().iter().map(|commit| &commit["id"]).collect();
  assert_eq!(listed, [&json!(short_ids[1]), &json!(short_ids[4])]);
  assert_eq!(
    output["dependencies"],
    json!([{ "from": short_ids[1], "to": short_ids[0], "files": ["src/auth/login.rs"] }, { "from": short_ids[4], "to": short_ids[2], "files": ["src/parser/p.rs"] }])
  );
  assert_eq!(output["cluster_overlap"], json!([]));

  let (exit_code, output) = run_in(&repo, &["commits", "--unassigned", "--brief"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["commits"].as_array().unwrap().len(), 1);
  assert_eq!(output["commits"][0]["id"], json!(short_ids[3]));
  assert_eq!((&output["dependencies"], &output["units"]), (&json!([]), &json!([])));

  let (exit_code, output) = run_in(&repo, &["commits", "abc", &ids[0][..4]]);
  assert_eq!(exit_code, 2, "{output}");
  assert_eq!((&output["error"]["code"], &output["error"]["commits"]), (&json!("not_ahead_of_baseline"), &json!(["abc"])));
  let (exit_code, output) = run_in(&repo, &["commits", "--branch", "auth", &ids[0]]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")));
  let (exit_code, output) = run_in(&repo, &["commits", "--branch", "auth", "--unassigned"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")));
}

#[test]
fn an_ambiguous_id_is_invalid() {
  let commit_ids = ["abcd1111".to_string(), "abcd2222".to_string()];
  let error = resolve_ids(&["abcd".to_string()], &commit_ids).unwrap_err();
  assert_eq!((error.code, error.commits), (ErrorCode::InvalidArguments, vec!["abcd".to_string()]));
  let resolved = resolve_ids(&["abcd2".to_string()], &commit_ids).unwrap();
  assert_eq!(resolved.into_iter().collect::<Vec<_>>(), ["abcd2222"]);
}
