## MODIFIED Requirements

### Requirement: Copies: The copies are in `branch-deck/resolve/r<n>/` under flat names

The copies MUST be in `branch-deck/resolve/r<n>/` of the worktree's git dir: `edit/`, `before/`, `onto/` and
`commit/`, each file under the flat name `<key>-<basename>`, and `commit.patch`. Two paths that differ only in case
MUST get distinct names. In a slot of the lease pool whose `notes.local` links to the notes of its lease, a session
without `--dir` MUST put them into `resolve/` of those notes instead, by the path the link resolves to, and the finish
and the abort MUST remove them there.

#### Scenario: Flat names that never collide
- **WHEN** the copies of a round are written
- **THEN** each file has the flat name `<key>-<basename>`, and two paths that differ only in case get distinct names
  [@test] crates/git-ops/src/update_resolve_test.rs (`copies_have_flat_names_that_never_collide`)

#### Scenario: The copies of a session in a lease
- **WHEN** a pick of a lease conflicts and opens a resolve session in the lease, and the session finishes or is aborted
- **THEN** the copies are in `resolve/` of the notes of the lease, in the pool directory, not in `.git`, and the finish
  or the abort removes them
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)
  [@test] crates/bd-lease/src/lease_pick_test.rs (`an_update_in_a_lease_is_refused_and_its_session_is_closed_with_abort`)
