## ADDED Requirements

### Requirement: Get: The gets that wait take their slots in the order they asked

A get that finds no free slot MUST enter a queue in the state of the pool. A free slot MUST go to the first waiting get
that has not tried it, an idle lease to the first waiting get that takes idle leases over and has not tried it, and a
get without a ticket MUST come after every waiting get. Only the first waiting get MUST wait for the clones lock. A
busy get and one without a heartbeat for 60 s MUST be passed over and keep their place.

#### Scenario: Three gets that wait
- **GIVEN** a get that adds a slot under the clones lock
- **WHEN** three gets start one after the other and find no free slot, in one process or in processes of their own
- **THEN** they take the clones lock and add their slots in the order they started
- **AND** a get behind another says so on stderr, `waiting behind the get of <holder> (pid <pid>, in the queue for
  <time>), <n> get(s) ahead`, when the wait starts, when the get right ahead changes and once a minute
  [@test] crates/bd-lease/src/clones_lock_test.rs (`three_gets_that_wait_take_the_clones_lock_in_the_order_they_asked`, `gets_of_other_processes_take_the_clones_lock_in_the_order_they_asked`)
  [@test] crates/lease-pool/src/get_queue_test.rs (`the_tickets_wait_in_the_order_they_entered_and_a_get_without_one_comes_last`)

#### Scenario: A freed slot
- **WHEN** a return frees a slot while gets wait
- **THEN** the first waiting get takes it at once, also while another get adds a slot, and the get behind it waits on
- **AND** a new get takes no free slot that a waiting get has not tried: it waits behind that get
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_slot_freed_while_the_head_adds_goes_to_the_next_waiting_get_at_once`, `a_new_get_does_not_pass_a_get_that_waits`)
  [@test] crates/lease-pool/src/get_queue_test.rs (`a_free_slot_goes_to_the_first_ticket_that_has_not_tried_it_and_an_idle_lease_to_one_that_takes_it_over`)

#### Scenario: An idle lease that turns quiet
- **WHEN** an idle lease turns quiet while gets wait and another get adds a slot
- **THEN** the first waiting get that takes idle leases over ends its wait and takes the lease over at once
- **AND** the get behind it does not take that lease
  [@test] crates/bd-lease/src/clones_lock_test.rs (`an_idle_lease_that_turns_quiet_while_gets_wait_goes_to_the_first_waiting_get_at_once`)

#### Scenario: A busy, a stale and a killed get
- **WHEN** a get ahead checks a slot it reserved, frees space without the clones lock, wrote no heartbeat for 60 s, or
  its process is gone
- **THEN** the queue passes it over
- **AND** the ticket of a gone process leaves the queue at the next write of a get, and the others wait at their place
  again by their time of entry once they wait and write their heartbeat
- **AND** a get that waits for a return or a clean under the clones lock is no busy get: it stays waiting and writes
  its heartbeat, so no get behind it takes the slot it waits for
  [@test] crates/bd-lease/src/clones_lock_test.rs (`the_ticket_of_a_killed_get_and_a_stale_one_are_passed_over`, `a_dirty_slot_that_a_return_frees_under_the_lock_leaves_the_head_its_turn`)
  [@test] crates/lease-pool/src/get_queue_test.rs (`a_busy_a_stale_and_a_gone_ticket_are_passed_over_and_only_a_gone_one_leaves`)

#### Scenario: A get that fails in the queue
- **WHEN** a get fails while it waits in the queue
- **THEN** its ticket leaves the queue
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_fails_while_it_waits_leaves_no_ticket`)

#### Scenario: A phase of a later brd
- **WHEN** a later brd writes a ticket with a phase that this brd does not know
- **THEN** this brd reads the state and passes that ticket over as a busy one
  [@test] crates/lease-pool/src/get_queue_test.rs (`a_ticket_of_a_phase_that_a_later_brd_wrote_is_passed_over_as_a_busy_one`)

#### Scenario: A brd without the queue
- **WHEN** a brd that knows no queue writes the state
- **THEN** it drops the queue and takes a free slot past the gets that wait
- **AND** each waiting get writes its ticket again at its next look, with its time of entry
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_waiting_get_writes_its_ticket_again_once_a_brd_without_the_queue_dropped_it`)

## MODIFIED Requirements

### Requirement: Get: A pool adds one slot at a time under the clones lock

A pool MUST add one slot at a time: a get that finds no free slot takes the clones lock (`clones.lock` beside the
state), reserves again under it, and holds it until its new slot is checked out or given back, never across the seed.
A get that holds the lock MUST keep it across the checks of each slot that its reserve under the lock gives it, so it
keeps its turn, and MUST release it once the slot is its own, before its checkout.

#### Scenario: Two gets that must add a slot
- **WHEN** a get must add a slot while another adds one
- **THEN** it waits for the clones lock and reserves again under it, so a slot a return freed meanwhile wins over a
  new one (`reused: true`)
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot`)
  [@test] crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`)

#### Scenario: The seed runs without the lock
- **WHEN** the new slot is checked out
- **THEN** the get releases the clones lock before it seeds the slot
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_seeds_its_new_slot_without_the_clones_lock`)

#### Scenario: A check under the lock finds work
- **GIVEN** a get that holds the clones lock and waits for the return of a slot under it
- **WHEN** the return frees the slot with a local change
- **THEN** the get keeps the lock across the `status` of that slot and adds a slot with no second wait
- **AND** the get behind it takes neither that slot nor the lock first
- **AND** under the lock a get runs one stop of the Bazel servers and one `status` for each free slot it is given, and
  at most two full checks of an idle lease (`IDLE_CHECKS_PER_GET`), each with the stop of the servers of its slot
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_dirty_slot_that_a_return_frees_under_the_lock_leaves_the_head_its_turn`)

### Requirement: Get: A slot freed while a get waits for the clones lock ends the wait

A slot freed while gets wait (by a return, or a get or a clean whose process is gone) MUST end the wait of the first
waiting get that has not tried it within about a second, while the holder still clones: that get reserves the slot
without the lock. A slot the get
already tried (dirty, busy) MUST end no wait again: a dirty one freed meanwhile ends it once, and the get marks it dirty
and waits again.

#### Scenario: A slot freed during the wait
- **WHEN** a return frees a slot while a get waits for the clones lock
- **THEN** the get reserves that slot without the lock
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_slot_freed_while_a_get_waits_for_the_clones_lock_ends_the_wait`)

#### Scenario: A dirty slot freed during the wait
- **WHEN** a dirty slot is freed while a get waits
- **THEN** it ends the wait once, and the get marks it dirty and waits again
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_dirty_slot_freed_while_a_get_waits_ends_the_wait_once`)
