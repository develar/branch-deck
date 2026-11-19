## ADDED Requirements

### Requirement: Before the first sync: A sync whose history ahead does not build on the baseline is deferred

A sync whose commits ahead of the baseline reach a root commit the baseline lacks (a baseline unrelated to HEAD, an
unrelated history merged in ahead of it, a graft or a shallow fetch that cut the parents of a commit ahead) MUST be
refused at that root, before any event and the update check, moving no ref (`unrelated_history`): the range would be
the whole history of HEAD. Off the local baseline only; a history whose roots the baseline holds syncs.

#### Scenario: A baseline unrelated to HEAD
- **WHEN** the baseline is a history unrelated to HEAD (a server served another repository as master)
- **THEN** the sync is refused at the first root, before any event, and moves no ref; once the baseline is back it syncs the same branches
  [@test] crates/sync-core/src/sync_test.rs (`sync_waits_while_head_has_history_unrelated_to_the_baseline`)
  [@test] crates/git-ops/src/commit_list_test.rs (`a_head_unrelated_to_the_baseline_is_refused_at_its_first_root`)
  [@test] crates/git-ops/src/commit_list_test.rs (`a_root_listed_after_an_older_dated_commit_is_refused_too`)

#### Scenario: No check and no build
- **WHEN** the sync is refused for a history unrelated to the baseline
- **THEN** it ran the reads of the walk and its one `log`, no `merge-base`, tree, commit or ref
  [@test] crates/sync-core/src/process_budget_test.rs (`a_sync_of_a_head_unrelated_to_the_baseline_builds_nothing`)

#### Scenario: An unrelated history merged in ahead of the baseline
- **WHEN** a merge ahead of the baseline joined an unrelated history
- **THEN** the read of the commits ahead is refused at its root
  [@test] crates/git-ops/src/commit_list_test.rs (`an_unrelated_history_merged_in_ahead_of_the_baseline_is_refused`)

#### Scenario: A history the baseline joined by a merge
- **WHEN** the baseline holds the merge that joined another history (a monorepo of several roots)
- **THEN** the commits ahead are read as always
  [@test] crates/git-ops/src/commit_list_test.rs (`a_history_the_baseline_joined_by_a_merge_is_not_refused`)

#### Scenario: Cut parents
- **WHEN** a graft or a shallow fetch cut the parents of a commit ahead
- **THEN** the read is refused at it, and a shallow boundary is named shallow, not unrelated
  [@test] crates/git-ops/src/commit_list_test.rs (`a_commit_ahead_whose_parents_are_cut_is_refused`)
  [@test] crates/git-ops/src/commit_list_test.rs (`a_shallow_boundary_ahead_of_the_baseline_is_refused_as_shallow`)

#### Scenario: The local baseline
- **WHEN** HEAD is the local baseline branch itself
- **THEN** every commit counts, its roots too, and nothing is refused
  [@test] crates/git-ops/src/commit_list_test.rs (`on_a_baseline_branch_with_two_roots_both_roots_are_left_out`)
  [@test] crates/git-ops/src/history_test.rs (`the_history_on_the_local_baseline_reads_its_root`)

#### Scenario: A rewrite
- **WHEN** a rewrite reads the commits ahead of an unrelated baseline
- **THEN** it is refused with `unrelated_history` as the sync, in the app as in `brd`
  [@test] crates/git-ops/src/history_test.rs (`the_history_ahead_of_an_unrelated_baseline_is_refused`)
  [@test] crates/bd-cli/src/sync_test.rs (`refuses_a_head_unrelated_to_the_baseline`)

## MODIFIED Requirements

### Requirement: Before the first sync: A deferred sync the user asked for says so in a toast

A sync the user asked for (Sync, ⌘R, the Ready to Sync card) MUST say so in an `info` toast, "Not synced: a rebase is
in progress" / "The branches sync once git finished it" ("Not synced: HEAD is detached" / "Check out a branch to sync
its commits"), a `warning` one for a history unrelated to the baseline; a sync the app started (on open, on focus,
after a change) MUST wait without a toast.

#### Scenario: A sync the user asked for
- **WHEN** the user's sync is deferred by a rebase in progress
- **THEN** an `info` toast says "Not synced: a rebase is in progress" / "The branches sync once git finished it"
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in an info toast that a sync waits for git, and leaves the branches as they were`)

#### Scenario: A detached HEAD
- **WHEN** the user's sync is deferred by a detached HEAD
- **THEN** the toast says "Not synced: HEAD is detached" / "Check out a branch to sync its commits", and the view of the
  session the same
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)

#### Scenario: A history unrelated to the baseline
- **WHEN** the user's sync is deferred because the branch has history unrelated to the baseline
- **THEN** a `warning` toast says "Not synced: master has history unrelated to origin/master" / "origin/master lacks
  41b940fd, a root commit of master: the branches sync once origin/master holds every root of master"
  [@test] src/model/store.test.ts (`is no failure when the branch has history unrelated to the baseline: a warning toast that says which side lacks what for a sync the user asked for`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says above the table that a sync waits while the branch has history unrelated to the baseline, and syncs once the baseline is back`)

#### Scenario: A sync the app started
- **WHEN** the sync of the open, of a focus or after a change is deferred
- **THEN** it waits without a toast
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)

### Requirement: Before the first sync: The view says why the sync waits

The view of the session MUST say why it waits (`Deferred`) until the next sync starts: a page without rows (a
repository synced before, opened during a rebase) shows it as an `info` alert in the body, in the words of the toast,
and the focus after git finished syncs; a history unrelated to the baseline, which no work of the user ends, shows as a
`warning` alert above the table of the sync before too.

#### Scenario: The state of the view
- **WHEN** a sync is deferred
- **THEN** the view of the session is `Deferred`, the table kept, until the next sync
  [@test] crates/repository-model/src/session_test.rs (`a_failed_sync_keeps_the_table`)
  [@test] crates/repository-model/src/session_test.rs (`a_sync_of_a_history_unrelated_to_the_baseline_waits_with_the_table`)

#### Scenario: A page without rows
- **GIVEN** a repository synced before, opened during a rebase
- **WHEN** its sync is deferred
- **THEN** the body shows an `info` alert in the words of the toast
- **AND** the focus after git finished syncs
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in the body that a sync waits for git while there is no table, and syncs once git finished`)

#### Scenario: A history unrelated to the baseline above the table
- **GIVEN** a synced repository whose fetch took a baseline unrelated to the branch
- **WHEN** the sync of a focus is deferred
- **THEN** a `warning` alert above the table names the root the baseline lacks, the rows stay, and no error shows
- **AND** once the baseline is back, the next sync syncs and the alert goes
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says above the table that a sync waits while the branch has history unrelated to the baseline, and syncs once the baseline is back`)
