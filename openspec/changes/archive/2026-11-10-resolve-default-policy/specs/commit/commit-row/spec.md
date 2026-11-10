## MODIFIED Requirements

### Requirement: Conflict view: The merge sees the conflicts of git at the end of a file

The merge MUST pair the empty line after the final line end of a side, and the one line of an empty text, only with
the same line of the base, and a last line without an end only with an equal last line without an end, so that the
changes at the end of a file conflict where `git merge-file` conflicts; the resolve by words MUST never take such a
conflict for two changes apart.

#### Scenario: Two insertions at the end of a file
- **WHEN** one side adds a blank line at the end of a file and the other a line there, with LF or CRLF, or both add to
  an empty base
- **THEN** the merge has one conflict that its words do not resolve, and the resolve by words leaves the conflict of git
  [@test] crates/text-diff/src/merge_model_test.rs (`the_last_line_pairs_only_with_the_last_line_of_the_base_of_its_kind`)
  [@test] crates/git-ops/src/word_resolve_test.rs (`two_insertions_at_the_end_of_a_file_stay_the_conflict_of_git`)

#### Scenario: A change of the final line end beside a change of the last line
- **WHEN** one side adds or removes the final line end of a file, and the other changes its last line or adds lines
  after it
- **THEN** the merge has one conflict of that line, as git sees it; apart from the other change, the two merge
  [@test] crates/text-diff/src/merge_model_test.rs (`the_last_line_pairs_only_with_the_last_line_of_the_base_of_its_kind`)
  [@test] crates/git-ops/src/word_resolve_test.rs (`the_end_of_a_file_merges_as_git_merge_file_merges_it`)

#### Scenario: The end of a file as git merges it
- **WHEN** the sides change the end of a file: a blank line removed, a final line end added or removed, a line added
  beside a change elsewhere
- **THEN** the merge conflicts where `git merge-file` conflicts, and else gives its result
  [@test] crates/git-ops/src/word_resolve_test.rs (`the_end_of_a_file_merges_as_git_merge_file_merges_it`)

#### Scenario: Any three texts
- **WHEN** any three texts merge, under any whitespace policy
- **THEN** a line outside the blocks that is the last line of its text pairs with the last line of the base and of the
  other side, of its kind
  [@test] crates/text-diff/src/merge_model_test.rs (`blocks_partition_the_three_texts`)

### Requirement: Conflict view: A conflict its words resolve says so

A conflict that its words resolve MUST say so: "Git stops here; the conflict resolves by words" (`conflicts > 0`,
`unresolved == 0`, the `resolvedText` of each of its blocks).

#### Scenario: A conflict of other words
- **WHEN** the sides of a conflict changed other words
- **THEN** it resolves by its words, the `resolvedText` of each of its blocks
- **AND** with `conflicts > 0` and `unresolved == 0` the view says "Git stops here; the conflict resolves by words"
  [@test] crates/text-diff/src/merge_model_test.rs (`a_conflict_of_other_words_resolves_by_its_words`)

#### Scenario: A conflict of the same words
- **WHEN** the sides of a conflict changed the same words
- **THEN** its words do not resolve it, and the view does not say that it resolves by words
  [@test] crates/text-diff/src/merge_model_test.rs (`a_conflict_of_the_same_words_stays_with_the_user`)

#### Scenario: A conflict that only ignoring whitespace resolves
- **WHEN** the words resolve a conflict only when whitespace, the line ends too, does not count
- **THEN** its words do not resolve it: that resolve joins lines into one that no side has
  [@test] crates/text-diff/src/merge_model_test.rs (`a_conflict_that_only_ignoring_whitespace_resolves_stays_with_the_user`)
