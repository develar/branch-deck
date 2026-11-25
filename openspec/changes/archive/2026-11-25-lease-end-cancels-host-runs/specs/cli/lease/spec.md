## ADDED Requirements

### Requirement: Return: A return cancels the runs of brd-host that the lease leaves

Every end of a lease (a return, `pick --return`, a takeover, a reclaim) MUST cancel each run of brd-host of the slot
without an end (`<run>.host` without `<run>.end.json` in its run records) before it changes the slot, through
`brd-host cancel-unfinished --json --max 5s` beside the binary. A host that does not answer MUST hold the end for at
most 8 s; the end MUST go on and name the runs in `cancelled_runs` and `uncancelled_runs`.

#### Scenario: A return with runs that have no end
- **GIVEN** the slot of a lease has a run that ended and two runs without an end
- **WHEN** the lease is returned, alone or by `brd lease pick --return`
- **THEN** the return runs `brd-host cancel-unfinished --json --max 5s` in the slot before it takes the notes out of it
- **AND** `cancelled_runs` names each cancelled run with its host and the answer of its agent, and `uncancelled_runs`
  each other run with its host, its code and its message, and the slot is free
  [@test] crates/bd-lease/src/lease_host_runs_test.rs (`a_return_cancels_the_runs_of_brd_host_that_the_lease_leaves_before_it_changes_the_slot`, `a_pick_with_return_cancels_the_runs_that_the_lease_leaves`)
  [@test] crates/lease-pool/src/host_cancel_test.rs (`the_unfinished_runs_are_the_records_of_a_host_without_an_end`, `the_cancel_runs_brd_host_in_the_slot_only_for_a_run_without_an_end`)

#### Scenario: Only the runs without an end are cancelled on a real agent
- **GIVEN** a lease whose slot has a run that ended, a run that waits on an agent with no client, and a run of a host
  that takes the connection and never answers
- **WHEN** the lease is returned
- **THEN** only the waiting run is cancelled, and it ends on its host; the record of the ended run stays
- **AND** the silent host keeps its run with `timed_out`, and the return ends within the bound of the cancel
  [@test] crates/bd-host/tests/cli.rs (`a_return_of_a_lease_cancels_only_its_runs_without_an_end_and_a_silent_host_holds_it_within_the_bound`)

#### Scenario: A takeover and a reclaim
- **WHEN** a get takes over an idle lease, or `brd lease reclaim --idle` returns one, whose slot has runs without an end
- **THEN** the end cancels them as a return does, and `took_over` of the get or the entry of `reclaimed` names them in
  `cancelled_runs` and `uncancelled_runs`
  [@test] crates/bd-lease/src/lease_host_runs_test.rs (`a_takeover_and_a_reclaim_cancel_the_runs_that_the_old_lease_leaves`)

#### Scenario: A slot without such a run
- **WHEN** the runs of the slot all ended, or the slot has no run records
- **THEN** the end starts no `brd-host`, and its answer has no `cancelled_runs` and no `uncancelled_runs`
  [@test] crates/bd-lease/src/lease_host_runs_test.rs (`a_return_cancels_the_runs_of_brd_host_that_the_lease_leaves_before_it_changes_the_slot`)
  [@test] crates/lease-pool/src/host_cancel_test.rs (`the_cancel_runs_brd_host_in_the_slot_only_for_a_run_without_an_end`)

#### Scenario: No brd-host, or one that fails
- **WHEN** no `brd-host` is beside the binary, the child does not end within its `--max` and 3 s, or it answers no
  JSON
- **THEN** each run without an end stays in `uncancelled_runs` with `no_brd_host`, `timed_out` or `brd_host_failed`
  and the command `brd-host cancel <host> <run>`, and the end goes on
  [@test] crates/lease-pool/src/host_cancel_test.rs (`without_a_brd_host_the_runs_stay_with_the_command_that_cancels_each`, `a_brd_host_that_hangs_or_answers_no_json_leaves_each_run`)

#### Scenario: A process that inherits the pipes of the child
- **WHEN** the child exits while a process that it or another thread of the caller started holds its stdout open
- **THEN** the end reads the pipes for a moment after the exit, not to their end, and goes on
  [@test] crates/lease-pool/src/host_cancel_test.rs (`a_process_that_inherits_the_pipes_of_brd_host_holds_no_end_of_a_lease`)

#### Scenario: A record that cannot be read
- **WHEN** the run directory of the slot, or a `<run>.host` in it, cannot be read
- **THEN** the end does not count the slot as one without runs: `uncancelled_runs` names it with `records_unreadable`
  (the run and the host empty when the directory cannot be read), and the readable records are cancelled beside it
- **AND** a slot without the run directory has no runs
  [@test] crates/lease-pool/src/host_cancel_test.rs (`an_unreadable_record_stays_as_records_unreadable`, `the_unfinished_runs_are_the_records_of_a_host_without_an_end`)

#### Scenario: The brd-host of a binary
- **WHEN** brd, the app or the agent of a host ends a lease
- **THEN** the `brd-host` is the running binary when it is `brd-host`, else the `brd-host` beside its real path
  [@test] crates/lease-pool/src/host_cancel_test.rs (`the_brd_host_of_a_binary_is_itself_or_the_one_beside_it`)

## MODIFIED Requirements

### Requirement: Get: A takeover is a return of the old lease by the pool

The takeover MUST be a return of the old lease by the pool: the runs of brd-host that it leaves cancelled, its seeded
paths removed, HEAD detached, the clean rules of the source applied into the trash, which the get leaves for the next
return or clean to delete (the trash of a run that is gone), then the move of a reused slot.

#### Scenario: The slot of a takeover
- **WHEN** a get takes over an idle lease
- **THEN** the runs of brd-host of the slot without an end are cancelled, the seeded paths are removed, HEAD is
  detached and the clean rules of the source go into the trash
- **AND** the get leaves the trash for the next return or clean and moves the slot as a reused one
  [@test] crates/bd-lease/src/lease_host_runs_test.rs (`a_takeover_and_a_reclaim_cancel_the_runs_that_the_old_lease_leaves`)
