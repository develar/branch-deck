## ADDED Requirements

### Requirement: `brd sync`: A generated file the words merged is named with the commit its copy lacks

A copy whose conflict of git resolved by words in a generated file (by its name, its attributes `linguist-generated`
or `-diff`, or the header a tool wrote into a side) MUST still be committed, and `regenerate` of its branch MUST name
the file with `generated_by`, the copy that merged it (`commit`) and the last commit ahead of the baseline before it
that changes the file and that the copy lacks (`lacks`); `regenerate_fix` MUST say how the copy gets that commit.

#### Scenario: A lock file two branches change
- **GIVEN** `a` changes a line of `Cargo.lock`, and `b`, later, changes that line again
- **WHEN** `brd sync` copies `b` onto the baseline without `a`, and the words resolve the conflict
- **THEN** the copy of `b` is committed, and `regenerate` of `b` names `Cargo.lock` with `generatedBy` `lockFile`,
  the copy of `b` and the commit of `a` it lacks, by short ids
- **AND** `regenerate_fix` of `b` is the restack of `b` onto `a`, and `a`, whose copy is clean, has no `regenerate`
  [@test] crates/bd-cli/src/sync_test.rs (`brd_sync_names_the_generated_files_a_branch_must_regenerate_and_brd_push_refuses_them_until_allowed`)
  [@test] crates/sync-core/src/regenerate_test.rs (`a_generated_file_the_words_merged_is_named_with_the_commit_its_copy_lacks_until_a_later_commit_writes_it`)

#### Scenario: Why it is generated
- **WHEN** the resolve of a sync merges a generated file of each kind by words
- **THEN** each resolves, and its resolution names why it is generated: its name, its attributes, its header
- **AND** the same `check-attr` that reads the merge attribute reads its attributes, with no process more
  [@test] crates/git-ops/src/word_resolve_test.rs (`a_generated_file_resolves_in_a_sync_and_its_resolution_says_why_it_is_generated`)

#### Scenario: A file a person writes
- **WHEN** the words resolve the same conflict in a file a person writes
- **THEN** nothing is named
  [@test] crates/sync-core/src/regenerate_test.rs (`the_same_conflict_in_a_file_a_person_writes_needs_no_regenerate`)

#### Scenario: The commit the copy lacks
- **WHEN** the sync looks for the commit a copy lacks
- **THEN** it reads the commits ahead of the baseline that change the files, newest first, and takes the first below
  the original that lies in no branch of the tip
  [@test] crates/git-ops/src/generated_test.rs (`the_commits_that_change_a_generated_file_are_listed_newest_first`)

#### Scenario: The cost
- **WHEN** a sync, also an unchanged one, has no merge by words of a generated file
- **THEN** it runs no process for it
- **AND** with one at the tip of its branch it runs one `log`, and with one below a tip one `cat-file --batch-check`
  more, for every branch of the sync together
- **AND** a failure of either leaves the facts out with a warning, and the sync goes on
  [@test] crates/sync-core/src/process_budget_test.rs (`a_generated_file_merged_by_words_at_the_tip_costs_one_log`, `a_generated_file_merged_by_words_below_the_tip_costs_one_batch_check_and_one_log`, `a_copy_of_an_older_resolve_is_checked_once`)

### Requirement: `brd sync`: A generated file is named while the tip holds what the words wrote

A generated file MUST be named only while the tip of the branch holds the blob the words wrote (of each path the last
copy that merged it): a later commit of the branch that writes or deletes the file, or a sync that no longer needs the
words because the copy has the commit it lacked (stacked on its branch, the commit moved, or that branch landed),
MUST clear it. A copy reused by its mapping note MUST keep it.

#### Scenario: A later commit writes the file
- **WHEN** a later commit of the branch writes the generated file, as its generator would
- **THEN** the next sync names nothing
  [@test] crates/sync-core/src/regenerate_test.rs (`a_generated_file_the_words_merged_is_named_with_the_commit_its_copy_lacks_until_a_later_commit_writes_it`)

#### Scenario: The tip and the blob of the copy
- **WHEN** the merge lies below the tip, and the tip has the blob of the copy at the path, another blob or none
- **THEN** it is held only with the same blob; a merge in the copy at the tip is held without a read
  [@test] crates/git-ops/src/generated_test.rs (`a_merge_by_words_holds_while_the_tip_has_the_blob_the_words_wrote`)

#### Scenario: A branch stacked on the one its copy lacks
- **WHEN** the branch is stacked on the branch of the commit it lacked
- **THEN** its copy conflicts in nothing, and nothing is named
  [@test] crates/sync-core/src/regenerate_test.rs (`a_branch_stacked_on_the_one_its_copy_lacks_has_nothing_to_regenerate`)

#### Scenario: A reused copy
- **WHEN** the next sync reuses the copy by its mapping note
- **THEN** it names the file again, from the note
  [@test] crates/sync-core/src/regenerate_test.rs (`a_generated_file_the_words_merged_is_named_with_the_commit_its_copy_lacks_until_a_later_commit_writes_it`)

### Requirement: `brd sync`: A branch built on the copy that merged it names it too

