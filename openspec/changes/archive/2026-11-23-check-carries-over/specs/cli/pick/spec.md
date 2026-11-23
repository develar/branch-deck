## MODIFIED Requirements

### Requirement: The check of the tree: A pick from a worktree lands only a tree a green check recorded

With the setting in the checkout the pick runs in (`-C`), `brd pick --from <dir>` MUST land only a tree of HEAD of the
worktree of `dir` that a green record of its `bd-checks.jsonl` names, or that its newest record carries over onto by a
clean update onto the branch checked out in `-C`, as `brd lease pick` does
([brd lease](../lease/spec.md#pick)): else a pick that would land a commit lands nothing, `unchecked_tree` (exit 4).

#### Scenario: A tree that no green record names
- **GIVEN** the setting in the checkout of `-C`, a commit of the worktree of `dir` that the branch lacks, and a tree of
  its HEAD that no green record of its `bd-checks.jsonl` names
- **WHEN** `brd pick --from <dir>` runs
- **THEN** nothing lands, `unchecked_tree` (exit 4), whose message starts `brd pick:`, names the tree, HEAD and the
  last check
- **AND** the message names both ways on with this pick again: `brd pick --from <dir> [--follow]`, then the same with
  `--allow-unchecked '<reason>'`
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

#### Scenario: A tree that a green record names
- **GIVEN** the setting, and a green record of `bd-checks.jsonl` that names the tree of HEAD of the worktree
- **WHEN** `brd pick --from <dir>` runs
- **THEN** the tree lands
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

#### Scenario: A record that carries over a clean update
- **GIVEN** the setting, and a green record of the worktree's tree before a clean update onto the branch
- **WHEN** `brd pick --from <dir>` runs
- **THEN** the tree lands, `check.state` `carried_over` with `untested_master_commits`
- **AND** with `check_carries_over = false` in the checkout of `-C` the same tree is `unchecked_tree`
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_a_check_that_carries_over_a_clean_update`)

### Requirement: The check of the tree: The check reads git once, nothing without the setting

The check MUST be one `rev-parse` of the git dir, HEAD and its tree, and for a stale tree the git of the carry of its
newest record ([brd lease](../lease/spec.md#pick)) and one `diff-tree` unless it carries over; without the setting and
without --dry-run the pick MUST read neither the records nor git for it.

#### Scenario: The processes of the tree check
- **WHEN** a pick with the setting checks the tree
- **THEN** it runs one `rev-parse` of the git dir, HEAD and its tree, and one `diff-tree` for a stale tree
- **AND** for a record that carries over two `merge-base`, one `merge-tree` and one `rev-list --count`, and no
  `diff-tree`
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_tree_check_of_a_lease_pick`)

#### Scenario: The check of a dry run without a record
- **WHEN** `--dry-run` of `--from` reports the check of a worktree that has no record
- **THEN** the check runs one `rev-parse` of the git dir, HEAD and its tree, and no `diff-tree`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick_into_a_branch_the_target_has`)

#### Scenario: A pick without the setting
- **WHEN** a pick without the setting and without --dry-run runs
- **THEN** it reads neither the records nor git for the check
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick`)

### Requirement: The check of the tree: A dry run of a worktree reports the check

`--dry-run` of `--from` MUST report the check in `check` (`state` `ok`, `carried_over`, `stale` or `missing`,
`record`), with or without the setting, and MUST never refuse for it.

#### Scenario: A dry run of a worktree
- **WHEN** `--dry-run` of `--from` runs, with or without the setting
- **THEN** it reports the check in `check`: its `state` `ok`, `carried_over`, `stale` or `missing`, and the `record`
- **AND** it does not refuse for it
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)
