## ADDED Requirements

### Requirement: Pick: An update that the words resolved lands nothing

An update after a conflict that applies at once because its words resolved a conflict of git (`update.resolved`) MUST
land nothing, with or without `pick_requires_check`: a merge of words is a guess that no build saw. The answer stays
`state: conflict` (exit 1) with the `update`, and its `conflict.next` MUST name the files the words merged
(`update.verify.paths`) and say to build and run the tests in the lease, then `brd lease pick` again.

#### Scenario: An update that the words resolved
- **WHEN** the update applies at once because its words resolved the conflict
- **THEN** nothing lands, the lease is on the tip with the merged files, and `conflict.next` names them and says to
  build and test, then pick again
- **AND** the next `brd lease pick` lands the merged tree
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_whose_update_resolves_by_words_lands_nothing_and_names_the_files`)

## MODIFIED Requirements

### Requirement: Pick: A conflict lands nothing and comes back to the lease

A conflict MUST land nothing (exit 1, `state: conflict`) and come back to the lease: the lease is updated onto the tip
of its home branch without a fetch (`update`, what `brd update` prints, [brd update](../update/spec.md)), and the
conflict there opens a resolve session in the lease. While the session is open a `brd lease pick` MUST refuse
(`resolve_in_progress`, exit 4). The output is `lease_pick` of `brd schema`. Only an update that resolved no content
MUST be picked again at once.

#### Scenario: A pick that conflicts
- **WHEN** the commits of the lease conflict with the tip of the home branch
- **THEN** nothing lands, and a resolve session opens in the lease, `conflict.next` saying so
- **AND** the lease moves only when `brd update --continue` finishes it, whose `next` is `brd -C <lease> lease pick`
  again, and `brd update --abort` closes it
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_pick_that_conflicts`)

#### Scenario: An update that applies at once
- **WHEN** the update applies at once and resolved no content: it dropped the conflicting commit as present
- **THEN** it is picked again at once and the rest lands in the same call
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_whose_update_drops_the_conflicting_commit_lands_in_one_call`)

### Requirement: Pick: An update that applies after a conflict waits for a check of the new tree

With the setting, an update after a conflict that applies at once without a merge of words (which lands nothing in any
case) moves the lease onto a new tree: the pick again MUST run only when a record names that tree too or carries over
onto it, else the answer stays `state: conflict` (exit 1) with the `update`, the `check` of the new tree, and a
`conflict.next` that says to build and test in the lease, then pick again.

#### Scenario: A new tree without a record
- **WHEN** an update that applies at once after a conflict leaves a tree no record names
- **THEN** the answer stays `state: conflict` with the `check` of the new tree
  [@test] crates/bd-lease/src/lease_pick_test.rs (`an_update_that_applies_after_a_conflict_waits_for_a_check_of_the_new_tree`)
