//! `brd commits --mark-stats` on the `commit_marks` template: the counts of each kind, the stops, the sample, the
//! subjects of a range, the same subject without its marks.

use bd_cli::fixture_test::{assert_error, environment, run_json};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The repository of the `commit_marks` template: one commit ahead of its baseline
fn template() -> tempfile::TempDir {
  let directory = tempfile::tempdir().unwrap();
  test_utils::repo_template::templates::commit_marks().build(directory.path()).unwrap();
  directory
}

fn mark_stats(directory: &tempfile::TempDir, args: &[&str]) -> (i32, Value) {
  let mut all = vec!["commits", "--mark-stats"];
  all.extend_from_slice(args);
  run_json(&environment(directory.path()), &all)
}

/// The lines of a side of a sampled mark that are lines of the mark, without what marks them
fn marked(mark: &Value, side: &str) -> Vec<String> {
  let lines = mark[side].as_array().unwrap().iter().map(|line| line.as_str().unwrap());
  lines
    .filter_map(|line| line.strip_prefix("| ").or_else(|| line.strip_prefix("~ ")))
    .map(|line| line.trim().to_string())
    .collect()
}

fn sampled<'a>(output: &'a Value, kind: &str) -> &'a Value {
  let sample = output["sample"].as_array().unwrap();
  sample
    .iter()
    .find(|mark| mark["kind"] == json!(kind))
    .unwrap_or_else(|| panic!("no {kind} in the sample: {output}"))
}

/// The counts of the commit of the template, as a view of it gets its marks: three files of Kotlin, fifteen blocks
/// and nine of them to read
#[test_log::test]
fn the_mark_stats_of_a_commit_count_each_kind_and_print_a_sample_of_their_lines() {
  let directory = template();
  let (exit_code, output) = mark_stats(&directory, &["--mark-sample", "2"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&output["subjects"], &output["subjects_of"], &output["marks"]),
    (&json!(1), &json!({ "kotlin": 1 }), &json!(true))
  );
  // the kinds a reader passes, and the thresholds the moves were read under
  assert_eq!(output["quiet_kinds"], json!(["formatting", "renamed"]));
  assert_eq!(
    output["limits"],
    json!({ "min_move_chars": 24, "max_edited_lines": [1, 4], "min_across_chars": 48, "min_across_lines": 2 })
  );

  let total = &output["total"];
  assert_eq!(total, &output["languages"]["kotlin"]);
  // every file was parsed, the marks read again are those of the view, and the wrapped call with the three lines of
  // the renamed local and the two blocks the moves left are six stops fewer
  let counts = ["files", "parsed", "differ", "blocks", "stops", "stops_proven"].map(|count| total[count].as_u64().unwrap());
  assert_eq!(counts, [3, 3, 0, 15, 9, 9]);
  assert_eq!(total["formatting"], json!({ "runs": 1, "quiet": 1, "proven": 1, "old_lines": 1, "new_lines": 3 }));
  assert_eq!(total["comments"], json!({ "runs": 1, "quiet": 0, "proven": 0, "old_lines": 1, "new_lines": 1 }));
  // the three lines of the local, and the two calls of the name another file declares, which no index proved
  assert_eq!(total["renamed"], json!({ "runs": 5, "quiet": 3, "proven": 3, "old_lines": 5, "new_lines": 5 }));
  assert_eq!(
    total["moves"],
    json!({
      "moves": 2, "lines": 11, "exact": 1, "edited": 1, "edited_lines": 1, "reindented": 1, "one_line": 0,
      "chars": [0, 0, 1, 1], "edited_share": [1, 0, 1], "unanchored": 0, "one_line_unanchored": 0,
    })
  );
  assert_eq!(
    total["renames"],
    json!({
      "local": 1, "local_proven": 1, "local_sites": 3, "declared": 1, "used": 1, "other_sites": 3,
      "proofs": { "unavailable": 2 },
    })
  );
  assert_eq!(total["across"], json!({ "moves": 1, "lines": 4, "chars": [0, 0, 1] }));
  assert_eq!(output.get("differ_in"), None);
  // one subject, with its times
  let time = &output["time"];
  assert_eq!((time["slowest"].as_array().unwrap().len(), &time["slowest"][0]["files"]), (1, &json!(3)));
  assert!(time["first_content_ms"]["max"].as_f64() <= time["end_ms"]["max"].as_f64(), "{time}");

  // the sample: two marks of a kind at most, each with the lines of both sides
  let kinds: Vec<&str> = output["sample"].as_array().unwrap().iter().map(|mark| mark["kind"].as_str().unwrap()).collect();
  assert_eq!(
    kinds,
    [
      "comments",
      "formatting",
      "move",
      "moveAcross",
      "moveEdited",
      "renameDeclared",
      "renameLocalProven",
      "renameUsed",
      "renamedLabel",
      "renamedLabel",
      "renamedProven",
      "renamedProven"
    ]
  );
  let wrapped = sampled(&output, "formatting");
  assert_eq!(
    (&wrapped["path"], &wrapped["language"], &wrapped["old_line"], &wrapped["new_line"]),
    (&json!("src/Ledger.kt"), &json!("kotlin"), &json!(7), &json!(12))
  );
  assert_eq!(marked(wrapped, "old"), ["val entry = Entry(account, amount, note)"]);
  assert_eq!(marked(wrapped, "new"), ["val entry = Entry(", "account, amount, note", ")"]);
  // a move with its edited line, the blank line that moved with it among its lines
  let moved = sampled(&output, "moveEdited");
  assert_eq!(moved["old"].as_array().unwrap().iter().filter(|line| line.as_str().unwrap().starts_with("~ ")).count(), 1);
  assert_eq!(marked(moved, "old").len(), 7);
  assert!(moved["note"].as_str().unwrap().starts_with("7 lines, 1 edited, indent 2, "), "{moved}");
  // a rename with the lines from its first place to its last, its places marked
  let renamed = sampled(&output, "renameLocalProven");
  assert_eq!(renamed["note"], json!("total -> sum, 3 places, 1 declared"));
  assert_eq!(marked(renamed, "new"), ["var sum = 0L", "sum += entry.amount", "return sum"]);
  // the lines that left for another file, from that file
  let across = sampled(&output, "moveAcross");
  assert_eq!((&across["path"], &across["note"]), (&json!("src/Report.kt"), &json!("4 lines from src/Ledger.kt:55")));
  assert_eq!(marked(across, "old"), marked(across, "new"));
}

