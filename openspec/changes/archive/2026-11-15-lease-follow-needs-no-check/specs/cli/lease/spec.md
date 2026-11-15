## MODIFIED Requirements

### Requirement: Pick: A tree no record names lands nothing

When no record names `HEAD^{tree}` of the lease, nothing MUST land: `unchecked_tree` (exit 4), whose message names the
tree and HEAD, the last check (its tree, time and command, how many files changed since and the first 10 of them, left
out when git no longer has that tree) and both ways on. A lease with nothing to land MUST follow its home branch
without a record ([brd pick](../pick/spec.md#the-check-of-the-tree)).

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
