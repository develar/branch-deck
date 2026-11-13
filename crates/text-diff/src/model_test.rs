//! The wire model of a diff: its JSON, and the values of the comparison it maps to and back.

use crate::model::{DiffMarks, FileMove, LineMove, LineRun, Rename, RenameCheck, RenameProof, ReorderMark, RunKind, SubjectMarks, TextDiff, UsedRename};
use line_diff::model::{DiffBudget, RangeKind, diff_texts};
use line_diff::policy::WhitespacePolicy;
use pretty_assertions::assert_eq;

const MODIFIED: u32 = RangeKind::Modified as u32;

/// The diff of the comparison alone, in the wire model
fn diff(old: &str, new: &str) -> TextDiff {
  diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE).into()
}

/// The model holds the values of the comparison as they are: the blocks mapped there and back are the blocks, and
/// the marks of a diff of the comparison alone are where the reading of each block starts
#[test]
fn the_model_maps_the_values_of_the_comparison_there_and_back() {
  let (old, new) = ("a\nb\r\nc\nd\n", "a\nB\r\nc\n\nx");
  let values = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
  let wire = TextDiff::from(values.clone());
  assert_eq!(wire.line_diff(), values);
  assert_eq!(wire.marks.starts, line_diff::model::starts_of(&values.blocks, &[], &[]));
  assert_eq!((wire.old_scopes.len(), wire.new_scopes.len(), wire.import_blocks.len()), (0, 0, 0));
}

#[test]
fn the_model_is_camel_case_json() {
  let json = serde_json::to_value(diff("a\n", "b\n")).expect("serializable");
  assert_eq!(json["blocks"][0]["kind"], "changed");
  assert_eq!(json["oldLineCount"], 2);
  assert_eq!(json["lineLevelOnly"], false);
}

/// The marks are a field of their own with every field written, an empty list too, and nothing of them is named
/// `kind` beside the tag of the content a view reads the diff under
#[test]
fn the_marks_are_camel_case_json_with_every_field() {
  let mut result = diff("a\nb\n", "a\nc\n");
  let json = serde_json::to_value(&result).expect("serializable");
  // what the other files of a subject say is no part of the diff of a pair: written, as nothing
  assert_eq!(
    json["marks"],
    serde_json::json!({ "runs": [], "moves": [], "renames": [], "starts": [0, 1, 1], "subject": null })
  );
  assert!(json.get("kind").is_none());

  result.marks = DiffMarks {
    runs: vec![LineRun {
      old_start: 1,
      old_end: 2,
      new_start: 1,
      new_end: 3,
      kind: RunKind::Formatting,
      quiet: true,
    }],
    moves: vec![LineMove {
      old_start: 4,
      new_start: 9,
      len: 2,
      indent: -2,
      blanks_before: 1,
      blanks_after: 0,
      edited: vec![0],
      old_ranges: vec![vec![0, 1, MODIFIED], vec![]],
      new_ranges: vec![vec![0, 2, MODIFIED], vec![]],
    }],
    renames: vec![Rename {
      old: "total".to_string(),
      new: "sum".to_string(),
      sites: 1,
      local: true,
      declared: vec![0],
      old_sites: vec![3, 4, 9],
      new_sites: vec![3, 4, 7],
    }],
    starts: vec![],
    subject: Some(Box::new(SubjectMarks {
      moved_out: vec![FileMove {
        start: 53,
        len: 4,
        file: 1,
        line: 17,
        declaration: None,
      }],
      moved_in: vec![],
      declared: vec![RenameCheck {
        rename: 0,
        proof: RenameProof::UsesOutside,
        sites: 3,
        files: 2,
      }],
      used: vec![UsedRename {
        rename: Rename {
          old: "openStore".to_string(),
          new: "connectStore".to_string(),
          sites: 1,
          old_sites: vec![4, 14, 23],
          new_sites: vec![4, 14, 26],
          ..Rename::default()
        },
        file: 2,
        line: 18,
        proof: RenameProof::Asked,
        moved: Some("reportTitle".to_string()),
      }],
      confirmed: vec![0],
      reordered: vec![ReorderMark {
        names: vec!["accounts".to_string()],
        old_start: 21,
        old_len: 3,
        new_start: 5,
        new_len: 3,
      }],
    })),
  };
  let json = serde_json::to_value(&result).expect("serializable");
  assert_eq!(
    json["marks"],
    serde_json::json!({
      "runs": [{ "oldStart": 1, "oldEnd": 2, "newStart": 1, "newEnd": 3, "kind": "formatting", "quiet": true }],
      "moves": [{
        "oldStart": 4, "newStart": 9, "len": 2, "indent": -2, "blanksBefore": 1, "blanksAfter": 0, "edited": [0],
        "oldRanges": [[0, 1, 2], []], "newRanges": [[0, 2, 2], []],
      }],
      "renames": [{ "old": "total", "new": "sum", "sites": 1, "local": true, "declared": [0], "oldSites": [3, 4, 9], "newSites": [3, 4, 7] }],
      "starts": [],
      "subject": {
        "movedOut": [{ "start": 53, "len": 4, "file": 1, "line": 17 }],
        "movedIn": [],
        "declared": [{ "rename": 0, "proof": "usesOutside", "sites": 3, "files": 2 }],
        "used": [{
          "rename": {
            "old": "openStore", "new": "connectStore", "sites": 1, "local": false, "declared": [],
            "oldSites": [4, 14, 23], "newSites": [4, 14, 26],
          },
          "file": 2, "line": 18, "proof": "asked", "moved": "reportTitle",
        }],
        "confirmed": [0],
        "reordered": [{ "names": ["accounts"], "oldStart": 21, "oldLen": 3, "newStart": 5, "newLen": 3 }],
      },
    })
  );
  for (proof, word) in [
    (RenameProof::Confirmed, "confirmed"),
    (RenameProof::OtherMeaning, "otherMeaning"),
    (RenameProof::OtherDeclaration, "otherDeclaration"),
    (RenameProof::Incomplete, "incomplete"),
    (RenameProof::Unavailable, "unavailable"),
  ] {
    assert_eq!(serde_json::to_value(proof).expect("serializable"), word);
  }
  // a diff written before the subject had marks reads as one without them
  let mut older = json.clone();
  older["marks"].as_object_mut().expect("an object").remove("subject");
  assert_eq!(serde_json::from_value::<TextDiff>(older).expect("deserializable").marks.subject, None);
  assert_eq!(serde_json::to_value(RunKind::Comments).expect("serializable"), "comments");
  assert_eq!(serde_json::to_value(RunKind::Renamed).expect("serializable"), "renamed");
  let back: TextDiff = serde_json::from_value(json).expect("deserializable");
  assert_eq!(back, result);
}
