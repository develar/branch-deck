use crate::resolve_check::{CheckFailure, Labels, block_count, check_resolution, count_markers, dropped_lines, has_markers};
use pretty_assertions::assert_eq;

const LABELS: Labels<'static> = Labels {
  onto: "ae71478",
  base: "af45219^",
  commit: "af45219",
};

/// `head`, a zdiff3 block (onto `U`, base `b`, commit `L`), `tail`
const MERGED: &[u8] = b"head\n<<<<<<< ae71478\nU\n||||||| af45219^\nb\n=======\nL\n>>>>>>> af45219\ntail\n";

#[test]
fn a_resolution_inside_the_block_passes() {
  assert_eq!(check_resolution(MERGED, b"head\nU and L\ntail\n", LABELS), Ok(()));
  assert_eq!(check_resolution(MERGED, b"head\ntail\n", LABELS), Ok(()));
}

#[test]
fn an_edit_outside_the_block_is_refused() {
  assert_eq!(check_resolution(MERGED, b"HEAD\nU\ntail\n", LABELS), Err(CheckFailure::OutsideConflict));
  assert_eq!(check_resolution(MERGED, b"head\nU\ntail\nmore\n", LABELS), Err(CheckFailure::OutsideConflict));
  assert_eq!(check_resolution(MERGED, b"head\nU\n", LABELS), Err(CheckFailure::OutsideConflict));
}

#[test]
fn changed_line_ends_are_named() {
  assert_eq!(check_resolution(MERGED, b"head\r\nU\r\ntail\r\n", LABELS), Err(CheckFailure::LineEnds));
  assert_eq!(check_resolution(MERGED, b"head\nU\ntail", LABELS), Err(CheckFailure::LineEnds));
}

#[test]
fn markers_left_are_refused() {
  assert_eq!(check_resolution(MERGED, MERGED, LABELS), Err(CheckFailure::UnresolvedMarkers));
  assert_eq!(check_resolution(MERGED, b"head\nU\n>>>>>>> af45219\ntail\n", LABELS), Err(CheckFailure::UnresolvedMarkers));
}

#[test]
fn the_plain_runs_between_two_blocks_must_stay_in_order() {
  let merged = b"a\n<<<<<<< ae71478\n1\n||||||| af45219^\n0\n=======\n2\n>>>>>>> af45219\nmiddle\n<<<<<<< ae71478\n3\n||||||| af45219^\n0\n=======\n4\n>>>>>>> af45219\nb\n";
  assert_eq!(block_count(merged, LABELS), 2);
  assert_eq!(check_resolution(merged, b"a\n1\nmiddle\n4\nb\n", LABELS), Ok(()));
  assert_eq!(check_resolution(merged, b"a\n1\n4\nb\n", LABELS), Err(CheckFailure::OutsideConflict));
}

/// A Markdown setext heading underlines with `=======`; outside a block it is text, inside a side too
#[test]
fn a_setext_heading_is_text() {
  let merged = b"Setup\n=======\n<<<<<<< ae71478\nU\n=======\n||||||| af45219^\nb\n=======\nL\n>>>>>>> af45219\nUsage\n=======\n";
  assert_eq!(block_count(merged, LABELS), 1);
  assert_eq!(check_resolution(merged, b"Setup\n=======\nU and L\nUsage\n=======\n", LABELS), Ok(()));
  assert!(!has_markers(b"Title\n=======\n<<<<<<< other\n", LABELS));
  assert_eq!(count_markers(merged, LABELS), 1);
}

/// `conflict-marker-size=10` in `.gitattributes`: the markers are longer, and only the labels tell them
#[test]
fn longer_markers_are_found_by_their_labels() {
  let merged = b"a\n<<<<<<<<<< ae71478\nU\n|||||||||| af45219^\nb\n==========\nL\n>>>>>>>>>> af45219\nc\n";
  assert_eq!((block_count(merged, LABELS), count_markers(merged, LABELS)), (1, 1));
  assert_eq!(check_resolution(merged, merged, LABELS), Err(CheckFailure::UnresolvedMarkers));
  assert_eq!(check_resolution(merged, b"a\nU L\nc\n", LABELS), Ok(()));
  // markers of another merge, and runs that are too short, are text
  assert_eq!(block_count(b"<<<<<<< 1234567\nx\n", LABELS), 0);
  assert_eq!(block_count(b"<<<<<< ae71478\nx\n", LABELS), 0);
}

