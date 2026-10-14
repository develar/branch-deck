use crate::fixture_test::{commit_all, git, numbered_lines, repo_with_baseline, run_in, write};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn short(id: &str) -> String {
  id[..7].to_string()
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
