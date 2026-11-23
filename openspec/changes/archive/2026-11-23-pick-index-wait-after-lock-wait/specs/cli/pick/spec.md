## MODIFIED Requirements

### Requirement: The lock: An index lock that appears under the repository lock sends the pick back

An index lock that appears while the pick takes the repository lock MUST make it release the repository lock and send
the pick back to check and wait outside it. A check under the lock (after a wait for another holder whose witness
missed, or after the checks without the lock ran out) whose moves meet the index lock of another process MUST release
the repository lock, wait for the index lock outside it and check under the lock again.

#### Scenario: An index lock appears while the pick takes the repository lock
- **WHEN** an index lock appears while the pick takes the repository lock
- **THEN** the pick releases it, checks again and waits for the index lock outside it

#### Scenario: An index lock meets the check under the lock after a wait
- **GIVEN** another process holds the repository lock and moves the target, and the `git status` of an IDE holds the
  index lock of the target when it releases the repository lock
- **WHEN** the pick checks under the repository lock after that wait
- **THEN** it releases the repository lock, which another writer takes meanwhile, waits for the index lock outside it,
  checks under the lock again once it is gone and lands on the new tip
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_that_meets_an_index_lock_under_the_lock_after_a_wait_waits_for_it_outside_the_lock`)
