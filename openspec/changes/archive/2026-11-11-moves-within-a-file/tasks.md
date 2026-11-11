# Tasks

- [x] 1.1 `find_moves`: the lines a move may take, the key of a line, the chains of pairs along two blocks, the
  longest rows of exact lines with at most a quarter of edited ones, the choice of the longest first, the blank lines
  that moved with a move; the table of moves and the properties of any texts.
- [x] 1.2 The indent of a move, and a line at another indent where the indentation of the file has a meaning; the
  ranges of an edited pair by word (`is_similar_pair` for the crate).
- [x] 1.3 The limits by work (`MAX_MOVE_LINES`, `MAX_KEY_CANDIDATES`, `MAX_MOVE_WORK`, `MAX_COMPARED_PAIRS`,
  `MAX_PAIR_UNITS`) and their tests on 10,000 lines.
- [x] 1.4 `starts_of` with the moves: the old lines of a move are nothing to read; the property of the starts.
- [x] 1.5 `marks_of` fills the moves of every pair after its runs; `MODEL_VERSION`; the moves with the content, from
  memory, from the store and for a scan; the template test; the random edits with moved lines.
- [x] 1.6 The benchmark of the moves on 10,000 lines and its numbers in `docs/design/diff-view.md`.
- [x] 1.7 The empty last line of a text is no line of a move (`without_empty_last_line`); its table and the
  property of any texts.
- [x] 1.8 A pair of a row read again is work, and a chain of many short moves is read once for all of them
  (`Finder::many_moves`, `Reach`); its test of work, the property against the row read again for each move, its
  benchmark on 65,000 lines.
