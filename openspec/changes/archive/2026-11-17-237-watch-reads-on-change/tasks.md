# Tasks

- [x] `operations.json` is in the `Fingerprints` of the registry watch; a look reads it when its stat changed, at the
  first look, at the look a subscription or a focus asks (`Wake::read`), after a read that failed and once its last
  read is `recheck` old, the wait for the next look ending then at the latest
- [x] `Watches::given`: what reached the actor of a session is what a read compares with
- [x] An open, a read or a close without a write wakes no waiter (`dir_watch::changes_nothing`)
- [x] `a_look_reads_the_registry_only_once_it_changed_a_session_joined_or_its_last_read_is_a_recheck_old`,
  `a_change_no_stat_tells_shows_a_recheck_after_the_last_read_though_a_look_of_an_event_came_between`,
  `a_read_of_the_registry_that_failed_is_taken_again_at_the_next_look`,
  `a_read_before_the_session_opened_records_nothing_as_given_so_the_subscription_shows_it`,
  `an_open_a_read_or_a_close_without_a_write_wakes_no_waiter`,
  `the_reads_of_a_file_of_the_directory_wake_no_waiter_on_the_watch_of_the_system`
- [x] docs/design/git-operations.md ("The watch") and docs/design/async-patterns.md ("A watch is a hint")
