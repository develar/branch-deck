# Tasks

- [x] 1.1 `last_line::with_empty_last_lines` pairs the empty last lines of a comparison of the lines without them; the
  comparison of the views takes it.
- [x] 1.2 `by_line::merge_whole_texts` compares each side with the base without the empty last lines and joins the two;
  `merge_texts` takes it through `manager::merge_content_lines`.
- [x] 1.3 Tests: the shapes of the merges at the end of a file, a property that the empty last line pairs only with
  that of the base, the cases against `git merge-file`, and the resolve by words that leaves two insertions at the end
  the conflict of git.
- [x] 1.4 `RESOLVE_VERSION` 2 and the notes of the tests, `MERGE_MODEL_VERSION` 4.
