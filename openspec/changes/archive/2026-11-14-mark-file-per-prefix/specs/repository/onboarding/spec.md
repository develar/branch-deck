## MODIFIED Requirements

### Requirement: Before the first sync: The mark of a sync names its prefix

The mark of a sync in the worktree MUST name the prefix of each sync that wrote it (`branch-deck/synced.d/`, a file per
prefix, so two syncs at once both land), and a session MUST be synced before by the mark only when it names the
session's prefix: `brd sync --prefix X` marks the session of `X` alone. The `lastSyncTime` of the app is of the path.

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
- **GIVEN** a build before left one file of lines, a time among them
- **THEN** its prefixes count and the time names none
- **AND** the next mark moves its prefixes into files of their own and removes it
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_of_a_build_before_is_read_and_moved_by_the_next_mark`)
