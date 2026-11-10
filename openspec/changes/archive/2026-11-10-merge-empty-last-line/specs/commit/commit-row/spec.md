## ADDED Requirements

### Requirement: Conflict view: The merge sees the conflicts of git at the end of a file

The merge MUST pair the empty line after the final line end of a side, and the one line of an empty text, only with
the same line of the base, so that the changes at the end of a file conflict where `git merge-file` conflicts and
merge clean to its result elsewhere; the resolve by words MUST never take such a conflict for two insertions.

#### Scenario: Two insertions at the end of a file
- **WHEN** one side adds a blank line at the end of a file and the other a line there, with LF or CRLF, or both add to
  an empty base
- **THEN** the merge has one conflict that its words do not resolve, and the resolve by words leaves the conflict of git
  [@test] crates/text-diff/src/merge_model_test.rs (`the_empty_last_line_pairs_only_with_the_empty_last_line_of_the_base`)
  [@test] crates/git-ops/src/word_resolve_test.rs (`two_insertions_at_the_end_of_a_file_stay_the_conflict_of_git`)

#### Scenario: The end of a file as git merges it
- **WHEN** the sides change the end of a file: a blank line removed, a final line end added or removed, a line added
  beside a change elsewhere
- **THEN** the merge conflicts where `git merge-file` conflicts, and else gives its result
  [@test] crates/git-ops/src/word_resolve_test.rs (`the_end_of_a_file_merges_as_git_merge_file_merges_it`)

#### Scenario: Any three texts
- **WHEN** any three texts merge, under any whitespace policy
- **THEN** a line outside the blocks that is the empty last line of its text pairs with the empty last line of the
  base and of the other side
  [@test] crates/text-diff/src/merge_model_test.rs (`blocks_partition_the_three_texts`)
