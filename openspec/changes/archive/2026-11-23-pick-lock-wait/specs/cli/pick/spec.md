## ADDED Requirements

### Requirement: The lock: A pick waits for the repository lock while its holder lives

`brd pick`, `brd lease pick` and the update of a lease after its conflict MUST wait for the repository lock while the
record of its holder names a live process, naming the holder on stderr at once and every 10 s, up to `--lock-wait
<secs>` in all (540 by default). Past it the answer MUST be `locked` (retryable, exit 3), naming the bound, nothing
changed.

#### Scenario: A live holder of the lock
- **GIVEN** another process holds the repository lock
- **WHEN** a pick with `--follow` reaches the lock
- **THEN** it waits, its steps name the wait and the holder, and it lands once the holder released the lock
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_waits_for_a_live_holder_of_the_lock_and_lands_after_it`)

#### Scenario: A holder past the bound
- **GIVEN** another process holds the repository lock for longer than `--lock-wait`
- **WHEN** a pick reaches the lock
- **THEN** it answers `locked`, retryable, exit 3, naming the bound, and nothing moved
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_whose_lock_stays_held_past_its_lock_wait_is_locked_and_retryable`)

### Requirement: The lock: A witness that missed after a wait checks under the lock at once

A pick whose witness missed after it waited for another holder of the repository lock MUST check again under that lock
at once, without a round outside it: the holder was most likely another pick, which moved the target.

#### Scenario: The target moved while the pick waited
- **GIVEN** another process holds the repository lock and moves the target before it releases it
- **WHEN** the pick takes the lock after that wait
- **THEN** it checks again under the lock and lands on the new tip, the source following there
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_whose_target_moved_while_it_waited_for_the_lock_checks_under_it_at_once`)
