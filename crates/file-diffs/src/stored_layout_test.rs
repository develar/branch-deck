//! What the stores keep is read back by its layout: a value written by a build whose type had other fields is read
//! as garbage, or not at all, unless its store was written under another version. So each stored type is pinned
//! here with the version of its store: a sample of it with something in every list, as JSON, which names every
//! field in its order. A field added, taken away, moved or renamed, in the type or in a type inside it, changes the
//! sample and fails the test, and the one who pins the new sample raises the version beside it.

use crate::merge::{MERGE_MODEL_VERSION, StoredMerge};
use crate::service::MODEL_VERSION;
use diff_marks::leftovers::Leftovers;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use text_diff::model::TextDiff;

/// The diff of a pair in the `file_diff` store (`MODEL_VERSION`), and inside a one-sided merge of the `file_merge`
/// store (`MERGE_MODEL_VERSION`)
const DIFF: &str = concat!(
  r#"{"oldLineCount":9,"newLineCount":9,"additions":1,"deletions":1,"lineEnds":{"old":"lf","new":"crlf","only":false},"#,
  r#""finalNewline":{"old":true,"new":false},"emptyLastLine":{"old":true,"new":false},"lineLevelOnly":false,"#,
  r#""blocks":[{"oldStart":1,"oldEnd":2,"newStart":1,"newEnd":2,"kind":"changed","inlinePairs":true,"oldRanges":[[0,1,2]],"newRanges":[[0,1,2]]}],"#,
  r#""oldScopes":[{"startLine":0,"endLine":9,"label":"class A"}],"newScopes":[{"startLine":0,"endLine":9,"label":"class A"}],"importBlocks":[0],"#,
  r#""marks":{"runs":[{"oldStart":1,"oldEnd":2,"newStart":1,"newEnd":2,"kind":"formatting","quiet":true}],"#,
  r#""moves":[{"oldStart":3,"newStart":7,"len":1,"indent":2,"blanksBefore":1,"blanksAfter":0,"edited":[0],"oldRanges":[[0,1,2]],"newRanges":[[0,1,2]]}],"#,
  r#""renames":[{"old":"a","new":"b","sites":1,"local":true,"declared":[0],"oldSites":[1,2,3],"newSites":[1,2,3]}],"starts":[0,1,1],"#,
  r#""subject":{"movedOut":[{"start":1,"len":2,"file":1,"line":3}],"movedIn":[{"start":1,"len":2,"file":1,"line":3}],"#,
  r#""declared":[{"rename":0,"proof":"asked","sites":3,"files":2}],"#,
  r#""used":[{"rename":{"old":"a","new":"b","sites":1,"local":false,"declared":[],"oldSites":[1,2,3],"newSites":[1,2,3]},"file":1,"line":2,"proof":"confirmed"}],"#,
  r#""confirmed":[0]}}}"#
);

/// What a pair left, in the `file_leftovers` store (`MODEL_VERSION`)
const LEFTOVERS: &str = concat!(
  r#"{"lines":[[{"start":1,"keys":[7],"solid":[3]}],[{"start":2,"keys":[7],"solid":[3]}]],"#,
  r#""undeclared":[{"old":"a","new":"b","sites":1,"local":false,"declared":[],"oldSites":[1,2,3],"newSites":[1,2,3]}],"#,
  r#""pending":[{"block":0,"runs":[{"run":{"oldStart":1,"oldEnd":2,"newStart":1,"newEnd":2,"kind":"renamed","quiet":false},"names":[0]}]}],"#,
  r#""declarations":[{"rename":0,"places":[[1,2,3,4]],"redeclared":true}],"proven":[0],"lone_returns":[false,true],"rewritten":[[3],[7]]}"#
);

/// A value read from its sample, written again
fn written<T: Serialize + DeserializeOwned>(sample: &str) -> String {
  let value: T = serde_json::from_str(sample).unwrap_or_else(|error| panic!("the sample is not read: {error}"));
  serde_json::to_string(&value).unwrap()
}

#[test]
fn the_layout_of_a_stored_value_is_pinned_with_the_version_of_its_store() {
  // the diffs and what they left: raise `MODEL_VERSION` with a new sample
  assert_eq!(
    (MODEL_VERSION, written::<TextDiff>(DIFF).as_str(), written::<Leftovers>(LEFTOVERS).as_str()),
    (20, DIFF, LEFTOVERS),
    "the layout of a stored diff or of what it left changed: raise MODEL_VERSION and pin the sample again"
  );
  // a merge of one side holds a diff: raise `MERGE_MODEL_VERSION` whenever the diff is another
  let one_sided = format!(r#"{{"OneSided":{{"deleted":"left","diff":{DIFF}}}}}"#);
  assert_eq!(
    (MERGE_MODEL_VERSION, written::<StoredMerge>(&one_sided)),
    (13, one_sided),
    "the layout of a stored merge changed: raise MERGE_MODEL_VERSION and pin the sample again"
  );
}
