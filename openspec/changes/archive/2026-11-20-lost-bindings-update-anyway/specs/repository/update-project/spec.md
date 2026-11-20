## ADDED Requirements

### Requirement: Check: The check of every sync reads the lost bindings of its replay

The check inside every sync (and of `brd update --dry-run`) MUST read the lost bindings of its clean replay as the
update does, so the header says the conflict the update would answer, never "Rebases cleanly". What the rule found is
kept with the replay in the preview cache: a check of the same state asks it no more.

#### Scenario: The incident before a click
- **WHEN** a sync checks the update of a commit that drops a property and an import the upstream uses again
- **THEN** the check is a conflict with both lost bindings, and the button says so before a click
  [@test] crates/update-core/src/update_project_test.rs (`the_check_of_a_sync_finds_the_lost_bindings_of_the_replay`)
  [@test] crates/update-core/src/process_budget_test.rs (`the_check_of_a_sync_reads_the_lost_bindings_once_per_state`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the bindings a clean replay loses and copy a prompt that keeps them`)

### Requirement: Check: The rule reads nothing when no file the update changes is a local one

The lost-binding rule MUST be asked only when a file the update changes is one a local commit changes: only such a
file can differ between a copy and its original. The paths of the local commits come with the `log` of the local
commits the pairing reads (`--name-only -z --no-renames`), so an update or a sync whose upstream changes no file of the
local commits runs no process more.

#### Scenario: No file in common
- **WHEN** the upstream changes no file the local commits change
- **THEN** the check and the update run the processes they ran without the rule
  [@test] crates/update-core/src/process_budget_test.rs (`the_check_of_a_sync_whose_upstream_changes_no_local_file_asks_no_rule`, `an_update_after_its_fetch`)
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_local_commits_come_with_the_paths_they_change`)

### Requirement: Conflict and blocked dialog: Update Anyway keeps the lost bindings and updates

A conflict of lost bindings MUST offer "Update Anyway": a name may resolve another way, which only the build proves.
It updates without a fetch onto the baseline the dialog shows, its lost bindings kept unbound on the user's word
(`Intent::UpdateProject::keep`, recorded in the journal with the intent); a lost binding of another name refuses it
again, and a moved branch or baseline is checked again. The toast and Activity name the kept names.

#### Scenario: Update Anyway
- **WHEN** the user presses Update Anyway in the dialog of the lost bindings of `update_lost_bindings`
- **THEN** the dialog closes, HEAD moves onto the baseline, and the toast says "Kept backend and rethrow unbound"
  [@test] tests/e2e/update-project/update-project.spec.ts (`should update anyway, keeping the lost bindings the dialog names`)
  [@test] src/lib/landed.test.ts (`names the lost bindings Update Anyway kept unbound`)

#### Scenario: Another name lost
- **WHEN** the update keeps one name of two
- **THEN** it is a conflict naming the other, and nothing moves
  [@test] crates/update-core/src/update_project_test.rs (`update_anyway_applies_and_names_the_bindings_it_kept`)

## MODIFIED Requirements

### Requirement: Header: A conflict of lost bindings says the commits remove names in use

The words of a conflict with lost bindings (the tooltip, the entry of Activity) MUST say "Your commits remove names that
origin/master uses in src/Launch.kt", short "removes names in use", never that the files conflict; the tooltip says
it before a click, from the check of the sync.

#### Scenario: Lost bindings
- **WHEN** the state is a conflict with lost bindings
- **THEN** its words are "Your commits remove names that origin/master uses in src/Launch.kt", short "removes names in use"
  [@test] src/lib/update-state.test.ts (`says that a replay that loses bindings removes names the upstream uses, not that it conflicts`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should name the bindings a clean replay loses and copy a prompt that keeps them`)
