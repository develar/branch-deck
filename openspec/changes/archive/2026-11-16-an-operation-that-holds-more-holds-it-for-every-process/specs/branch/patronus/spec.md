## ADDED Requirements

### Requirement: Operations: What an operation holds more, it holds for every process

What an operation holds more once it runs MUST go into its record in the operations registry of the repository before
it is held in the process: an operation of another process that holds one of them MUST make it `busy`, as no process
waits for another, its record as it was; else `brd` is `busy` where it needs what the wider record holds.

#### Scenario: A record of another process, and `brd` against the wider record
- **WHEN** an operation of the app needs a ref a record of `brd` holds, then another one
- **THEN** the first is `busy` and its record unchanged; the second goes into its record, and an admission of `brd` that needs it is refused by that record
  [@test] crates/operations/src/registry_test.rs (`what_an_operation_needs_once_it_runs_goes_into_its_record_unless_another_process_holds_it`)

### Requirement: Operations: An operation that holds more never waits for one that waits for it

An operation that needs more once it runs MUST be admitted again at the end of the line over the operations of the
process but those that wait in the queue for it, directly or behind one that does; those MUST follow it in their
order: no wait goes round, none of them runs while it holds what they need. A hold that waits gives its place up (one
that waited behind another for what it held may run first): it comes before the first effect, and is decided again.

#### Scenario: Three operations
- **WHEN** x holds r1, w1 waits behind it for r1 and r3, w2 behind w1 for r3 and r2, and x needs r2
- **THEN** x goes on at once, w1 and w2 wait on through the end of another operation, then run after x in their order
  [@test] crates/operations/src/registry_test.rs (`an_operation_that_needs_more_never_waits_for_the_ones_that_wait_for_it_and_they_run_after_it`)

#### Scenario: A hold that goes on at once
- **WHEN** x holds r1, w1 waits behind it for r1, x needs r2 that nobody holds, and another operation ends
- **THEN** w1 waits on behind x, and runs once x ended
  [@test] crates/operations/src/registry_test.rs (`after_a_hold_that_goes_on_at_once_the_end_of_another_operation_runs_none_that_waits_for_it`)

#### Scenario: Only the ones in the queue follow
- **WHEN** an operation queued behind x inside its wait for the rewrites before it runs already as x is admitted again
- **THEN** it is no follower of x and keeps its place
  [@test] crates/operations/src/registry_test.rs (`only_the_operations_that_wait_in_the_queue_follow_one_admitted_again`)

#### Scenario: What another holds
- **WHEN** x needs r2 that y holds
- **THEN** x waits behind y, its step `queued`, and goes on once y ended, saying it waited
  [@test] crates/operations/src/registry_test.rs (`an_operation_that_needs_what_another_holds_waits_for_it_and_says_so`)

#### Scenario: A hold that waits gives its place up
- **WHEN** x holds r1, q waits behind a later y for r1 and r2, and x needs r3 that z holds
- **THEN** x waits behind q, q runs once y ended, then x after z
  [@test] crates/operations/src/registry_test.rs (`a_hold_that_waits_lets_one_that_waited_behind_another_for_what_it_held_run_first`)
