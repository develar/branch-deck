use crate::resolve_check::{CheckFailure, DroppedLines, check_resolution, count_markers, has_markers};
use pretty_assertions::assert_eq;

/// `head`, a zdiff3 block (onto `U`, base `b`, commit `L`), `tail`
const MERGED: &[u8] = b"head\n<<<<<<< ae71478\nU\n||||||| e9319f9\nb\n=======\nL\n>>>>>>> af45219\ntail\n";

#[test]
fn a_resolution_inside_the_block_passes_and_reports_the_dropped_lines() {
  assert_eq!(
    check_resolution(MERGED, b"head\nU and L\ntail\n"),
    Ok(DroppedLines {
      upstream: vec!["U".to_string()],
      commit: vec!["L".to_string()],
    })
  );
  assert_eq!(check_resolution(MERGED, b"head\nU\nL\ntail\n"), Ok(DroppedLines::default()));
  // an empty replacement drops both sides
  assert_eq!(check_resolution(MERGED, b"head\ntail\n").unwrap().upstream, ["U"]);
}

#[test]
fn an_edit_outside_the_block_is_refused() {
  assert_eq!(check_resolution(MERGED, b"HEAD\nU\ntail\n"), Err(CheckFailure::OutsideConflict));
  assert_eq!(check_resolution(MERGED, b"head\nU\ntail\nmore\n"), Err(CheckFailure::OutsideConflict));
  assert_eq!(check_resolution(MERGED, b"head\nU\n"), Err(CheckFailure::OutsideConflict));
}

#[test]
fn markers_left_are_refused() {
  assert_eq!(check_resolution(MERGED, MERGED), Err(CheckFailure::UnresolvedMarkers));
  assert_eq!(check_resolution(MERGED, b"head\nU\n=======\nL\ntail\n"), Err(CheckFailure::UnresolvedMarkers));
}

#[test]
fn the_plain_runs_between_two_blocks_must_stay_in_order() {
  let merged = b"a\n<<<<<<< x\n1\n||||||| y\n0\n=======\n2\n>>>>>>> z\nmiddle\n<<<<<<< x\n3\n||||||| y\n0\n=======\n4\n>>>>>>> z\nb\n";
  assert_eq!(
    check_resolution(merged, b"a\n1\nmiddle\n4\nb\n").map(|d| (d.upstream, d.commit)),
    Ok((vec!["3".to_string()], vec!["2".to_string()]))
  );
  assert_eq!(check_resolution(merged, b"a\n1\n4\nb\n"), Err(CheckFailure::OutsideConflict));
}

#[test]
fn marker_lines_are_counted_only_with_seven_characters() {
  assert_eq!(count_markers(MERGED), 1);
  assert!(has_markers(MERGED));
  assert!(!has_markers(b"<<<<<<<<\n========\n== x\n"));
  assert_eq!(count_markers(b"<<<<<<< a\r\n<<<<<<<\n"), 2);
}
