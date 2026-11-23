## ADDED Requirements

### Requirement: Pick: A check carries over a clean update

A stale record MUST count for the tree of HEAD when it carries over a clean update onto the home branch: the newest
record's tree (not its HEAD, as it may hold the changes uncommitted then), merged by `git merge-tree --write-tree
--merge-base=<B>` onto the merge base of HEAD with the home branch, B the merge base of the record's HEAD with it, is
`HEAD^{tree}`. Then `check.state` MUST be `carried_over`, with the record and `untested_master_commits`, the commits of
the home branch from B to that base, which no check saw. The test MUST be the tree, exact for binaries and whitespace:
an update that needed a resolution, a change after the update, an edit on the base of the record, a record without its
HEAD, or a merge that conflicts carries nothing and stays `stale`. `check_carries_over = false` in `[lease]` of
`.branchdeck.toml` of the home checkout MUST turn it off; it is true by default.

#### Scenario: A clean update of the lease
- **GIVEN** `pick_requires_check = true`, and a green record of the lease's tree
- **WHEN** the home branch gets a commit, the lease is updated onto it without a conflict, and `brd lease pick` runs
- **THEN** the tree lands, and `check` says `carried_over` with `untested_master_commits: 1`
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_lands_a_check_that_carries_over_a_clean_update`)
  [@test] crates/lease-pool/src/check_record_test.rs (`a_record_carries_over_a_clean_update_onto_the_target`)

#### Scenario: A tree the merge of git does not make
- **WHEN** the update needed a resolution, a binary byte or a whitespace changed after the update, the lease was edited
  on the base of the record, or the record names no HEAD
- **THEN** the record stays `stale`
  [@test] crates/lease-pool/src/check_record_test.rs (`a_record_carries_over_a_clean_update_onto_the_target`)

#### Scenario: The changes uncommitted at the record's HEAD
- **GIVEN** a record whose tree holds changes that were uncommitted at its HEAD, committed since
- **WHEN** the lease is updated onto the home branch without a conflict
- **THEN** the record carries over
  [@test] crates/lease-pool/src/check_record_test.rs (`a_record_carries_over_a_clean_update_onto_the_target`)

#### Scenario: The carry turned off
- **WHEN** `.branchdeck.toml` sets `check_carries_over = false` in `[lease]`
- **THEN** no record carries over; without the key it is true, and a value that is no boolean is an error of the file
  [@test] crates/lease-pool/src/check_record_test.rs (`check_carries_over_is_a_boolean_true_by_default`, `a_record_carries_over_a_clean_update_onto_the_target`)

## MODIFIED Requirements

### Requirement: Pick: A tree no record names lands nothing

When no record names `HEAD^{tree}` of the lease and none carries over onto it, nothing MUST land: `unchecked_tree`
(exit 4), whose message names the tree and HEAD, the last check (its tree, time and command, how many files changed
since and the first 10 of them, left out when git no longer has that tree) and both ways on. A lease with nothing to
land MUST follow its home branch without a record ([brd pick](../pick/spec.md#the-check-of-the-tree)).

#### Scenario: An unchecked tree
- **GIVEN** `pick_requires_check = true`
- **WHEN** no record names `HEAD^{tree}` of the lease
- **THEN** the pick refuses with `unchecked_tree` and names the last check
- **AND** the message names both ways on: build and test, then `brd -C <lease> lease pick` again, or
  `brd -C <lease> lease pick --allow-unchecked '<reason>'`
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_tree_that_no_green_check_recorded`)

#### Scenario: A lease with nothing to land
- **GIVEN** `pick_requires_check = true`, and a lease with no commit that its home branch lacks
- **WHEN** no record names `HEAD^{tree}` of the lease, and `brd lease pick` runs
- **THEN** the lease follows its home branch onto the tip, with `check` in the answer
  [@test] crates/bd-lease/src/workspace_test.rs (`a_lease_with_nothing_to_land_follows_a_build_config_change_without_a_check`)

### Requirement: Pick: A dry run reports the check of the tree

`--dry-run` MUST report the check of the tree in `check` and never refuse for it, with or without the setting. With
the setting, a pick MUST report `check` too.

#### Scenario: The check of a dry run
- **WHEN** a pick runs with `--dry-run`
- **THEN** `check` has `state`: `ok`, a record names the tree; `carried_over`, the newest record carries over a clean
  update onto it, with `untested_master_commits`; `stale`, records exist and none names it or carries over; `missing`,
  no record
- **AND** `record`: the newest record of the tree, else the newest one, with `changed_files` and the first 10 of them
  in `changed_paths` for a stale tree
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_tree_that_no_green_check_recorded`, `a_lease_pick_lands_a_check_that_carries_over_a_clean_update`)

### Requirement: Pick: An update that applies after a conflict waits for a check of the new tree

With the setting, an update after a conflict that applies at once moves the lease onto a new tree: the pick again MUST
run only when a record names that tree too or carries over onto it, else the answer stays `state: conflict` (exit 1)
with the `update`, the `check` of the new tree, and a `conflict.next` that says to build and test in the lease, then
pick again. An update whose words resolved the conflict writes a tree that the merge of git does not, so the record of
the tree before it stays `stale`.

#### Scenario: A new tree without a record
- **WHEN** an update that applies at once after a conflict leaves a tree no record names
- **THEN** the answer stays `state: conflict` with the `check` of the new tree
  [@test] crates/bd-lease/src/lease_pick_test.rs (`an_update_that_applies_after_a_conflict_waits_for_a_check_of_the_new_tree`)

### Requirement: Pick: The check costs one rev-parse

The check MUST cost one `rev-parse` (HEAD and its tree). For a stale tree whose newest record names its HEAD, with the
carry on, it MUST add two `merge-base` (of the record's HEAD and of HEAD with the home branch) and, when they differ,
one `merge-tree`; then one `rev-list --count` when the record carries over, else one `diff-tree`, as for any other
stale tree. Without the setting and without --dry-run the pick MUST run none of them.

#### Scenario: The git of the check
- **WHEN** a lease pick checks its tree, or runs without the setting
- **THEN** it runs one `rev-parse` and for a stale tree one `diff-tree`, or neither
- **AND** for a record that carries over two `merge-base`, one `merge-tree` and one `rev-list --count`, and no
  `diff-tree`
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_tree_check_of_a_lease_pick`, `a_lease_pick`)
