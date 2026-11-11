# Tasks

- [x] 1.1 The shift of a move as a text; the whitespace at the ends of a line as ranges under the policy that counts
  every character, none under the others; `find_moves` and `marks_of` take the policy.
- [x] 1.2 `LineMove.edited`; a line that stayed at its indent where the indentation has a meaning is edited without
  a range; no range empty on both sides or of the same text (`check` of `moves_test.rs`).
- [x] 1.3 `blanksBefore` and `blanksAfter` in place of a move of blank lines alone; `starts_of` reads them.
- [x] 1.4 `MAX_MOVE_WORK` 262,144, the test and the benchmark of the file a search costs most in, the moves and the
  stream of 200 files with moved functions.
- [x] 1.5 A pair of other keys is compared by word only in a row that may be a move (`Pair::Unread`);
  `MAX_UNREAD_PAIRS` and the test of a sorted list of lines that are alike.
