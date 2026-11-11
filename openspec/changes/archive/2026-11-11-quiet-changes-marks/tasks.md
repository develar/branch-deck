# Tasks

- [x] 1.1 The model: `TextDiff.marks` with the runs, the moves, the renames and the starts; `starts_of`, filled by
  the comparison for every diff; the JSON shape and the property test of the starts.
- [x] 1.2 `marks_of`: the runs of formatting and of comments of a block from its tokens, a block cut at the line
  boundaries both sides share, the ends of a block, what is never marked; `is_quiet`, the one place a kind is quiet.
- [x] 1.3 The question a block is asked before the tokens, and the test that it changes no answer; the tokens of a
  blob kept per grammar; the marks in `work()` before the content goes out.
- [x] 1.4 The limits by work: `build_changes_ints_within`, `MAX_BLOCK_TOKENS`, `MAX_TOKEN_EDITS`,
  `MAX_FILE_TOKEN_WORK`, the size of a side; no limit by time (a diff the time cut short has the marks of its blocks).
- [x] 1.5 The `commit_marks` template, registered in the test server, and the test of its layout.
- [x] 1.6 The benchmark `marks_bench` and its numbers in `docs/design/diff-view.md`.
- [x] 1.7 The bindings and the `TextDiff` literals of the page.