/// The commits of a range are those ahead of the baseline here, and the range whole is one subject: the diff of its
/// two ends, as a review reads it
#[test_log::test]
fn the_subjects_are_the_commits_of_a_range_or_the_range_whole() {
  let directory = template();
  let (_, ahead) = mark_stats(&directory, &[]);
  let (exit_code, each) = mark_stats(&directory, &["--range", "origin/master..HEAD"]);
  assert_eq!(exit_code, 0, "{each}");
  assert_eq!((&each["subjects"], &each["head"], &each["total"]), (&json!(1), &ahead["head"], &ahead["total"]));
  // no sample was asked for
  assert_eq!((&ahead["sample"], &each["sample"]), (&json!([]), &json!([])));

  let (exit_code, whole) = mark_stats(&directory, &["--range", "origin/master..HEAD", "--whole"]);
  assert_eq!(exit_code, 0, "{whole}");
  assert_eq!((&whole["subjects"], &whole["total"]), (&json!(1), &ahead["total"]));
  let head = whole["head"].as_str().unwrap();
  assert!(head.contains("..") && head.ends_with(ahead["head"].as_str().unwrap()), "{head}");

  // an empty range has no subject
  let (exit_code, none) = mark_stats(&directory, &["--range", "HEAD..HEAD"]);
  assert_eq!((exit_code, &none["subjects"], &none["total"]["files"]), (0, &json!(0), &json!(0)));
  // a range is two revisions
  for range in ["HEAD", "no-such-commit..HEAD"] {
    let (exit_code, output) = mark_stats(&directory, &["--range", range]);
    assert_error(&output, exit_code, (2, "invalid_arguments"));
  }
}

/// Without the marks the same commit is its blocks, each a stop: what the marks are compared with for their cost
#[test_log::test]
fn the_mark_stats_without_the_marks_have_a_stop_at_each_block() {
  let directory = template();
  let (exit_code, output) = mark_stats(&directory, &["--no-marks", "--mark-sample", "2"]);
  assert_eq!(exit_code, 0, "{output}");
  let total = &output["total"];
  let counts = ["files", "parsed", "blocks", "stops", "stops_proven"].map(|count| total[count].as_u64().unwrap());
  assert_eq!((&output["marks"], counts), (&json!(false), [3, 0, 15, 15, 15]));
  for kind in ["formatting", "comments", "renamed"] {
    assert_eq!(total[kind]["runs"], json!(0), "{kind}");
  }
  assert_eq!((&total["moves"]["moves"], &total["across"]["moves"], &output["sample"]), (&json!(0), &json!(0), &json!([])));
  // and a view that reads the commit next gets its marks: nothing of this read was kept
  let (_, marked) = mark_stats(&directory, &[]);
  assert_eq!(marked["total"]["stops"], json!(9));
}
