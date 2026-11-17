# Tasks

- [x] The dispatcher marks the task of a closed session, which ends at its next turn (`SessionTask::closed`, `SessionFacts::run`)
- [x] `a_session_that_closes_while_a_read_is_in_flight_stops_its_timers_at_the_close` in `scheduler_test.rs`
- [x] docs/design/git-operations.md, the scheduler of the facts
- [x] `the_answer_of_a_read_in_flight_at_the_close_asks_nothing_after_it`: an answer before any timer asks nothing
- [x] The reads of a task take tickets of one epoch (`TaskPort`), `a_read_of_a_task_takes_no_ticket_of_another_epoch_of_its_session`
- [x] `Actor::reopened` always emits `Closed`, `a_window_back_during_the_run_before_a_new_lease_hears_the_session_close_again_as_it_opens_anew`
