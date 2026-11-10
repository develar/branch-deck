## MODIFIED Requirements

### Requirement: `brd sync`: A conflict the words resolve is committed

With `branchdeck.resolveByWords` on (the default), a cherry-pick that git stops at MUST be committed when the words
resolve every conflicting file (IntelliJ's simple resolve under the default policy, all or nothing per commit; only
content conflicts of UTF-8 text), with the author, the date and the message of the commit, and the branch built.

#### Scenario: A conflict its words resolve
- **WHEN** git stops at a commit whose conflicts its words resolve
- **THEN** the copy is committed with the author, the date and the message of the commit, and the branch is built
  [@test] crates/bd-cli/src/sync_test.rs (`a_conflict_that_its_words_resolve_builds_and_says_so`)
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_conflict_that_its_words_resolve_is_copied_with_the_author_and_the_message_of_the_commit`, `crlf_lines_keep_their_ends`, `a_stack_of_commits_builds_on_the_resolved_copy`)

#### Scenario: A conflict that only ignoring whitespace resolves
- **WHEN** the words of a conflict resolve only when whitespace, the line ends too, does not count: lines joined into
  one that no side has, or a final line end removed beside a removed last line
- **THEN** the commit stays a conflict of git
  [@test] crates/git-ops/src/word_resolve_test.rs (`a_conflict_that_only_ignoring_whitespace_resolves_stays_the_conflict_of_git`)

#### Scenario: Only content conflicts of UTF-8 text
- **WHEN** a conflict is a modify/delete, an add/add, a binary file, a file of a merge driver or text that is not UTF-8
- **THEN** the commit stays a conflict of git
  [@test] crates/git-ops/src/word_resolve_test.rs (`a_modify_delete_beside_a_resolvable_file_keeps_the_commit_a_conflict`, `an_add_add_and_a_binary_file_stay_conflicts`, `a_merge_driver_keeps_the_conflict_of_git`, `text_that_is_not_utf8_stays_a_conflict`)
