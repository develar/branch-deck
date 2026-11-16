# Tasks

- [x] The watch keeps when the first of the missing looks in a row looked and closes the sessions once they span
  `RECHECK` (`registry_watch::GONE_SPAN`); a test stops the clock of the looks and moves it on (`seam::freeze`,
  `seam::advance`)
- [x] `the_sessions_close_once_the_repository_is_missing_for_a_recheck_not_at_one_look_or_a_burst`: two missing looks
  milliseconds apart keep the sessions, one a `RECHECK` after the first closes them
- [x] The child of the tests of the watch is killed as it drops (`test_utils::killed::Killed`), also when the test
  fails