A branch built on a branch whose copy merged a generated file by words MUST name it too, while its tip holds it: its
history holds that copy, and its pull request lands it. A stack branch MUST name what its build holds, of every member.

#### Scenario: The children and the stack branch
- **GIVEN** `beta` copies a lock file merged by words, and `gamma` and `delta` are stacked on `beta`, a stack that forks
- **WHEN** `brd sync` runs
- **THEN** `regenerate` of `beta`, `gamma` and `delta` names it, each with the copy of `beta` and the commit it lacks
- **AND** so does the stack branch of `beta`, which copies `beta` onto the baseline the same way
  [@test] crates/sync-core/src/regenerate_test.rs (`a_branch_built_on_the_one_whose_copy_merged_it_names_it_too_and_so_does_the_stack_branch`)

## MODIFIED Requirements

### Requirement: `brd sync`: The commits the words resolved are listed

`resolved` of the branch MUST list such commits with git's conflicts, the files and the generated ones among them
(`generated`, each with `generatedBy`); the mapping note keeps the counts and the generated files, and `brd status` and
`brd commits` MUST show them as `resolved_by_words` of the commit. `brd check` MUST build alike.

#### Scenario: The sync says so
- **WHEN** a sync resolved a commit by its words
- **THEN** `resolved` of the branch lists it with git's conflicts and the files
- **AND** `generated` names the generated files among them, and `brd status` names them in `resolved_by_words` from
  the mapping note
  [@test] crates/bd-cli/src/sync_test.rs (`a_conflict_that_its_words_resolve_builds_and_says_so`, `brd_sync_names_the_generated_files_a_branch_must_regenerate_and_brd_push_refuses_them_until_allowed`)

#### Scenario: The next sync
- **WHEN** the next sync reuses the resolved copy
- **THEN** it says so from the counts of its mapping note
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`the_next_sync_reuses_the_resolved_copy_and_says_so_from_its_note`)

#### Scenario: The resolve line of the mapping note
- **WHEN** a copy resolved generated files by words
- **THEN** its resolve line is `v-commit-resolved-v1:<version> <conflicts> <files> <json>`, one line, the JSON
  `[{"path","generatedBy"}]` left out for none
- **AND** a reader of the three numbers alone reads them and ignores the rest
  [@test] crates/git-ops/src/notes_test.rs (`a_resolve_line_carries_the_generated_files_after_its_counts_which_an_older_reader_ignores`)

#### Scenario: A check
- **WHEN** `brd check` builds a branch with such a commit
- **THEN** it resolves it as the sync does
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_check_resolves_as_the_sync_does`)

### Requirement: `brd sync`: A copy of an older resolve by words is checked once

A copy whose mapping note names an older resolve by words (before version 5 none names its generated files), or a
resolve line whose generated files do not parse, MUST be checked once by its tree: the same tree keeps the commit, with
no new copy and no move of its branch, and its note MUST take the current resolve, so that no later sync checks it
again; another tree MUST copy the commit again, and a conflict that the current resolve leaves MUST be the conflict of
git.

#### Scenario: The same bytes
- **WHEN** the current resolve gives the tree of the copy
- **THEN** the copy stays, unchanged, and its note names the current resolve
- **AND** the next sync runs the processes of an unchanged sync
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_with_the_same_bytes_keeps_its_commit_and_takes_this_resolve`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_copy_of_an_older_resolve_is_checked_once`)

#### Scenario: Other bytes
- **WHEN** the current resolve gives another tree
- **THEN** the commit is copied again with the tree of the current resolve, and its note names the new copy
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_with_other_bytes_is_copied_again`)

#### Scenario: A conflict the current resolve leaves
- **WHEN** the current resolve leaves the conflict of git
- **THEN** the commit is that conflict, and a copy noted by the current resolve is taken as it is
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_that_this_resolve_leaves_is_the_conflict_of_git`, `a_copy_of_resolve_3_with_a_line_its_whitespace_step_joined_is_the_conflict_of_git`)

#### Scenario: Generated files that do not parse
- **WHEN** the tail of a resolve line is no list of generated files
- **THEN** the note reads as resolve 0, an older resolve, and names no generated file
  [@test] crates/git-ops/src/notes_test.rs (`a_resolve_line_whose_generated_files_do_not_parse_is_read_as_an_older_resolve`)

### Requirement: `brd move-hunk`: Only the changed commits get a new committer

`from`, `to` and the commits between whose diff of the file changed (`patch-id --stable`) MUST keep their author and
get the current user as committer and no mapping note (the sync copies them again); every other commit MUST keep
author and committer byte for byte, and its mapping note: with the new trees as its source for a commit between, its
resolve by words and the generated files it merged kept, as it was above. So the sync rebuilds the branches of `from`
and `to` and keeps the others.

#### Scenario: The commits of a move
- **WHEN** a change moves past commits between
- **THEN** the commits whose diff of the file changed get the current user as committer and no mapping note
- **AND** the others keep author, committer and their mapping note, a commit between its resolve line with its
  generated files too
  [@test] crates/git-ops/src/move_hunk_test.rs (`a_commit_between_that_shifts_the_lines_still_applies`, `moves_a_file_change_into_an_older_commit`)
