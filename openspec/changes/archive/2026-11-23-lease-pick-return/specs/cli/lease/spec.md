## MODIFIED Requirements

### Requirement: Get: The answer names the home of the lease and the next commands

The answer of a get MUST name where the commits of the lease land, `home`: `checkout` (the source checkout, its top
level) and `branch` (the branch it had checked out, `null` for a detached HEAD), its `notes` ("Notes" below), and
`next`, what the session runs in order.

#### Scenario: A lease with a home branch
- **WHEN** a get leases a slot from a checkout with a branch checked out
- **THEN** `next` is `brd -C <path> lease pick` after each commit that builds and passes its tests, then
  `brd -C <path> lease pick --return` once the task is done, then `brd lease notes --lease-id <id>`, which finds the
  notes of the lease after the return
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`)

#### Scenario: No branch in home
- **WHEN** `home` has no branch
- **THEN** there is nothing to pick onto, and `next` is `brd lease return --lease-id <id>` and the notes command
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`)

## ADDED Requirements

### Requirement: Pick: Return lands the commits and returns the lease in one call

`brd lease pick --return` MUST be the pick without the follow and, once it landed the commits or found them all on the
branch, the return of the lease in the same process (`returned`), after the pick released the repository lock. The
commits of the pick's range MUST be landed by it (`proof: picked`). A refusal of the return MUST keep the lease
(`return_refused`, the exit code of its code), a conflict too; `source_head` names the HEAD the pick read.

#### Scenario: Commits to land
- **WHEN** `brd lease pick --return` runs in a lease with commits its home branch lacks
- **THEN** they land, the lease does not follow (no `followed`: no dry run, refresh or move of its index), and the slot
  is free at `source_head`, the return naming each commit landed by `picked`
- **AND** the undo token has no `+follow`: `brd undo <undo_token>` moves only the home branch back, and `source_head`
  holds the commits
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_with_return_lands_the_commits_and_frees_the_slot`)

#### Scenario: A return that refuses, a conflict
- **WHEN** the lease has local changes, or its commits conflict with the tip of the home branch
- **THEN** the commits land and `return_refused` is `dirty_worktree` (exit 4), its message starting "The commits landed,
  the lease stays", the lease kept; or nothing lands and the conflict comes back to the lease, nothing returned
- **AND** `--dry-run` with `--return` is `invalid_arguments`, and `--stop-own` passes to the return
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_with_return_keeps_the_lease_on_local_changes_and_on_a_conflict`)

#### Scenario: A commit made after the pick
- **WHEN** the HEAD of the lease has a commit beyond the range of the pick
- **THEN** the return checks it as any return does and refuses it as `unlanded_commits`
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_return_after_a_pick_checks_a_commit_made_after_the_pick`)

#### Scenario: The git of a pick with return
- **WHEN** `brd lease pick --return` lands two commits
- **THEN** it runs the git of the pick without the follow, then the git of a return and no proof
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_pick_that_returns`)