/// With a rename, merge-ort writes `<label>:<path>` after each marker, the path as it is
#[test]
fn markers_of_a_renamed_file_carry_the_path() {
  let merged = "a\n<<<<<<< ae71478:d é\".txt\nU\n||||||| af45219^:a b.txt\nb\n=======\nL\n>>>>>>> af45219:a b.txt\nc\n".as_bytes();
  assert_eq!((block_count(merged, LABELS), count_markers(merged, LABELS)), (1, 1));
  assert_eq!(check_resolution(merged, merged, LABELS), Err(CheckFailure::UnresolvedMarkers));
  assert_eq!(check_resolution(merged, b"a\nU L\nc\n", LABELS), Ok(()));
  // another label that only starts with this one is not this merge's
  assert_eq!(block_count(b"<<<<<<< ae71478x\n||||||| af45219^\n=======\n>>>>>>> af45219\n", LABELS), 0);
}

#[test]
fn dropped_lines_are_the_lines_of_a_side_that_the_result_lost() {
  let before = b"fn parse(path) {\n  read(path)\n}\n";
  let onto = b"fn parse(path, strict) {\n  read(path)\n}\n";
  let commit = b"fn load(path) {\n  read(path)\n}\n";
  // both changes combined: a rewritten line is no drop
  assert_eq!(dropped_lines(before, onto, b"fn load(path, strict) {\n  read(path)\n}\n"), Vec::<String>::new());
  assert_eq!(dropped_lines(before, commit, b"fn load(path, strict) {\n  read(path)\n}\n"), Vec::<String>::new());
  // the commit's side taken: the upstream line is gone
  assert_eq!(dropped_lines(b"a\n", b"a\nupstream only line\n", b"a\ncompletely different\n"), ["upstream only line"]);
  // a deleted file drops every line the side added
  assert_eq!(dropped_lines(b"", b"x = 1\n}\n", b""), ["x = 1"]);
}

/// The cases of the dogfood review: a lost change of one value is reported, a rewrite that keeps the change is not
#[test]
fn a_lost_change_of_one_value_is_reported() {
  assert_eq!(dropped_lines(b"set APP_MODE=dev\n", b"set APP_MODE=test\n", b"set APP_MODE=prod\n"), ["set APP_MODE=test"]);
  let lock = b"name = \"app\"\nversion = \"1.0.149\"\nchecksum = \"aaaa\"\n";
  let upstream = b"name = \"app\"\nversion = \"1.0.150\"\nchecksum = \"cccc\"\n";
  assert_eq!(dropped_lines(lock, upstream, lock), ["version = \"1.0.150\"", "checksum = \"cccc\""]);
  // `--take commit:` loses the upstream version, `--take onto:` the commit's flag
  assert_eq!(
    dropped_lines(b"cargo install app@1\n", b"cargo install app@2\n", b"cargo install app@1 --force\n"),
    ["cargo install app@2"]
  );
  assert_eq!(
    dropped_lines(b"cargo build\n", b"cargo build --locked\n", b"cargo build --release\n"),
    ["cargo build --locked"]
  );
  // the result keeps the word the side added
  assert_eq!(dropped_lines(b"Setup\n", b"Setup steps\n", b"Set up steps\n"), Vec::<String>::new());
  // the merge of the first round: both changes in one line, and in the comment above it
  let before = b"/// Reads the config file at `path`.\npub fn parse_config(path: &str) -> Config {\n";
  let onto = b"/// Reads the config file at `path`; `strict` fails on a missing file.\npub fn parse_config(path: &str, strict: bool) -> Config {\n";
  let commit = b"/// Loads the config file at `path`.\npub fn load_config(path: &str) -> Config {\n";
  let result = b"/// Loads the config file at `path`; `strict` fails on a missing file.\npub fn load_config(path: &str, strict: bool) -> Config {\n";
  assert_eq!((dropped_lines(before, onto, result), dropped_lines(before, commit, result)), (Vec::new(), Vec::new()));
}
