# Tasks

- [x] `sync_utils::dir_watch` (`WatchedDir::wake_of`, `Fingerprints`, `RECHECK`, `WatchConfig`) with its tests
- [x] `registry_watch`: a look at every event and every `RECHECK`; `Registry::look`; the tests of the guarantee (no
  event), of the reaction to given events, and of the events of the system with a deadline
- [x] `app_settings`: the event rule and the looks of the writer; the tests of the event path and of the guarantee
- [x] `TableWake::wait` on `WatchedDir::wake_of`, answering whether an event ended it; its test
- [x] The stamp fixture of the E2E server: a look of the watching sessions, the answer of the session of the window
- [x] The measurement of a look (0.14 ms at a load of 64); docs: async-patterns ("A watch is a hint", the table of the
  watches), git-operations, rust-tests
