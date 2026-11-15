## ADDED Requirements

### Requirement: The check of the tree: A pick that lands nothing follows without a check

With the setting, a pick from a worktree whose commits the branch has already MUST land nothing and MUST need no green
record: with `--follow` it moves the worktree onto the tip, and `check` names the state of the record. Each check of
the pick, before the lock and again under it, MUST refuse a pick that would land a commit or that conflicts as
`unchecked_tree` (exit 4), with nothing landed.

#### Scenario: A worktree with nothing to land
- **GIVEN** the setting, a worktree whose commits are all on the branch, and a tree that no green record names
- **WHEN** the branch gets a new commit and `brd pick --from <dir> --follow` runs
- **THEN** the pick is `up_to_date`, `followed.state` is `moved`, and `check.state` is `missing`
- **AND** after a new commit in the worktree the same pick is `unchecked_tree`, and nothing lands
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_that_lands_nothing_follows_without_a_check`)

## MODIFIED Requirements

### Requirement: The check of the tree: A pick from a worktree lands only a tree a green check recorded

With the setting in the checkout the pick runs in (`-C`), `brd pick --from <dir>` MUST land only a tree of HEAD of the
worktree of `dir` that a green record of its `bd-checks.jsonl` names, as `brd lease pick` does: else a pick that would
land a commit lands nothing, `unchecked_tree` (exit 4).

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
