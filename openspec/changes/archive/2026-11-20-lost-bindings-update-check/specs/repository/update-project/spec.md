## ADDED Requirements

### Requirement: Check: A clean replay that loses a binding is a conflict

The check of an update that would apply a clean rebase MUST read the lost bindings of its replay before the lock: a
copy of a local commit (paired by author email and time and subject) removes the import or the private declaration of
a name its file still uses, which the local commit does not. A finding MUST make it a conflict that moves nothing:
`FirstConflict` names the first such commit, its files, their upstream commits and `lost_bindings` with their lines.

#### Scenario: The incident
- **WHEN** the clean replay of a commit that drops a private property and an import with their last use lands on an
  upstream commit that uses both again in another hunk
- **THEN** the update answers a conflict naming the commit, `src/Launch.kt`, the upstream commit and both lost bindings
  with their lines
- **AND** nothing moves and no ref is written
  [@test] crates/update-core/src/update_project_test.rs (`a_clean_replay_that_loses_bindings_is_a_conflict_that_moves_nothing`)
  [@test] crates/update-core/src/process_budget_test.rs (`an_update_whose_clean_replay_loses_bindings`)
  [@test] crates/trial-check/src/lost_bindings_test.rs (`a_replay_brings_the_lost_bindings_of_its_copy_in_two_processes`)

### Requirement: Check: Only what the replay brought and the tip lacks is a lost binding

A lost binding MUST be what the replay brought: an upstream change of the file without a use of what the commit
removes, a lost binding the local commit had where it was authored (it resolves another way there), and a name that a
later local commit binds again at the tip are none, and the update applies. An upstream that changed no file of the
local commits costs one `log` and no summary.

#### Scenario: No use upstream
- **WHEN** the upstream changes the file without a use of what the commit removes
- **THEN** the update rebases cleanly
  [@test] crates/update-core/src/update_project_test.rs (`an_upstream_change_without_a_use_of_what_the_commit_removes_applies`)

#### Scenario: The commit's own finding
- **WHEN** the local commit removes an import its file still uses where it was authored
- **THEN** the update rebases cleanly
  [@test] crates/update-core/src/update_project_test.rs (`a_lost_binding_the_commit_had_where_it_was_authored_is_no_finding`)

#### Scenario: Bound again at the tip
- **WHEN** a later local commit puts the import and the property back
- **THEN** the update rebases cleanly, though the copy of the first commit loses both
  [@test] crates/update-core/src/update_project_test.rs (`a_later_commit_that_binds_the_name_again_leaves_nothing_lost`)

#### Scenario: No file in common
- **WHEN** the upstream changed no file of the local commits
- **THEN** the check reads one `log` and no summary
  [@test] crates/trial-check/src/lost_bindings_test.rs (`a_replay_that_changes_no_file_of_a_copy_reads_no_summary`)

### Requirement: Header: A conflict of lost bindings says the commits remove names in use

The words of a conflict with lost bindings (the tooltip, the entry of Activity) MUST say "Your commits remove names that
origin/master uses in src/Launch.kt", short "removes names in use", never that the files conflict.

#### Scenario: Lost bindings
- **WHEN** the state is a conflict with lost bindings
- **THEN** its words are "Your commits remove names that origin/master uses in src/Launch.kt", short "removes names in use"
  [@test] src/lib/update-state.test.ts (`says that a replay that loses bindings removes names the upstream uses, not that it conflicts`)

### Requirement: Conflict and blocked dialog: Lost bindings say what is lost and where

A conflict with lost bindings MUST title "Your commits remove names that origin/master uses" and list each lost
binding as "<commit> removes the import of rethrow, which <upstream commit> uses at Launch.kt:12" (the upstream commit of
the file when it has one, else the upstream), then its files with the commits of both sides; no "First conflict" line.

#### Scenario: The incident in the dialog
- **WHEN** the update of `update_lost_bindings` is refused
- **THEN** the dialog titles "Your commits remove names that origin/master uses" and lists `backend` and `rethrow` with
  the upstream commit and their lines, and the file `src/Launch.kt`
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the bindings a clean replay loses and copy a prompt that keeps them`)
  [@test] src/presenters/lost-bindings.test.ts (`names the commit, what it removes, and the upstream commit of the file that uses it`, `names the upstream when several of its commits change the file`)

### Requirement: Conflict and blocked dialog: The prompt of lost bindings keeps each name in a fixup commit

The prompt of a conflict with lost bindings MUST name each lost binding with its whole path instead of a first
conflict. With brd its steps are those of a conflict: brd opens the lost-bindings round. Without brd it MUST rebase,
then keep each name (the import or the private declaration back, or the use dropped) in a commit
`fixup! <subject>`.

#### Scenario: With and without brd
- **WHEN** the user copies the prompt of a conflict with lost bindings
- **THEN** with brd it names the lost bindings and runs `brd update --no-fetch`; without brd it rebases and keeps each
  name in a `fixup!` commit
  [@test] src/lib/agent-prompt.test.ts (`with brd, names the lost bindings instead of a first conflict: brd opens their round`, `without brd, rebases and keeps each lost name in a fixup commit`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the bindings a clean replay loses and copy a prompt that keeps them`)

## MODIFIED Requirements

### Requirement: Conflict and blocked dialog: The line above the conflict says what to do

The line above SHALL say to rebase in a terminal or the IDE, or, with a brd command, that the prompt resolves each
commit with brd, outside the working tree. A conflict with lost bindings SHALL say instead that git merges the commits
but the result would not compile, and that the prompt keeps each name in a fixup commit: no rebase in a terminal helps.

#### Scenario: With a brd command
- **WHEN** the app has a brd command in the repository
- **THEN** the line above says that the prompt resolves each commit with brd, outside the working tree

#### Scenario: Without a brd command
- **WHEN** the app has no brd command
- **THEN** the line above says to rebase in a terminal or the IDE

#### Scenario: Lost bindings
- **WHEN** the conflict has lost bindings
- **THEN** the line above says that git merges the commits but the result would not compile, and never to rebase in a terminal
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the bindings a clean replay loses and copy a prompt that keeps them`)
