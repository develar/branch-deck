## MODIFIED Requirements

### Requirement: Before the first sync: The mark of a sync names its prefix

The mark of a sync in the worktree MUST name the prefix of each sync that wrote it (`branch-deck/synced.d/` of the git
directory its `.git` names, a file per prefix named by its hash and holding it, so two syncs at once both land and a
prefix of any length is marked), and a session MUST be synced before by the mark only when it names the session's
prefix: `brd sync --prefix X` marks the session of `X` alone. The `lastSyncTime` of the app is of the path.

#### Scenario: A prefix changed after a sync
- **GIVEN** a repository the app synced under one prefix
- **WHEN** its prefix is another one, never synced
- **THEN** the session of the new prefix syncs as it opens, as `lastSyncTime` is of the path
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs the session of a prefix set in a terminal once the focus finds it`)

#### Scenario: brd synced under another prefix
- **GIVEN** a worktree only `brd sync --prefix other` synced
- **WHEN** a window opens it under the prefix `test`
- **THEN** the session of `test` does not sync of itself: the mark names `other` alone
- **AND** a sync under `test` adds its own, so both sessions sync of themselves
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_names_the_prefix_each_sync_built_under`)

#### Scenario: Two syncs under two prefixes at once
- **WHEN** syncs under several prefixes mark the worktree at once
- **THEN** every prefix is in the mark
  [@test] crates/sync-core/src/synced_mark_test.rs (`two_marks_under_two_prefixes_at_once_both_land`)

#### Scenario: The mark of a build before
- **GIVEN** a build before left one file of lines, a time among them, or a file per prefix named by the hex of its bytes
- **THEN** its prefixes count and the time names none
- **AND** the next mark moves its prefixes into files of this build and removes it
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_of_a_build_before_is_read_and_moved_by_the_next_mark`, `the_mark_named_by_the_hex_of_its_prefix_is_read_and_moved_by_the_next_mark`)

#### Scenario: A long prefix
- **WHEN** a sync succeeds under a prefix of 200 bytes
- **THEN** the mark names it, in a file named by the hash of the prefix that holds the prefix, and a file of that name
  that holds another prefix marks nothing
  [@test] crates/sync-core/src/synced_mark_test.rs (`a_prefix_of_200_bytes_is_marked_in_a_file_named_by_its_hash_that_holds_it`)

#### Scenario: A submodule and a separate git directory
- **WHEN** a sync succeeds in a submodule, or in a checkout of `git init --separate-git-dir`
- **THEN** the mark is read in the git directory their `.git` file names, where the sync wrote it
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_of_a_submodule_and_of_a_separate_git_directory_is_read_where_their_git_file_names_it`)
