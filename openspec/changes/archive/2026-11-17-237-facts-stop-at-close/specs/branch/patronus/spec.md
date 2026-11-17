## MODIFIED Requirements

### Requirement: Load on Patronus: The session lists and polls the robots, not the page

The session of the repository MUST list and poll the robots, not the page: one scheduler of the facts of the sessions
on the clock of the sessions (`remote_facts::scheduler`, by the rules of `domain_policy::refresh`), one list or poll in
flight per session, nothing for a session no window shows.

#### Scenario: A session no window shows
- **WHEN** no window watches a session, or the session is closed
- **THEN** the scheduler asks nothing for it
- **AND** a refresh of a closed session is answered with none
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_nothing_for_a_session_no_window_watches`, `a_closed_session_asks_nothing_and_answers_a_refresh_with_none`)

#### Scenario: A session that closes while a read of it is in flight
- **WHEN** a session closes while a window still watches it and a list or a read of it is in flight
- **THEN** its timers stop at the close: no poll, list or read of it is asked after it, of Patronus, GitHub or the
  review requests, also before the read in flight answered
- **AND** the answer of that read goes to no task, which asks nothing of what it tells, and the refresh it served is
  answered with none
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_session_that_closes_while_a_read_is_in_flight_stops_its_timers_at_the_close`, `the_answer_of_a_read_in_flight_at_the_close_asks_nothing_after_it`)

#### Scenario: A read of a session opened anew under its key
- **WHEN** the session of a task opened anew under the same key (a slot leased again) and a read of that task takes its
  ticket of the new epoch
- **THEN** the read is refused before it asks anything, and the refresh it served is answered with none: the reads of a
  task take their tickets only of the epoch of its first one, the new epoch has a task of its own
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_read_of_a_task_takes_no_ticket_of_another_epoch_of_its_session`)

#### Scenario: One request in flight per session
- **WHEN** a list or a poll of a session is in flight
- **THEN** no other list or poll of that session starts
